//! Application entry, root view composition, and visual-test capture hook.

#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::base::{Presence, Spring, spring};
use gpui_kit::{prelude::*, *};

mod app_update;
mod artwork;
mod bridge;
mod capture;
mod config;
mod desktop_open;
mod host;
mod i18n;
mod controls;
mod create_bottle;
mod container_import;
mod gallery;
mod immersive;
mod library_layout;
mod library_view;
#[cfg(target_os = "linux")]
mod linux_overlay;
#[cfg(target_os = "windows")]
mod windows_overlay;
#[cfg(target_os = "windows")]
mod overflow_trace;
#[cfg(target_os = "linux")]
mod linux_hud;
mod model;
mod metadata_editor;
mod platform;
#[cfg(feature = "visual-test")]
mod motion_capture;
#[cfg(target_os = "windows")]
mod native;
#[cfg(target_os = "windows")]
mod native_metadata;
mod preferences;
mod onboarding;
mod processes;
mod program_settings;
mod programs;
mod runtime;
mod scan;
mod state;
mod theme;
mod translation;
mod ui_state;
mod upscale;
mod views;
mod windows;
mod winetricks;
#[cfg(test)]
mod headless_state_machine_tests;

use crate::controls::{col, icon, row};
#[cfg(feature = "visual-test")]
use crate::model::demo_data;
use crate::state::{PreviewPage, WhiskyApp};
#[cfg(feature = "visual-test")]
use crate::state::PreviewSheet;
use crate::theme::{ACCENT, AppearanceMode, DETAIL_MS, NOTICE_MS, SIDEBAR_MS};

gpui_kit::actions!(
    whisky_preview,
    [
        DismissPreview,
        CloseWindow,
        NavigateBack,
        QuitPreview,
        ToggleMotion,
        RefreshLibrary,
        FocusSearch,
        ShowAbout,
        ShowSettings,
        ToggleSidebar,
        AppearanceSystem,
        AppearanceLight,
        AppearanceDark,
        LanguageSystem,
        LanguageZhHans,
        LanguageJa,
        LanguageEn,
        LanguageKo,
        LanguageFr,
        LanguageEs,
        LanguageIt,
        LanguageDe,
        LanguageRu,
        MinimizeWindow,
        ZoomWindow,
        ToggleFullScreen
    ]
);

