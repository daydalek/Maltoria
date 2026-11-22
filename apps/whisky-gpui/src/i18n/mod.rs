//! Application internationalization: Chinese (Simplified), Japanese, English,
//! and Korean with independent language selection (defaulting to system locale).

#![allow(dead_code)]

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

pub(crate) mod bridge_errors;
pub(crate) mod config;
pub(crate) mod fmt;
pub(crate) mod strings;

pub(crate) use strings::Strings;

/// Resolved UI locale used to select translated strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[repr(u8)]
pub(crate) enum Locale {
    #[default]
    ZhHans = 0,
    Ja = 1,
    En = 2,
    Ko = 3,
}

impl Locale {
    pub(crate) const ALL: [Self; 4] = [Self::ZhHans, Self::Ja, Self::En, Self::Ko];

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Ja,
            2 => Self::En,
            3 => Self::Ko,
            _ => Self::ZhHans,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::ZhHans => "zh-Hans",
            Self::Ja => "ja",
            Self::En => "en",
            Self::Ko => "ko",
        }
    }

    pub(crate) const fn native_name(self) -> &'static str {
        match self {
            Self::ZhHans => "简体中文",
            Self::Ja => "日本語",
            Self::En => "English",
            Self::Ko => "한국어",
        }
    }
}

/// User-facing interface language preference: follow OS locale or override explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[repr(u8)]
pub(crate) enum LanguageMode {
    #[default]
    #[serde(rename = "system")]
    System = 0,
    #[serde(rename = "zh-Hans")]
    ZhHans = 1,
    #[serde(rename = "ja")]
    Ja = 2,
    #[serde(rename = "en")]
    En = 3,
    #[serde(rename = "ko")]
    Ko = 4,
}

impl LanguageMode {
    pub(crate) const ALL: [Self; 5] = [
        Self::System,
        Self::ZhHans,
        Self::Ja,
        Self::En,
        Self::Ko,
    ];

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::ZhHans,
            2 => Self::Ja,
            3 => Self::En,
            4 => Self::Ko,
            _ => Self::System,
        }
    }

    pub(crate) fn from_wire(raw: &str) -> Self {
        let normalized = raw.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "" | "system" | "auto" => Self::System,
            "zh" | "zh-hans" | "zh-cn" | "zh-sg" | "zh-hant" | "zh-tw" | "zh-hk" => Self::ZhHans,
            "ja" | "ja-jp" => Self::Ja,
            "en" | "en-us" | "en-gb" | "en-au" | "en-ca" => Self::En,
            "ko" | "ko-kr" => Self::Ko,
            _ if normalized.starts_with("zh-") => Self::ZhHans,
            _ if normalized.starts_with("ja-") => Self::Ja,
            _ if normalized.starts_with("en-") => Self::En,
            _ if normalized.starts_with("ko-") => Self::Ko,
            _ => Self::System,
        }
    }

    pub(crate) const fn wire(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::ZhHans => "zh-Hans",
            Self::Ja => "ja",
            Self::En => "en",
            Self::Ko => "ko",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => s().settings.language_system,
            Self::ZhHans => "简体中文",
            Self::Ja => "日本語",
            Self::En => "English",
            Self::Ko => "한국어",
        }
    }

    pub(crate) fn options() -> [(&'static str, &'static str); 5] {
        [
            (s().settings.language_system, Self::System.wire()),
            ("简体中文", Self::ZhHans.wire()),
            ("日本語", Self::Ja.wire()),
            ("English", Self::En.wire()),
            ("한국어", Self::Ko.wire()),
        ]
    }

    pub(crate) fn resolve(self) -> Locale {
        match self {
            Self::System => detect_system_locale(),
            Self::ZhHans => Locale::ZhHans,
            Self::Ja => Locale::Ja,
            Self::En => Locale::En,
            Self::Ko => Locale::Ko,
        }
    }
}

