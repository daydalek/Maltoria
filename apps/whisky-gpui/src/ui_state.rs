//! Local UI state that is not shared with the SwiftUI app: the last window
//! frame, restored on the next launch.

use std::path::PathBuf;

use gpui_kit::{App, Bounds, Pixels, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

use crate::i18n::LanguageMode;
use crate::theme::AppearanceMode;

const MIN_WIDTH: f32 = 800.;
const MIN_HEIGHT: f32 = 580.;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub(crate) struct WindowFrame {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl WindowFrame {
    fn is_usable(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width >= MIN_WIDTH
            && self.height >= MIN_HEIGHT
    }

    fn bounds(self) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(self.x), px(self.y)),
            size: size(px(self.width), px(self.height)),
        }
    }
}

/// `on_open_urls` delivers `NSURL.absoluteString`, so Finder hands us
/// `file:///Users/...` with percent escapes. Decode it back to a local path.
pub(crate) fn path_from_open_url(raw: &str) -> PathBuf {
    let Some(rest) = raw.strip_prefix("file://") else {
        return PathBuf::from(raw);
    };
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    PathBuf::from(String::from_utf8_lossy(&percent_decode(rest)).into_owned())
}

fn percent_decode(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    out
}

fn state_path() -> Option<PathBuf> {    if let Some(path) = std::env::var_os("WHISKY_GPUI_STATE") {
        return Some(PathBuf::from(path));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    #[cfg(target_os = "macos")]
    let directory = home
        .join("Library")
        .join("Application Support")
        .join("Maltoria");
    #[cfg(not(target_os = "macos"))]
    let directory = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".config"))
        .join("maltoria");
    Some(directory.join("gpui-window.json"))
}

fn load() -> Option<WindowFrame> {
    let path = state_path()?;
    let data = std::fs::read(path).ok()?;
    serde_json::from_slice::<WindowFrame>(&data)
        .ok()
        .filter(|frame| frame.is_usable())
}

/// Saved frame when it is usable, otherwise the centered default size.
pub(crate) fn initial_window_bounds(cx: &mut App) -> WindowBounds {
    match load() {
        Some(frame) => WindowBounds::Windowed(frame.bounds()),
        None => WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.), px(820.)),
            cx,
        )),
    }
}

/// Persist the last windowed frame. Maximized and fullscreen windows store
/// their restored bounds, so quitting while zoomed still restores a sane size.
pub(crate) fn save_window(window_bounds: WindowBounds) {
    let bounds = match window_bounds {
        WindowBounds::Windowed(bounds)
        | WindowBounds::Maximized(bounds)
        | WindowBounds::Fullscreen(bounds) => bounds,
    };
    let frame = WindowFrame {
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
    };
    if !frame.is_usable() {
        return;
    }
    let Some(path) = state_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_vec(&frame) {
        let _ = std::fs::write(path, data);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
struct StoredAppearance {
    mode: AppearanceMode,
}

fn appearance_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WHISKY_GPUI_APPEARANCE_STATE") {
        return Some(PathBuf::from(path));
    }
    Some(state_path()?.with_file_name("gpui-appearance.json"))
}

/// Load the saved appearance preference, defaulting to `AppearanceMode::System`
/// so Whisky automatically follows the OS light/dark appearance out of the box.
pub(crate) fn load_appearance() -> AppearanceMode {
    if std::env::var_os("WHISKY_DEMO_CAPTURE").is_some()
        || std::env::var_os("WHISKY_MOTION_CAPTURE").is_some()
    {
        return AppearanceMode::Dark;
    }
    let Some(path) = appearance_path() else {
        return AppearanceMode::System;
    };
    let Ok(data) = std::fs::read(path) else {
        return AppearanceMode::System;
    };
    serde_json::from_slice::<StoredAppearance>(&data)
        .map(|stored| stored.mode)
        .unwrap_or(AppearanceMode::System)
}

pub(crate) fn save_appearance(mode: AppearanceMode) {
    let Some(path) = appearance_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let stored = StoredAppearance { mode };
    if let Ok(data) = serde_json::to_vec(&stored) {
        let _ = std::fs::write(path, data);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
struct StoredLanguage {
    mode: LanguageMode,
}

fn language_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WHISKY_GPUI_LANGUAGE_STATE") {
        return Some(PathBuf::from(path));
    }
    if cfg!(test) {
        return None;
    }
    Some(state_path()?.with_file_name("gpui-language.json"))
}

fn load_language_from(path: &std::path::Path) -> LanguageMode {
    let Ok(data) = std::fs::read(path) else {
        return LanguageMode::System;
    };
    serde_json::from_slice::<StoredLanguage>(&data)
        .map(|stored| stored.mode)
        .unwrap_or(LanguageMode::System)
}

fn save_language_to(path: &std::path::Path, mode: LanguageMode) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let stored = StoredLanguage { mode };
    if let Ok(data) = serde_json::to_vec(&stored) {
        let _ = std::fs::write(path, data);
    }
}

/// Load the saved UI language preference, defaulting to `LanguageMode::System`
/// so Maltoria follows the operating system language out of the box.
pub(crate) fn load_language() -> LanguageMode {
    let Some(path) = language_path() else {
        return LanguageMode::System;
    };
    load_language_from(&path)
}

