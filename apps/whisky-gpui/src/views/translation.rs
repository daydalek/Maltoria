use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants, DropdownButton},
    input::{Input, Textarea},
    menu::{DropdownMenu, PopupMenuItem},
    spinner::Spinner,
    switch::Switch,
    tooltip::Tooltip,
};
use gpui_kit::{prelude::*, *};

use crate::controls::{col, icon, muted, row};
use crate::state::WhiskyApp;
use crate::theme::*;
use crate::translation::{
    CacheEntryDto, LocalModelDto, ModelDescriptorDto, PROVIDER_PRESETS, PROVIDER_SPECS,
    ProviderCategory, ProviderDto, ProviderPreset, ProviderSpec, THINKING_LEVELS, TranslationTab,
    WizardStep, credential_storage_label, default_endpoint, default_sentence_cache_enabled,
    default_translation_debounce_ms, format_bytes_human, format_eta_human, format_speed_human,
    local_download_stage_label, normalize_credential_storage_mode, normalize_thinking_level,
    provider_category, provider_icon, provider_kind_label, provider_short_label, provider_spec,
    supports_thinking_level,
};

const LANGUAGE_PRESETS: [(&str, &str); 6] = [
    ("auto", "自动检测 (auto)"),
    ("ja", "日本語 (ja)"),
    ("en", "English (en)"),
    ("zh-Hans", "简体中文 (zh-Hans)"),
    ("zh-Hant", "繁體中文 (zh-Hant)"),
    ("ko", "한국어 (ko)"),
];

const CREDENTIAL_STORAGE_MODES: [(&str, &str); 3] = [
    ("keychain", SYSTEM_CREDENTIAL_LABEL),
    ("localFile", "本地文件（明文，免弹窗）"),
    ("sessionOnly", "仅本次运行（不落盘）"),
];

#[cfg(target_os = "linux")]
const SYSTEM_CREDENTIAL_LABEL: &str = "系统密钥环（Secret Service，加密）";
#[cfg(not(target_os = "linux"))]
const SYSTEM_CREDENTIAL_LABEL: &str = "macOS 钥匙串（加密）";

const DEBOUNCE_PRESETS: [(u64, &str); 5] = [
    (0, "0 ms（即时发送）"),
    (150, "150 ms（推荐）"),
    (300, "300 ms（适中）"),
    (500, "500 ms（防连跳）"),
    (1000, "1000 ms（长句等待）"),
];

impl WhiskyApp {
    fn spec_available(&self, spec: &ProviderSpec) -> bool {
        spec.required_service
            .is_none_or(|svc| self.service_available(svc))
    }

    fn preset_available(&self, preset: &ProviderPreset) -> bool {
        preset
            .required_service
            .is_none_or(|svc| self.service_available(svc))
    }

    fn category_has_available_specs(&self, cat: ProviderCategory) -> bool {
        PROVIDER_SPECS
            .iter()
            .any(|spec| spec.category == cat && self.spec_available(spec))
    }

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
        let context_badge = page.context.as_ref().map(|(container, game_id, title)| {
            pill_badge(
                format!("{title} · {game_id}（{container}）"),
                badge_bg(),
                badge_text(),
                None,
            )
        });
        let tab = page.tab;