static LANGUAGE_MODE: AtomicU8 = AtomicU8::new(LanguageMode::System as u8);
static ACTIVE_LOCALE: AtomicU8 = AtomicU8::new(Locale::ZhHans as u8);

#[cfg(test)]
std::thread_local! {
    static TEST_LOCALE_OVERRIDE: std::cell::Cell<Option<Locale>> = const { std::cell::Cell::new(None) };
    static TEST_MODE_OVERRIDE: std::cell::Cell<Option<LanguageMode>> = const { std::cell::Cell::new(None) };
}

/// Initialize the active UI language from the saved preference (resolving
/// `LanguageMode::System` against the OS locale).
pub(crate) fn init(mode: LanguageMode) -> Locale {
    set_language(mode)
}

/// Update the active UI language preference and return the resolved `Locale`.
pub(crate) fn set_language(mode: LanguageMode) -> Locale {
    let resolved = mode.resolve();
    #[cfg(test)]
    {
        TEST_MODE_OVERRIDE.with(|cell| cell.set(Some(mode)));
        TEST_LOCALE_OVERRIDE.with(|cell| cell.set(Some(resolved)));
    }
    #[cfg(not(test))]
    {
        LANGUAGE_MODE.store(mode as u8, Ordering::Relaxed);
        ACTIVE_LOCALE.store(resolved as u8, Ordering::Relaxed);
    }
    resolved
}

pub(crate) fn language_mode() -> LanguageMode {
    #[cfg(test)]
    if let Some(mode) = TEST_MODE_OVERRIDE.with(|cell| cell.get()) {
        return mode;
    }
    LanguageMode::from_u8(LANGUAGE_MODE.load(Ordering::Relaxed))
}

pub(crate) fn locale() -> Locale {
    #[cfg(test)]
    if let Some(locale) = TEST_LOCALE_OVERRIDE.with(|cell| cell.get()) {
        return locale;
    }
    Locale::from_u8(ACTIVE_LOCALE.load(Ordering::Relaxed))
}

/// Pick a value for the current active locale.
#[inline]
pub(crate) fn pick<T: Copy>(zh: T, ja: T, en: T, ko: T) -> T {
    match locale() {
        Locale::ZhHans => zh,
        Locale::Ja => ja,
        Locale::En => en,
        Locale::Ko => ko,
    }
}

/// Select a localized string slice for the current active locale.
#[inline]
pub(crate) fn tr(
    zh: &'static str,
    ja: &'static str,
    en: &'static str,
    ko: &'static str,
) -> &'static str {
    pick(zh, ja, en, ko)
}

/// Access the static string table for the current active locale.
#[inline]
pub(crate) fn s() -> &'static Strings {
    strings::table_for(locale())
}

/// Parse a single BCP-47 / POSIX locale tag into one of the four supported locales.
pub(crate) fn parse_locale_tag(raw: &str) -> Option<Locale> {
    let base = raw
        .trim()
        .split('@')
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .replace('_', "-");
    if base.is_empty() || base == "c" || base == "posix" {
        return None;
    }
    if base == "zh" || base.starts_with("zh-") {
        Some(Locale::ZhHans)
    } else if base == "ja" || base.starts_with("ja-") {
        Some(Locale::Ja)
    } else if base == "ko" || base.starts_with("ko-") {
        Some(Locale::Ko)
    } else if base == "en" || base.starts_with("en-") {
        Some(Locale::En)
    } else {
        None
    }
}

/// Resolve an ordered preference list of language tags. Falls back to English
/// when no preferred tag matches Chinese, Japanese, Korean, or English.
pub(crate) fn locale_from_tags<'a>(tags: impl IntoIterator<Item = &'a str>) -> Locale {
    for tag in tags {
        if let Some(locale) = parse_locale_tag(tag) {
            return locale;
        }
    }
    Locale::En
}

