use std::time::{Duration, Instant};

use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
};
use gpui_kit::{prelude::*, *};
use serde_json::{Value, json};

use crate::capture::{CapturePreset, exe_basename};
use crate::controls::{col, muted, row, selectable_pill};
use crate::linux_overlay::X11Overlay;
use crate::state::WhiskyApp;
use crate::theme;

#[derive(Clone, Default)]
pub(crate) struct HudState {
    pub(crate) visible: bool,
    pub(crate) collapsed: bool,
    pub(crate) open_requested: Option<(String, String)>,
    pub(crate) launch_requested: Option<(String, String)>,
    launch_at: Option<Instant>,
    watched_entry: Option<String>,
    was_active: bool,
    last_capture_state: String,
    hide_overlay_requested: bool,
}

impl HudState {
    pub(crate) fn hide(&mut self) {
        self.visible = false;
        self.open_requested = None;
        self.launch_requested = None;
        self.launch_at = None;
        self.watched_entry = None;
        self.was_active = false;
    }

    pub(crate) fn runtime_snapshot(&mut self, value: &Value) {
        let state = value
            .get("entries")
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|e| e.get("id").and_then(Value::as_str) == self.watched_entry.as_deref())
            })
            .and_then(|e| e.get("state"))
            .and_then(Value::as_str);
        if let Some(state) = state {
            if state != "idle" {
                self.was_active = true;
            }
            if state == "idle" && self.was_active {
                self.hide();
                self.hide_overlay_requested = true;
            }
        }
    }

    fn capture_state(&mut self, state: &str) {
        if state != self.last_capture_state
            && matches!(state, "targetExited" | "stopped")
            && matches!(self.last_capture_state.as_str(), "selecting" | "attached")
        {
            self.hide();
            self.hide_overlay_requested = true;
        }
        self.last_capture_state = state.into();
    }
}

fn launch_provider(preset: &CapturePreset, providers: &[String]) -> Option<String> {
    if let Some(id) = &preset.provider_id {
        if providers.is_empty() || providers.contains(id) {
            return Some(id.clone());
        }
    }
    (providers.len() == 1).then(|| providers[0].clone())
}

impl WhiskyApp {
    fn show_linux_hud(&mut self, container: String, entry: String) {
        let title = self
            .entries
            .iter()
            .find(|g| g.id == entry)
            .map(|g| g.title.clone())
            .unwrap_or_else(|| entry.clone());
        let context = (container, entry.clone(), title);
        let other_session = self.capture.active_game.as_ref().filter(|g| g.0 != context.0 || g.1 != context.1);
        let warning = other_session.map(|g| crate::i18n::fmt::other_game_capturing_warning(&g.2));
        self.capture.set_context(Some(context.clone()));
        if self.capture.active_game.as_ref().is_none_or(|g| g.0 != context.0 || g.1 != context.1) {
            let preset = self.capture.preset.clone().unwrap_or_default();
            let providers: Vec<_> = self
                .translation_page
                .providers
                .iter()
                .map(|p| p.id.clone())
                .collect();
            self.capture.provider_id = launch_provider(&preset, &providers);
            self.capture.architecture = if preset.architecture == "x86" {
                "x86"
            } else {
                "x64"
            }
            .into();
            self.capture.restore_history = preset.restore_history;
        }
        self.capture.hud.visible = true;
        self.capture.hud.collapsed = false;
        self.capture.hud.watched_entry = Some(context.1);
        self.capture.hud.was_active = false;
        self.capture.hud.last_capture_state = self
            .capture
            .snapshot
            .as_ref()
            .map(|s| s.capture_state.clone())
            .unwrap_or_default();
        self.capture.error = warning;
    }

