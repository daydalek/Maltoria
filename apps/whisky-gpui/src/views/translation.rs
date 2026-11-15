use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, Textarea},
    menu::{DropdownMenu, PopupMenuItem},
    spinner::Spinner,
    switch::Switch,
};
use gpui_kit::{prelude::*, *};

use crate::controls::{col, icon, muted, row};
use crate::state::WhiskyApp;
use crate::theme::*;
use crate::translation::{
    ModelDescriptorDto, ProviderDto, TranslationTab, default_endpoint, provider_kind_label,
};

const PROVIDER_KINDS: [(&str, &str); 6] = [
    ("OpenAI Chat", "openAIChat"),
    ("OpenAI Completions", "openAICompletions"),
    ("Ollama（本地）", "ollama"),
    ("DeepL", "deepL"),
    ("Anthropic", "anthropic"),
    ("Gemini", "gemini"),
];

const LANGUAGE_PRESETS: [(&str, &str); 6] = [
    ("auto", "自动检测 (auto)"),
    ("ja", "日本語 (ja)"),
    ("en", "English (en)"),
    ("zh-Hans", "简体中文 (zh-Hans)"),
    ("zh-Hant", "繁體中文 (zh-Hant)"),
    ("ko", "한국어 (ko)"),
];

impl WhiskyApp {
    pub(crate) fn translation_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let header = self.translation_header(cx);
        let banner = self.translation_status_banner();
        let tab = self.translation_page.tab;
        let body = match tab {
            TranslationTab::Providers => self.translation_providers_tab(window, cx),
            TranslationTab::Settings => self.translation_settings_tab(window, cx),
        };

        let content = col()
            .w_full()
            .gap(px(16.))
            .p(px(24.))
            .child(header)
            .when_some(banner, |d, b| d.child(b))
            .child(body);

