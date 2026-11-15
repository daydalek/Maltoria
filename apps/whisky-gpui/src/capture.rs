use std::collections::BTreeMap;
use std::time::Duration;

use gpui_kit::component::input::InputState;
use gpui_kit::{Context, Entity};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{bridge::BridgeSupervisor, state::WhiskyApp, translation::next_request_id};

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
pub(crate) struct Subtitle {
    pub(crate) source: String,
    pub(crate) translation: String,
    pub(crate) speaker: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) status: String,
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) translation_truncated: bool,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlaySettings {
    pub(crate) visible: bool,
    pub(crate) click_through: bool,
    pub(crate) show_source: bool,
    pub(crate) show_translation: bool,
    pub(crate) font_size: f64,
    pub(crate) opacity: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) x: Option<f64>,
    pub(crate) y: Option<f64>,
}

impl OverlaySettings {
    pub(crate) fn validate(&self) -> bool {
        (12.0..=72.0).contains(&self.font_size)
            && (0.1..=1.0).contains(&self.opacity)
            && (320.0..=3840.0).contains(&self.width)
            && (100.0..=1200.0).contains(&self.height)
            && self.x.is_none_or(f64::is_finite)
            && self.y.is_none_or(f64::is_finite)
            && (self.show_source || self.show_translation)
    }
}

#[derive(Clone, Debug)]
struct Pending {
    method: String,
    generation: u64,
    session_id: Option<String>,
}

#[derive(Clone, Default)]
pub(crate) struct CaptureState {
    pub(crate) panel_open: bool,
    pub(crate) context: Option<(String, String, String)>,
    pub(crate) active_game: Option<(String, String, String)>,
    pub(crate) history_owner: Option<(String, String, String)>,
    pub(crate) provider_id: Option<String>,
    pub(crate) architecture: String,
    pub(crate) restore_history: bool,
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
    pending: BTreeMap<u64, Pending>,
    generation: u64,
    poll_request: Option<u64>,
    poll_scheduled: bool,
    last_terminal: String,
}

impl CaptureState {
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

    pub(crate) fn start(&mut self, bridge: &BridgeSupervisor) {
        let Some((container, entry_id, _)) = &self.context else {
            self.error = Some("请从游戏菜单选择游戏后开始捕获。".into());
            return;
        };
        let Some(provider_id) = &self.provider_id else {
            self.error = Some("请先明确选择已保存的提供商。".into());
            return;
        };
        if self.active_game.is_some() || self.is_pending("translation.capture.start") {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.pending.clear();
        self.poll_request = None;
        self.snapshot = None;
        self.error = None;
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
        if self.request(bridge, "translation.capture.start", json!({
            "container":container,"entryID":entry_id,"providerID":provider_id,
            "architecture": if self.architecture.is_empty() { "x64" } else { &self.architecture },
            "restoreHistory":self.restore_history
        })).is_none() { self.active_game = None; }
    }

    pub(crate) fn request_processes(&mut self, bridge: &BridgeSupervisor) {
        let _ = self.request(bridge, "translation.capture.processes", json!({}));
    }
    pub(crate) fn attach(&mut self, bridge: &BridgeSupervisor) {
        if let Some(target) = &self.selected_process {
            let _ = self.request(
                bridge,
                "translation.capture.attach",
                json!({"target":target}),
            );
        }
    }
    pub(crate) fn stop(&mut self, bridge: &BridgeSupervisor, method: &str) {
        let was_active = self.active_game.is_some();
        self.generation = self.generation.wrapping_add(1);
        self.pending.clear();
        self.poll_request = None;
        self.poll_scheduled = false;
        self.active_game = None;
        self.snapshot = None;
        self.processes.clear();
        self.threads.clear();
        self.history_text.clear();
        self.thread_roles.clear();
        self.history_text_progress = None;
        if was_active {
            let _ = self.request(bridge, method, json!({}));
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
        self.configure_overlay(bridge, |s| match action {
            "visible" => s.visible = !s.visible,
            "clickThrough" => s.click_through = !s.click_through,
            "showSource" => s.show_source = !s.show_source,
            "showTranslation" => s.show_translation = !s.show_translation,
            "font-" => s.font_size -= 1.0,
            "font+" => s.font_size += 1.0,
            "opacity-" => s.opacity -= 0.05,
            "opacity+" => s.opacity += 0.05,
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
            _ => {}
        });
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
            self.error = Some("叠层设置超出支持范围，或必须至少显示一种文本。".into());
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
        self.active_game.is_some() && self.poll_request.is_none()
    }
    pub(crate) fn schedule_poll(&mut self, cx: &mut Context<WhiskyApp>) {
        if self.active_game.is_none() || self.poll_scheduled {
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
        self.pending.clear();
        self.poll_request = None;
        self.poll_scheduled = false;
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
                .unwrap_or("桥接已断开；请显式重新开始捕获。 ")
                .to_owned(),
        );
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
        match result {
            Err(error) => self.apply_error(method, error),
            Ok(value) => match method {
                "translation.capture.start"
                | "translation.capture.attach"
                | "translation.capture.detach"
                | "translation.capture.stop"
                | "translation.snapshot" => {
                    if let Ok(snapshot) = serde_json::from_value::<Snapshot>(value) {
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
                        self.thread_total = snapshot.threads_total;
                        let stopped = matches!(
                            snapshot.capture_state.as_str(),
                            "stopped" | "targetExited" | "error"
                        );
                        self.snapshot = Some(snapshot);
                        if stopped {
                            self.active_game = None;
                        } else if method == "translation.capture.start"
                            || method == "translation.capture.attach"
                            || method == "translation.snapshot"
                        {
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
                            self.error = Some("历史文本响应未前进；已停止分块读取。".into());
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
                }
                _ => {}
            },
        }
        if method == "translation.snapshot" && self.active_game.is_some() {
            self.schedule_poll(cx);
        }
        cx.notify();
    }
    fn reply_is_current(&self, pending: &Pending, method: &str) -> bool {
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
    }
    pub(crate) fn select_role(&mut self, bridge: &BridgeSupervisor, thread_id: &str, role: &str) {
        if self.is_pending("translation.threads.select") {
            return;
        }
        let Ok(next) = role_selection(&self.thread_roles, thread_id, role) else {
            self.error = Some("最多选择 8 个文本线程，且正文/说话者各只能选择一个。".into());
            return;
        };
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
            opacity: 0.1,
            width: 320.0,
            height: 100.0,
            x: None,
            y: None,
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
        state.active_game = Some(("c".into(), "e".into(), "g".into()));
        state.panel_open = false;
        assert!(state.can_poll());
        state.poll_request = Some(8);
        assert!(!state.can_poll());
        let pending = Pending {
            method: "translation.snapshot".into(),
            generation: state.generation,
            session_id: None,
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
}
