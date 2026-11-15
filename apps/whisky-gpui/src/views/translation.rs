use gpui_kit::component::{
    Disableable,
    button::Button,
    input::{Input, Textarea},
    switch::Switch,
};
use gpui_kit::{prelude::*, *};

use crate::controls::{col, muted, row};
use crate::state::WhiskyApp;

const PROVIDER_KINDS: [(&str, &str); 6] = [
    ("OpenAI Chat", "openAIChat"),
    ("OpenAI Completions", "openAICompletions"),
    ("Ollama", "ollama"),
    ("DeepL", "deepL"),
    ("Anthropic", "anthropic"),
    ("Gemini", "gemini"),
];

impl WhiskyApp {
    pub(crate) fn translation_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = &mut self.translation_page;
        let selected = page.selected_id.is_some();
        let profiles = page
            .providers
            .iter()
            .map(|p| {
                let id = p.id.clone();
                Button::new(format!("translation-provider:{id}"))
                    .label(format!("{} · {}", p.name, p.kind))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.translation_page.select(Some(id.clone()));
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let mut add = Vec::new();
        for (label, kind) in PROVIDER_KINDS {
            add.push(
                Button::new(format!("translation-add:{kind}"))
                    .label(format!("+ {label}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.translation_page.new_provider(kind);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        let kind_buttons = PROVIDER_KINDS
            .into_iter()
            .map(|(label, kind)| {
                let kind = kind.to_owned();
                Button::new(format!("translation-kind:{kind}"))
                    .label(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.translation_page.set_kind(&kind);
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let settings = page.settings.clone();
        let name_value = page.draft.name.clone();
        let endpoint_value = page.draft.base_url.clone();
        let model_value = page.draft.model_id.clone();
        let source_value = page.draft.source_language.clone();
        let target_value = page.draft.target_language.clone();
        let global_source_value = settings
            .as_ref()
            .map(|s| s.source_language.as_str())
            .unwrap_or("auto")
            .to_owned();
        let global_target_value = settings
            .as_ref()
            .map(|s| s.target_language.as_str())
            .unwrap_or("en")
            .to_owned();
        let mut field =
            |key, value: &str, hint: &str, window: &mut Window, cx: &mut Context<Self>| {
                Input::new(&page.input(key, value, hint, window, cx))
            };
        let name = field("name", &name_value, "提供商名称", window, cx);
        let endpoint = field("endpoint", &endpoint_value, "API 端点", window, cx);
        let model = field("model", &model_value, "模型 ID（支持手动输入）", window, cx);
        let source = field(
            "provider-source-language",
            &source_value,
            "提供商默认源语言",
            window,
            cx,
        );
        let target = field(
            "provider-target-language",
            &target_value,
            "提供商默认目标语言",
            window,
            cx,
        );
        let global_source = field(
            "source-language",
            &global_source_value,
            "全局源语言",
            window,
            cx,
        );
        let global_target = field(
            "target-language",
            &global_target_value,
            "全局目标语言",
            window,
            cx,
        );
        let credential = field("credential", "", "新凭证（仅写入，不会回显）", window, cx);
        let context_value = settings
            .as_ref()
            .and_then(|s| s.context_window_override)
            .map(|n| n.to_string())
            .unwrap_or_default();
        let output_value = settings
            .as_ref()
            .map(|s| s.output_token_limit.to_string())
            .unwrap_or_else(|| "2048".into());
        let retention_value = settings
            .as_ref()
            .map(|s| s.history_retention_turns.to_string())
            .unwrap_or_else(|| "20".into());
        let turns_value = settings
            .as_ref()
            .map(|s| s.context_turn_limit.to_string())
            .unwrap_or_else(|| "8".into());
        let prompt_value = settings
            .as_ref()
            .map(|s| s.custom_prompt.as_str())
            .unwrap_or("");
        let context = field(
            "context-window",
            &context_value,
            "上下文覆盖（留空使用检测值）",
            window,
            cx,
        );
        let output = field("output-limit", &output_value, "输出 token 上限", window, cx);
        let retention = field("retention", &retention_value, "历史保留轮数", window, cx);
        let turns = field("context-turns", &turns_value, "上下文轮数", window, cx);
        let model_search = field("model-search", "", "筛选已发现模型", window, cx);
        drop(field);
        let prompt_input = page.textarea(prompt_value, "自定义系统提示", window, cx);
        let prompt = Textarea::new(&prompt_input).h(px(120.));
        let model_search_value = page.field("model-search", cx).to_lowercase();
        let deep_l = page.draft.kind == "deepL";
        let capabilities = page
            .providers
            .iter()
            .find(|p| {
                Some(p.id.as_str()) == page.selected_id.as_deref() && p.kind == page.draft.kind
            })
            .map(|p| &p.capabilities);
        let custom_prompt = capabilities
            .map(|capabilities| capabilities.custom_prompt)
            .unwrap_or(!deep_l);
        let model_discovery = capabilities
            .map(|capabilities| capabilities.model_discovery)
            .unwrap_or(!deep_l);
        let context_discovery = capabilities
            .map(|capabilities| capabilities.context_discovery)
            .unwrap_or(matches!(
                page.draft.kind.as_str(),
                "ollama" | "anthropic" | "gemini"
            ));
        let model_info_available =
            crate::translation::can_inspect_model(context_discovery, model_discovery);
        let capture_active = self.capture.active_game.is_some();
        let probes_blocked = capture_active || page.provider_probe_pending();
        let source_history = page
            .settings
            .as_ref()
            .is_some_and(|s| s.include_source_history);
        let translated_history = page
            .settings
            .as_ref()
            .is_some_and(|s| s.include_translated_history);
        let remote_enabled = page.remote_enabled;
        let storage = match page.credential_storage.as_str() {
            "persistentKeychain" => "macOS 钥匙串".to_owned(),
            "sessionOnly" => "仅当前会话内存（重启后清除）".to_owned(),
            "" => "正在读取".to_owned(),
            other => other.to_owned(),
        };
        let has_credential = page
            .providers
            .iter()
            .find(|p| Some(p.id.as_str()) == page.selected_id.as_deref())
            .is_some_and(|p| p.has_credential);
        let token_field = page.draft.token_limit_field.clone();
        let models = page
            .models
            .iter()
            .filter(|model| {
                model.id.to_lowercase().contains(&model_search_value)
                    || model
                        .display_name
                        .to_lowercase()
                        .contains(&model_search_value)
            })
            .map(|model| {
                let model = model.clone();
                let id = model.id.clone();
                Button::new(format!("translation-model:{id}"))
                    .label(id.clone())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(input) = this.translation_page.inputs.get("model") {
                            input.update(cx, |state, cx| state.set_value(id.clone(), window, cx));
                        }
                        this.translation_page.choose_model(&model);
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let loading = !page.pending.is_empty();
        let context_label = page.context.as_ref().map(|(container, game_id, title)| {
            format!("当前游戏：{title}（{game_id} · {container}）")
        });
        let error = page.error.clone();
        let message = page.message.clone();
        let info = page.model_info.clone().map(|m| match m.context_window {
            Some(n) => format!(
                "模型上下文：{n} tokens（{}）",
                provenance_label(&m.context_window_provenance)
            ),
            None => "模型上下文未知；不会根据模型名称推断。".into(),
        });
        let content =
            col()
                .w_full()
                .gap(px(10.))
                .p(px(22.))
                .child(
                    row()
                        .gap(px(8.))
                        .child(Button::new("translation-back").label("返回设置").on_click(
                            cx.listener(|this, _, window, cx| {
                                this.translation_page.close(window, cx);
                                cx.notify();
                            }),
                        ))
                        .child("游戏翻译"),
                )
                .when_some(context_label, |d, text| d.child(muted(text)))
                .child(
                    row()
                        .gap(px(6.))
                        .flex_wrap()
                        .children(profiles)
                        .children(add),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .flex_wrap()
                        .child("名称")
                        .child(name)
                        .child("类型")
                        .children(kind_buttons),
                )
                .child(row().gap(px(8.)).child("端点").child(endpoint))
                .child(
                    row()
                        .gap(px(8.))
                        .child("模型")
                        .child(model.disabled(deep_l))
                        .child(
                            Button::new("translation-discover")
                                .label("发现模型")
                                .disabled(!selected || !model_discovery || probes_blocked)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.discover(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("translation-model-info")
                                .label("模型信息")
                                .disabled(!selected || !model_info_available || probes_blocked)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.inspect_model(&this.bridge, cx);
                                    cx.notify();
                                })),
                        ),
                )
                .when(capture_active, |d| {
                    d.child(muted("请先停止当前捕获，再执行提供商网络探测。"))
                })
                .when_some(info, |d, text| d.child(muted(text)))
                .child(model_search)
                .child(
                    div()
                        .id("translation-model-list")
                        .max_h(px(180.))
                        .overflow_y_scroll()
                        .child(col().gap(px(4.)).children(models)),
                )
                .when(page.draft.kind == "openAIChat", |d| {
                    d.child(
                        row()
                            .gap(px(6.))
                            .child(muted(format!("OpenAI Chat token 参数：{token_field}")))
                            .child(
                                Button::new("translation-max-completion")
                                    .label("max_completion_tokens")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.translation_page
                                            .set_token_field("max_completion_tokens");
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("translation-max-tokens")
                                    .label("max_tokens")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.translation_page.set_token_field("max_tokens");
                                        cx.notify();
                                    })),
                            ),
                    )
                })
                .child(
                    row()
                        .gap(px(8.))
                        .child("提供商默认源语言")
                        .child(source)
                        .child("提供商默认目标语言")
                        .child(target),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child(
                            Button::new("translation-save-provider")
                                .label("保存提供商")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.save_provider(&this.bridge, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("translation-test")
                                .label("测试连接")
                                .disabled(!selected || probes_blocked)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.test_connection(&this.bridge);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("translation-delete")
                                .label("删除")
                                .disabled(!selected)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.delete_provider(&this.bridge);
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child("API 凭证")
                        .child(credential)
                        .child(
                            Button::new("translation-credential-save")
                                .label("安全保存凭证")
                                .disabled(!selected)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.translation_page
                                        .save_credential(&this.bridge, window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("translation-credential-remove")
                                .label("移除凭证")
                                .disabled(!selected || !has_credential)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.remove_credential(&this.bridge);
                                    cx.notify();
                                })),
                        ),
                )
                .child(muted(if has_credential {
                    "已存有凭证；输入仅用于替换，内容不会回显。"
                } else {
                    "凭证尚未设置；凭证输入为掩码且只写入平台凭证存储。"
                }))
                .child(
                    row()
                        .gap(px(8.))
                        .child("上下文覆盖")
                        .child(context)
                        .child("输出上限")
                        .child(output),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child("保留历史轮数")
                        .child(retention)
                        .child("上下文轮数")
                        .child(turns),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child("源文本历史")
                        .child(
                            Switch::new("translation-source-history")
                                .checked(source_history)
                                .on_change(cx.listener(|this, _, _, cx| {
                                    this.translation_page.toggle_source_history();
                                    cx.notify();
                                })),
                        )
                        .child("译文历史")
                        .child(
                            Switch::new("translation-translated-history")
                                .checked(translated_history)
                                .on_change(cx.listener(|this, _, _, cx| {
                                    this.translation_page.toggle_translated_history();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child("允许远程提供商")
                        .child(
                            Switch::new("translation-remote")
                                .checked(remote_enabled)
                                .on_change(cx.listener(|this, _, _, cx| {
                                    this.translation_page.toggle_remote_enabled();
                                    cx.notify();
                                })),
                        )
                        .child(muted(format!(
                            "凭证存储：{storage}；本机环回 API 可在不启用远程提供商时使用。"
                        ))),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child("全局源语言")
                        .child(global_source)
                        .child("全局目标语言")
                        .child(global_target),
                )
                .child(
                    row()
                        .gap(px(8.))
                        .child("自定义系统提示")
                        .child(prompt.disabled(deep_l || !custom_prompt))
                        .when(deep_l || !custom_prompt, |d| {
                            d.child(muted(
                                "DeepL 不使用模型 ID，不支持模型发现、上下文元数据或自定义提示。",
                            ))
                        }),
                )
                .child(
                    Button::new("translation-save-settings")
                        .label("保存翻译设置")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.translation_page.save_settings(&this.bridge, cx);
                            cx.notify();
                        })),
                )
                .when(loading, |d| d.child(muted("正在请求翻译服务…")))
                .when_some(error, |d, e| d.child(muted(format!("错误：{e}"))))
                .when_some(message, |d, m| d.child(muted(m)));
        self.preview_scroll("translation-settings-scroll", content)
    }
}

fn provenance_label(value: &str) -> &'static str {
    match value {
        "detected" => "已检测",
        "manual" => "手动指定",
        _ => "未知",
    }
}