        self.preview_scroll("translation-settings-scroll", content)
    }

    fn translation_header(&self, cx: &Context<Self>) -> AnyElement {
        let page = &self.translation_page;
        let back_label = if page.return_to_capture {
            "← 返回游戏捕获"
        } else {
            "← 返回设置"
        };
        let context_badge = page.context.as_ref().map(|(container, game_id, title)| {
            pill_badge(
                format!("{title} · {game_id}（{container}）"),
                badge_bg(),
                badge_text(),
                None,
            )
        });
        let tab = page.tab;

        let mut providers_btn = Button::new("translation-tab-providers").label("提供商配置");
        providers_btn = if tab == TranslationTab::Providers {
            providers_btn.primary()
        } else {
            providers_btn.ghost()
        };
        let mut settings_btn = Button::new("translation-tab-settings").label("全局翻译设置");
        settings_btn = if tab == TranslationTab::Settings {
            settings_btn.primary()
        } else {
            settings_btn.ghost()
        };

        row()
            .justify_between()
            .gap(px(12.))
            .flex_wrap()
            .child(
                row()
                    .gap(px(10.))
                    .flex_wrap()
                    .child(
                        Button::new("translation-back")
                            .ghost()
                            .label(back_label)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_translation_settings(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(text()))
                            .child("游戏翻译"),
                    )
                    .when_some(context_badge, |d, badge| d.child(badge)),
            )
            .child(
                row()
                    .gap(px(4.))
                    .p(px(3.))
                    .rounded(px(8.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(providers_btn.on_click(cx.listener(|this, _, _, cx| {
                        this.translation_page.tab = TranslationTab::Providers;
                        cx.notify();
                    })))
                    .child(settings_btn.on_click(cx.listener(|this, _, _, cx| {
                        this.translation_page.tab = TranslationTab::Settings;
                        cx.notify();
                    }))),
            )
            .into_any_element()
    }

    fn translation_status_banner(&self) -> Option<AnyElement> {
        let page = &self.translation_page;
        let loading = !page.pending.is_empty();
        if !loading && page.error.is_none() && page.message.is_none() {
            return None;
        }

        let mut items: Vec<AnyElement> = Vec::new();
        if loading {
            items.push(
                row()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(Spinner::new().with_size(px(13.)))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(subtext()))
                            .child("正在请求翻译服务…"),
                    )
                    .into_any_element(),
            );
        }
        if let Some(err) = &page.error {
            items.push(
                row()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(9.))
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
                            .child(err.clone()),
                    )
                    .into_any_element(),
            );
        }
        if let Some(msg) = &page.message {
            items.push(
                row()
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(9.))
                    .rounded(px(8.))
                    .bg(rgba(installed_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(icon("check", 14.).text_color(rgb(installed_fg())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(rgb(installed_fg()))
                            .child(msg.clone()),
                    )
                    .into_any_element(),
            );
        }

        Some(col().gap(px(6.)).children(items).into_any_element())
    }

    fn translation_providers_tab(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sidebar = self.translation_provider_sidebar(cx);
        let editor = self.translation_provider_editor(window, cx);

        row()
            .items_start()
            .gap(px(18.))
            .child(sidebar)
            .child(div().flex_1().min_w_0().child(editor))
            .into_any_element()
    }

    fn translation_provider_sidebar(&self, cx: &Context<Self>) -> AnyElement {
        let page = &self.translation_page;
        let view = cx.entity().downgrade();
        let new_button = Button::new("translation-new-provider")
            .primary()
            .label("+ 新建 ⌄")
            .dropdown_menu(move |mut menu, _, _| {
                for (label, kind) in PROVIDER_KINDS {
                    let view = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.translation_page.new_provider(kind);
                            cx.notify();
                        });
                    }));
                }
                menu
            });

        let creating_new = page.selected_id.is_none() && !page.draft.kind.is_empty();
        let mut cards: Vec<AnyElement> = Vec::new();
        if creating_new {
            let kind_label = provider_kind_label(&page.draft.kind).to_owned();
            cards.push(
                col()
                    .id("translation-provider-draft")
                    .gap(px(5.))
                    .p(px(10.))
                    .rounded(px(9.))
                    .bg(rgba(accent_subtle_bg()))
                    .border_1()
                    .border_color(rgba(accent_subtle_border()))
                    .child(
                        row()
                            .justify_between()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(13.))
                                    .text_color(rgb(text()))
                                    .child("新建提供商（未保存）"),
                            )
                            .child(pill_badge("草稿", accent_subtle_bg(), accent_fg(), None)),
                    )
                    .child(muted(kind_label).text_size(px(11.)))
                    .into_any_element(),
            );
        }

        for p in &page.providers {
            cards.push(provider_sidebar_card(
                p,
                page.selected_id.as_deref() == Some(p.id.as_str()),
                cx,
            ));
        }

        if cards.is_empty() {
            cards.push(
                col()
                    .gap(px(4.))
                    .p(px(12.))
                    .rounded(px(9.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(subtext()))
                            .child("暂无已保存提供商"),
                    )
                    .child(muted("点击右上角「+ 新建」开始配置。").text_size(px(11.)))
                    .into_any_element(),
            );
        }

        col()
            .w(px(236.))
            .flex_shrink_0()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(10.))
            .bg(rgba(section_bg()))
            .border_1()
            .border_color(rgba(section_border()))
            .child(
                row()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(13.))
                            .text_color(rgb(text()))
                            .child("已配置提供商"),
                    )
                    .child(new_button),
            )
            .child(col().gap(px(6.)).children(cards))
            .into_any_element()
    }

    fn translation_provider_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.translation_page.selected_id.is_none()
            && self.translation_page.draft.kind.is_empty()
        {
            return self.translation_empty_editor(cx);
        }

        let selected = self.translation_page.selected_id.is_some();
        let kind = self.translation_page.draft.kind.clone();
        let kind_label = provider_kind_label(&kind).to_owned();
        let deep_l = kind == "deepL";
        let capture_active = self.capture.active_game.is_some();
        let probes_blocked = capture_active || self.translation_page.provider_probe_pending();
        let capabilities = self
            .translation_page
            .providers
            .iter()
            .find(|p| {
                Some(p.id.as_str()) == self.translation_page.selected_id.as_deref()
                    && p.kind == kind
            })
            .map(|p| p.capabilities.clone());
        let model_discovery = capabilities
            .as_ref()
            .map(|c| c.model_discovery)
            .unwrap_or(!deep_l);
        let context_discovery = capabilities
            .as_ref()
            .map(|c| c.context_discovery)
            .unwrap_or(matches!(kind.as_str(), "ollama" | "anthropic" | "gemini"));
        let model_info_available =
            crate::translation::can_inspect_model(context_discovery, model_discovery);
        let has_credential = self
            .translation_page
            .providers
            .iter()
            .find(|p| Some(p.id.as_str()) == self.translation_page.selected_id.as_deref())
            .is_some_and(|p| p.has_credential);
        let token_field = self.translation_page.draft.token_limit_field.clone();

        let title = if selected {
            let draft_name = self.translation_page.draft.name.trim();
            if draft_name.is_empty() {
                format!("编辑：{kind_label}")
            } else {
                draft_name.to_owned()
            }
        } else {
            format!("新建：{kind_label}")
        };

        let name_value = self.translation_page.draft.name.clone();
        let endpoint_value = self.translation_page.draft.base_url.clone();
        let model_value = self.translation_page.draft.model_id.clone();
        let source_value = self.translation_page.draft.source_language.clone();
        let target_value = self.translation_page.draft.target_language.clone();

        let name_input = Input::new(&self.translation_page.input(
            "name",
            &name_value,
            "例如：本地 Ollama / OpenAI 主力",
            window,
            cx,
        ));
        let endpoint_input = Input::new(&self.translation_page.input(
            "endpoint",
            &endpoint_value,
            default_endpoint(&kind),
            window,
            cx,
        ));
        let model_input = Input::new(
            &self
                .translation_page
                .input("model", &model_value, "模型 ID（支持手动输入）", window, cx),
        );
        let model_search_input = Input::new(
            &self
                .translation_page
                .input("model-search", "", "筛选已发现模型…", window, cx),
        )
        .prefix(icon("search", 13.).text_color(rgb(muted_color())))
        .cleanable(true);
        let credential_input = Input::new(&self.translation_page.input(
            "credential",
            "",
            if has_credential {
                "输入新密钥以替换（不会回显）"
            } else {
                "输入 API 密钥（掩码存储，不会回显）"
            },
            window,
            cx,
        ));

        let model_search_value = self
            .translation_page
            .field("model-search", cx)
            .to_lowercase();
        let active_model_id = {
            let from_input = self.translation_page.field("model", cx);
            if from_input.is_empty() {
                model_value.clone()
            } else {
                from_input
            }
        };
        let filtered_models: Vec<ModelDescriptorDto> = self
            .translation_page
            .models
            .iter()
            .filter(|m| {
                m.id.to_lowercase().contains(&model_search_value)
                    || m.display_name.to_lowercase().contains(&model_search_value)
            })
            .cloned()
            .collect();
        let has_models = !self.translation_page.models.is_empty();
        let context_badge = self
            .translation_page
            .model_info
            .clone()
            .or_else(|| self.translation_page.draft.model_metadata.clone())
            .map(|m| match m.context_window {
                Some(n) => pill_badge(
                    format!(
                        "上下文：{n} tokens（{}）",
                        provenance_label(&m.context_window_provenance)
                    ),
                    installed_bg(),
                    installed_fg(),
                    None,
                ),
                None => pill_badge(
                    "模型上下文未知；不会根据模型名称推断",
                    badge_bg(),
                    badge_text(),
                    None,
                ),
            });

        let source_field = language_preset_field(
            self,
            "provider-source-language",
            "默认源语言",
            &source_value,
            "例如：auto / ja / en",
            true,
            window,
            cx,
        );
        let target_field = language_preset_field(
            self,
            "provider-target-language",
            "默认目标语言",
            &target_value,
            "例如：zh-Hans / en",
            false,
            window,
            cx,
        );

        let kind_chips: Vec<AnyElement> = PROVIDER_KINDS
            .into_iter()
            .map(|(label, k)| {
                let active = kind == k;
                let k_owned = k.to_owned();
                selectable_chip(
                    format!("translation-kind:{k}"),
                    label,
                    active,
                    cx.listener(move |this, _, window, cx| {
                        this.translation_page.switch_kind(&k_owned, window, cx);
                        cx.notify();
                    }),
                )
            })
            .collect();

        // Editor Title Row
        let title_row = row()
            .justify_between()
            .gap(px(12.))
            .flex_wrap()
            .child(
                row()
                    .gap(px(8.))
                    .flex_wrap()
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(text()))
                            .child(title),
                    )
                    .child(if selected {
                        pill_badge("已保存", installed_bg(), installed_fg(), None)
                    } else {
                        pill_badge("草稿未保存", accent_subtle_bg(), accent_fg(), None)
                    }),
            )
            .child(
                row()
                    .gap(px(8.))
                    .child(
                        Button::new("translation-test")
                            .label("测试连接")
                            .disabled(!selected || probes_blocked)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.test_connection(&this.bridge);
                                cx.notify();
                            })),
                    )
                    .when(selected, |d| {
                        d.child(
                            Button::new("translation-delete")
                                .danger()
                                .label("删除")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.delete_provider(&this.bridge);
                                    cx.notify();
                                })),
                        )
                    }),
            );

        // Card 1 — 基本信息与接口
        let basic_card = section_card(
            "基本信息与接口",
            Some("选择提供商协议类型，并填写名称与 API 接口地址。"),
            col()
                .gap(px(10.))
                .child(labeled_field(
                    "提供商类型",
                    row().gap(px(6.)).flex_wrap().children(kind_chips),
                ))
                .child(labeled_field("名称", name_input))
                .child(labeled_field(
                    "API 端点",
                    col()
                        .gap(px(4.))
                        .child(endpoint_input)
                        .child(
                            muted(format!("默认端点：{}", default_endpoint(&kind)))
                                .text_size(px(11.)),
                        ),
                )),
        );

        // Card 2 — 模型配置
        let model_card_body = if deep_l {
            row()
                .gap(px(8.))
                .p(px(10.))
                .rounded(px(8.))
                .bg(rgba(card_bg()))
                .border_1()
                .border_color(rgba(card_border()))
                .child(icon("info", 14.).text_color(rgb(muted_color())))
                .child(
                    muted("DeepL 为专用机器翻译引擎，不使用模型 ID，无需配置或发现模型。")
                        .line_height(relative(1.4)),
                )
                .into_any_element()
        } else {
            let model_rows: Vec<AnyElement> = filtered_models
                .iter()
                .map(|m| {
                    let is_active = m.id == active_model_id;
                    let model_clone = m.clone();
                    let id = m.id.clone();
                    let id_for_click = id.clone();
                    let display = (!m.display_name.is_empty() && m.display_name != m.id)
                        .then(|| m.display_name.clone());
                    let ctx_label = m.context_window.map(format_context_window);

                    row()
                        .id(format!("translation-model:{id}"))
                        .justify_between()
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(7.))
                        .cursor_pointer()
                        .bg(rgba(if is_active {
                            accent_subtle_bg()
                        } else {
                            card_bg()
                        }))
                        .border_1()
                        .border_color(rgba(if is_active {
                            accent_subtle_border()
                        } else {
                            card_border()
                        }))
                        .hover(|s| {
                            if is_active {
                                s
                            } else {
                                s.bg(rgba(hover()))
                            }
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if let Some(input) = this.translation_page.inputs.get("model") {
                                input.update(cx, |state, cx| {
                                    state.set_value(id_for_click.clone(), window, cx)
                                });
                            }
                            this.translation_page.choose_model(&model_clone);
                            cx.notify();
                        }))
                        .child(
                            row()
                                .gap(px(6.))
                                .flex_1()
                                .min_w_0()
                                .when(is_active, |d| {
                                    d.child(icon("check", 12.).text_color(rgb(accent_fg())))
                                })
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(12.))
                                        .font_weight(if is_active {
                                            FontWeight::SEMIBOLD
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .text_color(rgb(if is_active {
                                            accent_fg()
                                        } else {
                                            text()
                                        }))
                                        .child(id),
                                )
                                .when_some(display, |d, name| {
                                    d.child(muted(name).text_size(px(11.)).truncate())
                                }),
                        )
                        .when_some(ctx_label, |d, label| {
                            d.child(pill_badge(label, badge_bg(), badge_text(), None))
                        })
                        .into_any_element()
                })
                .collect();

            let probe_hint = if capture_active {
                Some("请先停止当前游戏捕获，再执行模型发现或连接探测。")
            } else if !selected {
                Some("先保存提供商草稿，再执行模型发现或查看模型信息。")
            } else {
                None
            };

            col()
                .gap(px(10.))
                .child(labeled_field(
                    "模型 ID",
                    row()
                        .gap(px(8.))
                        .child(div().flex_1().min_w_0().child(model_input))
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
                ))
                .when_some(probe_hint, |d, hint| {
                    d.child(muted(hint).text_size(px(11.)))
                })
                .when_some(context_badge, |d, badge| d.child(row().child(badge)))
                .when(has_models, |d| {
                    d.child(
                        col()
                            .gap(px(8.))
                            .p(px(10.))
                            .rounded(px(8.))
                            .bg(rgba(card_bg_subtle()))
                            .border_1()
                            .border_color(rgba(card_border()))
                            .child(model_search_input)
                            .child(
                                div()
                                    .id("translation-model-list")
                                    .max_h(px(180.))
                                    .overflow_y_scroll()
                                    .child(if model_rows.is_empty() {
                                        muted("没有匹配筛选条件的模型。").into_any_element()
                                    } else {
                                        col().gap(px(4.)).children(model_rows).into_any_element()
                                    }),
                            ),
                    )
                })
                .when(kind == "openAIChat", |d| {
                    d.child(divider_line()).child(labeled_field(
                        "Token 参数",
                        row()
                            .gap(px(6.))
                            .flex_wrap()
                            .child(selectable_chip(
                                "translation-max-completion",
                                "max_completion_tokens",
                                token_field != "max_tokens",
                                cx.listener(|this, _, _, cx| {
                                    this.translation_page
                                        .set_token_field("max_completion_tokens");
                                    cx.notify();
                                }),
                            ))
                            .child(selectable_chip(
                                "translation-max-tokens",
                                "max_tokens",
                                token_field == "max_tokens",
                                cx.listener(|this, _, _, cx| {
                                    this.translation_page.set_token_field("max_tokens");
                                    cx.notify();
                                }),
                            )),
                    ))
                })
                .into_any_element()
        };
        let model_card = section_card("模型配置", None, model_card_body);

        // Card 3 — API 凭证与默认语言
        let cred_status = if has_credential {
            pill_badge("✓ 已安全保存", installed_bg(), installed_fg(), None)
        } else if kind == "ollama" {
            pill_badge("本地通常免密钥", badge_bg(), badge_text(), None)
        } else {
            pill_badge("尚未设置", badge_bg(), warning_fg(), None)
        };

        let cred_card = section_card(
            "API 凭证与默认语言",
            None,
            col()
                .gap(px(12.))
                .child(labeled_field(
                    "凭证状态",
                    row().gap(px(8.)).child(cred_status),
                ))
                .child(labeled_field(
                    "API 凭证",
                    col()
                        .gap(px(6.))
                        .child(
                            row()
                                .gap(px(8.))
                                .child(div().flex_1().min_w_0().child(credential_input))
                                .child(
                                    Button::new("translation-credential-save")
                                        .label("仅保存凭证")
                                        .disabled(!selected)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.translation_page
                                                .save_credential(&this.bridge, window, cx);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("translation-credential-remove")
                                        .danger()
                                        .label("移除凭证")
                                        .disabled(!selected || !has_credential)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.translation_page.remove_credential(&this.bridge);
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            muted("输入新密钥后直接点击底部「保存提供商」，即可同时保存配置与凭证。")
                                .text_size(px(11.)),
                        ),
                ))
                .child(divider_line())
                .child(
                    row()
                        .gap(px(12.))
                        .items_start()
                        .child(div().flex_1().min_w_0().child(source_field))
                        .child(div().flex_1().min_w_0().child(target_field)),
                ),
        );

        // Bottom Action Bar
        let action_bar = row()
            .justify_between()
            .gap(px(12.))
            .flex_wrap()
            .child(
                row()
                    .gap(px(8.))
                    .child(
                        Button::new("translation-save-provider")
                            .primary()
                            .label("保存提供商")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.translation_page
                                    .save_provider(&this.bridge, window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("translation-test-bottom")
                            .label("测试连接")
                            .disabled(!selected || probes_blocked)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.test_connection(&this.bridge);
                                cx.notify();
                            })),
                    ),
            )
            .when(!selected, |d| {
                d.child(muted("当前为未保存草稿；保存后即可测试连接或发现模型。"))
            });

        col()
            .gap(px(14.))
            .child(title_row)
            .child(basic_card)
            .child(model_card)
            .child(cred_card)
            .child(action_bar)
            .into_any_element()
    }

    fn translation_empty_editor(&self, cx: &Context<Self>) -> AnyElement {
        let quick_buttons: Vec<AnyElement> = PROVIDER_KINDS
            .into_iter()
            .map(|(label, kind)| {
                Button::new(format!("translation-add:{kind}"))
                    .label(format!("+ {label}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.translation_page.new_provider(kind);
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();

        section_card(
            "选择或新建翻译提供商",
            Some("从左侧列表选择已配置的服务，或点击下方预设快速创建。"),
            col()
                .gap(px(12.))
                .child(row().gap(px(8.)).flex_wrap().children(quick_buttons)),
        )
        .into_any_element()
    }

    fn translation_settings_tab(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let settings = self.translation_page.settings.clone();
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
            .map(|s| s.custom_prompt.clone())
            .unwrap_or_default();
        let source_history = settings.as_ref().is_some_and(|s| s.include_source_history);
        let translated_history = settings
            .as_ref()
            .is_some_and(|s| s.include_translated_history);
        let remote_enabled = self.translation_page.remote_enabled;
        let storage = match self.translation_page.credential_storage.as_str() {
            "persistentKeychain" => "macOS 钥匙串".to_owned(),
            "sessionOnly" => "仅当前会话内存（重启后清除）".to_owned(),
            "" => "正在读取".to_owned(),
            other => other.to_owned(),
        };

        let global_source_field = language_preset_field(
            self,
            "source-language",
            "全局源语言",
            &global_source_value,
            "例如：auto / ja / en",
            true,
            window,
            cx,
        );
        let global_target_field = language_preset_field(
            self,
            "target-language",
            "全局目标语言",
            &global_target_value,
            "例如：zh-Hans / en",
            false,
            window,
            cx,
        );

        let output_input = Input::new(&self.translation_page.input(
            "output-limit",
            &output_value,
            "1 - 32768",
            window,
            cx,
        ));
        let context_input = Input::new(&self.translation_page.input(
            "context-window",
            &context_value,
            "留空使用检测值",
            window,
            cx,
        ));
        let turns_input = Input::new(&self.translation_page.input(
            "context-turns",
            &turns_value,
            "0 - 1000",
            window,
            cx,
        ));
        let retention_input = Input::new(&self.translation_page.input(
            "retention",
            &retention_value,
            "0 - 1000",
            window,
            cx,
        ));
        let prompt_entity = self
            .translation_page
            .textarea(&prompt_value, "自定义系统提示词（留空使用默认翻译指令）", window, cx);
        let prompt_area = Textarea::new(&prompt_entity).h(px(128.));

        let languages_card = section_card(
            "翻译语言",
            Some("当提供商未指定默认语言时，使用此处的全局源语言与目标语言。"),
            row()
                .gap(px(12.))
                .items_start()
                .child(div().flex_1().min_w_0().child(global_source_field))
                .child(div().flex_1().min_w_0().child(global_target_field)),
        );

        let budget_card = section_card(
            "上下文与 Token 预算",
            None,
            col()
                .gap(px(12.))
                .child(
                    row()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(labeled_field("输出 Token 上限", output_input)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(labeled_field("上下文窗口覆盖", context_input)),
                        ),
                )
                .child(
                    row()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(labeled_field("上下文参考轮数", turns_input)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(labeled_field("历史保留轮数", retention_input)),
                        ),
                )
                .child(divider_line())
                .child(switch_row(
                    "translation-source-history",
                    "携带源文历史",
                    "在翻译请求中附上最近几轮的原文，有助于保持人称和语境一致。",
                    source_history,
                    cx.listener(|this, _, _, cx| {
                        this.translation_page.toggle_source_history();
                        cx.notify();
                    }),
                ))
                .child(switch_row(
                    "translation-translated-history",
                    "携带译文历史",
                    "附上近期已翻出的译文，有助于保持专有名词与口吻前后统一。",
                    translated_history,
                    cx.listener(|this, _, _, cx| {
                        this.translation_page.toggle_translated_history();
                        cx.notify();
                    }),
                )),
        );

        let prompt_card = section_card(
            "自定义系统提示词",
            Some("用于指导大语言模型翻译风格、术语表或人称语气（DeepL 会自动忽略此提示词）。"),
            prompt_area,
        );

        let security_card = section_card(
            "网络与凭证存储",
            None,
            col()
                .gap(px(12.))
                .child(switch_row(
                    "translation-remote",
                    "允许远程提供商",
                    "关闭后仅允许访问本机环回地址（如本地 Ollama），禁止向远程 API 发送文本。",
                    remote_enabled,
                    cx.listener(|this, _, _, cx| {
                        this.translation_page.toggle_remote_enabled();
                        cx.notify();
                    }),
                ))
                .child(divider_line())
                .child(
                    row()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(rgb(text()))
                                        .child("凭证存储方式"),
                                )
                                .child(muted("密钥仅写入安全存储，接口不会回显明文。").text_size(px(11.))),
                        )
                        .child(pill_badge(storage, badge_bg(), badge_text(), None)),
                ),
        );

        col()
            .gap(px(14.))
            .child(languages_card)
            .child(budget_card)
            .child(prompt_card)
            .child(security_card)
            .child(
                row().child(
                    Button::new("translation-save-settings")
                        .primary()
                        .label("保存翻译设置")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.translation_page.save_settings(&this.bridge, cx);
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }
}

fn provider_sidebar_card(
    provider: &ProviderDto,
    selected: bool,
    cx: &Context<WhiskyApp>,
) -> AnyElement {
    let id = provider.id.clone();
    let kind_label = provider_kind_label(&provider.kind).to_owned();
    let cred_pill = if provider.has_credential {
        pill_badge("已存密钥", installed_bg(), installed_fg(), None)
    } else if provider.kind == "ollama" {
        pill_badge("本地免密钥", badge_bg(), badge_text(), None)
    } else {
        pill_badge("未设密钥", badge_bg(), warning_fg(), None)
    };
    let model = provider
        .model_id
        .as_deref()
        .filter(|m| !m.is_empty())
        .map(str::to_owned);

    col()
        .id(format!("translation-provider:{id}"))
        .gap(px(6.))
        .p(px(10.))
        .rounded(px(9.))
        .cursor_pointer()
        .bg(rgba(if selected {
            accent_subtle_bg()
        } else {
            card_bg()
        }))
        .border_1()
        .border_color(rgba(if selected {
            accent_subtle_border()
        } else {
            card_border()
        }))
        .hover(|s| if selected { s } else { s.bg(rgba(hover())) })
        .on_click(cx.listener(move |this, _, _, cx| {
            this.translation_page.select(Some(id.clone()));
            cx.notify();
        }))
        .child(
            div()
                .truncate()
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(13.))
                .text_color(rgb(if selected { accent_fg() } else { text() }))
                .child(provider.name.clone()),
        )
        .child(
            row()
                .gap(px(5.))
                .flex_wrap()
                .child(pill_badge(kind_label, badge_bg(), badge_text(), None))
                .child(cred_pill),
        )
        .when_some(model, |d, m| {
            d.child(muted(m).text_size(px(11.)).truncate())
        })
        .into_any_element()
}

fn language_preset_field(
    app: &mut WhiskyApp,
    key: &'static str,
    label: &str,
    value: &str,
    placeholder: &str,
    allow_auto: bool,
    window: &mut Window,
    cx: &mut Context<WhiskyApp>,
) -> AnyElement {
    let input = app
        .translation_page
        .input(key, value, placeholder, window, cx);
    let view = cx.entity().downgrade();
    let preset_btn = Button::new(format!("translation-lang-preset:{key}"))
        .label("预设 ⌄")
        .dropdown_menu(move |mut menu, _, _| {
            for (code, display) in LANGUAGE_PRESETS {
                if !allow_auto && code == "auto" {
                    continue;
                }
                let view = view.clone();
                menu = menu.item(PopupMenuItem::new(display).on_click(move |_, window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.translation_page
                            .set_language_preset(key, code, window, cx);
                        cx.notify();
                    });
                }));
            }
            menu
        });

    col()
        .gap(px(6.))
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(subtext()))
                .child(label.to_owned()),
        )
        .child(
            row()
                .gap(px(6.))
                .child(div().flex_1().min_w_0().child(Input::new(&input)))
                .child(preset_btn),
        )
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