        let mut summary_pills: Vec<AnyElement> = Vec::new();
        if tab == TranslationTab::Providers {
            for cat in ProviderCategory::ALL {
                let count = page
                    .providers
                    .iter()
                    .filter(|p| provider_category(&p.kind) == cat)
                    .count();
                let available = self.category_has_available_specs(cat);
                let active_filter = page.sidebar_category_filter == Some(cat);
                let (fg, bg, border) = cat.colors();
                let tip = if !available && count == 0 {
                    format!("{}：当前构建未启用", cat.label())
                } else {
                    format!(
                        "{}（{} 个）— {}\n点击筛选左侧列表，再次点击取消筛选",
                        cat.label(),
                        count,
                        cat.tooltip()
                    )
                };

                summary_pills.push(
                    row()
                        .id(format!("translation-cat-filter:{}", cat.short_label()))
                        .gap(px(5.))
                        .px(px(9.))
                        .py(px(4.))
                        .rounded_full()
                        .cursor_pointer()
                        .bg(rgba(if active_filter { bg } else { card_bg() }))
                        .border_1()
                        .border_color(rgba(if active_filter {
                            border
                        } else {
                            card_border()
                        }))
                        .when(!available && count == 0, |d| d.opacity(0.65))
                        .hover(|s| s.bg(rgba(bg)))
                        .tooltip(hover_tip(tip))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.translation_page.sidebar_category_filter == Some(cat) {
                                this.translation_page.sidebar_category_filter = None;
                            } else {
                                this.translation_page.sidebar_category_filter = Some(cat);
                            }
                            cx.notify();
                        }))
                        .child(icon(cat.icon(), 12.).text_color(rgb(fg)))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(if active_filter {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::MEDIUM
                                })
                                .text_color(rgb(if active_filter { fg } else { text() }))
                                .child(format!("{} {count}", cat.short_label())),
                        )
                        .into_any_element(),
                );
            }
        }

        let mut providers_btn = Button::new("translation-tab-providers")
            .small()
            .icon(icon("layers", 13.))
            .label("提供商");
        providers_btn = if tab == TranslationTab::Providers {
            providers_btn.primary()
        } else {
            providers_btn.ghost()
        };
        let mut settings_btn = Button::new("translation-tab-settings")
            .small()
            .icon(icon("sliders-horizontal", 13.))
            .label("全局设置");
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
                    .gap(px(8.))
                    .flex_wrap()
                    .when_some(context_badge, |d, badge| d.child(badge))
                    .children(summary_pills)
                    .when(tab == TranslationTab::Settings, |d| {
                        d.child(
                            row()
                                .gap(px(6.))
                                .child(icon("sliders-horizontal", 14.).text_color(rgb(accent_fg())))
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_size(px(13.))
                                        .text_color(rgb(text()))
                                        .child("全局翻译行为与安全设置"),
                                ),
                        )
                    }),
            )
            .child(
                row()
                    .gap(px(8.))
                    .flex_wrap()
                    .child(
                        Button::new("translation-open-wizard")
                            .small()
                            .outline()
                            .icon(icon("wand-sparkles", 13.))
                            .label("配置向导")
                            .tooltip("打开三步交互式向导，快速接入本地模型、OAuth 或 API")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.open_wizard(None);
                                this.translation_page
                                    .refresh_local_status_silent(&this.bridge);
                                cx.notify();
                            })),
                    )
                    .child(
                        row()
                            .gap(px(3.))
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
                    ),
            )
            .into_any_element()
    }

    fn translation_status_banner(&self) -> Option<AnyElement> {
        let page = &self.translation_page;
        let loading = page.has_visible_pending();
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
                    .child(icon("circle-check", 14.).text_color(rgb(installed_fg())))
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
        let mut available_specs: Vec<&'static ProviderSpec> = Vec::new();
        for spec in PROVIDER_SPECS {
            if self.spec_available(spec) {
                available_specs.push(spec);
            }
        }

        let new_button = DropdownButton::new("translation-new-provider")
            .primary()
            .small()
            .button(
                Button::new("translation-wizard-add")
                    .icon(icon("plus", 13.))
                    .label("添加")
                    .tooltip("打开配置向导（点击右侧箭头可直接按协议新建）")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.translation_page
                            .open_wizard(this.translation_page.sidebar_category_filter);
                        this.translation_page
                            .refresh_local_status_silent(&this.bridge);
                        cx.notify();
                    })),
            )
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                let mut added_group = false;
                for cat in ProviderCategory::ALL {
                    let group_specs: Vec<&'static ProviderSpec> = available_specs
                        .iter()
                        .copied()
                        .filter(|s| s.category == cat)
                        .collect();
                    if group_specs.is_empty() {
                        continue;
                    }
                    if added_group {
                        menu = menu.separator();
                    }
                    added_group = true;
                    menu = menu.label(cat.label());
                    for spec in group_specs {
                        let view = view.clone();
                        let kind = spec.kind;
                        menu = menu.item(
                            PopupMenuItem::new(spec.short_label)
                                .icon(icon(spec.icon, 14.))
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.translation_page.sidebar_category_filter = None;
                                        this.translation_page.new_provider(kind);
                                        if kind == "localBuiltIn" {
                                            this.translation_page
                                                .refresh_local_status_silent(&this.bridge);
                                        }
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                }
                let wizard_view = view.clone();
                menu.separator().item(
                    PopupMenuItem::new("打开配置向导…")
                        .icon(icon("wand-sparkles", 14.))
                        .on_click(move |_, _, cx| {
                            let _ = wizard_view.update(cx, |this, cx| {
                                this.translation_page.open_wizard(None);
                                this.translation_page
                                    .refresh_local_status_silent(&this.bridge);
                                cx.notify();
                            });
                        }),
                )
            });

        let creating_new = page.selected_id.is_none() && !page.draft.kind.is_empty();
        let mut list_items: Vec<AnyElement> = Vec::new();

        if let Some(filter_cat) = page.sidebar_category_filter {
            let (fg, bg, border) = filter_cat.colors();
            list_items.push(
                row()
                    .justify_between()
                    .gap(px(6.))
                    .px(px(8.))
                    .py(px(5.))
                    .rounded(px(7.))
                    .bg(rgba(bg))
                    .border_1()
                    .border_color(rgba(border))
                    .child(
                        row()
                            .gap(px(5.))
                            .child(icon(filter_cat.icon(), 12.).text_color(rgb(fg)))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(fg))
                                    .child(format!("筛选：{}", filter_cat.label())),
                            ),
                    )
                    .child(
                        Button::new("translation-clear-filter")
                            .xsmall()
                            .ghost()
                            .label("全部")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.sidebar_category_filter = None;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            );
        }

        if creating_new {
            let draft_cat = provider_category(&page.draft.kind);
            let (fg, bg, border) = draft_cat.colors();
            let draft_name = if page.draft.name.trim().is_empty() {
                format!("新建{}", provider_short_label(&page.draft.kind))
            } else {
                page.draft.name.clone()
            };
            list_items.push(
                row()
                    .id("translation-provider-draft")
                    .overflow_hidden()
                    .rounded(px(9.))
                    .bg(rgba(accent_subtle_bg()))
                    .border_1()
                    .border_color(rgba(accent_subtle_border()))
                    .child(div().w(px(3.)).self_stretch().bg(rgb(accent_fg())))
                    .child(
                        row()
                            .flex_1()
                            .min_w_0()
                            .gap(px(8.))
                            .p(px(9.))
                            .child(category_icon_tile_raw(
                                fg,
                                bg,
                                border,
                                provider_icon(&page.draft.kind),
                                28.,
                                14.,
                            ))
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        row()
                                            .justify_between()
                                            .gap(px(4.))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(px(12.5))
                                                    .text_color(rgb(text()))
                                                    .child(draft_name),
                                            )
                                            .child(pill_badge(
                                                "草稿",
                                                accent_subtle_bg(),
                                                accent_fg(),
                                                None,
                                            )),
                                    )
                                    .child(
                                        muted(format!(
                                            "{} · {}",
                                            draft_cat.short_label(),
                                            provider_short_label(&page.draft.kind)
                                        ))
                                        .text_size(px(11.))
                                        .truncate(),
                                    ),
                            ),
                    )
                    .into_any_element(),
            );
        }

        let mut total_shown = 0usize;
        for cat in ProviderCategory::ALL {
            if page.sidebar_category_filter.is_some_and(|f| f != cat) {
                continue;
            }
            let group_providers: Vec<&ProviderDto> = page
                .providers
                .iter()
                .filter(|p| provider_category(&p.kind) == cat)
                .collect();
            if group_providers.is_empty() {
                continue;
            }
            total_shown += group_providers.len();
            let (fg, _, _) = cat.colors();
            list_items.push(
                row()
                    .justify_between()
                    .gap(px(6.))
                    .pt(px(4.))
                    .px(px(2.))
                    .child(
                        row()
                            .gap(px(5.))
                            .child(icon(cat.icon(), 12.).text_color(rgb(fg)))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(fg))
                                    .child(cat.label()),
                            ),
                    )
                    .child(muted(group_providers.len().to_string()).text_size(px(10.5)))
                    .into_any_element(),
            );
            for p in group_providers {
                list_items.push(provider_sidebar_card(
                    p,
                    page.selected_id.as_deref() == Some(p.id.as_str()),
                    cx,
                ));
            }
        }

        if total_shown == 0 && !creating_new {
            let empty_msg = match page.sidebar_category_filter {
                Some(cat) => format!("暂无{}提供商", cat.label()),
                None => "暂无已配置提供商".to_owned(),
            };
            list_items.push(
                col()
                    .gap(px(6.))
                    .p(px(12.))
                    .rounded(px(9.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(subtext()))
                            .child(empty_msg),
                    )
                    .child(muted("点击右上角「+ 添加」启动向导").text_size(px(11.)))
                    .into_any_element(),
            );
        }

        col()
            .w(px(264.))
            .flex_shrink_0()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(11.))
            .bg(rgba(section_bg()))
            .border_1()
            .border_color(rgba(section_border()))
            .child(
                row()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_size(px(13.))
                                    .text_color(rgb(text()))
                                    .child("服务列表"),
                            )
                            .when(!page.providers.is_empty(), |d| {
                                d.child(pill_badge(
                                    page.providers.len().to_string(),
                                    badge_bg(),
                                    badge_text(),
                                    None,
                                ))
                            }),
                    )
                    .child(new_button),
            )
            .child(col().gap(px(6.)).children(list_items))
            .into_any_element()
    }

    fn kind_dropdown(&self, kind: &str, cx: &Context<Self>) -> AnyElement {
        let view = cx.entity().downgrade();
        let active_kind = kind.to_owned();
        let label = provider_short_label(kind).to_owned();
        let icon_name = provider_icon(kind);
        let mut available_specs: Vec<&'static ProviderSpec> = Vec::new();
        for spec in PROVIDER_SPECS {
            if self.spec_available(spec) || spec.kind == kind {
                available_specs.push(spec);
            }
        }

        Button::new("translation-kind-dropdown")
            .outline()
            .w_full()
            .icon(icon(icon_name, 14.))
            .label(label)
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                let mut added_group = false;
                for cat in ProviderCategory::ALL {
                    let group_specs: Vec<&'static ProviderSpec> = available_specs
                        .iter()
                        .copied()
                        .filter(|s| s.category == cat)
                        .collect();
                    if group_specs.is_empty() {
                        continue;
                    }
                    if added_group {
                        menu = menu.separator();
                    }
                    added_group = true;
                    menu = menu.label(cat.label());
                    for spec in group_specs {
                        let view = view.clone();
                        let k = spec.kind;
                        let checked = k == active_kind;
                        menu = menu.item(
                            PopupMenuItem::new(spec.label)
                                .icon(icon(spec.icon, 14.))
                                .checked(checked)
                                .on_click(move |_, window, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.translation_page.switch_kind(k, window, cx);
                                        if k == "localBuiltIn" {
                                            this.translation_page
                                                .refresh_local_status_silent(&this.bridge);
                                        }
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                }
                menu
            })
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
        let category = provider_category(&kind);
        let (cat_fg, cat_bg, cat_border) = category.colors();
        let kind_label = provider_kind_label(&kind).to_owned();
        let spec_tip = provider_spec(&kind)
            .map(|s| s.tip)
            .unwrap_or(category.tooltip());
        let deep_l = kind == "deepL";
        let local_built_in = kind == "localBuiltIn";
        let is_ollama = kind == "ollama";
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
        let thinking_level =
            normalize_thinking_level(&self.translation_page.draft.thinking_level);

        let title = if selected {
            let draft_name = self.translation_page.draft.name.trim();
            if draft_name.is_empty() {
                kind_label.clone()
            } else {
                draft_name.to_owned()
            }
        } else {
            let draft_name = self.translation_page.draft.name.trim();
            if draft_name.is_empty() {
                format!("新建 {}", provider_short_label(&kind))
            } else {
                draft_name.to_owned()
            }
        };

        let name_value = self.translation_page.draft.name.clone();
        let endpoint_value = self.translation_page.draft.base_url.clone();
        let model_value = self.translation_page.draft.model_id.clone();
        let source_value = self.translation_page.draft.source_language.clone();
        let target_value = self.translation_page.draft.target_language.clone();

        let name_input = Input::new(&self.translation_page.input(
            "name",
            &name_value,
            "例如：DeepSeek 主力 / 本地 Ollama",
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
        let model_input = Input::new(&self.translation_page.input(
            "model",
            &model_value,
            "模型 ID（支持手动输入或在线发现）",
            window,
            cx,
        ));
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
                "输入新密钥以替换（掩码保护）"
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
        let current_endpoint = {
            let from_input = self.translation_page.field("endpoint", cx);
            if from_input.is_empty() {
                endpoint_value.clone()
            } else {
                from_input
            }
        };
        let endpoint_is_custom =
            !current_endpoint.trim().is_empty() && current_endpoint.trim() != default_endpoint(&kind);

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
                Some(n) => {
                    let prov = provenance_label(&m.context_window_provenance);
                    pill_badge(
                        format!("{} 上下文", format_context_window(n)),
                        installed_bg(),
                        installed_fg(),
                        None,
                    )
                    .id("model-context-badge")
                    .tooltip(hover_tip(format!(
                        "当前模型上下文窗口：{n} tokens（来源：{prov}）。"
                    )))
                    .into_any_element()
                }
                None => pill_badge("上下文未知", badge_bg(), badge_text(), None)
                    .id("model-context-badge")
                    .tooltip(hover_tip(
                        "尚未探测到该模型的上下文窗口大小，不会根据模型名称猜测。",
                    ))
                    .into_any_element(),
            });

        let source_field = language_preset_field(
            self,
            "provider-source-language",
            "源语言",
            &source_value,
            "auto / ja",
            true,
            window,
            cx,
        );
        let target_field = language_preset_field(
            self,
            "provider-target-language",
            "目标语言",
            &target_value,
            "zh-Hans / en",
            false,
            window,
            cx,
        );

        let test_tooltip = if !selected {
            "请先保存提供商再测试连接"
        } else if capture_active {
            "游戏捕获进行中，请先停止捕获"
        } else if self.translation_page.provider_probe_pending() {
            "正在执行探测请求，请稍候"
        } else {
            "向当前配置发送一次快速探测请求"
        };

        // Hero Header Card
        let hero_subtitle = if active_model_id.trim().is_empty() {
            kind_label.clone()
        } else {
            format!("{kind_label} · {}", active_model_id.trim())
        };
        let hero_card = row()
            .overflow_hidden()
            .rounded(px(11.))
            .bg(rgba(section_bg()))
            .border_1()
            .border_color(rgba(if selected {
                cat_border
            } else {
                accent_subtle_border()
            }))
            .child(div().w(px(4.)).self_stretch().bg(rgb(cat_fg)))
            .child(
                row()
                    .flex_1()
                    .min_w_0()
                    .justify_between()
                    .gap(px(12.))
                    .flex_wrap()
                    .p(px(14.))
                    .child(
                        row()
                            .flex_1()
                            .min_w_0()
                            .gap(px(12.))
                            .child(category_icon_tile_raw(
                                cat_fg,
                                cat_bg,
                                cat_border,
                                provider_icon(&kind),
                                38.,
                                18.,
                            ))
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(4.))
                                    .child(
                                        row()
                                            .gap(px(7.))
                                            .flex_wrap()
                                            .child(
                                                div()
                                                    .text_size(px(15.5))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(text()))
                                                    .truncate()
                                                    .child(title),
                                            )
                                            .child(category_pill(category))
                                            .child(if selected {
                                                pill_badge(
                                                    "已保存",
                                                    installed_bg(),
                                                    installed_fg(),
                                                    None,
                                                )
                                            } else {
                                                pill_badge(
                                                    "草稿",
                                                    accent_subtle_bg(),
                                                    accent_fg(),
                                                    None,
                                                )
                                            })
                                            .child(help_tip("editor-hero-tip", spec_tip)),
                                    )
                                    .child(muted(hero_subtitle).text_size(px(11.5)).truncate()),
                            ),
                    )
                    .child(
                        row()
                            .gap(px(8.))
                            .child(
                                Button::new("translation-test")
                                    .small()
                                    .icon(icon("plug-zap", 13.))
                                    .label("测试连接")
                                    .tooltip(test_tooltip)
                                    .disabled(!selected || probes_blocked)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.translation_page.test_connection(&this.bridge);
                                        cx.notify();
                                    })),
                            )
                            .when(selected, |d| {
                                d.child(
                                    Button::new("translation-delete")
                                        .small()
                                        .danger()
                                        .icon(icon("delete", 14.))
                                        .tooltip("删除此提供商配置")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.translation_page.delete_provider(&this.bridge);
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            );

        // Card 1 — Protocol, Endpoint & Credential
        let kind_selector = self.kind_dropdown(&kind, cx);
        let storage_label = credential_storage_label(&self.translation_page.credential_storage);
        let is_plaintext_storage = self.translation_page.credential_storage == "localPlaintextFile";
        let storage_short = match self.translation_page.credential_storage.as_str() {
            "persistentKeychain" => "钥匙串",
            "persistentSecretService" => "密钥环",
            "localPlaintextFile" => "明文文件",
            "sessionOnly" => "仅会话",
            "unavailable" => "存储不可用",
            _ => "存储",
        };
        let storage_tip = if is_plaintext_storage {
            format!(
                "当前存储方式：{storage_label}。以 0600 权限明文落盘免弹窗；切换存储模式后需在此重新保存密钥。"
            )
        } else {
            format!("当前存储方式：{storage_label}。密钥仅写入安全凭据存储，接口不会回显明文。")
        };
        let storage_badge = row()
            .id("editor-storage-badge")
            .flex_shrink_0()
            .gap(px(4.))
            .px(px(8.))
            .py(px(2.))
            .rounded_full()
            .bg(rgba(badge_bg()))
            .tooltip(hover_tip(storage_tip))
            .child(
                icon(
                    if is_plaintext_storage {
                        "triangle-alert"
                    } else {
                        "lock"
                    },
                    11.,
                )
                .text_color(rgb(if is_plaintext_storage {
                    warning_fg()
                } else {
                    badge_text()
                })),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if is_plaintext_storage {
                        warning_fg()
                    } else {
                        badge_text()
                    }))
                    .child(storage_short),
            );

        let cred_status = if has_credential {
            pill_badge("✓ 已保存", installed_bg(), installed_fg(), None)
        } else if is_ollama {
            pill_badge("本地免密钥", cat_local_bg(), cat_local_fg(), None)
        } else {
            pill_badge("未设置", badge_bg(), warning_fg(), None)
        };
        let cred_hint = if has_credential {
            "留空将保留现有密钥；输入新密钥后点击底部「保存提供商」即可一并更新。"
        } else if is_ollama {
            "本地 Ollama 通常无需密钥；如通过反向代理访问或开启了鉴权，可填写并保存。"
        } else {
            "输入 API 密钥后点击底部「保存提供商」，即可与接口配置一并保存。"
        };

        let basic_card = if local_built_in {
            section_card_v2(
                "hard-drive",
                cat_fg,
                "基本信息",
                Some("local-basic-tip"),
                Some(spec_tip),
                None,
                two_col_row(
                    labeled_stack("名称", name_input),
                    labeled_stack("协议类型", kind_selector),
                ),
            )
        } else if is_ollama {
            let show_cred = has_credential || self.translation_page.ollama_credential_expanded;
            let default_ep = default_endpoint(&kind);
            let cred_block = if show_cred {
                col()
                    .gap(px(6.))
                    .child(
                        row()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                row()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(subtext()))
                                            .child("可选鉴权密钥"),
                                    )
                                    .child(cred_status)
                                    .child(storage_badge)
                                    .child(help_tip("ollama-cred-tip", cred_hint)),
                            )
                            .when(!has_credential, |d| {
                                d.child(
                                    Button::new("ollama-collapse-cred")
                                        .xsmall()
                                        .ghost()
                                        .label("收起")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.translation_page.ollama_credential_expanded =
                                                false;
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .child(
                        row()
                            .gap(px(8.))
                            .child(div().flex_1().min_w_0().child(credential_input))
                            .when(selected && has_credential, |d| {
                                d.child(
                                    Button::new("translation-credential-remove")
                                        .small()
                                        .danger()
                                        .icon(icon("delete", 13.))
                                        .label("移除密钥")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.translation_page.remove_credential(&this.bridge);
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .into_any_element()
            } else {
                row()
                    .justify_between()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(7.))
                    .rounded(px(8.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(
                        row()
                            .gap(px(6.))
                            .child(icon("shield-check", 13.).text_color(rgb(cat_fg)))
                            .child(muted("本机 Ollama 通常免密直连").text_size(px(12.)))
                            .child(help_tip("ollama-cred-tip", cred_hint)),
                    )
                    .child(
                        Button::new("ollama-expand-cred")
                            .xsmall()
                            .ghost()
                            .icon(icon("key-round", 12.))
                            .label("配置可选密钥")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.ollama_credential_expanded = true;
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            };

            section_card_v2(
                "terminal",
                cat_fg,
                "本地服务与连接",
                Some("ollama-basic-tip"),
                Some(spec_tip),
                None,
                col()
                    .gap(px(12.))
                    .child(two_col_row(
                        labeled_stack("名称", name_input),
                        labeled_stack("协议类型", kind_selector),
                    ))
                    .child(labeled_stack_with_tip(
                        "服务地址",
                        Some(("ollama-ep-tip", format!("默认本机端口：{default_ep}"))),
                        row()
                            .gap(px(8.))
                            .child(div().flex_1().min_w_0().child(endpoint_input))
                            .when(endpoint_is_custom, |d| {
                                d.child(
                                    Button::new("translation-reset-endpoint")
                                        .small()
                                        .ghost()
                                        .label("恢复默认")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if let Some(input) =
                                                this.translation_page.inputs.get("endpoint")
                                            {
                                                input.update(cx, |state, cx| {
                                                    state.set_value(
                                                        default_ep.to_owned(),
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            }
                                            this.translation_page.draft.base_url =
                                                default_ep.to_owned();
                                            cx.notify();
                                        })),
                                )
                            }),
                    ))
                    .child(cred_block),
            )
        } else {
            let default_ep = default_endpoint(&kind);
            section_card_v2(
                "key-round",
                cat_fg,
                "接口与鉴权",
                Some("api-basic-tip"),
                Some(spec_tip),
                None,
                col()
                    .gap(px(12.))
                    .child(two_col_row(
                        labeled_stack("名称", name_input),
                        labeled_stack("协议类型", kind_selector),
                    ))
                    .child(labeled_stack_with_tip(
                        "API 端点",
                        Some(("api-ep-tip", format!("当前协议默认端点：{default_ep}"))),
                        row()
                            .gap(px(8.))
                            .child(div().flex_1().min_w_0().child(endpoint_input))
                            .when(endpoint_is_custom, |d| {
                                d.child(
                                    Button::new("translation-reset-endpoint")
                                        .small()
                                        .ghost()
                                        .label("恢复默认")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if let Some(input) =
                                                this.translation_page.inputs.get("endpoint")
                                            {
                                                input.update(cx, |state, cx| {
                                                    state.set_value(
                                                        default_ep.to_owned(),
                                                        window,
                                                        cx,
                                                    )
                                                });
                                            }
                                            this.translation_page.draft.base_url =
                                                default_ep.to_owned();
                                            cx.notify();
                                        })),
                                )
                            }),
                    ))
                    .child(divider_line())
                    .child(
                        col()
                            .gap(px(6.))
                            .child(
                                row()
                                    .gap(px(7.))
                                    .flex_wrap()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(rgb(subtext()))
                                            .child("API 密钥"),
                                    )
                                    .child(cred_status)
                                    .child(storage_badge)
                                    .child(help_tip("api-cred-tip", cred_hint)),
                            )
                            .child(
                                row()
                                    .gap(px(8.))
                                    .child(div().flex_1().min_w_0().child(credential_input))
                                    .when(selected && has_credential, |d| {
                                        d.child(
                                            Button::new("translation-credential-remove")
                                                .small()
                                                .danger()
                                                .icon(icon("delete", 13.))
                                                .label("移除密钥")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.translation_page
                                                        .remove_credential(&this.bridge);
                                                    cx.notify();
                                                })),
                                        )
                                    }),
                            ),
                    ),
            )
        };

        // Card 2 — Model & Inference Tuning
        let model_card = if local_built_in {
            let license_pill = pill_badge(
                "PLaMo Community License",
                badge_bg(),
                badge_text(),
                None,
            )
            .id("local-license-badge")
            .tooltip(hover_tip(
                "模型权重遵循 PLaMo Community License（个人与非商用免费），按需下载至本地目录，支持断点续传与 SHA-256 校验。",
            ))
            .into_any_element();

            section_card_v2(
                "cpu",
                cat_fg,
                "本地引擎与模型",
                Some("local-models-tip"),
                Some("由 Maltoria 自动拉取 llama.cpp 引擎与官方 GGUF 权重并校验 SHA-256；空闲 5 分钟自动释放内存。"),
                Some(license_pill),
                self.local_models_panel(&active_model_id, cx),
            )
        } else if deep_l {
            section_card_v2(
                "languages",
                cat_fg,
                "模型配置",
                None,
                None,
                None,
                row()
                    .gap(px(8.))
                    .p(px(10.))
                    .rounded(px(8.))
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .child(icon("info", 14.).text_color(rgb(cat_fg)))
                    .child(muted("DeepL 为专用机器翻译引擎，无需选择或配置模型 ID。")),
            )
        } else {
            let discover_tip = if capture_active {
                "请先停止当前游戏捕获再发现模型"
            } else if !selected {
                "请先保存提供商再在线发现模型"
            } else if !model_discovery {
                "当前协议不支持在线拉取模型列表"
            } else {
                "从接口拉取当前可用的模型列表"
            };
            let inspect_tip = if capture_active {
                "请先停止当前游戏捕获再探测模型信息"
            } else if !selected {
                "请先保存提供商再探测模型上下文"
            } else if !model_info_available {
                "当前协议不支持探测模型详情"
            } else {
                "探测当前模型 ID 的上下文窗口等元数据"
            };

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

            let thinking_supported = supports_thinking_level(&kind);
            let show_token_param = kind == "openAIChat";
            let view_for_thinking = cx.entity().downgrade();
            let current_thinking_label = THINKING_LEVELS
                .iter()
                .find(|(code, _)| *code == thinking_level)
                .map(|(_, label)| *label)
                .unwrap_or("模型默认");
            let thinking_dropdown = Button::new("translation-thinking-dropdown")
                .outline()
                .w_full()
                .icon(icon("brain", 13.))
                .label(current_thinking_label)
                .dropdown_caret(true)
                .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                    for (code, label) in THINKING_LEVELS {
                        let view = view_for_thinking.clone();
                        let checked = code == thinking_level;
                        menu = menu.item(
                            PopupMenuItem::new(label)
                                .checked(checked)
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.translation_page.set_thinking_level(code);
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                    menu
                });

            let view_for_token = cx.entity().downgrade();
            let current_token_label = if token_field == "max_tokens" {
                "max_tokens"
            } else {
                "max_completion_tokens"
            };
            let token_dropdown = Button::new("translation-token-field-dropdown")
                .outline()
                .w_full()
                .icon(icon("sliders-horizontal", 13.))
                .label(current_token_label)
                .dropdown_caret(true)
                .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                    let v1 = view_for_token.clone();
                    let v2 = view_for_token.clone();
                    menu.item(
                        PopupMenuItem::new("max_completion_tokens（OpenAI 推荐）")
                            .checked(current_token_label == "max_completion_tokens")
                            .on_click(move |_, _, cx| {
                                let _ = v1.update(cx, |this, cx| {
                                    this.translation_page
                                        .set_token_field("max_completion_tokens");
                                    cx.notify();
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new("max_tokens（旧版兼容）")
                            .checked(current_token_label == "max_tokens")
                            .on_click(move |_, _, cx| {
                                let _ = v2.update(cx, |this, cx| {
                                    this.translation_page.set_token_field("max_tokens");
                                    cx.notify();
                                });
                            }),
                    )
                });

            let tuning_row = if thinking_supported || show_token_param {
                Some(
                    row()
                        .gap(px(12.))
                        .items_start()
                        .when(thinking_supported, |r| {
                            r.child(
                                div().flex_1().min_w_0().child(labeled_stack_with_tip(
                                    "思考程度 (Thinking)",
                                    Some((
                                        "thinking-level-tip",
                                        "关闭或调低思考深度可显著降低实时字幕翻译延迟（适用于 GPT-5 / Claude / Gemini / cliproxyapi 等推理模型）。"
                                            .into(),
                                    )),
                                    thinking_dropdown,
                                )),
                            )
                        })
                        .when(show_token_param, |r| {
                            r.child(
                                div().flex_1().min_w_0().child(labeled_stack_with_tip(
                                    "Token 限制参数",
                                    Some((
                                        "token-param-tip",
                                        "OpenAI 新版及推理模型要求 max_completion_tokens；部分第三方中转或旧服务需切换为 max_tokens。"
                                            .into(),
                                    )),
                                    token_dropdown,
                                )),
                            )
                        }),
                )
            } else {
                None
            };

            section_card_v2(
                "sparkles",
                cat_fg,
                "模型与推理参数",
                Some("model-cfg-tip"),
                Some("输入模型 ID 或从接口发现模型；推理模型可按需调低思考深度以加速字幕响应。"),
                context_badge,
                col()
                    .gap(px(12.))
                    .child(labeled_stack(
                        "模型 ID",
                        row()
                            .gap(px(8.))
                            .child(div().flex_1().min_w_0().child(model_input))
                            .child(
                                Button::new("translation-discover")
                                    .small()
                                    .icon(icon("search", 13.))
                                    .label("发现模型")
                                    .tooltip(discover_tip)
                                    .disabled(!selected || !model_discovery || probes_blocked)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.translation_page.discover(&this.bridge);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("translation-model-info")
                                    .small()
                                    .icon(icon("info", 13.))
                                    .label("模型信息")
                                    .tooltip(inspect_tip)
                                    .disabled(!selected || !model_info_available || probes_blocked)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.translation_page.inspect_model(&this.bridge, cx);
                                        cx.notify();
                                    })),
                            ),
                    ))
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
                    .when_some(tuning_row, |d, r| d.child(divider_line()).child(r)),
            )
        };

        // Card 3 — 默认语言
        let lang_card = section_card_v2(
            "languages",
            cat_fg,
            "默认语言",
            Some("provider-lang-tip"),
            Some("可选：为当前提供商指定默认源语言与目标语言；留空或设为 auto 时跟随全局翻译设置。"),
            None,
            row()
                .gap(px(10.))
                .items_end()
                .child(div().flex_1().min_w_0().child(source_field))
                .child(
                    div()
                        .pb(px(9.))
                        .child(icon("arrow-right", 14.).text_color(rgb(muted_color()))),
                )
                .child(div().flex_1().min_w_0().child(target_field)),
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
                            .icon(icon("check", 14.))
                            .label("保存提供商")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.translation_page
                                    .save_provider(&this.bridge, window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("translation-test-bottom")
                            .icon(icon("plug-zap", 14.))
                            .label("测试连接")
                            .tooltip(test_tooltip)
                            .disabled(!selected || probes_blocked)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.test_connection(&this.bridge);
                                cx.notify();
                            })),
                    ),
            )
            .when(!selected, |d| {
                d.child(
                    pill_badge("草稿未保存", accent_subtle_bg(), accent_fg(), None)
                        .id("draft-unsaved-tip")
                        .tooltip(hover_tip("保存草稿后即可测试连接或发现模型。")),
                )
            });

        col()
            .gap(px(14.))
            .child(hero_card)
            .child(basic_card)
            .child(model_card)
            .child(lang_card)
            .child(action_bar)
            .into_any_element()
    }

    fn local_models_panel(&self, active_model_id: &str, cx: &Context<Self>) -> AnyElement {
        let Some(status) = self.translation_page.local_status.as_ref() else {
            return row()
                .justify_between()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(8.))
                .bg(rgba(card_bg()))
                .border_1()
                .border_color(rgba(card_border()))
                .child(
                    row()
                        .gap(px(8.))
                        .child(Spinner::new().with_size(px(13.)))
                        .child(muted("正在读取本地推理引擎与模型状态…")),
                )
                .child(
                    Button::new("translation-local-refresh")
                        .small()
                        .icon(icon("refresh-cw", 13.))
                        .label("刷新")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.translation_page
                                .refresh_local_status_silent(&this.bridge);
                            cx.notify();
                        })),
                )
                .into_any_element();
        };

        let status = status.clone();
        let runtime = &status.runtime;
        let gpu_tag = runtime
            .gpu_acceleration
            .as_deref()
            .map(|g| match g {
                "metal" => "Metal GPU",
                "vulkan" => "Vulkan GPU",
                "cpu" => "CPU",
                other => other,
            })
            .unwrap_or("自动");

        let runtime_badge = if runtime.running {
            let loaded_name = runtime
                .loaded_model_id
                .as_deref()
                .and_then(|id| status.models.iter().find(|m| m.id == id))
                .map(|m| m.display_name.as_str())
                .or(runtime.loaded_model_id.as_deref())
                .unwrap_or("本地模型");
            pill_badge(
                format!("● 运行中 · {loaded_name}"),
                installed_bg(),
                installed_fg(),
                None,
            )
            .id("local-runtime-pill")
            .tooltip(hover_tip(format!(
                "llama.cpp 正在运行（{gpu_tag}），空闲 5 分钟后自动释放内存。"
            )))
        } else if runtime.installed {
            let ver = runtime
                .version
                .as_deref()
                .unwrap_or(&runtime.pinned_version);
            pill_badge(
                format!("✓ 引擎就绪 · {gpu_tag}"),
                installed_bg(),
                installed_fg(),
                None,
            )
            .id("local-runtime-pill")
            .tooltip(hover_tip(format!(
                "llama.cpp {ver}（{gpu_tag}）已安装，开始翻译时按需启动。"
            )))
        } else {
            pill_badge(
                format!("未安装引擎 · {}", format_bytes_human(runtime.size_bytes)),
                badge_bg(),
                badge_text(),
                None,
            )
            .id("local-runtime-pill")
            .tooltip(hover_tip(format!(
                "首次下载模型时将自动安装 llama.cpp（{gpu_tag}，约 {}）。",
                format_bytes_human(runtime.size_bytes)
            )))
        };

        let mirror_control = if let Some(override_url) = status.hf_endpoint_override.as_deref() {
            pill_badge("环境变量镜像", badge_bg(), accent_fg(), None)
                .id("local-mirror-override")
                .tooltip(hover_tip(format!("下载源已被环境变量覆盖为：{override_url}")))
                .into_any_element()
        } else {
            let mirror_tip = if status.use_china_mirror {
                "当前使用国内镜像源（hf-mirror.com），适合中国大陆网络下载 GGUF 权重。"
            } else {
                "当前使用 Hugging Face 官方源（huggingface.co）；国内下载缓慢时建议开启镜像。"
            };
            row()
                .gap(px(6.))
                .child(icon("globe", 13.).text_color(rgb(muted_color())))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(subtext()))
                        .child("国内镜像"),
                )
                .child(help_tip("local-mirror-tip", mirror_tip))
                .child(
                    Switch::new("translation-local-china-mirror")
                        .checked(status.use_china_mirror)
                        .on_change(cx.listener(|this, checked: &bool, _, cx| {
                            this.translation_page
                                .set_local_china_mirror(&this.bridge, *checked);
                            cx.notify();
                        })),
                )
                .into_any_element()
        };

        let runtime_toolbar = row()
            .justify_between()
            .gap(px(10.))
            .flex_wrap()
            .px(px(11.))
            .py(px(8.))
            .rounded(px(8.))
            .bg(rgba(card_bg()))
            .border_1()
            .border_color(rgba(card_border()))
            .child(
                row()
                    .gap(px(6.))
                    .flex_wrap()
                    .child(runtime_badge)
                    .when(runtime.running, |d| {
                        d.child(
                            Button::new("translation-local-unload")
                                .xsmall()
                                .label("释放内存")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.translation_page.unload_local_runtime(&this.bridge);
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("translation-local-refresh-bar")
                            .xsmall()
                            .ghost()
                            .icon(icon("refresh-cw", 12.))
                            .tooltip("刷新引擎与模型状态")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page
                                    .refresh_local_status_silent(&this.bridge);
                                cx.notify();
                            })),
                    )
                    .when_some(runtime.last_error.clone(), |d, err| {
                        d.child(
                            row()
                                .id("local-runtime-err")
                                .gap(px(4.))
                                .tooltip(hover_tip(err))
                                .child(icon("triangle-alert", 13.).text_color(rgb(danger_fg())))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(danger_fg()))
                                        .child("引擎异常"),
                                ),
                        )
                    }),
            )
            .child(mirror_control);

        let cards: Vec<AnyElement> = status
            .models
            .iter()
            .map(|model| self.local_model_card(model, model.id == active_model_id, cx))
            .collect();

        col()
            .gap(px(10.))
            .child(runtime_toolbar)
            .child(col().gap(px(8.)).children(cards))
            .into_any_element()
    }

    fn local_model_card(
        &self,
        model: &LocalModelDto,
        is_selected: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let model_id = model.id.clone();
        let model_clone = model.clone();
        let size_label = format_bytes_human(model.installed_size_bytes.unwrap_or(model.size_bytes));
        let ctx_label = format_context_window(model.context_window);
        let (cat_fg, cat_bg, cat_border) = ProviderCategory::Local.colors();

        let status_pill = if model.loaded {
            pill_badge("● 运行中", installed_bg(), installed_fg(), None)
        } else if model.installed {
            pill_badge("✓ 已下载", installed_bg(), installed_fg(), None)
        } else {
            pill_badge("未下载", badge_bg(), badge_text(), None)
        };

        let progress_block = model.download.as_ref().map(|dl| {
            let total = dl.total_bytes.max(1);
            let clamped = dl.downloaded_bytes.min(total);
            let ratio = (clamped as f32 / total as f32).clamp(0.0, 1.0);
            let pct = ratio * 100.0;
            let stage_label = local_download_stage_label(&dl.stage);
            let is_verifying_or_extracting = matches!(
                dl.stage.as_str(),
                "verifyingRuntime" | "extractingRuntime" | "verifyingModel"
            );
            let detail_text = if is_verifying_or_extracting {
                "正在本地校验与准备文件…".to_owned()
            } else {
                format!(
                    "{} / {} · {} · {}",
                    format_bytes_human(clamped),
                    format_bytes_human(total),
                    format_speed_human(dl.bytes_per_second),
                    format_eta_human(dl.eta_seconds)
                )
            };
            let cancel_id = model_id.clone();

            col()
                .gap(px(6.))
                .p(px(9.))
                .rounded(px(7.))
                .bg(rgba(card_bg_subtle()))
                .border_1()
                .border_color(rgba(accent_subtle_border()))
                .child(
                    row()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            row()
                                .gap(px(6.))
                                .child(Spinner::new().with_size(px(12.)))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(text()))
                                        .child(stage_label),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(accent_fg()))
                                .child(if is_verifying_or_extracting {
                                    "校验中…".to_owned()
                                } else {
                                    format!("{pct:.1}%")
                                }),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(6.))
                        .rounded(px(3.))
                        .bg(rgba(badge_bg()))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(if is_verifying_or_extracting {
                                    1.0
                                } else {
                                    ratio.max(0.01)
                                }))
                                .rounded(px(3.))
                                .bg(rgb(if is_verifying_or_extracting {
                                    installed_fg()
                                } else {
                                    accent_fg()
                                })),
                        ),
                )
                .child(
                    row()
                        .justify_between()
                        .gap(px(8.))
                        .flex_wrap()
                        .child(muted(detail_text).text_size(px(11.)))
                        .child(
                            Button::new(format!("translation-local-cancel:{cancel_id}"))
                                .xsmall()
                                .danger()
                                .label("取消下载")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.translation_page
                                        .cancel_local_download(&this.bridge, &cancel_id);
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element()
        });

        let error_block = model.last_error.as_ref().map(|err| {
            row()
                .gap(px(6.))
                .p(px(8.))
                .rounded(px(6.))
                .bg(rgba(0xff5f571a))
                .border_1()
                .border_color(rgba(0xff5f574d))
                .child(icon("triangle-alert", 12.).text_color(rgb(danger_fg())))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(11.))
                        .text_color(rgb(danger_fg()))
                        .child(format!("下载失败：{err}")),
                )
                .into_any_element()
        });

        let downloading = model.download.is_some();
        let select_model = model_clone.clone();
        let download_id = model_id.clone();
        let delete_id = model_id.clone();

        let actions = row()
            .gap(px(6.))
            .flex_shrink_0()
            .when(!downloading && !model.installed, |d| {
                let model_for_pick = select_model.clone();
                let dl_id = download_id.clone();
                d.child(
                    Button::new(format!("translation-local-dl:{dl_id}"))
                        .small()
                        .primary()
                        .icon(icon("download", 13.))
                        .label(if model.last_error.is_some() {
                            "重试下载"
                        } else {
                            "下载模型"
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.translation_page
                                .choose_local_model(&model_for_pick, window, cx);
                            this.translation_page
                                .download_local_model(&this.bridge, &dl_id, cx);
                            cx.notify();
                        })),
                )
            })
            .when(!downloading && model.installed, |d| {
                let model_for_pick = select_model.clone();
                let del_id = delete_id.clone();
                let select_btn = if is_selected {
                    Button::new(format!("translation-local-use:{model_id}"))
                        .small()
                        .icon(icon("check", 13.))
                        .label("当前已选")
                        .disabled(true)
                } else {
                    Button::new(format!("translation-local-use:{model_id}"))
                        .small()
                        .primary()
                        .label("选用")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.translation_page
                                .choose_local_model(&model_for_pick, window, cx);
                            cx.notify();
                        }))
                };
                d.child(select_btn).child(
                    Button::new(format!("translation-local-del:{del_id}"))
                        .small()
                        .danger()
                        .icon(icon("delete", 13.))
                        .tooltip("删除本地模型权重文件")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.translation_page
                                .delete_local_model(&this.bridge, &del_id);
                            cx.notify();
                        })),
                )
            });

        let subtitle_tip = if model.subtitle.trim().is_empty() {
            None
        } else {
            Some(help_tip(
                format!("local-model-tip:{}", model.id),
                model.subtitle.clone(),
            ))
        };

        let specs_summary = if model.recommended_ram_gb > 0 {
            format!(
                "{size_label} · 上下文 {ctx_label} · 建议 {} GB 内存",
                model.recommended_ram_gb
            )
        } else {
            format!("{size_label} · 上下文 {ctx_label}")
        };

        col()
            .id(format!("translation-local-card:{}", model.id))
            .gap(px(8.))
            .p(px(11.))
            .rounded(px(9.))
            .bg(rgba(if is_selected {
                accent_subtle_bg()
            } else {
                card_bg()
            }))
            .border_1()
            .border_color(rgba(if is_selected {
                accent_subtle_border()
            } else {
                card_border()
            }))
            .child(
                row()
                    .justify_between()
                    .gap(px(10.))
                    .child(
                        row()
                            .flex_1()
                            .min_w_0()
                            .gap(px(10.))
                            .child(category_icon_tile_raw(
                                cat_fg,
                                cat_bg,
                                cat_border,
                                "cpu",
                                32.,
                                15.,
                            ))
                            .child(
                                col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(3.))
                                    .child(
                                        row()
                                            .gap(px(6.))
                                            .flex_wrap()
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_size(px(13.))
                                                    .text_color(rgb(text()))
                                                    .child(model.display_name.clone()),
                                            )
                                            .when(model.recommended, |d| {
                                                d.child(pill_badge(
                                                    "推荐",
                                                    accent_subtle_bg(),
                                                    accent_fg(),
                                                    None,
                                                ))
                                            })
                                            .child(status_pill)
                                            .when_some(subtitle_tip, |d, tip| d.child(tip)),
                                    )
                                    .child(muted(specs_summary).text_size(px(11.)).truncate()),
                            ),
                    )
                    .child(actions),
            )
            .when_some(progress_block, |d, block| d.child(block))
            .when_some(error_block, |d, block| d.child(block))
            .into_any_element()
    }

    fn translation_empty_editor(&self, cx: &Context<Self>) -> AnyElement {
        let oauth_available = self.category_has_available_specs(ProviderCategory::OAuth);
        let cards = ProviderCategory::ALL.into_iter().map(|cat| {
            let (fg, bg, border) = cat.colors();
            let available = cat != ProviderCategory::OAuth || oauth_available;
            let (tags, action_label): (Vec<&'static str>, &'static str) = match cat {
                ProviderCategory::Local => (vec!["内置 PLaMo 3", "Ollama"], "开始配置"),
                ProviderCategory::OAuth => {
                    if available {
                        (vec!["账号直连", "浏览器授权"], "开始配置")
                    } else {
                        (vec!["浏览器授权", "免密钥"], "当前版本未启用")
                    }
                }
                ProviderCategory::Api => (vec!["DeepSeek / OpenAI", "Claude / Gemini"], "开始配置"),
            };

            let card = col()
                .id(format!("translation-empty-cat:{}", cat.short_label()))
                .flex_1()
                .min_w(px(170.))
                .gap(px(12.))
                .p(px(16.))
                .rounded(px(11.))
                .bg(rgba(card_bg()))
                .border_1()
                .border_color(rgba(if available { border } else { card_border() }))
                .when(!available, |d| d.opacity(0.68))
                .tooltip(hover_tip(if available {
                    cat.tooltip()
                } else {
                    "当前构建未启用 OAuth 账号接入；可使用本地模型或 API 密钥提供商。"
                }))
                .when(available, |d| {
                    d.cursor_pointer()
                        .hover(|s| s.bg(rgba(bg)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.translation_page.open_wizard(Some(cat));
                            if cat == ProviderCategory::Local {
                                this.translation_page
                                    .refresh_local_status_silent(&this.bridge);
                            }
                            cx.notify();
                        }))
                })
                .child(
                    row()
                        .justify_between()
                        .child(category_icon_tile_raw(fg, bg, border, cat.icon(), 38., 18.))
                        .when(available, |d| {
                            d.child(icon("arrow-right", 14.).text_color(rgb(fg)))
                        }),
                )
                .child(
                    col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(text()))
                                .child(cat.label()),
                        )
                        .child(muted(cat.tagline()).text_size(px(11.5))),
                )
                .child(
                    row()
                        .gap(px(5.))
                        .flex_wrap()
                        .children(
                            tags.into_iter()
                                .map(|t| pill_badge(t, badge_bg(), badge_text(), None)),
                        ),
                )
                .child(
                    div()
                        .pt(px(2.))
                        .text_size(px(11.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(if available { fg } else { muted_color() }))
                        .child(action_label),
                );
            card.into_any_element()
        });

        col()
            .gap(px(16.))
            .p(px(20.))
            .rounded(px(12.))
            .bg(rgba(section_bg()))
            .border_1()
            .border_color(rgba(section_border()))
            .child(
                row()
                    .justify_between()
                    .gap(px(12.))
                    .flex_wrap()
                    .child(
                        row()
                            .gap(px(8.))
                            .child(icon("sparkles", 16.).text_color(rgb(accent_fg())))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(text()))
                                    .child("选择一种方式接入 AI 翻译"),
                            ),
                    )
                    .child(muted("悬停卡片查看详情，点击直接进入配置").text_size(px(11.5))),
            )
            .child(row().gap(px(12.)).items_stretch().flex_wrap().children(cards))
            .child(
                row()
                    .gap(px(8.))
                    .flex_wrap()
                    .child(
                        Button::new("translation-empty-wizard")
                            .primary()
                            .icon(icon("wand-sparkles", 14.))
                            .label("打开交互式配置向导")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.open_wizard(None);
                                this.translation_page
                                    .refresh_local_status_silent(&this.bridge);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("translation-empty-manual")
                            .ghost()
                            .icon(icon("plus", 14.))
                            .label("直接填写自定义 API")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.new_provider("openAIChat");
                                cx.notify();
                            })),
                    ),
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
        let debounce_ms = settings
            .as_ref()
            .map(|s| s.translation_debounce_ms)
            .unwrap_or_else(default_translation_debounce_ms);
        let debounce_value = debounce_ms.to_string();
        let sentence_cache_enabled = settings
            .as_ref()
            .map(|s| s.sentence_cache_enabled)
            .unwrap_or_else(default_sentence_cache_enabled);
        let remote_enabled = self.translation_page.remote_enabled;
        let storage = credential_storage_label(&self.translation_page.credential_storage);
        let active_storage_is_plaintext =
            self.translation_page.credential_storage == "localPlaintextFile";
        let selected_storage_mode = normalize_credential_storage_mode(
            if self.translation_page.credential_storage_mode.is_empty() {
                settings
                    .as_ref()
                    .map(|s| s.credential_storage_mode.as_str())
                    .unwrap_or("keychain")
            } else {
                self.translation_page.credential_storage_mode.as_str()
            },
        );

        let global_source_field = language_preset_field(
            self,
            "source-language",
            "全局源语言",
            &global_source_value,
            "例如：auto / ja",
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
        let debounce_input = Input::new(&self.translation_page.input(
            "debounce-ms",
            &debounce_value,
            "0 - 5000",
            window,
            cx,
        ));

        let view_for_debounce = cx.entity().downgrade();
        let debounce_label = DEBOUNCE_PRESETS
            .iter()
            .find(|(ms, _)| *ms == debounce_ms)
            .map(|(_, label)| *label)
            .unwrap_or("自定义预设");
        let debounce_dropdown = Button::new("translation-debounce-dropdown")
            .outline()
            .icon(icon("clock-3", 13.))
            .label(debounce_label)
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for (ms, label) in DEBOUNCE_PRESETS {
                    let view = view_for_debounce.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(debounce_ms == ms)
                            .on_click(move |_, window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.translation_page
                                        .apply_translation_debounce_preset(ms, window, cx);
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            });

        let view_for_storage = cx.entity().downgrade();
        let storage_mode_label = CREDENTIAL_STORAGE_MODES
            .iter()
            .find(|(mode, _)| *mode == selected_storage_mode)
            .map(|(_, label)| *label)
            .unwrap_or(SYSTEM_CREDENTIAL_LABEL);
        let storage_mode_icon = if selected_storage_mode == "localFile" {
            "shield-alert"
        } else if selected_storage_mode == "sessionOnly" {
            "clock-3"
        } else {
            "shield-check"
        };
        let storage_mode_for_menu = selected_storage_mode.clone();
        let storage_dropdown = Button::new("translation-storage-dropdown")
            .outline()
            .icon(icon(storage_mode_icon, 13.))
            .label(storage_mode_label)
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for (mode, label) in CREDENTIAL_STORAGE_MODES {
                    let view = view_for_storage.clone();
                    let mode_icon = match mode {
                        "localFile" => "shield-alert",
                        "sessionOnly" => "clock-3",
                        _ => "shield-check",
                    };
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .icon(icon(mode_icon, 13.))
                            .checked(storage_mode_for_menu == mode)
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.translation_page.set_credential_storage_mode(mode);
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            });

        let prompt_entity = self
            .translation_page
            .textarea(&prompt_value, "自定义系统提示词（留空使用默认指令）", window, cx);
        let prompt_area = Textarea::new(&prompt_entity).h(px(120.));

        let languages_card = section_card_v2(
            "languages",
            accent_fg(),
            "默认翻译语言",
            Some("settings-lang-tip"),
            Some("当提供商未单独指定语言时，将使用此处的全局源语言与目标语言。"),
            None,
            row()
                .gap(px(10.))
                .items_end()
                .child(div().flex_1().min_w_0().child(global_source_field))
                .child(
                    div()
                        .pb(px(9.))
                        .child(icon("arrow-right", 14.).text_color(rgb(muted_color()))),
                )
                .child(div().flex_1().min_w_0().child(global_target_field)),
        );

        let budget_card = section_card_v2(
            "database",
            accent_fg(),
            "上下文与 Token 预算",
            Some("settings-budget-tip"),
            Some("控制单次输出长度限制、历史记忆轮数以及上下文窗口覆盖。"),
            None,
            col()
                .gap(px(12.))
                .child(two_col_row(
                    labeled_stack("输出 Token 上限", output_input),
                    labeled_stack("上下文窗口覆盖", context_input),
                ))
                .child(two_col_row(
                    labeled_stack("上下文参考轮数", turns_input),
                    labeled_stack("历史保留轮数", retention_input),
                ))
                .child(divider_line())
                .child(compact_switch_row(
                    "translation-source-history",
                    "携带源文历史",
                    "settings-source-hist-tip",
                    "在翻译请求中附上最近几轮原文，有助于保持人称和语境一致。",
                    source_history,
                    cx.listener(|this, _, _, cx| {
                        this.translation_page.toggle_source_history();
                        cx.notify();
                    }),
                ))
                .child(compact_switch_row(
                    "translation-translated-history",
                    "携带译文历史",
                    "settings-trans-hist-tip",
                    "附上近期已翻出的译文，有助于保持专有名词与口吻前后统一。",
                    translated_history,
                    cx.listener(|this, _, _, cx| {
                        this.translation_page.toggle_translated_history();
                        cx.notify();
                    }),
                )),
        );

        let debounce_cache_card = section_card_v2(
            "zap",
            accent_fg(),
            "快进防抖与句子缓存",
            Some("settings-debounce-tip"),
            Some("控制文本稳定后再发送翻译，并跨游戏重启复用相同句子的译文以节省调用与费用。"),
            None,
            col()
                .gap(px(12.))
                .child(compact_switch_row(
                    "translation-sentence-cache",
                    "相同句子缓存复用",
                    "settings-cache-reuse-tip",
                    "命中同游戏、同说话人与语言配置的相同原文时直接复用译文，跨游戏重启生效。",
                    sentence_cache_enabled,
                    cx.listener(|this, _, _, cx| {
                        this.translation_page.toggle_sentence_cache_enabled();
                        cx.notify();
                    }),
                ))
                .child(divider_line())
                .child(
                    row()
                        .justify_between()
                        .gap(px(12.))
                        .flex_wrap()
                        .child(
                            row()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(rgb(text()))
                                        .child("防抖延迟"),
                                )
                                .child(help_tip(
                                    "settings-debounce-ms-tip",
                                    "快进/Ctrl 跳过或 Hook 分段输出时等待文本稳定再翻译，可设为 0–5000 ms。",
                                )),
                        )
                        .child(
                            row()
                                .gap(px(8.))
                                .child(debounce_dropdown)
                                .child(div().w(px(96.)).child(debounce_input))
                                .child(muted("ms").text_size(px(12.))),
                        ),
                ),
        );

        let prompt_card = section_card_v2(
            "sparkles",
            accent_fg(),
            "自定义系统提示词",
            Some("settings-prompt-tip"),
            Some("用于指导大语言模型翻译风格、术语表或人称语气（DeepL 会自动忽略此提示词）。"),
            None,
            prompt_area,
        );

        let storage_tip = storage_mode_detail(&selected_storage_mode);
        let storage_badge = row()
            .id("settings-storage-status-badge")
            .gap(px(5.))
            .px(px(8.))
            .py(px(3.))
            .rounded_full()
            .bg(rgba(if active_storage_is_plaintext {
                0xf5a6231c
            } else {
                badge_bg()
            }))
            .tooltip(hover_tip(storage_tip))
            .child(
                icon(
                    if active_storage_is_plaintext {
                        "triangle-alert"
                    } else {
                        "lock"
                    },
                    12.,
                )
                .text_color(rgb(if active_storage_is_plaintext {
                    warning_fg()
                } else {
                    badge_text()
                })),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if active_storage_is_plaintext {
                        warning_fg()
                    } else {
                        badge_text()
                    }))
                    .child(storage),
            );

        let security_card = section_card_v2(
            "shield-check",
            accent_fg(),
            "网络与凭证存储",
            Some("settings-security-tip"),
            Some("控制是否允许连接远程服务，以及 API 密钥的本机保存方式；悬停状态徽章可查看安全说明。"),
            Some(storage_badge.into_any_element()),
            col()
                .gap(px(12.))
                .child(compact_switch_row(
                    "translation-remote",
                    "允许远程提供商",
                    "settings-remote-tip",
                    "关闭后仅允许访问本机环回地址（如本地模型或本机 Ollama），禁止向远程 API 发送文本。",
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
                        .flex_wrap()
                        .child(
                            row()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(rgb(text()))
                                        .child("凭证存储方式"),
                                )
                                .child(help_tip("settings-storage-mode-tip", storage_tip)),
                        )
                        .child(storage_dropdown),
                ),
        );

        let current_game_id = self.translation_page.cache.scope_game_id.clone();
        let cache_management_card = section_card_v2(
            "layers",
            accent_fg(),
            "翻译结果缓存管理",
            Some("settings-cache-browser-tip"),
            Some("查看、检索、按条删除或清空已持久化保存的句子翻译缓存。"),
            None,
            self.translation_cache_browser(false, current_game_id, window, cx),
        );

        col()
            .gap(px(14.))
            .child(languages_card)
            .child(budget_card)
            .child(debounce_cache_card)
            .child(prompt_card)
            .child(security_card)
            .child(
                row().child(
                    Button::new("translation-save-settings")
                        .primary()
                        .icon(icon("check", 14.))
                        .label("保存翻译设置")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.translation_page.save_settings(&this.bridge, cx);
                            cx.notify();
                        })),
                ),
            )
            .child(cache_management_card)
            .into_any_element()
    }

    pub(crate) fn translation_wizard_modal(
        &mut self,
        width: f32,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(wizard) = self.translation_page.wizard.clone() else {
            return div().into_any_element();
        };

        let step = wizard.step;
        let stepper = wizard_stepper(step);

        let body: AnyElement = match step {
            WizardStep::ChooseCategory => {
                let oauth_available = self.category_has_available_specs(ProviderCategory::OAuth);
                let cards = ProviderCategory::ALL.into_iter().map(|cat| {
                    let (fg, bg, border) = cat.colors();
                    let available = cat != ProviderCategory::OAuth || oauth_available;
                    col()
                        .id(format!("wiz-cat:{}", cat.short_label()))
                        .flex_1()
                        .min_w(px(160.))
                        .gap(px(10.))
                        .p(px(14.))
                        .rounded(px(11.))
                        .bg(rgba(card_bg()))
                        .border_1()
                        .border_color(rgba(if available { border } else { card_border() }))
                        .when(!available, |d| d.opacity(0.65))
                        .tooltip(hover_tip(if available {
                            cat.tooltip()
                        } else {
                            "当前构建未启用 OAuth 账号接入；请选择本地模型或 API 密钥。"
                        }))
                        .when(available, |d| {
                            d.cursor_pointer()
                                .hover(|s| s.bg(rgba(bg)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.translation_page.wizard_select_category(cat);
                                    if cat == ProviderCategory::Local {
                                        this.translation_page
                                            .refresh_local_status_silent(&this.bridge);
                                    }
                                    cx.notify();
                                }))
                        })
                        .child(
                            row()
                                .justify_between()
                                .child(category_icon_tile_raw(fg, bg, border, cat.icon(), 36., 17.))
                                .when(available, |d| {
                                    d.child(icon("chevron-right", 14.).text_color(rgb(fg)))
                                }),
                        )
                        .child(
                            col()
                                .gap(px(3.))
                                .child(
                                    div()
                                        .text_size(px(13.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(text()))
                                        .child(cat.label()),
                                )
                                .child(muted(cat.tagline()).text_size(px(11.))),
                        )
                        .when(!available, |d| {
                            d.child(pill_badge("未启用", badge_bg(), badge_text(), None))
                        })
                        .into_any_element()
                });

                col()
                    .gap(px(12.))
                    .child(
                        muted("第 1 步：选择希望使用的 AI 接入方式")
                            .text_size(px(12.5)),
                    )
                    .child(row().gap(px(10.)).items_stretch().flex_wrap().children(cards))
                    .into_any_element()
            }
            WizardStep::ChoosePreset => {
                let category = wizard.category.unwrap_or(ProviderCategory::Api);
                let (fg, bg, border) = category.colors();
                let presets: Vec<&'static ProviderPreset> = PROVIDER_PRESETS
                    .iter()
                    .filter(|p| p.category == category && self.preset_available(p))
                    .collect();

                let mut rows: Vec<AnyElement> = Vec::new();
                for pair in presets.chunks(2) {
                    let pair_cards = pair.iter().map(|preset| {
                        let preset_ref = *preset;
                        row()
                            .id(format!("wiz-preset:{}", preset.id))
                            .flex_1()
                            .min_w(px(220.))
                            .justify_between()
                            .gap(px(10.))
                            .p(px(12.))
                            .rounded(px(10.))
                            .cursor_pointer()
                            .bg(rgba(card_bg()))
                            .border_1()
                            .border_color(rgba(card_border()))
                            .hover(|s| s.bg(rgba(bg)).border_color(rgba(border)))
                            .tooltip(hover_tip(preset.tip))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.translation_page.wizard_select_preset(preset_ref);
                                if preset_ref.kind == "localBuiltIn" {
                                    this.translation_page
                                        .refresh_local_status_silent(&this.bridge);
                                }
                                cx.notify();
                            }))
                            .child(
                                row()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(10.))
                                    .child(category_icon_tile_raw(
                                        fg,
                                        bg,
                                        border,
                                        preset.icon,
                                        34.,
                                        16.,
                                    ))
                                    .child(
                                        col()
                                            .flex_1()
                                            .min_w_0()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_size(px(13.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(text()))
                                                    .child(preset.name),
                                            )
                                            .child(
                                                row().gap(px(5.)).child(pill_badge(
                                                    preset.tag,
                                                    bg,
                                                    fg,
                                                    None,
                                                )),
                                            ),
                                    ),
                            )
                            .child(icon("chevron-right", 14.).text_color(rgb(muted_color())))
                            .into_any_element()
                    });

                    let mut r = row().gap(px(10.)).children(pair_cards);
                    if pair.len() == 1 {
                        r = r.child(div().flex_1().min_w(px(220.)));
                    }
                    rows.push(r.into_any_element());
                }

                col()
                    .gap(px(12.))
                    .child(
                        row()
                            .gap(px(8.))
                            .child(category_pill(category))
                            .child(muted("第 2 步：选择常用预设或自定义模板").text_size(px(12.))),
                    )
                    .child(col().gap(px(8.)).children(rows))
                    .into_any_element()
            }
            WizardStep::Configure => {
                let kind = wizard.kind.clone();
                let cat = wizard.category.unwrap_or_else(|| provider_category(&kind));
                let is_local = kind == "localBuiltIn";
                let is_ollama = kind == "ollama";
                let is_deepl = kind == "deepL";

                let wiz_name = Input::new(&self.translation_page.input(
                    "wiz-name",
                    &wizard.name,
                    "显示名称",
                    window,
                    cx,
                ));
                let wiz_endpoint = Input::new(&self.translation_page.input(
                    "wiz-endpoint",
                    &wizard.base_url,
                    default_endpoint(&kind),
                    window,
                    cx,
                ));
                let wiz_model = Input::new(&self.translation_page.input(
                    "wiz-model",
                    &wizard.model_id,
                    "留空或保存后在线发现",
                    window,
                    cx,
                ));
                let wiz_cred = Input::new(&self.translation_page.input(
                    "wiz-credential",
                    "",
                    if is_ollama {
                        "可选：如本地无鉴权可留空"
                    } else {
                        "输入 API 密钥（掩码保护）"
                    },
                    window,
                    cx,
                ));

                let mut fields = col().gap(px(12.)).child(
                    row()
                        .gap(px(8.))
                        .child(category_pill(cat))
                        .child(pill_badge(
                            provider_short_label(&kind).to_owned(),
                            badge_bg(),
                            badge_text(),
                            None,
                        ))
                        .child(muted("第 3 步：填写必要连接信息").text_size(px(12.))),
                );

                fields = fields.child(labeled_stack("显示名称", wiz_name));

                if is_local {
                    let active_local = wizard.model_id.clone();
                    let model_choices: Vec<(String, String, String, bool)> = self
                        .translation_page
                        .local_status
                        .as_ref()
                        .filter(|s| !s.models.is_empty())
                        .map(|s| {
                            s.models
                                .iter()
                                .map(|m| {
                                    (
                                        m.id.clone(),
                                        m.display_name.clone(),
                                        format!(
                                            "{} · {} 上下文",
                                            format_bytes_human(
                                                m.installed_size_bytes.unwrap_or(m.size_bytes)
                                            ),
                                            format_context_window(m.context_window)
                                        ),
                                        m.installed,
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_else(|| {
                            vec![
                                (
                                    "plamo-3-translate-2b-q4_k_m".into(),
                                    "PLaMo 3 Translate 2B (Q4_K_M)".into(),
                                    "约 1.68 GB · 推荐主力".into(),
                                    false,
                                ),
                                (
                                    "plamo-3-translate-0.8b-q4_k_m".into(),
                                    "PLaMo 3 Translate 0.8B (Q4_K_M)".into(),
                                    "约 546 MB · 轻量极速".into(),
                                    false,
                                ),
                            ]
                        });

                    let local_cards = model_choices.into_iter().map(
                        |(id, display, meta, installed)| {
                            let picked = active_local == id;
                            let id_click = id.clone();
                            row()
                                .id(format!("wiz-local-model:{id}"))
                                .justify_between()
                                .gap(px(10.))
                                .p(px(10.))
                                .rounded(px(9.))
                                .cursor_pointer()
                                .bg(rgba(if picked {
                                    accent_subtle_bg()
                                } else {
                                    card_bg()
                                }))
                                .border_1()
                                .border_color(rgba(if picked {
                                    accent_subtle_border()
                                } else {
                                    card_border()
                                }))
                                .hover(|s| if picked { s } else { s.bg(rgba(hover())) })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.translation_page.wizard_set_local_model(&id_click);
                                    cx.notify();
                                }))
                                .child(
                                    row()
                                        .gap(px(8.))
                                        .child(
                                            icon(
                                                if picked { "circle-check" } else { "cpu" },
                                                15.,
                                            )
                                            .text_color(rgb(if picked {
                                                accent_fg()
                                            } else {
                                                muted_color()
                                            })),
                                        )
                                        .child(
                                            col()
                                                .gap(px(2.))
                                                .child(
                                                    div()
                                                        .text_size(px(12.5))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(rgb(text()))
                                                        .child(display),
                                                )
                                                .child(muted(meta).text_size(px(11.))),
                                        ),
                                )
                                .when(installed, |d| {
                                    d.child(pill_badge(
                                        "已下载",
                                        installed_bg(),
                                        installed_fg(),
                                        None,
                                    ))
                                })
                                .into_any_element()
                        },
                    );

                    fields = fields
                        .child(labeled_stack_with_tip(
                            "默认本地模型",
                            Some((
                                "wiz-local-tip",
                                "保存后可在主界面点击「下载模型」一键拉取 GGUF 权重与 llama.cpp 引擎。"
                                    .into(),
                            )),
                            col().gap(px(6.)).children(local_cards),
                        ))
                        .when_some(
                            self.translation_page
                                .local_status
                                .as_ref()
                                .filter(|s| s.hf_endpoint_override.is_none()),
                            |d, status| {
                                d.child(compact_switch_row(
                                    "wiz-local-mirror",
                                    "使用国内镜像源 (hf-mirror.com)",
                                    "wiz-mirror-tip",
                                    "中国大陆网络建议开启，可显著提升模型下载速度。",
                                    status.use_china_mirror,
                                    cx.listener(|this, checked: &bool, _, cx| {
                                        this.translation_page
                                            .set_local_china_mirror(&this.bridge, *checked);
                                        cx.notify();
                                    }),
                                ))
                            },
                        );
                } else {
                    fields = fields.child(labeled_stack_with_tip(
                        "API 端点",
                        Some((
                            "wiz-ep-tip",
                            format!("默认地址：{}", default_endpoint(&kind)),
                        )),
                        wiz_endpoint,
                    ));
                    if !is_deepl {
                        fields = fields.child(labeled_stack_with_tip(
                            "模型 ID",
                            Some((
                                "wiz-model-tip",
                                "可直接填写模型名（如 deepseek-chat），或保存后点击「发现模型」在线挑选。"
                                    .into(),
                            )),
                            wiz_model,
                        ));
                    }
                    fields = fields.child(labeled_stack_with_tip(
                        if is_ollama {
                            "访问密钥（可选）"
                        } else {
                            "API 密钥"
                        },
                        Some((
                            "wiz-cred-tip",
                            "密钥将使用系统安全的凭据存储加密/受限保存，不会在界面回显。".into(),
                        )),
                        wiz_cred,
                    ));
                }

                fields
                    .when_some(wizard.error.clone(), |d, err| {
                        d.child(
                            row()
                                .gap(px(6.))
                                .p(px(9.))
                                .rounded(px(8.))
                                .bg(rgba(0xff5f571a))
                                .border_1()
                                .border_color(rgba(0xff5f574d))
                                .child(icon("triangle-alert", 13.).text_color(rgb(danger_fg())))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(rgb(danger_fg()))
                                        .child(err),
                                ),
                        )
                    })
                    .into_any_element()
            }
        };

        let footer = row()
            .justify_between()
            .gap(px(10.))
            .p(px(16.))
            .border_t_1()
            .border_color(rgba(surface_border()))
            .child(
                row().when(step != WizardStep::ChooseCategory, |d| {
                    d.child(
                        Button::new("wiz-back")
                            .ghost()
                            .icon(icon("arrow-left", 13.))
                            .label("上一步")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.translation_page.wizard_back();
                                cx.notify();
                            })),
                    )
                }),
            )
            .child(
                row()
                    .gap(px(8.))
                    .child(
                        Button::new("wiz-cancel")
                            .ghost()
                            .label("取消")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.translation_page
                                    .close_wizard(Some(window), Some(cx));
                                cx.notify();
                            })),
                    )
                    .when(step == WizardStep::Configure, |d| {
                        d.child(
                            Button::new("wiz-full-editor")
                                .outline()
                                .label("完整配置")
                                .tooltip("带着当前预设进入完整界面调整思考深度或默认语言")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.translation_page
                                        .wizard_open_full_editor(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("wiz-save")
                                .primary()
                                .icon(icon("check", 13.))
                                .label("保存并完成")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.translation_page
                                        .wizard_save_provider(&this.bridge, window, cx);
                                    cx.notify();
                                })),
                        )
                    }),
            );

        div()
            .absolute()
            .inset_0()
            .size_full()
            .bg(rgba(overlay()))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                col()
                    .w(px(640_f32.min(width - 32.)))
                    .max_h(px(height - 32.))
                    .bg(rgb(crate::theme::surface()))
                    .rounded(px(18.))
                    .border_1()
                    .border_color(rgba(surface_border()))
                    .shadow_xl()
                    .child(
                        col()
                            .gap(px(12.))
                            .p(px(18.))
                            .border_b_1()
                            .border_color(rgba(surface_border()))
                            .child(
                                row()
                                    .justify_between()
                                    .gap(px(10.))
                                    .child(
                                        row()
                                            .gap(px(8.))
                                            .child(
                                                icon("wand-sparkles", 17.)
                                                    .text_color(rgb(accent_fg())),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(16.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(rgb(text()))
                                                    .child("添加 AI 翻译提供商"),
                                            ),
                                    )
                                    .child(
                                        Button::new("wiz-close")
                                            .small()
                                            .ghost()
                                            .icon(icon("x", 14.))
                                            .tooltip("关闭（Esc）")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.translation_page
                                                    .close_wizard(Some(window), Some(cx));
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(stepper),
                    )
                    .child(
                        div()
                            .id("translation-wizard-scroll")
                            .overflow_y_scroll()
                            .min_h_0()
                            .p(px(20.))
                            .child(body),
                    )
                    .child(footer),
            )
            .into_any_element()
    }

    pub(crate) fn translation_cache_browser(
        &mut self,
        compact: bool,
        current_game_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scope_game_id = current_game_id
            .filter(|gid| !gid.trim().is_empty())
            .or_else(|| self.translation_page.cache.scope_game_id.clone());
        let has_scope = scope_game_id.is_some() || self.translation_page.cache.has_scope();
        let only_current = has_scope && self.translation_page.cache.only_current_game;
        let query_default = self.translation_page.cache.query.clone();
        let query_input = Input::new(&self.translation_page.input(
            "cache-query",
            &query_default,
            "搜索原文、译文、说话人、模型或游戏 ID…",
            window,
            cx,
        ))
        .prefix(icon("search", 13.).text_color(rgb(muted_color())))
        .cleanable(true);

        let cache = &self.translation_page.cache;
        let total_count = cache.total_count;
        let game_count = cache.game_count;
        let total_bytes = cache.total_bytes;
        let loading = cache.loading || self.translation_page.is_pending("translation.cache/list");
        let clear_pending = self.translation_page.is_pending("translation.cache.clear");
        let delete_pending = self.translation_page.is_pending("translation.cache/delete");
        let cache_error = cache.error.clone();
        let cache_query_value = cache.query.clone();
        let entries = cache.entries.clone();

        let refresh_game_id = if only_current {
            scope_game_id.clone()
        } else {
            None
        };

        let mut stats_row = row()
            .gap(px(6.))
            .flex_wrap()
            .child(pill_badge(
                format!("已缓存 {total_count} 条"),
                badge_bg(),
                badge_text(),
                None,
            ))
            .child(pill_badge(
                format!("占用 {}", format_cache_bytes(total_bytes)),
                badge_bg(),
                badge_text(),
                None,
            ));
        if only_current {
            stats_row = stats_row.child(pill_badge(
                format!("当前游戏 {game_count} 条"),
                accent_subtle_bg(),
                accent_fg(),
                Some(accent_subtle_border()),
            ));
        }
        if loading {
            stats_row = stats_row.child(Spinner::new().with_size(px(12.)));
        }

        let mut toolbar = row()
            .gap(px(8.))
            .flex_wrap()
            .child(div().flex_1().min_w(px(180.)).child(query_input))
            .child(
                Button::new("cache-refresh")
                    .small()
                    .icon(icon("refresh-cw", 13.))
                    .label("刷新")
                    .disabled(loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.translation_page
                            .refresh_cache(&this.bridge, refresh_game_id.clone());
                        cx.notify();
                    })),
            );
        if has_scope {
            let next_only = !only_current;
            let clear_gid = scope_game_id.clone();
            toolbar = toolbar
                .child(selectable_chip(
                    "cache-scope-current",
                    "仅当前游戏",
                    only_current,
                    cx.listener(move |this, _, _, cx| {
                        this.translation_page
                            .set_cache_only_current_game(&this.bridge, next_only);
                        cx.notify();
                    }),
                ))
                .child(
                    Button::new("cache-clear-current")
                        .small()
                        .label("清空当前游戏")
                        .disabled(clear_pending)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.translation_page
                                .clear_scoped_cache(&this.bridge, clear_gid.clone());
                            cx.notify();
                        })),
                );
        }
        toolbar = toolbar.child(
            Button::new("cache-clear-all")
                .small()
                .danger()
                .icon(icon("delete", 13.))
                .label("清空全部")
                .disabled(clear_pending)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.translation_page.clear_cache(&this.bridge, None);
                    cx.notify();
                })),
        );

        let entry_rows: Vec<AnyElement> = entries
            .iter()
            .map(|entry| cache_entry_row(entry, compact, delete_pending, cx))
            .collect();
        let list_max_h = if compact { px(240.) } else { px(340.) };
        let entries_view = if entry_rows.is_empty() {
            div()
                .p(px(12.))
                .rounded(px(8.))
                .bg(rgba(card_bg()))
                .border_1()
                .border_color(rgba(card_border()))
                .child(muted(if loading {
                    "正在读取缓存记录…"
                } else if !cache_query_value.trim().is_empty() {
                    "未找到匹配的缓存句子。"
                } else {
                    "当前暂无句子翻译缓存。"
                }))
                .into_any_element()
        } else {
            div()
                .id("cache-entries-scroll")
                .w_full()
                .max_h(list_max_h)
                .overflow_y_scroll()
                .child(col().gap(px(6.)).children(entry_rows))
                .into_any_element()
        };

        col()
            .gap(px(10.))
            .child(stats_row)
            .child(toolbar)
            .when_some(cache_error, |d, err| {
                d.child(
                    row()
                        .gap(px(8.))
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .bg(rgba(0xff5f571a))
                        .border_1()
                        .border_color(rgba(0xff5f574d))
                        .child(icon("triangle-alert", 13.).text_color(rgb(danger_fg())))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(12.))
                                .text_color(rgb(danger_fg()))
                                .child(err),
                        ),
                )
            })
            .child(entries_view)
            .into_any_element()
    }
}

