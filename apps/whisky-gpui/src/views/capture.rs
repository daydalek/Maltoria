use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::controls::{col, icon, live_badge, muted, row};
use crate::state::WhiskyApp;
use crate::theme::*;
use crate::translation::provider_kind_label;

const THREAD_ROLES: [(&str, &str); 4] = [
    ("ignore", "忽略"),
    ("body", "正文"),
    ("speaker", "说话人"),
    ("choice", "选项"),
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
        let hook_input = if let Some(input) = self.capture.hook_input.clone() {
            input
        } else {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("显式 Hook code（UTF-8 <= 2048 字节）")
                    .default_value(self.capture.hook_code.clone())
            });
            cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.capture.hook_code = input.read(cx).value().to_string();
                    cx.notify();
                }
            })
            .detach();
            self.capture.hook_input = Some(input.clone());
            input
        };

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
        let thread_cards: Vec<AnyElement> = threads
            .iter()
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
                .child(if thread_cards.is_empty() {
                    muted("附加游戏进程并触发对话后，文本线程将显示在此处。").into_any_element()
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
                                        .label("清除会话缓存")
                                        .disabled(
                                            self.capture.is_pending("translation.cache.clear"),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let _ = this.capture.request(
                                                &this.bridge,
                                                "translation.cache.clear",
                                                json!({}),
                                            );
                                            cx.notify();
                                        })),
                                ),
                        ),
                ),
        );

        // Card 6 — macOS 原生字幕叠层
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
                                "点击穿透",
                                s.click_through,
                                overlay_disabled,
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
                    .child(divider_line())
                    .child(overlay_adjust_row(
                        "字号",
                        format!("{:.0} pt", s.font_size),
                        &[("font-", "−"), ("font+", "+")],
                        overlay_disabled,
                        cx,
                    ))
                    .child(overlay_adjust_row(
                        "不透明度",
                        format!("{:.0}%", s.opacity * 100.0),
                        &[("opacity-", "−"), ("opacity+", "+")],
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
            section_card("macOS 原生字幕叠层", None, overlay_body)
        } else {
            section_card(
                "原生字幕叠层",
                None,
                muted(if cfg!(target_os = "linux") {
                    "字幕叠层：Linux 原生窗口待实现；当前不可用。"
                } else {
                    "字幕叠层：当前桥接未公布此功能，因此不可用。"
                }),
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

fn selectable_pill(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut pill = row()
        .id(id)
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
                .child(label.into()),
        );
    if disabled {
        pill = pill.opacity(0.5);
    } else {
        pill = pill
            .cursor_pointer()
            .hover(|s| if active { s } else { s.bg(rgba(hover())) })
            .on_click(on_click);
    }
    pill.into_any_element()
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
                        .w(px(64.))
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
            Some("请先从游戏卡片右键菜单打开「游戏翻译…」。")
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
