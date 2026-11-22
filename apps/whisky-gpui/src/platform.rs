//! Desktop conventions shared by the root view and menus.

#[cfg(target_os = "macos")]
pub(crate) fn ui_font() -> &'static str {
    ".AppleSystemUIFont"
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ui_font() -> &'static str {
    match crate::i18n::locale() {
        crate::i18n::Locale::Ja => "Noto Sans CJK JP",
        crate::i18n::Locale::Ko => "Noto Sans CJK KR",
        crate::i18n::Locale::ZhHans
        | crate::i18n::Locale::En
        | crate::i18n::Locale::Fr
        | crate::i18n::Locale::Es
        | crate::i18n::Locale::It
        | crate::i18n::Locale::De
        | crate::i18n::Locale::Ru => "Noto Sans CJK SC",
    }
}

#[cfg(target_os = "macos")]
pub(crate) const SHORTCUTS: [&str; 9] = [
    "cmd-q",
    "cmd-r",
    "cmd-shift-m",
    "cmd-,",
    "cmd-ctrl-s",
    "cmd-m",
    "cmd-f",
    "cmd-w",
    "cmd-[",
];
#[cfg(not(target_os = "macos"))]
pub(crate) const SHORTCUTS: [&str; 9] = [
    "ctrl-q",
    "ctrl-r",
    "ctrl-shift-m",
    "ctrl-,",
    "ctrl-alt-s",
    "ctrl-m",
    "ctrl-f",
    "ctrl-w",
    "alt-left",
];

pub(crate) fn settings_tip() -> &'static str {
    let p = &crate::i18n::s().platform;
    if cfg!(target_os = "macos") {
        p.settings_tip_mac
    } else {
        p.settings_tip_other
    }
}

#[cfg(target_os = "macos")]
pub(crate) const BACK_SHORTCUT_LABEL: &str = "⌘[";
#[cfg(not(target_os = "macos"))]
pub(crate) const BACK_SHORTCUT_LABEL: &str = "Alt+←";

#[allow(dead_code)]
pub(crate) fn motion_menu() -> &'static str {
    let p = &crate::i18n::s().platform;
    if cfg!(target_os = "macos") {
        p.motion_menu_mac
    } else {
        p.motion_menu_other
    }
}

pub(crate) fn reveal_label() -> &'static str {
    let p = &crate::i18n::s().platform;
    if cfg!(target_os = "macos") {
        p.reveal_mac
    } else {
        p.reveal_other
    }
}

/// The macOS traffic light cluster starts at x=15 (`traffic_light_position`)
/// and ends near x=73; 80 leaves a small margin after the zoom button.
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_CLEARANCE: f32 = 80.;

/// Leading padding that keeps content clear of the macOS traffic lights, which
/// overlay the content region while the sidebar is narrower than the cluster.
/// Fullscreen windows hide the lights, and other platforms draw their window
/// controls in a separate title bar — both keep the regular `base` padding.
#[cfg(target_os = "macos")]
pub(crate) fn traffic_light_padding(sidebar_width: f32, base: f32, fullscreen: bool) -> f32 {
    if fullscreen {
        base
    } else {
        (TRAFFIC_LIGHT_CLEARANCE - sidebar_width).max(base)
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn traffic_light_padding(_sidebar_width: f32, base: f32, _fullscreen: bool) -> f32 {
    base
}

/// Desktop entries need the persistent AppImage path across mount generations.
pub(crate) fn desktop_launcher() -> std::io::Result<std::path::PathBuf> {
    let current = std::env::current_exe()?;
    #[cfg(target_os = "linux")]
    if let (Some(image), Some(directory)) =
        (std::env::var_os("APPIMAGE"), std::env::var_os("APPDIR"))
    {
        return Ok(persistent_launcher(
            &current,
            std::path::Path::new(&image),
            std::path::Path::new(&directory),
        ));
    }
    Ok(current)
}

#[cfg(any(target_os = "linux", test))]
fn persistent_launcher(
    current: &std::path::Path,
    image: &std::path::Path,
    directory: &std::path::Path,
) -> std::path::PathBuf {
    if image.is_absolute()
        && image.is_file()
        && let (Ok(root), Ok(executable)) = (directory.canonicalize(), current.canonicalize())
        && executable.starts_with(root)
    {
        return image.to_path_buf();
    }
    current.to_path_buf()
}

#[cfg(test)]
mod launcher_tests {
    use super::persistent_launcher;

    #[test]
    fn appimage_uses_persistent_path_and_ignores_another_apps_environment() {
        let directory =
            std::env::temp_dir().join(format!("whisky-appimage-launcher-{}", std::process::id()));
        let mount = directory.join("mount/usr/bin");
        std::fs::create_dir_all(&mount).unwrap();
        let image = directory.join("Maltoria 包.AppImage");
        let binary = mount.join("whisky-gpui");
        let unrelated = directory.join("native");
        for file in [&image, &binary, &unrelated] {
            std::fs::write(file, b"fixture").unwrap();
        }
        assert_eq!(
            persistent_launcher(&binary, &image, &directory.join("mount")),
            image
        );
        assert_eq!(
            persistent_launcher(&unrelated, &image, &directory.join("mount")),
            unrelated
        );
        std::fs::remove_file(&image).unwrap();
        assert_eq!(
            persistent_launcher(&binary, &image, &directory.join("mount")),
            binary
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
