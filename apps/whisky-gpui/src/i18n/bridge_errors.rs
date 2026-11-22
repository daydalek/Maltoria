//! Localized descriptions for bridge `{code, message}` failures.
//!
//! The bridge reports stable error codes; this module maps them to translated
//! text so the UI can stay localized while the code remains visible for
//! diagnosis. Unknown codes fall back to the raw `{code}: {message}` pair.


/// Render a bridge failure for display.
///
/// `{code}: {localized}` for known codes; codes whose message carries unique
/// context keep the original message appended; anything else falls back to
/// `{code}: {message}`.
pub(crate) fn describe(code: &str, message: &str) -> String {
    if code.is_empty() {
        return message.to_owned();
    }
    if let Some(inner) = code.strip_prefix("capture_") {
        return match lookup(crate::i18n::s().maps.bridge_capture, inner) {
            Some(text) => format!("{code}: {text}"),
            None => fallback(code, message),
        };
    }
    if let Some(text) = lookup(crate::i18n::s().maps.bridge_static, code) {
        return format!("{code}: {text}");
    }
    if let Some(text) = lookup(crate::i18n::s().maps.bridge_detail, code) {
        return if message.is_empty() {
            format!("{code}: {text}")
        } else {
            format!("{code}: {text}: {message}")
        };
    }
    fallback(code, message)
}

/// Localized text for a bare error code (capture HUD `lastError`, subtitle
/// `error`, and the `capture_*` hook codes). `None` when the code is unknown.
pub(crate) fn code_text(code: &str) -> Option<&'static str> {
    if let Some(inner) = code.strip_prefix("capture_") {
        return lookup(crate::i18n::s().maps.bridge_capture, inner);
    }
    lookup(crate::i18n::s().maps.bridge_common, code)
        .or_else(|| lookup(crate::i18n::s().maps.bridge_static, code))
        .or_else(|| lookup(crate::i18n::s().maps.bridge_detail, code))
        .or_else(|| lookup(crate::i18n::s().maps.bridge_capture, code))
}

fn fallback(code: &str, message: &str) -> String {
    if message.is_empty() {
        code.to_owned()
    } else {
        format!("{code}: {message}")
    }
}

fn lookup(map: &'static [(&'static str, &'static str)], code: &str) -> Option<&'static str> {
    crate::i18n::lookup(map, code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Locale, LocaleGuard};

    #[test]
    fn describes_known_codes_in_every_locale() {
        for (locale, expected) in [
            (Locale::ZhHans, "translation_storage_unavailable: 无法读取或保存翻译设置"),
            (Locale::Ja, "translation_storage_unavailable: 翻訳設定を読み書きできません"),
            (Locale::En, "translation_storage_unavailable: Translation settings could not be read or saved"),
            (Locale::Ko, "translation_storage_unavailable: 번역 설정을 읽거나 저장할 수 없습니다"),
        ] {
            let _guard = LocaleGuard::new(locale);
            assert_eq!(
                describe("translation_storage_unavailable", "Saved translation settings could not be read"),
                expected
            );
        }
    }

    #[test]
    fn detail_codes_keep_the_original_message() {
        let _guard = LocaleGuard::new(Locale::ZhHans);
        assert_eq!(
            describe("invalid_params", "Expected boolean: useChinaMirror"),
            "invalid_params: 请求参数无效: Expected boolean: useChinaMirror"
        );
        assert_eq!(describe("invalid_params", ""), "invalid_params: 请求参数无效");
    }

    #[test]
    fn unknown_codes_and_empty_code_fall_back() {
        let _guard = LocaleGuard::new(Locale::ZhHans);
        assert_eq!(describe("brand_new_code", "raw message"), "brand_new_code: raw message");
        assert_eq!(describe("brand_new_code", ""), "brand_new_code");
        assert_eq!(describe("", "only message"), "only message");
    }

    #[test]
    fn capture_codes_resolve_through_prefix_and_bare_lookup() {
        let _guard = LocaleGuard::new(Locale::ZhHans);
        assert_eq!(
            describe("capture_disconnected", "Text capture: disconnected"),
            "capture_disconnected: 文本钩子连接已断开"
        );
        assert_eq!(code_text("disconnected"), Some("文本钩子连接已断开"));
        assert_eq!(code_text("timeout"), Some("翻译超时"));
        assert_eq!(code_text("provider_not_found"), Some("找不到该服务商配置"));
        assert_eq!(code_text("nonexistent_code"), None);
    }

}