    fn render_capture_hud(&mut self, scroll: &ScrollHandle, cx: &mut Context<Self>) -> AnyElement {
        let collapsed = self.capture.hud.collapsed;
        let attached = self.capture.is_actively_attached();
        let selecting = self.capture.is_selecting();
        let saved_target = self
            .capture
            .preset
            .as_ref()
            .and_then(|p| p.target_exe_path.as_ref())
            .is_some_and(|p| !p.is_empty());
        let overlay = self.capture.overlay.clone().unwrap_or_default();
        let working = self.capture.hud.launch_at.is_some()
            || [
                "translation.capture.start",
                "translation.capture.attach",
                "translation.capture.detach",
                "translation.capture.stop",
                "translation.capture.autoAttach",
            ]
            .iter()
            .any(|m| self.capture.is_pending(m));
        let other_session = self.capture.active_game.as_ref().zip(self.capture.context.as_ref())
            .is_some_and(|(active, context)| active.0 != context.0 || active.1 != context.1);
        let settings_disabled =
            (attached && !other_session) || working || self.capture.is_pending("translation.capture.preset/save");
        let badge = if self.capture.hud.launch_at.is_some() {
            strings.linux_hud.waiting_game
        } else if attached {
            strings.linux_hud.attached
        } else if selecting {
            strings.linux_hud.pick_process
        } else {
            strings.linux_hud.standby
        };
        let header = row()
            .h(px(34.))
            .flex_shrink_0()
            .px(px(8.))
            .justify_between()
            .child(
                div()
                    .text_size(px(12.))
                    .px(px(4.))
                    .rounded(px(5.))
                    .bg(rgba(if attached {
                        0x3ECF8E20
                    } else if selecting {
                        0xFF891B20
                    } else {
                        theme::card_bg()
                    }))
                    .text_color(rgb(if attached {
                        theme::SUCCESS
                    } else if selecting {
                        theme::ACCENT
                    } else {
                        theme::subtext()
                    }))
                    .child(if collapsed {
                        badge.to_owned()
                    } else {
                        crate::i18n::fmt::hud_capture_badge(badge)
                    })
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                            window.start_window_move();
                        }
                    }),
            )
            .child(
                row()
                    .gap(px(4.))
                    .when(collapsed, |r| {
                        r.child(
                            Button::new("hud-collapsed-overlay")
                                .compact()
                                .label(if overlay.visible {
                                    strings.linux_hud.sub_on
                                } else {
                                    strings.linux_hud.sub_off
                                })
                                .disabled(self.capture.overlay.is_none())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.adjust_overlay(&this.bridge, "visible");
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("hud-collapsed-input")
                                .compact()
                                .label(if overlay.click_through {
                                    strings.linux_hud.passthrough
                                } else {
                                    strings.linux_hud.drag
                                })
                                .disabled(!self.capture.overlay_supports_click_through)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.adjust_overlay(&this.bridge, "clickThrough");
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("hud-collapse")
                            .compact()
                            .label(if collapsed {
                                strings.common.expand
                            } else {
                                strings.common.collapse
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.capture.hud.collapsed = !this.capture.hud.collapsed;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("hud-close")
                            .compact()
                            .label(if collapsed { "×" } else { strings.common.close })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.capture.hud.hide();
                                cx.notify();
                            })),
                    ),
            );
        let mut root = col()
            .size_full()
            .bg(rgb(theme::bg()))
            .text_color(rgb(theme::text()))
            .text_size(px(12.))
            .font_family(crate::platform::ui_font())
            .border_1()
            .border_color(rgba(theme::surface_border()))
            .child(header);
        if collapsed {
            return root.into_any_element();
        }
        let mut body = col()
            .w_full()
            .flex_shrink_0()
            .p(px(10.))
            .gap(px(8.));
        body = body.child(muted(
            self.capture
                .context
                .as_ref()
                .or(self.capture.active_game.as_ref())
                .map(|g| g.2.clone())
                .unwrap_or_else(|| strings.linux_hud.open_from_menu.into()),
        ));
        let mut providers = row().gap(px(4.)).flex_wrap().child(strings.linux_hud.provider);
        for provider in self.translation_page.providers.clone() {
            let id = provider.id.clone();
            providers = providers.child(selectable_pill(
                SharedString::from(format!("hud-provider-{id}")),
                provider.name,
                self.capture.provider_id.as_deref() == Some(&id),
                settings_disabled,
                cx.listener(move |this, _, _, cx| {
                    this.capture.provider_id = Some(id.clone());
                    this.capture.save_hud_selection(&this.bridge);
                    cx.notify();
                }),
            ));
        }
        if self.translation_page.providers.is_empty() {
            providers = providers.child(muted(strings.translation.no_providers_generic));
        }
        body = body.child(providers);
        let mut architecture = row().gap(px(4.)).child(strings.linux_hud.arch);
        for arch in ["x64", "x86"] {
            architecture = architecture.child(selectable_pill(
                arch,
                arch,
                self.capture.effective_architecture() == arch,
                settings_disabled,
                cx.listener(move |this, _, _, cx| {
                    this.capture.architecture = arch.into();
                    this.capture.save_hud_selection(&this.bridge);
                    cx.notify();
                }),
            ));
        }
        body = body.child(architecture);
        body = body.child(
            row()
                .gap(px(5.))
                .child(strings.linux_hud.subtitle_overlay)
                .child(
                    Button::new("hud-overlay-visible")
                        .label(if overlay.visible {
                            strings.linux_hud.hide_subtitle
                        } else {
                            strings.linux_hud.show_subtitle
                        })
                        .disabled(self.capture.overlay.is_none())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.capture.adjust_overlay(&this.bridge, "visible");
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("hud-overlay-input")
                        .label(if overlay.click_through {
                            strings.linux_hud.unlock_drag
                        } else {
                            strings.linux_hud.mouse_passthrough
                        })
                        .disabled(!self.capture.overlay_supports_click_through)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.capture.adjust_overlay(&this.bridge, "clickThrough");
                            cx.notify();
                        })),
                ),
        );
        if let Some(error) = self.capture.error.clone().or_else(|| {
            self.capture
                .snapshot
                .as_ref()
                .and_then(|s| s.last_error.clone())
        }) {
            body = body.child(div().text_color(rgb(theme::DANGER)).child(error));
        }
        if let Some(notice) = &self.capture.preset_notice {
            body = body.child(muted(notice.clone()));
        }
        if attached {
            body = body.child(
                row().justify_between().child(strings.linux_hud.thread_roles_header).child(
                    Button::new("hud-detach")
                        .label(strings.capture.detach)
                        .disabled(working)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let _ = this.capture.request(
                                &this.bridge,
                                "translation.capture.detach",
                                json!({}),
                            );
                            cx.notify();
                        })),
                ),
            );
            if self.capture.threads.is_empty() {
                body = body.child(muted(strings.linux_hud.waiting_text_output));
            }
            for thread in self.capture.threads.clone() {
                let mut roles = row().gap(px(3.)).flex_wrap();
                for (role, label) in [
                    ("ignore", strings.capture.role_ignore),
                    ("body", strings.capture.role_body),
                    ("speaker", strings.capture.role_speaker),
                    ("choice", strings.capture.role_choice),
                ] {
                    let id = thread.id.clone();
                    let active = self
                        .capture
                        .thread_roles
                        .get(&id)
                        .map(String::as_str)
                        .or(thread.role.as_deref())
                        == Some(role);
                    roles = roles.child(selectable_pill(
                        SharedString::from(format!("hud-role-{id}-{role}")),
                        label,
                        active,
                        false,
                        cx.listener(move |this, _, _, cx| {
                            this.capture.select_role(&this.bridge, &id, role);
                            cx.notify();
                        }),
                    ));
                }
                body = body.child(
                    col()
                        .gap(px(4.))
                        .child(format!("{} · {}", thread.hook_name, thread.id))
                        .child(muted(thread.preview))
                        .child(roles),
                );
            }
            if self.capture.thread_total > self.capture.threads.len() {
                body = body.child(
                    row()
                        .gap(px(4.))
                        .child(
                            Button::new("hud-threads-prev")
                                .label(strings.common.previous_page)
                                .disabled(self.capture.thread_offset == 0)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.refresh_threads(
                                        &this.bridge,
                                        this.capture.thread_offset.saturating_sub(64),
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("hud-threads-next")
                                .label(strings.common.next_page)
                                .disabled(
                                    self.capture.thread_offset + self.capture.threads.len()
                                        >= self.capture.thread_total,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.refresh_threads(
                                        &this.bridge,
                                        this.capture.thread_offset + this.capture.threads.len(),
                                    );
                                    cx.notify();
                                })),
                        ),
                );
            }
        } else {
            body = body.child(
                row()
                    .gap(px(4.))
                    .flex_wrap()
                    .child(
                        Button::new("hud-start")
                            .label(strings.linux_hud.start_scan)
                            .primary()
                            .disabled(
                                working
                                    || self.capture.context.is_none()
                                    || self.capture.provider_id.is_none()
                                    || self.capture.active_game.is_some(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.capture.start(&this.bridge);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("hud-refresh")
                            .label(strings.capture.refresh_processes)
                            .disabled(!selecting || working)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.capture.request_processes(&this.bridge);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("hud-auto-attach")
                            .label(strings.linux_hud.auto_attach)
                            .disabled(
                                working
                                    || (!selecting
                                        && (!saved_target
                                            || self.capture.context.is_none()
                                            || self.capture.provider_id.is_none()
                                            || self.capture.active_game.is_some())),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.capture.is_selecting() {
                                    this.capture.trigger_manual_auto_attach(&this.bridge);
                                } else {
                                    this.capture.start_with_auto_attach(&this.bridge, true);
                                }
                                cx.notify();
                            })),
                    ),
            );
            body = body.child(div().child(strings.linux_hud.game_processes));
            if self.capture.processes.is_empty() {
                body = body.child(muted(if selecting {
                    strings.linux_hud.no_process_hint
                } else {
                    strings.linux_hud.pick_provider_hint
                }));
            }
            for process in self.capture.processes.clone() {
                let selected = self.capture.selected_process.as_ref() == Some(&process);
                let label = format!(
                    "{} · PID {} · {}",
                    exe_basename(&process.exe_path),
                    process.guest_pid,
                    process.arch
                );
                body = body.child(selectable_pill(
                    SharedString::from(format!("hud-process-{}", process.guest_pid)),
                    label,
                    selected,
                    working,
                    cx.listener(move |this, _, _, cx| {
                        this.capture.selected_process = Some(process.clone());
                        cx.notify();
                    }),
                ));
            }
            if self.capture.truncated {
                body = body.child(muted(strings.linux_hud.list_truncated_hint));
            }
            body = body.child(
                Button::new("hud-attach")
                    .label(strings.capture.attach_selected)
                    .disabled(!selecting || working || self.capture.selected_process.is_none())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.capture.attach(&this.bridge);
                        cx.notify();
                    })),
            );
        }
        root = root.child(
            div()
                .id("capture-hud-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(scroll)
                .child(body),
        );
        root = root.child(
            col()
                .h(px(52.))
                .flex_shrink_0()
                .p(px(10.))
                .child(
                    Button::new("hud-stop")
                        .label(strings.capture.stop_capture)
                        .danger()
                        .disabled(self.capture.active_game.is_none() || working)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.capture.stop(&this.bridge, "translation.capture.stop");
                            cx.notify();
                        })),
                ),
        );
        root.into_any_element()
    }
}

