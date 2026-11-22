//! Independent native windows for Preferences (`SettingsWindow`) and live
//! game text capture / translation (`CaptureWindow`).

use gpui_kit::base::Presence;
use gpui_kit::component::button::Button;
use gpui_kit::component::Sizable;
use gpui_kit::{prelude::*, *};

use crate::controls::{col, gradient, icon, row};
use crate::platform;
use crate::state::{DetachedWindow, SettingsSection, WhiskyApp};
use crate::theme::{
    self, ACCENT, AppearanceMode, NOTICE_MS, divider, hover, muted_color, selected_bg,
    sidebar_gradient, text, toast_bg, toast_border, toolbar_bg,
};
use crate::{
    AppearanceDark, AppearanceLight, AppearanceSystem, CloseWindow, DismissPreview, LanguageEn,
    LanguageJa, LanguageKo, LanguageSystem, LanguageZhHans, MinimizeWindow, QuitPreview,
    ShowSettings, ToggleFullScreen, ZoomWindow,
};

/// macOS-style two-pane Settings window (`⌘,`).
pub(crate) struct SettingsWindow {
    app: Entity<WhiskyApp>,
    focus_handle: FocusHandle,
    rendered_locale: crate::i18n::Locale,
    _subscription: Subscription,
}

/// Companion Live Game Translation / Hook Capture window.
pub(crate) struct CaptureWindow {
    app: Entity<WhiskyApp>,
    focus_handle: FocusHandle,
    rendered_locale: crate::i18n::Locale,
    _subscription: Subscription,
}

/// Opens `SettingsWindow` on the next app turn if settings are active and no
/// window handle has been created yet.
pub(crate) fn open_settings_window_if_needed(weak: WeakEntity<WhiskyApp>, cx: &mut App) {
    let Some(app) = weak.upgrade() else {
        return;
    };
    let (active, existing) = {
        let guard = app.read(cx);
        (guard.settings, guard.settings_window.handle())
    };
    if !active {
        app.update(cx, |this, _| {
            this.settings_window = DetachedWindow::Closed;
        });
        return;
    }
    if let Some(handle) = existing {
        if handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
        app.update(cx, |this, _| {
            this.settings_window = DetachedWindow::Opening;
        });
    }

    let bounds = WindowBounds::Windowed(Bounds::centered(None, size(px(980.), px(680.)), cx));
    let app_for_build = app.clone();
    let result = gpui_kit::open_window(
        WindowOptions {
            #[cfg(target_os = "linux")]
            app_id: Some("com.daydalek.Maltoria.settings".into()),
            #[cfg(target_os = "linux")]
            window_decorations: Some(WindowDecorations::Client),
            titlebar: Some(TitlebarOptions {
                title: Some(crate::i18n::s().windows.settings_title.into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(15.), px(21.))),
            }),
            window_bounds: Some(bounds),
            window_min_size: Some(size(px(800.), px(560.))),
            ..Default::default()
        },
        cx,
        move |window, cx| {
            let weak_close = app_for_build.downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                if let Some(app) = weak_close.upgrade() {
                    app.update(cx, |this, cx| this.handle_settings_window_closing(window, cx));
                }
                true
            });
            cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                window.focus(&focus_handle, cx);
                let _subscription = cx.observe(&app_for_build, |_, _, cx| cx.notify());
                SettingsWindow {
                    app: app_for_build,
                    focus_handle,
                    rendered_locale: crate::i18n::locale(),
                    _subscription,
                }
            })
        },
    );

    match result {
        Ok((handle, _)) => {
            app.update(cx, |this, cx| {
                if this.settings {
                    this.settings_window = DetachedWindow::Open(handle);
                } else {
                    this.settings_window = DetachedWindow::Closed;
                    (**cx).defer(move |cx| {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    });
                }
                cx.notify();
            });
        }
        Err(_) => {
            app.update(cx, |this, cx| {
                this.settings_window = DetachedWindow::Closed;
                cx.notify();
            });
        }
    }
}