fn labeled_field(label: &str, control: impl IntoElement) -> Div {
    row()
        .items_start()
        .gap(px(12.))
        .child(
            div()
                .w(px(88.))
                .pt(px(6.))
                .flex_shrink_0()
                .text_size(px(12.))
                .text_color(rgb(subtext()))
                .child(label.to_owned()),
        )
        .child(div().flex_1().min_w_0().child(control))
}

fn selectable_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    row()
        .id(id)
        .gap(px(5.))
        .px(px(10.))
        .py(px(5.))
        .rounded(px(7.))
        .cursor_pointer()
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
        .hover(|s| if active { s } else { s.bg(rgba(hover())) })
        .on_click(on_click)
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
        )
        .into_any_element()
}

fn switch_row(
    id: &'static str,
    title: &str,
    description: &str,
    checked: bool,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    row()
        .justify_between()
        .gap(px(16.))
        .child(
            col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(rgb(text()))
                        .child(title.to_owned()),
                )
                .child(muted(description.to_owned()).line_height(relative(1.4))),
        )
        .child(Switch::new(id).checked(checked).on_change(on_change))
        .into_any_element()
}

fn pill_badge(
    label: impl Into<SharedString>,
    bg_rgba: u32,
    fg_rgb: u32,
    border_rgba: Option<u32>,
) -> Div {
    let mut d = row()
        .flex_shrink_0()
        .px(px(8.))
        .py(px(2.))
        .rounded_full()
        .bg(rgba(bg_rgba))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(fg_rgb))
        .child(label.into());
    if let Some(border) = border_rgba {
        d = d.border_1().border_color(rgba(border));
    }
    d
}

fn divider_line() -> Div {
    div().w_full().h(px(1.)).bg(rgba(divider()))
}

fn format_context_window(tokens: u64) -> String {
    if tokens >= 1_000_000 && tokens % 1_000_000 == 0 {
        format!("{}M", tokens / 1_000_000)
    } else if tokens >= 1024 && tokens % 1024 == 0 {
        format!("{}K", tokens / 1024)
    } else if tokens >= 1000 && tokens % 1000 == 0 {
        format!("{}k", tokens / 1000)
    } else {
        format!("{tokens} tokens")
    }
}

fn provenance_label(value: &str) -> &'static str {
    match value {
        "detected" => "已检测",
        "manual" => "手动指定",
        _ => "未知",
    }
}
