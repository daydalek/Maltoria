use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::capture::{CapturePreset, exe_basename, filter_threads};
use crate::controls::{col, icon, live_badge, muted, row, selectable_pill};
use crate::state::WhiskyApp;
use crate::theme::*;
use crate::translation::provider_kind_label;

const THREAD_ROLES: [(&str, &str); 4] = [
    ("ignore", "忽略"),
    ("body", "正文"),
    ("speaker", "说话人"),
    ("choice", "选项"),
];

const OVERLAY_TEXT_COLORS: [(&str, &str); 5] = [
    ("纯白", "#FFFFFF"),
    ("暖黄", "#FDE047"),
    ("冰蓝", "#7DD3FC"),
    ("薄荷绿", "#86EFAC"),
    ("樱粉", "#F9A8D4"),
];

const OVERLAY_SOURCE_COLORS: [(&str, &str); 4] = [
    ("柔灰", "#D4D4D8"),
    ("纯白", "#FFFFFF"),
    ("浅黄", "#FEF08A"),
    ("浅蓝", "#BAE6FD"),
];

impl WhiskyApp {
    pub(crate) fn capture_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let context = self.capture.context.clone();
        let game_title = context
            .as_ref()
            .map(|x| x.2.as_str())
            .unwrap_or("未选择游戏")
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
            "显式 Hook code（UTF-8 <= 2048 字节）",
            self.capture.hook_code.clone(),
            |this, value| this.capture.hook_code = value,
            window,
            cx,
        );
        let thread_filter_input = ensure_capture_input(
            &mut self.capture.thread_filter_input,
            "过滤 Hook 名称或预览文本...",
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

        // Top Bar
        let top_bar = row()
            .justify_between()
            .gap(px(12.))
            .flex_wrap()
            .child(
                row()
                    .gap(px(10.))
                    .flex_wrap()
                    .child(
                        Button::new("capture-close")
                            .ghost()
                            .label("← 返回")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.capture.panel_open = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(text()))
                            .child("游戏翻译"),
                    )
                    .child(pill_badge(game_title, badge_bg(), badge_text()))
                    .child(if is_live {
                        live_badge(if attached { "捕获中" } else { "扫描中" }).into_any_element()
                    } else {
                        pill_badge(
                            capture_status_label(&capture_status),
                            badge_bg(),
                            badge_text(),
                        )
                        .into_any_element()
                    }),
            )
            .child(
                Button::new("capture-provider-settings")
                    .label("提供商设置…")
                    .on_click(cx.listener(|this, _, _, cx| this.open_translation_settings(cx))),
            );

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
            .map(|g| format!("当前活动会话：{}", g.2));
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
            "提供商与捕获控制",
            active_session_subtitle.as_deref(),
            col()
                .gap(px(12.))
                .child(
                    col()
                        .gap(px(6.))
                        .child(field_label("翻译提供商"))
                        .child(if provider_pills.is_empty() {
                            row()
                                .gap(px(8.))
                                .p(px(10.))
                                .rounded(px(8.))
                                .bg(rgba(card_bg()))
                                .border_1()
                                .border_color(rgba(card_border()))
                                .child(muted("尚未配置翻译提供商，请点击右上角「提供商设置…」添加。"))
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
                                .child(field_label("目标架构"))
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
                                        .child("恢复历史"),
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
                                    "等待提供商探测完成"
                                } else {
                                    "开始并扫描进程"
                                })
                                .disabled(start_disabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.start(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-refresh-processes")
                                .label("刷新进程")
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
                                .label("附加所选进程")
                                .disabled(attach_disabled)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.attach(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-detach")
                                .label("分离")
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
                                .label("停止捕获")
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
                                    format!("✓ 已自动恢复 {auto_bound_count} 个线程绑定"),
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
            "游戏进程清单",
            Some(if self.capture.truncated {
                "进程清单达到服务端上限；可刷新后继续选择完整进程身份记录。"
            } else {
                "选择完整身份记录（架构、PID 与启动时间）后再附加。"
            }),
            if process_cards.is_empty() {
                muted("尚无可用进程；启动游戏并点击「开始并扫描进程」或「刷新进程」。")
                    .into_any_element()
            } else {
                col().gap(px(6.)).children(process_cards).into_any_element()
            },
        );

        // Card 3 — 实时字幕预览
        let has_error = !subtitle_error.is_empty() || !last_error.is_empty();
        let subtitle_card = section_card(
            "实时字幕预览",
            None,
            col()
                .gap(px(10.))
                .child(
                    row()
                        .gap(px(6.))
                        .flex_wrap()
                        .child(pill_badge(
                            format!("捕获：{}", capture_status_label(&capture_status)),
                            badge_bg(),
                            badge_text(),
                        ))
                        .child(pill_badge(
                            format!("翻译：{}", subtitle_status_label(&subtitle_status)),
                            status_badge_bg(&subtitle_status),
                            status_badge_fg(&subtitle_status),
                        ))
                        .child(pill_badge(
                            if request_pending {
                                "请求处理中"
                            } else {
                                "通道空闲"
                            },
                            badge_bg(),
                            badge_text(),
                        ))
                        .when(!speaker.is_empty(), |d| {
                            d.child(pill_badge(
                                format!("说话人：{speaker}"),
                                accent_subtle_bg(),
                                accent_fg(),
                            ))
                        }),
                )
                .child(dialogue_box(
                    "原文",
                    if source.is_empty() {
                        "等待游戏文本输入…"
                    } else {
                        &source
                    },
                    source.is_empty(),
                ))
                .child(dialogue_box(
                    "译文",
                    if translation.is_empty() {
                        "等待翻译结果…"
                    } else {
                        &translation
                    },
                    translation.is_empty(),
                ))
                .when(translation_truncated, |d| {
                    d.child(muted("译文预览已截断；可在下方会话历史中读取完整文本。").text_size(px(11.)))
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
                let role_buttons: Vec<AnyElement> = THREAD_ROLES
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
                                "（暂无预览文本）".to_owned()
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
            &format!(
                "文本线程与 Hook 注入（{}/{}）",
                self.capture.threads.len(),
                self.capture.thread_total
            ),
            Some("为捕获到的文本线程分配角色（正文 / 说话人 / 选项），或注入自定义 Hook code。"),
            col()
                .gap(px(10.))
                .child(
                    row()
                        .gap(px(8.))
                        .child(div().flex_1().min_w_0().child(Input::new(&hook_input)))
                        .child(
                            Button::new("capture-insert-hook")
                                .label("注入 Hook")
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
                            "仅看有文本",
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
                                muted(format!("已隐藏 {hidden_empty_count} 条空预览"))
                                    .text_size(px(11.)),
                            )
                        }),
                )
                .child(if thread_cards.is_empty() {
                    if threads.is_empty() {
                        muted("附加游戏进程并触发对话后，文本线程将显示在此处。")
                    } else if hidden_empty_count > 0
                        && self.capture.thread_filter_query.trim().is_empty()
                    {
                        muted(format!(
                            "当前有 {hidden_empty_count} 条空预览线程被隐藏，点击「仅看有文本」可查看全部。"
                        ))
                    } else if hidden_empty_count > 0 {
                        muted(format!(
                            "没有匹配当前搜索条件的可见线程（另有 {hidden_empty_count} 条空预览线程被「仅看有文本」隐藏）。"
                        ))
                    } else {
                        muted("没有匹配当前搜索条件的文本线程。")
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
                                .label("上一页")
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
                                .label("下一页")
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
                                            "已纳入上下文",
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
                                                "排除上下文"
                                            } else {
                                                "纳入上下文"
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
                                            .label("读取完整来源")
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
                                            .label("读取完整译文")
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
                        d.child(muted("译文预览已截断；点击「读取完整译文」查看全部。").text_size(px(11.)))
                    })
                    .when_some(full_source, |d, full| {
                        d.child(dialogue_box("完整来源", &full, false))
                    })
                    .when_some(full_translation, |d, full| {
                        d.child(dialogue_box("完整译文", &full, false))
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
            &format!("会话历史（{}/{}）", self.capture.history.len(), history_total),
            None,
            col()
                .gap(px(10.))
                .child(if history_cards.is_empty() {
                    muted("当前游戏暂无翻译历史记录。").into_any_element()
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
                                        .label("上一页")
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
                                        .label("下一页")
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
                                        .label("清除当前游戏历史")
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
                                        .label("清除当前游戏缓存")
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
                                            "收起句缓存"
                                        } else {
                                            "查看/管理句缓存"
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
                            "控制桌面原生悬浮字幕窗口的显示方式、尺寸与位置。"
                        } else {
                            "叠层配置方法未公布；当前仅支持只读查看。"
                        }))
                        .child(
                            Button::new("overlay-read")
                                .label("读取设置")
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
                        _ => "默认".to_owned(),
                    };
                    d.child(
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(overlay_toggle_pill(
                                "visible",
                                "显示叠层",
                                s.visible,
                                overlay_disabled,
                                cx,
                            ))
                            .child(overlay_toggle_pill(
                                "clickThrough",
                                if s.click_through {
                                    "锁定位置（穿透）"
                                } else {
                                    "可拖动（已解锁）"
                                },
                                s.click_through,
                                overlay_disabled || !self.capture.overlay_supports_click_through,
                                cx,
                            ))
                            .child(overlay_toggle_pill(
                                "showSource",
                                "显示原文",
                                s.show_source,
                                overlay_disabled,
                                cx,
                            ))
                            .child(overlay_toggle_pill(
                                "showTranslation",
                                "显示译文",
                                s.show_translation,
                                overlay_disabled,
                                cx,
                            )),
                    )
                    .child(
                        muted("提示：也可直接从桌面悬浮球快速切换字幕显隐与位置锁定/拖动状态。")
                            .text_size(px(11.)),
                    )
                    .child(divider_line())
                    .child(overlay_adjust_row(
                        "字号",
                        format!("{:.0} pt", s.font_size),
                        &[("font-", "−"), ("font+", "+")],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        "背景不透明度",
                        format!("{:.0}%（0%–100%，0% 全透明）", s.opacity * 100.0),
                        &[("opacity-", "−"), ("opacity+", "+")],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_color_row(
                        "译文字色",
                        "textColor",
                        &s.text_color,
                        &OVERLAY_TEXT_COLORS,
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_color_row(
                        "原文字色",
                        "sourceColor",
                        &s.source_text_color,
                        &OVERLAY_SOURCE_COLORS,
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        "尺寸",
                        format!("{:.0} × {:.0}", s.width, s.height),
                        &[
                            ("width-", "宽 −"),
                            ("width+", "宽 +"),
                            ("height-", "高 −"),
                            ("height+", "高 +"),
                        ],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        "位置",
                        pos_text,
                        &[
                            ("x-", "左移"),
                            ("x+", "右移"),
                            ("y-", "上移"),
                            ("y+", "下移"),
                            ("reset", "重置位置"),
                        ],
                        overlay_disabled,
                        cx,
                    ))
                })
                .when(!self.capture.overlay_platform.is_empty(), |d| {
                    d.child(muted(self.capture.overlay_platform.clone()).text_size(px(11.)))
                });
            section_card(
                if cfg!(target_os = "linux") { "Linux 原生字幕叠层" } else { "macOS 原生字幕叠层" },
                Some(if cfg!(target_os = "linux") { "X11 支持全屏置顶与鼠标穿透；解锁后拖动窗口内部移动，拖动边缘或角落调整尺寸。" } else { "支持在全屏游戏 Space 上方悬浮显示原生字幕窗口。" }),
                overlay_body.when(cfg!(target_os = "linux") && !self.capture.overlay_supports_click_through, |d| {
                    d.child(muted("Wayland 不支持鼠标穿透或精确定位；使用 LayerShell（如可用），否则为尽力置顶的浮动窗口。"))
                }),
            )
        } else {
            section_card(
                "原生字幕叠层",
                None,
                muted("字幕叠层：当前桥接未公布此功能，因此不可用。"),
            )
        };

        let body = col()
            .w_full()
            .gap(px(14.))
            .p(px(22.))
            .child(top_bar)
            .child(control_card)
            .child(process_card)
            .child(subtitle_card)
            .child(threads_card)
            .child(history_card)
            .child(overlay_card);

        div()
            .id("capture-panel-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(body)
            .into_any_element()
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
        .unwrap_or("未记录目标进程")
        .to_owned();
    let binding_summary = if total_bindings > 0 {
        format!(
            "线程绑定 {total_bindings}（正文 {body_count} · 说话人 {speaker_count} · 选项 {choice_count}）"
        )
    } else {
        "线程绑定 0".to_owned()
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
                                .child("游戏翻译预设"),
                        )
                        .child(selectable_pill(
                            "capture-preset-translation-enabled",
                            "为此游戏启用翻译",
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
                            "启动时弹出悬浮捕获窗",
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
                            .label("打开悬浮捕获窗")
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
                                    .child("已记忆附加预设"),
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
                                    format!("自定义 Hook {custom_hooks}"),
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
                                "打开捕获时自动附加",
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
                                            "自动附加中…"
                                        } else {
                                            "自动附加并恢复"
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
                                    .label("清除记忆")
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
        if !has_providers {
            Some("请先点击右上角「提供商设置…」添加并保存至少一个翻译提供商。")
        } else if !has_provider_id {
            Some("请先在上方点击选择一个翻译提供商，然后即可点击「开始并扫描进程」。")
        } else if !has_context {
            Some("请先从游戏详情、画廊、沉浸视图、运行设置或卡片右键菜单打开「游戏翻译与捕获」。")
        } else if probe_pending {
            Some("正在等待提供商探测完成，完成后即可开始捕获。")
        } else {
            None
        }
    } else if !attached {
        if !has_processes {
            Some("正在扫描或暂未发现可用进程，可点击「刷新进程」重试。")
        } else if !has_selected_process {
            Some("请在下方「游戏进程清单」中点击选择目标进程（如游戏主程序），再点击「附加所选进程」。")
        } else {
            None
        }
    } else {
        None
    }
}

fn capture_status_label(status: &str) -> &str {
    match status {
        "idle" => "空闲",
        "scanning" => "扫描中",
        "running" => "运行中",
        "attached" => "已附加",
        "detaching" => "正在分离",
        "stopping" => "正在停止",
        "failed" | "error" => "异常",
        other => other,
    }
}

fn subtitle_status_label(status: &str) -> &str {
    match status {
        "idle" => "空闲",
        "translating" | "pending" => "翻译中",
        "translated" | "completed" => "已翻译",
        "failed" | "error" => "失败",
        "skipped" => "已跳过",
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
        "events_dropped" => {
            "events_dropped（游戏高频触发底层文本钩子导致事件队列溢出；可忽略系统噪声线程，仅保留正文线程）"
        }
        other => other,
    }
}

fn format_subtitle_error_banner(subtitle_error: &str, last_error: &str) -> String {
    let formatted_last = format_capture_error(last_error);
    match (subtitle_error.is_empty(), last_error.is_empty()) {
        (false, false) => format!("翻译错误：{subtitle_error}；捕获错误：{formatted_last}"),
        (false, true) => format!("翻译错误：{subtitle_error}"),
        (true, false) => format!("捕获错误：{formatted_last}"),
        (true, true) => String::new(),
    }
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
            "翻译错误：timeout；捕获错误：events_dropped（游戏高频触发底层文本钩子导致事件队列溢出；可忽略系统噪声线程，仅保留正文线程）"
        );
        assert_eq!(
            format_subtitle_error_banner("", "disconnected"),
            "捕获错误：disconnected"
        );
    }
}