/// Opens `CaptureWindow` on the next app turn if the capture panel is active
/// and no window handle exists yet.
pub(crate) fn open_capture_window_if_needed(weak: WeakEntity<WhiskyApp>, cx: &mut App) {
    let Some(app) = weak.upgrade() else {
        return;
    };
    let (active, existing) = {
        let guard = app.read(cx);
        (guard.capture.panel_open, guard.capture_window.handle())
    };
    if !active {
        app.update(cx, |this, _| {
            this.capture_window = DetachedWindow::Closed;
        });
        return;
    }
    if let Some(handle) = existing {
        if handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
        app.update(cx, |this, _| {
            this.capture_window = DetachedWindow::Opening;
        });
    }

    let bounds = WindowBounds::Windowed(Bounds::centered(None, size(px(940.), px(680.)), cx));
    let app_for_build = app.clone();
    let result = gpui_kit::open_window(
        WindowOptions {
            #[cfg(target_os = "linux")]
            app_id: Some("com.daydalek.Maltoria.capture".into()),
            #[cfg(target_os = "linux")]
            window_decorations: Some(WindowDecorations::Client),
            titlebar: Some(TitlebarOptions {
                title: Some(crate::i18n::s().windows.capture_title.into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(15.), px(21.))),
            }),
            window_bounds: Some(bounds),
            window_min_size: Some(size(px(720.), px(520.))),
            ..Default::default()
        },
        cx,
        move |window, cx| {
            let weak_close = app_for_build.downgrade();
            window.on_window_should_close(cx, move |_, cx| {
                if let Some(app) = weak_close.upgrade() {
                    app.update(cx, |this, cx| this.handle_capture_window_closing(cx));
                }
                true
            });
            cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                window.focus(&focus_handle, cx);
                let _subscription = cx.observe(&app_for_build, |_, _, cx| cx.notify());
                CaptureWindow {
                    app: app_for_build,
                    focus_handle,
                    rendered_locale: crate::i18n::locale(),
                    _subscription,
                }
            })
        },
    );

    match result {
        Ok((handle, _)) => {
            app.update(cx, |this, cx| {
                if this.capture.panel_open {
                    this.capture_window = DetachedWindow::Open(handle);
                } else {
                    this.capture_window = DetachedWindow::Closed;
                    (**cx).defer(move |cx| {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    });
                }
                cx.notify();
            });
        }
        Err(_) => {
            app.update(cx, |this, cx| {
                this.capture_window = DetachedWindow::Closed;
                cx.notify();
            });
        }
    }
}

fn render_secondary_notice(app: &WhiskyApp, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let notification = Presence::new("secondary-notice-presence", app.notice.is_some())
        .transition(app.timing(NOTICE_MS))
        .sample(window, cx);
    if !notification.should_render() {
        return None;
    }
    let message = app.notice.clone().unwrap_or_default();
    Some(
        row()
            .absolute()
            .bottom(px(20.))
            .right(px(20.))
            .gap(px(8.))
            .px(px(16.))
            .py(px(11.))
            .rounded(px(12.))
            .bg(rgb(toast_bg()))
            .border_1()
            .border_color(rgba(toast_border()))
            .shadow_lg()
            .opacity(notification.progress)
            .child(icon("info", 16.).text_color(rgb(ACCENT)))
            .child(message)
            .into_any_element(),
    )
}