fn provider_sidebar_card(
    provider: &ProviderDto,
    selected: bool,
    cx: &Context<WhiskyApp>,
) -> AnyElement {
    let id = provider.id.clone();
    let cat = provider_category(&provider.kind);
    let (cat_fg, cat_bg, cat_border) = cat.colors();
    let short_kind = provider_short_label(&provider.kind).to_owned();
    let (dot_color, status_tip) = if provider.kind == "localBuiltIn" {
        (installed_fg(), "内置本地模型（无需密钥）")
    } else if provider.has_credential {
        (installed_fg(), "密钥已安全保存")
    } else if provider.kind == "ollama" {
        (cat_fg, "本地 Ollama 服务（免密钥）")
    } else {
        (warning_fg(), "尚未设置 API 密钥")
    };
    let subtitle = match provider
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        Some(m) => format!("{short_kind} · {m}"),
        None => short_kind,
    };

    row()
        .id(format!("translation-provider:{id}"))
        .overflow_hidden()
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
                .w(px(3.))
                .self_stretch()
                .bg(rgb(if selected { accent_fg() } else { cat_fg })),
        )
        .child(
            row()
                .flex_1()
                .min_w_0()
                .gap(px(9.))
                .p(px(9.))
                .child(category_icon_tile_raw(
                    cat_fg,
                    cat_bg,
                    cat_border,
                    provider_icon(&provider.kind),
                    28.,
                    14.,
                ))
                .child(
                    col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.))
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
                                        .text_size(px(12.5))
                                        .text_color(rgb(if selected { accent_fg() } else { text() }))
                                        .child(provider.name.clone()),
                                )
                                .child(
                                    div()
                                        .id(format!("provider-dot:{}", provider.id))
                                        .size(px(7.))
                                        .rounded_full()
                                        .flex_shrink_0()
                                        .bg(rgb(dot_color))
                                        .tooltip(hover_tip(status_tip)),
                                ),
                        )
                        .child(muted(subtitle).text_size(px(11.)).truncate()),
                ),
        )
        .into_any_element()
}

