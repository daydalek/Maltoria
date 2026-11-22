//! Application settings surface.

use gpui_kit::component::button::{Button, ButtonVariants};
#[cfg(target_os = "linux")]
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable, Sizable};
use gpui_kit::{prelude::*, *};

use crate::controls::{col, muted, row};
use crate::preferences;
use crate::state::{SettingsSection, WhiskyApp};
use crate::theme::*;

const BLUR_SLIDER_WIDTH: f32 = 132.;
const BLUR_SLIDER_THUMB: f32 = 12.;

/// The blur level under the pointer, snapped to the decode variants the
/// artwork cache can produce.
fn blur_level_at(x: f32, bounds: [f32; 2]) -> f64 {
    let max = f32::from(crate::artwork::BLUR_LEVELS - 1);
    if bounds[1] <= 0. {
        return 0.;
    }
    f64::from(((x - bounds[0]) / bounds[1] * max).round().clamp(0., max))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn cover_upscale_label() -> &'static str {
    crate::i18n::s().settings.cover_upscale
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn cover_engine_label() -> &'static str {
    crate::i18n::s().settings.cover_engine
}

#[cfg(target_os = "macos")]
fn cover_engines() -> [(&'static str, &'static str); 2] {
    let s = &crate::i18n::s().settings;
    [
        (s.cover_engine_realcugan_mac, "realcugan"),
        (s.cover_engine_anime4k_mac, "anime4k"),
    ]
}
#[cfg(target_os = "linux")]
fn cover_engines() -> [(&'static str, &'static str); 2] {
    let s = &crate::i18n::s().settings;
    [
        (s.cover_engine_realcugan_gpu_linux, "realcugan"),
        (s.cover_engine_realcugan_cpu_linux, "realcugan-cpu"),
    ]
}

#[cfg(not(target_os = "linux"))]
fn terminals() -> [(&'static str, &'static str); 3] {
    [
        ("Terminal", preferences::TERMINAL_VALUES[0]),
        ("iTerm2", preferences::TERMINAL_VALUES[1]),
        ("Warp", preferences::TERMINAL_VALUES[2]),
    ]
}

#[cfg(target_os = "linux")]
fn terminals() -> [(&'static str, &'static str); 5] {
    let s = &crate::i18n::s().settings;
    [
        (s.terminal_auto, preferences::TERMINAL_VALUES[0]),
        (s.terminal_system, preferences::TERMINAL_VALUES[1]),
        ("GNOME Terminal", preferences::TERMINAL_VALUES[2]),
        ("XTerm", preferences::TERMINAL_VALUES[3]),
        ("Foot", preferences::TERMINAL_VALUES[4]),
    ]
}