/// Detect the operating system's preferred language.
pub(crate) fn detect_system_locale() -> Locale {
    if let Ok(explicit) = std::env::var("WHISKY_GPUI_LOCALE")
        && let Some(locale) = parse_locale_tag(&explicit)
    {
        return locale;
    }
    #[cfg(target_os = "macos")]
    {
        let preferred = macos_preferred_languages();
        if !preferred.is_empty() {
            return locale_from_tags(preferred.iter().map(String::as_str));
        }
    }
    let mut env_tags = Vec::new();
    if let Ok(language_list) = std::env::var("LANGUAGE") {
        for part in language_list.split(':') {
            if !part.trim().is_empty() {
                env_tags.push(part.trim().to_owned());
            }
        }
    }
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var)
            && !val.trim().is_empty()
        {
            env_tags.push(val);
        }
    }
    locale_from_tags(env_tags.iter().map(String::as_str))
}

#[cfg(target_os = "macos")]
fn macos_preferred_languages() -> Vec<String> {
    use std::ffi::{CStr, c_char, c_void};

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFLocaleCopyPreferredLanguages() -> *const c_void;
        fn CFArrayGetCount(the_array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(the_array: *const c_void, idx: isize) -> *const c_void;
        fn CFStringGetCString(
            the_string: *const c_void,
            buffer: *mut c_char,
            buffer_size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(cf: *const c_void);
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    let mut out = Vec::new();
    unsafe {
        let array = CFLocaleCopyPreferredLanguages();
        if array.is_null() {
            return out;
        }
        let count = CFArrayGetCount(array);
        for index in 0..count {
            let cf_str = CFArrayGetValueAtIndex(array, index);
            if cf_str.is_null() {
                continue;
            }
            let mut buf = [0 as c_char; 128];
            if CFStringGetCString(
                cf_str,
                buf.as_mut_ptr(),
                buf.len() as isize,
                K_CF_STRING_ENCODING_UTF8,
            ) != 0
            {
                let text = CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
                if !text.is_empty() {
                    out.push(text);
                }
            }
        }
        CFRelease(array);
    }
    out
}

#[cfg(test)]
pub(crate) struct LocaleGuard {
    prev_locale: Option<Locale>,
    prev_mode: Option<LanguageMode>,
}

#[cfg(test)]
impl LocaleGuard {
    pub(crate) fn new(locale: Locale) -> Self {
        let prev_locale = TEST_LOCALE_OVERRIDE.with(|cell| cell.replace(Some(locale)));
        let mode = match locale {
            Locale::ZhHans => LanguageMode::ZhHans,
            Locale::Ja => LanguageMode::Ja,
            Locale::En => LanguageMode::En,
            Locale::Ko => LanguageMode::Ko,
        };
        let prev_mode = TEST_MODE_OVERRIDE.with(|cell| cell.replace(Some(mode)));
        Self {
            prev_locale,
            prev_mode,
        }
    }
}

#[cfg(test)]
impl Drop for LocaleGuard {
    fn drop(&mut self) {
        TEST_LOCALE_OVERRIDE.with(|cell| cell.set(self.prev_locale));
        TEST_MODE_OVERRIDE.with(|cell| cell.set(self.prev_mode));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_mode_wire_round_trips_and_parses_variants() {
        for mode in LanguageMode::ALL {
            assert_eq!(LanguageMode::from_wire(mode.wire()), mode);
        }
        assert_eq!(LanguageMode::from_wire("zh-CN"), LanguageMode::ZhHans);
        assert_eq!(LanguageMode::from_wire("zh_TW"), LanguageMode::ZhHans);
        assert_eq!(LanguageMode::from_wire("ja_JP"), LanguageMode::Ja);
        assert_eq!(LanguageMode::from_wire("en_US"), LanguageMode::En);
        assert_eq!(LanguageMode::from_wire("ko_KR"), LanguageMode::Ko);
        assert_eq!(LanguageMode::from_wire("unknown"), LanguageMode::System);
    }

    #[test]
    fn parse_locale_tag_handles_posix_and_bcp47_tags() {
        assert_eq!(parse_locale_tag("zh-Hans-CN"), Some(Locale::ZhHans));
        assert_eq!(parse_locale_tag("zh_CN.UTF-8"), Some(Locale::ZhHans));
        assert_eq!(parse_locale_tag("ja_JP.UTF-8"), Some(Locale::Ja));
        assert_eq!(parse_locale_tag("ja-JP"), Some(Locale::Ja));
        assert_eq!(parse_locale_tag("en_US.UTF-8"), Some(Locale::En));
        assert_eq!(parse_locale_tag("ko_KR.UTF-8"), Some(Locale::Ko));
        assert_eq!(parse_locale_tag("C.UTF-8"), None);
        assert_eq!(parse_locale_tag("POSIX"), None);
        assert_eq!(parse_locale_tag("fr_FR.UTF-8"), None);
        assert_eq!(locale_from_tags(["C", "fr-FR", "ja-JP", "en-US"]), Locale::Ja);
        assert_eq!(locale_from_tags(["fr-FR", "de-DE"]), Locale::En);
    }

    #[test]
    fn all_four_locales_are_distinct_and_non_empty() {
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let tbl = s();
            assert!(!tbl.common.cancel.is_empty());
            assert!(!tbl.menus.about.is_empty());
            assert!(!tbl.settings.language_label.is_empty());
            assert!(!tbl.library.title.is_empty());
            assert!(!tbl.dialogs.about_title.is_empty());
            assert!(!tbl.capture.title.is_empty());
            assert!(!tbl.translation.title_providers.is_empty());
        }
        assert_ne!(
            {
                let _g = LocaleGuard::new(Locale::ZhHans);
                s().common.cancel
            },
            {
                let _g = LocaleGuard::new(Locale::Ja);
                s().common.cancel
            }
        );
        assert_ne!(
            {
                let _g = LocaleGuard::new(Locale::En);
                s().common.cancel
            },
            {
                let _g = LocaleGuard::new(Locale::Ko);
                s().common.cancel
            }
        );
    }

    #[test]
    fn locale_metadata_and_language_mode_labels_match_active_locale() {
        assert_eq!(Locale::ZhHans.code(), "zh-Hans");
        assert_eq!(Locale::Ja.code(), "ja");
        assert_eq!(Locale::En.code(), "en");
        assert_eq!(Locale::Ko.code(), "ko");

        assert_eq!(Locale::ZhHans.native_name(), "简体中文");
        assert_eq!(Locale::Ja.native_name(), "日本語");
        assert_eq!(Locale::En.native_name(), "English");
        assert_eq!(Locale::Ko.native_name(), "한국어");

        assert_eq!(LanguageMode::ZhHans.resolve(), Locale::ZhHans);
        assert_eq!(LanguageMode::Ja.resolve(), Locale::Ja);
        assert_eq!(LanguageMode::En.resolve(), Locale::En);
        assert_eq!(LanguageMode::Ko.resolve(), Locale::Ko);

        for (loc, sys_label) in [
            (Locale::ZhHans, "跟随系统"),
            (Locale::Ja, "システム設定に従う"),
            (Locale::En, "Follow System"),
            (Locale::Ko, "시스템 설정 따름"),
        ] {
            let _guard = LocaleGuard::new(loc);
            assert_eq!(LanguageMode::System.label(), sys_label);
            let opts = LanguageMode::options();
            assert_eq!(opts[0], (sys_label, "system"));
            assert_eq!(opts[1], ("简体中文", "zh-Hans"));
            assert_eq!(opts[2], ("日本語", "ja"));
            assert_eq!(opts[3], ("English", "en"));
            assert_eq!(opts[4], ("한국어", "ko"));
        }
    }
}
