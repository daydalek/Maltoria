//! Bottle configuration schema and Wine engine feature localization.


/// Localized bottle configuration section title for `section_id`.
pub(crate) fn section_title(section_id: &str) -> &'static str {
    let map = crate::i18n::s().maps.config_section;
    crate::i18n::lookup(map, section_id)
        .or_else(|| crate::i18n::lookup(map, "other"))
        .unwrap_or("Other")
}

/// Field id → `maps.config_field` key for fields whose wire `titleKey` is empty.
pub(crate) const FIELD_TITLE_KEYS: &[(&str, &str)] = &[
    ("windowsVersion", "config.field.windowsVersion"),
    ("enhancedSync", "config.field.enhancedSync"),
    ("asfTimestampBase", "config.field.asfTimestampBase"),
    ("avxEnabled", "config.field.avxEnabled"),
    (
        "launcherCompatibilityMode",
        "config.field.launcherCompatibilityMode",
    ),
    ("launcherMode", "config.field.launcherMode"),
    ("detectedLauncher", "config.field.detectedLauncher"),
    ("launcherLocale", "config.field.launcherLocale"),
    ("gpuSpoofing", "config.field.gpuSpoofing"),
    ("gpuVendor", "config.field.gpuVendor"),
    ("autoEnableDXVK", "config.field.autoEnableDXVK"),
    (
        "controllerCompatibilityMode",
        "config.field.controllerCompatibilityMode",
    ),
    ("disableHIDAPI", "config.field.disableHIDAPI"),
    ("allowBackgroundEvents", "config.field.allowBackgroundEvents"),
    ("useButtonLabels", "config.field.useButtonLabels"),
    ("commandActsAsControl", "config.field.commandActsAsControl"),
    ("graphicsBackend", "config.field.graphicsBackend"),
    ("frameGeneration", "config.field.frameGeneration"),
    ("hostMemoryImport", "config.field.hostMemoryImport"),
    (
        "shadowImportedHostMemory",
        "config.field.shadowImportedHostMemory",
    ),
    ("metal4Enabled", "config.field.metal4Enabled"),
    ("metalHud", "config.field.metalHud"),
    ("metalTrace", "config.field.metalTrace"),
    ("dxrEnabled", "config.field.dxrEnabled"),
    ("metalValidation", "config.field.metalValidation"),
    ("dxvkAsync", "config.field.dxvkAsync"),
    ("dxvkHud", "config.field.dxvkHud"),
    ("metalFX", "config.field.metalFX"),
    ("metalFXSpatial", "config.field.metalFXSpatial"),
    ("gdiUpscaler", "config.field.gdiUpscaler"),
    (
        "gdiUpscalerExcludedRegions",
        "config.field.gdiUpscalerExcludedRegions",
    ),
    ("gdiUpscalerCache", "config.field.gdiUpscalerCache"),
    ("audioDriver", "config.field.audioDriver"),
    ("audioLatencyPreset", "config.field.audioLatencyPreset"),
    ("discordPresence", "config.field.discordPresence"),
    ("discordBridge", "config.field.discordBridge"),
    (
        "virtualDesktopEnabled",
        "config.field.virtualDesktopEnabled",
    ),
    ("resolutionPreset", "config.field.resolutionPreset"),
    (
        "customResolutionWidth",
        "config.field.customResolutionWidth",
    ),
    (
        "customResolutionHeight",
        "config.field.customResolutionHeight",
    ),
    ("shaderCacheEnabled", "config.field.shaderCacheEnabled"),
    ("disableAppNap", "config.field.disableAppNap"),
    ("clipboardPolicy", "config.field.clipboardPolicy"),
    ("clipboardThreshold", "config.field.clipboardThreshold"),
    ("killOnQuit", "config.field.killOnQuit"),
];

/// Localized bottle configuration field title, falling back to the wire `fallback`.
pub(crate) fn field_title<'a>(title_key: &str, id: &str, fallback: &'a str) -> &'a str {
    let key = if !title_key.is_empty() {
        title_key
    } else {
        FIELD_TITLE_KEYS
            .iter()
            .find(|(field_id, _)| *field_id == id)
            .map(|(_, key)| *key)
            .unwrap_or("")
    };
    crate::i18n::lookup(crate::i18n::s().maps.config_field, key).unwrap_or(fallback)
}

/// Localized bottle configuration choice option title.
pub(crate) fn option_title<'a>(
    field_id: &str,
    option_id: &str,
    fallback: &'a str,
) -> &'a str {
    let map = crate::i18n::s().maps.config_option;
    let exact = format!("{field_id}.{option_id}");
    crate::i18n::lookup(map, &exact)
        .or_else(|| crate::i18n::lookup(map, &format!(".{option_id}")))
        .unwrap_or(fallback)
}

/// Localized Wine engine feature name.
pub(crate) fn engine_feature_label(id: &str) -> &str {
    crate::i18n::lookup(crate::i18n::s().maps.config_feature, id).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Locale, LocaleGuard};

    #[test]
    fn config_sections_fields_options_and_features_localize_across_all_locales() {
        let mut titles_by_locale = Vec::new();
        for loc in Locale::ALL {
            let _guard = LocaleGuard::new(loc);
            let sec = section_title("graphics");
            let fld = field_title("config.field.dxvk", "dxvk", "DXVK");
            let opt = option_title("killOnQuit", "inherit", "Inherit");
            let feat = engine_feature_label("gdiUpscaling");
            assert!(!sec.is_empty());
            assert!(!fld.is_empty());
            assert!(!opt.is_empty());
            assert!(!feat.is_empty());
            titles_by_locale.push((sec, opt, feat));
        }
        // All 4 locales produce distinct section, option, and feature labels.
        let sections: std::collections::HashSet<_> = titles_by_locale.iter().map(|t| t.0).collect();
        let options: std::collections::HashSet<_> = titles_by_locale.iter().map(|t| t.1).collect();
        let features: std::collections::HashSet<_> = titles_by_locale.iter().map(|t| t.2).collect();
        assert_eq!(sections.len(), 4);
        assert_eq!(options.len(), 4);
        assert_eq!(features.len(), 4);
        assert_eq!(section_title("unknown_id"), section_title("other"));
        assert_eq!(
            option_title("unknownField", "unknownOpt", "Raw Fallback"),
            "Raw Fallback"
        );
    }
}