impl WhiskyApp {
    /// One-click derivation for the whole library, with live progress; the same
    /// button cancels a pass that is already running.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn cover_batch_row(&self, cx: &Context<Self>) -> AnyElement {
        let s = &crate::i18n::s().settings;
        let running = self.cover_batch;
        let (action, detail) = match running {
            Some(batch) => (
                crate::i18n::s().settings.cover_batch_cancel(batch.done, batch.total),
                s.cover_batch_running_desc.to_owned(),
            ),
            None => (
                s.cover_batch_all.to_owned(),
                s.cover_batch_idle_desc.to_owned(),
            ),
        };
        let disabled = running.is_none()
            && (self.cover_upscale == crate::upscale::Factor::Off || !crate::upscale::ready());
        row()
            .py(px(10.))
            .gap(px(16.))
            .items_start()
            .child(
                col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(s.cover_batch_title)
                    .child(muted(detail).line_height(relative(1.4))),
            )
            .child(
                Button::new("cover-batch")
                    .label(action)
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| this.start_cover_batch(cx))),
            )
            .into_any_element()
    }

    /// Stepped backdrop blur. Each notch is a cached decode variant, so a
    /// drag only swaps which prepared image the backdrop draws — it never
    /// queues work mid-level. Disabled while the fill toggle is off.
    fn backdrop_blur_row(&self, cx: &Context<Self>) -> AnyElement {
        let enabled = self.flag("backdrop-fill");
        let max = crate::artwork::BLUR_LEVELS - 1;
        let level = self.backdrop_blur.min(max);
        let fraction = f32::from(level) / f32::from(max);
        let thumb = fraction * BLUR_SLIDER_WIDTH - BLUR_SLIDER_THUMB / 2.;
        let ticks: Vec<AnyElement> = (1..max)
            .map(|index| {
                let offset = BLUR_SLIDER_WIDTH * f32::from(index) / f32::from(max);
                div()
                    .absolute()
                    .left(px(offset - 1.))
                    .top(px(BLUR_SLIDER_THUMB / 2. + 0.5 + 1.))
                    .size(px(3.))
                    .rounded_full()
                    .bg(rgba(round_btn_border()))
                    .into_any_element()
            })
            .collect();
        // Hit testing needs the track's window position; prepaint is the only
        // place GPUI hands it out.
        let bounds = std::rc::Rc::new(std::cell::Cell::new([0f32; 2]));
        let measured = bounds.clone();
        let down = bounds.clone();
        let moved = bounds.clone();
        let mut track = div()
            .id("backdrop-blur-track")
            .relative()
            .w(px(BLUR_SLIDER_WIDTH))
            .h(px(BLUR_SLIDER_THUMB + 6.))
            .opacity(if enabled { 1. } else { 0.4 })
            .child(
                canvas(
                    move |bounds, _, _| {
                        measured.set([
                            f32::from(bounds.origin.x),
                            f32::from(bounds.size.width),
                        ]);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(BLUR_SLIDER_THUMB / 2. + 0.5))
                    .h(px(5.))
                    .w_full()
                    .rounded_full()
                    .bg(rgba(slider_track_bg())),
            )
            .child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(BLUR_SLIDER_THUMB / 2. + 0.5))
                    .h(px(5.))
                    .w(px(BLUR_SLIDER_WIDTH * fraction))
                    .rounded_full()
                    .bg(rgb(ACCENT)),
            )
            .children(ticks)
            .child(
                div()
                    .absolute()
                    .left(px(thumb.clamp(0., BLUR_SLIDER_WIDTH - BLUR_SLIDER_THUMB)))
                    .top(px(3.))
                    .size(px(BLUR_SLIDER_THUMB))
                    .rounded_full()
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgba(slider_thumb_border()))
                    .shadow_xs(),
            );
        if enabled {
            track = track
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if event.click_count >= 2 {
                            this.reset_backdrop_blur(cx);
                            return;
                        }
                        this.set_backdrop_blur(
                            blur_level_at(f32::from(event.position.x), down.get()),
                            cx,
                        );
                    }),
                )
                .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                    if !event.dragging() {
                        return;
                    }
                    this.set_backdrop_blur(
                        blur_level_at(f32::from(event.position.x), moved.get()),
                        cx,
                    );
                }));
        }
        let s = &crate::i18n::s().settings;
        let blur_names = crate::preferences::backdrop_blur_names();
        row()
            .py(px(10.))
            .gap(px(16.))
            .items_center()
            .child(
                col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(s.backdrop_blur)
                    .child(
                        muted(s.backdrop_blur_desc)
                            .line_height(relative(1.4)),
                    ),
            )
            .child(track)
            .child(
                muted(blur_names[usize::from(level)])
                    .w(px(30.))
                    .text_size(px(11.)),
            )
            .into_any_element()
    }

    #[cfg(target_os = "linux")]
    fn cover_upscale_status(&self) -> AnyElement {
        let s = &crate::i18n::s().settings;
        let text = if crate::upscale::ready() {
            s.cover_upscale_linux_ready
        } else {
            s.cover_upscale_linux_missing
        };
        row()
            .py(px(10.))
            .child(muted(text).line_height(relative(1.4)))
            .into_any_element()
    }

    fn terminal_preferences(&self, cx: &Context<Self>) -> AnyElement {
        let s = &crate::i18n::s().settings;
        let term_list = terminals();
        #[cfg(not(target_os = "linux"))]
        {
            return self.choice_pairs("terminal", s.preferred_terminal, &term_list, cx);
        }
        #[cfg(target_os = "linux")]
        {
            let selected = self
                .preview
                .choices
                .get("terminal")
                .map(|value| value.as_ref())
                .unwrap_or("auto");
            let label = term_list
                .iter()
                .find(|(_, id)| *id == selected)
                .map(|(label, _)| *label)
                .unwrap_or(s.terminal_configured);
            let choices = term_list
                .iter()
                .copied()
                .filter(|(_, id)| {
                    *id == "auto"
                        || self.host_terminals.as_ref().is_some_and(|status| {
                            status.available.iter().any(|term| term.id == *id)
                        })
                })
                .collect::<Vec<_>>();
            let status = if !self.service_available("system.terminals") {
                s.terminal_service_unavailable.to_owned()
            } else {
                self.host_terminal_error
                    .clone()
                    .unwrap_or_else(|| match &self.host_terminals {
                        Some(status) if status.available.is_empty() => {
                            s.terminal_none_available.to_owned()
                        }
                        Some(status)
                            if selected != "auto"
                                && !status.available.iter().any(|term| term.id == selected) =>
                        {
                            s.terminal_preferred_unavailable.to_owned()
                        }
                        Some(status) if !status.display_available => {
                            s.terminal_need_display.to_owned()
                        }
                        Some(status) => status
                            .available
                            .iter()
                            .map(|term| format!("{} · {}", term.name, term.path))
                            .collect::<Vec<_>>()
                            .join("；"),
                        None => s.terminal_discovering.to_owned(),
                    })
            };
            let view = cx.entity().downgrade();
            let enabled = self
                .host_terminals
                .as_ref()
                .is_some_and(|status| !status.available.is_empty())
                && self.service_available("system.terminals")
                && self.service_available("preferences.set");
            crate::controls::col()
                .gap(px(6.))
                .child(
                    row()
                        .py(px(10.))
                        .gap(px(16.))
                        .child(div().flex_1().child(s.preferred_terminal))
                        .child(
                            Button::new("linux-terminal")
                                .label(format!("{label}  ⌄"))
                                .disabled(!enabled)
                                .dropdown_menu(move |mut menu, _, _| {
                                    for &(label, id) in &choices {
                                        let view = view.clone();
                                        menu = menu.item(PopupMenuItem::new(label).on_click(
                                            move |_, _, cx| {
                                                let _ = view.update(cx, |this, cx| {
                                                    this.command(
                                                        super::Command::Choose("terminal", id),
                                                        cx,
                                                    )
                                                });
                                            },
                                        ));
                                    }
                                    menu
                                }),
                        )
                        .child(
                            Button::new("refresh-terminals")
                                .label(s.terminal_redetect)
                                .disabled(!self.service_available("system.terminals"))
                                .on_click(
                                    cx.listener(|this, _, _, _| this.bridge.load_terminals()),
                                ),
                        ),
                )
                .child(muted(status).text_size(px(11.)))
                .into_any_element()
        }
    }

    #[cfg(target_os = "linux")]
    fn desktop_preferences(&self, cx: &Context<Self>) -> AnyElement {
        let s = &crate::i18n::s().settings;
        let installed = self
            .desktop_status
            .as_ref()
            .is_some_and(|status| status.installed);
        let message = if !self.service_available("system.desktop.status") {
            s.desktop_service_unavailable.into()
        } else {
            self.desktop_error
                .clone()
                .or_else(|| {
                    self.desktop_status
                        .as_ref()
                        .map(|status| status.path.clone())
                })
                .unwrap_or_else(|| s.desktop_reading.into())
        };
        crate::controls::col()
            .gap(px(6.))
            .child(
                row()
                    .py(px(10.))
                    .gap(px(16.))
                    .child(div().flex_1().child(s.desktop_entry_title))
                    .child(
                        Button::new("install-desktop")
                            .label(if self.desktop_installing {
                                s.desktop_adding
                            } else if self
                                .desktop_status
                                .as_ref()
                                .is_some_and(|status| status.needs_update)
                            {
                                s.desktop_update
                            } else if installed {
                                s.desktop_added
                            } else {
                                s.desktop_add
                            })
                            .disabled(
                                self.desktop_installing
                                    || !self.service_available("system.desktop.install"),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.desktop_installing = true;
                                this.desktop_error = None;
                                this.bridge.desktop_integration(true);
                                cx.notify();
                            })),
                    ),
            )
            .child(muted(message).text_size(px(11.)))
            .into_any_element()
    }

    pub(crate) fn application_settings(&self, cx: &Context<Self>) -> AnyElement {
        self.form(
            "app-settings-scroll",
            vec![
                self.general_section(cx),
                self.library_prefs_section(cx),
                self.translation_entry_section(cx),
                self.metadata_section(cx),
                self.runtime_section(cx),
                self.network_section(cx),
                #[cfg(not(target_os = "linux"))]
                self.gptk_section(cx),
                self.engines_section(cx),
            ],
        )
    }

    pub(crate) fn settings_section_page(
        &self,
        section: SettingsSection,
        cx: &Context<Self>,
    ) -> AnyElement {
        let sections = match section {
            SettingsSection::General => vec![self.general_section(cx)],
            SettingsSection::Library => vec![self.library_prefs_section(cx)],
            SettingsSection::Translation => vec![self.translation_entry_section(cx)],
            SettingsSection::Metadata => vec![self.metadata_section(cx)],
            SettingsSection::Runtime => vec![
                self.runtime_section(cx),
                #[cfg(not(target_os = "linux"))]
                self.gptk_section(cx),
                self.engines_section(cx),
            ],
            SettingsSection::Network => vec![self.network_section(cx)],
        };
        self.form(section.scroll_id(), sections)
    }

    fn general_section(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let s = &strings.settings;
        let bottle_location = self
            .default_bottle_location
            .clone()
            .unwrap_or_else(|| s.reading_bottle_location.into());
        let terminal = self.terminal_preferences(cx);
        #[cfg(target_os = "linux")]
        let terminal = crate::controls::col()
            .gap(px(8.))
            .child(terminal)
            .child(self.desktop_preferences(cx))
            .into_any_element();
        self.section(
            "general",
            s.general_title,
            vec![
                self.toggle(
                    "kill",
                    s.kill_on_terminate_title,
                    if self.toggle_available("kill") {
                        s.kill_on_terminate_desc
                    } else {
                        s.kill_on_terminate_unavailable
                    },
                    cx,
                ),
                // "Show Whisky in Menu Bar" is deliberately absent: the
                // GPUI toolkit has no menu-bar/status-item API, so
                // the toggle would write a shared default nothing
                // reads here. It stays in the schema for SwiftUI.
                #[cfg(not(target_os = "linux"))]
                self.choice_pairs(
                    "appearance",
                    s.appearance_label,
                    &crate::theme::appearance_options(),
                    cx,
                ),
                self.choice_pairs(
                    "language",
                    s.language_label,
                    &crate::i18n::LanguageMode::options(),
                    cx,
                ),
                terminal,
                row()
                    .py(px(10.))
                    .gap(px(16.))
                    .child(div().flex_1().min_w_0().child(s.default_bottle_location))
                    .child(
                        muted(bottle_location)
                            .max_w(relative(0.5))
                            .truncate(),
                    )
                    .child(
                        Button::new("choose-bottle-location")
                            .label(s.change_ellipsis)
                            .disabled(
                                !self.service_available("system.chooseFolder")
                                    || !self.service_available("preferences.set"),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.choose_bottle_location(cx);
                            })),
                    )
                    .into_any_element(),
            ],
            cx,
        )
    }

    fn library_prefs_section(&self, cx: &Context<Self>) -> AnyElement {
        let s = &crate::i18n::s().settings;
        let mut rows = vec![
            self.library_mode_cards(cx),
            self.toggle(
                "refresh",
                s.refresh_after_exit,
                if self.toggle_available("refresh") {
                    ""
                } else {
                    s.refresh_after_exit_unavailable
                },
                cx,
            ),
            self.toggle(
                "group-by-studio",
                s.group_by_studio,
                s.group_by_studio_desc,
                cx,
            ),
            self.choice_pairs(
                "tag-bar-mode",
                s.tag_bar_mode,
                &[
                    (s.tag_bar_pull_down, "pull-down"),
                    (s.tag_bar_always, "always"),
                ],
                cx,
            ),
        ];
        if self.tag_bar_mode == crate::ui_state::TagBarMode::PullDown {
            rows.push(
                row()
                    .py(px(10.))
                    .gap(px(16.))
                    .items_center()
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(s.tag_bar_hint_title)
                            .child(
                                muted(s.tag_bar_hint_desc)
                                    .line_height(relative(1.4)),
                            ),
                    )
                    .child(
                        Button::new("replay-tag-bar-hint")
                            .label(s.tag_bar_hint_replay)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.command(super::Command::ReplayTagBarHint, cx);
                            })),
                    )
                    .into_any_element(),
            );
        }
        rows.extend([
            self.toggle(
                "mixed",
                s.mixed_cover,
                s.mixed_cover_desc,
                cx,
            ),
            self.toggle("single-title", s.single_line_titles, "", cx),
            self.toggle(
                "backdrop-fill",
                s.backdrop_fill,
                s.backdrop_fill_desc,
                cx,
            ),
            self.backdrop_blur_row(cx),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            self.choice_pairs(
                "cover-upscale",
                cover_upscale_label(),
                &[
                    (crate::i18n::s().common.off, "off"),
                    (s.cover_upscale_x2, "x2"),
                    (s.cover_upscale_x4, "x4"),
                ],
                cx,
            ),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            self.choice_pairs(
                "cover-engine",
                cover_engine_label(),
                &cover_engines(),
                cx,
            ),
            #[cfg(target_os = "linux")]
            self.cover_upscale_status(),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            self.cover_batch_row(cx),
            self.toggle(
                "scrape-characters",
                s.scrape_characters,
                s.scrape_characters_desc,
                cx,
            ),
            self.toggle(
                "center-cover-text",
                s.center_wide_text,
                s.center_wide_text_desc,
                cx,
            ),
            // The SwiftUI-only display preferences (blurred cover
            // fill, fill style, detail layout) stay absent: this
            // renderer crops covers with object-fit so a fill never
            // shows, and implements one detail layout. They stay in
            // the shared preference schema and still affect the
            // SwiftUI app. Backdrop blur is a GPUI decode variant
            // driven by the `libraryBackdrop*` keys above.
            self.toggle(
                "motion",
                s.motion_title,
                s.motion_desc,
                cx,
            ),
            self.toggle(
                "lively",
                s.lively_title,
                s.lively_desc,
                cx,
            ),
        ]);
        self.section("library-prefs", s.library_title, rows, cx)
    }

    fn translation_entry_section(&self, cx: &Context<Self>) -> AnyElement {
        let s = &crate::i18n::s().settings;
        self.section(
            "translation-settings",
            s.translation_title,
            vec![
                muted(s.translation_desc)
                    .line_height(relative(1.4))
                    .into_any_element(),
                row()
                    .py(px(6.))
                    .child(
                        Button::new("translation-settings-open")
                            .label(s.translation_open)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_translation_settings(cx)
                            })),
                    )
                    .into_any_element(),
            ],
            cx,
        )
    }

    fn metadata_section(&self, cx: &Context<Self>) -> AnyElement {
        let s = &crate::i18n::s().settings;
        self.section(
            "metadata-preferences",
            s.metadata_title,
            vec![
                self.info(
                    s.metadata_default_source,
                    crate::onboarding::source_label(&self.metadata_preferences.default_source),
                ),
                muted(s.metadata_desc)
                    .into_any_element(),
                row()
                    .gap(px(8.))
                    .child(
                        Button::new("metadata-preferences-open")
                            .label(s.metadata_adjust)
                            .disabled(!self.service_available("preferences.setMany"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_onboarding(false, cx)
                            })),
                    )
                    .child(
                        Button::new("welcome-reopen")
                            .label(s.metadata_guide)
                            .ghost()
                            .disabled(!self.service_available("preferences.setMany"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_onboarding(true, cx)
                            })),
                    )
                    .into_any_element(),
            ],
            cx,
        )
    }

    fn runtime_section(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let s = &strings.settings;
        self.section(
            "runtime",
            s.runtime_title,
            if self.service_available("setup.status") {
                vec![
                    self.info(
                        strings.common.status,
                        self.setup_status.as_ref().map_or_else(
                            || strings.common.unknown.to_owned(),
                            |status| {
                                if status.installed {
                                    status.version.clone().map_or_else(
                                        || strings.common.installed.to_owned(),
                                        |version| crate::i18n::s().settings.runtime_installed_version(&version),
                                    )
                                } else {
                                    strings.common.not_installed.to_owned()
                                }
                            },
                        ),
                    ),
                    row()
                        .py(px(6.))
                        .child(
                            Button::new("runtime-install")
                                .label(s.runtime_install_update)
                                .disabled(!self.service_available("setup.install"))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.install_wine_runtime(cx)),
                                ),
                        )
                        .into_any_element(),
                ]
            } else {
                vec![self.info(s.current_service, s.service_file_host)]
            },
            cx,
        )
    }

    fn network_section(&self, cx: &Context<Self>) -> AnyElement {
        let s = &crate::i18n::s().settings;
        self.section(
            "network",
            s.network_title,
            vec![
                self.toggle(
                    "offline",
                    s.offline_mode,
                    s.offline_mode_desc,
                    cx,
                ),
                self.toggle(
                    "updates",
                    s.check_updates,
                    if self.toggle_available("updates") {
                        ""
                    } else {
                        s.check_updates_unavailable
                    },
                    cx,
                ),
            ],
            cx,
        )
    }

    #[cfg(not(target_os = "linux"))]
    fn gptk_section(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let s = &strings.settings;
        let status = self.gptk_status.as_ref();
        let installed = status.is_some_and(|s| s.installed);
        let capable = status.is_some_and(|s| s.capable);
        let version = status
            .and_then(|s| s.version.clone())
            .unwrap_or_else(|| strings.common.unknown.into());
        let unverified = status.is_some_and(|s| s.unverified);
        let error = status.and_then(|s| s.error.clone());

        let mut rows = Vec::new();

        if self.gptk_importing {
            rows.push(
                row()
                    .gap(px(8.))
                    .py(px(10.))
                    .child(Spinner::new().with_size(px(14.)))
                    .child(muted(s.gptk_importing))
                    .into_any_element(),
            );
        } else if installed {
            rows.push(self.info(s.gptk_d3dmetal_version, version));
            if unverified || error.is_some() {
                rows.push(
                    muted(error.unwrap_or_else(|| s.gptk_unverified_default.into()))
                        .text_size(px(12.))
                        .into_any_element(),
                );
            }
            rows.push(
                row()
                    .py(px(6.))
                    .gap(px(12.))
                    .child(
                        Button::new("gptk-reimport")
                            .label(s.gptk_import_btn)
                            .disabled(self.gptk_importing)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_gptk_image(cx))),
                    )
                    .child(
                        Button::new("gptk-remove")
                            .label(s.gptk_remove_btn)
                            .disabled(self.gptk_importing)
                            .on_click(cx.listener(|this, _, _, cx| this.remove_gptk(cx))),
                    )
                    .into_any_element(),
            );
        } else {
            rows.push(self.info(strings.common.status, s.gptk_not_imported));
            rows.push(
                row()
                    .py(px(6.))
                    .child(
                        Button::new("gptk-import")
                            .label(s.gptk_import_btn)
                            .disabled(self.gptk_importing)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_gptk_image(cx))),
                    )
                    .into_any_element(),
            );
        }

        let footer_text = if capable {
            s.gptk_capable_footer
        } else {
            s.gptk_incapable_footer
        };
        rows.push(muted(footer_text).text_size(px(11.)).into_any_element());

        self.section("gptk", "Game Porting Toolkit", rows, cx)
    }
}