pub(crate) fn start(app: &Entity<WhiskyApp>, cx: &mut App) {
    let app = app.downgrade();
    cx.spawn(async move |cx| {
        let mut controller = HudController::default();
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(40))
                .await;
            let Some(entity) = app.upgrade() else { break };
            let (visible, collapsed) = entity.update(cx, |app, cx| {
                controller.sync(app, cx);
                (app.capture.hud.visible, app.capture.hud.collapsed)
            });
            cx.update(|cx| controller.sync_window(&entity, visible, collapsed, cx));
        }
    })
    .detach();
}

#[derive(Default)]
struct HudController {
    window: Option<WindowHandle<HudWindow>>,
    last_poll: Option<Instant>,
    #[cfg(debug_assertions)]
    fixture_opened: bool,
}

impl HudController {
    fn sync(&mut self, app: &mut WhiskyApp, cx: &mut Context<WhiskyApp>) {
        if !app.capture_hud_available() {
            app.capture.hud.hide();
        }
        if app.capture.hud.hide_overlay_requested
            && !app.capture.is_pending("translation.overlay.configure")
            && app.service_available("translation.overlay.configure")
        {
            if app
                .capture
                .request(
                    &app.bridge,
                    "translation.overlay.configure",
                    json!({"visible":false}),
                )
                .is_some()
            {
                app.capture.hud.hide_overlay_requested = false;
            }
        }
        #[cfg(debug_assertions)]
        if !self.fixture_opened
            && app.capture_hud_available()
            && std::env::var_os("WHISKY_HUD_TEST_OPEN").is_some()
        {
            self.fixture_opened = true;
            app.capture.hud.visible = true;
            cx.notify();
        }
        if let Some((container, entry)) = app.capture.hud.open_requested.take() {
            app.show_linux_hud(container.clone(), entry.clone());
            app.capture
                .load_preset_for_game(&app.bridge, &container, &entry);
            cx.notify();
        }
        if !app.capture.is_pending("translation.capture.preset/get") {
            if let Some((container, entry)) = app.capture.hud.launch_requested.take() {
                if let Some(preset) = app
                    .capture
                    .preset_for_game(&container, &entry)
                    .cloned()
                    .filter(|p| p.translation_enabled && p.prompt_attach_on_launch)
                {
                    let other_session = app
                        .capture
                        .active_game
                        .as_ref()
                        .is_some_and(|g| g.0 != container || g.1 != entry);
                    app.show_linux_hud(container, entry);
                    if other_session {
                        app.capture.error = Some(crate::i18n::s().linux_hud.other_session_active.into());
                    } else {
                        app.capture.preset = Some(preset);
                        app.capture.hud.launch_at =
                            Some(Instant::now() + Duration::from_millis(1200));
                        app.capture.hud.was_active = true;
                    }
                    cx.notify();
                }
            }
        }
        if app.capture.hud.visible {
            if !app.translation_page.providers_loaded
                && !app
                    .translation_page
                    .is_pending("translation.providers/list")
            {
                app.translation_page
                    .request(&app.bridge, "translation.providers/list", json!({}));
            }
            if app
                .capture
                .hud
                .launch_at
                .is_some_and(|at| Instant::now() >= at)
                && !app
                    .translation_page
                    .is_pending("translation.providers/list")
            {
                app.capture.hud.launch_at = None;
                let preset = app.capture.preset.clone().unwrap_or_default();
                let providers: Vec<_> = app
                    .translation_page
                    .providers
                    .iter()
                    .map(|p| p.id.clone())
                    .collect();
                if app.capture.active_game.is_none() {
                    app.capture.provider_id = launch_provider(&preset, &providers);
                    if app.capture.provider_id.is_none() {
                        app.capture.error = Some(
                            if providers.is_empty() {
                                crate::i18n::tr(
                                    "未配置翻译提供商，请先在 Maltoria 中配置",
                                    "翻訳プロバイダーが未設定です。先に Maltoria で設定してください",
                                    "No translation provider configured; configure one in Maltoria first",
                                    "번역 제공자가 설정되지 않았습니다. 먼저 Maltoria에서 설정하세요",
                                )
                            } else {
                                crate::i18n::s().linux_hud.pick_provider_then_scan
                            }
                            .into(),
                        );
                    } else {
                        app.capture
                            .start_with_auto_attach(&app.bridge, preset.auto_attach);
                    }
                }
                cx.notify();
            }
            let state = app
                .capture
                .snapshot
                .as_ref()
                .map(|s| s.capture_state.clone())
                .unwrap_or_default();
            let visible = app.capture.hud.visible;
            app.capture.hud.capture_state(&state);
            if visible != app.capture.hud.visible {
                cx.notify();
            }
            if self
                .last_poll
                .is_none_or(|last| last.elapsed() >= Duration::from_millis(500))
            {
                if app.capture.hud.watched_entry.is_some() {
                    let _ = app
                        .capture
                        .request(&app.bridge, "runtime.snapshot", json!({}));
                }
                if app.capture.active_game.is_some() {
                    app.capture.poll(&app.bridge);
                }
                self.last_poll = Some(Instant::now());
            }
        }
    }