fn wizard_stepper(step: WizardStep) -> AnyElement {
    let steps = [
        (WizardStep::ChooseCategory, "1", "连接方式"),
        (WizardStep::ChoosePreset, "2", "选择服务"),
        (WizardStep::Configure, "3", "快速配置"),
    ];
    let current_idx = match step {
        WizardStep::ChooseCategory => 0,
        WizardStep::ChoosePreset => 1,
        WizardStep::Configure => 2,
    };

    let mut items: Vec<AnyElement> = Vec::new();
    for (idx, (_, num, label)) in steps.into_iter().enumerate() {
        let done = idx < current_idx;
        let active = idx == current_idx;
        let pill = row()
            .gap(px(6.))
            .px(px(9.))
            .py(px(4.))
            .rounded_full()
            .bg(rgba(if active {
                accent_subtle_bg()
            } else if done {
                installed_bg()
            } else {
                badge_bg()
            }))
            .border_1()
            .border_color(rgba(if active {
                accent_subtle_border()
            } else {
                card_border()
            }))
            .child(if done {
                icon("check", 11.)
                    .text_color(rgb(installed_fg()))
                    .into_any_element()
            } else {
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(if active { accent_fg() } else { muted_color() }))
                    .child(num)
                    .into_any_element()
            })
            .child(
                div()
                    .text_size(px(11.5))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(rgb(if active {
                        accent_fg()
                    } else if done {
                        installed_fg()
                    } else {
                        subtext()
                    }))
                    .child(label),
            );
        items.push(pill.into_any_element());
        if idx < 2 {
            items.push(
                div()
                    .flex_1()
                    .h(px(1.))
                    .bg(rgba(if done {
                        accent_subtle_border()
                    } else {
                        divider()
                    }))
                    .into_any_element(),
            );
        }
    }

    row().gap(px(8.)).w_full().children(items).into_any_element()
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
        .label("预设")
        .dropdown_caret(true)
        .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
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

