//! Application internationalization: Chinese (Simplified), Japanese, English,
//! Korean, French, Spanish, Italian, German, and Russian with independent
//! language selection (defaulting to system locale).

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
    Fr = 4,
    Es = 5,
    It = 6,
    De = 7,
    Ru = 8,
}

impl Locale {
    pub(crate) const ALL: [Self; 9] = [
        Self::ZhHans,
        Self::Ja,
        Self::En,
        Self::Ko,
        Self::Fr,
        Self::Es,
        Self::It,
        Self::De,
        Self::Ru,
    ];

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Ja,
            2 => Self::En,
            3 => Self::Ko,
            4 => Self::Fr,
            5 => Self::Es,
            6 => Self::It,
            7 => Self::De,
            8 => Self::Ru,
            _ => Self::ZhHans,
        }
    }

    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::ZhHans => "zh-Hans",
            Self::Ja => "ja",
            Self::En => "en",
            Self::Ko => "ko",
            Self::Fr => "fr",
            Self::Es => "es",
            Self::It => "it",
            Self::De => "de",
            Self::Ru => "ru",
        }
    }

    pub(crate) const fn native_name(self) -> &'static str {
        match self {
            Self::ZhHans => "简体中文",
            Self::Ja => "日本語",
            Self::En => "English",
            Self::Ko => "한국어",
            Self::Fr => "Français",
            Self::Es => "Español",
            Self::It => "Italiano",
            Self::De => "Deutsch",
            Self::Ru => "Русский",
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
    #[serde(rename = "fr")]
    Fr = 5,
    #[serde(rename = "es")]
    Es = 6,
    #[serde(rename = "it")]
    It = 7,
    #[serde(rename = "de")]
    De = 8,
    #[serde(rename = "ru")]
    Ru = 9,
}

impl LanguageMode {
    pub(crate) const ALL: [Self; 10] = [
        Self::System,
        Self::ZhHans,
        Self::Ja,
        Self::En,
        Self::Ko,
        Self::Fr,
        Self::Es,
        Self::It,
        Self::De,
        Self::Ru,
    ];

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::ZhHans,
            2 => Self::Ja,
            3 => Self::En,
            4 => Self::Ko,
            5 => Self::Fr,
            6 => Self::Es,
            7 => Self::It,
            8 => Self::De,
            9 => Self::Ru,
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
            "fr" | "fr-fr" | "fr-be" | "fr-ca" | "fr-ch" => Self::Fr,
            "es" | "es-es" | "es-mx" | "es-ar" | "es-419" => Self::Es,
            "it" | "it-it" | "it-ch" => Self::It,
            "de" | "de-de" | "de-at" | "de-ch" => Self::De,
            "ru" | "ru-ru" | "ru-by" | "ru-kz" => Self::Ru,
            _ if normalized.starts_with("zh-") => Self::ZhHans,
            _ if normalized.starts_with("ja-") => Self::Ja,
            _ if normalized.starts_with("en-") => Self::En,
            _ if normalized.starts_with("ko-") => Self::Ko,
            _ if normalized.starts_with("fr-") => Self::Fr,
            _ if normalized.starts_with("es-") => Self::Es,
            _ if normalized.starts_with("it-") => Self::It,
            _ if normalized.starts_with("de-") => Self::De,
            _ if normalized.starts_with("ru-") => Self::Ru,
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
            Self::Fr => "fr",
            Self::Es => "es",
            Self::It => "it",
            Self::De => "de",
            Self::Ru => "ru",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => s().settings.language_system,
            Self::ZhHans => "简体中文",
            Self::Ja => "日本語",
            Self::En => "English",
            Self::Ko => "한국어",
            Self::Fr => "Français",
            Self::Es => "Español",
            Self::It => "Italiano",
            Self::De => "Deutsch",
            Self::Ru => "Русский",
        }
    }

    pub(crate) fn options() -> [(&'static str, &'static str); 10] {
        [
            (s().settings.language_system, Self::System.wire()),
            ("简体中文", Self::ZhHans.wire()),
            ("日本語", Self::Ja.wire()),
            ("English", Self::En.wire()),
            ("한국어", Self::Ko.wire()),
            ("Français", Self::Fr.wire()),
            ("Español", Self::Es.wire()),
            ("Italiano", Self::It.wire()),
            ("Deutsch", Self::De.wire()),
            ("Русский", Self::Ru.wire()),
        ]
    }

    pub(crate) fn resolve(self) -> Locale {
        match self {
            Self::System => detect_system_locale(),
            Self::ZhHans => Locale::ZhHans,
            Self::Ja => Locale::Ja,
            Self::En => Locale::En,
            Self::Ko => Locale::Ko,
            Self::Fr => Locale::Fr,
            Self::Es => Locale::Es,
            Self::It => Locale::It,
            Self::De => Locale::De,
            Self::Ru => Locale::Ru,
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

/// Access the static string table for the current active locale.
#[inline]
pub(crate) fn s() -> &'static Strings {
    strings::table_for(locale())
}

/// Plural-category templates stored by generated string tables.
pub(crate) struct Plural {
    pub(crate) zero: Option<&'static str>,
    pub(crate) one: Option<&'static str>,
    pub(crate) few: Option<&'static str>,
    pub(crate) many: Option<&'static str>,
    pub(crate) other: &'static str,
}

/// CLDR cardinal plural category used to select a `Plural` template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PluralCategory {
    One,
    Few,
    Many,
    Other,
}