impl WhiskyApp {
    /// Custom Wine engines: registered external engine trees. The managed row is
    /// immutable — install/update lives in the Wine runtime section — and
    /// removal only drops the registration, never the files.
    fn engines_section(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let s = &strings.settings;
        let mut rows = Vec::new();

        if self.service_available("wine.engines.list") {
            for engine in &self.wine_engines {
                let mut status = if engine.managed {
                    s.engine_managed.to_owned()
                } else if engine.available {
                    // Detection never launches the engine, so a version is
                    // known only when the tree ships metadata.
                    match &engine.version {
                        Some(version) => crate::i18n::s().settings.engine_registered(version),
                        None => s.engine_registered_unknown_ver.to_owned(),
                    }
                } else {
                    strings.common.unavailable.to_owned()
                };
                if !engine.architectures.is_empty() {
                    status = format!("{status} · {}", engine.architectures.join("/"));
                }
                rows.push(self.info(engine.name.clone(), status));
                if let Some(reason) = &engine.error {
                    rows.push(muted(reason.clone()).text_size(px(11.)).into_any_element());
                }
                // Static detection cannot prove a feature works — list what
                // the tree carries unverified and what it lacks entirely.
                let unverified: Vec<&str> = engine
                    .features
                    .iter()
                    .filter(|feature| feature.state == "unverified")
                    .map(|feature| crate::i18n::config::engine_feature_label(&feature.id))
                    .collect();
                let unavailable: Vec<&str> = engine
                    .features
                    .iter()
                    .filter(|feature| feature.state == "unavailable")
                    .map(|feature| crate::i18n::config::engine_feature_label(&feature.id))
                    .collect();
                if !unverified.is_empty() || !unavailable.is_empty() {
                    let summary = crate::i18n::fmt::engine_features_summary(&unverified, &unavailable);
                    rows.push(
                        muted(summary)
                            .text_size(px(11.))
                            .line_height(relative(1.4))
                            .into_any_element(),
                    );
                }
                if !engine.managed {
                    rows.push(
                        muted(engine.path.clone()).text_size(px(11.)).truncate().into_any_element(),
                    );
                    let id = engine.id.clone();
                    rows.push(
                        row()
                            .py(px(4.))
                            .child(
                                Button::new(format!("engine-remove:{id}"))
                                    .label(s.engine_remove_reg)
                                    .danger()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_wine_engine(&id, cx);
                                    })),
                            )
                            .into_any_element(),
                    );
                }
            }
            if let Some(error) = &self.wine_engines_error {
                rows.push(muted(crate::i18n::s().settings.engine_registry_read_failed(error)).into_any_element());
            }
            rows.push(
                row()
                    .py(px(6.))
                    .gap(px(12.))
                    .child(
                        Button::new("engine-register")
                            .label(s.engine_add_dir)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.choose_wine_engine_directory(cx);
                            })),
                    )
                    .into_any_element(),
            );
            rows.push(
                #[cfg(not(target_os = "linux"))]
                muted(s.engines_footer_mac)
                    .text_size(px(11.))
                    .line_height(relative(1.4))
                    .into_any_element(),
                #[cfg(target_os = "linux")]
                muted(s.engines_footer_linux)
                    .text_size(px(11.))
                    .line_height(relative(1.4))
                    .into_any_element(),
            );
        } else {
            rows.push(self.info(s.current_service, s.engines_service_unavailable));
        }

        self.section("wine-engines", s.engines_title, rows, cx)
    }
}
