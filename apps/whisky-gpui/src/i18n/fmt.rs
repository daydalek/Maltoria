//! Composition-only formatting helpers; all text lives in `locales/*.toml`.

use crate::i18n::{lookup, s};

pub(crate) fn play_time(hours: u64, minutes: u64) -> String {
    let st = &s().statistics;
    match (hours, minutes) {
        (0, m) => st.play_time_minutes(m),
        (h, 0) => st.play_time_hours(h),
        (h, m) => st.play_time_hours_minutes(h, m),
    }
}

pub(crate) fn statistics_duration(minutes: u64) -> String {
    if minutes < 60 {
        play_time(0, minutes)
    } else {
        play_time(minutes / 60, minutes % 60)
    }
}

pub(crate) fn statistics_month(month: u32) -> String {
    lookup(s().maps.month_name, &month.to_string())
        .unwrap_or_default()
        .to_owned()
}

pub(crate) fn sort_tooltip(current: &str, reversed: bool) -> String {
    let lib = &s().library;
    let order = if reversed { lib.sort_order_desc } else { "" };
    lib.sort_tooltip_current(current, order)
}

pub(crate) fn engine_features_summary(unverified: &[&str], unavailable: &[&str]) -> String {
    let st = &s().settings;
    let mut summary = String::new();
    if !unverified.is_empty() {
        summary.push_str(&st.features_unverified(unverified.join(st.features_list_sep)));
    }
    if !unavailable.is_empty() {
        if !summary.is_empty() {
            summary.push_str(st.features_separator);
        }
        summary.push_str(&st.features_unavailable(unavailable.join(st.features_list_sep)));
    }
    summary
}

pub(crate) fn sidebar_title_with_bottle(page: &str, bottle: &str) -> String {
    s().windows.sidebar_title_bottle(bottle, page)
}

pub(crate) fn notice_bottle_renamed(name: Option<&str>) -> String {
    let st = &s().state;
    match name {
        Some(n) => st.notice_bottle_renamed(n),
        None => st.notice_bottle_renamed_anon.to_owned(),
    }
}

pub(crate) fn notice_bottle_duplicated(name: Option<&str>) -> String {
    let st = &s().state;
    match name {
        Some(n) => st.notice_bottle_duplicated(n),
        None => st.notice_bottle_duplicated_anon.to_owned(),
    }
}

pub(crate) fn notice_bottle_exported(path: Option<&str>) -> String {
    let st = &s().state;
    match path {
        Some(p) => st.notice_bottle_exported(p),
        None => st.notice_bottle_exported_anon.to_owned(),
    }
}

pub(crate) fn notice_gamedb_applied(count: usize, missing: Option<&str>) -> String {
    let st = &s().state;
    match missing {
        Some(m) => st.notice_gamedb_applied_missing(count, m),
        None => st.notice_gamedb_applied(count),
    }
}

pub(crate) fn notice_sync_done(matched: usize, failed: usize) -> String {
    let st = &s().state;
    if failed > 0 {
        st.notice_sync_done_full(failed, matched)
    } else {
        st.notice_sync_done(matched)
    }
}

pub(crate) fn notice_wine_installed(version: Option<&str>) -> String {
    let st = &s().state;
    match version {
        Some(v) => st.notice_wine_installed(v),
        None => st.notice_wine_installed_anon.to_owned(),
    }
}

pub(crate) fn overlay_opacity_label(pct: f32) -> String {
    s().linux_hud.overlay_opacity(format!("{pct:.0}"))
}

pub(crate) fn capture_and_subtitle_errors(subtitle_error: &str, capture_error: &str) -> String {
    let st = &s().capture;
    match (subtitle_error.is_empty(), capture_error.is_empty()) {
        (false, false) => st.capture_both_errors(capture_error, subtitle_error),
        (false, true) => st.capture_subtitle_error(subtitle_error),
        (true, false) => st.capture_capture_error(capture_error),
        (true, true) => String::new(),
    }
}