    fn sync_window(
        &mut self,
        entity: &Entity<WhiskyApp>,
        visible: bool,
        collapsed: bool,
        cx: &mut App,
    ) {
        if !visible {
            if let Some(handle) = self.window.take() {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
            return;
        }
        if self.window.is_none() {
            let screen = cx
                .primary_display()
                .map(|d| d.bounds())
                .unwrap_or(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1440.), px(900.)),
                ));
            let bounds = Bounds::new(
                point(screen.right() - px(420.), screen.top() + px(40.)),
                size(px(400.), px(240.)),
            );
            let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
            let kind = if wayland {
                WindowKind::LayerShell(layer_shell::LayerShellOptions {
                    namespace: "maltoria-capture-hud".into(),
                    layer: layer_shell::Layer::Overlay,
                    anchor: layer_shell::Anchor::TOP | layer_shell::Anchor::RIGHT,
                    margin: Some((px(40.), px(20.), px(0.), px(0.))),
                    keyboard_interactivity: layer_shell::KeyboardInteractivity::None,
                    ..Default::default()
                })
            } else {
                WindowKind::PopUp
            };
            let options = |kind| WindowOptions {
                kind,
                focus: false,
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(280.), px(34.))),
                app_id: Some("com.daydalek.Maltoria.capture-hud".into()),
                ..Default::default()
            };
            let make = |window: &mut Window, cx: &mut App| {
                window.set_window_title("Maltoria capture HUD");
                cx.new(|cx| {
                    let subscription = cx.observe(entity, |_, _, cx| cx.notify());
                    HudWindow {
                        app: entity.downgrade(),
                        _subscription: subscription,
                        x11: X11Overlay::new(window, bounds),
                        scroll: ScrollHandle::new(),
                    }
                })
            };
            let opened = cx.open_window(options(kind), make);
            let opened = if wayland && opened.is_err() {
                cx.open_window(options(WindowKind::Floating), make)
            } else {
                opened
            };
            match opened {
                Ok(handle) => self.window = Some(handle),
                Err(error) => {
                    entity.update(cx, |app, cx| {
                        app.capture.hud.hide();
                        app.capture.error = Some(match crate::i18n::locale() {
                            crate::i18n::Locale::ZhHans => {
                                format!("无法打开悬浮捕获窗：{error}")
                            }
                            crate::i18n::Locale::Ja => {
                                format!("フローティングキャプチャウィンドウを開けません：{error}")
                            }
                            crate::i18n::Locale::En => {
                                format!("Unable to open floating capture window: {error}")
                            }
                            crate::i18n::Locale::Ko => {
                                format!("플로팅 캡처 창을 열 수 없습니다: {error}")
                            }
                        });
                        cx.notify();
                    });
                }
            }
        }
        if let Some(handle) = self.window {
            if handle
                .update(cx, |view, window, cx| {
                    let height = view.scroll.bounds_for_item(0).map(|b| f32::from(b.size.height));
                    let screen_height = window
                        .display(cx)
                        .map(|d| f32::from(d.bounds().size.height))
                        .unwrap_or(900.);
                    let desired = if collapsed {
                        size(px(286.), px(34.))
                    } else {
                        size(px(400.), px(expanded_height(height.unwrap_or(152.), screen_height)))
                    };
                    if window.viewport_size() != desired {
                        window.resize(desired);
                    }
                    if let Some(x11) = &mut view.x11 {
                        x11.sync_hud();
                    }
                })
                .is_err()
            {
                self.window = None;
                entity.update(cx, |app, cx| {
                    app.capture.hud.hide();
                    cx.notify();
                });
            }
        }
    }
}

