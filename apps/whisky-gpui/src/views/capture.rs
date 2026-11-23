use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::capture::{CapturePreset, exe_basename, filter_threads};
use crate::controls::{col, icon, live_badge, muted, round_button, row, selectable_pill, text_tip};
use crate::state::WhiskyApp;
use crate::theme::*;
use crate::translation::provider_kind_label;
use crate::windows::capture_header_settings_button;

fn thread_roles() -> [(&'static str, &'static str); 4] {
    let c = &crate::i18n::s().capture;
    [
        ("ignore", c.role_ignore),
        ("body", c.role_body),
        ("speaker", c.role_speaker),
        ("choice", c.role_choice),
    ]
}

fn overlay_text_colors() -> [(&'static str, &'static str); 5] {
    let c = &crate::i18n::s().capture;
    [
        (c.color_white, "#FFFFFF"),
        (c.color_warm_yellow, "#FDE047"),
        (c.color_ice_blue, "#7DD3FC"),
        (c.color_mint_green, "#86EFAC"),
        (c.color_sakura_pink, "#F9A8D4"),
    ]
}

fn overlay_source_colors() -> [(&'static str, &'static str); 4] {
    let c = &crate::i18n::s().capture;
    [
        (c.color_soft_gray, "#D4D4D8"),
        (c.color_white, "#FFFFFF"),
        (c.color_light_yellow, "#FEF08A"),
        (c.color_light_blue, "#BAE6FD"),
    ]
}

