use gpui_kit::component::{
    Disableable,
    button::Button,
    input::{Input, InputEvent, InputState},
};
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::controls::{col, muted, row};
use crate::state::WhiskyApp;

impl WhiskyApp {
    pub(crate) fn capture_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let providers = self
            .translation_page
            .providers
            .iter()
            .map(|p| {
                let id = p.id.clone();
                Button::new(format!("capture-provider:{id}"))
                    .label(format!(
                        "{} · {}{}",
                        p.name,
                        p.kind,
                        if self.capture.provider_id.as_deref() == Some(&id) {
                            " ✓"
                        } else {
                            ""
                        }
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.capture.provider_id = Some(id.clone());
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let context = self.capture.context.clone();
        let game_title = context
            .as_ref()
            .map(|x| x.2.as_str())
            .unwrap_or("未选择游戏");
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
                    .placeholder("显式 Hook code (UTF-8 <= 2048 bytes)")
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
        let body =
            col()
                .gap(px(10.))
                .p(px(18.))
                .child(
                    row()
                        .gap(px(8.))
                        .child("游戏翻译捕获")
                        .child(muted(game_title))
                        .child(
                            Button::new("capture-close")
                                .label("关闭")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.panel_open = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(row().gap(px(8.)).children(providers))
                .child(
                    row()
                        .gap(px(8.))
                        .child(
                            Button::new("capture-provider-settings")
                                .label("提供商设置…")
                                .on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.open_translation_settings(cx)
                                    }),
                                ),
                        )
                        .child(
                            Button::new("capture-arch-x64")
                                .label(if self.capture.architecture.is_empty() || self.capture.architecture == "x64" {
                                    "x64 ✓"
                                } else {
                                    "x64"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.architecture = "x64".into();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-arch-x86")
                                .label(if self.capture.architecture == "x86" {
                                    "x86 ✓"
                                } else {
                                    "x86"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.architecture = "x86".into();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-restore-history")
                                .label(if self.capture.restore_history {
                                    "恢复历史 ✓"
                                } else {
                                    "恢复历史（默认关闭）"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.restore_history = !this.capture.restore_history;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-start")
                                .label(if self.translation_page.provider_probe_pending() {
                                    "等待提供商探测完成"
                                } else {
                                    "开始并扫描进程"
                                })
                                .disabled(
                                    self.capture.provider_id.is_none()
                                        || context.is_none()
                                        || self.capture.active_game.is_some()
                                        || self.capture.is_pending("translation.capture.start")
                                        || self.translation_page.provider_probe_pending()
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.start(&this.bridge);
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child(
                            Button::new("capture-refresh-processes")
                                .label("刷新进程")
                                .disabled(self.capture.active_game.is_none() || self.capture.is_pending("translation.capture.processes"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.request_processes(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-attach")
                                .label("附加所选进程")
                                .disabled(
                                    self.capture.selected_process.is_none()
                                        || snap.as_ref().is_some_and(|s| s.target.is_some())
                                        || self.capture.is_pending("translation.capture.attach"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.attach(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("capture-detach")
                                .label("Detach")
                                .disabled(snap.as_ref().is_none_or(|s| s.target.is_none()) || self.capture.is_pending("translation.capture.detach"))
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
                                .label("停止捕获")
                                .disabled(self.capture.active_game.is_none() || self.capture.is_pending("translation.capture.stop"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.capture.stop(&this.bridge, "translation.capture.stop");
                                    cx.notify();
                                })),
                        ),
                )
                .child(muted(format!(
                    "捕获状态：{capture_status}  · 当前活动游戏：{}",
                    self.capture
                        .active_game
                        .as_ref()
                        .map(|g| g.2.as_str())
                        .unwrap_or("无")
                )))
                .child(
                    col()
                        .gap(px(4.))
                        .child("进程清单（选择完整身份记录）")
                        .child(muted(if self.capture.truncated { "进程清单达到服务端上限；可刷新后继续选择。" } else { "" }))
                        .children(processes.iter().map(|p| {
                            let record = p.clone();
                            let chosen = self.capture.selected_process.as_ref() == Some(p);
                            Button::new(format!(
                                "capture-process:{}:{}",
                                p.guest_pid, p.creation_time
                            ))
                            .label(format!(
                                "{}{} · PID {} · {} · {}",
                                if chosen { "✓ " } else { "" },
                                p.arch,
                                p.guest_pid,
                                p.exe_path,
                                p.creation_time
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.capture.selected_process = Some(record.clone());
                                cx.notify();
                            }))
                            .into_any_element()
                        })),
                )
                .child(
                    col()
                        .gap(px(4.))
                        .child("当前字幕（来源与译文状态分开显示）")
                        .child(muted(format!("来源：{source}")))
                        .child(muted(format!("译文：{translation}")))
                        .child(muted(format!("说话者：{speaker}")))
                        .child(muted(format!("翻译状态：{subtitle_status}")))
                        .child(muted(if request_pending { "捕获请求：处理中" } else { "捕获请求：空闲" }))
                        .child(muted(if translation_truncated { "译文预览已截断；可在历史中读取完整文本。" } else { "" }))
                        .child(muted(format!("翻译错误：{subtitle_error}")))
                        .child(muted(format!("捕获错误：{last_error}"))),
                )
                .child(Input::new(&hook_input))
                .child(
                    Button::new("capture-insert-hook")
                        .label("插入指定 Hook code")
                        .disabled(
                            self.capture.hook_code.is_empty()
                                || self.capture.hook_code.as_bytes().len() > 2048
                                || snap.as_ref().is_none_or(|s|s.target.is_none())
                                || self.capture.is_pending("translation.capture.insertHook"),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.capture.hook_code.as_bytes().len() <= 2048 {
                                let _ = this.capture.request(
                                    &this.bridge,
                                    "translation.capture.insertHook",
                                    json!({"hookCode":this.capture.hook_code}),
                                );
                            }
                            cx.notify();
                        })),
                )
                .child(
                    col()
                        .gap(px(4.))
                        .child(format!(
                            "文本线程 ({}/{})",
                            self.capture.threads.len(),
                            self.capture.thread_total
                        ))
                        .children(threads.iter().map(|t| {
                            let id = t.id.clone();
                            let name = if t.hook_name.is_empty() {
                                t.hook_code.clone()
                            } else {
                                t.hook_name.clone()
                            };
                            col()
                                .gap(px(3.))
                                .child(muted(format!("{} · {} · {}", name, t.id, t.preview)))
                                .child(row().gap(px(4.)).children(
                                    ["ignore", "body", "speaker", "choice"].into_iter().map(
                                        |role| {
                                            let id = id.clone();
                                            let label =
                                                if role == "ignore" { "忽略" } else { role };
                                            Button::new(format!("capture-role:{}:{role}", id))
                                                .label(format!(
                                                    "{label}{}",
                                                    if self
                                                        .capture
                                                        .thread_roles
                                                        .get(&id)
                                                        .is_some_and(|r| r == role)
                                                    {
                                                        " ✓"
                                                    } else {
                                                        ""
                                                    }
                                                ))
                                                .disabled(self.capture.is_pending("translation.threads.select"))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.capture.select_role(
                                                        &this.bridge,
                                                        &id,
                                                        role,
                                                    );
                                                    cx.notify();
                                                }))
                                                .into_any_element()
                                        },
                                    ),
                                ))
                                .into_any_element()
                        })),
                )
                .child(
                    row()
                        .gap(px(6.))
                        .child(
                            Button::new("threads-prev")
                                .label("线程上一页")
                                .disabled(self.capture.thread_offset == 0 || self.capture.is_pending("translation.threads.list"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let o = this.capture.thread_offset.saturating_sub(64);
                                    this.capture.refresh_threads(&this.bridge, o);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("threads-next")
                                .label("线程下一页")
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
                )
                .child(
                    col()
                        .gap(px(4.))
                        .child(format!(
                            "此游戏会话历史 {}/{}",
                            self.capture.history.len(),
                            history_total
                        ))
                        .children(history.iter().map(|e| {
                            let id = e.id.clone();
                            let source_id = id.clone();
                            let translation_id = id.clone();
                            let source = e.source.text.clone();
                            let text = e.translated_text.clone().unwrap_or_default();
                            let include = !e.include_in_context;
                            let full_source = self.capture.history_text.get(&(id.clone(), "source".into())).cloned();
                            let full_translation = self.capture.history_text.get(&(id.clone(), "translation".into())).cloned();
                            col()
                                .gap(px(3.))
                                .child(muted(format!("{} · {}", e.status, source)))
                                .child(muted(text))
                                .child(muted(if e.translation_truncated { "译文预览已截断；可读取完整译文。" } else { "" }))
                                .child(muted(full_source.unwrap_or_default()))
                                .child(muted(full_translation.unwrap_or_default()))
                                .child(
                                    Button::new(format!("history-include:{id}"))
                                        .label(if e.include_in_context {
                                            "排除上下文"
                                        } else {
                                            "纳入上下文"
                                        })
                                        .disabled(self.capture.is_pending("translation.history.include"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let _ = this.capture.request(
                                                &this.bridge,
                                                "translation.history.include",
                                                json!({"entryID":id,"include":include}),
                                            );
                                            cx.notify();
                                        })),
                                )
                                .child(row().gap(px(4.))
                                    .child(Button::new(format!("history-source:{source_id}")).label("读取完整来源").disabled(self.capture.is_pending("translation.history.text")).on_click(cx.listener(move |this,_,_,cx|{this.capture.load_history_text(&this.bridge,source_id.clone(),"source");cx.notify();})))
                                    .child(Button::new(format!("history-translation:{translation_id}")).label("读取完整译文").disabled(self.capture.is_pending("translation.history.text")).on_click(cx.listener(move |this,_,_,cx|{this.capture.load_history_text(&this.bridge,translation_id.clone(),"translation");cx.notify();}))))
                                .into_any_element()
                        })),
                )
                .child(
                    row()
                        .gap(px(6.))
                        .child(
                            Button::new("history-prev")
                                .label("历史上一页")
                                .disabled(self.capture.history_offset == 0 || self.capture.is_pending("translation.history"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let o = this.capture.history_offset.saturating_sub(20);
                                    this.capture.refresh_history(&this.bridge, o);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("history-next")
                                .label("历史下一页")
                                .disabled(self.capture.history_offset + 20 >= history_total || self.capture.is_pending("translation.history"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let o = this.capture.history_offset + 20;
                                    this.capture.refresh_history(&this.bridge, o);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("history-clear")
                                .label("清除当前游戏历史")
                                .disabled(self.capture.is_pending("translation.history.clear"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let _ = this.capture.request(
                                        &this.bridge,
                                        "translation.history.clear",
                                        json!({}),
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(Button::new("cache-clear").label("清除会话缓存").disabled(self.capture.is_pending("translation.cache.clear")).on_click(
                            cx.listener(|this, _, _, cx| {
                                let _ = this.capture.request(
                                    &this.bridge,
                                    "translation.cache.clear",
                                    json!({}),
                                );
                                cx.notify();
                            }),
                        )),
                )
                .child(muted(self.capture.error.clone().unwrap_or_default()))
                .child(if self.service_available("translation.overlay.read") {
                    col().gap(px(4.)).child(row().gap(px(6.)).child("macOS 原生字幕叠层")
                        .child(Button::new("overlay-read").label("读取设置").disabled(self.capture.is_pending("translation.overlay.read")).on_click(cx.listener(|this,_,_,cx|{let _=this.capture.request(&this.bridge,"translation.overlay.read",json!({}));cx.notify();}))))
                        .when_some(self.capture.overlay.clone(), |view, _settings| view
                            .child(muted(self.capture.overlay.as_ref().map(|s|format!("显示 {} · 点击穿透 {} · 源文 {} · 译文 {} · 字号 {:.0} · 不透明度 {:.2} · 尺寸 {:.0}×{:.0} · 位置 {:?},{:?}",s.visible,s.click_through,s.show_source,s.show_translation,s.font_size,s.opacity,s.width,s.height,s.x,s.y)).unwrap_or_default()))
                            .child(muted(if overlay_configure_available { "叠层配置可用。" } else { "叠层配置方法未公布；当前为只读。" }))
                            .child(row().gap(px(4.)).children([("visible","显示"),("clickThrough","点击穿透"),("showSource","源文"),("showTranslation","译文")].into_iter().map(|(action,label)|{let action=action.to_owned();Button::new(format!("overlay:{action}")).label(label).disabled(!overlay_configure_available || self.capture.is_pending("translation.overlay.configure")).on_click(cx.listener(move|this,_,_,cx|{this.capture.adjust_overlay(&this.bridge,&action);cx.notify();})).into_any_element()})))
                            .child(row().gap(px(4.)).children([("font-","字号−"),("font+","字号+"),("opacity-","不透明度−"),("opacity+","不透明度+")].into_iter().map(|(action,label)|{let action=action.to_owned();Button::new(format!("overlay:{action}")).label(label).disabled(!overlay_configure_available || self.capture.is_pending("translation.overlay.configure")).on_click(cx.listener(move|this,_,_,cx|{this.capture.adjust_overlay(&this.bridge,&action);cx.notify();})).into_any_element()})))
                            .child(row().gap(px(4.)).children([("width-","宽度−"),("width+","宽度+") ,("height-","高度−"),("height+","高度+")].into_iter().map(|(action,label)|{let action=action.to_owned();Button::new(format!("overlay:{action}")).label(label).disabled(!overlay_configure_available || self.capture.is_pending("translation.overlay.configure")).on_click(cx.listener(move|this,_,_,cx|{this.capture.adjust_overlay(&this.bridge,&action);cx.notify();})).into_any_element()})))
                            .child(row().gap(px(4.)).children([("x-","左移"),("x+","右移"),("y-","上移"),("y+","下移"),("reset","重置位置")].into_iter().map(|(action,label)|{let action=action.to_owned();Button::new(format!("overlay:{action}")).label(label).disabled(!overlay_configure_available || self.capture.is_pending("translation.overlay.configure")).on_click(cx.listener(move|this,_,_,cx|{this.capture.adjust_overlay(&this.bridge,&action);cx.notify();})).into_any_element()}))))
                } else {
                    row().child(muted(if cfg!(target_os = "linux") { "字幕叠层：Linux 原生窗口待实现；当前不可用。" } else { "字幕叠层：当前桥接未公布此功能，因此不可用。" }))
                })
                .child(muted(self.capture.overlay_platform.clone()));
        body.into_any_element()
    }
}