/// CLDR cardinal plural rules for `n` in `locale` (`zero` is handled
/// separately by `pick_plural` as an explicit n == 0 override).
pub(crate) fn plural_category(locale: Locale, n: u64) -> PluralCategory {
    match locale {
        Locale::ZhHans | Locale::Ja | Locale::Ko => PluralCategory::Other,
        Locale::En | Locale::Es | Locale::It | Locale::De => {
            if n == 1 {
                PluralCategory::One
            } else {
                PluralCategory::Other
            }
        }
        Locale::Fr => {
            if n <= 1 {
                PluralCategory::One
            } else {
                PluralCategory::Other
            }
        }
        Locale::Ru => {
            let mod10 = n % 10;
            let mod100 = n % 100;
            if mod10 == 1 && mod100 != 11 {
                PluralCategory::One
            } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
                PluralCategory::Few
            } else {
                PluralCategory::Many
            }
        }
    }
}

/// Select the template for `count` from a plural table: `zero` wins for
/// n == 0 when present, then the locale's CLDR category, falling back to
/// `other` when that category has no template.
pub(crate) fn pick_plural(table: &Plural, count: impl PluralCount) -> &'static str {
    let n = count.as_u64();
    if n == 0 && table.zero.is_some() {
        return table.zero.unwrap();
    }
    match plural_category(locale(), n) {
        PluralCategory::One => table.one,
        PluralCategory::Few => table.few,
        PluralCategory::Many => table.many,
        PluralCategory::Other => None,
    }
    .unwrap_or(table.other)
}

/// Integer argument usable for plural-category selection.
pub(crate) trait PluralCount: Copy + std::fmt::Display {
    fn as_u64(self) -> u64;
}

macro_rules! impl_plural_count {
    ($($t:ty),* $(,)?) => {
        $(impl PluralCount for $t {
            #[inline]
            fn as_u64(self) -> u64 {
                self.max(0) as u64
            }
        })*
    };
}
impl_plural_count!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

