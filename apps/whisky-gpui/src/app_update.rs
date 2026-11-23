use serde::Deserialize;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Check,
    Download,
}

impl Action {
    pub(crate) fn method(self) -> &'static str {
        match self {
            Self::Check => "appUpdate.check",
            Self::Download => "appUpdate.download",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Format {
    Mac,
    AppImage,
    Deb,
    Rpm,
}

impl Format {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Mac => "macos",
            Self::AppImage => "appimage",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
        }
    }

    pub(crate) fn package_command(self, path: &Path) -> Option<String> {
        let manager = match self {
            Self::Deb => "sudo apt install --",
            Self::Rpm => "sudo dnf install --",
            _ => return None,
        };
        Some(format!(
            "{manager} '{}'",
            path.to_str()?.replace('\'', "'\\''")
        ))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct InstalledApp {
    pub(crate) version: String,
    pub(crate) format: Format,
    destination: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Downloaded {
    pub(crate) version: String,
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) checked: bool,
    pub(crate) busy: bool,
    pub(crate) version: Option<String>,
    pub(crate) downloaded: Option<Downloaded>,
    pub(crate) message: Option<String>,
    pub(crate) pending: Option<PendingReplacement>,
}

pub(crate) struct PendingReplacement {
    scratch: PathBuf,
    authorized: bool,
}

impl PendingReplacement {
    pub(crate) fn authorize(mut self) -> io::Result<()> {
        File::create_new(self.scratch.join("approved"))?;
        self.authorized = true;
        Ok(())
    }
}

impl Drop for PendingReplacement {
    fn drop(&mut self) {
        if !self.authorized {
            let _ = fs::remove_dir_all(&self.scratch);
        }
    }
}

pub(crate) struct PreparedReplacement {
    pending: PendingReplacement,
    staged: PathBuf,
    destination: PathBuf,
    format: Format,
}

impl PreparedReplacement {
    pub(crate) fn arm(self) -> io::Result<PendingReplacement> {
        Command::new("/bin/sh")
            .arg("-c")
            .arg(REPLACE_SCRIPT)
            .arg("maltoria-update")
            .arg(std::process::id().to_string())
            .arg(self.staged)
            .arg(self.destination)
            .arg(&self.pending.scratch)
            .arg(self.format.wire())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?;
        Ok(self.pending)
    }
}

impl InstalledApp {
    pub(crate) fn detect() -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        let root = executable.parent()?.parent()?;
        #[cfg(target_os = "macos")]
        {
            let bundle = root.parent()?;
            if bundle.extension()? != "app"
                || executable.file_name()? != "whisky-gpui"
                || plist(bundle, "CFBundleIdentifier").as_deref() != Some("com.daydalek.Maltoria")
            {
                return None;
            }
            Some(Self {
                version: plist(bundle, "CFBundleShortVersionString")?,
                format: Format::Mac,
                destination: Some(bundle.to_path_buf()),
            })
        }
        #[cfg(target_os = "linux")]
        {
            let (manifest, format, destination) = if let Some(path) = std::env::var_os("APPIMAGE") {
                (
                    root.join("input-manifest.json"),
                    Format::AppImage,
                    Some(PathBuf::from(path)),
                )
            } else if root == Path::new("/opt/whisky") {
                if root.join("input-manifest.json").is_file() {
                    (root.join("input-manifest.json"), Format::Rpm, None)
                } else {
                    (root.join("manifest.json"), Format::Deb, None)
                }
            } else {
                return None;
            };
            let data = fs::read(manifest).ok()?;
            let manifest: serde_json::Value = serde_json::from_slice(&data).ok()?;
            Some(Self {
                version: manifest.get("applicationVersion")?.as_str()?.to_owned(),
                format,
                destination,
            })
        }
    }

    pub(crate) fn prepare(&self, update: &Downloaded) -> io::Result<PreparedReplacement> {
        let destination = self.destination.as_ref().ok_or_else(invalid_update)?;
        let parent = destination.parent().ok_or_else(invalid_update)?;
        if !destination.is_absolute()
            || fs::canonicalize(destination)? != *destination
            || (self.format == Format::AppImage && !is_appimage(destination)?)
        {
            return Err(invalid_update());
        }
        let scratch = parent.join(format!(".maltoria-update-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&scratch)?;
        let pending = PendingReplacement {
            scratch,
            authorized: false,
        };
        let staged = self.stage_package(update, &pending.scratch, destination)?;
        Ok(PreparedReplacement {
            pending,
            staged,
            destination: destination.clone(),
            format: self.format,
        })
    }

    fn stage_package(
        &self,
        update: &Downloaded,
        scratch: &Path,
        dest: &Path,
    ) -> io::Result<PathBuf> {
        let package = scratch.join("download");
        fs::copy(&update.path, &package)?;
        if crate::app_update::sha256(&package)? != update.sha256 {
            return Err(invalid_update());
        }
        match self.format {
            Format::AppImage => {
                if !is_appimage(&package)? {
                    return Err(invalid_update());
                }
                let mode = fs::metadata(dest)?.permissions().mode();
                fs::set_permissions(&package, fs::Permissions::from_mode(mode & 0o777))?;
                File::open(&package)?.sync_all()?;
                Ok(package)
            }
            #[cfg(target_os = "macos")]
            Format::Mac => stage_mac(&package, scratch, &update.version),
            _ => Err(invalid_update()),
        }
    }
}

fn is_appimage(path: &Path) -> io::Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let mut header = [0; 11];
    let mut file = File::open(path)?;
    Ok(file.read_exact(&mut header).is_ok()
        && &header[..4] == b"\x7fELF"
        && &header[8..11] == b"AI\x02")
}

fn invalid_update() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid app update package or destination",
    )
}