pub(crate) fn save_language(mode: LanguageMode) {
    let Some(path) = language_path() else { return };
    save_language_to(&path, mode);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TagBarMode {
    #[default]
    PullDown,
    AlwaysVisible,
}

impl TagBarMode {
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::PullDown => "pull-down",
            Self::AlwaysVisible => "always",
        }
    }

    pub(crate) fn from_wire(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "always" => Self::AlwaysVisible,
            _ => Self::PullDown,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FilterBarTab {
    #[default]
    Tags,
    Studios,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct LibraryUiPrefs {
    pub(crate) tag_bar_mode: TagBarMode,
    pub(crate) tag_bar_hint_seen: bool,
    pub(crate) group_by_studio: bool,
    pub(crate) filter_bar_tab: FilterBarTab,
}

pub(crate) fn capture_mode_active() -> bool {
    std::env::var_os("WHISKY_DEMO_CAPTURE").is_some()
        || std::env::var_os("WHISKY_MOTION_CAPTURE").is_some()
}

fn library_ui_state_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("WHISKY_GPUI_LIBRARY_UI_STATE") {
        return Some(PathBuf::from(path));
    }
    if cfg!(test) {
        return None;
    }
    Some(state_path()?.with_file_name("gpui-library-ui.json"))
}

fn load_library_ui_prefs_from(path: &std::path::Path) -> LibraryUiPrefs {
    let Ok(data) = std::fs::read(path) else {
        return LibraryUiPrefs::default();
    };
    serde_json::from_slice::<LibraryUiPrefs>(&data).unwrap_or_default()
}

fn save_library_ui_prefs_to(path: &std::path::Path, prefs: &LibraryUiPrefs) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_vec(prefs) {
        let _ = std::fs::write(path, data);
    }
}

pub(crate) fn load_library_ui_prefs() -> LibraryUiPrefs {
    let Some(path) = library_ui_state_path() else {
        return LibraryUiPrefs::default();
    };
    load_library_ui_prefs_from(&path)
}

pub(crate) fn save_library_ui_prefs(prefs: &LibraryUiPrefs) {
    let Some(path) = library_ui_state_path() else {
        return;
    };
    save_library_ui_prefs_to(&path, prefs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_ui_prefs_round_trip_through_json_file() {
        let dir = std::env::temp_dir().join(format!("maltoria-ui-prefs-{}", uuid::Uuid::new_v4()));
        let path = dir.join("gpui-library-ui.json");
        let prefs = LibraryUiPrefs {
            tag_bar_mode: TagBarMode::AlwaysVisible,
            tag_bar_hint_seen: true,
            group_by_studio: true,
            filter_bar_tab: FilterBarTab::Studios,
        };
        save_library_ui_prefs_to(&path, &prefs);
        assert_eq!(load_library_ui_prefs_from(&path), prefs);
        assert_eq!(TagBarMode::from_wire("always"), TagBarMode::AlwaysVisible);
        assert_eq!(TagBarMode::from_wire("pull-down"), TagBarMode::PullDown);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn frames_below_the_window_minimum_are_not_restored() {
        let frame = WindowFrame {
            x: 10.,
            y: 20.,
            width: MIN_WIDTH,
            height: MIN_HEIGHT,
        };
        assert!(frame.is_usable());
        assert!(!WindowFrame { width: 500., ..frame }.is_usable());
        assert!(!WindowFrame { height: f32::NAN, ..frame }.is_usable());
    }

    #[test]
    fn frames_round_trip_through_json() {
        let frame = WindowFrame {
            x: -120.5,
            y: 40.,
            width: 1440.,
            height: 900.,
        };
        let data = serde_json::to_vec(&frame).unwrap();
        assert_eq!(
            serde_json::from_slice::<WindowFrame>(&data).unwrap(),
            frame
        );
    }

    #[test]
    fn stored_appearance_round_trips_through_json() {
        for mode in [
            AppearanceMode::System,
            AppearanceMode::Light,
            AppearanceMode::Dark,
        ] {
            let data = serde_json::to_vec(&StoredAppearance { mode }).unwrap();
            let decoded: StoredAppearance = serde_json::from_slice(&data).unwrap();
            assert_eq!(decoded.mode, mode);
        }
    }

    #[test]
    fn stored_language_round_trips_through_file_and_defaults_to_system() {
        let dir = std::env::temp_dir().join(format!("maltoria-lang-prefs-{}", uuid::Uuid::new_v4()));
        let path = dir.join("gpui-language.json");
        assert_eq!(load_language_from(&path), LanguageMode::System);
        for mode in LanguageMode::ALL {
            save_language_to(&path, mode);
            assert_eq!(load_language_from(&path), mode);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn finder_file_urls_decode_to_local_paths() {
        assert_eq!(
            path_from_open_url("file:///Users/me/My%20Games/setup.exe"),
            PathBuf::from("/Users/me/My Games/setup.exe")
        );
        assert_eq!(
            path_from_open_url("file://localhost/Users/me/game.exe"),
            PathBuf::from("/Users/me/game.exe")
        );
        assert_eq!(
            path_from_open_url("/Users/me/game.exe"),
            PathBuf::from("/Users/me/game.exe")
        );
    }
}