fn attach_shared_window_actions<V: Render + 'static>(
    element: Div,
    app: &Entity<WhiskyApp>,
    cx: &mut Context<V>,
) -> Div {
    let app_quit = app.clone();
    let app_sys = app.clone();
    let app_light = app.clone();
    let app_dark = app.clone();
    let app_lang_sys = app.clone();
    let app_lang_zh = app.clone();
    let app_lang_ja = app.clone();
    let app_lang_en = app.clone();
    let app_lang_ko = app.clone();
    element
        .on_action(cx.listener(move |_, _: &QuitPreview, window, cx| {
            app_quit.update(cx, |this, cx| this.request_quit(window, cx));
        }))
        .on_action(cx.listener(move |_, _: &AppearanceSystem, _, cx| {
            app_sys.update(cx, |this, cx| this.set_appearance(AppearanceMode::System, cx));
        }))
        .on_action(cx.listener(move |_, _: &AppearanceLight, _, cx| {
            app_light.update(cx, |this, cx| this.set_appearance(AppearanceMode::Light, cx));
        }))
        .on_action(cx.listener(move |_, _: &AppearanceDark, _, cx| {
            app_dark.update(cx, |this, cx| this.set_appearance(AppearanceMode::Dark, cx));
        }))
        .on_action(cx.listener(move |_, _: &LanguageSystem, _, cx| {
            app_lang_sys.update(cx, |this, cx| {
                this.set_language(crate::i18n::LanguageMode::System, cx);
            });
        }))
        .on_action(cx.listener(move |_, _: &LanguageZhHans, _, cx| {
            app_lang_zh.update(cx, |this, cx| {
                this.set_language(crate::i18n::LanguageMode::ZhHans, cx);
            });
        }))
        .on_action(cx.listener(move |_, _: &LanguageJa, _, cx| {
            app_lang_ja.update(cx, |this, cx| {
                this.set_language(crate::i18n::LanguageMode::Ja, cx);
            });
        }))
        .on_action(cx.listener(move |_, _: &LanguageEn, _, cx| {
            app_lang_en.update(cx, |this, cx| {
                this.set_language(crate::i18n::LanguageMode::En, cx);
            });
        }))
        .on_action(cx.listener(move |_, _: &LanguageKo, _, cx| {
            app_lang_ko.update(cx, |this, cx| {
                this.set_language(crate::i18n::LanguageMode::Ko, cx);
            });
        }))
        .on_action(cx.listener(|_, _: &MinimizeWindow, window, _| {
            window.minimize_window();
        }))
        .on_action(cx.listener(|_, _: &ZoomWindow, window, _| {
            window.zoom_window();
        }))
        .on_action(cx.listener(|_, _: &ToggleFullScreen, window, _| {
            window.toggle_fullscreen();
        }))
}

