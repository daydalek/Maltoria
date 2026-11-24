use std::collections::BTreeMap;
use std::time::Duration;

use gpui_kit::component::input::InputState;
use gpui_kit::{Context, Entity};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    bridge::BridgeSupervisor,
    state::WhiskyApp,
    translation::{ProviderDto, next_request_id},
};

const MAX_AUTO_ATTACH_ATTEMPTS: u8 = 8;

pub(crate) fn native_popup_strings() -> serde_json::Value {
    let strings = crate::i18n::s();
    let mut values: serde_json::Map<String, Value> = [
        ("hud_title", strings.capture.title.to_string()),
        ("overlay_title", strings.capture.overlay_generic_title.to_string()),
        ("capture_unavailable", strings.linux_hud.capture_unavailable.to_string()),
        ("no_provider", strings.linux_hud.no_provider_notice.to_string()),
        ("pick_provider", strings.linux_hud.pick_provider_then_scan.to_string()),
        ("pick_game", strings.capture.err_select_game_first.to_string()),
        ("preset_target_missing", strings.linux_hud.preset_target_missing.to_string()),
        ("attached", strings.linux_hud.attached.to_string()),
        ("standby", strings.linux_hud.standby.to_string()),
        ("expand", strings.common.expand.to_string()),
        ("collapse", strings.common.collapse.to_string()),
        ("close", strings.common.close.to_string()),
        ("subtitle_overlay", strings.linux_hud.subtitle_overlay.to_string()),
        ("sub_on", strings.linux_hud.sub_on.to_string()),
        ("sub_off", strings.linux_hud.sub_off.to_string()),
        ("draggable", strings.capture.draggable_unlocked.to_string()),
        ("locked", strings.linux_hud.locked_label.to_string()),
        ("release_mouse", strings.capture.release_mouse_btn.to_string()),
        ("refresh", strings.common.refresh.to_string()),
        ("waiting_game", strings.linux_hud.waiting_game.to_string()),
        ("error", strings.common.error.to_string()),
        ("pick_process", strings.linux_hud.pick_process.to_string()),
        ("connecting", strings.dialogs.bridge_connecting.to_string()),
        ("attached_target", strings.linux_hud.attached_target("{exe}").to_string()),
        ("standby_scan", strings.linux_hud.standby.to_string()),
        ("architecture_label", strings.linux_hud.architecture_label("{arch}").to_string()),
        ("waiting_launch_detail", strings.linux_hud.waiting_launch_detail.to_string()),
        ("attaching_detail", strings.linux_hud.attaching_detail.to_string()),
        ("starting_helper_detail", strings.linux_hud.starting_helper_detail.to_string()),
        ("starting_label", strings.linux_hud.starting_label.to_string()),
        ("start_scan", strings.linux_hud.start_scan.to_string()),
        ("preset_attach_label", strings.linux_hud.preset_attach_label.to_string()),
        ("preset_attach_target", strings.linux_hud.preset_attach_target("{exe}").to_string()),
        ("process_empty_detail", strings.linux_hud.process_empty_detail.to_string()),
        ("process_metadata", strings.linux_hud.process_metadata("{arch}", "{pid}").to_string()),
        ("attach", strings.capture.attach_selected.to_string()),
        ("game_process", strings.linux_hud.game_process_label.to_string()),
        ("detach", strings.capture.detach.to_string()),
        ("collapse_ball", strings.linux_hud.collapse_ball_label.to_string()),
        ("waiting_text", strings.linux_hud.waiting_text_output.to_string()),
        ("thread", strings.linux_hud.thread_label.to_string()),
        ("empty_preview", strings.capture.empty_preview_placeholder.to_string()),
        ("role_body", strings.capture.role_body.to_string()),
        ("role_speaker", strings.capture.role_speaker.to_string()),
        ("role_choice", strings.capture.role_choice.to_string()),
        ("role_ignore", strings.capture.role_ignore.to_string()),
        ("translating", strings.capture.overlay_translating.to_string()),
        ("translation_failed", strings.capture.overlay_failed.to_string()),
        ("translation_cancelled", strings.capture.overlay_cancelled.to_string()),
    ].into_iter().map(|(id, text)| (id.to_owned(), Value::String(text))).collect();
    for map in [strings.maps.bridge_capture, strings.maps.bridge_detail,
        strings.maps.bridge_static, strings.maps.bridge_common] {
        for &(code, text) in map {
            values.insert(format!("error:{code}"), json!(text));
        }
    }
    for &(code, text) in strings.maps.bridge_capture {
        values.insert(format!("error:capture_{code}"), json!(text));
    }
    Value::Object(values)
}
const AUTO_ATTACH_RETRY_INTERVAL: Duration = Duration::from_millis(1200);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessRecord {
    #[serde(rename = "guestPID")]
    pub(crate) guest_pid: u32,
    pub(crate) exe_path: String,
    pub(crate) arch: String,
    pub(crate) creation_time: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureThread {
    pub(crate) id: String,
    #[serde(rename = "guestPID")]
    pub(crate) guest_pid: u32,
    pub(crate) address_hex: String,
    pub(crate) context_hex: String,
    pub(crate) context2_hex: String,
    pub(crate) hook_code: String,
    pub(crate) hook_name: String,
    pub(crate) preview: String,
    pub(crate) role: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubtitleEntry {
    pub(crate) source: String,
    pub(crate) translation: String,
    pub(crate) speaker: Option<String>,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Subtitle {
    pub(crate) source: String,
    pub(crate) translation: String,
    pub(crate) speaker: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) translation_truncated: bool,
    #[serde(default)]
    pub(crate) entries: Vec<SubtitleEntry>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CaptureThreadBinding {
    pub(crate) role: String,
    #[serde(default)]
    pub(crate) hook_code: String,
    #[serde(default)]
    pub(crate) hook_name: String,
    #[serde(default)]
    pub(crate) context_hex: String,
    #[serde(default)]
    pub(crate) context2_hex: String,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturePreset {
    #[serde(rename = "providerID", default)]
    pub(crate) provider_id: Option<String>,
    #[serde(default)]
    pub(crate) architecture: String,
    #[serde(default)]
    pub(crate) restore_history: bool,
    #[serde(default)]
    pub(crate) auto_attach: bool,
    #[serde(default)]
    pub(crate) translation_enabled: bool,
    #[serde(default = "default_true")]
    pub(crate) prompt_attach_on_launch: bool,
    #[serde(default)]
    pub(crate) target_exe_path: Option<String>,
    #[serde(default)]
    pub(crate) custom_hook_codes: Vec<String>,
    #[serde(default)]
    pub(crate) thread_bindings: Vec<CaptureThreadBinding>,
}

impl Default for CapturePreset {
    fn default() -> Self {
        Self {
            provider_id: None,
            architecture: String::new(),
            restore_history: false,
            auto_attach: false,
            translation_enabled: false,
            prompt_attach_on_launch: true,
            target_exe_path: None,
            custom_hook_codes: Vec::new(),
            thread_bindings: Vec::new(),
        }
    }
}

impl CapturePreset {
    pub(crate) fn has_saved_state(&self) -> bool {
        self.target_exe_path
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty())
            || !self.thread_bindings.is_empty()
            || !self.custom_hook_codes.is_empty()
            || self.auto_attach
    }

    pub(crate) fn has_auto_attach_target(&self) -> bool {
        self.auto_attach
            && self
                .target_exe_path
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty())
    }

    pub(crate) fn binding_counts(&self) -> (usize, usize, usize) {
        let mut body = 0;
        let mut speaker = 0;
        let mut choice = 0;
        for binding in &self.thread_bindings {
            match binding.role.as_str() {
                "body" => body += 1,
                "speaker" => speaker += 1,
                "choice" => choice += 1,
                _ => {}
            }
        }
        (body, speaker, choice)
    }
}

pub(crate) fn exe_basename(path: &str) -> &str {
    let trimmed = path.trim();
    trimmed
        .rsplit(['\\', '/'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(trimmed)
}

pub(crate) struct FilteredThreads<'a> {
    pub(crate) threads: Vec<&'a CaptureThread>,
    pub(crate) hidden_empty_count: usize,
}

fn thread_has_assigned_role(t: &CaptureThread, thread_roles: &BTreeMap<String, String>) -> bool {
    let effective_role = thread_roles
        .get(&t.id)
        .map(String::as_str)
        .or(t.role.as_deref());
    matches!(effective_role, Some("body" | "speaker" | "choice"))
}

pub(crate) fn filter_threads<'a>(
    threads: &'a [CaptureThread],
    thread_roles: &BTreeMap<String, String>,
    non_empty_only: bool,
    query: &str,
) -> FilteredThreads<'a> {
    let q = query.trim().to_lowercase();
    let mut visible = Vec::with_capacity(threads.len());
    let mut hidden_empty_count = 0usize;

    for thread in threads {
        let matches_query = q.is_empty()
            || thread.hook_name.to_lowercase().contains(&q)
            || thread.hook_code.to_lowercase().contains(&q)
            || thread.preview.to_lowercase().contains(&q)
            || thread.id.to_lowercase().contains(&q);
        if !matches_query {
            continue;
        }
        if non_empty_only
            && thread.preview.trim().is_empty()
            && !thread_has_assigned_role(thread, thread_roles)
        {
            hidden_empty_count += 1;
            continue;
        }
        visible.push(thread);
    }

    FilteredThreads {
        threads: visible,
        hidden_empty_count,
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    #[serde(rename = "sessionID")]
    pub(crate) session_id: Option<String>,
    #[serde(rename = "gameID")]
    pub(crate) game_id: Option<String>,
    pub(crate) capture_state: String,
    pub(crate) last_error: Option<String>,
    pub(crate) target: Option<ProcessRecord>,
    pub(crate) subtitle: Subtitle,
    pub(crate) cursor: u64,
    pub(crate) processes: Vec<ProcessRecord>,
    pub(crate) history_count: u64,
    pub(crate) threads: Vec<CaptureThread>,
    pub(crate) threads_total: usize,
    #[serde(default)]
    pub(crate) auto_bound_count: u64,
    #[serde(default)]
    pub(crate) preset: Option<CapturePreset>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistorySource {
    #[serde(rename = "eventID")]
    pub(crate) event_id: String,
    pub(crate) revision: i64,
    #[serde(rename = "streamID")]
    pub(crate) stream_id: String,
    pub(crate) generation: i64,
    pub(crate) text: String,
    pub(crate) speaker: Option<String>,
    pub(crate) captured_at: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryEntry {
    pub(crate) id: String,
    pub(crate) source: HistorySource,
    pub(crate) translated_text: Option<String>,
    pub(crate) translation_truncated: bool,
    pub(crate) status: String,
    pub(crate) include_in_context: bool,
    pub(crate) failure_description: Option<String>,
    pub(crate) created_at: f64,
}

fn default_overlay_text_color() -> String {
    "#FFFFFF".into()
}

fn default_overlay_source_text_color() -> String {
    "#D4D4D8".into()
}

fn default_overlay_max_entries() -> u32 {
    1
}

pub(crate) fn is_valid_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlaySettings {
    pub(crate) visible: bool,
    pub(crate) click_through: bool,
    pub(crate) show_source: bool,
    pub(crate) show_translation: bool,
    #[serde(default = "default_overlay_max_entries")]
    pub(crate) max_entries: u32,
    pub(crate) font_size: f64,
    pub(crate) opacity: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) x: Option<f64>,
    pub(crate) y: Option<f64>,
    #[serde(default = "default_overlay_text_color")]
    pub(crate) text_color: String,
    #[serde(default = "default_overlay_source_text_color")]
    pub(crate) source_text_color: String,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            visible: false,
            click_through: true,
            show_source: true,
            show_translation: true,
            max_entries: default_overlay_max_entries(),
            font_size: 28.0,
            opacity: 0.85,
            width: 900.0,
            height: 180.0,
            x: None,
            y: None,
            text_color: default_overlay_text_color(),
            source_text_color: default_overlay_source_text_color(),
        }
    }
}

impl OverlaySettings {
    pub(crate) fn validate(&self) -> bool {
        (1..=10).contains(&self.max_entries)
            && self.font_size.is_finite()
            && (12.0..=72.0).contains(&self.font_size)
            && self.opacity.is_finite()
            && (0.0..=1.0).contains(&self.opacity)
            && self.width.is_finite()
            && (320.0..=3840.0).contains(&self.width)
            && self.height.is_finite()
            && (100.0..=1200.0).contains(&self.height)
            && self.x.is_none_or(f64::is_finite)
            && self.y.is_none_or(f64::is_finite)
            && (self.show_source || self.show_translation)
            && is_valid_hex_color(&self.text_color)
            && is_valid_hex_color(&self.source_text_color)
    }
}

fn step_overlay_opacity(current: f64, delta: f64) -> f64 {
    (((current + delta).clamp(0.0, 1.0)) * 1000.0).round() / 1000.0
}

pub(crate) fn apply_overlay_action(s: &mut OverlaySettings, action: &str) {
    match action {
        "visible" => s.visible = !s.visible,
        "clickThrough" => s.click_through = !s.click_through,
        "showSource" => s.show_source = !s.show_source,
        "showTranslation" => s.show_translation = !s.show_translation,
        "maxEntries-" => s.max_entries = s.max_entries.saturating_sub(1).max(1),
        "maxEntries+" => s.max_entries = (s.max_entries + 1).min(10),
        "font-" => s.font_size -= 1.0,
        "font+" => s.font_size += 1.0,
        "opacity-" => s.opacity = step_overlay_opacity(s.opacity, -0.05),
        "opacity+" => s.opacity = step_overlay_opacity(s.opacity, 0.05),
        "width-" => s.width -= 20.0,
        "width+" => s.width += 20.0,
        "height-" => s.height -= 20.0,
        "height+" => s.height += 20.0,
        "x-" => s.x = Some(s.x.unwrap_or(100.0) - 20.0),
        "x+" => s.x = Some(s.x.unwrap_or(100.0) + 20.0),
        "y-" => s.y = Some(s.y.unwrap_or(100.0) - 20.0),
        "y+" => s.y = Some(s.y.unwrap_or(100.0) + 20.0),
        "reset" => {
            s.x = None;
            s.y = None;
        }
        _ => {
            if let Some(hex) = action.strip_prefix("textColor:") {
                s.text_color = hex.to_owned();
            } else if let Some(hex) = action.strip_prefix("sourceColor:") {
                s.source_text_color = hex.to_owned();
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Pending {
    method: String,
    generation: u64,
    session_id: Option<String>,
    snapshot_owner: Option<(String, String, String)>,
    context: Option<(String, String, String)>,
    preset_key: Option<(String, String)>,
    role_edit_generation: u64,
}

#[derive(Clone)]
pub(crate) struct CaptureState {
    #[cfg(target_os = "linux")]
    pub(crate) hud: crate::linux_hud::HudState,
    pub(crate) panel_open: bool,
    pub(crate) context: Option<(String, String, String)>,
    pub(crate) active_game: Option<(String, String, String)>,
    pub(crate) history_owner: Option<(String, String, String)>,
    pub(crate) provider_id: Option<String>,
    pub(crate) architecture: String,
    pub(crate) restore_history: bool,
    pub(crate) preset: Option<CapturePreset>,
    pub(crate) presets_by_game: BTreeMap<(String, String), CapturePreset>,
    pub(crate) preset_notice: Option<String>,
    pub(crate) auto_attach_pending: bool,
    pub(crate) thread_filter_non_empty: bool,
    pub(crate) thread_filter_query: String,
    pub(crate) thread_filter_input: Option<Entity<InputState>>,
    pub(crate) snapshot: Option<Snapshot>,
    pub(crate) processes: Vec<ProcessRecord>,
    pub(crate) truncated: bool,
    pub(crate) selected_process: Option<ProcessRecord>,
    pub(crate) thread_offset: usize,
    pub(crate) thread_total: usize,
    pub(crate) threads: Vec<CaptureThread>,
    pub(crate) thread_roles: BTreeMap<String, String>,
    pub(crate) history_offset: usize,
    pub(crate) history_total: usize,
    pub(crate) history: Vec<HistoryEntry>,
    pub(crate) history_text: BTreeMap<(String, String), String>,
    history_text_progress: Option<(String, String, usize)>,
    pub(crate) hook_code: String,
    pub(crate) hook_input: Option<Entity<InputState>>,
    pub(crate) error: Option<String>,
    pub(crate) overlay: Option<OverlaySettings>,
    pub(crate) overlay_platform: String,
    pub(crate) overlay_supports_click_through: bool,
    pending: BTreeMap<u64, Pending>,
    generation: u64,
    poll_request: Option<u64>,
    poll_scheduled: bool,
    last_terminal: String,
    auto_attach_on_preset_load: bool,
    preset_needs_apply: bool,
    auto_attach_attempts: u8,
    auto_attach_retry_scheduled: bool,
    role_edit_generation: u64,
}

impl Default for CaptureState {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            hud: crate::linux_hud::HudState::default(),
            panel_open: false,
            context: None,
            active_game: None,
            history_owner: None,
            provider_id: None,
            architecture: String::new(),
            restore_history: false,
            preset: None,
            presets_by_game: BTreeMap::new(),
            preset_notice: None,
            auto_attach_pending: false,
            thread_filter_non_empty: true,
            thread_filter_query: String::new(),
            thread_filter_input: None,
            snapshot: None,
            processes: Vec::new(),
            truncated: false,
            selected_process: None,
            thread_offset: 0,
            thread_total: 0,
            threads: Vec::new(),
            thread_roles: BTreeMap::new(),
            history_offset: 0,
            history_total: 0,
            history: Vec::new(),
            history_text: BTreeMap::new(),
            history_text_progress: None,
            hook_code: String::new(),
            hook_input: None,
            error: None,
            overlay: None,
            overlay_platform: String::new(),
            overlay_supports_click_through: false,
            pending: BTreeMap::new(),
            generation: 0,
            poll_request: None,
            poll_scheduled: false,
            last_terminal: String::new(),
            auto_attach_on_preset_load: false,
            preset_needs_apply: false,
            auto_attach_attempts: 0,
            auto_attach_retry_scheduled: false,
            role_edit_generation: 0,
        }
    }
}

impl CaptureState {
    pub(crate) fn clear_window_inputs(&mut self) {
        self.hook_input = None;
        self.thread_filter_input = None;
    }

    pub(crate) fn set_context(&mut self, context: Option<(String, String, String)>) {
        if self.context != context {
            self.context = context;
            self.provider_id = None;
            self.preset = self
                .context
                .as_ref()
                .and_then(|(c, e, _)| self.presets_by_game.get(&(c.clone(), e.clone())).cloned());
            self.preset_notice = None;
            self.preset_needs_apply = false;
            self.auto_attach_on_preset_load = false;
            self.auto_attach_pending = false;
            self.auto_attach_attempts = 0;
            self.auto_attach_retry_scheduled = false;
            self.pending.retain(|_, p| {
                !(p.method.starts_with("translation.capture.preset/") && p.preset_key.is_none())
            });
        }
    }

    pub(crate) fn preset_for_game(
        &self,
        container: &str,
        entry_id: &str,
    ) -> Option<&CapturePreset> {
        self.presets_by_game
            .get(&(container.to_owned(), entry_id.to_owned()))
            .or_else(|| {
                self.context
                    .as_ref()
                    .filter(|(c, e, _)| c == container && e == entry_id)
                    .and(self.preset.as_ref())
            })
    }

    pub(crate) fn effective_architecture(&self) -> &str {
        if self.architecture.is_empty() {
            "x64"
        } else {
            &self.architecture
        }
    }

    pub(crate) fn is_actively_attached(&self) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|s| s.target.is_some() || s.capture_state == "attached")
    }

    pub(crate) fn context_matches_active_game(&self) -> bool {
        self.active_game.as_ref().zip(self.context.as_ref())
            .is_some_and(|(active, context)| active.0 == context.0 && active.1 == context.1)
    }

    pub(crate) fn is_selecting(&self) -> bool {
        self.active_game.is_some()
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| s.capture_state == "selecting" && s.target.is_none())
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.active_game.is_none()
            && !self.is_actively_attached()
            && !self.is_pending("translation.capture.start")
            && !self.is_pending("translation.capture.attach")
            && !self.is_pending("translation.capture.autoAttach")
    }

    pub(crate) fn request(
        &mut self,
        bridge: &BridgeSupervisor,
        method: &str,
        params: Value,
    ) -> Option<u64> {
        if self.pending.values().any(|p| p.method == method) {
            return None;
        }
        let id = next_request_id();
        if let Err(error) = bridge.translation_request(id, method, params) {
            self.error = Some(error);
            return None;
        }
        self.pending.insert(
            id,
            Pending {
                method: method.into(),
                generation: self.generation,
                session_id: self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.session_id.clone()),
                context: self.context.clone(),
                preset_key: None,
                snapshot_owner: self.active_game.clone(),
                role_edit_generation: self.role_edit_generation,
            },
        );
        if method == "translation.snapshot" {
            self.poll_request = Some(id);
        }
        Some(id)
    }
    pub(crate) fn is_pending(&self, method: &str) -> bool {
        self.pending
            .values()
            .any(|pending| pending.method == method)
    }
    pub(crate) fn history_belongs_to_context(&self) -> bool {
        self.history_owner.is_some() && self.history_owner == self.context
    }

    fn request_for_preset(
        &mut self,
        bridge: &BridgeSupervisor,
        method: &str,
        container: &str,
        entry_id: &str,
        params: Value,
    ) -> Option<u64> {
        let key = (container.to_owned(), entry_id.to_owned());
        if self
            .pending
            .values()
            .any(|p| p.method == method && p.preset_key.as_ref() == Some(&key))
        {
            return None;
        }
        self.pending
            .retain(|_, p| !(p.method == method && p.preset_key.is_some()));
        let id = next_request_id();
        if let Err(error) = bridge.translation_request(id, method, params) {
            self.error = Some(error);
            return None;
        }
        self.pending.insert(
            id,
            Pending {
                method: method.into(),
                generation: self.generation,
                session_id: self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.session_id.clone()),
                context: self.context.clone(),
                preset_key: Some(key),
                snapshot_owner: None,
                role_edit_generation: self.role_edit_generation,
            },
        );
        Some(id)
    }

    pub(crate) fn load_preset_for_game(
        &mut self,
        bridge: &BridgeSupervisor,
        container: &str,
        entry_id: &str,
    ) {
        let _ = self.request_for_preset(
            bridge,
            "translation.capture.preset/get",
            container,
            entry_id,
            json!({"container": container, "entryID": entry_id}),
        );
    }

    pub(crate) fn load_preset_for_context(&mut self, bridge: &BridgeSupervisor) {
        let Some((container, entry_id, _)) = &self.context else {
            return;
        };
        let container = container.clone();
        let entry_id = entry_id.clone();
        self.auto_attach_on_preset_load = true;
        if self
            .request_for_preset(
                bridge,
                "translation.capture.preset/get",
                &container,
                &entry_id,
                json!({"container": container, "entryID": entry_id}),
            )
            .is_none()
        {
            self.auto_attach_on_preset_load = false;
        }
    }

    pub(crate) fn toggle_preset_translation_enabled(
        &mut self,
        bridge: &BridgeSupervisor,
        container: &str,
        entry_id: &str,
        enabled: bool,
    ) {
        let existing = self.preset_for_game(container, entry_id).cloned();
        let mut payload = serde_json::Map::new();
        payload.insert("container".into(), json!(container));
        payload.insert("entryID".into(), json!(entry_id));
        payload.insert("translationEnabled".into(), json!(enabled));
        if enabled {
            if existing.as_ref().is_none_or(|p| p.provider_id.is_none())
                && let Some(pid) = self.provider_id.as_deref()
            {
                payload.insert("providerID".into(), json!(pid));
            }
            if existing
                .as_ref()
                .is_none_or(|p| p.architecture.trim().is_empty())
            {
                payload.insert("architecture".into(), json!(self.effective_architecture()));
            }
        }
        self.preset_notice = None;
        let _ = self.request_for_preset(
            bridge,
            "translation.capture.preset/save",
            container,
            entry_id,
            Value::Object(payload),
        );
    }

    pub(crate) fn toggle_preset_prompt_on_launch(
        &mut self,
        bridge: &BridgeSupervisor,
        container: &str,
        entry_id: &str,
        prompt: bool,
    ) {
        self.preset_notice = None;
        let _ = self.request_for_preset(
            bridge,
            "translation.capture.preset/save",
            container,
            entry_id,
            json!({
                "container": container,
                "entryID": entry_id,
                "promptAttachOnLaunch": prompt,
            }),
        );
    }

    pub(crate) fn open_capture_hud(
        &mut self,
        _bridge: &BridgeSupervisor,
        container: &str,
        entry_id: &str,
    ) {
        #[cfg(target_os = "linux")]
        {
            self.hud.open_requested = Some((container.into(), entry_id.into()));
        }
        #[cfg(not(target_os = "linux"))]
        let _ = self.request(
            _bridge,
            "translation.capture.hud/show",
            json!({
                "container": container,
                "entryID": entry_id,
                "strings": native_popup_strings(),
            }),
        );
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn save_hud_selection(&mut self, bridge: &BridgeSupervisor) {
        let Some((container, entry, _)) = self.context.clone() else { return };
        let _ = self.request_for_preset(bridge, "translation.capture.preset/save", &container, &entry, json!({
            "container": container, "entryID": entry, "providerID": self.provider_id,
            "architecture": self.effective_architecture(), "restoreHistory": self.restore_history
        }));
    }

    pub(crate) fn toggle_preset_auto_attach(&mut self, bridge: &BridgeSupervisor) {
        let Some((container, entry_id, _)) = &self.context else {
            return;
        };
        let container = container.clone();
        let entry_id = entry_id.clone();
        let mut next = self.preset.clone().unwrap_or_default();
        next.auto_attach = !next.auto_attach;
        if !self.is_actively_attached() {
            if self.provider_id.is_some() {
                next.provider_id = self.provider_id.clone();
            }
            next.architecture = self.effective_architecture().to_owned();
            next.restore_history = self.restore_history;
        } else {
            if next.provider_id.is_none() {
                next.provider_id = self.provider_id.clone();
            }
            if next.architecture.is_empty() {
                next.architecture = self.effective_architecture().to_owned();
            }
        }
        let mut payload = serde_json::Map::new();
        payload.insert("container".into(), json!(container));
        payload.insert("entryID".into(), json!(entry_id));
        payload.insert("autoAttach".into(), json!(next.auto_attach));
        payload.insert("restoreHistory".into(), json!(next.restore_history));
        payload.insert("architecture".into(), json!(next.architecture));
        if let Some(pid) = next.provider_id.as_deref() {
            payload.insert("providerID".into(), json!(pid));
        }
        self.preset_notice = None;
        let _ = self.request_for_preset(
            bridge,
            "translation.capture.preset/save",
            &container,
            &entry_id,
            Value::Object(payload),
        );
    }

    pub(crate) fn clear_preset(&mut self, bridge: &BridgeSupervisor) {
        let Some((container, entry_id, _)) = &self.context else {
            return;
        };
        let container = container.clone();
        let entry_id = entry_id.clone();
        self.auto_attach_on_preset_load = false;
        self.preset_needs_apply = false;
        let _ = self.request_for_preset(
            bridge,
            "translation.capture.preset/clear",
            &container,
            &entry_id,
            json!({"container": container, "entryID": entry_id}),
        );
    }

    pub(crate) fn start(&mut self, bridge: &BridgeSupervisor) {
        self.start_with_auto_attach(bridge, false);
    }

    pub(crate) fn start_with_auto_attach(&mut self, bridge: &BridgeSupervisor, auto_attach: bool) {
        let Some((container, entry_id, _)) = &self.context else {
            self.error = Some(crate::i18n::s().capture.err_select_game_first.into());
            return;
        };
        let Some(provider_id) = &self.provider_id else {
            self.error = Some(crate::i18n::s().capture.err_select_saved_provider.into());
            return;
        };
        if self.active_game.is_some() || self.is_pending("translation.capture.start") {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.role_edit_generation = self.role_edit_generation.wrapping_add(1);
        self.pending.clear();
        self.poll_request = None;
        self.poll_scheduled = false;
        self.snapshot = None;
        self.error = None;
        self.preset_notice = None;
        self.auto_attach_pending = auto_attach;
        self.auto_attach_attempts = 0;
        self.auto_attach_retry_scheduled = false;
        self.processes.clear();
        self.selected_process = None;
        self.thread_roles.clear();
        self.threads.clear();
        self.thread_total = 0;
        self.thread_offset = 0;
        self.history.clear();
        self.history_text.clear();
        self.history_text_progress = None;
        self.history_offset = 0;
        self.history_total = 0;
        self.active_game = Some((
            container.clone(),
            entry_id.clone(),
            self.context.as_ref().unwrap().2.clone(),
        ));
        self.history_owner = self.active_game.clone();
        let arch = self.effective_architecture().to_owned();
        if self
            .request(
                bridge,
                "translation.capture.start",
                json!({
                    "container": container,
                    "entryID": entry_id,
                    "providerID": provider_id,
                    "architecture": arch,
                    "restoreHistory": self.restore_history,
                    "autoAttach": auto_attach
                }),
            )
            .is_none()
        {
            self.active_game = None;
            self.auto_attach_pending = false;
        }
    }

    pub(crate) fn request_auto_attach(&mut self, bridge: &BridgeSupervisor) {
        let params = if let Some((container, entry_id, _)) = &self.context {
            json!({"container": container, "entryID": entry_id})
        } else {
            json!({})
        };
        let _ = self.request(bridge, "translation.capture.autoAttach", params);
    }

    pub(crate) fn trigger_manual_auto_attach(&mut self, bridge: &BridgeSupervisor) {
        self.auto_attach_pending = true;
        self.auto_attach_attempts = 0;
        self.auto_attach_retry_scheduled = false;
        self.preset_notice = None;
        self.request_auto_attach(bridge);
    }

    pub(crate) fn schedule_auto_attach_retry(&mut self, cx: &mut Context<WhiskyApp>) {
        if !self.auto_attach_pending
            || self.active_game.is_none()
            || self.auto_attach_retry_scheduled
            || self.auto_attach_attempts >= MAX_AUTO_ATTACH_ATTEMPTS
        {
            return;
        }
        self.auto_attach_retry_scheduled = true;
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(AUTO_ATTACH_RETRY_INTERVAL)
                .await;
            let _ = this.update(cx, |app, cx| {
                if app.capture.generation != generation {
                    return;
                }
                app.capture.auto_attach_retry_scheduled = false;
                if app.capture.auto_attach_pending
                    && app.capture.is_selecting()
                    && !app.capture.is_pending("translation.capture.autoAttach")
                {
                    app.capture.request_auto_attach(&app.bridge);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn apply_pending_preset(&mut self, providers: &[ProviderDto]) {
        if !self.preset_needs_apply {
            return;
        }
        self.preset_needs_apply = false;
        let Some(preset) = &self.preset else {
            return;
        };
        if self.is_actively_attached() && self.context_matches_active_game() {
            return;
        }
        if let Some(pid) = &preset.provider_id {
            if providers.iter().any(|p| &p.id == pid) {
                self.provider_id = Some(pid.clone());
            }
        }
        if !preset.architecture.trim().is_empty() {
            self.architecture = preset.architecture.clone();
        }
        self.restore_history = preset.restore_history;
    }

    pub(crate) fn maybe_auto_attach_on_preset_load(
        &mut self,
        bridge: &BridgeSupervisor,
        probe_pending: bool,
    ) {
        if !self.auto_attach_on_preset_load || self.is_pending("translation.capture.preset/get") {
            return;
        }
        self.auto_attach_on_preset_load = false;
        let Some(preset) = &self.preset else {
            return;
        };
        if preset.has_auto_attach_target()
            && self.is_idle()
            && self.provider_id.is_some()
            && !probe_pending
        {
            self.start_with_auto_attach(bridge, true);
        }
    }

    pub(crate) fn on_game_launched(
        &mut self,
        bridge: &BridgeSupervisor,
        probe_pending: bool,
        cx: &mut Context<WhiskyApp>,
    ) {
        let Some(preset) = self.preset.clone() else {
            return;
        };
        if preset.has_auto_attach_target() {
            if self.is_idle() {
                if self.provider_id.is_some() && !probe_pending {
                    self.start_with_auto_attach(bridge, true);
                    if self.active_game.is_some() {
                        self.preset_notice =
                            Some(crate::i18n::s().capture.msg_auto_scanning_proc.into());
                    }
                } else {
                    self.preset_notice =
                        Some(crate::i18n::s().capture.msg_launch_need_provider.into());
                }
            } else if self.is_selecting() {
                self.auto_attach_pending = true;
                self.auto_attach_attempts = 0;
                self.schedule_auto_attach_retry(cx);
                self.preset_notice =
                    Some(crate::i18n::s().capture.msg_auto_detecting_proc.into());
            }
        } else if preset.has_saved_state() && (self.is_idle() || self.is_selecting()) {
            self.preset_notice =
                Some(crate::i18n::s().capture.msg_launch_remembered.into());
        }
    }

    pub(crate) fn request_processes(&mut self, bridge: &BridgeSupervisor) {
        let _ = self.request(bridge, "translation.capture.processes", json!({}));
    }
    pub(crate) fn attach(&mut self, bridge: &BridgeSupervisor) {
        if let Some(target) = &self.selected_process {
            self.auto_attach_pending = false;
            self.auto_attach_attempts = 0;
            self.auto_attach_retry_scheduled = false;
            let _ = self.request(
                bridge,
                "translation.capture.attach",
                json!({"target":target}),
            );
        }
    }
    pub(crate) fn stop(&mut self, bridge: &BridgeSupervisor, method: &str) {
        #[cfg(target_os = "linux")]
        { self.hud.hide(); }
        let stopping_game = self.active_game.take();
        self.generation = self.generation.wrapping_add(1);
        self.role_edit_generation = self.role_edit_generation.wrapping_add(1);
        self.pending.clear();
        self.poll_request = None;
        self.poll_scheduled = false;
        self.auto_attach_pending = false;
        self.auto_attach_on_preset_load = false;
        self.auto_attach_attempts = 0;
        self.auto_attach_retry_scheduled = false;
        self.snapshot = None;
        self.processes.clear();
        self.threads.clear();
        self.history_text.clear();
        self.thread_roles.clear();
        self.history_text_progress = None;
        if let Some(owner) = stopping_game
            && let Some(request) = self.request(bridge, method, json!({}))
            && let Some(pending) = self.pending.get_mut(&request)
        {
            pending.snapshot_owner = Some(owner);
        }
    }
    pub(crate) fn refresh_threads(&mut self, bridge: &BridgeSupervisor, offset: usize) {
        self.thread_offset = offset;
        let _ = self.request(
            bridge,
            "translation.threads.list",
            json!({"offset":offset,"maxCount":64}),
        );
    }
    pub(crate) fn refresh_history(&mut self, bridge: &BridgeSupervisor, offset: usize) {
        self.history_offset = offset;
        let _ = self.request(
            bridge,
            "translation.history",
            json!({"offset":offset,"maxCount":20}),
        );
    }
    pub(crate) fn load_history_text(
        &mut self,
        bridge: &BridgeSupervisor,
        entry_id: String,
        field: &str,
    ) {
        if !["source", "translation"].contains(&field)
            || self.is_pending("translation.history.text")
        {
            return;
        }
        self.history_text
            .insert((entry_id.clone(), field.into()), String::new());
        self.history_text_progress = Some((entry_id, field.into(), 0));
        self.request_history_chunk(bridge);
    }
    fn request_history_chunk(&mut self, bridge: &BridgeSupervisor) {
        if let Some((entry_id, field, offset)) = &self.history_text_progress {
            let _ = self.request(
                bridge,
                "translation.history.text",
                json!({
                    "entryID":entry_id,"field":field,"offset":offset,"maxCharacters":16384
                }),
            );
        }
    }
    pub(crate) fn adjust_overlay(&mut self, bridge: &BridgeSupervisor, action: &str) {
        if self.is_pending("translation.overlay.configure") {
            return;
        }
        self.configure_overlay(bridge, |s| apply_overlay_action(s, action));
    }
    fn configure_overlay(
        &mut self,
        bridge: &BridgeSupervisor,
        update: impl FnOnce(&mut OverlaySettings),
    ) {
        let Some(mut settings) = self.overlay.clone() else {
            return;
        };
        update(&mut settings);
        if !settings.validate() {
            self.error = Some(crate::i18n::s().capture.err_overlay_range.into());
            return;
        }
        let _ = self.request(
            bridge,
            "translation.overlay.configure",
            json!({"settings":settings}),
        );
    }
    pub(crate) fn poll(&mut self, bridge: &BridgeSupervisor) {
        if self.can_poll() {
            let _ = self.request(bridge, "translation.snapshot", json!({}));
        }
    }
    fn can_poll(&self) -> bool {
        (self.active_game.is_some() || self.overlay.as_ref().is_some_and(|s| s.visible))
            && self.poll_request.is_none()
    }
    pub(crate) fn schedule_poll(&mut self, cx: &mut Context<WhiskyApp>) {
        if (self.active_game.is_none() && !self.overlay.as_ref().is_some_and(|s| s.visible)) || self.poll_scheduled {
            return;
        }
        self.poll_scheduled = true;
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let _ = this.update(cx, |app, _cx| {
                if app.capture.generation != generation {
                    return;
                }
                app.capture.poll_scheduled = false;
                app.capture.poll(&app.bridge);
            });
        })
        .detach();
    }
    pub(crate) fn disconnected(&mut self, reason: Option<&str>) {
        self.generation = self.generation.wrapping_add(1);
        self.role_edit_generation = self.role_edit_generation.wrapping_add(1);
        self.pending.clear();
        self.poll_request = None;
        self.poll_scheduled = false;
        self.auto_attach_pending = false;
        self.auto_attach_on_preset_load = false;
        self.preset_needs_apply = false;
        self.auto_attach_attempts = 0;
        self.auto_attach_retry_scheduled = false;
        self.active_game = None;
        self.snapshot = None;
        self.processes.clear();
        self.threads.clear();
        self.history.clear();
        self.history_owner = None;
        self.thread_roles.clear();
        self.selected_process = None;
        self.history_text.clear();
        self.history_text_progress = None;
        self.error = Some(
            reason
                .unwrap_or(crate::i18n::s().capture.err_bridge_disconnected)
                .to_owned(),
        );
    }

    fn sync_thread_roles_from_server(
        &mut self,
        threads: &[CaptureThread],
        role_gen_matches: bool,
    ) {
        if !role_gen_matches || self.is_pending("translation.threads.select") {
            return;
        }
        for thread in threads {
            match thread.role.as_deref() {
                Some(role @ ("body" | "speaker" | "choice")) => {
                    self.thread_roles.insert(thread.id.clone(), role.to_owned());
                }
                _ => {
                    self.thread_roles.remove(&thread.id);
                }
            }
        }
    }

    fn apply_snapshot(
        &mut self,
        snapshot: Snapshot,
        method: &str,
        snapshot_owner: Option<&(String, String, String)>,
        role_gen_matches: bool,
        bridge: &BridgeSupervisor,
        cx: &mut Context<WhiskyApp>,
    ) {
        let history_changed = self
            .snapshot
            .as_ref()
            .is_none_or(|old| old.history_count != snapshot.history_count);
        let terminal = snapshot.subtitle.status.clone();
        let terminal_changed = terminal != self.last_terminal
            && matches!(terminal.as_str(), "completed" | "failed" | "cancelled");
        self.last_terminal = terminal;
        self.processes = snapshot.processes.clone();
        if self
            .selected_process
            .as_ref()
            .is_some_and(|selected| !self.processes.contains(selected))
        {
            self.selected_process = None;
        }
        if self.thread_offset == 0 {
            self.threads = snapshot.threads.clone();
        }
        self.sync_thread_roles_from_server(&snapshot.threads, role_gen_matches);
        self.thread_total = snapshot.threads_total;
        if let (Some(preset), Some((container, entry_id, _))) =
            (snapshot.preset.as_ref(), snapshot_owner.or(self.active_game.as_ref()))
        {
            if self.context.as_ref().is_some_and(|context| {
                context.0 == *container && context.1 == *entry_id
            }) {
                self.preset = Some(preset.clone());
            }
            self.presets_by_game
                .insert((container.clone(), entry_id.clone()), preset.clone());
        }
        let stopped = matches!(
            snapshot.capture_state.as_str(),
            "stopped" | "targetExited" | "error"
        );
        let attached = snapshot.capture_state == "attached" || snapshot.target.is_some();
        let selecting = snapshot.capture_state == "selecting" && snapshot.target.is_none();
        self.snapshot = Some(snapshot);

        if stopped || attached {
            self.auto_attach_pending = false;
            self.auto_attach_attempts = 0;
            self.auto_attach_retry_scheduled = false;
            if attached {
                self.preset_notice = None;
            }
        } else if selecting
            && self.auto_attach_pending
            && matches!(
                method,
                "translation.capture.start" | "translation.capture.autoAttach"
            )
        {
            if method == "translation.capture.autoAttach" {
                self.auto_attach_attempts = self.auto_attach_attempts.saturating_add(1);
            }
            if self.auto_attach_attempts < MAX_AUTO_ATTACH_ATTEMPTS {
                self.schedule_auto_attach_retry(cx);
            } else {
                self.auto_attach_pending = false;
                self.auto_attach_retry_scheduled = false;
                self.preset_notice = Some(
                    crate::i18n::s().capture.msg_no_target_proc.into(),
                );
            }
        }

        if stopped {
            self.active_game = None;
        } else if matches!(
            method,
            "translation.capture.start"
                | "translation.capture.attach"
                | "translation.capture.autoAttach"
                | "translation.snapshot"
        ) {
            self.schedule_poll(cx);
        }
        if self.panel_open && (history_changed || terminal_changed) {
            self.refresh_history(bridge, self.history_offset);
        }
        if method == "translation.capture.start" && !stopped {
            self.request_processes(bridge);
            self.refresh_history(bridge, 0);
        }
    }

    pub(crate) fn reply(
        &mut self,
        request: u64,
        method: &str,
        result: Result<Value, String>,
        bridge: &BridgeSupervisor,
        cx: &mut Context<WhiskyApp>,
    ) {
        let Some(pending) = self.pending.remove(&request) else {
            return;
        };
        if !self.reply_is_current(&pending, method) {
            return;
        }
        if self.poll_request == Some(request) {
            self.poll_request = None;
        }
        let role_gen_matches = pending.role_edit_generation == self.role_edit_generation;
        match result {
            Err(error) => self.apply_error(method, error),
            Ok(value) => match method {
                #[cfg(target_os = "linux")]
                "runtime.snapshot" => { self.hud.runtime_snapshot(&value); }
                "translation.capture.start"
                | "translation.capture.attach"
                | "translation.capture.detach"
                | "translation.capture.stop"
                | "translation.snapshot" => {
                    if let Ok(snapshot) = serde_json::from_value::<Snapshot>(value) {
                        self.apply_snapshot(snapshot, method, pending.snapshot_owner.as_ref(), role_gen_matches, bridge, cx);
                    }
                }
                "translation.capture.autoAttach" => {
                    let snapshot_val = value.get("snapshot").cloned().unwrap_or(value);
                    if let Ok(snapshot) = serde_json::from_value::<Snapshot>(snapshot_val) {
                        self.apply_snapshot(snapshot, method, pending.snapshot_owner.as_ref(), role_gen_matches, bridge, cx);
                    }
                }
                "translation.capture.preset/get" | "translation.capture.preset/save" => {
                    let (key, matches_context) = self.preset_reply_target(&pending);
                    match decode_preset_response(&value) {
                        Ok(preset) => {
                            if let Some(key) = key {
                                if let Some(p) = &preset {
                                    self.presets_by_game.insert(key, p.clone());
                                } else {
                                    self.presets_by_game.remove(&key);
                                }
                            }
                            if matches_context {
                                self.preset = preset;
                                self.preset_needs_apply = self.preset.is_some();
                            }
                        }
                        Err(error) => {
                            if matches_context {
                                self.auto_attach_on_preset_load = false;
                                self.preset_needs_apply = false;
                            }
                            self.error = Some(crate::i18n::s().capture.invalid_capture_preset_data(error));
                        }
                    }
                }
                "translation.capture.preset/clear" => {
                    let (key, matches_context) = self.preset_reply_target(&pending);
                    if let Some(key) = key {
                        self.presets_by_game.remove(&key);
                    }
                    if matches_context {
                        self.preset = None;
                        self.preset_needs_apply = false;
                        self.auto_attach_on_preset_load = false;
                        self.auto_attach_pending = false;
                        self.auto_attach_attempts = 0;
                        self.auto_attach_retry_scheduled = false;
                        if let Some(snapshot) = &mut self.snapshot {
                            snapshot.preset = None;
                        }
                        self.preset_notice = Some(crate::i18n::s().capture.msg_cleared_preset.into());
                    }
                }
                "translation.capture.processes" => {
                    if let Ok(page) = serde_json::from_value::<ProcessPage>(value) {
                        self.processes = page.processes;
                        self.truncated = page.truncated;
                        if self
                            .selected_process
                            .as_ref()
                            .is_some_and(|selected| !self.processes.contains(selected))
                        {
                            self.selected_process = None;
                        }
                    }
                }
                "translation.threads.list" => {
                    if let Ok(page) = serde_json::from_value::<ThreadPage>(value) {
                        self.threads = page.threads;
                        self.sync_thread_roles_from_server(&self.threads.clone(), role_gen_matches);
                        self.thread_total = page.total;
                        self.thread_offset = page.next_offset.saturating_sub(self.threads.len());
                    }
                }
                "translation.history" => {
                    if let Ok(page) = serde_json::from_value::<HistoryPage>(value) {
                        self.history = page.entries;
                        self.history_total = page.total;
                        self.history_offset = page.next_offset.saturating_sub(self.history.len());
                    }
                }
                "translation.history.text" => {
                    if let Some((entry_id, field, current_offset)) =
                        self.history_text_progress.clone()
                    {
                        let chunk = value.get("text").and_then(Value::as_str).unwrap_or("");
                        let next =
                            value.get("nextOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
                        let total = value
                            .get("total")
                            .and_then(Value::as_u64)
                            .unwrap_or(next as u64) as usize;
                        if next < total && next <= current_offset {
                            self.error = Some(crate::i18n::s().capture.err_history_stalled.into());
                            self.history_text_progress = None;
                        } else {
                            self.history_text
                                .entry((entry_id.clone(), field.clone()))
                                .or_default()
                                .push_str(chunk);
                        }
                        if next < total && next > current_offset {
                            self.history_text_progress = Some((entry_id, field, next));
                            self.request_history_chunk(bridge);
                        } else if next >= total {
                            self.history_text_progress = None;
                        }
                    }
                }
                "translation.threads.select" => {}
                "translation.history.include" => self.refresh_history(bridge, self.history_offset),
                "translation.history.clear" | "translation.cache.clear" => {
                    self.refresh_history(bridge, 0)
                }
                "translation.overlay.read" | "translation.overlay.configure" => {
                    if let Some(settings) = value
                        .get("settings")
                        .cloned()
                        .and_then(|v| serde_json::from_value(v).ok())
                    {
                        self.overlay = Some(settings);
                    }
                    self.overlay_platform = value
                        .get("platform")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into();
                    self.overlay_supports_click_through = value.get("supportsClickThrough").and_then(Value::as_bool).unwrap_or(false);
                    if self.overlay.as_ref().is_some_and(|s| s.visible) { self.schedule_poll(cx); }
                }
                _ => {}
            },
        }
        if method == "translation.snapshot" && (self.active_game.is_some() || self.overlay.as_ref().is_some_and(|s| s.visible)) {
            self.schedule_poll(cx);
        }
        cx.notify();
    }
    fn preset_reply_target(&self, pending: &Pending) -> (Option<(String, String)>, bool) {
        let key = pending.preset_key.clone().or_else(|| {
            pending
                .context
                .as_ref()
                .map(|(c, e, _)| (c.clone(), e.clone()))
        });
        let matches_context = key.as_ref().is_some_and(|(c, e)| {
            self.context
                .as_ref()
                .is_some_and(|(cc, ce, _)| cc == c && ce == e)
        }) || (key.is_none() && pending.context == self.context);
        (key, matches_context)
    }

    fn reply_is_current(&self, pending: &Pending, method: &str) -> bool {
        if method.starts_with("translation.capture.preset/")
            && pending.preset_key.is_none()
            && pending.context != self.context
        {
            return false;
        }
        pending.generation == self.generation
            && pending.method == method
            && pending.session_id.as_ref().is_none_or(|session| {
                self.snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.session_id.as_ref())
                    == Some(session)
            })
    }
    fn apply_error(&mut self, method: &str, error: String) {
        self.error = Some(error);
        if method == "translation.history.text" {
            self.history_text_progress = None;
        }
        if method == "translation.capture.start" || method == "translation.capture.attach" {
            self.active_game = None;
        }
        if method == "translation.capture.start" || method == "translation.capture.autoAttach" {
            self.auto_attach_pending = false;
            self.auto_attach_retry_scheduled = false;
        }
        if method == "translation.capture.preset/get" {
            self.auto_attach_on_preset_load = false;
            self.preset_needs_apply = false;
        }
    }
    pub(crate) fn select_role(&mut self, bridge: &BridgeSupervisor, thread_id: &str, role: &str) {
        if self.is_pending("translation.threads.select") {
            return;
        }
        let Ok(next) = role_selection(&self.thread_roles, thread_id, role) else {
            self.error = Some(crate::i18n::s().capture.err_thread_limit.into());
            return;
        };
        self.role_edit_generation = self.role_edit_generation.wrapping_add(1);
        self.thread_roles = next;
        let selections: Vec<Value> = self
            .thread_roles
            .iter()
            .map(|(id, role)| json!({"threadID":id,"role":role}))
            .collect();
        let _ = self.request(
            bridge,
            "translation.threads.select",
            json!({"selections":selections}),
        );
    }
}

fn decode_preset_response(value: &Value) -> Result<Option<CapturePreset>, serde_json::Error> {
    if value.is_null() || value.as_object().is_some_and(|map| map.is_empty()) {
        return Ok(None);
    }
    if let Some(preset_val) = value.get("preset") {
        if preset_val.is_null() {
            return Ok(None);
        }
        return serde_json::from_value(preset_val.clone()).map(Some);
    }
    serde_json::from_value(value.clone()).map(Some)
}

fn role_selection(
    current: &BTreeMap<String, String>,
    thread_id: &str,
    role: &str,
) -> Result<BTreeMap<String, String>, ()> {
    if !["ignore", "body", "speaker", "choice"].contains(&role) {
        return Err(());
    }
    let mut next = current.clone();
    if role == "ignore" {
        next.remove(thread_id);
        return Ok(next);
    }
    if role == "body" || role == "speaker" {
        next.retain(|_, existing| existing != role);
    }
    if !next.contains_key(thread_id) && next.len() >= 8 {
        return Err(());
    }
    next.insert(thread_id.into(), role.into());
    Ok(next)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessPage {
    processes: Vec<ProcessRecord>,
    truncated: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadPage {
    threads: Vec<CaptureThread>,
    total: usize,
    next_offset: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryPage {
    entries: Vec<HistoryEntry>,
    total: usize,
    next_offset: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_popup_payload_covers_swift_contract_in_every_locale() {
        let contract = include_str!("../../../WhiskyBridge/Sources/WhiskyNativeBridge/CapturePopupStrings.swift");
        let ids: Vec<_> = contract.lines().filter_map(|line| {
            line.trim().strip_prefix('"')?.split_once("\": ").map(|(id, _)| id)
        }).collect();
        assert!(ids.len() > 40);
        for locale in crate::i18n::Locale::ALL {
            let _locale = crate::i18n::LocaleGuard::new(locale);
            let payload = native_popup_strings();
            for id in &ids {
                assert!(!payload[*id].as_str().unwrap_or_default().is_empty(), "{locale:?}: {id}");
            }
            assert_eq!(payload["attached_target"], crate::i18n::s().linux_hud.attached_target("{exe}"));
            assert!(payload["error:capture_accept_failed"].is_string());
        }
    }

    #[gpui_kit::test]
    fn native_popup_localization_is_sent_after_handshake_and_language_change(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        let _locale = crate::i18n::LocaleGuard::new(crate::i18n::Locale::En);
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });
        window.update(cx, |app, _, cx| {
            drain();
            app.set_language(crate::i18n::LanguageMode::En, cx);
            assert!(drain().is_empty());
            app.bridge_status = crate::bridge::BridgeStatus::Connected {
                helper: "test".into(), version: "test".into(), methods: 1,
            };
            app.apply_bridge_event(crate::bridge::BridgeEvent::Services(vec![
                "translation.capture.hud/localize".into(),
            ]), cx);
            let calls = drain();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "translation.capture.hud/localize");
            assert_eq!(calls[0].1["strings"], native_popup_strings());
            app.set_language(crate::i18n::LanguageMode::ZhHans, cx);
            app.set_language(crate::i18n::LanguageMode::Ja, cx);
            let calls = drain();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[1].0, "translation.capture.hud/localize");
            assert_eq!(calls[1].1["strings"], native_popup_strings());
            app.apply_bridge_event(crate::bridge::BridgeEvent::Services(vec![]), cx);
            app.set_language(crate::i18n::LanguageMode::En, cx);
            assert!(drain().is_empty());
        }).unwrap();
    }

    #[test]
    fn process_identity_uses_swift_wire_casing() {
        let process = ProcessRecord {
            guest_pid: 9,
            exe_path: "C:\\a.exe".into(),
            arch: "x64".into(),
            creation_time: "0123456789ABCDEF".into(),
        };
        let value = serde_json::to_value(process).unwrap();
        assert_eq!(value["guestPID"], 9);
        assert_eq!(value["exePath"], "C:\\a.exe");
        assert_eq!(value["arch"], "x64");
        assert_eq!(value["creationTime"], "0123456789ABCDEF");
    }
    #[test]
    fn snapshot_and_thread_dtos_use_uppercase_id_wire_keys() {
        let snapshot = Snapshot {
            session_id: Some("session".into()),
            game_id: Some("game".into()),
            ..Snapshot::default()
        };
        let value = serde_json::to_value(snapshot).unwrap();
        assert_eq!(value["sessionID"], "session");
        assert_eq!(value["gameID"], "game");
        let thread = CaptureThread {
            guest_pid: 9,
            ..CaptureThread::default()
        };
        assert_eq!(serde_json::to_value(thread).unwrap()["guestPID"], 9);
    }
    #[test]
    fn overlay_settings_reject_values_outside_swift_bounds() {
        let settings = OverlaySettings {
            visible: true,
            click_through: true,
            show_source: true,
            show_translation: true,
            font_size: 73.0,
            opacity: 0.8,
            width: 900.0,
            height: 180.0,
            x: None,
            y: None,
            ..OverlaySettings::default()
        };
        assert!(!settings.validate());
    }
    #[test]
    fn overlay_settings_accept_swift_bounds_and_reject_hidden_text() {
        let settings = OverlaySettings {
            visible: true,
            click_through: true,
            show_source: true,
            show_translation: false,
            font_size: 12.0,
            opacity: 0.0,
            width: 320.0,
            height: 100.0,
            x: None,
            y: None,
            ..OverlaySettings::default()
        };
        assert!(settings.validate());
        let invalid = OverlaySettings {
            show_source: false,
            show_translation: false,
            ..settings
        };
        assert!(!invalid.validate());
    }
    #[test]
    fn polling_is_single_flight_and_stale_generations_are_rejected() {
        let mut state = CaptureState::default();
        assert!(!state.can_poll());
        state.overlay = Some(OverlaySettings { visible: true, ..OverlaySettings::default() });
        assert!(state.can_poll());
        state.overlay = None;
        state.active_game = Some(("c".into(), "e".into(), "g".into()));
        state.panel_open = false;
        assert!(state.can_poll());
        state.poll_request = Some(8);
        assert!(!state.can_poll());
        let pending = Pending {
            method: "translation.snapshot".into(),
            generation: state.generation,
            session_id: None,
            snapshot_owner: None,
            context: None,
            preset_key: None,
            role_edit_generation: state.role_edit_generation,
        };
        state.generation += 1;
        assert!(!state.reply_is_current(&pending, "translation.snapshot"));
    }
    #[test]
    fn stale_session_replies_are_rejected() {
        let mut state = CaptureState::default();
        state.snapshot = Some(Snapshot {
            session_id: Some("new-session".into()),
            ..Snapshot::default()
        });
        let pending = Pending {
            method: "translation.snapshot".into(),
            generation: state.generation,
            session_id: Some("old-session".into()),
            snapshot_owner: None,
            context: None,
            preset_key: None,
            role_edit_generation: state.role_edit_generation,
        };
        assert!(!state.reply_is_current(&pending, "translation.snapshot"));
    }
    #[test]
    fn history_visibility_is_bound_to_the_viewed_game() {
        let mut state = CaptureState::default();
        state.history_owner = Some(("container-a".into(), "entry-a".into(), "Game A".into()));
        state.context = Some(("container-b".into(), "entry-b".into(), "Game B".into()));
        assert!(!state.history_belongs_to_context());
        state.context = state.history_owner.clone();
        assert!(state.history_belongs_to_context());
    }
    #[test]
    fn selection_roles_are_empty_until_explicitly_selected_and_limit_is_eight() {
        let mut roles = BTreeMap::new();
        for index in 0..8 {
            roles.insert(format!("t{index}"), "choice".into());
        }
        assert_eq!(roles.len(), 8);
        assert!(role_selection(&roles, "t9", "choice").is_err());
        let body = role_selection(&roles, "t0", "body").unwrap();
        assert_eq!(body.values().filter(|r| *r == "body").count(), 1);
        let body = role_selection(&body, "t1", "body").unwrap();
        assert_eq!(body.values().filter(|r| *r == "body").count(), 1);
        assert!(!body.contains_key("t0"));
        let speaker = role_selection(&body, "t2", "speaker").unwrap();
        assert_eq!(speaker.values().filter(|r| *r == "speaker").count(), 1);
        let ignored = role_selection(&speaker, "t2", "ignore").unwrap();
        assert!(!ignored.contains_key("t2"));
        let state = CaptureState::default();
        assert!(state.thread_roles.is_empty());
    }
    #[test]
    fn snapshot_source_is_preserved_when_request_fails() {
        let mut state = CaptureState::default();
        state.snapshot = Some(Snapshot {
            subtitle: Subtitle {
                source: "captured".into(),
                status: "failed".into(),
                ..Subtitle::default()
            },
            ..Snapshot::default()
        });
        state.apply_error("translation.snapshot", "offline".into());
        assert_eq!(state.snapshot.as_ref().unwrap().subtitle.source, "captured");
        assert_eq!(state.error.as_deref(), Some("offline"));
    }
    #[test]
    fn history_inclusion_field_matches_swift_casing() {
        let entry = HistoryEntry {
            id: "entry".into(),
            source: HistorySource {
                text: "source".into(),
                ..HistorySource::default()
            },
            include_in_context: false,
            ..HistoryEntry::default()
        };
        let value = serde_json::to_value(entry).unwrap();
        assert_eq!(value["includeInContext"], false);
        assert_eq!(value["source"]["text"], "source");
        assert!(value["source"].get("eventID").is_some());
        assert!(value["source"].get("streamID").is_some());
    }

    #[test]
    fn capture_preset_and_snapshot_decode_camel_case_contract() {
        let payload = serde_json::json!({
            "sessionID": "sess-1",
            "gameID": "game-1",
            "captureState": "attached",
            "lastError": null,
            "target": {
                "guestPID": 42,
                "exePath": "C:\\Games\\SiglusEngine.exe",
                "arch": "x86",
                "creationTime": "12345"
            },
            "subtitle": {
                "source": "",
                "translation": "",
                "speaker": null,
                "kind": null,
                "status": "idle",
                "error": null
            },
            "cursor": 1,
            "processes": [],
            "historyCount": 0,
            "threads": [],
            "threadsTotal": 0,
            "autoBoundCount": 2,
            "preset": {
                "providerID": "prov-1",
                "architecture": "x86",
                "restoreHistory": true,
                "autoAttach": true,
                "targetExePath": "C:\\Games\\SiglusEngine.exe",
                "customHookCodes": ["HS12@-4:gdi32.dll"],
                "threadBindings": [
                    {
                        "role": "body",
                        "hookCode": "HS12@-4:gdi32.dll",
                        "hookName": "TextOutA",
                        "contextHex": "0x10",
                        "context2Hex": "0x20"
                    },
                    {
                        "role": "speaker",
                        "hookCode": "HS13@-4:gdi32.dll",
                        "hookName": "Speaker",
                        "contextHex": "0x30",
                        "context2Hex": "0x40"
                    }
                ]
            }
        });
        let snapshot: Snapshot = serde_json::from_value(payload).unwrap();
        assert_eq!(snapshot.auto_bound_count, 2);
        let preset = snapshot.preset.expect("preset should decode");
        assert_eq!(preset.provider_id.as_deref(), Some("prov-1"));
        assert_eq!(preset.architecture, "x86");
        assert!(preset.restore_history);
        assert!(preset.auto_attach);
        assert_eq!(
            preset.target_exe_path.as_deref(),
            Some("C:\\Games\\SiglusEngine.exe")
        );
        assert_eq!(preset.custom_hook_codes, vec!["HS12@-4:gdi32.dll"]);
        assert_eq!(preset.binding_counts(), (1, 1, 0));
        assert!(preset.has_saved_state());
        assert!(preset.has_auto_attach_target());
        assert_eq!(
            exe_basename("C:\\Games\\Key\\SiglusEngine.exe"),
            "SiglusEngine.exe"
        );
        assert_eq!(exe_basename("/tmp/game/app.exe"), "app.exe");
        assert_eq!(exe_basename("game.exe"), "game.exe");
    }

    #[test]
    fn thread_filtering_hides_empty_previews_unless_role_assigned_and_matches_query() {
        let state = CaptureState::default();
        assert!(state.thread_filter_non_empty);

        let threads = vec![
            CaptureThread {
                id: "t1".into(),
                hook_name: "SystemNoise".into(),
                hook_code: "HS01".into(),
                preview: "   ".into(),
                role: None,
                ..CaptureThread::default()
            },
            CaptureThread {
                id: "t2".into(),
                hook_name: "BoundEmptyBody".into(),
                hook_code: "HS02".into(),
                preview: "".into(),
                role: Some("body".into()),
                ..CaptureThread::default()
            },
            CaptureThread {
                id: "t3".into(),
                hook_name: "LocalAssignedChoice".into(),
                hook_code: "HS03".into(),
                preview: "".into(),
                role: None,
                ..CaptureThread::default()
            },
            CaptureThread {
                id: "t4".into(),
                hook_name: "TextOutW".into(),
                hook_code: "HS04".into(),
                preview: "おはようございます".into(),
                role: None,
                ..CaptureThread::default()
            },
        ];
        let mut roles = BTreeMap::new();
        roles.insert("t3".into(), "choice".into());

        // Non-empty filter on, empty query: t1 hidden, t2/t3 shown (assigned role), t4 shown (text)
        let filtered = filter_threads(&threads, &roles, true, "");
        assert_eq!(filtered.hidden_empty_count, 1);
        let ids: Vec<&str> = filtered.threads.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["t2", "t3", "t4"]);

        // Non-empty filter off, empty query: all 4 shown
        let all = filter_threads(&threads, &roles, false, "");
        assert_eq!(all.hidden_empty_count, 0);
        assert_eq!(all.threads.len(), 4);

        // Search query matches hook_name / hook_code / preview case-insensitively
        let by_name = filter_threads(&threads, &roles, true, "textout");
        assert_eq!(by_name.threads.len(), 1);
        assert_eq!(by_name.threads[0].id, "t4");

        let by_code = filter_threads(&threads, &roles, true, "hs03");
        assert_eq!(by_code.threads.len(), 1);
        assert_eq!(by_code.threads[0].id, "t3");
    }

    #[gpui_kit::test]
    fn preset_replies_and_auto_attach_state_transitions_work(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, _, cx| {
                use crate::translation::{ProviderCapabilities, ProviderDto, default_thinking_level};

                let ctx_tuple = ("c1".to_owned(), "g1".to_owned(), "Game One".to_owned());
                this.capture.set_context(Some(ctx_tuple.clone()));
                this.translation_page.providers_loaded = true;
                this.translation_page.providers = vec![ProviderDto {
                    id: "prov-saved".into(),
                    name: "Saved".into(),
                    kind: "openAIChat".into(),
                    base_url: "https://api.openai.com/v1".into(),
                    model_id: Some("gpt-5".into()),
                    default_source_language: "ja".into(),
                    default_target_language: "zh-Hans".into(),
                    model_metadata: None,
                    open_ai_chat_token_limit_field: "max_completion_tokens".into(),
                    thinking_level: default_thinking_level(),
                    capabilities: ProviderCapabilities {
                        model_discovery: true,
                        context_discovery: false,
                        streaming: true,
                        custom_prompt: true,
                        context: true,
                    },
                    has_credential: true,
                }];

                // 1. preset/get decodes preset and applies provider_id, arch, restore_history when idle
                this.capture.pending.insert(
                    201,
                    Pending {
                        method: "translation.capture.preset/get".into(),
                        generation: this.capture.generation,
                        session_id: None,
                        snapshot_owner: None,
                        context: Some(ctx_tuple.clone()),
                        preset_key: None,
                        role_edit_generation: this.capture.role_edit_generation,
                    },
                );
                let disconnected = BridgeSupervisor::disconnected_for_tests();
                this.capture.reply(
                    201,
                    "translation.capture.preset/get",
                    Ok(serde_json::json!({
                        "preset": {
                            "providerID": "prov-saved",
                            "architecture": "x86",
                            "restoreHistory": true,
                            "autoAttach": false,
                            "targetExePath": "C:\\Game\\Main.exe",
                            "customHookCodes": [],
                            "threadBindings": []
                        }
                    })),
                    &disconnected,
                    cx,
                );
                this.capture.apply_pending_preset(&this.translation_page.providers);
                this.capture
                    .maybe_auto_attach_on_preset_load(&disconnected, false);
                assert_eq!(this.capture.provider_id.as_deref(), Some("prov-saved"));
                assert_eq!(this.capture.architecture, "x86");
                assert!(this.capture.restore_history);

                // 1b. preset/save updates preset; when autoAttach=true on preset/get load, triggers start(..., autoAttach: true)
                this.capture.pending.insert(
                    205,
                    Pending {
                        method: "translation.capture.preset/save".into(),
                        generation: this.capture.generation,
                        session_id: None,
                        snapshot_owner: None,
                        context: Some(ctx_tuple.clone()),
                        preset_key: None,
                        role_edit_generation: this.capture.role_edit_generation,
                    },
                );
                this.capture.reply(
                    205,
                    "translation.capture.preset/save",
                    Ok(serde_json::json!({
                        "preset": {
                            "providerID": "prov-saved",
                            "architecture": "x86",
                            "restoreHistory": true,
                            "autoAttach": true,
                            "targetExePath": "C:\\Game\\Main.exe",
                            "customHookCodes": ["HS1"],
                            "threadBindings": []
                        }
                    })),
                    &disconnected,
                    cx,
                );
                assert!(this.capture.preset.as_ref().is_some_and(|p| p.auto_attach));
                this.capture.auto_attach_on_preset_load = true;
                this.capture.maybe_auto_attach_on_preset_load(&disconnected, false);
                // Disconnected test supervisor proves `translation.capture.start` was attempted.
                assert!(
                    this.capture
                        .error
                        .as_deref()
                        .is_some_and(|e| e.contains("disconnected"))
                );
                this.capture.error = None;

                // 2. autoAttach reply with selecting increments bounded retry attempts, then attached clears pending
                this.capture.active_game = Some(ctx_tuple.clone());
                this.capture.auto_attach_pending = true;
                this.capture.pending.insert(
                    202,
                    Pending {
                        method: "translation.capture.autoAttach".into(),
                        generation: this.capture.generation,
                        session_id: None,
                        snapshot_owner: None,
                        context: Some(ctx_tuple.clone()),
                        preset_key: None,
                        role_edit_generation: this.capture.role_edit_generation,
                    },
                );
                this.capture.reply(
                    202,
                    "translation.capture.autoAttach",
                    Ok(serde_json::json!({
                        "snapshot": {
                            "sessionID": "s1",
                            "gameID": "g1",
                            "captureState": "selecting",
                            "lastError": null,
                            "target": null,
                            "subtitle": { "source": "", "translation": "", "speaker": null, "kind": null, "status": "idle", "error": null },
                            "cursor": 0,
                            "processes": [],
                            "historyCount": 0,
                            "threads": [],
                            "threadsTotal": 0
                        }
                    })),
                    &disconnected,
                    cx,
                );
                assert!(this.capture.auto_attach_pending);
                assert_eq!(this.capture.auto_attach_attempts, 1);

                this.capture.pending.insert(
                    203,
                    Pending {
                        method: "translation.capture.autoAttach".into(),
                        generation: this.capture.generation,
                        session_id: Some("s1".into()),
                        snapshot_owner: None,
                        context: Some(ctx_tuple.clone()),
                        preset_key: None,
                        role_edit_generation: this.capture.role_edit_generation,
                    },
                );
                this.capture.reply(
                    203,
                    "translation.capture.autoAttach",
                    Ok(serde_json::json!({
                        "snapshot": {
                            "sessionID": "s1",
                            "gameID": "g1",
                            "captureState": "attached",
                            "lastError": null,
                            "target": {
                                "guestPID": 100,
                                "exePath": "C:\\Game\\Main.exe",
                                "arch": "x86",
                                "creationTime": "01"
                            },
                            "subtitle": { "source": "", "translation": "", "speaker": null, "kind": null, "status": "idle", "error": null },
                            "cursor": 0,
                            "processes": [],
                            "historyCount": 0,
                            "threads": [
                                {
                                    "id": "t-auto",
                                    "guestPID": 100,
                                    "addressHex": "0x1",
                                    "contextHex": "0x2",
                                    "context2Hex": "0x3",
                                    "hookCode": "HS1",
                                    "hookName": "MainText",
                                    "preview": "",
                                    "role": "body"
                                }
                            ],
                            "threadsTotal": 1,
                            "autoBoundCount": 1
                        }
                    })),
                    &disconnected,
                    cx,
                );
                assert!(!this.capture.auto_attach_pending);
                assert_eq!(this.capture.auto_attach_attempts, 0);
                assert_eq!(
                    this.capture.thread_roles.get("t-auto").map(String::as_str),
                    Some("body")
                );

                // 3. preset/clear clears preset and sets preset_notice
                this.capture.pending.insert(
                    204,
                    Pending {
                        method: "translation.capture.preset/clear".into(),
                        generation: this.capture.generation,
                        session_id: Some("s1".into()),
                        snapshot_owner: None,
                        context: Some(ctx_tuple),
                        preset_key: None,
                        role_edit_generation: this.capture.role_edit_generation,
                    },
                );
                this.capture.reply(
                    204,
                    "translation.capture.preset/clear",
                    Ok(serde_json::json!({"cleared": true})),
                    &disconnected,
                    cx,
                );
                assert!(this.capture.preset.is_none());
                assert!(this.capture.preset_notice.is_some());
            })
            .unwrap();
    }

    #[gpui_kit::test]
    fn stop_reply_updates_only_its_session_owners_final_preset(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_tx, rx) = smol::channel::unbounded();
        let (bridge, _drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, rx, false)
        });
        window.update(cx, |app, _, cx| {
            let a: (String, String, String) = ("container".into(), "game-a".into(), "Game A".into());
            let b: (String, String, String) = ("container".into(), "game-b".into(), "Game B".into());
            let old_a = CapturePreset::default();
            let final_a = CapturePreset {
                translation_enabled: true,
                target_exe_path: Some("C:\\e2e-gdi.exe".into()),
                ..Default::default()
            };
            let preset_b = CapturePreset {
                architecture: "x86".into(),
                ..Default::default()
            };
            for context in [&a, &b] {
                for switch_context in [false, true] {
                    for outcome in ["preset", "empty", "error"] {
                        app.capture = CaptureState::default();
                        app.capture.presets_by_game.insert(
                            (a.0.clone(), a.1.clone()), old_a.clone(),
                        );
                        app.capture.presets_by_game.insert(
                            (b.0.clone(), b.1.clone()), preset_b.clone(),
                        );
                        app.capture.set_context(Some(context.clone()));
                        app.capture.active_game = Some(a.clone());
                        app.capture.snapshot = Some(Snapshot {
                            session_id: Some("session-a".into()),
                            capture_state: "attached".into(),
                            ..Default::default()
                        });
                        app.capture.stop(&app.bridge, "translation.capture.stop");
                        assert!(app.capture.active_game.is_none());
                        let stop = *app.capture.pending.keys().next().unwrap();
                        assert_eq!(app.capture.pending[&stop].snapshot_owner.as_ref(), Some(&a));
                        if switch_context {
                            let next = if context == &a { &b } else { &a };
                            app.capture.set_context(Some(next.clone()));
                        }
                        let response = if outcome == "error" {
                            Err("stop failed".into())
                        } else {
                            Ok(serde_json::to_value(Snapshot {
                                session_id: Some("session-a".into()),
                                game_id: Some("opaque-game-a-storage-hash".into()),
                                capture_state: "stopped".into(),
                                preset: (outcome == "preset").then(|| final_a.clone()),
                                ..Default::default()
                            }).unwrap())
                        };
                        app.capture.reply(stop, "translation.capture.stop", response, &app.bridge, cx);
                        let expected_a = if outcome == "preset" { &final_a } else { &old_a };
                        assert_eq!(app.capture.preset_for_game("container", "game-a"), Some(expected_a));
                        assert_eq!(app.capture.preset_for_game("container", "game-b"), Some(&preset_b));
                        let expected_selected = if app.capture.context.as_ref() == Some(&a) {
                            expected_a
                        } else {
                            &preset_b
                        };
                        assert_eq!(app.capture.preset.as_ref(), Some(expected_selected));
                        assert!(!app.capture.pending.contains_key(&stop));
                    }
                }
            }
        }).unwrap();
    }

    #[gpui_kit::test]
    fn stopping_from_another_games_hud_cannot_attribute_or_save_the_session_preset(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_tx, rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, rx, false)
        });
        window.update(cx, |app, _, cx| {
            let a = ("container".into(), "game-a".into(), "Game A".into());
            let preset_a = CapturePreset {
                provider_id: Some("provider-a".into()),
                architecture: "x64".into(),
                restore_history: true,
                auto_attach: true,
                translation_enabled: true,
                target_exe_path: Some("C:\\e2e-gdi.exe".into()),
                ..Default::default()
            };
            let preset_b = CapturePreset {
                architecture: "x86".into(), ..Default::default()
            };
            for expected_b in [None, Some(preset_b)] {
                app.capture = CaptureState::default();
                app.capture.set_context(Some(a.clone()));
                app.capture.provider_id = Some("provider-a".into());
                app.capture.start(&app.bridge);
                let start = *app.capture.pending.keys().next().unwrap();
                let mut snapshot = Snapshot {
                    session_id: Some("session-a".into()),
                    game_id: Some("opaque-game-a-storage-hash".into()),
                    capture_state: "attached".into(),
                    preset: Some(preset_a.clone()),
                    ..Default::default()
                };
                app.capture.reply(start, "translation.capture.start",
                    Ok(serde_json::to_value(&snapshot).unwrap()), &app.bridge, cx);
                assert_eq!(app.capture.preset_for_game("container", "game-a"), Some(&preset_a));
                app.capture.set_context(Some(("container".into(), "game-b".into(), "Game B".into())));
                app.capture.load_preset_for_game(&app.bridge, "container", "game-b");
                let get = *app.capture.pending.iter()
                    .find(|(_, p)| p.method == "translation.capture.preset/get").unwrap().0;
                app.capture.reply(get, "translation.capture.preset/get",
                    Ok(json!({"preset": expected_b})), &app.bridge, cx);
                assert_eq!(app.capture.preset.as_ref(), expected_b.as_ref());

                app.capture.stop(&app.bridge, "translation.capture.stop");
                assert!(app.capture.active_game.is_none());
                let stop = *app.capture.pending.keys().next().unwrap();
                snapshot.capture_state = "stopped".into();
                app.capture.reply(stop, "translation.capture.stop",
                    Ok(serde_json::to_value(&snapshot).unwrap()), &app.bridge, cx);
                assert_eq!(app.capture.preset.as_ref(), expected_b.as_ref());
                for state in ["stopped", "targetExited", "error"] {
                    snapshot.capture_state = state.into();
                    for game_id in [Some("opaque-game-a-storage-hash".into()), None] {
                        snapshot.game_id = game_id;
                        let poll = app.capture.request(&app.bridge, "translation.snapshot", json!({})).unwrap();
                        app.capture.reply(poll, "translation.snapshot",
                            Ok(serde_json::to_value(&snapshot).unwrap()), &app.bridge, cx);
                        assert_eq!(app.capture.preset_for_game("container", "game-b"), expected_b.as_ref());
                        assert_eq!(app.capture.preset.as_ref(), expected_b.as_ref());
                        assert_eq!(app.capture.preset_for_game("container", "game-a"), Some(&preset_a));
                    }
                }
                assert!(!drain().iter().any(|(m, _)| m == "translation.capture.preset/save"));

                app.capture.architecture = "x86".into();
                app.capture.toggle_preset_auto_attach(&app.bridge);
                let calls = drain();
                let (_, saved) = calls.iter().find(|(m, _)| m == "translation.capture.preset/save").unwrap();
                assert_eq!(saved["entryID"], "game-b");
                assert_eq!(saved["restoreHistory"], false);
                assert_eq!(saved["architecture"], "x86");
                assert!(saved.get("translationEnabled").is_none());
                assert!(saved.get("targetExePath").is_none());
                assert!(saved.get("threadBindings").is_none());
            }
        }).unwrap();
    }

    #[test]
    fn capture_preset_decodes_translation_enabled_and_prompt_defaults() {
        let omitted: CapturePreset = serde_json::from_value(serde_json::json!({
            "architecture": "x64"
        }))
        .unwrap();
        assert!(!omitted.translation_enabled);
        assert!(omitted.prompt_attach_on_launch);
        assert!(CapturePreset::default().prompt_attach_on_launch);
        assert!(!CapturePreset::default().translation_enabled);

        let explicit: CapturePreset = serde_json::from_value(serde_json::json!({
            "translationEnabled": true,
            "promptAttachOnLaunch": false
        }))
        .unwrap();
        assert!(explicit.translation_enabled);
        assert!(!explicit.prompt_attach_on_launch);
    }

    #[gpui_kit::test]
    fn preset_save_toggles_translation_and_prompt_and_caches_by_game(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, _, cx| {
                this.capture.set_context(Some((
                    "c-active".into(),
                    "g-active".into(),
                    "Active Game".into(),
                )));
                this.capture.provider_id = Some("prov-1".into());
                this.capture.architecture = "x86".into();

                // Toggle translationEnabled=true for another game (e.g. from Run Settings).
                this.capture.toggle_preset_translation_enabled(
                    &this.bridge,
                    "c-other",
                    "g-other",
                    true,
                );
                let calls = drain();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].0, "translation.capture.preset/save");
                assert_eq!(calls[0].1["container"], "c-other");
                assert_eq!(calls[0].1["entryID"], "g-other");
                assert_eq!(calls[0].1["translationEnabled"], true);
                assert_eq!(calls[0].1["providerID"], "prov-1");
                assert_eq!(calls[0].1["architecture"], "x86");

                let req_id = *this.capture.pending.keys().next().unwrap();
                this.capture.reply(
                    req_id,
                    "translation.capture.preset/save",
                    Ok(serde_json::json!({
                        "preset": {
                            "providerID": "prov-1",
                            "architecture": "x86",
                            "translationEnabled": true,
                            "promptAttachOnLaunch": true
                        }
                    })),
                    &this.bridge,
                    cx,
                );

                // Cached under (c-other, g-other) without clobbering active context's self.preset.
                assert!(this.capture.preset.is_none());
                let cached = this
                    .capture
                    .preset_for_game("c-other", "g-other")
                    .expect("preset should be cached by game");
                assert!(cached.translation_enabled);
                assert!(cached.prompt_attach_on_launch);

                // Toggle promptAttachOnLaunch=false.
                this.capture.toggle_preset_prompt_on_launch(
                    &this.bridge,
                    "c-other",
                    "g-other",
                    false,
                );
                let calls = drain();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].0, "translation.capture.preset/save");
                assert_eq!(calls[0].1["container"], "c-other");
                assert_eq!(calls[0].1["entryID"], "g-other");
                assert_eq!(calls[0].1["promptAttachOnLaunch"], false);

                let req_id = *this.capture.pending.keys().next().unwrap();
                this.capture.reply(
                    req_id,
                    "translation.capture.preset/save",
                    Ok(serde_json::json!({
                        "preset": {
                            "providerID": "prov-1",
                            "architecture": "x86",
                            "translationEnabled": true,
                            "promptAttachOnLaunch": false
                        }
                    })),
                    &this.bridge,
                    cx,
                );
                let updated = this
                    .capture
                    .preset_for_game("c-other", "g-other")
                    .expect("cached preset should update");
                assert!(updated.translation_enabled);
                assert!(!updated.prompt_attach_on_launch);

                // Switching context to (c-other, g-other) immediately populates self.preset from cache.
                this.capture.set_context(Some((
                    "c-other".into(),
                    "g-other".into(),
                    "Other Game".into(),
                )));
                assert!(
                    this.capture
                        .preset
                        .as_ref()
                        .is_some_and(|p| p.translation_enabled && !p.prompt_attach_on_launch)
                );

                this.capture
                    .open_capture_hud(&this.bridge, "c-other", "g-other");
                let calls = drain();
                #[cfg(target_os = "linux")]
                {
                    assert!(calls.is_empty());
                    assert_eq!(this.capture.hud.open_requested, Some(("c-other".into(), "g-other".into())));
                }
                #[cfg(not(target_os = "linux"))]
                {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].0, "translation.capture.hud/show");
                assert_eq!(calls[0].1["container"], "c-other");
                assert_eq!(calls[0].1["entryID"], "g-other");
                }

                // open_translation_for_game auto-reads overlay settings when service is available.
                this.bridge_status = crate::bridge::BridgeStatus::Connected {
                    helper: "test".into(),
                    version: "test".into(),
                    methods: 3,
                };
                this.service_methods
                    .insert("translation.overlay.read".into());
                this.service_methods
                    .insert("translation.capture.preset/get".into());
                this.service_methods
                    .insert("translation.providers/list".into());
                this.entries.push(crate::model::Game {
                    metadata: None,
                    character_images: BTreeMap::new(),
                    id: "g-entry".into(),
                    art: None,
                    title: "Entry Game".into(),
                    studio: None,
                    description: None,
                    characters: Vec::new(),
                    metadata_sources: BTreeMap::new(),
                    release: None,
                    landscape: false,
                    bottle: 1,
                    hidden: false,
                    rating: None,
                    play_time: None,
                    daily_play_time: BTreeMap::new(),
                    last_played: None,
                    executable: Some(std::path::PathBuf::from("game.exe")),
                    folder_path: None,
                    game_directory: None,
                    is_folder: false,
                    needs_executable: false,
                    container: "c-entry".into(),
                    last_session_ended_at: None,
                    is_launcher: false,
                    identity: "g-entry".into(),
                    tags: Vec::new(),
                    multi_executable: false,
                    launch_executables: Vec::new(),
                    last_executable: None,
                });
                this.open_translation_for_game(0, cx);
                let calls = drain();
                assert!(calls.iter().any(|(m, _)| m == "translation.overlay.read"));
                assert!(
                    calls
                        .iter()
                        .any(|(m, p)| m == "translation.capture.preset/get"
                            && p["container"] == "c-entry"
                            && p["entryID"] == "g-entry")
                );

                // open_preview_sheet(RunSettings) fetches preset and providers for that game.
                this.capture.pending.clear();
                this.open_preview_sheet(crate::state::PreviewSheet::RunSettings(0), cx);
                let calls = drain();
                assert!(
                    calls
                        .iter()
                        .any(|(m, p)| m == "translation.capture.preset/get"
                            && p["container"] == "c-entry"
                            && p["entryID"] == "g-entry")
                );
                assert!(calls.iter().any(|(m, _)| m == "translation.providers/list"));
            })
            .unwrap();
    }

    #[test]
    fn overlay_settings_allow_zero_opacity_and_validate_hex_colors() {
        let mut settings = OverlaySettings::default();
        assert_eq!(settings.text_color, "#FFFFFF");
        assert_eq!(settings.source_text_color, "#D4D4D8");
        assert!(settings.validate());

        // Missing hex fields deserialize to defaults; explicit fields round-trip.
        let legacy: OverlaySettings = serde_json::from_value(serde_json::json!({
            "visible": true,
            "clickThrough": false,
            "showSource": true,
            "showTranslation": true,
            "fontSize": 28.0,
            "opacity": 0.0,
            "width": 900.0,
            "height": 180.0,
            "x": null,
            "y": null
        }))
        .unwrap();
        assert_eq!(legacy.opacity, 0.0);
        assert_eq!(legacy.max_entries, 1);
        assert_eq!(legacy.text_color, "#FFFFFF");
        assert_eq!(legacy.source_text_color, "#D4D4D8");
        assert!(legacy.validate());

        settings.max_entries = 6;
        settings.text_color = "#FDE047".into();
        settings.source_text_color = "#BAE6FD".into();
        let encoded = serde_json::to_value(&settings).unwrap();
        assert_eq!(encoded["maxEntries"], 6);
        assert_eq!(encoded["textColor"], "#FDE047");
        assert_eq!(encoded["sourceTextColor"], "#BAE6FD");
        let decoded: OverlaySettings = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, settings);

        // maxEntries bounds & clamping actions
        settings.max_entries = 0;
        assert!(!settings.validate());
        settings.max_entries = 11;
        assert!(!settings.validate());
        settings.max_entries = 1;
        assert!(settings.validate());
        apply_overlay_action(&mut settings, "maxEntries-");
        assert_eq!(settings.max_entries, 1);
        for _ in 0..12 {
            apply_overlay_action(&mut settings, "maxEntries+");
        }
        assert_eq!(settings.max_entries, 10);
        apply_overlay_action(&mut settings, "maxEntries-");
        assert_eq!(settings.max_entries, 9);

        // Opacity bounds
        settings.opacity = -0.01;
        assert!(!settings.validate());
        settings.opacity = 1.01;
        assert!(!settings.validate());
        settings.opacity = 0.0;
        assert!(settings.validate());

        // Strict 7-char #RRGGBB hex validation
        assert!(is_valid_hex_color("#ffffff"));
        assert!(is_valid_hex_color("#7DD3FC"));
        for bad in ["FFFFFF", "#FFF", "#GGGGGG", "#1234567", ""] {
            assert!(!is_valid_hex_color(bad), "expected {bad:?} to be rejected");
            let mut invalid = settings.clone();
            invalid.text_color = bad.into();
            assert!(!invalid.validate());

            let mut invalid_source = settings.clone();
            invalid_source.source_text_color = bad.into();
            assert!(!invalid_source.validate());
        }
    }

    #[gpui_kit::test]
    fn adjust_overlay_clamps_opacity_to_zero_and_updates_hex_colors(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(|app| gpui_kit::init(app));
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, false)
        });

        window
            .update(cx, |this, _, _| {
                this.capture.overlay = Some(OverlaySettings {
                    opacity: 0.02,
                    ..OverlaySettings::default()
                });

                // opacity- clamps to 0.0 instead of going negative or failing validation.
                this.capture.adjust_overlay(&this.bridge, "opacity-");
                let calls = drain();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].0, "translation.overlay.configure");
                assert_eq!(calls[0].1["settings"]["opacity"], 0.0);
                assert!(this.capture.error.is_none());

                // Clear pending before dispatching next configure calls.
                this.capture.pending.clear();
                this.capture
                    .adjust_overlay(&this.bridge, "textColor:#FDE047");
                let calls = drain();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].1["settings"]["textColor"], "#FDE047");

                this.capture.pending.clear();
                this.capture
                    .adjust_overlay(&this.bridge, "sourceColor:#BAE6FD");
                let calls = drain();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].1["settings"]["sourceTextColor"], "#BAE6FD");

                // Invalid hex color is rejected locally without sending bridge request.
                this.capture.pending.clear();
                this.capture.adjust_overlay(&this.bridge, "textColor:#XYZ");
                let calls = drain();
                assert!(calls.is_empty());
                assert!(this.capture.error.is_some());
            })
            .unwrap();
    }
}