/// Substitute `{name}` placeholders in a template; `{{`/`}}` escape literal braces.
/// Placeholders are validated at build time, so unknown names only trip a debug_assert.
pub(crate) fn render(
    template: &'static str,
    args: &[(&'static str, &dyn std::fmt::Display)],
) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(template.len() + 16);
    let mut i = 0;
    while i < template.len() {
        let b = template.as_bytes()[i];
        if b == b'{' {
            if template.as_bytes().get(i + 1) == Some(&b'{') {
                out.push('{');
                i += 2;
                continue;
            }
            match template[i + 1..].find('}') {
                Some(rel) => {
                    let name = &template[i + 1..i + 1 + rel];
                    match args.iter().find(|(n, _)| *n == name) {
                        Some((_, v)) => {
                            let _ = write!(out, "{v}");
                        }
                        None => {
                            debug_assert!(false, "i18n::render: unknown placeholder {{{name}}}");
                            out.push_str(&template[i..i + rel + 2]);
                        }
                    }
                    i += rel + 2;
                }
                None => {
                    out.push_str(&template[i..]);
                    break;
                }
            }
        } else if b == b'}' {
            if template.as_bytes().get(i + 1) == Some(&b'}') {
                out.push('}');
                i += 2;
                continue;
            }
            debug_assert!(false, "i18n::render: stray `}}`");
            out.push('}');
            i += 1;
        } else {
            let next = template[i..]
                .find(['{', '}'])
                .map(|rel| i + rel)
                .unwrap_or(template.len());
            out.push_str(&template[i..next]);
            i = next;
        }
    }
    out
}

/// Binary-search a generated `maps.*` table (sorted by key at build time).
pub(crate) fn lookup(
    map: &'static [(&'static str, &'static str)],
    key: &str,
) -> Option<&'static str> {
    map.binary_search_by(|(k, _)| k.cmp(&key))
        .ok()
        .map(|i| map[i].1)
}

/// Parse a single BCP-47 / POSIX locale tag into one of the supported locales.
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
    } else if base == "fr" || base.starts_with("fr-") {
        Some(Locale::Fr)
    } else if base == "es" || base.starts_with("es-") {
        Some(Locale::Es)
    } else if base == "it" || base.starts_with("it-") {
        Some(Locale::It)
    } else if base == "de" || base.starts_with("de-") {
        Some(Locale::De)
    } else if base == "ru" || base.starts_with("ru-") {
        Some(Locale::Ru)
    } else {
        None
    }
}