#[cfg(target_os = "linux")]
fn wrap_linux_client_titlebar(
    title: impl Into<SharedString>,
    root: Div,
    window: &Window,
    on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let client_titlebar = matches!(window.window_decorations(), Decorations::Client { .. });
    if client_titlebar {
        col()
            .size_full()
            .child(
                gpui_kit::component::TitleBar::new()
                    .child(title.into())
                    .on_close_window(on_close),
            )
            .child(div().relative().flex_1().min_h_0().child(root))
            .into_any_element()
    } else {
        root.into_any_element()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.app.read(cx).appearance;
        theme::sync_theme(appearance, Some(window), cx);
        let current_locale = crate::i18n::locale();
        if self.rendered_locale != current_locale {
            self.rendered_locale = current_locale;
            window.set_window_title(crate::i18n::s().windows.settings_title);
        }
        let ww = f32::from(window.viewport_size().width);
        let wh = f32::from(window.viewport_size().height);
        let fullscreen = window.is_fullscreen() || window.is_simple_fullscreen();

        let (content, needs_frame) = self.app.update(cx, |app, app_cx| {
            app.scroll_needs_measurement.set(false);
            let active_section = app.settings_section;
            let provider_count = app.translation_page.providers.len();
            let (grad_start, grad_end) = sidebar_gradient();
            let sidebar_top_pad = if fullscreen { 14. } else { 52. };

            let nav_rows: Vec<AnyElement> = SettingsSection::ALL
                .into_iter()
                .map(|section| {
                    let selected = section == active_section;
                    let badge_text = (section == SettingsSection::Translation && provider_count > 0)
                        .then(|| provider_count.to_string());
                    row()
                        .id(format!("settings-nav:{}", section.id()))
                        .h(px(32.))
                        .px(px(10.))
                        .gap(px(9.))
                        .rounded(px(7.))
                        .cursor_pointer()
                        .bg(rgba(if selected { selected_bg() } else { 0x00000000 }))
                        .hover(|s| if selected { s } else { s.bg(rgba(hover())) })
                        .on_click(app_cx.listener(move |this, _, window, cx| {
                            this.open_settings_section(section, Some(window), cx);
                        }))
                        .child(icon(section.icon(), 14.).text_color(rgb(if selected {
                            ACCENT
                        } else {
                            muted_color()
                        })))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.))
                                .font_weight(if selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::MEDIUM
                                })
                                .text_color(rgb(text()))
                                .child(section.label()),
                        )
                        .when_some(badge_text, |d, count| {
                            d.child(
                                div()
                                    .px(px(6.))
                                    .py(px(1.))
                                    .rounded_full()
                                    .bg(rgba(theme::badge_bg()))
                                    .text_size(px(10.5))
                                    .text_color(rgb(if selected { ACCENT } else { muted_color() }))
                                    .child(count),
                            )
                        })
                        .into_any_element()
                })
                .collect();

            let sidebar = col()
                .w(px(196.))
                .min_w(px(196.))
                .h_full()
                .flex_shrink_0()
                .bg(gradient(grad_start, grad_end))
                .border_r_1()
                .border_color(rgba(divider()))
                .child(div().h(px(sidebar_top_pad)).flex_shrink_0())
                .child(
                    col()
                        .flex_1()
                        .px(px(10.))
                        .gap(px(2.))
                        .children(nav_rows),
                );

            let section_header = row()
                .h(px(52.))
                .flex_shrink_0()
                .px(px(24.))
                .gap(px(10.))
                .items_center()
                .bg(rgba(toolbar_bg()))
                .border_b_1()
                .border_color(rgba(divider()))
                .child(icon(active_section.icon(), 16.).text_color(rgb(ACCENT)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(15.))
                        .font_weight(FontWeight::BOLD)
                        .child(active_section.label()),
                );

            let section_body = if active_section == SettingsSection::Translation {
                app.translation_settings(window, app_cx)
            } else {
                app.settings_section_page(active_section, app_cx)
            };

            let right_pane = col()
                .relative()
                .flex_1()
                .min_w_0()
                .h_full()
                .bg(rgb(theme::bg()))
                .child(section_header)
                .child(div().relative().flex_1().min_h_0().w_full().child(section_body));

            let mut container = div()
                .relative()
                .size_full()
                .child(row().size_full().child(sidebar).child(right_pane));

            if app.translation_page.active && app.translation_page.wizard.is_some() {
                container = container.child(app.translation_wizard_modal(ww, wh, window, app_cx));
            }
            if app.onboarding.is_some() {
                container = container.child(app.onboarding_view(ww, wh, app_cx));
            }
            if app.setup_dialog {
                container = container.child(app.setup_dialog_view(ww, wh, app_cx));
            }
            if let Some(notice) = render_secondary_notice(app, window, app_cx) {
                container = container.child(notice);
            }

            (container, app.scroll_needs_measurement.get())
        });

        if needs_frame {
            window.request_animation_frame();
        }

        let root = attach_shared_window_actions(
            col()
                .track_focus(&self.focus_handle)
                .relative()
                .size_full()
                .overflow_hidden()
                .bg(rgb(theme::bg()))
                .font_family(platform::ui_font())
                .text_color(rgb(text()))
                .text_size(px(13.)),
            &self.app,
            cx,
        )
        .on_action(cx.listener(|this, _: &ShowSettings, _, _| {
            let _ = this;
        }))
        .on_action(cx.listener(|this, _: &CloseWindow, window, cx| {
            this.app
                .update(cx, |app, cx| app.handle_settings_window_closing(window, cx));
            window.remove_window();
        }))
        .on_action(cx.listener(|this, _: &DismissPreview, window, cx| {
            let handled_modal = this.app.update(cx, |app, cx| {
                if let Some(state) = &app.onboarding {
                    if !state.saving {
                        app.onboarding = None;
                        cx.notify();
                    }
                    return true;
                }
                if app.setup_dialog && !app.setup_installing {
                    app.dismiss_setup(cx);
                    return true;
                }
                if app.translation_page.active && app.translation_page.wizard.is_some() {
                    app.translation_page.close_wizard(Some(window), Some(cx));
                    cx.notify();
                    return true;
                }
                app.handle_settings_window_closing(window, cx);
                false
            });
            if handled_modal {
                this.focus_handle.focus(window, cx);
            } else {
                window.remove_window();
            }
        }))
        .child(content);

        #[cfg(target_os = "linux")]
        {
            wrap_linux_client_titlebar(
                crate::i18n::s().windows.settings_title,
                root,
                window,
                cx.listener(|this, _, window, cx| {
                    this.app
                        .update(cx, |app, cx| app.handle_settings_window_closing(window, cx));
                    window.remove_window();
                }),
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            root
        }
    }
}

