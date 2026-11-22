//! A small welcome flow, also reused by the source preferences entry point.
use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{prelude::*, *};
use serde_json::json;

use crate::controls::{AutoFocusButton, col, muted, row};
use crate::onboarding::{CATEGORIES, COMPLETED, Onboarding, SOURCES, category_label, source_label};
use crate::state::WhiskyApp;
use crate::theme::{surface, surface_border};

impl WhiskyApp {
    pub(crate) fn open_onboarding(&mut self, welcome: bool, cx: &mut Context<Self>) {
        self.onboarding = Some(Onboarding::new(self.metadata_preferences.clone(), welcome));
        cx.notify();
    }

    fn save_onboarding(&mut self, skip: bool, cx: &mut Context<Self>) {
        let Some(state) = &mut self.onboarding else {
            return;
        };
        if state.saving {
            return;
        }
        let mut values = if skip {
            // Skipping is an explicit no-scraping choice, not consent to defaults.
            state.draft.enabled_sources.clear();
            state.draft.values()
        } else {
            state.draft.values()
        };
        if state.welcome {
            values[COMPLETED] = json!(true);
        }
        state.saving = true;
        state.error = None;
        self.bridge.save_metadata_preferences(values, true);
        cx.notify();
    }

    fn onboarding_source_menu(
        &self,
        category: Option<&'static str>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = self.onboarding.as_ref().expect("visible onboarding");
        let selected = category
            .map(|key| {
                state
                    .draft
                    .category_sources
                    .get(key)
                    .map(String::as_str)
                    .unwrap_or("inherit")
            })
            .unwrap_or(&state.draft.default_source);
        let strings = crate::i18n::s();
        let label = SOURCES
            .iter()
            .find(|(id, _)| *id == selected)
            .map(|(id, _)| source_label(id))
            .unwrap_or(strings.onboarding.follow_default);
        let view = cx.entity().downgrade();
        Button::new(format!(
            "onboarding-source-{}",
            category.unwrap_or("default")
        ))
        .label(format!("{label}  ⌄"))
        .disabled(state.saving)
        .dropdown_menu(move |mut menu, _, _| {
            let follow_default = crate::i18n::s().onboarding.follow_default;
            let choices = category
                .map(|_| vec![("inherit", follow_default)])
                .unwrap_or_default()
                .into_iter()
                .chain(SOURCES.iter().map(|(id, _)| (*id, source_label(id))));
            for (id, label) in choices {
                // Only providers with a character endpoint may be selected.
                if category == Some("characters") && !["inherit", "bangumi", "vndb", "nextmoe"].contains(&id) {
                    continue;
                }
                let view = view.clone();
                menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        if let Some(state) = &mut this.onboarding {
                            if state.saving {
                                return;
                            }
                            if let Some(category) = category {
                                if id == "inherit" {
                                    state.draft.category_sources.remove(category);
                                } else {
                                    state
                                        .draft
                                        .category_sources
                                        .insert(category.into(), id.into());
                                }
                            } else {
                                state.draft.default_source = id.into();
                            }
                        }
                        cx.notify();
                    });
                }));
            }
            menu
        })
        .into_any_element()
    }

    pub(crate) fn onboarding_view(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let strings = crate::i18n::s();
        let state = self.onboarding.as_ref().expect("visible onboarding");
        let introduction = state.welcome && state.step == 0;
        let mut content = col().gap(px(14.));
        if introduction {
            content = content
                .child(
                    div()
                        .text_size(px(26.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(strings.onboarding.welcome_title),
                )
                .child(
                    muted(strings.onboarding.welcome_sub)
                        .line_height(relative(1.6)),
                )
                .child(
                    col()
                        .gap(px(8.))
                        .p(px(16.))
                        .rounded(px(12.))
                        .bg(rgba(crate::theme::card_bg()))
                        .child(strings.onboarding.step1_title)
                        .child(muted(strings.onboarding.step1_desc))
                        .child(strings.onboarding.step2_title)
                        .child(muted(strings.onboarding.step2_desc))
                        .child(strings.onboarding.step3_title)
                        .child(muted(strings.onboarding.step3_desc)),
                )
                .child(muted(strings.onboarding.welcome_note));
        } else {
            content = content
                .child(div().text_size(px(24.)).font_weight(FontWeight::SEMIBOLD).child(strings.onboarding.choose_sources_title))
                .child(muted(strings.onboarding.choose_sources_desc).line_height(relative(1.5)))
                .child(row().justify_between().child(strings.onboarding.default_source).child(self.onboarding_source_menu(None, cx)))
                .child(muted(strings.onboarding.sources_legend).text_size(px(11.)))
                .child(Button::new("onboarding-expand")
                    .label(if state.expanded {
                        strings.onboarding.collapse_categories
                    } else {
                        strings.onboarding.expand_categories
                    })
                    .ghost().disabled(state.saving)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(state) = &mut this.onboarding { state.expanded = !state.expanded; }
                        cx.notify();
                    })));
            if state.expanded {
                for (category, _, _) in CATEGORIES {
                    content = content.child(
                        row()
                            .justify_between()
                            .child(category_label(category))
                            .child(self.onboarding_source_menu(Some(category), cx)),
                    );
                }
                content = content.child(
                    muted(strings.onboarding.characters_support_note)
                        .text_size(px(11.)),
                );
            }
            let mut sources = row().gap(px(8.)).flex_wrap();
            for (id, _) in SOURCES {
                let label = source_label(id);
                let selected = state
                    .draft
                    .enabled_sources
                    .iter()
                    .any(|source| source == id);
                sources = sources.child(
                    Button::new(format!("onboarding-enabled-{id}"))
                        .label(format!("{} {label}", if selected { "✓" } else { "+" }))
                        .disabled(state.saving)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(state) = &mut this.onboarding {
                                if state
                                    .draft
                                    .enabled_sources
                                    .iter()
                                    .any(|source| source == id)
                                {
                                    state.draft.enabled_sources.retain(|source| source != id);
                                } else {
                                    state.draft.enabled_sources.push(id.into());
                                }
                            }
                            cx.notify();
                        })),
                );
            }
            content = content
                .child(muted(strings.onboarding.allowed_platforms).text_size(px(11.)))
                .child(sources)
                .child(
                    muted(if state.draft.enabled_sources.is_empty() {
                        strings.onboarding.scraping_paused_note
                    } else {
                        strings.onboarding.search_note
                    })
                    .text_size(px(11.))
                    .line_height(relative(1.5)),
                );
        }
        if let Some(error) = &state.error {
            content = content.child(muted(crate::i18n::fmt::dialog_save_failed(error)));
        }
        let mut footer = row().gap(px(8.)).justify_end();
        if state.welcome {
            footer = footer.child(
                Button::new("onboarding-skip")
                    .label(strings.onboarding.skip_scraping)
                    .ghost()
                    .disabled(state.saving)
                    .on_click(cx.listener(|this, _, _, cx| this.save_onboarding(true, cx))),
            );
        } else {
            footer = footer.child(
                Button::new("onboarding-cancel")
                    .label(strings.common.cancel)
                    .ghost()
                    .disabled(state.saving)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.onboarding = None;
                        this.focus_handle.focus(window, cx);
                        cx.notify();
                    })),
            );
        }
        if introduction {
            footer = footer.child(
                AutoFocusButton::new(
                    "onboarding-next",
                    Button::new("onboarding-next")
                        .label(strings.onboarding.next_choose_sources)
                        .primary()
                        .disabled(state.saving)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(state) = &mut this.onboarding {
                                state.step = 1;
                            }
                            cx.notify();
                        })),
                )
                .when_focusable(!state.saving),
            );
        } else {
            if state.welcome {
                footer = footer.child(
                    Button::new("onboarding-back")
                        .label(strings.common.previous_step)
                        .ghost()
                        .disabled(state.saving)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(state) = &mut this.onboarding {
                                state.step = 0;
                            }
                            cx.notify();
                        })),
                );
            }
            footer = footer.child(
                AutoFocusButton::new(
                    "onboarding-save",
                    Button::new("onboarding-save")
                        .label(if state.saving {
                            strings.common.saving
                        } else if state.welcome {
                            strings.onboarding.start_using
                        } else {
                            strings.common.save
                        })
                        .primary()
                        .disabled(state.saving)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.save_onboarding(false, cx);
                            this.focus_handle.focus(window, cx);
                        })),
                )
                .when_focusable(!state.saving),
            );
        }
        div()
            .absolute()
            .inset_0()
            .size_full()
            .bg(rgba(crate::theme::overlay()))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                col()
                    .w(px(560_f32.min(width - 32.)))
                    .max_h(px(height - 32.))
                    .bg(rgb(surface()))
                    .rounded(px(18.))
                    .border_1()
                    .border_color(rgba(surface_border()))
                    .shadow_xl()
                    .child(
                        div()
                            .id("onboarding-scroll")
                            .overflow_y_scroll()
                            .min_h_0()
                            .p(px(24.))
                            .child(content),
                    )
                    .child(
                        footer
                            .p(px(16.))
                            .border_t_1()
                            .border_color(rgba(surface_border())),
                    ),
            )
            .into_any_element()
    }
}