/// Resolve an ordered preference list of language tags. Falls back to English
/// when no preferred tag matches a supported locale.
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
            Locale::Fr => LanguageMode::Fr,
            Locale::Es => LanguageMode::Es,
            Locale::It => LanguageMode::It,
            Locale::De => LanguageMode::De,
            Locale::Ru => LanguageMode::Ru,
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
        assert_eq!(LanguageMode::from_wire("fr_FR"), LanguageMode::Fr);
        assert_eq!(LanguageMode::from_wire("es-MX"), LanguageMode::Es);
        assert_eq!(LanguageMode::from_wire("it_CH"), LanguageMode::It);
        assert_eq!(LanguageMode::from_wire("de_AT"), LanguageMode::De);
        assert_eq!(LanguageMode::from_wire("ru-RU"), LanguageMode::Ru);
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
        assert_eq!(parse_locale_tag("fr_FR.UTF-8"), Some(Locale::Fr));
        assert_eq!(parse_locale_tag("es_419.UTF-8"), Some(Locale::Es));
        assert_eq!(parse_locale_tag("it_IT.UTF-8"), Some(Locale::It));
        assert_eq!(parse_locale_tag("de_DE.UTF-8"), Some(Locale::De));
        assert_eq!(parse_locale_tag("ru_RU.UTF-8"), Some(Locale::Ru));
        assert_eq!(parse_locale_tag("C.UTF-8"), None);
        assert_eq!(parse_locale_tag("POSIX"), None);
        assert_eq!(parse_locale_tag("pt_BR.UTF-8"), None);
        assert_eq!(locale_from_tags(["C", "pt-BR", "ja-JP", "en-US"]), Locale::Ja);
        assert_eq!(locale_from_tags(["fr-FR", "ja-JP"]), Locale::Fr);
        assert_eq!(locale_from_tags(["pt-BR", "nl-NL"]), Locale::En);
    }

    #[test]
    fn every_locale_table_is_non_empty_and_reference_locales_differ() {
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let tbl = s();
            assert!(!tbl.common.cancel.is_empty());
            assert!(!tbl.menus.about.is_empty());
            assert!(!tbl.settings.language_label.is_empty());
            assert!(!tbl.library.title.is_empty());
            assert!(!tbl.dialogs.about_title.is_empty());
            assert!(!tbl.capture.title.is_empty());
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
        assert_eq!(Locale::Fr.code(), "fr");
        assert_eq!(Locale::Es.code(), "es");
        assert_eq!(Locale::It.code(), "it");
        assert_eq!(Locale::De.code(), "de");
        assert_eq!(Locale::Ru.code(), "ru");

        assert_eq!(Locale::ZhHans.native_name(), "简体中文");
        assert_eq!(Locale::Ja.native_name(), "日本語");
        assert_eq!(Locale::En.native_name(), "English");
        assert_eq!(Locale::Ko.native_name(), "한국어");
        assert_eq!(Locale::Fr.native_name(), "Français");
        assert_eq!(Locale::Es.native_name(), "Español");
        assert_eq!(Locale::It.native_name(), "Italiano");
        assert_eq!(Locale::De.native_name(), "Deutsch");
        assert_eq!(Locale::Ru.native_name(), "Русский");

        assert_eq!(LanguageMode::ZhHans.resolve(), Locale::ZhHans);
        assert_eq!(LanguageMode::Ja.resolve(), Locale::Ja);
        assert_eq!(LanguageMode::En.resolve(), Locale::En);
        assert_eq!(LanguageMode::Ko.resolve(), Locale::Ko);
        assert_eq!(LanguageMode::Fr.resolve(), Locale::Fr);
        assert_eq!(LanguageMode::Es.resolve(), Locale::Es);
        assert_eq!(LanguageMode::It.resolve(), Locale::It);
        assert_eq!(LanguageMode::De.resolve(), Locale::De);
        assert_eq!(LanguageMode::Ru.resolve(), Locale::Ru);

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
            assert_eq!(opts[5], ("Français", "fr"));
            assert_eq!(opts[6], ("Español", "es"));
            assert_eq!(opts[7], ("Italiano", "it"));
            assert_eq!(opts[8], ("Deutsch", "de"));
            assert_eq!(opts[9], ("Русский", "ru"));
        }
    }

    #[test]
    fn plural_category_matches_cldr_cardinal_rules() {
        use PluralCategory::*;
        // Russian: one/few/many on mod10/mod100.
        for (n, expected) in [
            (1, One),
            (2, Few),
            (5, Many),
            (11, Many),
            (21, One),
            (22, Few),
            (25, Many),
            (101, One),
            (111, Many),
        ] {
            assert_eq!(plural_category(Locale::Ru, n), expected, "ru n={n}");
        }
        // French: 0 and 1 are singular.
        assert_eq!(plural_category(Locale::Fr, 0), One);
        assert_eq!(plural_category(Locale::Fr, 1), One);
        assert_eq!(plural_category(Locale::Fr, 2), Other);
        // English and the other Latin locales: only n == 1 is singular.
        for loc in [Locale::En, Locale::De, Locale::Es, Locale::It] {
            assert_eq!(plural_category(loc, 1), One, "{loc:?} n=1");
            assert_eq!(plural_category(loc, 0), Other, "{loc:?} n=0");
            assert_eq!(plural_category(loc, 2), Other, "{loc:?} n=2");
        }
        // CJK locales have a single form.
        for loc in [Locale::ZhHans, Locale::Ja, Locale::Ko] {
            for n in [0, 1, 2, 5, 21, 101] {
                assert_eq!(plural_category(loc, n), Other, "{loc:?} n={n}");
            }
        }
    }

    #[test]
    fn pick_plural_prefers_zero_then_category_then_other() {
        let full = Plural {
            zero: Some("zero"),
            one: Some("one"),
            few: Some("few"),
            many: Some("many"),
            other: "other",
        };
        {
            let _g = LocaleGuard::new(Locale::En);
            assert_eq!(pick_plural(&full, 0u64), "zero");
            assert_eq!(pick_plural(&full, 1u64), "one");
            assert_eq!(pick_plural(&full, 2u64), "other");
        }
        {
            let _g = LocaleGuard::new(Locale::Fr);
            // n == 0 with a `zero` template wins over fr's One category.
            assert_eq!(pick_plural(&full, 0u64), "zero");
            let no_zero = Plural {
                zero: None,
                one: Some("one"),
                few: None,
                many: None,
                other: "other",
            };
            assert_eq!(pick_plural(&no_zero, 0u64), "one");
            assert_eq!(pick_plural(&no_zero, 2u64), "other");
        }
        {
            let _g = LocaleGuard::new(Locale::Ru);
            assert_eq!(pick_plural(&full, 2u64), "few");
            assert_eq!(pick_plural(&full, 5u64), "many");
            assert_eq!(pick_plural(&full, 21u64), "one");
            let sparse = Plural {
                zero: None,
                one: None,
                few: None,
                many: None,
                other: "other",
            };
            // Categories missing from the table fall back to `other`.
            assert_eq!(pick_plural(&sparse, 2u64), "other");
        }
        {
            let _g = LocaleGuard::new(Locale::Ja);
            assert_eq!(pick_plural(&full, 0u64), "zero");
            assert_eq!(pick_plural(&full, 1u64), "other");
            assert_eq!(pick_plural(&full, 9u64), "other");
        }
    }
}