fn hover_tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |window, cx| {
        let content = text.clone();
        Tooltip::element(move |_, _| {
            div()
                .max_w(px(300.))
                .text_size(px(12.))
                .line_height(relative(1.45))
                .child(content.clone())
        })
        .build(window, cx)
    }
}

fn help_tip(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    row()
        .id(id)
        .justify_center()
        .size(px(18.))
        .rounded_full()
        .flex_shrink_0()
        .cursor_pointer()
        .hover(|s| s.bg(rgba(hover())))
        .tooltip(hover_tip(text))
        .child(icon("circle-question-mark", 13.).text_color(rgb(muted_color())))
}

fn category_icon_tile_raw(
    fg: u32,
    bg: u32,
    border: u32,
    icon_name: &str,
    tile_size: f32,
    icon_size: f32,
) -> Div {
    row()
        .justify_center()
        .flex_shrink_0()
        .size(px(tile_size))
        .rounded(px(8.))
        .bg(rgba(bg))
        .border_1()
        .border_color(rgba(border))
        .child(icon(icon_name, icon_size).text_color(rgb(fg)))
}

fn category_pill(cat: ProviderCategory) -> Div {
    let (fg, bg, border) = cat.colors();
    row()
        .flex_shrink_0()
        .gap(px(4.))
        .px(px(8.))
        .py(px(2.))
        .rounded_full()
        .bg(rgba(bg))
        .border_1()
        .border_color(rgba(border))
        .child(icon(cat.icon(), 11.).text_color(rgb(fg)))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(fg))
                .child(cat.label()),
        )
}