fn sha256(path: &Path) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(target_os = "macos")]
fn plist(bundle: &Path, key: &str) -> Option<String> {
    let result = Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(bundle.join("Contents/Info.plist"))
        .output()
        .ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().into())
}

#[cfg(target_os = "macos")]
fn stage_mac(package: &Path, scratch: &Path, version: &str) -> io::Result<PathBuf> {
    let archive = Command::new("/usr/bin/tar")
        .arg("-tf")
        .arg(package)
        .output()?;
    if !archive.status.success() {
        return Err(invalid_update());
    }
    for entry in std::str::from_utf8(&archive.stdout)
        .map_err(|_| invalid_update())?
        .lines()
    {
        use std::path::Component;
        let mut parts = Path::new(entry).components();
        let root = parts.next();
        let allowed = match root {
            Some(Component::Normal(part)) if part == "Maltoria.app" => true,
            Some(Component::Normal(part)) if part == "__MACOSX" => match parts.next() {
                None => true,
                Some(Component::Normal(part)) => part == "Maltoria.app",
                _ => false,
            },
            _ => false,
        };
        if !allowed || !parts.all(|part| matches!(part, Component::Normal(_))) {
            return Err(invalid_update());
        }
    }
    let listing = Command::new("/usr/bin/tar")
        .arg("-tvf")
        .arg(package)
        .output()?;
    if !listing.status.success()
        || !listing
            .stdout
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.is_empty())
            .all(|line| matches!(line[0], b'd' | b'-'))
    {
        return Err(invalid_update());
    }
    if !Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(package)
        .arg(scratch)
        .status()?
        .success()
    {
        return Err(invalid_update());
    }
    let bundle = scratch.join("Maltoria.app");
    if plist(&bundle, "CFBundleIdentifier").as_deref() != Some("com.daydalek.Maltoria")
        || plist(&bundle, "CFBundleShortVersionString").as_deref() != Some(version)
        || plist(&bundle, "CFBundleExecutable").as_deref() != Some("whisky-gpui")
        || !Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&bundle)
            .status()?
            .success()
        || !Command::new("/usr/bin/xattr")
            .args(["-w", "com.apple.quarantine", "0081;00000000;Maltoria;"])
            .arg(&bundle)
            .status()?
            .success()
    {
        return Err(invalid_update());
    }
    Ok(bundle)
}

const REPLACE_SCRIPT: &str = r#"
set -eu
pid=$1 new=$2 dest=$3 scratch=$4 format=$5
count=0
while kill -0 "$pid" 2>/dev/null; do
    [ -d "$scratch" ] || exit 1
    if [ "$count" -ge 90 ]; then
        /bin/rm -rf "$scratch"
        exit 1
    fi
    /bin/sleep 1
    count=$((count + 1))
done
[ -f "$scratch/approved" ] || { /bin/rm -rf "$scratch"; exit 1; }
if [ "$format" = macos ]; then
    old="$scratch/previous.app"
    /bin/mv "$dest" "$old"
    if ! /bin/mv "$new" "$dest"; then
        /bin/mv "$old" "$dest"
        exit 1
    fi
    if ! /usr/bin/codesign --verify --deep --strict "$dest"; then
        /bin/mv "$dest" "$new"
        /bin/mv "$old" "$dest"
        exit 1
    fi
else
    /bin/mv -fT "$new" "$dest"
fi
/bin/rm -rf "$scratch"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_package_handoff_quotes_paths_and_never_applies_to_appimage() {
        let path = Path::new("/tmp/Some user's package.deb");
        assert_eq!(
            Format::Deb.package_command(path).as_deref(),
            Some("sudo apt install -- '/tmp/Some user'\\''s package.deb'")
        );
        assert!(Format::AppImage.package_command(path).is_none());
    }
}
