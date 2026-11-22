//! Bottle surfaces: home, configuration, programs, processes, and game rules.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable, Sizable};
use gpui_kit::component::{
    menu::{DropdownMenu, PopupMenuItem},
    switch::Switch,
};
use gpui_kit::{prelude::*, *};

use super::Command;
use crate::config::{self, ConfigField};
use crate::controls::*;
use crate::state::{PreviewPage, PreviewSheet, WhiskyApp};
use crate::theme;

impl WhiskyApp {
    pub(crate) fn bottle_bar(&self, cx: &Context<Self>) -> AnyElement {
        let terminal_enabled = if cfg!(target_os = "linux") {
            let preferred = self.preview.choices.get("terminal")
                .map(|value| value.as_ref()).unwrap_or("auto");
            self.host_terminals.as_ref().is_some_and(|status| status.can_open(preferred))
        } else {
            true
        };
        let strings = crate::i18n::s();
        row()
            .flex_shrink_0()
            .justify_end()
            .flex_wrap()
            .gap(px(9.))
            .p(px(14.))
            .border_t_1()
            .border_color(rgba(theme::card_border()))
            .child(self.action("bottle-c-drive", strings.bottle.open_c_drive, Command::OpenDrive, cx))
            .child(
                Button::new("bottle-terminal")
                    .label(strings.bottle.terminal)
                    .disabled(!terminal_enabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.command(Command::BottleTool("terminal"), cx)
                    })),
            )
            .child(self.action(
                "bottle-tricks",
                strings.bottle.winetricks_ellipsis,
                Command::Sheet(PreviewSheet::Winetricks),
                cx,
            ))
            .child(self.action("bottle-run", strings.bottle.run_ellipsis, Command::BottleRun, cx))
            .when(cfg!(target_os = "linux"), |bar| {
                bar.child(self.action(
                    "bottle-diagnostics",
                    strings.bottle.diagnostics_ellipsis,
                    Command::Sheet(PreviewSheet::Diagnostics),
                    cx,
                ))
            })
            .into_any_element()
    }
    pub(crate) fn preview_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = match self.preview.page {
            PreviewPage::Bottle => match self.visible_load_error() {
                Some(error) => self.library_error_surface(error, cx),
                None => self.bottle_home(window, cx),
            },
            PreviewPage::ConfigureBottle => self.bottle_config(window, cx),
            PreviewPage::Programs => self.programs(cx),
            PreviewPage::Processes => self.processes(cx),
            PreviewPage::GameConfigs => self.game_configs(cx),
            // Library/Statistics/Launchers are rendered by a different surface;
            // if state desyncs and we land here, degrade to the bottle home
            // rather than panic — a transient wrong page is recoverable.
            PreviewPage::Library | PreviewPage::Statistics | PreviewPage::Launchers => {
                self.bottle_home(window, cx)
            }
        };
        let strings = crate::i18n::s();
        col()
            .size_full()
            .child(div().flex_1().min_h_0().child(body))
            .child(if self.preview.page == PreviewPage::ConfigureBottle {
                row()
                    .flex_shrink_0()
                    .justify_end()
                    .flex_wrap()
                    .gap(px(8.))
                    .p(px(14.))
                    .border_t_1()
                    .border_color(rgba(theme::card_border()))
                    .child(self.action(
                        "control",
                        strings.bottle.control_panel,
                        Command::BottleTool("control"),
                        cx,
                    ))
                    .child(self.action(
                        "regedit",
                        strings.bottle.regedit,
                        Command::BottleTool("regedit"),
                        cx,
                    ))
                    .child(self.action(
                        "winecfg",
                        strings.bottle.wine_config,
                        Command::BottleTool("winecfg"),
                        cx,
                    ))
                    .into_any_element()
            } else {
                self.bottle_bar(cx)
            })
            .into_any_element()
    }
    fn bottle_home(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let slot = self.bottle_slot();
        let bottle_games: Vec<_> = self
            .games()
            .iter()
            .enumerate()
            .filter(|(_, game)| game.bottle == slot && !game.is_launcher)
            .collect();
        let total_games = bottle_games.len();
        let pins: Vec<_> = bottle_games
            .into_iter()
            .take(6)
            .map(|(i, game)| {
                let (top, bottom) = theme::placeholder_gradient();
                col()
                    .w(px(110.))
                    .items_center()
                    .child(
                        // Explicit clipped frame: the artwork can never grow
                        // into the label strip below.
        track_key_focus(
            div()
                .id(("pin", i))
                .cursor_pointer()
                .w(px(80.))
                .h(px(108.))
                .rounded(px(9.))
                .overflow_hidden()
                .border_1()
                .border_color(rgba(theme::surface_border()))
                .hover(|d| d.border_color(rgb(theme::ACCENT)))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_detail_from(i, window, cx)
                })),
            ("bottle-pin", i),
            0,
            theme::ACCENT,
            window,
            cx,
        )
                            .child(match self.artwork(i) {
                                Some(path) => self.artwork_image(path)
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                                    .into_any_element(),
                                None => div()
                                    .size_full()
                                    .bg(gradient(top, bottom))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon("package", 22.).text_color(rgb(theme::muted_color())))
                                    .into_any_element(),
                            })
                            .context_menu(self.game_menu(i, cx)),
                    )
                    .child(
                        // Reserved band keeps the two-line label entirely
                        // below the cover; without it a wrapping title hugs
                        // the artwork's bottom edge.
                        div()
                            .w_full()
                            .h(px(34.))
                            .pt(px(8.))
                            .text_size(px(11.))
                            .text_center()
                            .line_clamp(2)
                            .child(game.title.clone()),
                    )
                    .into_any_element()
            })
            .collect();
        let pins_empty = pins.is_empty();
        let strings = crate::i18n::s();
        let nav_items = [
            ("app-window", strings.bottle.installed_programs, strings.bottle.installed_programs_sub, PreviewPage::Programs),
            ("sliders-horizontal", strings.bottle.configure_bottle, strings.bottle.configure_bottle_sub, PreviewPage::ConfigureBottle),
            ("activity", strings.bottle.running_processes, strings.bottle.running_processes_sub, PreviewPage::Processes),
            ("sparkles", strings.bottle.game_configs, strings.bottle.game_configs_sub, PreviewPage::GameConfigs),
        ];
        let mut links = col()
            .rounded(px(12.))
            .overflow_hidden()
            .bg(rgba(theme::section_bg()))
            .border_1()
            .border_color(rgba(theme::section_border()));
        for (i, (symbol, label, subtitle, page)) in nav_items.into_iter().enumerate() {
            if i > 0 {
                links = links.child(div().h(px(1.)).mx(px(16.)).bg(rgba(theme::divider())));
            }
            links = links.child(
                row()
                    .id(("bottle-link", i))
                    .px(px(16.))
                    .py(px(12.))
                    .gap(px(12.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgba(theme::hover())))
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(page, cx)))
                    .child(
                        div()
                            .size(px(32.))
                            .rounded(px(8.))
                            .bg(rgba(theme::accent_subtle_bg()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .child(icon(symbol, 16.).text_color(rgb(theme::accent_fg()))),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(div().font_weight(FontWeight::MEDIUM).child(label))
                            .child(muted(subtitle).text_size(px(11.)).truncate()),
                    )
                    .child(icon("chevron-right", 15.).text_color(rgb(theme::muted_color()))),
            );
        }

        let strings = crate::i18n::s();
        let summary_line = if self.demo {
            strings.bottle.demo_summary.to_owned()
        } else {
            let bottle = self.selected_bottle.and_then(|idx| self.bottles.get(idx));
            let mut parts = vec![crate::i18n::s().library.games_count(total_games)];
            if let Some(bottle) = bottle {
                if let Some(cfg) = self.config.as_ref().filter(|c| c.container == bottle.path && !c.loading) {
                    if let Some(win) = cfg.values.get("windowsVersion").and_then(|v| v.as_str()) {
                        let win_label = match win {
                            "win11" => "Windows 11",
                            "win10" => "Windows 10",
                            "win81" => "Windows 8.1",
                            "win8" => "Windows 8",
                            "win7" => "Windows 7",
                            "winxp" => "Windows XP",
                            other => other,
                        };
                        parts.push(win_label.to_owned());
                    }
                    if let Some(backend) = cfg.values.get("graphicsBackend").and_then(|v| v.as_str()) {
                        parts.push(crate::program_settings::graphics_label(backend).to_owned());
                    }
                }
                parts.push(crate::create_bottle::display_bottle_location(&bottle.path));
            }
            parts.join(" · ")
        };

        let games_section = col()
            .gap(px(12.))
            .child(
                row()
                    .justify_between()
                    .items_center()
                    .child(
                        row()
                            .gap(px(8.))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(strings.bottle.games_in_bottle))
                            .child(muted(total_games.to_string()).text_size(px(11.))),
                    )
                    .when(total_games > 6, |d| {
                        d.child(
                            Button::new("bottle-view-all-games")
                                .label(crate::i18n::s().library.view_all_games_in_library(total_games))
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.preview.page = PreviewPage::Library;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .when(!pins_empty, |d| {
                d.child(row().items_start().flex_wrap().gap(px(14.)).children(pins))
            })
            .when(pins_empty, |d| {
                d.child(
                    row()
                        .justify_between()
                        .items_center()
                        .flex_wrap()
                        .gap(px(12.))
                        .p(px(16.))
                        .rounded(px(12.))
                        .bg(rgba(theme::section_bg()))
                        .border_1()
                        .border_color(rgba(theme::section_border()))
                        .child(
                            col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(4.))
                                .child(div().font_weight(FontWeight::MEDIUM).child(strings.library.bottle_empty_title))
                                .child(muted(strings.bottle.empty_bottle_desc).text_size(px(11.))),
                        )
                        .child(
                            row()
                                .gap(px(8.))
                                .child(self.action(
                                    "bottle-empty-scan",
                                    strings.bottle.scan_folder_ellipsis,
                                    Command::Sheet(PreviewSheet::Scan),
                                    cx,
                                ))
                                .child(self.action(
                                    "bottle-empty-pin",
                                    strings.bottle.add_program_ellipsis,
                                    Command::AddProgram,
                                    cx,
                                )),
                        ),
                )
            });

        self.preview_scroll(
            "bottle-home-scroll",
            col()
                .p(px(26.))
                .gap(px(24.))
                .child(games_section)
                .child(links)
                .child(muted(summary_line).text_size(px(11.)).truncate()),
        )
    }
    /// Configure Bottle. The demo keeps its static design snapshot; a live
    /// bottle renders the bridge schema, one section at a time.
    fn bottle_config(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.demo {
            return self.demo_bottle_config(cx);
        }
        self.apply_pending_input_text(window, cx);
        let strings = crate::i18n::s();
        let Some(container) = self.selected_container() else {
            return self.form(
                "bottle-config-scroll",
                vec![self.section(
                    "config-empty",
                    strings.bottle.configure_bottle,
                    vec![muted(strings.bottle.select_bottle_first).into_any_element()],
                    cx,
                )],
            );
        };
        // A stale page belongs to the previously selected bottle.
        let page = self
            .config
            .clone()
            .filter(|page| page.container == container);
        let Some(page) = page else {
            return self.config_loading(cx);
        };
        if page.loading {
            return self.config_loading(cx);
        }
        if let Some(reason) = &page.error {
            return self.config_error(reason, cx);
        }

        let mut sections = Vec::new();
        for group in config::group_sections(&page.fields) {
            let mut rows = Vec::new();
            for field in group.fields {
                let value = page
                    .values
                    .get(&field.id)
                    .cloned()
                    .unwrap_or_else(|| field.default.clone());
                let enabled = field.enabled(&page.capabilities);
                let has_coreml = page.capabilities.iter().any(|cap| cap == "gdi.coreml");
                rows.push(self.config_row(&container, field, &value, enabled, has_coreml, window, cx));
            }
            sections.push(self.section(group.spec.id, group.spec.display_title(), rows, cx));
        }
        if let Some(engine) = self.engine_section(&container, cx) {
            sections.insert(0, engine);
        }
        sections.push(self.environment_section(&container, cx));
        self.form("bottle-config-scroll", sections)
    }

    fn config_loading(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        self.form(
            "bottle-config-scroll",
            vec![self.section(
                "config-loading",
                strings.bottle.configure_bottle,
                vec![
                    row()
                        .gap(px(8.))
                        .py(px(10.))
                        .child(Spinner::new().with_size(px(14.)))
                        .child(muted(strings.bottle.reading_config))
                        .into_any_element(),
                ],
                cx,
            )],
        )
    }

    fn config_error(&self, reason: &str, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        self.form(
            "bottle-config-scroll",
            vec![self.section(
                "config-error",
                strings.bottle.configure_bottle,
                vec![
                    muted(reason.to_owned()).into_any_element(),
                    row()
                        .py(px(8.))
                        .child(
                            Button::new("config-retry")
                                .label(strings.common.retry)
                                .on_click(cx.listener(|this, _, _, cx| this.reload_config(cx))),
                        )
                        .into_any_element(),
                ],
                cx,
            )],
        )
    }

    /// One schema row. Toggles and pickers commit on click; text and number
    /// rows are debounced by their `InputState` observer.
    fn config_row(
        &mut self,
        container: &str,
        field: &ConfigField,
        value: &serde_json::Value,
        enabled: bool,
        has_coreml: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let strings = crate::i18n::s();
        let b = &strings.bottle;
        // Selected-engine policy: unavailable options stay visibly disabled
        // with the bridge's reason; `unverified` ones show an explicit
        // confirm action — the control itself stays off until the
        // approval lands (`engine_gate_check` queues the write behind it).
        let policy = self.engine_field_policy(container, &field.id).cloned();
        let gated = policy.as_ref().is_some_and(|policy| policy.disabled);
        let confirmable = policy
            .as_ref()
            .is_some_and(|policy| policy.state == "unverified" && policy.can_override);
        // `outer_enabled` is the section's own editability; `enabled` folds
        // the engine gate in. Off/neutral actions stay reachable even while
        // gated — only enabling writes are blocked.
        let outer_enabled = enabled;
        let enabled = enabled && !gated;
        let gate_reason = gated.then(|| {
            policy
                .as_ref()
                .and_then(|policy| policy.reason.clone())
                .unwrap_or_else(|| b.unsupported_by_engine.to_owned())
        });
        match &field.kind {
            config::FieldKind::Toggle => {
                let checked_now = value.as_bool().unwrap_or(false);
                let id = field.id.clone();
                let feature = policy
                    .as_ref()
                    .and_then(|policy| policy.feature.clone())
                    .unwrap_or_default();
                let container_owned = container.to_owned();
                row()
                    .gap(px(20.))
                    .py(px(10.))
                    .items_start()
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(field.display_title().to_owned())
                            .when(field.id == "metalFXSpatial", |d| {
                                d.child(muted(b.metalfx_spatial_desc))
                            })
                            .when(field.id == "metalFX", |d| {
                                d.child(muted(b.metalfx_bridge_desc))
                            })
                            .when(!enabled && gate_reason.is_none(), |d| {
                                d.child(muted(b.needs_wine_host).line_height(relative(1.4)))
                            })
                            .when_some(gate_reason.clone(), |d, reason| {
                                d.child(muted(reason).line_height(relative(1.4))).when(
                                    confirmable,
                                    |d| {
                                        d.child(
                                            muted(b.confirm_required_note)
                                                .line_height(relative(1.4)),
                                        )
                                    },
                                )
                            })
                            // A saved ON the engine does not honor is shown
                            // as an inactive preference, not a live setting.
                            .when(gated && checked_now, |d| {
                                d.child(
                                    muted(b.saved_pref_inactive)
                                        .line_height(relative(1.4)),
                                )
                            }),
                    )
                    .child(
                        col()
                            .gap(px(6.))
                            .child(
                                Switch::new(format!("config-toggle:{}", field.id))
                                    .checked(checked_now)
                                    // Off stays reachable: a gated saved ON
                                    // must remain switchable to OFF.
                                    .disabled(!outer_enabled || (gated && !checked_now))
                                    .on_change(cx.listener(move |this, checked, _, cx| {
                                        this.set_config_value(
                                            &id,
                                            serde_json::Value::Bool(*checked),
                                            cx,
                                        );
                                    })),
                            )
                            .when(confirmable, |d| {
                                let feature = feature.clone();
                                let container = container_owned.clone();
                                let field_id = field.id.clone();
                                d.child(
                                    Button::new(format!("config-confirm:{}", field.id))
                                        .label(b.confirm_and_try_enable)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.request_feature_confirm(
                                                &container,
                                                &feature,
                                                &field_id,
                                                serde_json::Value::Bool(true),
                                                cx,
                                            );
                                        })),
                                )
                            }),
                    )
                    .into_any_element()
            }
            config::FieldKind::Choice { options } => {
                let id = field.id.clone();
                let selected = value.as_str().unwrap_or_default().to_owned();
                let mut title = config::display_value(field, value).unwrap_or_default();
                let choice_policies = policy
                    .as_ref()
                    .map(|policy| policy.choices.clone())
                    .unwrap_or_default();
                // Field-level gate fallback: fields like `dxvkHud` carry the
                // gate on the FIELD, not per choice — every non-neutral
                // option inherits it, neutral/off choices stay selectable.
                let neutral = |id: &str| id.is_empty() || id == "none" || id == "off";
                let policy_for = move |option_id: &str| -> Option<crate::bridge::dto::EnginePolicyDto> {
                    if let Some(policy) = choice_policies.get(option_id) {
                        return Some(policy.clone());
                    }
                    let policy = policy.as_ref()?;
                    if policy.disabled && !neutral(option_id) {
                        return Some(crate::bridge::dto::EnginePolicyDto {
                            disabled: policy.disabled,
                            reason: policy.reason.clone(),
                            state: policy.state.clone(),
                            can_override: policy.can_override,
                            feature: policy.feature.clone(),
                        });
                    }
                    None
                };
                // A saved selection the engine does not enable stays visible
                // but is marked inactive — the runtime ignores it until the
                // user either reverts to a permitted value or confirms the
                // feature.
                let selected_gated = policy_for(selected.as_str())
                    .is_some_and(|policy| policy.disabled);
                if selected_gated {
                    title = crate::i18n::s().program_settings.value_inactive(&title);
                }
                let disabled_reasons: Vec<String> = options
                    .iter()
                    .filter_map(|option| {
                        let policy = policy_for(&option.id)?;
                        policy.disabled.then(|| {
                            crate::i18n::s().bottle.engine_unsupported_reason(option.display_title(&field.id), policy
                                    .reason
                                    .as_deref()
                                    .unwrap_or(b.unsupported_by_engine))
                        })
                    })
                    .collect();
                let options: Vec<(String, String)> = options
                    .iter()
                    .map(|option| (option.id.clone(), option.display_title(&field.id).to_owned()))
                    .collect();
                let view = cx.entity().downgrade();
                row()
                    .py(px(10.))
                    .gap(px(16.))
                    .child(
                        col().flex_1().min_w_0().gap(px(4.)).child(field.display_title().to_owned())
                            .when(field.id == "asfTimestampBase", |d| {
                                d.child(muted(b.asf_timestamp_desc))
                            })
                            .when(field.id == "gdiUpscaler", |d| {
                                d.child(muted(b.gdi_upscaler_desc))
                                    .when(value.as_str() == Some("adaptive"), |d| {
                                        d.child(muted(b.gdi_adaptive_desc))
                                    })
                                    .when(value.as_str() == Some("realcugan"), |d| {
                                        d.child(muted(b.gdi_realcugan_desc))
                                    })
                                    .when(value.as_str() == Some("adaptive-realesrgan"), |d| {
                                        d.child(muted(b.gdi_adaptive_realesrgan_desc))
                                    })
                                    .when(value.as_str() == Some("adaptive-anime4k"), |d| {
                                        d.child(muted(b.gdi_adaptive_anime4k_desc))
                                    })
                                    .when(value.as_str() == Some("adaptive-anime4k-realesrgan"), |d| {
                                        d.child(muted(b.gdi_adaptive_anime4k_realesrgan_desc))
                                    })
                                    .when(value.as_str() == Some("realesrgan"), |d| {
                                        d.child(muted(b.gdi_realesrgan_desc))
                                    })
                                    .when(
                                        !has_coreml
                                            && matches!(
                                                value.as_str(),
                                                Some("realcugan")
                                                    | Some("realesrgan")
                                                    | Some("adaptive")
                                                    | Some("adaptive-realesrgan")
                                                    | Some("adaptive-anime4k")
                                                    | Some("adaptive-anime4k-realesrgan")
                                            ),
                                        |d| {
                                            d.child(muted(b.gdi_no_coreml_warn))
                                        },
                                    )
                            })
                            .when(field.id == "gdiUpscalerCache", |d| {
                                d.child(muted(b.gdi_cache_desc1))
                                    .child(muted(b.gdi_cache_desc2))
                                    .child(muted(b.gdi_cache_desc3))
                            })
                            .when_some(gate_reason.clone(), |d, reason| {
                                d.child(muted(reason).line_height(relative(1.4)))
                                    .when(confirmable, |d| {
                                        d.child(muted(b.confirm_required_note).line_height(relative(1.4)))
                                    })
                            })
                            .children(
                                disabled_reasons
                                    .iter()
                                    .map(|reason| muted(reason.clone()).text_size(px(11.))),
                            ),
                    )
                    .child(
                        Button::new(format!("config-choice:{}", field.id))
                            .label(format!("{title}  ⌄"))
                            // Gated fields keep the menu reachable: neutral
                            // choices (off/none) are always selectable so a
                            // saved gated value can be reverted.
                            .disabled(!outer_enabled)
                            .dropdown_menu(move |mut menu, _, _| {
                                for (option_id, option_title) in &options {
                                    let view = view.clone();
                                    let id = id.clone();
                                    let option_id = option_id.clone();
                                    let choice_disabled = policy_for(option_id.as_str())
                                        .is_some_and(|policy| {
                                            policy.disabled
                                                && !(policy.can_override
                                                    && policy.state == "unverified")
                                        });
                                    let choice_confirmable = policy_for(option_id.as_str())
                                        .is_some_and(|policy| {
                                            policy.disabled
                                                && policy.can_override
                                                && policy.state == "unverified"
                                        });
                                    let option_title = if choice_confirmable {
                                        crate::i18n::s().program_settings.value_requires_confirm(option_title)
                                    } else {
                                        option_title.clone()
                                    };
                                    menu = menu.item(
                                        PopupMenuItem::new(option_title)
                                            .disabled(choice_disabled)
                                            .checked(selected == option_id)
                                            .on_click(move |_, _, cx| {
                                                let _ = view.update(cx, |this, cx| {
                                                    this.set_config_value(
                                                        &id,
                                                        serde_json::Value::String(
                                                            option_id.clone(),
                                                        ),
                                                        cx,
                                                    );
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .into_any_element()
            }
            config::FieldKind::Text | config::FieldKind::Number { .. } => {
                let text = config::display_value(field, value).unwrap_or_default();
                let input = self.config_input(field, &text, window, cx);
                let hint = match &field.kind {
                    config::FieldKind::Number { min, max } => Some(format!("{min}–{max}")),
                    _ => None,
                };
                col()
                    .gap(px(7.))
                    .py(px(8.))
                    .child(
                        row()
                            .gap(px(8.))
                            .child(div().flex_1().child(field.display_title().to_owned()))
                            .when_some(hint, |d, hint| d.child(muted(hint))),
                    )
                    // Clearing a gated saved value (empty write) always
                    // passes the engine gate — keep the field editable.
                    .child(Input::new(&input).disabled(!outer_enabled))
                    .when(field.id == "gdiUpscalerExcludedRegions", |d| {
                        d.child(muted(b.gdi_excluded_desc1))
                            .child(muted(b.gdi_excluded_desc2))
                    })
                    .into_any_element()
            }
        }
    }

    /// Schema text/number fields need an `InputState`, which requires a window;
    /// create one on first render and observe it for debounced writes.
    fn config_input(
        &mut self,
        field: &ConfigField,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.preview.config_inputs.get(&field.id) {
            return input.clone();
        }
        let placeholder = match field.kind {
            config::FieldKind::Number { .. } => crate::i18n::s().bottle.integer_placeholder.to_owned(),
            _ => field.display_title().to_owned(),
        };
        let entity = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(text.to_owned())
        });
        self.preview
            .config_inputs
            .insert(field.id.clone(), entity.clone());
        let id = field.id.clone();
        let subscription = cx.observe(&entity, move |this, input, cx| {
            let text = input.read(cx).value().to_string();
            this.config_text_changed(&id, &text, cx);
        });
        self._subscriptions.push(subscription);
        entity
    }

    /// Rebuild inputs that a rejected write touched. Creating a fresh
    /// `InputState` with the committed value avoids mutating an input while the
    /// root view is rendering (which would re-enter this view's observer).
    fn apply_pending_input_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for field in std::mem::take(&mut self.preview.config_resets) {
            self.preview.config_inputs.remove(&field);
        }
        let strings = crate::i18n::s();
        if let Some((key, value)) = self.env_fill.take() {
            let key_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(strings.bottle.env_key_placeholder)
                    .default_value(key)
            });
            let value_input = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(strings.bottle.env_value_placeholder)
                    .default_value(value)
            });
            self.preview.inputs.insert("env-key", key_input);
            self.preview.inputs.insert("env-value", value_input);
        }
    }

    /// Container environment. The rows and the add form reuse the run-settings
    /// sheet's layout; each save is one `environment.set` call.
    fn environment_section(&self, container: &str, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let values = self.environments.get(container).cloned().unwrap_or_default();
        let mut rows: Vec<AnyElement> = Vec::new();
        if values.is_empty() {
            rows.push(muted(strings.bottle.env_empty).into_any_element());
        }
        for (key, value) in config::environment_rows(&values) {
            let edit_key = key.to_owned();
            let edit_value = value.to_owned();
            let remove_key = key.to_owned();
            rows.push(
                row()
                    .gap(px(12.))
                    .py(px(10.))
                    .child(div().w(px(180.)).truncate().child(key.to_owned()))
                    .child(muted(value.to_owned()).flex_1().truncate())
                    .child(
                        Button::new(format!("env-edit:{key}"))
                            .label(strings.common.edit)
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.begin_environment_edit(&edit_key, &edit_value, cx);
                            })),
                    )
                    .child(
                        Button::new(format!("env-remove:{key}"))
                            .label(strings.common.remove)
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_environment(&remove_key, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        rows.push(
            row()
                .gap(px(10.))
                .py(px(10.))
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.preview.inputs["env-key"])),
                )
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.preview.inputs["env-value"])),
                )
                .child(
                    Button::new("env-save")
                        .label(strings.bottle.env_save)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let key = this.preview.inputs["env-key"].read(cx).value().to_string();
                            let value =
                                this.preview.inputs["env-value"].read(cx).value().to_string();
                            this.save_environment(&key, &value, cx);
                        })),
                )
                .into_any_element(),
        );
        if let Some(reason) = self.environment_error.get(container) {
            rows.push(muted(reason.clone()).into_any_element());
        }
        self.section("environment", strings.bottle.env_section, rows, cx)
    }

    fn demo_bottle_config(&self, cx: &Context<Self>) -> AnyElement {
        self.form(
            "bottle-config-scroll",
            vec![
                self.section(
                    "wine",
                    "Wine",
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.choice(
                            "windows",
                            crate::i18n::config::field_title("config.field.windowsVersion", "windowsVersion", "Windows"),
                            &["Windows 11", "Windows 10", "Windows 7", "Windows XP"],
                            cx,
                        ),
                        self.field("build", crate::i18n::tr("内部版本号", "ビルド番号", "Build Version", "빌드 버전")),
                        self.choice("retina", crate::i18n::tr("Retina 模式", "Retina モード", "Retina Mode", "Retina 모드"), &[crate::i18n::s().common.off, crate::i18n::s().common.on], cx),
                        self.choice("sync", crate::i18n::config::field_title("config.field.enhancedSync", "enhancedSync", "Sync"), &["MSync", "ESync", crate::i18n::s().common.off], cx),
                        self.field("dpi", crate::i18n::tr("屏幕分辨率（DPI）", "画面解像度 (DPI)", "Screen Resolution (DPI)", "화면 해상도 (DPI)")),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![
                        self.choice(
                            "windows",
                            crate::i18n::config::field_title("config.field.windowsVersion", "windowsVersion", "Windows"),
                            &["Windows 11", "Windows 10", "Windows 7", "Windows XP"],
                            cx,
                        ),
                        self.field("build", crate::i18n::tr("内部版本号", "ビルド番号", "Build Version", "빌드 버전")),
                        self.choice("sync", crate::i18n::config::field_title("config.field.enhancedSync", "enhancedSync", "Sync"), &["FSync", "ESync", crate::i18n::s().common.off], cx),
                        self.field("dpi", crate::i18n::tr("屏幕分辨率（DPI）", "画面解像度 (DPI)", "Screen Resolution (DPI)", "화면 해상도 (DPI)")),
                    ],
                    cx,
                ),
                self.section(
                    "launcher",
                    crate::i18n::config::section_title("launcher"),
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.toggle(
                            "launcher-mode",
                            crate::i18n::config::field_title("config.field.launcherCompatibilityMode", "launcherCompatibilityMode", "Launcher"),
                            crate::i18n::tr("为常见游戏启动器使用兼容设置。", "一般的なゲームランチャー向けの互換設定を使用します。", "Use compatibility settings for common game launchers.", "일반 게임 런처에 호환 설정을 사용합니다."),
                            cx,
                        ),
                        self.choice("launcher-type", crate::i18n::config::field_title("config.field.launcherMode", "launcherMode", "Mode"), &[crate::i18n::s().common.auto, crate::i18n::tr("手动", "手動", "Manual", "수동")], cx),
                        self.toggle("gpu-spoof", crate::i18n::config::field_title("config.field.gpuSpoofing", "gpuSpoofing", "GPU"), "", cx),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![
                        self.toggle(
                            "launcher-mode",
                            crate::i18n::config::field_title("config.field.launcherCompatibilityMode", "launcherCompatibilityMode", "Launcher"),
                            crate::i18n::tr("为常见游戏启动器使用兼容设置。", "一般的なゲームランチャー向けの互換設定を使用します。", "Use compatibility settings for common game launchers.", "일반 게임 런처에 호환 설정을 사용합니다."),
                            cx,
                        ),
                        self.toggle("gpu-spoof", crate::i18n::config::field_title("config.field.gpuSpoofing", "gpuSpoofing", "GPU"), "", cx),
                    ],
                    cx,
                ),
                self.section(
                    "input",
                    crate::i18n::config::section_title("input"),
                    vec![
                        self.toggle("controller", crate::i18n::config::field_title("config.field.controllerCompatibilityMode", "controllerCompatibilityMode", "Controller"), "", cx),
                        self.toggle("hid", crate::i18n::config::field_title("config.field.disableHIDAPI", "disableHIDAPI", "HIDAPI"), "", cx),
                        self.toggle("background-input", crate::i18n::config::field_title("config.field.allowBackgroundEvents", "allowBackgroundEvents", "Background"), "", cx),
                    ],
                    cx,
                ),
                self.section(
                    "graphics",
                    crate::i18n::config::section_title("graphics"),
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.choice("graphics-mode", crate::i18n::tr("设置模式", "設定モード", "Mode", "설정 모드"), &[crate::i18n::tr("简单", "簡易", "Simple", "간단히"), crate::i18n::tr("高级", "詳細", "Advanced", "고급")], cx),
                        self.choice(
                            "backend",
                            crate::i18n::config::field_title("config.field.graphicsBackend", "graphicsBackend", "Backend"),
                            &[crate::i18n::s().program_settings.gfx_recommended, "D3DMetal", "DXVK", "WineD3D"],
                            cx,
                        ),
                        self.toggle("metalfx", crate::i18n::config::field_title("config.field.metalFX", "metalFX", "MetalFX"), "", cx),
                        self.choice(
                            "gdi-upscaler",
                            crate::i18n::config::field_title("config.field.gdiUpscaler", "gdiUpscaler", "Upscaler"),
                            &[
                                crate::i18n::s().common.off,
                                "MetalFX Spatial",
                                "Anime4K",
                                "Real-CUGAN",
                                crate::i18n::config::option_title("config.option.gdiUpscaler.adaptive", "adaptive", "Adaptive"),
                                "AnimeVideo-v3",
                                crate::i18n::config::option_title("config.option.gdiUpscaler.adaptive-realesrgan", "adaptive-realesrgan", "Adaptive"),
                            ],
                            cx,
                        ),
                        self.toggle("metal4", "Metal 4", "", cx),
                        self.toggle("framegen", crate::i18n::config::field_title("config.field.frameGeneration", "frameGeneration", "FrameGen"), "", cx),
                        self.action(
                            "graphics-detail",
                            crate::i18n::tr("高级图形设置…", "詳細グラフィックス設定…", "Advanced Graphics…", "고급 그래픽 설정…"),
                            Command::Sheet(PreviewSheet::Graphics),
                            cx,
                        ),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![
                        self.choice("graphics-mode", crate::i18n::tr("设置模式", "設定モード", "Mode", "설정 모드"), &[crate::i18n::tr("简单", "簡易", "Simple", "간단히"), crate::i18n::tr("高级", "詳細", "Advanced", "고급")], cx),
                        self.choice(
                            "backend",
                            crate::i18n::config::field_title("config.field.graphicsBackend", "graphicsBackend", "Backend"),
                            &[crate::i18n::s().program_settings.gfx_recommended, "DXVK", "WineD3D"],
                            cx,
                        ),
                        self.action(
                            "graphics-detail",
                            crate::i18n::tr("高级图形设置…", "詳細グラフィックス設定…", "Advanced Graphics…", "고급 그래픽 설정…"),
                            Command::Sheet(PreviewSheet::Graphics),
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "audio",
                    crate::i18n::config::section_title("audio"),
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.info(
                            crate::i18n::tr("默认输出设备", "既定の出力デバイス", "Default Output Device", "기본 출력 장치"),
                            crate::i18n::tr("系统默认输出 · 48,000 Hz · 立体声", "システム既定 · 48,000 Hz · ステレオ", "System Default · 48,000 Hz · Stereo", "시스템 기본값 · 48,000 Hz · 스테레오"),
                        ),
                        self.choice("audio-mode", crate::i18n::tr("设置模式", "設定モード", "Mode", "설정 모드"), &[crate::i18n::tr("简单", "簡易", "Simple", "간단히"), crate::i18n::tr("高级", "詳細", "Advanced", "고급")], cx),
                        self.choice(
                            "audio-driver",
                            crate::i18n::config::field_title("config.field.audioDriver", "audioDriver", "Audio Driver"),
                            &[crate::i18n::s().common.auto, "CoreAudio", crate::i18n::s().program_settings.dll_disabled],
                            cx,
                        ),
                        self.action(
                            "audio-test",
                            crate::i18n::tr("测试音频", "オーディオテスト", "Test Audio", "오디오 테스트"),
                            Command::Sheet(PreviewSheet::Audio),
                            cx,
                        ),
                        self.action(
                            "audio-diagnosis",
                            crate::i18n::tr("音频故障排查…", "オーディオ診断…", "Audio Troubleshooting…", "오디오 문제 해결…"),
                            Command::Sheet(PreviewSheet::Audio),
                            cx,
                        ),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![
                        self.info(
                            crate::i18n::tr("默认输出设备", "既定の出力デバイス", "Default Output Device", "기본 출력 장치"),
                            crate::i18n::tr("系统默认输出 · 48,000 Hz · 立体声", "システム既定 · 48,000 Hz · ステレオ", "System Default · 48,000 Hz · Stereo", "시스템 기본값 · 48,000 Hz · 스테레오"),
                        ),
                        self.choice("audio-mode", crate::i18n::tr("设置模式", "設定モード", "Mode", "설정 모드"), &[crate::i18n::tr("简单", "簡易", "Simple", "간단히"), crate::i18n::tr("高级", "詳細", "Advanced", "고급")], cx),
                        self.choice(
                            "audio-driver",
                            crate::i18n::config::field_title("config.field.audioDriver", "audioDriver", "Audio Driver"),
                            &[crate::i18n::s().common.auto, "PulseAudio", "ALSA", crate::i18n::s().program_settings.dll_disabled],
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "resolution",
                    crate::i18n::config::section_title("display"),
                    vec![
                        self.toggle(
                            "virtual-desktop",
                            crate::i18n::config::field_title("config.field.virtualDesktopEnabled", "virtualDesktopEnabled", "Virtual Desktop"),
                            crate::i18n::tr("下次启动时生效。", "次回起動時から適用されます。", "Takes effect on next launch.", "다음 실행부터 적용됩니다."),
                            cx,
                        ),
                        self.choice(
                            "desktop-size",
                            crate::i18n::config::field_title("config.field.resolutionPreset", "resolutionPreset", "Resolution"),
                            &[
                                crate::i18n::config::option_title("config.option.resolutionPreset.matchDisplay", "matchDisplay", "Match Display"),
                                "1920 × 1080",
                                "1280 × 720",
                                crate::i18n::s().common.custom,
                            ],
                            cx,
                        ),
                        self.field("desktop-width", crate::i18n::config::field_title("config.field.customResolutionWidth", "customResolutionWidth", "Width")),
                        self.field("desktop-height", crate::i18n::config::field_title("config.field.customResolutionHeight", "customResolutionHeight", "Height")),
                    ],
                    cx,
                ),
                self.section(
                    "performance",
                    crate::i18n::config::section_title("performance"),
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.toggle(
                            "shader",
                            crate::i18n::config::field_title("config.field.shaderCacheEnabled", "shaderCacheEnabled", "Shader Cache"),
                            crate::i18n::tr("缓存编译后的着色器，减少重复编译。", "コンパイル済みシェーダーをキャッシュして再コンパイルを減らします。", "Cache compiled shaders to reduce recompilation.", "컴파일된 셰이더를 캐시하여 재컴파일을 줄입니다."),
                            cx,
                        ),
                        self.toggle(
                            "appnap",
                            crate::i18n::config::field_title("config.field.disableAppNap", "disableAppNap", "Disable App Nap"),
                            crate::i18n::tr("游戏运行期间防止应用被系统暂停。", "ゲーム実行中にシステムによるアプリの一時停止を防ぎます。", "Prevent macOS from throttling while a game is running.", "게임 실행 중 시스템이 앱을 일시 중단하지 않도록 방지합니다."),
                            cx,
                        ),
                        self.action(
                            "vcredist",
                            crate::i18n::tr("安装 Visual C++ 运行库…", "Visual C++ ランタイムを導入…", "Install Visual C++ Runtime…", "Visual C++ 런타임 설치…"),
                            Command::Sheet(PreviewSheet::Winetricks),
                            cx,
                        ),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![
                        self.toggle(
                            "shader",
                            crate::i18n::config::field_title("config.field.shaderCacheEnabled", "shaderCacheEnabled", "Shader Cache"),
                            crate::i18n::tr("缓存编译后的着色器，减少重复编译。", "コンパイル済みシェーダーをキャッシュして再コンパイルを減らします。", "Cache compiled shaders to reduce recompilation.", "컴파일된 셰이더를 캐시하여 재컴파일을 줄입니다."),
                            cx,
                        ),
                        self.action(
                            "vcredist",
                            crate::i18n::tr("安装 Visual C++ 运行库…", "Visual C++ ランタイムを導入…", "Install Visual C++ Runtime…", "Visual C++ 런타임 설치…"),
                            Command::Sheet(PreviewSheet::Winetricks),
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "dll",
                    crate::i18n::s().dialogs.sheet_dll,
                    vec![
                        self.info("d3dcompiler_47", crate::i18n::tr("原生优先，内建后备", "ネイティブ優先、ビルトイン代替", "Native then Builtin", "네이티브 우선, 내장 대체")),
                        self.action(
                            "dll-edit",
                            crate::i18n::tr("编辑 DLL 覆盖…", "DLL オーバーライドを編集…", "Edit DLL Overrides…", "DLL 재정의 편집…"),
                            Command::Sheet(PreviewSheet::Dll),
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "dependencies",
                    crate::i18n::tr("依赖组件", "依存コンポーネント", "Dependencies", "종속 구성 요소"),
                    vec![
                        self.info("Visual C++ 2015–2022", crate::i18n::s().common.installed),
                        self.action(
                            "deps",
                            crate::i18n::tr("管理组件…", "コンポーネントを管理…", "Manage Components…", "구성 요소 관리…"),
                            Command::Sheet(PreviewSheet::Winetricks),
                            cx,
                        ),
                    ],
                    cx,
                ),
                self.section(
                    "diagnostics",
                    crate::i18n::tr("诊断", "診断", "Diagnostics", "진단"),
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.action(
                            "guided",
                            crate::i18n::tr("开始引导式故障排查…", "ガイド付き診断を開始…", "Start Guided Troubleshooting…", "안내형 문제 해결 시작…"),
                            Command::Sheet(PreviewSheet::Diagnostics),
                            cx,
                        ),
                        self.action("export", crate::i18n::tr("导出诊断报告…", "診断レポートをエクスポート…", "Export Diagnostics…", "진단 보고서 내보내기…"), Command::ExportDiagnostics, cx),
                        self.action("repair", crate::i18n::tr("修复容器…", "コンテナを修復…", "Repair Bottle…", "컨테이너 복구…"), Command::BottleRepair, cx),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![
                        self.action("export", crate::i18n::tr("导出诊断报告…", "診断レポートをエクスポート…", "Export Diagnostics…", "진단 보고서 내보내기…"), Command::ExportDiagnostics, cx),
                    ],
                    cx,
                ),
                self.section(
                    "cleanup",
                    crate::i18n::tr("存储与清理", "ストレージと整理", "Storage & Cleanup", "저장 공간 및 정리"),
                    #[cfg(not(target_os = "linux"))]
                    vec![
                        self.info(crate::i18n::tr("容器大小", "コンテナサイズ", "Bottle Size", "컨테이너 크기"), "2.4 GB"),
                        self.action(
                            "cache-clean",
                            crate::i18n::tr("清理着色器缓存…", "シェーダーキャッシュを削除…", "Clean Shader Cache…", "셰이더 캐시 지우기…"),
                            Command::CleanShaderCache,
                            cx,
                        ),
                    ],
                    #[cfg(target_os = "linux")]
                    vec![self.info(crate::i18n::tr("容器大小", "コンテナサイズ", "Bottle Size", "컨테이너 크기"), "2.4 GB")],
                    cx,
                ),
            ],
        )
    }
    /// Installed programs. The demo keeps its static design snapshot; a live
    /// bottle renders `programs.list` with pin/block/Play rows.
    fn programs(&self, cx: &Context<Self>) -> AnyElement {
        if self.demo {
            return self.demo_programs(cx);
        }
        let Some(container) = self.selected_container() else {
            return self.form(
                "programs-scroll",
                vec![self.section(
                    "program-list",
                    crate::i18n::s().bottle.programs_title,
                    vec![muted(crate::i18n::s().bottle.select_bottle_first).into_any_element()],
                    cx,
                )],
            );
        };
        let page = self
            .programs
            .as_ref()
            .filter(|page| page.container == container);
        let Some(page) = page else {
            return self.programs_status(crate::i18n::s().bottle.reading_programs, cx);
        };
        if page.loading {
            return self.programs_status(crate::i18n::s().bottle.reading_programs, cx);
        }
        if let Some(reason) = &page.error {
            return self.form(
                "programs-scroll",
                vec![self.section(
                    "program-list",
                    crate::i18n::s().bottle.programs_title,
                    vec![
                        muted(reason.clone()).into_any_element(),
                        row()
                            .py(px(8.))
                            .child(
                                Button::new("programs-retry")
                                    .label(crate::i18n::s().common.retry)
                                    .on_click(cx.listener(|this, _, _, cx| this.reload_programs(cx))),
                            )
                            .into_any_element(),
                    ],
                    cx,
                )],
            );
        }
        let mut rows: Vec<AnyElement> = vec![row()
            .justify_end()
            .py(px(8.))
            .child(self.action(
                "add-program",
                crate::i18n::s().library.add_program_btn,
                Command::AddProgram,
                cx,
            ))
            .into_any_element()];
        if page.table.is_empty() {
            rows.push(muted(crate::programs::empty_programs()).into_any_element());
        }
        for program in page.table.ordered() {
            let path = program.path.clone();
            let pin_path = path.clone();
            let block_path = path.clone();
            let play_path = path.clone();
            let pinned = program.pinned;
            let blocked = program.blocked;
            let launchable = program.available && !program.blocked;
            let (top, bottom) = theme::placeholder_gradient();
            rows.push(
                row()
                    .id(path.clone())
                    .gap(px(15.))
                    .p(px(13.))
                    .border_b_1()
                    .border_color(rgba(theme::divider()))
                    .child(
                        div()
                            .w(px(32.))
                            .h(px(40.))
                            .rounded(px(4.))
                            .bg(gradient(top, bottom))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("package", 14.).text_color(rgb(theme::muted_color()))),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(
                                row()
                                    .gap(px(6.))
                                    .child(div().truncate().font_weight(FontWeight::MEDIUM).child(program.name.clone()))
                                    .when(pinned, |d| {
                                        d.child(
                                            div()
                                                .px(px(6.))
                                                .py(px(1.))
                                                .rounded(px(4.))
                                                .bg(rgba(theme::accent_subtle_bg()))
                                                .text_size(px(10.))
                                                .text_color(rgb(theme::accent_fg()))
                                                .child(crate::i18n::s().bottle.pinned_badge),
                                        )
                                    })
                                    .when(blocked, |d| {
                                        d.child(
                                            div()
                                                .px(px(6.))
                                                .py(px(1.))
                                                .rounded(px(4.))
                                                .bg(rgba(theme::badge_bg()))
                                                .text_size(px(10.))
                                                .text_color(rgb(theme::muted_color()))
                                                .child(crate::i18n::s().bottle.blocked_badge),
                                        )
                                    }),
                            )
                            .child(muted(path.clone()).text_size(px(11.)).truncate()),
                    )
                    .child(
                        Button::new(format!("program-pin:{path}"))
                            .label(if pinned {
                                crate::i18n::s().bottle.unpin
                            } else {
                                crate::i18n::s().bottle.pin
                            })
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_program_pin(&pin_path, cx);
                            })),
                    )
                    .child(
                        Button::new(format!("program-block:{path}"))
                            .label(if blocked {
                                crate::i18n::s().bottle.unblock
                            } else {
                                crate::i18n::s().bottle.block
                            })
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_program_block(&block_path, cx);
                            })),
                    )
                    .when(launchable, |d| {
                        d.child(
                            Button::new(format!("program-play:{path}"))
                                .label(crate::i18n::s().common.run)
                                .ghost()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.play_program(&play_path, cx);
                                })),
                        )
                    })
                    .into_any_element(),
            );
        }
        self.form(
            "programs-scroll",
            vec![self.section("program-list", crate::i18n::s().bottle.programs_title, rows, cx)],
        )
    }

    fn programs_status(&self, message: &'static str, cx: &Context<Self>) -> AnyElement {
        self.form(
            "programs-scroll",
            vec![self.section(
                "program-list",
                crate::i18n::s().bottle.programs_title,
                vec![
                    row()
                        .gap(px(8.))
                        .py(px(10.))
                        .child(Spinner::new().with_size(px(14.)))
                        .child(muted(message))
                        .into_any_element(),
                ],
                cx,
            )],
        )
    }

    fn demo_programs(&self, cx: &Context<Self>) -> AnyElement {
        let slot = self.bottle_slot();
        let rows = self
            .games()
            .iter()
            .enumerate()
            .filter(|(_, game)| game.bottle == slot)
            .map(|(i, game)| {
                let path = format!("C:\\Games\\{}\\game.exe", i + 1);
                let (top, bottom) = theme::placeholder_gradient();
                row()
                    .id(game.id.clone())
                    .gap(px(15.))
                    .p(px(13.))
                    .border_b_1()
                    .border_color(rgba(theme::divider()))
                    .child(match self.artwork(i) {
                        Some(artwork) => self.artwork_image(artwork)
                            .w(px(32.))
                            .h(px(40.))
                            .rounded(px(4.))
                            .object_fit(ObjectFit::Cover)
                            .into_any_element(),
                        None => div()
                            .w(px(32.))
                            .h(px(40.))
                            .rounded(px(4.))
                            .bg(gradient(top, bottom))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("package", 14.).text_color(rgb(theme::muted_color())))
                            .into_any_element(),
                    })
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(div().truncate().child(game.title.clone()))
                            .when(!path.is_empty(), |d| d.child(muted(path).truncate())),
                    )
                    .child(self.game_options(i, cx))
                    .context_menu(self.game_menu(i, cx))
                    .into_any_element()
            })
            .collect();
        self.form(
            "programs-scroll",
            vec![self.section("program-list", crate::i18n::s().bottle.programs_title, rows, cx)],
        )
    }
    /// Running processes. The demo keeps its static design snapshot; a live
    /// bottle polls `processes.list` and renders each row in the demo list's
    /// two-column treatment with a kill action. The form scrolls, so the row
    /// count is unbounded.
    fn processes(&self, cx: &Context<Self>) -> AnyElement {
        if self.demo {
            return self.demo_processes(cx);
        }
        let Some(container) = self.selected_container() else {
            return self.form(
                "processes-scroll",
                vec![self.section(
                    "process-list",
                    crate::i18n::s().bottle.running_processes,
                    vec![muted(crate::i18n::s().bottle.select_bottle_first).into_any_element()],
                    cx,
                )],
            );
        };
        let page = self
            .processes
            .as_ref()
            .filter(|page| page.container == container);
        let has_processes = page.is_some_and(|p| !p.table.is_empty());
        let mut rows: Vec<AnyElement> = vec![
            row()
                .gap(px(18.))
                .py(px(8.))
                .border_b_1()
                .border_color(rgba(theme::divider()))
                .child(muted(crate::i18n::s().bottle.process_name).flex_1().font_weight(FontWeight::SEMIBOLD).text_size(px(11.)))
                .child(muted(crate::i18n::s().bottle.pid_memory).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD))
                .into_any_element(),
        ];
        match page {
            None => rows.push(self.process_status_row()),
            Some(page) => {
                if let Some(reason) = &page.error {
                    rows.push(muted(reason.clone()).py(px(10.)).into_any_element());
                } else if page.table.is_empty() {
                    if page.loading {
                        rows.push(self.process_status_row());
                    } else {
                        rows.push(muted(crate::i18n::s().bottle.no_running_processes).py(px(14.)).into_any_element());
                    }
                }
                for process in page.table.rows() {
                    let pid = process.pid;
                    let value = match &process.memory {
                        Some(memory) => format!("{pid} · {memory}"),
                        None => format!("{pid}"),
                    };
                    rows.push(
                        row()
                            .gap(px(16.))
                            .py(px(9.))
                            .border_b_1()
                            .border_color(rgba(theme::divider()))
                            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(process.name.clone()))
                            .child(muted(value).font_family("monospace").text_size(px(11.)))
                            .child(
                                Button::new(format!("process-kill:{pid}"))
                                    .label(crate::i18n::s().bottle.kill_process)
                                    .ghost()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.kill_process(pid, cx);
                                    })),
                            )
                            .into_any_element(),
                    );
                }
            }
        }
        rows.push(
            row()
                .justify_end()
                .gap(px(8.))
                .pt(px(10.))
                .pb(px(4.))
                .child(
                    Button::new("refresh-processes")
                        .label(crate::i18n::s().common.refresh)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_processes(cx))),
                )
                .child(
                    Button::new("stop-processes")
                        .label(crate::i18n::s().bottle.stop_all_processes)
                        .danger()
                        .disabled(!has_processes)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.command(Command::StopAllProcesses, cx);
                        })),
                )
                .into_any_element(),
        );
        self.form(
            "processes-scroll",
            vec![self.section("process-list", crate::i18n::s().bottle.running_processes, rows, cx)],
        )
    }

    fn process_status_row(&self) -> AnyElement {
        row()
            .gap(px(8.))
            .py(px(10.))
            .child(Spinner::new().with_size(px(14.)))
            .child(muted(crate::i18n::s().bottle.reading_processes))
            .into_any_element()
    }

    fn demo_processes(&self, cx: &Context<Self>) -> AnyElement {
        self.form(
            "processes-scroll",
            vec![self.section(
                "process-list",
                crate::i18n::s().bottle.running_processes,
                vec![
                    row()
                        .gap(px(18.))
                        .py(px(8.))
                        .border_b_1()
                        .border_color(rgba(theme::divider()))
                        .child(muted(crate::i18n::s().bottle.process_name).flex_1().font_weight(FontWeight::SEMIBOLD).text_size(px(11.)))
                        .child(muted(crate::i18n::s().bottle.pid_memory).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD))
                        .into_any_element(),
                    muted(crate::i18n::s().bottle.no_running_processes).py(px(14.)).into_any_element(),
                    row()
                        .justify_end()
                        .gap(px(8.))
                        .pt(px(10.))
                        .pb(px(4.))
                        .child(self.action(
                            "refresh-processes",
                            crate::i18n::s().common.refresh,
                            Command::RefreshProcesses,
                            cx,
                        ))
                        .child(
                            Button::new("stop-processes")
                                .label(crate::i18n::s().bottle.stop_all_processes)
                                .danger()
                                .disabled(true)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.command(Command::StopAllProcesses, cx);
                                })),
                        )
                        .into_any_element(),
                ],
                cx,
            )],
        )
    }
    fn game_configs(&self, cx: &Context<Self>) -> AnyElement {
        let slot = self.bottle_slot();
        let games: Vec<(usize, &crate::model::Game)> = self
            .games()
            .iter()
            .enumerate()
            .filter(|(_, game)| slot == 0 || game.bottle == slot)
            .collect();
        let mut rows: Vec<AnyElement> = vec![muted(crate::i18n::s().bottle.gamedb_desc)
        .text_size(px(11.))
        .into_any_element()];
        if games.is_empty() {
            rows.push(muted(crate::i18n::s().bottle.gamedb_empty).into_any_element());
        }
        for (index, game) in games {
            rows.push(
                row()
                    .gap(px(14.))
                    .p(px(11.))
                    .border_b_1()
                    .border_color(rgba(theme::divider()))
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(div().truncate().child(game.title.clone()))
                            .when(!game.is_folder, |d| {
                                d.child(
                                    muted(
                                        game.executable
                                            .as_ref()
                                            .and_then(|path| path.file_name())
                                            .map(|name| name.to_string_lossy().into_owned())
                                            .unwrap_or_default(),
                                    )
                                    .text_size(px(11.)),
                                )
                            }),
                    )
                    .child(
                        Button::new(("game-info", index))
                            .label(crate::i18n::s().bottle.info_and_config_ellipsis)
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.command(Command::GameInfo(index), cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        rows.push(
            row()
                .justify_end()
                .py(px(8.))
                .child(self.action(
                    "sync-metadata",
                    crate::i18n::s().library.sync_metadata,
                    Command::SyncMetadata,
                    cx,
                ))
                .into_any_element(),
        );
        self.form(
            "game-configs-scroll",
            vec![self.section("game-rules", crate::i18n::s().bottle.game_configs, rows, cx)],
        )
    }
}

impl WhiskyApp {
    /// The bound Wine engine row on the bottle configuration page: current
    /// engine, availability and a picker. Rebinding an existing prefix goes
    /// through `EngineDialogKind::Select`; on old helpers without the API the
    /// whole section disappears.
    fn engine_section(&self, container: &str, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.service_available("wine.engines.selection") {
            return None;
        }
        let selection = self.engine_selections.get(container);
        let engine = selection.map(|selection| &selection.engine);
        let current_id = selection
            .map(|selection| selection.engine_id.clone())
            .unwrap_or_else(|| "managed".to_owned());
        let title = engine
            .map(|engine| engine.name.clone())
            .unwrap_or_else(|| "Whisky Wine".to_owned());
        let unavailable_reason = engine.and_then(|engine| engine.error.clone());
        let has_engines = !self.wine_engines.is_empty();
        let view = cx.entity().downgrade();
        let container_owned = container.to_owned();
        let engines = self.wine_engines.clone();

        let mut rows = vec![self.info(
            crate::i18n::s().bottle.current_engine,
            if engine.is_some_and(|engine| engine.available) || engine.is_none() {
                title.clone()
            } else {
                crate::i18n::s().bottle.engine_unavailable_item(&title)
            },
        )];
        if let Some(reason) = unavailable_reason {
            rows.push(muted(reason).text_size(px(11.)).into_any_element());
        }
        rows.push(
            row()
                .py(px(6.))
                .child(
                    Button::new("engine-select")
                        .label(crate::i18n::s().bottle.change_engine)
                        .disabled(!has_engines)
                        .dropdown_menu(move |mut menu, _, _| {
                            if engines.is_empty() {
                                menu = menu.item(
                                    PopupMenuItem::new(crate::i18n::s().bottle.managed_only_item).disabled(true),
                                );
                            }
                            for engine in &engines {
                                let view = view.clone();
                                let container = container_owned.clone();
                                let engine_id = engine.id.clone();
                                let label = if engine.available {
                                    engine.name.clone()
                                } else {
                                    crate::i18n::s().bottle.engine_unavailable_item(&engine.name)
                                };
                                menu = menu.item(
                                    PopupMenuItem::new(label)
                                        .disabled(!engine.available)
                                        .checked(engine.id == current_id)
                                        .on_click(move |_, _, cx| {
                                            let _ = view.update(cx, |this, cx| {
                                                this.request_engine_select(
                                                    &container, &engine_id, cx,
                                                );
                                            });
                                        }),
                                );
                            }
                            menu
                        }),
                )
                .into_any_element(),
        );
        rows.push(
            #[cfg(not(target_os = "linux"))]
            muted(crate::i18n::s().bottle.engine_note_mac)
                .text_size(px(11.))
                .line_height(relative(1.4))
                .into_any_element(),
            #[cfg(target_os = "linux")]
            muted(crate::i18n::s().bottle.engine_note_linux)
                .text_size(px(11.))
                .line_height(relative(1.4))
                .into_any_element(),
        );

        // Capability section: every feature the engine reports, its state,
        // and a revoke control for approvals — revoking is separate from
        // turning a setting off, since several settings can share a feature.
        if let Some(selection) = selection {
            let container_features = container.to_owned();
            let interesting: Vec<_> = selection
                .features
                .iter()
                .filter(|feature| feature.state != "supported")
                .collect();
            if !interesting.is_empty() {
                for feature in interesting {
                    let state_label = match feature.state.as_str() {
                        "unverified" if feature.enabled => crate::i18n::s().bottle.feature_confirmed,
                        "unverified" => crate::i18n::s().bottle.feature_unverified,
                        _ => crate::i18n::s().bottle.feature_unavailable,
                    };
                    let reason = feature.reason.clone().unwrap_or_default();
                    let mut line = col().gap(px(2.)).py(px(4.)).child(
                        row()
                            .gap(px(10.))
                            .child(div().flex_1().min_w_0().child(format!(
                                "{} · {state_label}",
                                crate::i18n::config::engine_feature_label(&feature.id)
                            )))
                            .when(feature.enabled, |d| {
                                let container = container_features.clone();
                                let feature_id = feature.id.clone();
                                d.child(
                                    Button::new(format!("engine-revoke:{}", feature.id))
                                        .label(crate::i18n::s().bottle.revoke_confirmation)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.revoke_engine_feature(&container, &feature_id);
                                            cx.notify();
                                        })),
                                )
                            }),
                    );
                    if !reason.is_empty() {
                        line =
                            line.child(muted(reason).text_size(px(11.)).line_height(relative(1.4)));
                    }
                    rows.push(line.into_any_element());
                }
            }
        }
        Some(self.section("wine-engine", crate::i18n::s().bottle.wine_engine_section, rows, cx))
    }
}