fn section_card_v2(
    icon_name: &str,
    icon_fg: u32,
    title: &str,
    help_id: Option<&'static str>,
    help_text: Option<&'static str>,
    right_slot: Option<AnyElement>,
    body: impl IntoElement,
) -> Div {
    let mut header = row()
        .justify_between()
        .gap(px(10.))
        .flex_wrap()
        .child(
            row()
                .gap(px(7.))
                .child(icon(icon_name, 14.).text_color(rgb(icon_fg)))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(13.5))
                        .text_color(rgb(text()))
                        .child(title.to_owned()),
                )
                .when_some(help_id.zip(help_text), |d, (id, tip)| {
                    d.child(help_tip(id, tip))
                }),
        );
    if let Some(right) = right_slot {
        header = header.child(right);
    }

    col()
        .p(px(15.))
        .gap(px(12.))
        .rounded(px(11.))
        .bg(rgba(section_bg()))
        .border_1()
        .border_color(rgba(section_border()))
        .child(header)
        .child(body)
}

fn two_col_row(left: impl IntoElement, right: impl IntoElement) -> Div {
    row()
        .gap(px(12.))
        .items_start()
        .child(div().flex_1().min_w_0().child(left))
        .child(div().flex_1().min_w_0().child(right))
}

fn labeled_stack(label: &str, control: impl IntoElement) -> Div {
    labeled_stack_with_tip(label, None, control)
}