impl WhiskyApp {
    /// Content rendered inside the independent native `CaptureWindow` — uses
    /// a unified macOS titlebar with traffic-light clearance and no Back button.
    pub(crate) fn capture_window_content(
        &mut self,
        left_padding: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (cards, game_title, capture_status, is_live, attached) =
            self.capture_cards_body(window, cx);
        let status_badge = if is_live {
            live_badge(if attached { crate::i18n::s().capture.badge_capturing } else { crate::i18n::s().capture.badge_scanning }).into_any_element()
        } else {
            pill_badge(capture_status_label(&capture_status), badge_bg(), badge_text())
                .into_any_element()
        };
        let header = row()
            .h(px(52.))
            .flex_shrink_0()
            .pl(px(left_padding))
            .pr(px(18.))
            .gap(px(10.))
            .items_center()
            .justify_between()
            .bg(rgba(toolbar_bg()))
            .border_b_1()
            .border_color(rgba(divider()))
            .child(
                row()
                    .flex_1()
                    .min_w_0()
                    .gap(px(10.))
                    .items_center()
                    .child(icon("languages", 16.).text_color(rgb(ACCENT)))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(text()))
                            .child(crate::i18n::s().capture.title),
                    )
                    .child(pill_badge(game_title, badge_bg(), badge_text()))
                    .child(status_badge),
            )
            .child(capture_header_settings_button(cx));

        let body = col().w_full().gap(px(14.)).p(px(22.)).children(cards);
        col()
            .size_full()
            .child(header)
            .child(div().flex_1().min_h_0().w_full().child(self.preview_scroll("capture-panel-scroll", body)))
            .into_any_element()
    }

    pub(crate) fn capture_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (cards, game_title, capture_status, is_live, attached) =
            self.capture_cards_body(window, cx);
        let top_bar = row()
            .justify_between()
            .gap(px(12.))
            .flex_wrap()
            .child(
                row()
                    .gap(px(10.))
                    .flex_wrap()
                    .child(
                        round_button("capture-close", "chevron-left")
                            .size(px(28.))
                            .rounded(px(7.))
                            .tooltip(text_tip(crate::i18n::s().library.back_to_library))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_capture_window(cx);
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(text()))
                            .child(crate::i18n::s().capture.title),
                    )
                    .child(pill_badge(game_title, badge_bg(), badge_text()))
                    .child(if is_live {
                        live_badge(if attached { crate::i18n::s().capture.badge_capturing } else { crate::i18n::s().capture.badge_scanning }).into_any_element()
                    } else {
                        pill_badge(
                            capture_status_label(&capture_status),
                            badge_bg(),
                            badge_text(),
                        )
                        .into_any_element()
                    }),
            )
            .child(capture_header_settings_button(cx));

        let body = col()
            .w_full()
            .gap(px(14.))
            .p(px(22.))
            .child(top_bar)
            .children(cards);

        self.preview_scroll("capture-panel-scroll", body)
    }

    fn capture_cards_body(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Vec<Div>, String, String, bool, bool) {
        let context = self.capture.context.clone();
        let game_title = context
            .as_ref()
            .map(|x| x.2.as_str())
            .unwrap_or(crate::i18n::s().capture.no_game_selected)
            .to_owned();
        let snap = self.capture.snapshot.clone();
        let source = snap
            .as_ref()
            .map(|s| s.subtitle.source.clone())
            .unwrap_or_default();
        let translation = snap
            .as_ref()
            .map(|s| s.subtitle.translation.clone())
            .unwrap_or_default();
        let capture_status = snap
            .as_ref()
            .map(|s| s.capture_state.as_str())
            .unwrap_or("idle")
            .to_owned();
        let subtitle_status = snap
            .as_ref()
            .map(|s| s.subtitle.status.as_str())
            .unwrap_or("idle")
            .to_owned();
        let speaker = snap
            .as_ref()
            .and_then(|s| s.subtitle.speaker.clone())
            .unwrap_or_default();
        let subtitle_error = snap
            .as_ref()
            .and_then(|s| s.subtitle.error.clone())
            .unwrap_or_default();
        let translation_truncated = snap
            .as_ref()
            .is_some_and(|s| s.subtitle.translation_truncated);
        let last_error = self
            .capture
            .error
            .clone()
            .or_else(|| snap.as_ref().and_then(|s| s.last_error.clone()))
            .unwrap_or_default();
        let request_pending = [
            "translation.capture.start",
            "translation.capture.attach",
            "translation.capture.detach",
            "translation.capture.stop",
        ]
        .into_iter()
        .any(|method| self.capture.is_pending(method));
        let overlay_configure_available = self.service_available("translation.overlay.configure");
        let processes = self.capture.processes.clone();
        let threads = self.capture.threads.clone();
        let history = if self.capture.history_belongs_to_context() {
            self.capture.history.clone()
        } else {
            Vec::new()
        };
        let history_total = if self.capture.history_belongs_to_context() {
            self.capture.history_total
        } else {
            0
        };
        let hook_input = ensure_capture_input(
            &mut self.capture.hook_input,
            crate::i18n::s().capture.hook_code_placeholder,
            self.capture.hook_code.clone(),
            |this, value| this.capture.hook_code = value,
            window,
            cx,
        );
        let thread_filter_input = ensure_capture_input(
            &mut self.capture.thread_filter_input,
            crate::i18n::s().capture.thread_filter_placeholder,
            self.capture.thread_filter_query.clone(),
            |this, value| this.capture.thread_filter_query = value,
            window,
            cx,
        );

        let is_live = self.capture.active_game.is_some()
            || snap
                .as_ref()
                .is_some_and(|s| s.target.is_some() || s.capture_state != "idle");
        let attached = snap.as_ref().is_some_and(|s| s.target.is_some());
        let arch_is_x64 =
            self.capture.architecture.is_empty() || self.capture.architecture == "x64";

        // Card 1 — 提供商与捕获控制
        let provider_pills: Vec<AnyElement> = self
            .translation_page
            .providers
            .iter()
            .map(|p| {
                let id = p.id.clone();
                let selected = self.capture.provider_id.as_deref() == Some(id.as_str());
                let kind_label = provider_kind_label(&p.kind).to_owned();
                selectable_pill(
                    format!("capture-provider:{id}"),
                    format!("{} · {kind_label}", p.name),
                    selected,
                    false,
                    cx.listener(move |this, _, _, cx| {
                        this.capture.provider_id = Some(id.clone());
                        cx.notify();
                    }),
                )
            })
            .collect();

        let active_session_subtitle = self
            .capture
            .active_game
            .as_ref()
            .map(|g| crate::i18n::s().capture.capture_active_session(&g.2));
        let probe_pending = self.translation_page.provider_probe_pending();
        let start_disabled = self.capture.provider_id.is_none()
            || context.is_none()
            || self.capture.active_game.is_some()
            || self.capture.is_pending("translation.capture.start")
            || probe_pending;
        let attach_disabled = self.capture.selected_process.is_none()
            || attached
            || self.capture.is_pending("translation.capture.attach");
        let guidance_hint = capture_guidance_hint(
            self.capture.active_game.is_some(),
            attached,
            !self.translation_page.providers.is_empty(),
            self.capture.provider_id.is_some(),
            context.is_some(),
            probe_pending,
            !processes.is_empty(),
            self.capture.selected_process.is_some(),
        );
        let auto_bound_count = snap.as_ref().map(|s| s.auto_bound_count).unwrap_or(0);
        let preset_notice = self.capture.preset_notice.clone();
        let current_preset = context
            .as_ref()
            .and_then(|(container, entry_id, _)| {
                self.capture.preset_for_game(container, entry_id).cloned()
            })
            .or_else(|| self.capture.preset.clone())
            .unwrap_or_default();
        let game_key = context
            .as_ref()
            .map(|(container, entry_id, _)| (container.clone(), entry_id.clone()));
        let preset_bar = capture_preset_bar(
            &current_preset,
            game_key,
            attached,
            self.capture.is_selecting(),
            self.capture.provider_id.is_some(),
            probe_pending,
            request_pending,
            self.capture.auto_attach_pending
                || self.capture.is_pending("translation.capture.autoAttach"),
            self.capture.is_pending("translation.capture.preset/save")
                || self.capture.is_pending("translation.capture.preset/clear"),
            self.capture_hud_available(),
            self.capture.is_pending("translation.capture.hud/show"),
            cx,
        );
        let control_card = section_card(
            crate::i18n::s().capture.section_provider_control,
            active_session_subtitle.as_deref(),
            col()
                .gap(px(12.))
                .child(
                    col()
                        .gap(px(6.))
                        .child(field_label(crate::i18n::s().capture.translation_provider))
                        .child(if provider_pills.is_empty() {
                            row()
                                .gap(px(8.))
                                .p(px(10.))
                                .rounded(px(8.))
                                .bg(rgba(card_bg()))
                                .border_1()
                                .border_color(rgba(card_border()))
                                .child(muted(crate::i18n::s().capture.no_provider_configured))
                                .into_any_element()
                        } else {
                            row().gap(px(6.)).flex_wrap().children(provider_pills).into_any_element()
                        }),
                )
                .child(
                    row()
                        .justify_between()
                        .gap(px(16.))
                        .flex_wrap()
                        .child(
                            row()
                                .gap(px(8.))
                                .child(field_label(crate::i18n::s().capture.target_architecture))
                                .child(selectable_pill(
                                    "capture-arch-x64",
                                    "x64",
                                    arch_is_x64,
                                    false,
                                    cx.listener(|this, _, _, cx| {
                                        this.capture.architecture = "x64".into();
                                        cx.notify();
                                    }),
                                ))
                                .child(selectable_pill(
                                    "capture-arch-x86",
                                    "x86",
                                    self.capture.architecture == "x86",
                                    false,
                                    cx.listener(|this, _, _, cx| {
                                        this.capture.architecture = "x86".into();
                                        cx.notify();
                                    }),
                                )),
                        )
                        .child(
                            row()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(subtext()))
                                        .child(crate::i18n::s().capture.restore_history),
                                )
                                .child(
                                    Switch::new("capture-restore-history")
                                        .checked(self.capture.restore_history)
                                        .on_change(cx.listener(|this, _, _, cx| {
                                            this.capture.restore_history =
                                                !this.capture.restore_history;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .child(divider_line())
                .child(
                    row()
                        .gap(px(8.))
                        .flex_wrap()
                        .child(
                            Button::new("capture-start")
                                .when(!start_disabled, |button| button.primary())
                                .label(if probe_pending {
                                    crate::i18n::s().capture.wait_provider_probe
                                } else {
                                    crate::i18n::s().capture.start_and_scan
                                })
                                .disabled(start_disabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.start(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-refresh-processes")
                                .label(crate::i18n::s().capture.refresh_processes)
                                .disabled(
                                    self.capture.active_game.is_none()
                                        || self.capture.is_pending("translation.capture.processes"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.request_processes(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-attach")
                                .when(!attach_disabled, |button| button.primary())
                                .label(crate::i18n::s().capture.attach_selected)
                                .disabled(attach_disabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.attach(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-detach")
                                .label(crate::i18n::s().capture.detach)
                                .disabled(
                                    !attached
                                        || self.capture.is_pending("translation.capture.detach"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let _ = this.capture.request(
                                        &this.bridge,
                                        "translation.capture.detach",
                                        json!({}),
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-stop")
                                .danger()
                                .label(crate::i18n::s().capture.stop_capture)
                                .disabled(
                                    self.capture.active_game.is_none()
                                        || self.capture.is_pending("translation.capture.stop"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.stop(&this.bridge, "translation.capture.stop");
                                    cx.notify();
                                })),
                        ),
                )
                .child(preset_bar)
                .when(auto_bound_count > 0 || preset_notice.is_some(), |d| {
                    d.child(
                        row()
                            .gap(px(8.))
                            .flex_wrap()
                            .items_center()
                            .when(auto_bound_count > 0, |r| {
                                r.child(pill_badge(
                                    crate::i18n::s().capture.capture_auto_bound(auto_bound_count as usize),
                                    installed_bg(),
                                    installed_fg(),
                                ))
                            })
                            .when_some(preset_notice, |r, note| {
                                r.child(pill_badge(note, accent_subtle_bg(), accent_fg()))
                            }),
                    )
                })
                .when_some(guidance_hint, |d, hint| {
                    d.child(
                        row()
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(8.))
                            .bg(rgba(accent_subtle_bg()))
                            .border_1()
                            .border_color(rgba(accent_subtle_border()))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(rgb(accent_fg()))
                                    .line_height(relative(1.4))
                                    .child(hint),
                            ),
                    )
                }),
        );

        // Card 2 — 游戏进程清单
        let process_cards: Vec<AnyElement> = processes
            .iter()
            .map(|p| {
                let record = p.clone();
                let chosen = self.capture.selected_process.as_ref() == Some(p);
                row()
                    .id(format!(
                        "capture-process:{}:{}",
                        p.guest_pid, p.creation_time
                    ))
                    .justify_between()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(9.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .bg(rgba(if chosen {
                        accent_subtle_bg()
                    } else {
                        card_bg()
                    }))
                    .border_1()
                    .border_color(rgba(if chosen {
                        accent_subtle_border()
                    } else {
                        card_border()
                    }))
                    .hover(|s| if chosen { s } else { s.bg(rgba(hover())) })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.capture.selected_process = Some(record.clone());
                        cx.notify();
                    }))
                    .child(
                        row()
                            .gap(px(8.))
                            .flex_1()
                            .min_w_0()
                            .when(chosen, |d| {
                                d.child(icon("check", 13.).text_color(rgb(accent_fg())))
                            })
                            .child(pill_badge(p.arch.clone(), badge_bg(), badge_text()))
                            .child(pill_badge(
                                format!("PID {}", p.guest_pid),
                                badge_bg(),
                                badge_text(),
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(12.))
                                    .font_weight(if chosen {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(rgb(if chosen { accent_fg() } else { text() }))
                                    .child(p.exe_path.clone()),
                            ),
                    )
                    .when(!p.creation_time.is_empty(), |d| {
                        d.child(muted(p.creation_time.clone()).text_size(px(11.)))
                    })
                    .into_any_element()
            })
            .collect();

        let process_card = section_card(
            crate::i18n::s().capture.process_list_title,
            Some(if self.capture.truncated {
                crate::i18n::s().capture.process_list_truncated
            } else {
                crate::i18n::s().capture.process_list_hint
            }),
            if process_cards.is_empty() {
                muted(crate::i18n::s().capture.process_list_empty)
                    .into_any_element()
            } else {
                col().gap(px(6.)).children(process_cards).into_any_element()
            },
        );

        // Card 3 — 实时字幕预览
        let has_error = !subtitle_error.is_empty() || !last_error.is_empty();
        let subtitle_card = section_card(
            crate::i18n::s().capture.live_preview_title,
            None,
            col()
                .gap(px(10.))
                .child(
                    row()
                        .gap(px(6.))
                        .flex_wrap()
                        .child(pill_badge(
                            crate::i18n::s().capture.capture_status_line(capture_status_label(&capture_status)),
                            badge_bg(),
                            badge_text(),
                        ))
                        .child(pill_badge(
                            crate::i18n::s().capture.translation_status_line(subtitle_status_label(&subtitle_status)),
                            status_badge_bg(&subtitle_status),
                            status_badge_fg(&subtitle_status),
                        ))
                        .child(pill_badge(
                            if request_pending {
                                crate::i18n::s().capture.request_in_flight
                            } else {
                                crate::i18n::s().capture.channel_idle
                            },
                            badge_bg(),
                            badge_text(),
                        ))
                        .when(!speaker.is_empty(), |d| {
                            d.child(pill_badge(
                                crate::i18n::s().capture.speaker_prefix(&speaker),
                                accent_subtle_bg(),
                                accent_fg(),
                            ))
                        }),
                )
                .child(dialogue_box(
                    crate::i18n::s().capture.source_text,
                    if source.is_empty() {
                        crate::i18n::s().capture.waiting_game_text
                    } else {
                        &source
                    },
                    source.is_empty(),
                ))
                .child(dialogue_box(
                    crate::i18n::s().capture.translated_text,
                    if translation.is_empty() {
                        crate::i18n::s().capture.waiting_translation
                    } else {
                        &translation
                    },
                    translation.is_empty(),
                ))
                .when(translation_truncated, |d| {
                    d.child(muted(crate::i18n::s().capture.preview_truncated).text_size(px(11.)))
                })
                .when(has_error, |d| {
                    d.child(error_banner(format_subtitle_error_banner(
                        &subtitle_error,
                        &last_error,
                    )))
                }),
        );

        // Card 4 — 文本线程与 Hook 注入
        let filtered_threads = filter_threads(
            &threads,
            &self.capture.thread_roles,
            self.capture.thread_filter_non_empty,
            &self.capture.thread_filter_query,
        );
        let hidden_empty_count = filtered_threads.hidden_empty_count;
        let thread_cards: Vec<AnyElement> = filtered_threads
            .threads
            .into_iter()
            .map(|t| {
                let id = t.id.clone();
                let name = if t.hook_name.is_empty() {
                    t.hook_code.clone()
                } else {
                    t.hook_name.clone()
                };
                let role_buttons: Vec<AnyElement> = thread_roles()
                    .into_iter()
                    .map(|(role, label)| {
                        let id_for_click = id.clone();
                        let active = self
                            .capture
                            .thread_roles
                            .get(&id)
                            .is_some_and(|r| r == role);
                        selectable_pill(
                            format!("capture-role:{id}:{role}"),
                            label,
                            active,
                            self.capture.is_pending("translation.threads.select"),
                            cx.listener(move |this, _, _, cx| {
                                this.capture.select_role(&this.bridge, &id_for_click, role);
                                cx.notify();
                            }),
                        )
                    })
                    .collect();

                col()
                    .min_w_0()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(
                        row()
                            .min_w_0()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(
                                div()
                                    .min_w_0()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(12.))
                                    .text_color(rgb(text()))
                                    .child(name),
                            )
                            .child(pill_badge(t.id.clone(), badge_bg(), badge_text())),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(rgb(subtext()))
                            .line_height(relative(1.4))
                            .child(if t.preview.is_empty() {
                                crate::i18n::s().capture.empty_preview_placeholder.to_owned()
                            } else {
                                t.preview.clone()
                            }),
                    )
                    .child(
                        row()
                            .min_w_0()
                            .gap(px(4.))
                            .flex_wrap()
                            .children(role_buttons),
                    )
                    .into_any_element()
            })
            .collect();

        let threads_card = section_card(
            &crate::i18n::s().capture.threads_section_title(self.capture.threads.len(), self.capture.thread_total),
            Some(crate::i18n::s().capture.threads_desc),
            col()
                .gap(px(10.))
                .child(
                    row()
                        .gap(px(8.))
                        .child(div().flex_1().min_w_0().child(Input::new(&hook_input)))
                        .child(
                            Button::new("capture-insert-hook")
                                .label(crate::i18n::s().capture.inject_hook)
                                .disabled(
                                    self.capture.hook_code.is_empty()
                                        || self.capture.hook_code.as_bytes().len() > 2048
                                        || !attached
                                        || self.capture.is_pending("translation.capture.insertHook"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if this.capture.hook_code.as_bytes().len() <= 2048 {
                                        let _ = this.capture.request(
                                            &this.bridge,
                                            "translation.capture.insertHook",
                                            json!({"hookCode": this.capture.hook_code}),
                                        );
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .items_center()
                        .flex_wrap()
                        .child(
                            div().flex_1().min_w(px(180.)).child(
                                Input::new(&thread_filter_input)
                                    .prefix(icon("search", 13.).text_color(rgb(muted_color())))
                                    .cleanable(true),
                            ),
                        )
                        .child(selectable_pill(
                            "capture-threads-filter-non-empty",
                            crate::i18n::s().capture.text_only_filter,
                            self.capture.thread_filter_non_empty,
                            false,
                            cx.listener(|this, _, _, cx| {
                                this.capture.thread_filter_non_empty =
                                    !this.capture.thread_filter_non_empty;
                                cx.notify();
                            }),
                        ))
                        .when(hidden_empty_count > 0, |d| {
                            d.child(
                                muted(crate::i18n::s().capture.hidden_empty_threads(hidden_empty_count))
                                    .text_size(px(11.)),
                            )
                        }),
                )
                .child(if thread_cards.is_empty() {
                    if threads.is_empty() {
                        muted(crate::i18n::s().capture.threads_empty_hint)
                    } else if hidden_empty_count > 0
                        && self.capture.thread_filter_query.trim().is_empty()
                    {
                        muted(crate::i18n::s().capture.hidden_empty_threads_hint(hidden_empty_count))
                    } else if hidden_empty_count > 0 {
                        muted(crate::i18n::s().capture.hidden_empty_threads_search_hint(hidden_empty_count))
                    } else {
                        muted(crate::i18n::s().capture.threads_no_match)
                    }
                    .into_any_element()
                } else {
                    col().gap(px(6.)).children(thread_cards).into_any_element()
                })
                .child(
                    row()
                        .gap(px(8.))
                        .child(
                            Button::new("threads-prev")
                                .label(crate::i18n::s().common.previous_page)
                                .disabled(
                                    self.capture.thread_offset == 0
                                        || self.capture.is_pending("translation.threads.list"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let o = this.capture.thread_offset.saturating_sub(64);
                                    this.capture.refresh_threads(&this.bridge, o);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("threads-next")
                                .label(crate::i18n::s().common.next_page)
                                .disabled(
                                    self.capture.thread_offset + 64 >= self.capture.thread_total
                                        || self.capture.is_pending("translation.threads.list"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let o = this.capture.thread_offset + 64;
                                    this.capture.refresh_threads(&this.bridge, o);
                                    cx.notify();
                                })),
                        ),
                ),
        );

        // Card 5 — 会话历史
        let history_cards: Vec<AnyElement> = history
            .iter()
            .map(|e| {
                let id = e.id.clone();
                let source_id = id.clone();
                let translation_id = id.clone();
                let source_text = e.source.text.clone();
                let translated = e.translated_text.clone().unwrap_or_default();
                let include = !e.include_in_context;
                let full_source = self
                    .capture
                    .history_text
                    .get(&(id.clone(), "source".into()))
                    .cloned();
                let full_translation = self
                    .capture
                    .history_text
                    .get(&(id.clone(), "translation".into()))
                    .cloned();

                col()
                    .gap(px(6.))
                    .p(px(12.))
                    .rounded(px(8.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(
                        row()
                            .justify_between()
                            .gap(px(8.))
                            .flex_wrap()
                            .child(
                                row()
                                    .gap(px(6.))
                                    .child(pill_badge(
                                        subtitle_status_label(&e.status),
                                        status_badge_bg(&e.status),
                                        status_badge_fg(&e.status),
                                    ))
                                    .when(e.include_in_context, |d| {
                                        d.child(pill_badge(
                                            crate::i18n::s().capture.in_context,
                                            installed_bg(),
                                            installed_fg(),
                                        ))
                                    }),
                            )
                            .child(
                                row()
                                    .gap(px(4.))
                                    .child(
                                        Button::new(format!("history-include:{id}"))
                                            .label(if e.include_in_context {
                                                crate::i18n::s().capture.exclude_context
                                            } else {
                                                crate::i18n::s().capture.include_context
                                            })
                                            .disabled(
                                                self.capture
                                                    .is_pending("translation.history.include"),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let _ = this.capture.request(
                                                    &this.bridge,
                                                    "translation.history.include",
                                                    json!({"entryID": id, "include": include}),
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new(format!("history-source:{source_id}"))
                                            .ghost()
                                            .label(crate::i18n::s().capture.load_full_source)
                                            .disabled(
                                                self.capture.is_pending("translation.history.text"),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.capture.load_history_text(
                                                    &this.bridge,
                                                    source_id.clone(),
                                                    "source",
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new(format!("history-translation:{translation_id}"))
                                            .ghost()
                                            .label(crate::i18n::s().capture.load_full_translation)
                                            .disabled(
                                                self.capture.is_pending("translation.history.text"),
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.capture.load_history_text(
                                                    &this.bridge,
                                                    translation_id.clone(),
                                                    "translation",
                                                );
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(text()))
                            .line_height(relative(1.4))
                            .child(source_text),
                    )
                    .when(!translated.is_empty(), |d| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(subtext()))
                                .line_height(relative(1.4))
                                .child(translated),
                        )
                    })
                    .when(e.translation_truncated, |d| {
                        d.child(muted(crate::i18n::s().capture.history_preview_truncated).text_size(px(11.)))
                    })
                    .when_some(full_source, |d, full| {
                        d.child(dialogue_box(crate::i18n::s().capture.full_source_title, &full, false))
                    })
                    .when_some(full_translation, |d, full| {
                        d.child(dialogue_box(crate::i18n::s().capture.full_translation_title, &full, false))
                    })
                    .into_any_element()
            })
            .collect();

        let active_game_id = snap.as_ref().and_then(|s| s.game_id.clone());
        if let Some((container, entry_id, _)) = context.as_ref() {
            self.translation_page.cache.scope_context =
                Some((container.clone(), entry_id.clone()));
        }
        if let Some(gid) = active_game_id.as_ref() {
            self.translation_page.cache.scope_game_id = Some(gid.clone());
        }
        let clear_game_id = active_game_id.clone();
        let browser_game_id = active_game_id;
        let cache_expanded = self.translation_page.cache.expanded_in_capture;
        let cache_clear_pending = self.capture.is_pending("translation.cache.clear")
            || self.translation_page.is_pending("translation.cache.clear");
        let cache_browser_view = cache_expanded
            .then(|| self.translation_cache_browser(true, browser_game_id, window, cx));

        let history_card = section_card(
            &crate::i18n::s().capture.history_section_title(self.capture.history.len(), history_total),
            None,
            col()
                .gap(px(10.))
                .child(if history_cards.is_empty() {
                    muted(crate::i18n::s().capture.history_empty).into_any_element()
                } else {
                    col().gap(px(6.)).children(history_cards).into_any_element()
                })
                .child(
                    row()
                        .justify_between()
                        .gap(px(8.))
                        .flex_wrap()
                        .child(
                            row()
                                .gap(px(6.))
                                .child(
                                    Button::new("history-prev")
                                        .label(crate::i18n::s().common.previous_page)
                                        .disabled(
                                            self.capture.history_offset == 0
                                                || self.capture.is_pending("translation.history"),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let o =
                                                this.capture.history_offset.saturating_sub(20);
                                            this.capture.refresh_history(&this.bridge, o);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("history-next")
                                        .label(crate::i18n::s().common.next_page)
                                        .disabled(
                                            self.capture.history_offset + 20 >= history_total
                                                || self.capture.is_pending("translation.history"),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let o = this.capture.history_offset + 20;
                                            this.capture.refresh_history(&this.bridge, o);
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            row()
                                .gap(px(6.))
                                .flex_wrap()
                                .child(
                                    Button::new("history-clear")
                                        .label(crate::i18n::s().capture.clear_game_history)
                                        .disabled(
                                            self.capture.is_pending("translation.history.clear"),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let _ = this.capture.request(
                                                &this.bridge,
                                                "translation.history.clear",
                                                json!({}),
                                            );
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("cache-clear")
                                        .label(crate::i18n::s().capture.clear_game_cache)
                                        .disabled(cache_clear_pending)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.translation_page.cache.has_scope()
                                                || clear_game_id.is_some()
                                            {
                                                this.translation_page.clear_scoped_cache(
                                                    &this.bridge,
                                                    clear_game_id.clone(),
                                                );
                                            } else {
                                                this.translation_page
                                                    .clear_cache(&this.bridge, None);
                                            }
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("cache-toggle-browser")
                                        .ghost()
                                        .label(if cache_expanded {
                                            crate::i18n::s().capture.collapse_sentence_cache
                                        } else {
                                            crate::i18n::s().capture.view_sentence_cache
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let next =
                                                !this.translation_page.cache.expanded_in_capture;
                                            this.translation_page.cache.expanded_in_capture = next;
                                            if next {
                                                let scope =
                                                    this.translation_page.cache.effective_game_id();
                                                this.translation_page
                                                    .refresh_cache(&this.bridge, scope);
                                            }
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .when_some(cache_browser_view, |d, browser| {
                    d.child(divider_line()).child(browser)
                }),
        );

        // Card 6 — 原生字幕叠层
        let overlay_pending = self.capture.is_pending("translation.overlay.configure");
        let overlay_disabled = !overlay_configure_available || overlay_pending;
        let overlay_card = if self.service_available("translation.overlay.read") {
            let overlay_body = col()
                .gap(px(10.))
                .child(
                    row()
                        .justify_between()
                        .gap(px(8.))
                        .child(muted(if overlay_configure_available {
                            crate::i18n::s().capture.overlay_desc
                        } else {
                            crate::i18n::s().capture.overlay_readonly
                        }))
                        .child(
                            Button::new("overlay-read")
                                .label(crate::i18n::s().capture.load_settings)
                                .disabled(self.capture.is_pending("translation.overlay.read"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let _ = this.capture.request(
                                        &this.bridge,
                                        "translation.overlay.read",
                                        json!({}),
                                    );
                                    cx.notify();
                                })),
                        ),
                )
                .when_some(self.capture.overlay.clone(), |d, s| {
                    let pos_text = match (s.x, s.y) {
                        (Some(x), Some(y)) => format!("{x:.0}, {y:.0}"),
                        _ => crate::i18n::s().common.default_label.to_owned(),
                    };
                    d.child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(overlay_toggle_pill(
                                "visible",
                                crate::i18n::s().capture.show_overlay,
                                s.visible,
                                overlay_disabled,
                                cx,
                            ))
                            .child(overlay_toggle_pill(
                                "clickThrough",
                                if s.click_through {
                                    crate::i18n::s().capture.lock_passthrough
                                } else {
                                    crate::i18n::s().capture.draggable_unlocked
                                },
                                s.click_through,
                                overlay_disabled || !self.capture.overlay_supports_click_through,
                                cx,
                            ))
                            .child(overlay_toggle_pill(
                                "showSource",
                                crate::i18n::s().capture.show_source,
                                s.show_source,
                                overlay_disabled,
                                cx,
                            ))
                            .child(overlay_toggle_pill(
                                "showTranslation",
                                crate::i18n::s().capture.show_translation,
                                s.show_translation,
                                overlay_disabled,
                                cx,
                            )),
                    )
                    .child(
                        muted(crate::i18n::s().capture.overlay_hud_tip)
                            .text_size(px(11.)),
                    )
                    .child(divider_line())
                    .child(overlay_adjust_row(
                        crate::i18n::s().capture.overlay_history_lines,
                        if s.max_entries == 1 {
                            crate::i18n::s().capture.overlay_history_single.to_owned()
                        } else {
                            crate::i18n::s().capture.overlay_history_multi(s.max_entries)
                        },
                        &[("maxEntries-", "−"), ("maxEntries+", "+")],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        crate::i18n::s().capture.font_size,
                        format!("{:.0} pt", s.font_size),
                        &[("font-", "−"), ("font+", "+")],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        crate::i18n::s().capture.bg_opacity,
                        crate::i18n::fmt::overlay_opacity_label((s.opacity * 100.0) as f32),
                        &[("opacity-", "−"), ("opacity+", "+")],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_color_row(
                        crate::i18n::s().capture.translation_color,
                        "textColor",
                        &s.text_color,
                        &overlay_text_colors(),
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_color_row(
                        crate::i18n::s().capture.source_color,
                        "sourceColor",
                        &s.source_text_color,
                        &overlay_source_colors(),
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        crate::i18n::s().capture.size_label,
                        format!("{:.0} × {:.0}", s.width, s.height),
                        &[
                            ("width-", crate::i18n::s().capture.width_minus),
                            ("width+", crate::i18n::s().capture.width_plus),
                            ("height-", crate::i18n::s().capture.height_minus),
                            ("height+", crate::i18n::s().capture.height_plus),
                        ],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        crate::i18n::s().capture.position_label,
                        pos_text,
                        &[
                            ("x-", crate::i18n::s().capture.move_left),
                            ("x+", crate::i18n::s().capture.move_right),
                            ("y-", crate::i18n::s().capture.move_up),
                            ("y+", crate::i18n::s().capture.move_down),
                            ("reset", crate::i18n::s().capture.reset_position),
                        ],
                        overlay_disabled,
                        cx,
                    ))
                })
                .when(!self.capture.overlay_platform.is_empty(), |d| {
                    d.child(muted(self.capture.overlay_platform.clone()).text_size(px(11.)))
                });
            section_card(
                if cfg!(target_os = "linux") {
                    crate::i18n::s().capture.overlay_linux_title
                } else {
                    crate::i18n::s().capture.overlay_macos_title
                },
                Some(if cfg!(target_os = "linux") {
                    crate::i18n::s().capture.overlay_linux_desc
                } else {
                    crate::i18n::s().capture.overlay_macos_desc
                }),
                overlay_body.when(cfg!(target_os = "linux") && !self.capture.overlay_supports_click_through, |d| {
                    d.child(muted(crate::i18n::s().capture.overlay_wayland_note))
                }),
            )
        } else {
            section_card(
                crate::i18n::s().capture.overlay_generic_title,
                None,
                muted(crate::i18n::s().capture.overlay_unavailable),
            )
        };

        let cards = vec![
            control_card,
            process_card,
            subtitle_card,
            threads_card,
            history_card,
            overlay_card,
        ];
        (cards, game_title, capture_status, is_live, attached)
    }
}

fn ensure_capture_input(
    slot: &mut Option<Entity<InputState>>,
    placeholder: &str,
    default_value: String,
    on_change: fn(&mut WhiskyApp, String),
    window: &mut Window,
    cx: &mut Context<WhiskyApp>,
) -> Entity<InputState> {
    if let Some(input) = slot.clone() {
        return input;
    }
    let hint = placeholder.to_owned();
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(hint)
            .default_value(default_value)
    });
    cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
        if matches!(event, InputEvent::Change) {
            on_change(this, input.read(cx).value().to_string());
            cx.notify();
        }
    })
    .detach();
    *slot = Some(input.clone());
    input
}

#[allow(clippy::too_many_arguments)]
fn capture_preset_bar(
    preset: &CapturePreset,
    game_key: Option<(String, String)>,
    attached: bool,
    selecting: bool,
    has_provider_id: bool,
    probe_pending: bool,
    request_pending: bool,
    auto_attaching: bool,
    preset_mutating: bool,
    hud_available: bool,
    hud_pending: bool,
    cx: &Context<WhiskyApp>,
) -> AnyElement {
    let has_context = game_key.is_some();
    let has_saved_state = preset.has_saved_state();
    let arch = if preset.architecture.trim().is_empty() {
        "x64".to_owned()
    } else {
        preset.architecture.clone()
    };
    let (body_count, speaker_count, choice_count) = preset.binding_counts();
    let total_bindings = preset.thread_bindings.len();
    let custom_hooks = preset.custom_hook_codes.len();
    let has_target = preset
        .target_exe_path
        .as_deref()
        .is_some_and(|p| !p.trim().is_empty());
    let target_name = preset
        .target_exe_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(exe_basename)
        .unwrap_or(crate::i18n::s().capture.no_recorded_target)
        .to_owned();
    let binding_summary = if total_bindings > 0 {
        crate::i18n::s().capture.thread_bindings_summary(body_count, choice_count, speaker_count, total_bindings)
    } else {
        crate::i18n::s().capture.thread_bindings_zero.to_owned()
    };
    let auto_attach_btn_disabled = attached
        || !has_target
        || request_pending
        || auto_attaching
        || (!selecting && (!has_context || !has_provider_id || probe_pending));

    let enabled = preset.translation_enabled;
    let prompt_on_launch = preset.prompt_attach_on_launch;
    let key_for_enable = game_key.clone();
    let key_for_prompt = game_key.clone();
    let key_for_hud = game_key;

    col()
        .gap(px(8.))
        .p(px(10.))
        .rounded(px(8.))
        .bg(rgba(card_bg()))
        .border_1()
        .border_color(rgba(card_border()))
        .child(
            row()
                .justify_between()
                .gap(px(8.))
                .flex_wrap()
                .items_center()
                .child(
                    row()
                        .gap(px(6.))
                        .flex_wrap()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(text()))
                                .child(crate::i18n::s().capture.game_preset_title),
                        )
                        .child(selectable_pill(
                            "capture-preset-translation-enabled",
                            crate::i18n::s().capture.enable_for_game,
                            enabled,
                            preset_mutating || !has_context,
                            cx.listener(move |this, _, _, cx| {
                                if let Some((container, entry_id)) = &key_for_enable {
                                    this.sync_capture_provider_selection();
                                    this.capture.toggle_preset_translation_enabled(
                                        &this.bridge,
                                        container,
                                        entry_id,
                                        !enabled,
                                    );
                                    cx.notify();
                                }
                            }),
                        ))
                        .child(selectable_pill(
                            "capture-preset-prompt-on-launch",
                            crate::i18n::s().capture.popup_hud_on_launch,
                            prompt_on_launch,
                            preset_mutating || !has_context || !enabled,
                            cx.listener(move |this, _, _, cx| {
                                if let Some((container, entry_id)) = &key_for_prompt {
                                    this.capture.toggle_preset_prompt_on_launch(
                                        &this.bridge,
                                        container,
                                        entry_id,
                                        !prompt_on_launch,
                                    );
                                    cx.notify();
                                }
                            }),
                        )),
                )
                .when(hud_available, |r| {
                    r.child(
                        Button::new("capture-open-hud")
                            .label(crate::i18n::s().capture.open_hud_window)
                            .disabled(!has_context || hud_pending)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some((container, entry_id)) = &key_for_hud {
                                    this.capture
                                        .open_capture_hud(&this.bridge, container, entry_id);
                                    cx.notify();
                                }
                            })),
                    )
                }),
        )
        .when(has_saved_state, |bar| {
            bar.child(divider_line()).child(
                row()
                    .justify_between()
                    .gap(px(8.))
                    .flex_wrap()
                    .items_center()
                    .child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(text()))
                                    .child(crate::i18n::s().capture.remembered_preset_title),
                            )
                            .child(pill_badge(
                                target_name,
                                accent_subtle_bg(),
                                accent_fg(),
                            ))
                            .child(pill_badge(arch, badge_bg(), badge_text()))
                            .child(pill_badge(binding_summary, badge_bg(), badge_text()))
                            .when(custom_hooks > 0, |r| {
                                r.child(pill_badge(
                                    crate::i18n::s().capture.custom_hooks_count(custom_hooks),
                                    badge_bg(),
                                    badge_text(),
                                ))
                            }),
                    )
                    .child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .items_center()
                            .child(selectable_pill(
                                "capture-preset-auto-attach",
                                crate::i18n::s().capture.auto_attach_on_open,
                                preset.auto_attach,
                                preset_mutating || !has_context,
                                cx.listener(|this, _, _, cx| {
                                    this.capture.toggle_preset_auto_attach(&this.bridge);
                                    cx.notify();
                                }),
                            ))
                            .when(!attached, |r| {
                                r.child(
                                    Button::new("capture-preset-run-auto-attach")
                                        .when(!auto_attach_btn_disabled, |btn| btn.primary())
                                        .label(if auto_attaching {
                                            crate::i18n::s().capture.auto_attaching
                                        } else {
                                            crate::i18n::s().capture.auto_attach_and_restore
                                        })
                                        .disabled(auto_attach_btn_disabled)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if this.capture.is_selecting() {
                                                this.capture.trigger_manual_auto_attach(&this.bridge);
                                            } else {
                                                this.capture.start_with_auto_attach(&this.bridge, true);
                                            }
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                Button::new("capture-preset-clear")
                                    .ghost()
                                    .label(crate::i18n::s().capture.clear_memory)
                                    .disabled(preset_mutating || !has_context)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.capture.clear_preset(&this.bridge);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
        })
        .into_any_element()
}

fn section_card(title: &str, subtitle: Option<&str>, body: impl IntoElement) -> Div {
    col()
        .p(px(16.))
        .gap(px(12.))
        .rounded(px(10.))
        .bg(rgba(section_bg()))
        .border_1()
        .border_color(rgba(section_border()))
        .child(
            col()
                .gap(px(3.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(14.))
                        .text_color(rgb(text()))
                        .child(title.to_owned()),
                )
                .when_some(subtitle, |d, sub| {
                    d.child(muted(sub.to_owned()).line_height(relative(1.4)))
                }),
        )
        .child(body)
}

fn field_label(label: &str) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(subtext()))
        .child(label.to_owned())
}

fn pill_badge(label: impl Into<SharedString>, bg_rgba: u32, fg_rgb: u32) -> Div {
    row()
        .flex_shrink_0()
        .px(px(8.))
        .py(px(2.))
        .rounded_full()
        .bg(rgba(bg_rgba))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(fg_rgb))
        .child(label.into())
}

fn dialogue_box(label: &str, text_value: &str, is_empty: bool) -> Div {
    col()
        .gap(px(4.))
        .p(px(12.))
        .rounded(px(8.))
        .bg(rgba(card_bg()))
        .border_1()
        .border_color(rgba(card_border()))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(muted_color()))
                .child(label.to_owned()),
        )
        .child(
            div()
                .text_size(px(13.))
                .line_height(relative(1.45))
                .text_color(rgb(if is_empty { muted_color() } else { text() }))
                .child(text_value.to_owned()),
        )
}

fn error_banner(message: String) -> Div {
    row()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .rounded(px(8.))
        .bg(rgba(0xff5f571a))
        .border_1()
        .border_color(rgba(0xff5f574d))
        .child(icon("triangle-alert", 14.).text_color(rgb(danger_fg())))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.))
                .text_color(rgb(danger_fg()))
                .child(message),
        )
}

fn overlay_toggle_pill(
    action: &'static str,
    label: &'static str,
    active: bool,
    disabled: bool,
    cx: &Context<WhiskyApp>,
) -> AnyElement {
    selectable_pill(
        format!("overlay:{action}"),
        label,
        active,
        disabled,
        cx.listener(move |this, _, _, cx| {
            this.capture.adjust_overlay(&this.bridge, action);
            cx.notify();
        }),
    )
}

fn overlay_adjust_row(
    label: &str,
    value: String,
    actions: &[(&'static str, &'static str)],
    disabled: bool,
    cx: &Context<WhiskyApp>,
) -> Div {
    let buttons = actions.iter().map(|&(action, btn_label)| {
        Button::new(format!("overlay:{action}"))
            .label(btn_label)
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.capture.adjust_overlay(&this.bridge, action);
                cx.notify();
            }))
            .into_any_element()
    });
    row()
        .justify_between()
        .gap(px(12.))
        .flex_wrap()
        .child(
            row()
                .gap(px(10.))
                .child(
                    div()
                        .w(px(88.))
                        .text_size(px(12.))
                        .text_color(rgb(subtext()))
                        .child(label.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(text()))
                        .child(value),
                ),
        )
        .child(row().gap(px(6.)).flex_wrap().children(buttons))
}

fn overlay_color_row(
    label: &str,
    action_prefix: &'static str,
    current_hex: &str,
    presets: &[(&'static str, &'static str)],
    disabled: bool,
    cx: &Context<WhiskyApp>,
) -> Div {
    let chips = presets.iter().map(|&(color_label, hex)| {
        let active = current_hex.eq_ignore_ascii_case(hex);
        overlay_color_chip(action_prefix, color_label, hex, active, disabled, cx)
    });
    row()
        .justify_between()
        .items_center()
        .gap(px(12.))
        .flex_wrap()
        .child(
            div()
                .w(px(88.))
                .text_size(px(12.))
                .text_color(rgb(subtext()))
                .child(label.to_owned()),
        )
        .child(row().flex_1().min_w_0().gap(px(6.)).flex_wrap().children(chips))
}

fn overlay_color_chip(
    action_prefix: &'static str,
    label: &'static str,
    hex: &'static str,
    active: bool,
    disabled: bool,
    cx: &Context<WhiskyApp>,
) -> AnyElement {
    let swatch_rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    let mut chip = row()
        .id(format!("overlay:{action_prefix}:{hex}"))
        .gap(px(5.))
        .px(px(10.))
        .py(px(5.))
        .rounded(px(7.))
        .bg(rgba(if active {
            accent_subtle_bg()
        } else {
            card_bg()
        }))
        .border_1()
        .border_color(rgba(if active {
            accent_subtle_border()
        } else {
            card_border()
        }))
        .child(
            div()
                .size(px(10.))
                .rounded(px(3.))
                .bg(rgb(swatch_rgb))
                .border_1()
                .border_color(rgba(card_border())),
        )
        .when(active, |d| {
            d.child(icon("check", 12.).text_color(rgb(accent_fg())))
        })
        .child(
            div()
                .text_size(px(12.))
                .font_weight(if active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(if active { accent_fg() } else { text() }))
                .child(label),
        );
    if disabled {
        chip = chip.opacity(0.5);
    } else {
        let action = format!("{action_prefix}:{hex}");
        chip = chip
            .cursor_pointer()
            .hover(|s| if active { s } else { s.bg(rgba(hover())) })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.capture.adjust_overlay(&this.bridge, &action);
                cx.notify();
            }));
    }
    chip.into_any_element()
}

fn divider_line() -> Div {
    div().w_full().h(px(1.)).bg(rgba(divider()))
}

#[allow(clippy::too_many_arguments)]
fn capture_guidance_hint(
    has_active_game: bool,
    attached: bool,
    has_providers: bool,
    has_provider_id: bool,
    has_context: bool,
    probe_pending: bool,
    has_processes: bool,
    has_selected_process: bool,
) -> Option<&'static str> {
    if !has_active_game {
        let c = &crate::i18n::s().capture;
        if !has_providers {
            Some(c.guide_need_provider)
        } else if !has_provider_id {
            Some(c.guide_select_provider)
        } else if !has_context {
            Some(c.guide_open_from_menu)
        } else if probe_pending {
            Some(c.guide_wait_probe)
        } else {
            None
        }
    } else if !attached {
        let c = &crate::i18n::s().capture;
        if !has_processes {
            Some(c.guide_refresh_proc)
        } else if !has_selected_process {
            Some(c.guide_pick_proc)
        } else {
            None
        }
    } else {
        None
    }
}

fn capture_status_label(status: &str) -> &str {
    let c = &crate::i18n::s().capture;
    match status {
        "idle" => c.status_idle,
        "scanning" => c.status_scanning,
        "running" => c.status_running,
        "attached" => c.status_attached,
        "detaching" => c.status_detaching,
        "stopping" => c.status_stopping,
        "failed" | "error" => c.status_abnormal,
        other => other,
    }
}

fn subtitle_status_label(status: &str) -> &str {
    let c = &crate::i18n::s().capture;
    match status {
        "idle" => c.status_idle,
        "translating" | "pending" => c.sub_translating,
        "translated" | "completed" => c.sub_translated,
        "failed" | "error" => c.sub_failed,
        "skipped" => crate::i18n::s().common.skipped,
        other => other,
    }
}

fn status_badge_bg(status: &str) -> u32 {
    match status {
        "translated" | "completed" => installed_bg(),
        "translating" | "pending" => accent_subtle_bg(),
        "failed" | "error" => 0xff5f571a,
        _ => badge_bg(),
    }
}

fn format_capture_error(error: &str) -> &str {
    match error {
        "events_dropped" => crate::i18n::s().capture.err_events_dropped,
        _ => crate::i18n::bridge_errors::code_text(error).unwrap_or(error),
    }
}

fn format_subtitle_error_banner(subtitle_error: &str, last_error: &str) -> String {
    let formatted_subtitle = crate::i18n::bridge_errors::code_text(subtitle_error)
        .unwrap_or(subtitle_error);
    let formatted_last = format_capture_error(last_error);
    crate::i18n::fmt::capture_and_subtitle_errors(formatted_subtitle, formatted_last)
}

fn status_badge_fg(status: &str) -> u32 {
    match status {
        "translated" | "completed" => installed_fg(),
        "translating" | "pending" => accent_fg(),
        "failed" | "error" => danger_fg(),
        _ => badge_text(),
    }
}

#[cfg(test)]
mod tests {
    use super::format_subtitle_error_banner;

    #[test]
    fn formats_events_dropped_with_explanation() {
        assert_eq!(
            format_subtitle_error_banner("", "events_dropped"),
            "捕获错误：events_dropped（游戏高频触发底层文本钩子导致事件队列溢出；可忽略系统噪声线程，仅保留正文线程）"
        );
        assert_eq!(
            format_subtitle_error_banner("timeout", "events_dropped"),
            "翻译错误：翻译超时；捕获错误：events_dropped（游戏高频触发底层文本钩子导致事件队列溢出；可忽略系统噪声线程，仅保留正文线程）"
        );
        assert_eq!(
            format_subtitle_error_banner("", "disconnected"),
            "捕获错误：文本钩子连接已断开"
        );
    }
}