impl Render for CaptureWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.app.read(cx).appearance;
        theme::sync_theme(appearance, Some(window), cx);
        let current_locale = crate::i18n::locale();
        if self.rendered_locale != current_locale {
            self.rendered_locale = current_locale;
            window.set_window_title(crate::i18n::s().windows.capture_title);
        }
        let fullscreen = window.is_fullscreen() || window.is_simple_fullscreen();

        let (content, needs_frame) = self.app.update(cx, |app, app_cx| {
            app.scroll_needs_measurement.set(false);
            let left_pad = platform::traffic_light_padding(0., 18., fullscreen);
            let capture_view = app.capture_window_content(left_pad, window, app_cx);
            let mut container = div().relative().size_full().child(capture_view);
            if let Some(notice) = render_secondary_notice(app, window, app_cx) {
                container = container.child(notice);
            }
            (container, app.scroll_needs_measurement.get())
        });

        if needs_frame {
            window.request_animation_frame();
        }

        let app_for_settings = self.app.clone();
        let root = attach_shared_window_actions(
            col()
                .track_focus(&self.focus_handle)
                .relative()
                .size_full()
                .overflow_hidden()
                .bg(rgb(theme::bg()))
                .font_family(platform::ui_font())
                .text_color(rgb(text()))
                .text_size(px(13.)),
            &self.app,
            cx,
        )
        .on_action(cx.listener(move |_, _: &ShowSettings, _, cx| {
            app_for_settings.update(cx, |this, cx| this.open_settings(cx));
        }))
        .on_action(cx.listener(|this, _: &CloseWindow, window, cx| {
            this.app
                .update(cx, |app, cx| app.handle_capture_window_closing(cx));
            window.remove_window();
        }))
        .on_action(cx.listener(|this, _: &DismissPreview, window, cx| {
            this.app
                .update(cx, |app, cx| app.handle_capture_window_closing(cx));
            window.remove_window();
        }))
        .child(content);

        #[cfg(target_os = "linux")]
        {
            wrap_linux_client_titlebar(
                crate::i18n::s().windows.capture_title,
                root,
                window,
                cx.listener(|this, _, window, cx| {
                    this.app
                        .update(cx, |app, cx| app.handle_capture_window_closing(cx));
                    window.remove_window();
                }),
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
            root
        }
    }
}

/// Shared AI-settings button used by `CaptureWindow`'s unified header bar.
pub(crate) fn capture_header_settings_button(
    cx: &mut Context<WhiskyApp>,
) -> Button {
    let w = &crate::i18n::s().windows;
    Button::new("capture-provider-settings")
        .small()
        .icon(icon("settings-2", 13.))
        .label(w.capture_ai_settings_btn)
        .tooltip(w.capture_ai_settings_tip)
        .on_click(cx.listener(|this, _, _, cx| this.open_translation_settings(cx)))
}

#[cfg(test)]
mod tests {
    use crate::bridge::BridgeSupervisor;
    use crate::state::{PreviewPage, WhiskyApp};

