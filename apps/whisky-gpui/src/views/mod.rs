//! View layer: shared form widgets, menus, scrolling, and the page modules.

pub(crate) mod bottle;
pub(crate) mod capture;
pub(crate) mod dialogs;
pub(crate) mod gallery;
pub(crate) mod immersive;
pub(crate) mod import_bottles;
pub(crate) mod launchers;
pub(crate) mod library;
pub(crate) mod settings;
pub(crate) mod staggered;
pub(crate) mod statistics;
pub(crate) mod translation;
pub(crate) mod onboarding;
pub(crate) mod view_picker;

use gpui_kit::base::{ScrollBounce, ScrollBounceMotion};
use gpui_kit::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    switch::Switch,
};
use gpui_kit::{prelude::*, *};

use crate::controls::*;
use crate::state::{PreviewPage, PreviewSheet, WhiskyApp};

#[derive(Clone, Copy)]
pub(crate) enum Command {
    Page(PreviewPage),
    Sheet(PreviewSheet),
    Details(usize),
    Configure(usize),
    /// Run the card's primary action (launch, cancel, or stop).
    Runtime(usize),
    Translation(usize),
    /// Name the executable a folder entry starts, without running it.
    Executable(usize),
    /// Flip a launcher's "open with Whisky" auto-start flag.
    LauncherAutoStart(usize),
    Toggle(&'static str),
    /// Set the shelf's cover width; zero restores the automatic shelf.
    CoverSize(f32),
    Choose(&'static str, &'static str),
    Sort(usize),
    Notice,
    Hide(usize),
    /// Select the entry's executable (or folder) in Finder.
    Reveal(usize),
    /// Open the actual installation directory in the host file manager.
    OpenGameDirectory(usize),
    /// Create a macOS shortcut for the entry's executable.
    Shortcut,
    /// Move an entry into another bottle.
    Move(usize),
    /// Open the selected bottle's `drive_c` in Finder.
    OpenDrive,
    /// Re-read the bottle catalogue and every bottle's entries.
    RefreshLibrary,
    /// Terminate all running processes in the selected bottle.
    StopAllProcesses,
    /// Sidebar bottle actions.
    BottleRename(usize),
    BottleDuplicate(usize),
    BottleRemove(usize),
    BottleMove(usize),
    BottleExport(usize),
    BottleReveal(usize),
    /// Close the rename/duplicate/remove modal.
    BottleDialogCancel,
    /// Accept the rename/duplicate modal.
    BottleDialogConfirm,
    /// Run a Windows file in the selected bottle through the native picker.
    BottleRun,
    /// Choose an arbitrary Windows file and add it to the library as a pin.
    AddProgram,
    /// Launch a Wine builtin (winecfg, regedit, control, terminal).
    BottleTool(&'static str),
    BottleRepair,
    CleanShaderCache,
    /// Collect `bottle.diagnostics` into the Diagnostics sheet.
    RunDiagnostics,
    /// Write the stability report to a chosen file.
    ExportDiagnostics,
    /// Run `audio.test` with the 440 Hz tone.
    TestAudio,
    /// Run the curated troubleshooting checks.
    RunBottleChecks,
    /// Re-read `processes.list` for the selected bottle.
    RefreshProcesses,
    /// Metadata, cover and GameDB sheet for one library card.
    GameInfo(usize),
    MetadataSearch,
    MetadataApply(usize, usize),
    MetadataClear,
    MetadataCover,
    MetadataCoverReset,
    GameDbApply(usize),
    /// Auto-match metadata for every folder entry in the selected bottle.
    SyncMetadata,
    EditTags(usize),
    ManageTags,
    FilterStudio(usize),
    ReplayTagBarHint,
    InstallWineRuntime,
    InstallRosetta,
    DismissSetup,
}
#[derive(Clone)]
struct MenuEntry {
    label: SharedString,
    command: Option<Command>,
    checked: bool,
    view: WeakEntity<WhiskyApp>,
}
impl MenuEntry {
    fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    fn separator(view: WeakEntity<WhiskyApp>) -> Self {
        Self {
            label: "".into(),
            command: None,
            checked: false,
            view,
        }
    }
    fn build(&self) -> PopupMenuItem {
        let Some(command) = self.command else {
            return PopupMenuItem::separator();
        };
        let view = self.view.clone();
        PopupMenuItem::new(self.label.clone())
            .checked(self.checked)
            .on_click(move |_, _, cx| {
                let _ = view.update(cx, |this, cx| this.command(command, cx));
            })
    }
}

/// Interpolates two `0xRRGGBB` colours.
pub(crate) fn mix_rgb(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    let mut out = 0u32;
    for shift in [16u32, 8, 0] {
        let from = (a >> shift) & 0xff;
        let to = (b >> shift) & 0xff;
        out |= ((from as f32 + (to as f32 - from as f32) * t).round() as u32) << shift;
    }
    out
}

/// Interpolates two `0xRRGGBBAA` colours, alpha included.
pub(crate) fn mix_rgba(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    let mut out = 0u32;
    for shift in [24u32, 16, 8, 0] {
        let from = (a >> shift) & 0xff;
        let to = (b >> shift) & 0xff;
        out |= ((from as f32 + (to as f32 - from as f32) * t).round() as u32) << shift;
    }
    out
}

/// `0xRRGGBB` re-encoded as `0xRRGGBBAA` for `rgba` backgrounds/paths.
pub(crate) fn fade_rgb(color: u32, alpha: f32) -> u32 {
    (color << 8) | (alpha.clamp(0., 1.) * 255.).round() as u32
}

/// Connector stroke, in points; the glow pass is three times as wide.
pub(crate) const CONNECTOR_WIDTH: f32 = 2.5;
/// The connector dips by a fraction of its span, capped, so it reads as a
/// drawn thread rather than a wire.
const CONNECTOR_SAG: f32 = 0.12;
const CONNECTOR_SAG_MAX: f32 = 9.;

fn lerp_point(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// One connector cubic, already trimmed for its grow/shrink progress.
pub(crate) struct ConnectorPaint {
    pub(crate) start: (f32, f32),
    pub(crate) control_a: (f32, f32),
    pub(crate) control_b: (f32, f32),
    pub(crate) end: (f32, f32),
    pub(crate) alpha: f32,
}

/// The timeline link's shape: leaves the rail node horizontally, sags
/// slightly, and arrives horizontally at the card edge. `progress` trims the
/// curve with de Casteljau so it grows out of the node and retracts back.
pub(crate) fn connector_curve(start: (f32, f32), end: (f32, f32), progress: f32) -> ConnectorPaint {
    let span = end.0 - start.0;
    let sag = (span.abs() * CONNECTOR_SAG).min(CONNECTOR_SAG_MAX);
    let control_a = (start.0 + span * 0.45, start.1 + sag);
    let control_b = (end.0 - span * 0.45, end.1 + sag);
    let t = progress.clamp(0., 1.);
    let q0 = lerp_point(start, control_a, t);
    let q1 = lerp_point(control_a, control_b, t);
    let q2 = lerp_point(control_b, end, t);
    let r0 = lerp_point(q0, q1, t);
    let r1 = lerp_point(q1, q2, t);
    ConnectorPaint {
        start,
        control_a: q0,
        control_b: r0,
        end: lerp_point(r0, r1, t),
        alpha: t,
    }
}

/// Paints accent connector cubics from content-space coordinates; `origin` is
/// the canvas element's window-space origin.
pub(crate) fn paint_connectors(
    origin: Point<Pixels>,
    connectors: &[ConnectorPaint],
    window: &mut Window,
) {
    let to_window = |p: (f32, f32)| origin + point(px(p.0), px(p.1));
    for connector in connectors {
        for (width, alpha) in [
            (CONNECTOR_WIDTH * 3., connector.alpha * 0.22),
            (CONNECTOR_WIDTH, connector.alpha * 0.95),
        ] {
            let mut builder = PathBuilder::stroke(px(width));
            builder.move_to(to_window(connector.start));
            builder.cubic_bezier_to(
                to_window(connector.end),
                to_window(connector.control_a),
                to_window(connector.control_b),
            );
            if let Ok(path) = builder.build() {
                window.paint_path(path, rgba(fade_rgb(crate::theme::ACCENT, alpha)));
            }
        }
    }
}

impl WhiskyApp {
    fn menu_item(
        &self,
        label: impl Into<SharedString>,
        command: Command,
        cx: &Context<Self>,
    ) -> MenuEntry {
        let view = cx.entity().downgrade();
        MenuEntry {
            label: label.into(),
            command: Some(command),
            checked: false,
            view,
        }
    }
    pub(crate) fn options_menu(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let is_library = self.preview.page == PreviewPage::Library && !self.settings;
        let has_bottle = self.selected_bottle.is_some() && !self.settings;
        let sort_names = [
            strings.library.sort_recent,
            strings.library.sort_name,
            strings.library.sort_bottle,
        ];
        let mut items = vec![];
        let tooltip = if is_library {
            let current_sort = sort_names
                .get(self.preview.sort)
                .copied()
                .unwrap_or(strings.library.sort_recent);
            SharedString::from(crate::i18n::fmt::sort_tooltip(current_sort, self.reversed))
        } else {
            SharedString::from(strings.common.more_actions)
        };
        if is_library {
            // Library view actions only; appearance, motion and layout
            // preferences live in the settings page's library section.
            for (i, label) in sort_names.into_iter().enumerate() {
                items.push(
                    self.menu_item(crate::i18n::s().library.sort_by(label), Command::Sort(i), cx)
                        .checked(self.preview.sort == i),
                );
            }
            items.push(
                self.menu_item(strings.library.sort_reverse, Command::Toggle("reverse-sort"), cx)
                    .checked(self.reversed),
            );
            items.push(
                self.menu_item(strings.library.group_by_studio, Command::Toggle("group-by-studio"), cx)
                    .checked(self.group_by_studio),
            );
            let hidden = self.hidden_count();
            let hidden_label = crate::i18n::s().library.show_hidden_games(hidden);
            items.push(
                self.menu_item(hidden_label, Command::Toggle("show-hidden"), cx)
                    .checked(self.show_hidden()),
            );
            items.push(MenuEntry::separator(cx.entity().downgrade()));
            if self.mode == 0 && self.cover_width > f32::EPSILON {
                items.push(self.menu_item(
                    strings.library.reset_cover_size,
                    Command::CoverSize(crate::library_layout::AUTOMATIC_COVER_WIDTH),
                    cx,
                ));
                items.push(MenuEntry::separator(cx.entity().downgrade()));
            }
            items.push(self.menu_item(strings.library.sync_metadata, Command::SyncMetadata, cx));
            items.push(self.menu_item(strings.library.manage_scan_folders_btn, Command::Sheet(PreviewSheet::Scan), cx));
            items.push(self.menu_item(strings.library.manage_tags, Command::ManageTags, cx));
            items.push(
                self.menu_item(strings.library.tag_bar_pull_down_item, Command::Toggle("tag-bar-pull-down"), cx)
                    .checked(self.tag_bar_mode == crate::ui_state::TagBarMode::PullDown),
            );
            if self.tag_bar_mode == crate::ui_state::TagBarMode::PullDown
                && self.has_filter_bar_content()
            {
                items.push(self.menu_item(
                    if self.pull_reveal.is_open() {
                        strings.library.collapse_filter_bar
                    } else {
                        strings.library.expand_filter_bar
                    },
                    Command::Toggle("toggle-tag-drawer"),
                    cx,
                ));
            }
            items.push(MenuEntry::separator(cx.entity().downgrade()));
            items.push(self.menu_item(strings.menus.refresh_library, Command::RefreshLibrary, cx));
        } else if has_bottle {
            items.push(self.menu_item(
                strings.bottle.configure_bottle,
                Command::Page(PreviewPage::ConfigureBottle),
                cx,
            ));
            items.push(self.menu_item(strings.bottle.installed_programs, Command::Page(PreviewPage::Programs), cx));
            items.push(self.menu_item(strings.bottle.running_processes, Command::Page(PreviewPage::Processes), cx));
            items.push(self.menu_item(strings.bottle.game_configs, Command::Page(PreviewPage::GameConfigs), cx));
            items.push(MenuEntry::separator(cx.entity().downgrade()));
            items.push(self.menu_item(strings.library.manage_scan_folders_btn, Command::Sheet(PreviewSheet::Scan), cx));
            items.push(self.menu_item(strings.bottle.winetricks_ellipsis, Command::Sheet(PreviewSheet::Winetricks), cx));
            items.push(self.menu_item(strings.library.create_new_bottle, Command::Sheet(PreviewSheet::CreateBottle), cx));
        } else {
            items.push(self.menu_item(strings.library.back_to_library, Command::Page(PreviewPage::Library), cx));
            items.push(self.menu_item(strings.library.create_bottle_btn, Command::Sheet(PreviewSheet::CreateBottle), cx));
            items.push(MenuEntry::separator(cx.entity().downgrade()));
            items.push(self.menu_item(strings.library.refresh_data, Command::RefreshLibrary, cx));
        }
        Button::new("options-menu")
            .label("•••")
            .rounded_full()
            .w(px(34.))
            .h(px(34.))
            .tooltip(tooltip)
            .dropdown_menu(move |mut menu, _, _| {
                for item in &items {
                    menu = menu.item(item.build());
                }
                menu
            })
            .into_any_element()
    }
    pub(crate) fn game_menu(
        &self,
        index: usize,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let game = self.game(index);
        let hidden = game.is_some_and(|game| game.hidden);
        let is_folder = game.is_some_and(|game| game.is_folder);
        let multi_exe = game.is_some_and(|game| game.multi_executable);
        let has_path = game.is_some_and(|game| game.executable.is_some());
        let entry_tags: Vec<String> = game.map(|g| g.tags.clone()).unwrap_or_default();
        let library_tags: Vec<String> =
            crate::library_view::collect_tag_counts(self.games(), true)
                .into_iter()
                .map(|(tag, _)| tag)
                .collect();
        let view = cx.entity().downgrade();
        let strings = crate::i18n::s();
        let l = &strings.library;
        let mut items = vec![
            self.menu_item(strings.common.run, Command::Runtime(index), cx),
            MenuEntry::separator(cx.entity().downgrade()),
            self.menu_item(
                if hidden { l.unhide_game } else { l.hide_game },
                Command::Hide(index),
                cx,
            ),
            self.menu_item(l.game_details, Command::Details(index), cx),
        ];
        if let Some(studio) = game.and_then(|game| game.studio_name()) {
            items.push(self.menu_item(
                crate::i18n::s().library.filter_same_studio(&studio),
                Command::FilterStudio(index),
                cx,
            ));
        }
        items.extend([
            self.menu_item(l.game_info_cover_ellipsis, Command::GameInfo(index), cx),
            self.menu_item(
                l.run_settings_ellipsis,
                Command::Sheet(PreviewSheet::RunSettings(index)),
                cx,
            ),
        ]);
        items.push(self.menu_item(l.game_translation_ellipsis, Command::Translation(index), cx));
        if has_path {
            items.push(self.menu_item(crate::platform::reveal_label(), Command::Reveal(index), cx));
        }
        if game.is_some_and(|game| game.game_directory().is_some()) {
            items.push(self.menu_item(l.open_game_dir, Command::OpenGameDirectory(index), cx));
        }
        items.extend([
            MenuEntry::separator(cx.entity().downgrade()),
            self.menu_item(l.configure_bottle, Command::Configure(index), cx),
        ]);
        if has_path && self.service_available("entry.move")
            && !game.is_some_and(|game| game.id.starts_with("steam:")) {
            items.push(self.menu_item(l.move_to_bottle_ellipsis, Command::Move(index), cx));
        }
        // Only a folder entry or a multi-exe work picks launch executables from
        // its directory; a single pinned or Steam entry runs its fixed target.
        if is_folder || multi_exe {
            items.push(MenuEntry::separator(cx.entity().downgrade()));
            items.push(self.menu_item(
                if multi_exe {
                    l.manage_launch_entries_ellipsis
                } else {
                    l.select_launch_exe_ellipsis
                },
                Command::Executable(index),
                cx,
            ));
        }
        let tags_label = l.tab_tags;
        let edit_tags_label = l.edit_new_tag_ellipsis;
        move |mut menu, window, cx| {
            let mut inserted_tags = false;
            for (idx, item) in items.iter().enumerate() {
                menu = menu.item(item.build());
                if idx == 3 && !inserted_tags {
                    inserted_tags = true;
                    let library_tags = library_tags.clone();
                    let entry_tags = entry_tags.clone();
                    let view = view.clone();
                    menu = menu.submenu(tags_label, window, cx, move |mut sub, _, _| {
                        for tag in &library_tags {
                            let tag_key = tag.to_lowercase();
                            let active = entry_tags.iter().any(|t| t.to_lowercase() == tag_key);
                            let tag_click = tag.clone();
                            let view = view.clone();
                            sub = sub.item(
                                PopupMenuItem::new(tag.clone())
                                    .checked(active)
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.toggle_game_tag(index, &tag_click, cx);
                                        });
                                    }),
                            );
                        }
                        if !library_tags.is_empty() {
                            sub = sub.separator();
                        }
                        let view = view.clone();
                        sub.item(PopupMenuItem::new(edit_tags_label).on_click(
                            move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.command(Command::EditTags(index), cx);
                                });
                            },
                        ))
                    });
                }
            }
            menu
        }
    }
    /// One bottle's context-menu entries, mirroring the SwiftUI bottle list.
    fn bottle_menu_entries(&self, index: usize, cx: &Context<Self>) -> Vec<MenuEntry> {
        let strings = crate::i18n::s();
        let available = self.bottles.get(index).is_some_and(|bottle| bottle.available);
        let mut items = vec![];
        if available {
            items.push(self.menu_item(strings.library.rename_ellipsis, Command::BottleRename(index), cx));
            items.push(self.menu_item(strings.library.bottle_delete_ellipsis, Command::BottleRemove(index), cx));
            items.push(MenuEntry::separator(cx.entity().downgrade()));
            items.push(self.menu_item(strings.library.bottle_move_ellipsis, Command::BottleMove(index), cx));
            items.push(self.menu_item(strings.library.bottle_duplicate_ellipsis, Command::BottleDuplicate(index), cx));
            items.push(self.menu_item(strings.library.bottle_export_ellipsis, Command::BottleExport(index), cx));
            items.push(MenuEntry::separator(cx.entity().downgrade()));
        }
        items.push(self.menu_item(crate::platform::reveal_label(), Command::BottleReveal(index), cx));
        items
    }
    /// Sidebar context menu, mirroring the SwiftUI bottle list entry.
    pub(crate) fn bottle_menu(
        &self,
        index: usize,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let items = self.bottle_menu_entries(index, cx);
        move |mut menu, _, _| {
            for item in &items {
                menu = menu.item(item.build());
            }
            menu
        }
    }
    pub(crate) fn game_options(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        Button::new(("game-options", index))
            .label("•••")
            .ghost()
            .tooltip(crate::i18n::s().common.more)
            .dropdown_menu(self.game_menu(index, cx))
            .into_any_element()
    }
    pub(crate) fn action(
        &self,
        id: &'static str,
        label: &'static str,
        command: Command,
        cx: &Context<Self>,
    ) -> AnyElement {
        Button::new(id)
            .label(label)
            .disabled(!self.command_available(command))
            .on_click(cx.listener(move |this, _, _, cx| this.command(command, cx)))
            .into_any_element()
    }
    pub(crate) fn toggle(
        &self,
        key: &'static str,
        label: &'static str,
        description: &'static str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let checked = match key {
            "mixed" => self.mixed,
            "motion" => self.motion,
            "group-by-studio" => self.group_by_studio,
            _ => self.flag(key),
        };
        row()
            .gap(px(20.))
            .py(px(10.))
            .items_start()
            .child(
                col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(label)
                    .when(!description.is_empty(), |d| {
                        d.child(muted(description).line_height(relative(1.4)))
                    }),
            )
            .child(
                Switch::new(key)
                    .accessibility_label(label)
                    .checked(checked)
                    .disabled(!self.toggle_available(key))
                    .on_change(cx.listener(move |this, _, _, cx| {
                        this.command(Command::Toggle(key), cx)
                    })),
            )
            .into_any_element()
    }
    pub(crate) fn choice(
        &self,
        key: &'static str,
        label: &'static str,
        options: &[&'static str],
        cx: &Context<Self>,
    ) -> AnyElement {
        let selected = self
            .preview
            .choices
            .get(key)
            .cloned()
            .unwrap_or_else(|| options[0].into());
        let view = cx.entity().downgrade();
        let current = selected.clone();
        let options: Vec<&'static str> = options.to_vec();
        row()
            .py(px(10.))
            .gap(px(16.))
            .child(div().flex_1().min_w_0().child(label))
            .child(
                Button::new(key)
                    .label(format!("{}  ⌄", selected))
                    .dropdown_menu(move |mut menu, _, _| {
                        for &option in &options {
                            let view = view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(option)
                                    .checked(current.as_ref() == option)
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.preview.choices.insert(key, option.into());
                                            cx.notify();
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .into_any_element()
    }
    /// A dropdown backed by raw values, showing the matching label. Used by
    /// settings controls whose stored value is a Swift enum raw value.
    pub(crate) fn choice_pairs(
        &self,
        key: &'static str,
        label: &'static str,
        options: &[(&'static str, &'static str)],
        cx: &Context<Self>,
    ) -> AnyElement {
        let selected = self
            .preview
            .choices
            .get(key)
            .map(AsRef::as_ref)
            .unwrap_or(options[0].1);
        let selected_label = options
            .iter()
            .find(|(_, value)| *value == selected)
            .map_or(options[0].0, |(label, _)| label);
        let view = cx.entity().downgrade();
        let current = SharedString::from(selected);
        let options: Vec<(&'static str, &'static str)> = options.to_vec();
        row()
            .py(px(10.))
            .gap(px(16.))
            .child(div().flex_1().min_w_0().child(label))
            .child(
                Button::new(key)
                    .label(format!("{selected_label}  ⌄"))
                    .dropdown_menu(move |mut menu, _, _| {
                        for &(option_label, value) in &options {
                            let view = view.clone();
                            let checked = current.as_ref() == value;
                            menu = menu.item(
                                PopupMenuItem::new(option_label).checked(checked).on_click(
                                    move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            this.command(Command::Choose(key, value), cx);
                                        });
                                    },
                                ),
                            );
                        }
                        menu
                    }),
            )
            .into_any_element()
    }
    pub(crate) fn field(&self, key: &'static str, label: &'static str) -> AnyElement {
        col()
            .gap(px(7.))
            .py(px(8.))
            .child(label)
            .child(Input::new(&self.preview.inputs[key]))
            .into_any_element()
    }
    pub(crate) fn info(&self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> AnyElement {
        row()
            .gap(px(18.))
            .py(px(10.))
            .items_start()
            .child(div().flex_1().min_w_0().child(label.into()))
            .child(muted(value).max_w(relative(0.65)))
            .into_any_element()
    }
    pub(crate) fn section(
        &self,
        key: &'static str,
        title: &'static str,
        children: Vec<AnyElement>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let collapsed = self.preview.collapsed.contains(key);
        col()
            .flex_shrink_0()
            .gap(px(8.))
            .child(
                row()
                    .id(SharedString::from(format!("section-{key}")))
                    .gap(px(7.))
                    .py(px(5.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.preview.collapsed.remove(key) {
                            this.preview.collapsed.insert(key);
                        }
                        cx.notify();
                    }))
                    .child(muted(if collapsed { "›" } else { "⌄" }))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title)),
            )
            .when(!collapsed, |d| {
                d.child(
                    col()
                        .px(px(17.))
                        .py(px(5.))
                        .rounded(px(10.))
                        .bg(rgba(crate::theme::section_bg()))
                        .border_1()
                        .border_color(rgba(crate::theme::section_border()))
                        .children(children),
                )
            })
            .into_any_element()
    }
    pub(crate) fn form(&self, id: &'static str, sections: Vec<AnyElement>) -> AnyElement {
        self.preview_scroll(
            id,
            col().w_full().p(px(26.)).gap(px(22.)).children(sections),
        )
    }
    pub(crate) fn preview_scroll(&self, id: &'static str, content: impl IntoElement) -> AnyElement {
        self.bouncing_scroll(id, &self.preview.form_scroll_handle(id), content)
    }
    pub(crate) fn bouncing_scroll(
        &self,
        id: impl Into<ElementId>,
        handle: &ScrollHandle,
        content: impl IntoElement,
    ) -> AnyElement {
        let id = id.into();
        // One follow-up frame resolves the document's initial scroll metrics.
        if handle.bounds().size.height == px(0.) {
            self.scroll_needs_measurement.set(true);
        }
        let viewport = div()
            .id((id.clone(), "viewport"))
            .size_full()
            .track_scroll(handle)
            .overflow_y_scroll()
            .child(content);
        ScrollBounce::new((id, "bounce"), handle, viewport)
            .enabled(self.motion)
            // Base's direct edge path also stretches documents that fit the viewport.
            .scroll_wheel(handle.max_offset().y > px(0.))
            .motion(ScrollBounceMotion::default())
            .into_any_element()
    }
    pub(crate) fn library_viewport(&self, mode: usize, content: impl IntoElement) -> AnyElement {
        self.bouncing_scroll(
            ("library-scroll", mode),
            &self.library_scroll[mode],
            content,
        )
    }
}