struct HudWindow {
    app: WeakEntity<WhiskyApp>,
    _subscription: Subscription,
    x11: Option<X11Overlay>,
    scroll: ScrollHandle,
}

fn expanded_height(content_height: f32, screen_height: f32) -> f32 {
    (content_height + 88.).clamp(168., 460.).min((screen_height - 80.).max(34.))
}

impl Render for HudWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app
            .update(cx, |app, cx| app.render_capture_hud(&self.scroll, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{HudController, HudState, launch_provider};
    use crate::bridge::{BridgeStatus, BridgeSupervisor};
    use crate::capture::{CapturePreset, CaptureState};
    use crate::state::WhiskyApp;
    use serde_json::json;
    use std::time::{Duration, Instant};

    #[test]
    fn expanded_height_fits_content_and_caps_at_screen_and_scroll_limits() {
        assert_eq!(super::expanded_height(0., 900.), 168.);
        assert_eq!(super::expanded_height(200., 900.), 288.);
        assert_eq!(super::expanded_height(2000., 900.), 460.);
        assert_eq!(super::expanded_height(2000., 400.), 320.);
        assert_eq!(super::expanded_height(2000., 100.), 34.);
    }

    #[gpui_kit::test]
    fn manual_hud_open_keeps_requested_game_and_existing_session(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|app| gpui_kit::init(app));
        let (_tx, rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window = cx.add_window(|window, cx| WhiskyApp::new_with_bridge(window, cx, bridge, rx, false));
        window.update(cx, |app, _, cx| {
            let active = ("container-a".into(), "entry-a".into(), "Game A".into());
            app.capture.active_game = Some(active.clone());
            app.capture.snapshot = Some(crate::capture::Snapshot {
                capture_state: "attached".into(), ..Default::default()
            });
            app.capture.presets_by_game.insert(("container-b".into(), "entry-b".into()), CapturePreset {
                provider_id: Some("provider-b".into()), architecture: "x86".into(), ..Default::default()
            });
            app.show_linux_hud("container-b".into(), "entry-b".into());
            assert_eq!(app.capture.context.as_ref().unwrap().1, "entry-b");
            assert_eq!(app.capture.active_game.as_ref(), Some(&active));
            assert_eq!(app.capture.provider_id.as_deref(), Some("provider-b"));
            assert_eq!(app.capture.architecture, "x86");
            assert!(app.capture.error.as_deref().unwrap().contains("Game A"));
            let request = app.capture.request(&app.bridge, "translation.snapshot", json!({})).unwrap();
            app.capture.reply(request, "translation.snapshot", Ok(serde_json::to_value(crate::capture::Snapshot {
                capture_state: "attached".into(), preset: Some(CapturePreset::default()), ..Default::default()
            }).unwrap()), &app.bridge, cx);
            assert_eq!(app.capture.preset.as_ref().unwrap().provider_id.as_deref(), Some("provider-b"));
            app.capture.save_hud_selection(&app.bridge);
            let calls = drain();
            let (_, saved) = calls.iter().find(|(m, _)| m == "translation.capture.preset/save").unwrap();
            assert_eq!(saved["entryID"], "entry-b");
            assert_eq!(saved["providerID"], "provider-b");
            assert!(!calls.iter().any(|(m, _)| m == "translation.capture.start" || m == "translation.capture.stop"));
            app.show_linux_hud("container-a".into(), "entry-a".into());
            assert!(app.capture.error.is_none());
            assert_eq!(app.capture.active_game.as_ref(), Some(&active));
            app.capture.active_game = None;
            app.show_linux_hud("container-b".into(), "entry-b".into());
            assert!(app.capture.error.is_none());
            assert_eq!(app.capture.context.as_ref().unwrap().1, "entry-b");
        }).unwrap();
    }
    #[gpui_kit::test]
    fn launch_flags_delay_and_shared_capture_commands(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|app| gpui_kit::init(app));
        let (_tx, rx) = smol::channel::unbounded();
        let (bridge, drain) = BridgeSupervisor::recording_for_tests();
        let window =
            cx.add_window(|window, cx| WhiskyApp::new_with_bridge(window, cx, bridge, rx, false));
        window.update(cx, |app, _, cx| {
            app.bridge_status = BridgeStatus::Connected { helper: "test".into(), version: "test".into(), methods: 3 };
            app.service_methods.insert("translation.capture.start".into());
            app.translation_page.providers_loaded = true;
            for (enabled, prompt) in [(false, true), (true, false), (true, true)] {
                app.capture = CaptureState::default();
                let preset = CapturePreset { translation_enabled: enabled, prompt_attach_on_launch: prompt,
                    provider_id: Some("saved-provider".into()), architecture: "x86".into(), auto_attach: true, restore_history: true, ..Default::default() };
                app.capture.presets_by_game.insert(("container".into(), "entry".into()), preset);
                app.capture.hud.launch_requested = Some(("container".into(), "entry".into()));
                let mut controller = HudController::default();
                controller.sync(app, cx);
                assert_eq!(app.capture.hud.visible, enabled && prompt);
                let calls = drain();
                assert!(!calls.iter().any(|(m, _)| m == "translation.capture.start"));
                if enabled && prompt {
                    assert!(app.capture.hud.launch_at.unwrap() > Instant::now());
                    app.capture.hud.launch_at = Some(Instant::now() - Duration::from_millis(1));
                    controller.sync(app, cx);
                    let calls = drain();
                    let (_, params) = calls.iter().find(|(m, _)| m == "translation.capture.start").unwrap();
                    assert_eq!(params, &json!({"container":"container", "entryID":"entry", "providerID":"saved-provider", "architecture":"x86", "restoreHistory":true, "autoAttach":true}));
                    assert_eq!(app.capture.active_game, app.capture.context);
                    app.capture.hud.hide();
                    assert!(app.capture.active_game.is_some());
                }
            }
            app.capture = CaptureState::default();
            app.capture.presets_by_game.insert(("container".into(), "entry".into()), CapturePreset { translation_enabled: true, ..Default::default() });
            app.capture.hud.launch_requested = Some(("container".into(), "entry".into()));
            let mut controller = HudController::default();
            controller.sync(app, cx);
            drain();
            app.capture.hud.launch_at = Some(Instant::now() - Duration::from_millis(1));
            controller.sync(app, cx);
            assert!(app.capture.error.as_deref().is_some_and(|e| e.contains("未配置翻译提供商")));
            assert!(app.capture.active_game.is_none());
            assert!(!drain().iter().any(|(m, _)| m == "translation.capture.start"));

            app.capture = CaptureState::default();
            app.capture.active_game = Some(("old-container".into(), "old-entry".into(), "Old Game".into()));
            app.capture.presets_by_game.insert(("container".into(), "entry".into()), CapturePreset { translation_enabled: true, ..Default::default() });
            app.capture.hud.launch_requested = Some(("container".into(), "entry".into()));
            controller.sync(app, cx);
            assert!(app.capture.hud.visible);
            assert!(app.capture.hud.launch_at.is_none());
            assert_eq!(app.capture.context.as_ref().map(|g| g.1.as_str()), Some("entry"));
            assert!(app.capture.error.is_some());
            assert!(!drain().iter().any(|(m, _)| m == "translation.capture.start"));

            app.capture = CaptureState::default();
            app.capture.set_context(Some(("container".into(), "entry".into(), "Game".into())));
            app.capture.provider_id = Some("chosen".into());
            app.capture.architecture = "x86".into();
            app.capture.save_hud_selection(&app.bridge);
            let calls = drain();
            assert_eq!(calls[0].0, "translation.capture.preset/save");
            assert_eq!(calls[0].1["providerID"], "chosen");
            assert_eq!(calls[0].1["architecture"], "x86");
            app.capture.hud.visible = true;
            app.capture.stop(&app.bridge, "translation.capture.stop");
            assert!(!app.capture.hud.visible);
            app.service_methods.insert("translation.overlay.configure".into());
            app.capture.hud.watched_entry = Some("entry".into());
            app.capture.hud.was_active = true;
            app.capture.hud.runtime_snapshot(&json!({"entries":[{"id":"entry","state":"idle"}]}));
            controller.sync(app, cx);
            assert!(drain().iter().any(|(method, params)| method == "translation.overlay.configure" && params["visible"] == false));
        }).unwrap();
    }

    #[test]
    fn launch_provider_matches_macos_resolution() {
        let mut preset = CapturePreset::default();
        assert_eq!(launch_provider(&preset, &[]), None);
        assert_eq!(
            launch_provider(&preset, &["one".into(), "two".into()]),
            None
        );
        assert_eq!(
            launch_provider(&preset, &["one".into()]),
            Some("one".into())
        );
        preset.provider_id = Some("saved".into());
        assert_eq!(launch_provider(&preset, &[]), Some("saved".into()));
        assert_eq!(
            launch_provider(&preset, &["saved".into(), "other".into()]),
            Some("saved".into())
        );
        assert_eq!(
            launch_provider(&preset, &["other".into()]),
            Some("other".into())
        );
        assert_eq!(
            launch_provider(&preset, &["one".into(), "two".into()]),
            None
        );
    }

    #[test]
    fn runtime_exit_hides_hud_and_cancels_launch() {
        let mut state = HudState {
            visible: true,
            watched_entry: Some("game".into()),
            ..Default::default()
        };
        state.runtime_snapshot(&json!({"entries":[]}));
        assert!(state.visible);
        state.runtime_snapshot(&json!({"entries":[{"id":"game","state":"running"}]}));
        state.runtime_snapshot(&json!({"entries":[{"id":"other","state":"idle"}]}));
        assert!(state.visible);
        state.runtime_snapshot(&json!({"entries":[{"id":"game","state":"idle"}]}));
        assert!(!state.visible);
        assert!(state.hide_overlay_requested);
        assert!(state.launch_at.is_none());
    }

    #[test]
    fn capture_stop_and_target_exit_hide_hud_but_selecting_keeps_it_open() {
        for terminal in ["stopped", "targetExited"] {
            let mut state = HudState {
                visible: true,
                ..Default::default()
            };
            state.capture_state("stopped");
            assert!(state.visible);
            state.capture_state("attached");
            state.capture_state("selecting");
            assert!(state.visible);
            state.capture_state(terminal);
            assert!(!state.visible);
        }
    }
}