    #[gpui_kit::test]
    fn back_navigation_only_applies_to_bottle_subpages(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|app| {
            gpui_kit::init(app);
            crate::bind_shortcuts(app);
        });
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, true)
        });

        window
            .update(cx, |this, _, cx| {
                assert!(!this.can_navigate_back(), "Library root has no Back");

                this.navigate(PreviewPage::Statistics, cx);
                assert!(!this.can_navigate_back(), "Statistics is top-level");

                this.navigate(PreviewPage::Launchers, cx);
                assert!(!this.can_navigate_back(), "Launchers is top-level");

                this.select_bottle(Some(0), cx);
                assert_eq!(this.preview.page, PreviewPage::Bottle);
                assert!(!this.can_navigate_back(), "Bottle root has no Back");

                let bottle_name = this.bottle_name().to_owned();
                this.navigate(PreviewPage::ConfigureBottle, cx);
                assert!(this.can_navigate_back(), "ConfigureBottle drills down");
                assert_eq!(this.back_destination_label(), bottle_name);

                this.navigate(PreviewPage::Programs, cx);
                assert!(this.can_navigate_back(), "Programs drills down");
                assert_eq!(this.back_destination_label(), "容器配置");
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), crate::platform::SHORTCUTS[8]);
        window
            .read_with(cx, |this, _| {
                assert_eq!(this.preview.page, PreviewPage::ConfigureBottle);
                assert!(this.can_navigate_back());
            })
            .unwrap();

        cx.simulate_keystrokes(window.into(), crate::platform::SHORTCUTS[8]);
        window
            .update(cx, |this, _, cx| {
                assert_eq!(this.preview.page, PreviewPage::Bottle);
                assert!(!this.can_navigate_back());

                this.navigate(PreviewPage::Processes, cx);
                assert!(this.can_navigate_back());
                this.navigate(PreviewPage::Launchers, cx);
                assert!(!this.can_navigate_back(), "Top-level resets history");
                assert!(this.preview.history.is_empty());
            })
            .unwrap();
    }

    #[cfg(not(target_os = "linux"))]
    #[gpui_kit::test]
    fn detached_settings_window_opens_switches_sections_and_cleans_up_on_close(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::state::SettingsSection;

        cx.update(|app| {
            gpui_kit::init(app);
            crate::bind_shortcuts(app);
        });
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, true)
        });

        window
            .update(cx, |this, _, cx| {
                this.open_settings(cx);
                assert!(this.settings);
            })
            .unwrap();
        cx.run_until_parked();

        let settings_handle = window
            .read_with(cx, |this, _| {
                assert!(this.settings);
                assert!(!this.inline_settings_active());
                assert_eq!(this.preview_title(), "游戏库");
                this.settings_window
                    .handle()
                    .expect("SettingsWindow must be opened")
            })
            .unwrap();

        for section in SettingsSection::ALL {
            window
                .update(cx, |this, _, cx| {
                    this.open_settings_section(section, None, cx);
                })
                .unwrap();
            cx.run_until_parked();
            settings_handle
                .update(cx, |_, win, cx| {
                    let _ = win.draw(cx);
                })
                .unwrap();
        }

        window
            .update(cx, |this, _, cx| {
                this.open_settings_section(SettingsSection::Translation, None, cx);
                this.translation_page.open_wizard(None);
            })
            .unwrap();
        cx.run_until_parked();
        settings_handle
            .update(cx, |_, win, cx| {
                let _ = win.draw(cx);
            })
            .unwrap();

        cx.simulate_keystrokes(settings_handle, "escape");
        cx.run_until_parked();
        window
            .read_with(cx, |this, _| {
                assert!(this.settings, "First Escape closes translation wizard only");
                assert!(this.translation_page.wizard.is_none());
                assert!(this.settings_window.is_active());
            })
            .unwrap();

        cx.simulate_keystrokes(settings_handle, crate::platform::SHORTCUTS[7]);
        cx.run_until_parked();
        window
            .read_with(cx, |this, _| {
                assert!(!this.settings);
                assert!(!this.settings_window.is_active());
                assert!(!this.translation_page.active);
                assert!(this.translation_page.inputs.is_empty());
                assert!(this.translation_page.prompt_input.is_none());
            })
            .unwrap();
    }

    #[cfg(not(target_os = "linux"))]
    #[gpui_kit::test]
    fn detached_capture_window_coexists_with_settings_window(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use crate::state::SettingsSection;

        cx.update(|app| {
            gpui_kit::init(app);
            crate::bind_shortcuts(app);
        });
        let (_events_tx, events_rx) = smol::channel::unbounded();
        let bridge = BridgeSupervisor::disconnected_for_tests();
        let window = cx.add_window(|window, cx| {
            WhiskyApp::new_with_bridge(window, cx, bridge, events_rx, true)
        });

        window
            .update(cx, |this, _, cx| {
                this.open_translation_for_game(0, cx);
                assert!(this.capture.panel_open);
            })
            .unwrap();
        cx.run_until_parked();

        let capture_handle = window
            .read_with(cx, |this, _| {
                assert!(!this.inline_capture_active());
                this.capture_window
                    .handle()
                    .expect("CaptureWindow must be opened")
            })
            .unwrap();

        window
            .update(cx, |this, _, cx| {
                this.open_translation_settings(cx);
            })
            .unwrap();
        cx.run_until_parked();

        window
            .read_with(cx, |this, _| {
                assert!(this.capture.panel_open, "CaptureWindow stays open");
                assert!(this.capture_window.handle().is_some());
                assert!(this.settings);
                assert_eq!(this.settings_section, SettingsSection::Translation);
                assert!(this.settings_window.handle().is_some());
            })
            .unwrap();

        cx.simulate_keystrokes(capture_handle, crate::platform::SHORTCUTS[7]);
        cx.run_until_parked();

        window
            .read_with(cx, |this, _| {
                assert!(!this.capture.panel_open);
                assert!(!this.capture_window.is_active());
                assert!(this.capture.hook_input.is_none());
                assert!(this.capture.thread_filter_input.is_none());
                assert!(this.settings && this.settings_window.is_active());
            })
            .unwrap();
    }
}