#[cfg(test)]
mod coverage_tests {
    //! Every runtime ID the app looks up must have a map entry in every locale.
    use super::*;

    #[test]
    fn provider_spec_kinds_have_labels_and_tips_in_every_locale() {
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let maps = &s().maps;
            for spec in crate::translation::PROVIDER_SPECS {
                assert!(
                    lookup(maps.provider_label, spec.kind).is_some(),
                    "{loc:?}: provider_label missing {}",
                    spec.kind
                );
                assert!(
                    lookup(maps.provider_short_label, spec.kind).is_some(),
                    "{loc:?}: provider_short_label missing {}",
                    spec.kind
                );
                assert!(
                    !spec.display_tip().is_empty(),
                    "{loc:?}: display_tip empty for {}",
                    spec.kind
                );
            }
        }
    }

    #[test]
    fn provider_presets_have_names_and_tags_in_every_locale() {
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let maps = &s().maps;
            for preset in crate::translation::PROVIDER_PRESETS {
                assert!(
                    lookup(maps.provider_preset_name, preset.id).is_some(),
                    "{loc:?}: provider_preset_name missing {}",
                    preset.id
                );
                assert!(
                    lookup(maps.provider_preset_tag, preset.id).is_some(),
                    "{loc:?}: provider_preset_tag missing {}",
                    preset.id
                );
            }
        }
    }

    #[test]
    fn metadata_source_ids_have_labels_in_every_locale() {
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let map = s().maps.source_label;
            let mut ids: std::collections::BTreeSet<&str> =
                crate::onboarding::SOURCES.iter().copied().collect();
            ids.extend(crate::metadata_editor::PROVIDERS.iter().copied());
            ids.extend(
                crate::state::METADATA_SOURCES
                    .iter()
                    .map(|(_, id)| *id),
            );
            for id in ids {
                assert!(
                    lookup(map, id).is_some(),
                    "{loc:?}: source_label missing {id}"
                );
            }
        }
    }

    #[test]
    fn config_ids_have_titles_in_every_locale() {
        let en = strings::table_for(Locale::En);
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let maps = &s().maps;
            for spec in crate::config::SECTIONS {
                assert!(
                    lookup(maps.config_section, spec.id).is_some(),
                    "{loc:?}: config_section missing {}",
                    spec.id
                );
            }
            for (field_id, key) in config::FIELD_TITLE_KEYS {
                assert!(
                    lookup(maps.config_field, key).is_some(),
                    "{loc:?}: config_field missing {key} (field {field_id})"
                );
            }
            // Every option/feature the en map defines resolves in this locale.
            for (key, _) in en.maps.config_option {
                assert!(
                    lookup(maps.config_option, key).is_some(),
                    "{loc:?}: config_option missing {key}"
                );
            }
            for (key, _) in en.maps.config_feature {
                assert!(
                    lookup(maps.config_feature, key).is_some(),
                    "{loc:?}: config_feature missing {key}"
                );
            }
        }
    }

    #[test]
    fn month_names_cover_one_through_twelve_in_every_locale() {
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let map = s().maps.month_name;
            for month in 1..=12 {
                assert!(
                    lookup(map, &month.to_string()).is_some(),
                    "{loc:?}: month_name missing {month}"
                );
            }
        }
    }
}