fn labeled_stack_with_tip(
    label: &str,
    tip: Option<(&'static str, String)>,
    control: impl IntoElement,
) -> Div {
    col()
        .gap(px(5.))
        .child(
            row()
                .gap(px(5.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(subtext()))
                        .child(label.to_owned()),
                )
                .when_some(tip, |d, (id, text)| d.child(help_tip(id, text))),
        )
        .child(control)
}

fn compact_switch_row(
    id: &'static str,
    title: &str,
    tip_id: &'static str,
    tip_text: &str,
    checked: bool,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    row()
        .justify_between()
        .gap(px(12.))
        .child(
            row()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(rgb(text()))
                        .child(title.to_owned()),
                )
                .child(help_tip(tip_id, tip_text.to_owned())),
        )
        .child(Switch::new(id).checked(checked).on_change(on_change))
        .into_any_element()
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

fn storage_mode_detail(mode: &str) -> &'static str {
    match mode {
        "localFile" => {
            if cfg!(target_os = "linux") {
                "本地文件模式会以未加密明文将 API 密钥写入本机受限目录（0600 权限），不使用 Secret Service；仅建议可信个人设备使用。切换模式后请先点击「保存翻译设置」，再到提供商页面重新保存密钥。"
            } else {
                "本地文件模式会以未加密明文将 API 密钥写入本机受限目录（0600 权限），可彻底避免钥匙串重复弹窗，但仅建议可信个人设备使用。切换模式后请先点击「保存翻译设置」，再到提供商页面重新保存密钥。"
            }
        }
        "sessionOnly" => {
            if cfg!(target_os = "linux") {
                "仅本次运行模式将密钥保留在当前进程内存中，不写入系统密钥环或磁盘；退出 Maltoria 后密钥将自动清除。"
            } else {
                "仅本次运行模式将密钥保留在当前进程内存中，不写入钥匙串或磁盘；退出 Maltoria 后密钥将自动清除。"
            }
        }
        _ => {
            if cfg!(target_os = "linux") {
                "系统密钥环模式通过 Secret Service 加密保存凭证，需要安装 libsecret-tools 并运行已解锁的密钥环服务。"
            } else {
                "macOS 钥匙串模式会加密保存凭证并启用内存缓存；若为本地 ad-hoc 签名构建，每次重新编译后系统钥匙串可能提示授权一次。"
            }
        }
    }
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

fn format_cache_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn cache_entry_row(
    entry: &CacheEntryDto,
    compact: bool,
    delete_pending: bool,
    cx: &Context<WhiskyApp>,
) -> AnyElement {
    let id = entry.id.clone();
    let model_label = match (
        entry.provider_name.trim(),
        entry.model_id.as_deref().map(str::trim).unwrap_or(""),
    ) {
        ("", "") => None,
        (provider, "") => Some(provider.to_owned()),
        ("", model) => Some(model.to_owned()),
        (provider, model) => Some(format!("{provider} · {model}")),
    };
    let speaker = entry
        .speaker
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let lang_pair =
        (!entry.source_language.trim().is_empty() || !entry.target_language.trim().is_empty())
            .then(|| format!("{} → {}", entry.source_language, entry.target_language));

    let mut badges = row()
        .gap(px(5.))
        .flex_wrap()
        .when_some(speaker, |d, s| {
            d.child(pill_badge(
                s,
                accent_subtle_bg(),
                accent_fg(),
                Some(accent_subtle_border()),
            ))
        })
        .when_some(model_label, |d, m| {
            d.child(pill_badge(m, badge_bg(), badge_text(), None))
        })
        .when_some(lang_pair, |d, lp| {
            d.child(pill_badge(lp, badge_bg(), badge_text(), None))
        })
        .child(pill_badge(
            format!("命中 {} 次", entry.hit_count),
            installed_bg(),
            installed_fg(),
            None,
        ));
    if !compact && !entry.game_id.trim().is_empty() {
        badges = badges.child(pill_badge(
            format!("游戏 {}", entry.game_id),
            badge_bg(),
            badge_text(),
            None,
        ));
    }

    col()
        .gap(px(6.))
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
                .child(badges)
                .child(
                    Button::new(format!("cache-delete:{id}"))
                        .xsmall()
                        .ghost()
                        .label("删除")
                        .disabled(delete_pending)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.translation_page
                                .delete_cache_entry(&this.bridge, id.clone());
                            cx.notify();
                        })),
                ),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(text()))
                .line_height(relative(1.4))
                .child(entry.source_text.clone()),
        )
        .when(!entry.translated_text.is_empty(), |d| {
            d.child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(subtext()))
                    .line_height(relative(1.4))
                    .child(entry.translated_text.clone()),
            )
        })
        .into_any_element()
}
