//! Host system services for the native backend: info, shell open/reveal, and
//! file picker panels.

use serde_json::{Value, json};

use crate::native::store::normalize_path;

pub(crate) fn system_info() -> Value {
    json!({
        "bundleVersion": env!("CARGO_PKG_VERSION"),
        "buildNumber": "1",
        "osVersion": os_version(),
    })
}

fn os_version() -> String {
    #[cfg(target_os = "windows")]
    {
        "Windows".to_owned()
    }
    #[cfg(target_os = "linux")]
    {
        "Linux".to_owned()
    }
    #[cfg(target_os = "macos")]
    {
        "macOS".to_owned()
    }
}

fn open_path(path: &str) -> Result<Value, (String, String)> {
    let normalized = normalize_path(path);
    #[cfg(target_os = "windows")]
    {
        // `explorer` returns non-zero for some successful opens; `cmd /c start`
        // has shell-injection surface, so use explorer and accept its status.
        let result = std::process::Command::new("explorer")
            .arg(&normalized)
            .spawn();
        return match result {
            Ok(_) => Ok(json!({})),
            Err(error) => Err(("open_failed".into(), format!("Cannot open {normalized}: {error}"))),
        };
    }
    #[cfg(target_os = "macos")]
    {
        let result = std::process::Command::new("open").arg(&normalized).status();
        return match result {
            Ok(status) if status.success() => Ok(json!({})),
            _ => Err(("open_failed".into(), format!("Cannot open {normalized}"))),
        };
    }
    #[cfg(target_os = "linux")]
    {
        let result = std::process::Command::new("xdg-open").arg(&normalized).status();
        return match result {
            Ok(status) if status.success() => Ok(json!({})),
            _ => Err(("open_failed".into(), format!("Cannot open {normalized}"))),
        };
    }
}

pub(crate) fn open(params: &Value) -> Result<Value, (String, String)> {
    let target = params
        .get("path")
        .or_else(|| params.get("url"))
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "system.open needs path".into()))?;
    if target.starts_with("http://") || target.starts_with("https://") {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            std::process::Command::new("rundll32")
                .args(["url.dll,FileProtocolHandler", target])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|error| ("open_failed".into(), error.to_string()))?;
            return Ok(json!({}));
        }
        #[cfg(not(target_os = "windows"))]
        return open_path(target);
    }
    open_path(target)
}

pub(crate) fn reveal(params: &Value) -> Result<Value, (String, String)> {
    let target = params
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| ("invalid_params".into(), "system.reveal needs path".into()))?;
    let normalized = normalize_path(target);
    #[cfg(target_os = "windows")]
    {
        let select = format!("/select,{}", normalized.replace('/', "\\"));
        std::process::Command::new("explorer")
            .arg(select)
            .spawn()
            .map_err(|error| ("reveal_failed".into(), error.to_string()))?;
        return Ok(json!({}));
    }
    #[cfg(not(target_os = "windows"))]
    {
        return open_path(&normalized);
    }
}

/// `system.files`: existence/type probe batch for the UI.
pub(crate) fn files(params: &Value) -> Result<Value, (String, String)> {
    let paths = params
        .get("paths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rows = paths
        .iter()
        .filter_map(Value::as_str)
        .map(|path| {
            let normalized = normalize_path(path);
            let meta = std::fs::metadata(&normalized).ok();
            json!({
                "path": normalized,
                "exists": meta.is_some(),
                "isFile": meta.as_ref().is_some_and(|m| m.is_file()),
                "isDirectory": meta.as_ref().is_some_and(|m| m.is_dir()),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "files": rows }))
}

/// File/folder panels. The native backend cannot block on a modal dialog
/// inside the bridge worker, so selection happens on a helper thread and the
/// reply is whatever the user picked; cancel maps to a null path.
pub(crate) fn choose_path(params: &Value) -> Result<Value, (String, String)> {
    let mode = params
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("open");
    let purpose = params
        .get("purpose")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let start = params.get("path").and_then(Value::as_str).unwrap_or("");
    let folder_mode = mode.contains("folder") || mode == "directory";
    let picked = pick_path(folder_mode, start);
    Ok(json!({
        "purpose": purpose,
        "path": picked,
    }))
}

pub(crate) fn choose_folder(params: &Value) -> Result<Value, (String, String)> {
    let start = params.get("path").and_then(Value::as_str).unwrap_or("");
    Ok(json!({
        "path": pick_path(true, start),
    }))
}

/// The managed folder browser is single-select, but `system.chooseFolders`
/// still answers the protocol's array shape; callers can repeat the picker to
/// add more roots.
pub(crate) fn choose_folders(params: &Value) -> Result<Value, (String, String)> {
    let start = params.get("path").and_then(Value::as_str).unwrap_or("");
    let paths = pick_path(true, start).into_iter().collect::<Vec<_>>();
    Ok(json!({ "paths": paths }))
}

#[cfg(target_os = "windows")]
fn powershell_string(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn pick_path(folder: bool, start: &str) -> Option<String> {
    // PowerShell hosts the shell common dialogs; no extra crate needed.
    // STA is required for the folder browser. Force UTF-8 so non-ASCII paths
    // survive the pipe on systems whose OEM code page is not UTF-8.
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let start = normalize_path(start);
    let initial = (!start.is_empty()).then(|| powershell_string(&start));
    let script = if folder {
        let selected = initial
            .as_ref()
            .map(|path| format!("$d.SelectedPath = {path};"))
            .unwrap_or_default();
        format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
             Add-Type -AssemblyName System.Windows.Forms; \
             $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
             $d.ShowNewFolderButton = $true; {selected} \
             if ($d.ShowDialog() -eq 'OK') {{ $d.SelectedPath }}"
        )
    } else {
        let initial_directory = initial
            .as_ref()
            .map(|path| format!("$d.InitialDirectory = {path};"))
            .unwrap_or_default();
        format!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
             Add-Type -AssemblyName System.Windows.Forms; \
             $d = New-Object System.Windows.Forms.OpenFileDialog; \
             $d.Filter = 'Programs (*.exe)|*.exe|All files (*.*)|*.*'; {initial_directory} \
             if ($d.ShowDialog() -eq 'OK') {{ $d.FileName }}"
        )
    };
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout)
        .trim_start_matches('\u{feff}')
        .trim()
        .to_owned();
    (!text.is_empty()).then(|| normalize_path(&text))
}

#[cfg(not(target_os = "windows"))]
fn pick_path(_folder: bool, _start: &str) -> Option<String> {
    None
}