impl Render for WhiskyApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.artwork_cache.update(cx, |cache, cx| cache.begin_frame(cx));
        crate::theme::sync_theme(self.appearance, Some(window), cx);
        self.sync_localized_inputs(window, cx);
        self.scroll_needs_measurement.set(false);
        let ww = f32::from(window.viewport_size().width);
        let wh = f32::from(window.viewport_size().height);
        #[cfg(target_os = "linux")]
        let client_titlebar = matches!(window.window_decorations(), Decorations::Client { .. });
        #[cfg(target_os = "windows")]
        let client_titlebar = true;
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        let wh = wh
            - if client_titlebar {
                f32::from(gpui_kit::component::TITLE_BAR_HEIGHT)
            } else {
                0.
            };
        let sw = spring(
            "sidebar-width",
            self.sidebar.width(),
            Spring::new(Duration::from_millis(SIDEBAR_MS))
                .with_epsilon(0.1)
                .with_travel(self.motion),
            window,
            cx,
        );
        let width = ww - sw;
        // Native fullscreen auto-hides the lights; simple (borderless)
        // fullscreen removes them entirely. Either way content hugs the edge.
        let fullscreen = window.is_fullscreen() || window.is_simple_fullscreen();
        self.apply_pending_tag_inputs(window, cx);
        let backdrop = self.backdrop(window, cx);
        let body = if self.inline_capture_active() {
            self.capture_panel(window, cx)
        } else if self.inline_settings_active() {
            if self.translation_page.active {
                self.translation_settings(window, cx)
            } else {
                self.application_settings(cx)
            }
        } else if self.preview.page == PreviewPage::Statistics {
            self.statistics_page(width, cx)
        } else if self.preview.page == PreviewPage::Launchers {
            self.launchers_page(cx)
        } else if self.preview.page != PreviewPage::Library {
            self.preview_page(window, cx)
        } else {
            self.library_page(width, window, cx)
        };
        let presence = Presence::new("detail-presence", self.detail_open)
            .transition(self.timing(DETAIL_MS))
            .sample(window, cx);
        let notification = Presence::new("notice-presence", self.notice.is_some())
            .transition(self.timing(NOTICE_MS))
            .sample(window, cx);
        let mut root = col()
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(rgb(crate::theme::bg()))
            .font_family(platform::ui_font())
            .text_color(rgb(crate::theme::text()))
            .text_size(px(13.))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if this.onboarding.is_some() { return; }
                if let Some(path) = paths.0.first() {
                    this.open_external_file(path.clone(), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleMotion, _, cx| {
                this.motion = !this.motion;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &AppearanceSystem, _, cx| {
                this.set_appearance(AppearanceMode::System, cx);
            }))
            .on_action(cx.listener(|this, _: &AppearanceLight, _, cx| {
                this.set_appearance(AppearanceMode::Light, cx);
            }))
            .on_action(cx.listener(|this, _: &AppearanceDark, _, cx| {
                this.set_appearance(AppearanceMode::Dark, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageSystem, _, cx| {
                this.set_language(crate::i18n::LanguageMode::System, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageZhHans, _, cx| {
                this.set_language(crate::i18n::LanguageMode::ZhHans, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageJa, _, cx| {
                this.set_language(crate::i18n::LanguageMode::Ja, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageEn, _, cx| {
                this.set_language(crate::i18n::LanguageMode::En, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageKo, _, cx| {
                this.set_language(crate::i18n::LanguageMode::Ko, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageFr, _, cx| {
                this.set_language(crate::i18n::LanguageMode::Fr, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageEs, _, cx| {
                this.set_language(crate::i18n::LanguageMode::Es, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageIt, _, cx| {
                this.set_language(crate::i18n::LanguageMode::It, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageDe, _, cx| {
                this.set_language(crate::i18n::LanguageMode::De, cx);
            }))
            .on_action(cx.listener(|this, _: &LanguageRu, _, cx| {
                this.set_language(crate::i18n::LanguageMode::Ru, cx);
            }))
            .on_action(cx.listener(|this, _: &RefreshLibrary, _, cx| {
                if this.onboarding.is_some() { return; }
                if this.demo {
                    this.show_notice(crate::state::demo_notice(), cx);
                } else {
                    this.refresh_library(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                if this.onboarding.is_some()
                    || this.bridge_dialog
                    || this.bottle_dialog.is_some()
                    || this.engine_dialog.is_some()
                    || this.launch_chooser.is_some()
                    || this.clipboard_prompt.is_some()
                    || this.legacy_fullscreen_prompt.is_some()
                    || this.import_bottles.is_some()
                {
                    return;
                }
                if this.preview.sheet == Some(crate::state::PreviewSheet::Winetricks) {
                    if let Some(input) = this.preview.inputs.get("dependency-search") {
                        let handle = input.read(cx).focus_handle(cx);
                        window.focus(&handle, cx);
                    }
                    return;
                }
                if let Some(crate::state::PreviewSheet::GameInfo(_)) = this.preview.sheet {
                    if let Some(input) = this.game_info.as_ref().and_then(|page| page.input.as_ref()) {
                        let handle = input.read(cx).focus_handle(cx);
                        window.focus(&handle, cx);
                    }
                    return;
                }
                if this.preview.sheet.is_some() || this.detail_open {
                    return;
                }
                if this.settings || this.preview.page != PreviewPage::Library {
                    if this.settings {
                        this.close_settings(Some(window), cx);
                    }
                    this.preview.page = PreviewPage::Library;
                    this.selected_bottle = None;
                }
                let handle = this.search.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowAbout, _, cx| this.show_about(cx)))
            .on_action(cx.listener(|this, _: &ShowSettings, _, cx| this.open_settings(cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                let next = this.sidebar.toggled(this.sidebar_collapsed_style);
                this.set_sidebar(next, cx);
            }))
            .on_action(cx.listener(|this, _: &QuitPreview, window, cx| {
                this.request_quit(window, cx);
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
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| {
                window.remove_window();
            }))
            .on_action(cx.listener(|this, _: &NavigateBack, _, cx| {
                if this.preview.sheet.is_some() && !this.preview.sheet_history.is_empty() {
                    this.preview.sheet = this.preview.sheet_history.pop();
                    cx.notify();
                } else if this.detail_open && this.detail_character.is_some() {
                    this.detail_character = None;
                    cx.notify();
                } else if this.can_navigate_back() {
                    this.navigate_back(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &DismissPreview, window, cx| {
                if this.bridge_dialog {
                    this.bridge_dialog = false;
                } else if let Some(state) = &this.onboarding {
                    if !state.saving {
                        this.onboarding = None;
                    }
                } else if this.import_bottles.is_some() {
                    if !this.import_bottles.as_ref().is_some_and(|state| state.importing) {
                        this.import_bottles = None;
                    }
                } else if this.engine_dialog.is_some() {
                    this.dismiss_engine_dialog(cx);
                } else if this.bottle_dialog.is_some() {
                    this.dismiss_bottle_dialog(cx);
                } else if this.legacy_fullscreen_prompt.is_some() {
                    this.dismiss_legacy_fullscreen(cx);
                } else if this.clipboard_prompt.is_some() {
                    this.dismiss_clipboard(cx);
                } else if this.launch_chooser.is_some() {
                    this.close_launch_chooser(window, cx);
                    return;
                } else if this.setup_dialog && !this.setup_installing {
                    this.dismiss_setup(cx);
                } else if this.about {
                    this.dismiss_about();
                } else if this.preview.sheet == Some(crate::state::PreviewSheet::Executable)
                    && this.executable_picker.is_some()
                {
                    this.cancel_executable_picker(cx);
                    if this.preview.sheet.is_none() {
                        this.restore_preview_sheet_focus(window, cx);
                    }
                    return;
                } else if this.preview.sheet.is_some() {
                    this.preview_dismiss(cx);
                    this.restore_preview_sheet_focus(window, cx);
                    return;
                } else if this.detail_open {
                    this.close_detail(window, cx);
                    return;
                } else if this.staggered_selection.is_some() {
                    this.staggered_selection = None;
                } else if this.inline_settings_active()
                    && this.translation_page.active
                    && this.translation_page.wizard.is_some()
                {
                    this.translation_page.close_wizard(Some(window), Some(cx));
                } else if this.settings {
                    this.close_settings(Some(window), cx);
                }
                this.focus_handle.focus(window, cx);
                cx.notify();
            }))
            // The toolbar spans the window's full width so the sidebar's right
            // border starts below it, never slicing the traffic light cluster.
            .child(
                div()
                    .relative()
                    .child(self.toolbar(
                        platform::traffic_light_padding(0., 18., fullscreen),
                        window,
                        cx,
                    )),
            )
            .child(
                row()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(
                        div()
                            .w(px(sw))
                            .min_w(px(sw))
                            .h_full()
                            .overflow_hidden()
                            .flex_shrink_0()
                            .child(
                                div()
                                    .w(px(self.sidebar.layout_width()))
                                    .h_full()
                                    .tab_group()
                                    .tab_index(1)
                                    .child(self.sidebar(window, cx)),
                            ),
                    )
                    .child(
                        col()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .child(backdrop)
                            .child(div().relative().flex_1().min_h_0().w_full().child(body)),
                    ),
            );
        if presence.should_render() {
            if let Some(index) = self.detail {
                root = root.child(self.detail_panel(index, presence.progress, ww, wh, window, cx));
            }
        }
        if self.preview.sheet.is_none() && self.launch_chooser.is_none() {
            if let Some(handle) = self.preview_sheet_return_focus.take() {
                handle.focus(window, cx);
            }
        }
        if let Some(sheet) = self.preview.sheet {
            root = root.child(self.preview_sheet(sheet, ww, wh, window, cx));
        }
        if self.launch_chooser.is_some() {
            root = root.child(self.launch_chooser_dialog(ww, wh, window, cx));
        }
        if self.clipboard_prompt.is_some() {
            root = root.child(self.clipboard_dialog(ww, wh, cx));
        }
        if self.legacy_fullscreen_prompt.is_some() {
            root = root.child(self.legacy_fullscreen_dialog(ww, wh, cx));
        }
        if self.bottle_dialog.is_some() {
            root = root.child(self.bottle_dialog_view(ww, wh, window, cx));
        }
        if self.engine_dialog.is_some() {
            root = root.child(self.engine_dialog_view(ww, wh, window, cx));
        }
        if self.import_bottles.is_some() {
            root = root.child(self.import_bottles_view(ww, wh, cx));
        }
        if self.setup_dialog && !self.settings_window.is_active() {
            root = root.child(self.setup_dialog_view(ww, wh, cx));
        }
        if self.about {
            root = root.child(self.about_dialog(ww, wh, cx));
        }
        if self.onboarding.is_some() && !self.settings_window.is_active() {
            root = root.child(self.onboarding_view(ww, wh, cx));
        }
        if self.inline_settings_active()
            && self.translation_page.active
            && self.translation_page.wizard.is_some()
        {
            root = root.child(self.translation_wizard_modal(ww, wh, window, cx));
        }
        // Recovery and quit errors must stay actionable even during setup saves.
        if self.bridge_dialog {
            root = root.child(self.bridge_status_dialog(ww, wh, cx));
        }
        if notification.should_render() {
            let message = self.notice.clone().unwrap_or_default();
            root = root.child(
                row()
                    .absolute()
                    .bottom(px(24.))
                    .left(px(sw + 24.))
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(12.))
                    .rounded(px(12.))
                    .bg(rgb(crate::theme::toast_bg()))
                    .border_1()
                    .border_color(rgba(crate::theme::toast_border()))
                    .shadow_lg()
                    .opacity(notification.progress)
                    .child(icon("info", 16.).text_color(rgb(ACCENT)))
                    .child(message),
            );
        }
        if self.scroll_needs_measurement.get() {
            window.request_animation_frame();
        }
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            col()
                .size_full()
                .when(client_titlebar, |column| {
                    column.child(
                        gpui_kit::component::TitleBar::new()
                            .child("Maltoria")
                            .on_close_window(cx.listener(|this, _, window, cx| {
                                this.request_quit(window, cx);
                            })),
                    )
                })
                .child(div().relative().flex_1().min_h_0().child(root))
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        root
    }
}
#[cfg(feature = "visual-test")]
fn capture_preview(handle: AnyWindowHandle, view: Entity<WhiskyApp>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("screenshots");
        std::fs::create_dir_all(&out).unwrap();
        let names = [
            "library",
            "timeline",
            "details",
            "settings",
            "compact",
            "classic",
            "bottle",
            "configure-bottle",
            "programs",
            "run-settings",
            "scan-settings",
            "winetricks",
            "compact-bottle",
            "welcome",
            "source-preferences",
            "metadata-editor",
            "statistics",
            "statistics-compact",
            "immersive",
            "immersive-compact",
            "staggered",
            "staggered-compact",
            "timeline-hover",
            "sidebar-rail",
        ];
        for (stage, name) in names.into_iter().enumerate() {
            if std::env::var_os("WHISKY_STATISTICS_CAPTURE").is_some() && stage < 16 {
                continue;
            }
            handle
                .update(cx, |_, window, cx| {
                    view.update(cx, |this, cx| {
                        this.translation_page.dismiss();
                        this.settings = false;
                        this.detail_open = false;
                        this.preview.sheet = None;
                        this.onboarding = None;
                        this.preview.page = PreviewPage::Library;
                        this.mode = 0;
                        this.selected_bottle = None;
                        match stage {
                            0 => window.resize(size(px(1280.), px(820.))),
                            1 => this.mode = 1,
                            2 => {
                                this.mode = 1;
                                this.open_detail(
                                    demo_data()
                                        .iter()
                                        .position(|g| g.title.starts_with("Saltlight"))
                                        .unwrap(),
                                    cx,
                                );
                            }
                            3 => this.settings = true,
                            4 => window.resize(size(px(900.), px(650.))),
                            5 => this.mode = 2,
                            6 => {
                                window.resize(size(px(1280.), px(820.)));
                                this.selected_bottle = Some(0);
                                this.preview.page = PreviewPage::Bottle;
                            }
                            7 => {
                                this.selected_bottle = Some(0);
                                this.preview.page = PreviewPage::ConfigureBottle;
                            }
                            8 => {
                                this.selected_bottle = Some(0);
                                this.preview.page = PreviewPage::Programs;
                            }
                            9 => this.open_preview_sheet(PreviewSheet::RunSettings(0), cx),
                            10 => this.open_preview_sheet(PreviewSheet::Scan, cx),
                            11 => this.open_preview_sheet(PreviewSheet::Winetricks, cx),
                            13 => {
                                window.resize(size(px(900.), px(650.)));
                                this.open_onboarding(true, cx);
                            }
                            14 => {
                                window.resize(size(px(920.), px(680.)));
                                this.open_onboarding(false, cx);
                                if let Some(state) = &mut this.onboarding {
                                    state.expanded = true;
                                }
                            }
                            15 => {
                                window.resize(size(px(1000.), px(780.)));
                                let mut editor = metadata_editor::MetadataEditor::default();
                                editor.accept_reply(&serde_json::json!({
                                    "record": {
                                        "version":1, "defaultSource":"bangumi",
                                        "categorySources":{"characters":"vndb"},
                                        "bindings":{"bangumi":"123","vndb":"v17"},
                                        "snapshots":{
                                            "bangumi":{"source":"bangumi","sourceID":"123","title":"作品的中文名称","summary":"来自所选平台的简介。"},
                                            "vndb":{"source":"vndb","sourceID":"v17","title":"Original title","characters":[{"id":"c1","name":"Character","originalName":"人物","role":"main"}]}
                                        }
                                    },
                                    "capabilities":{
                                        "bangumi":{"refresh":true,"characters":true},
                                        "vndb":{"refresh":true,"characters":true},
                                        "steam":{"refresh":true,"characters":false},
                                        "ymgal":{"refresh":true,"characters":false}
                                    }
                                })).unwrap();
                                this.game_info = Some(state::GameInfoPage {
                                    editor, container: "visual-fixture".into(), entry_id: "demo:0".into(),
                                    title: "作品的中文名称".into(), input: None, searching: false,
                                    searching_sources: Vec::new(), sources_expanded: false,
                                    selected_character: None,
                                    applying: false, error: None, groups: Vec::new(), matches: None,
                                    config_message: None, config_error: None,
                                });
                                this.preview.sheet = Some(PreviewSheet::GameInfo(0));
                            }
                            16 | 17 => {
                                window.resize(if stage == 16 {
                                    size(px(1280.), px(900.))
                                } else {
                                    size(px(900.), px(700.))
                                });
                                this.preview.page = PreviewPage::Statistics;
                                this.preview.scroll_handles.borrow_mut().remove("statistics-scroll");
                            }
                            18 | 19 => {
                                window.resize(if stage == 18 {
                                    size(px(1280.), px(820.))
                                } else {
                                    size(px(900.), px(650.))
                                });
                                this.mode = 3;
                            }
                            20 | 21 => {
                                window.resize(if stage == 20 {
                                    size(px(1280.), px(820.))
                                } else {
                                    size(px(900.), px(650.))
                                });
                                this.mode = 5;
                                // Focus one card per side so the rail shows
                                // the accent node, connector and dimmed
                                // siblings in both directions.
                                let ordered = this.ordered(cx);
                                if let Some(&index) = ordered.get(if stage == 20 { 0 } else { 1 }) {
                                    this.select_staggered(index, cx);
                                }
                            }
                            22 => {
                                window.resize(size(px(1280.), px(820.)));
                                this.mode = 1;
                                // Hover state lights the hovered row's
                                // node→card link and its play button.
                                let ordered = this.timeline_ordered(cx);
                                if let Some(&index) = ordered.get(1) {
                                    this.hovered = Some(index);
                                }
                            }
                            // Last: the icon rail — the stage reset above does
                            // not restore the sidebar mode.
                            23 => {
                                window.resize(size(px(1280.), px(820.)));
                                this.set_sidebar(crate::state::SidebarMode::Icons, cx);
                            }
                            _ => {
                                window.resize(size(px(800.), px(580.)));
                                this.selected_bottle = Some(0);
                                this.preview.page = PreviewPage::ConfigureBottle;
                            }
                        }
                        cx.notify();
                    });
                })
                .unwrap();
            cx.background_executor()
                .timer(Duration::from_millis(750))
                .await;
            handle
                .update(cx, |_, window, cx| {
                    // `render_to_image` snapshots the last painted scene; an
                    // unchanged size (stage 23 resizes to the current extent)
                    // leaves the stage's update unpainted. Draw explicitly.
                    let _ = window.draw(cx);
                    window
                        .render_to_image()
                        .expect("GPU capture failed")
                        .save(out.join(format!("{name}.png")))
                        .unwrap();
                })
                .unwrap();
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

/// Value following `flag`, if the flag is present. `None` when absent, `Err`
/// when the flag has no value (so the caller can print usage).
fn cli_value<'a>(arguments: &'a [String], flag: &str) -> Result<Option<&'a str>, ()> {
    match arguments.iter().position(|argument| argument == flag) {
        Some(index) => match arguments.get(index + 1) {
            Some(value) if !value.starts_with("--") => Ok(Some(value.as_str())),
            _ => Err(()),
        },
        None => Ok(None),
    }
}

/// Native menu bar. GPUI does not add the standard macOS app menu, so About,
/// Settings and Quit are declared explicitly; text fields rely on the Editing
/// OS actions for their standard shortcuts.
pub(crate) fn install_menus(cx: &mut App) {
    let m = &crate::i18n::s().menus;
    let lang_mode = crate::i18n::language_mode();
    cx.set_menus(vec![
        Menu::new("Maltoria").items(vec![
            MenuItem::action(m.about, ShowAbout),
            MenuItem::separator(),
            MenuItem::action(m.settings, ShowSettings),
            MenuItem::separator(),
            MenuItem::action(m.quit, QuitPreview),
        ]),
        Menu::new(m.edit_menu).items(vec![
            MenuItem::os_action(m.undo, NoAction, OsAction::Undo),
            MenuItem::os_action(m.redo, NoAction, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(m.cut, NoAction, OsAction::Cut),
            MenuItem::os_action(m.copy, NoAction, OsAction::Copy),
            MenuItem::os_action(m.paste, NoAction, OsAction::Paste),
            MenuItem::os_action(m.select_all, NoAction, OsAction::SelectAll),
        ]),
        Menu::new(m.view_menu).items(vec![
            MenuItem::action(m.navigate_back, NavigateBack),
            MenuItem::separator(),
            MenuItem::action(m.focus_search, FocusSearch),
            MenuItem::action(m.refresh_library, RefreshLibrary),
            MenuItem::separator(),
            MenuItem::action(m.toggle_sidebar, ToggleSidebar),
            MenuItem::separator(),
            MenuItem::action(m.appearance_system, AppearanceSystem),
            MenuItem::action(m.appearance_light, AppearanceLight),
            MenuItem::action(m.appearance_dark, AppearanceDark),
            MenuItem::submenu(Menu::new(m.language_submenu).items(vec![
                MenuItem::action(m.language_system, LanguageSystem)
                    .checked(lang_mode == crate::i18n::LanguageMode::System),
                MenuItem::separator(),
                MenuItem::action("简体中文", LanguageZhHans)
                    .checked(lang_mode == crate::i18n::LanguageMode::ZhHans),
                MenuItem::action("日本語", LanguageJa)
                    .checked(lang_mode == crate::i18n::LanguageMode::Ja),
                MenuItem::action("English", LanguageEn)
                    .checked(lang_mode == crate::i18n::LanguageMode::En),
                MenuItem::action("한국어", LanguageKo)
                    .checked(lang_mode == crate::i18n::LanguageMode::Ko),
                MenuItem::action("Français", LanguageFr)
                    .checked(lang_mode == crate::i18n::LanguageMode::Fr),
                MenuItem::action("Español", LanguageEs)
                    .checked(lang_mode == crate::i18n::LanguageMode::Es),
                MenuItem::action("Italiano", LanguageIt)
                    .checked(lang_mode == crate::i18n::LanguageMode::It),
                MenuItem::action("Deutsch", LanguageDe)
                    .checked(lang_mode == crate::i18n::LanguageMode::De),
                MenuItem::action("Русский", LanguageRu)
                    .checked(lang_mode == crate::i18n::LanguageMode::Ru),
            ])),
            MenuItem::separator(),
            MenuItem::action(m.settings, ShowSettings),
        ]),
        Menu::new(m.window_menu).items(vec![
            MenuItem::action(m.close_window, CloseWindow),
            MenuItem::separator(),
            MenuItem::action(m.minimize, MinimizeWindow),
            MenuItem::action(m.zoom, ZoomWindow),
            MenuItem::separator(),
            MenuItem::action(m.fullscreen, ToggleFullScreen),
        ]),
    ]);
}

/// Window-level key bindings, shared by `main` and headless UI tests so the
/// simulated-keystroke tests drive the same binding table the app installs.
pub(crate) fn bind_shortcuts(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", DismissPreview, None),
        KeyBinding::new(platform::SHORTCUTS[0], QuitPreview, None),
        KeyBinding::new(platform::SHORTCUTS[1], RefreshLibrary, None),
        KeyBinding::new(platform::SHORTCUTS[2], ToggleMotion, None),
        KeyBinding::new(platform::SHORTCUTS[3], ShowSettings, None),
        KeyBinding::new(platform::SHORTCUTS[4], ToggleSidebar, None),
        KeyBinding::new(platform::SHORTCUTS[5], MinimizeWindow, None),
        KeyBinding::new(platform::SHORTCUTS[6], FocusSearch, None),
        KeyBinding::new(platform::SHORTCUTS[7], CloseWindow, None),
        KeyBinding::new(platform::SHORTCUTS[8], NavigateBack, None),
    ]);
}

fn main() {
    #[cfg(target_os = "windows")]
    overflow_trace::install();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--print-bridge-status") {
        std::process::exit(bridge::print_status());
    }
    if arguments.iter().any(|argument| argument == "--print-runtime") {
        std::process::exit(bridge::print_runtime());
    }
    if arguments.iter().any(|argument| argument == "--print-config") {
        const USAGE: &str = "usage: whisky-gpui --print-config [--bottle <name-or-path>]";
        let bottle = match cli_value(&arguments, "--bottle") {
            Ok(bottle) => bottle,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        std::process::exit(bridge::print_config(bottle));
    }
    if arguments.iter().any(|argument| argument == "--print-folders") {
        const USAGE: &str = "usage: whisky-gpui --print-folders [--bottle <name-or-path>]";
        let bottle = match cli_value(&arguments, "--bottle") {
            Ok(bottle) => bottle,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        std::process::exit(bridge::print_folders(bottle));
    }
    if arguments.iter().any(|argument| argument == "--print-winetricks") {
        const USAGE: &str = "usage: whisky-gpui --print-winetricks [--bottle <name-or-path>]";
        let bottle = match cli_value(&arguments, "--bottle") {
            Ok(bottle) => bottle,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        std::process::exit(bridge::print_winetricks(bottle));
    }
    if arguments.iter().any(|argument| argument == "--print-programs") {
        const USAGE: &str = "usage: whisky-gpui --print-programs [--bottle <name-or-path>]";
        let bottle = match cli_value(&arguments, "--bottle") {
            Ok(bottle) => bottle,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        std::process::exit(bridge::print_programs(bottle));
    }
    if arguments.iter().any(|argument| argument == "--print-processes") {
        const USAGE: &str = "usage: whisky-gpui --print-processes [--bottle <name-or-path>]";
        let bottle = match cli_value(&arguments, "--bottle") {
            Ok(bottle) => bottle,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        std::process::exit(bridge::print_processes(bottle));
    }
    if arguments.iter().any(|argument| argument == "--runtime-watch") {
        const USAGE: &str =
            "usage: whisky-gpui --runtime-watch [--seconds N] [--bottle <name-or-path>]";
        let seconds = match cli_value(&arguments, "--seconds") {
            Ok(Some(raw)) => match raw.parse::<u64>() {
                Ok(seconds) => seconds,
                Err(_) => {
                    eprintln!("{USAGE}");
                    std::process::exit(2);
                }
            },
            Ok(None) => 5,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        let bottle = match cli_value(&arguments, "--bottle") {
            Ok(bottle) => bottle,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        std::process::exit(bridge::watch_runtime(bottle, seconds));
    }
    if arguments.iter().any(|argument| argument == "--diagnose-launch") {
        const USAGE: &str =
            "usage: whisky-gpui --diagnose-launch --bottle <name-or-path> [--entry <substring>]";
        let bottle = match cli_value(&arguments, "--bottle") {
            Ok(Some(bottle)) => Some(bottle),
            Ok(None) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        let entry = match cli_value(&arguments, "--entry") {
            Ok(entry) => entry,
            Err(()) => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };
        std::process::exit(bridge::diagnose_launch(bottle, entry));
    }
    if arguments.iter().any(|argument| argument == "--print-library") {
        let all = arguments.iter().any(|argument| argument == "--all");
        let bottle = match arguments.iter().position(|argument| argument == "--bottle") {
            Some(index) => match arguments.get(index + 1) {
                Some(value) if !value.starts_with("--") => Some(value.as_str()),
                _ => {
                    eprintln!(
                        "usage: whisky-gpui --print-library [--bottle <name-or-path>] [--all]"
                    );
                    std::process::exit(2);
                }
            },
            None => None,
        };
        std::process::exit(bridge::print_library(bottle, all));
    }
    let startup_file = match desktop_open::parse(&arguments) {
        Ok(value) => value,
        Err(reason) => { eprintln!("{reason}"); std::process::exit(2); }
    };
    let pending_urls: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    let url_dispatch: Rc<RefCell<Option<Box<dyn FnMut(Vec<PathBuf>)>>>> =
        Rc::new(RefCell::new(None));
    let application = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
    {
        let pending_urls = pending_urls.clone();
        let url_dispatch = url_dispatch.clone();
        application.on_open_urls(move |urls| {
            let paths = urls
                .iter()
                .map(|raw| ui_state::path_from_open_url(raw))
                .collect::<Vec<_>>();
            if let Some(dispatch) = url_dispatch.borrow_mut().as_mut() {
                dispatch(paths);
            } else {
                // The launch URL can arrive before the window exists.
                pending_urls.borrow_mut().extend(paths);
            }
        });
    }
    application.run(move |cx| {
            gpui_kit::init(cx);
            let language = ui_state::load_language();
            i18n::init(language);
            bind_shortcuts(cx);
            install_menus(cx);
            let appearance = ui_state::load_appearance();
            cx.set_window_appearance(appearance.window_override());
            theme::sync_theme(appearance, None, cx);
            let bounds = ui_state::initial_window_bounds(cx);
            let (_handle, _view) = gpui_kit::open_window(
                WindowOptions {
                    #[cfg(target_os = "linux")]
                    app_id: Some("com.daydalek.Maltoria".into()),
                    #[cfg(target_os = "linux")]
                    window_decorations: Some(WindowDecorations::Client),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Maltoria".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(15.), px(21.))),
                    }),
                    window_bounds: Some(bounds),
                    window_min_size: Some(size(px(800.), px(580.))),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| WhiskyApp::new(window, cx)),
            )
            .expect("Failed to open GPUI preview");
            let main_id = _handle.window_id();
            let app_weak = _view.downgrade();
            cx.on_window_closed(move |cx, closed_id| {
                if closed_id == main_id || !cx.windows().iter().any(|handle| handle.window_id() == main_id) {
                    for handle in cx.windows() {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                    cx.quit();
                } else if let Some(app) = app_weak.upgrade() {
                    let _ = app.update(cx, |this, cx| {
                        this.handle_secondary_window_closed(closed_id, cx);
                    });
                }
            })
            .detach();
            #[cfg(target_os = "linux")]
            {
                linux_overlay::start(&_view, cx);
                linux_hud::start(&_view, cx);
            }
            #[cfg(target_os = "windows")]
            {
                windows_overlay::start(&_view, cx);
            }
            if let Some(file) = startup_file.clone() {
                let _ = _view.update(cx, |this, cx| this.receive_external_file(file, cx));
            }
            let async_cx = cx.to_async();
            let url_view = _view.clone();
            for path in std::mem::take(&mut *pending_urls.borrow_mut()) {
                let _ = url_view.update(cx, |this, cx| this.open_external_file(path, cx));
            }
            *url_dispatch.borrow_mut() = Some(Box::new(move |paths| {
                let cx = async_cx.clone();
                let view = url_view.clone();
                cx.update(|cx| {
                    let _ = view.update(cx, |this, cx| {
                        for path in paths {
                            this.open_external_file(path, cx);
                        }
                    });
                });
            }));
            cx.activate(true);
            #[cfg(feature = "visual-test")]
            if std::env::var_os("WHISKY_MOTION_CAPTURE").is_some() {
                motion_capture::run(_handle, _view.clone(), cx);
            }
            #[cfg(feature = "visual-test")]
            if std::env::var_os("WHISKY_DEMO_CAPTURE").is_some() {
                capture_preview(_handle, _view, cx);
            }
        });
}
