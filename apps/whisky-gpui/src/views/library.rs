//! Library surface: sidebar, toolbar, three presentations, and the detail panel.

use std::time::Duration;

use gpui_kit::base::{FocusTrapElement as _, Spring, spring, transition};
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
    spinner::Spinner,
};
use gpui_kit::{IntoElement, prelude::*, *};

use crate::bridge::BridgeStatus;
use crate::controls::*;
use crate::library_layout::{CardRect, LibraryLayout};
use crate::model::{ambient, art};
use crate::runtime::{RuntimeAction, RuntimeState};
use crate::state::{PreviewPage, SidebarMode, WhiskyApp};
use crate::theme::*;
use crate::views::{
    ConnectorPaint, connector_curve, fade_rgb, mix_rgb, mix_rgba, paint_connectors,
};

const CHARACTER_CARD_WIDTH: f32 = 164.;
const CHARACTER_CARD_PADDING: f32 = 10.;
const CHARACTER_PORTRAIT_HEIGHT: f32 = 144.;

/// Cover size track and thumb, in points.
const COVER_SLIDER_WIDTH: f32 = 118.;
const COVER_SLIDER_THUMB: f32 = 12.;

/// Pointer x on the cover size track, snapped to the nearest level the shelf
/// can draw flush. `bounds` is `[origin x, width]`; `levels` is narrowest
/// first, matching the track's left-to-right growth.
fn cover_slider_value(x: f32, bounds: [f32; 2], levels: &[f32]) -> f32 {
    let fraction = if bounds[1] > 0. {
        ((x - bounds[0]) / bounds[1]).clamp(0., 1.)
    } else {
        0.
    };
    match levels.len() {
        0 => 0.,
        1 => levels[0],
        count => levels[(fraction * (count - 1) as f32).round() as usize],
    }
}

/// Index of the level nearest `current` — where the control's thumb sits.
fn cover_level_index(levels: &[f32], current: f32) -> usize {
    levels
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - current).abs().total_cmp(&(*b - current).abs()))
        .map(|(index, _)| index)
        .unwrap_or(0)
}

/// Cover only when at least 80% of the artwork remains visible. Both the card
/// and portrait have a one-pixel border, which is excluded from the viewport.
fn character_portrait_fit(width: i32, height: i32) -> ObjectFit {
    if width <= 0 || height <= 0 {
        return ObjectFit::Contain;
    }
    let viewport_width = CHARACTER_CARD_WIDTH - 2. * (CHARACTER_CARD_PADDING + 2.);
    let viewport_height = CHARACTER_PORTRAIT_HEIGHT - 2.;
    let relative_width = f64::from(width) * f64::from(viewport_height);
    let relative_height = f64::from(height) * f64::from(viewport_width);
    // Cross-multiply the 4/5 threshold so exactly 20% cropping is stable.
    if 5. * relative_width.min(relative_height) >= 4. * relative_width.max(relative_height) {
        ObjectFit::Cover
    } else {
        ObjectFit::Contain
    }
}

fn character_portrait_placeholder(label: &'static str) -> AnyElement {
    col()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .child(icon("user", 22.).text_color(rgb(muted_color())))
        .child(muted(label).text_size(px(11.)))
        .into_any_element()
}

#[derive(IntoElement)]
struct CharacterPortrait {
    path: std::path::PathBuf,
}

impl RenderOnce for CharacterPortrait {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Reuse GPUI's asynchronous image cache: no file reads or second image
        // decode on the render thread just to discover the artwork's dimensions.
        match window.use_asset::<ImgResourceLoader>(&self.path.into(), cx) {
            Some(Ok(image)) => {
                let dimensions = image.size(0);
                let fit = character_portrait_fit(dimensions.width.0, dimensions.height.0);
                div()
                    .size_full()
                    .when(matches!(fit, ObjectFit::Contain), |d| d.p(px(4.)))
                    .child(img(image).size_full().object_fit(fit))
                    .into_any_element()
            }
            Some(Err(_)) => character_portrait_placeholder(crate::i18n::s().library.avatar_failed),
            None => character_portrait_placeholder(crate::i18n::s().common.loading),
        }
    }
}

fn characters_in_display_order<'a>(
    characters: &'a [crate::bridge::dto::CharacterDto],
    images: &std::collections::BTreeMap<String, String>,
) -> Vec<&'a crate::bridge::dto::CharacterDto> {
    let mut ordered: Vec<_> = characters.iter().collect();
    ordered.sort_by_key(|character| !images.contains_key(&character.id));
    ordered
}

#[cfg(test)]
mod character_presentation_tests {
    use super::{character_portrait_fit, characters_in_display_order};
    use crate::bridge::dto::CharacterDto;
    use gpui_kit::ObjectFit;
    use std::collections::BTreeMap;

    #[test]
    fn ordinary_portraits_fill_the_fixed_viewport() {
        for (width, height) in [(400, 400), (450, 500), (500, 450)] {
            assert!(matches!(character_portrait_fit(width, height), ObjectFit::Cover));
        }
    }

    #[test]
    fn full_body_and_extremely_tall_artwork_remains_complete() {
        for (width, height) in [(400, 650), (300, 1200), (1, 10000)] {
            assert!(matches!(character_portrait_fit(width, height), ObjectFit::Contain));
        }
    }

    #[test]
    fn wide_artwork_remains_complete() {
        for (width, height) in [(800, 400), (10000, 1)] {
            assert!(matches!(character_portrait_fit(width, height), ObjectFit::Contain));
        }
    }

    #[test]
    fn crop_limit_applies_in_both_directions() {
        // The drawable viewport is 140 × 142. These retain exactly 80%.
        for (width, height) in [(112, 142), (175, 142)] {
            assert!(matches!(character_portrait_fit(width, height), ObjectFit::Cover));
        }
        for (width, height) in [(111, 142), (176, 142)] {
            assert!(matches!(character_portrait_fit(width, height), ObjectFit::Contain));
        }
    }

    #[test]
    fn missing_dimensions_never_trigger_cropping() {
        for (width, height) in [(0, 0), (0, 400), (400, 0), (-1, 400), (400, -1)] {
            assert!(matches!(character_portrait_fit(width, height), ObjectFit::Contain));
        }
    }

    #[test]
    fn cached_portraits_come_first_and_each_group_keeps_its_order() {
        let characters: Vec<CharacterDto> = ["missing", "first", "failed", "second"]
            .into_iter()
            .map(|id| CharacterDto {
                id: id.into(),
                image_url: Some("https://example.com/portrait.png".into()),
                ..Default::default()
            })
            .collect();
        let images = BTreeMap::from([
            ("first".into(), "/cache/first.png".into()),
            ("second".into(), "/cache/second.png".into()),
        ]);
        let ordered = characters_in_display_order(&characters, &images);
        assert_eq!(ordered.iter().map(|character| character.id.as_str()).collect::<Vec<_>>(),
            ["first", "second", "missing", "failed"]);
    }
}

/// One sidebar row. Kept next to the sidebar so the Library and bottle entries
/// cannot drift apart visually.
fn sidebar_row(
    id: impl Into<ElementId>,
    label: &str,
    symbol: &str,
    count: usize,
    selected: bool,
) -> Stateful<Div> {
    row()
        .id(id)
        .h(px(34.))
        .px(px(13.))
        .gap(px(10.))
        .rounded(px(6.))
        .bg(if selected {
            rgba(selected_bg())
        } else {
            rgba(0x00000000)
        })
        .cursor_pointer()
        .hover(|s| s.bg(rgba(hover())))
        .child(icon(symbol, 18.).text_color(rgb(ACCENT)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label.to_owned()),
        )
        .child(muted(count.to_string()).text_size(px(11.)))
}

/// One rail button, mirroring a `sidebar_row` icon. The label and count move
/// into the tooltip so the strip can stay at icon width.
fn sidebar_icon(id: impl Into<ElementId>, symbol: &str, selected: bool) -> Stateful<Div> {
    row()
        .id(id)
        .size(px(36.))
        .justify_center()
        .rounded(px(6.))
        .bg(if selected {
            rgba(selected_bg())
        } else {
            rgba(0x00000000)
        })
        .cursor_pointer()
        .hover(|s| s.bg(rgba(hover())))
        .child(icon(symbol, 18.).text_color(rgb(ACCENT)))
}

impl WhiskyApp {
    /// Existing artwork only; the demo keeps its bundled sample covers.
    pub(crate) fn artwork(&self, index: usize) -> Option<std::path::PathBuf> {
        if self.demo {
            Some(art(index))
        } else {
            self.game(index).and_then(|game| game.art.clone())
        }
    }

    /// The blurred backdrop is derived from the same artwork.
    pub(crate) fn ambient_art(&self, index: usize) -> Option<std::path::PathBuf> {
        if self.demo {
            Some(ambient(index))
        } else {
            self.artwork(index)
        }
    }

    /// The library body plus the shared loading/error surfaces.
    pub(crate) fn library_page(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(error) = self.visible_load_error() {
            return self.library_error_surface(error, cx);
        }
        if self.games().is_empty() && self.loading {
            return col()
                .size_full()
                .items_center()
                .justify_center()
                .p(px(32.))
                .gap(px(12.))
                .child(Spinner::new().with_size(px(20.)))
                .child(muted(crate::i18n::s().library.loading_library))
                .into_any_element();
        }
        let content = match self.mode {
            1 => self.timeline(width, window, cx),
            2 => self.classic(width, window, cx),
            3 => self.immersive(width, window, cx),
            4 => self.gallery(width, window, cx),
            5 => self.staggered(width, window, cx),
            _ => self.shelf(width, window, cx),
        };
        const FILTER_BAR_HEIGHT: f32 = 44.;
        let target_frac = self.filter_bar_target_fraction();
        let dragging_pull = self.tag_bar_mode == crate::ui_state::TagBarMode::PullDown
            && !self.pull_reveal.is_open()
            && self.pull_reveal.pull_fraction() > 0.0;
        let drawer_spring = Spring::new(Duration::from_millis(if dragging_pull { 60 } else { 260 }))
            .with_damping(0.88)
            .with_epsilon(0.002)
            .with_travel(self.motion);
        let reveal_frac = spring(
            ElementId::from("library-filter-drawer-reveal"),
            target_frac,
            drawer_spring,
            window,
            cx,
        )
        .clamp(0.0, 1.0);
        let hint_opacity = transition(
            ElementId::from("library-pull-hint-opacity"),
            if self.tag_bar_hint_peeking { 1_f32 } else { 0.0 },
            self.timing(NOTICE_MS),
            window,
            cx,
        );

        let view = cx.entity().downgrade();
        let pull_observer = canvas(
            move |bounds, window, _| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                (bounds, hitbox.id)
            },
            move |_, (bounds, hitbox): (Bounds<Pixels>, HitboxId), window, _| {
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                    if phase != DispatchPhase::Capture
                        || !bounds.contains(&event.position)
                        || !hitbox.should_handle_scroll(window)
                    {
                        return;
                    }
                    let delta = event.delta.pixel_delta(window.line_height());
                    if delta.x.abs() > delta.y.abs() {
                        return;
                    }
                    let pull_phase = match event.touch_phase {
                        TouchPhase::Started => crate::library_view::PullWheelPhase::Started,
                        TouchPhase::Ended | TouchPhase::Cancelled => {
                            crate::library_view::PullWheelPhase::Ended
                        }
                        TouchPhase::Moved => crate::library_view::PullWheelPhase::Moved,
                    };
                    let consumed = view
                        .update(cx, |this, cx| {
                            if this.mode == 4 {
                                let detail_bounds = this.gallery_detail_scroll.bounds();
                                if detail_bounds.size.height > px(0.)
                                    && detail_bounds.contains(&event.position)
                                {
                                    return false;
                                }
                            }
                            this.handle_library_pull_wheel(f32::from(delta.y), pull_phase, cx)
                        })
                        .unwrap_or(false);
                    if consumed {
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .inset_0();

        col()
            .relative()
            .size_full()
            .child(pull_observer)
            .when(reveal_frac > 0.004, |d| {
                d.child(
                    div()
                        .id("library-filter-drawer-clip")
                        .w_full()
                        .h(px(FILTER_BAR_HEIGHT * reveal_frac))
                        .flex_shrink_0()
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .justify_end()
                        .when(self.tag_bar_hint_peeking, |clip| {
                            clip.cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                                this.dismiss_tag_bar_hint(true, cx);
                            }))
                        })
                        .child(self.library_filter_drawer(cx)),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(content)
                    .when(hint_opacity > 0.01, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(8.))
                                .left_0()
                                .right_0()
                                .flex()
                                .justify_center()
                                .opacity(hint_opacity)
                                .child(
                                    row()
                                        .id("library-pull-hint-pill")
                                        .occlude()
                                        .items_center()
                                        .gap(px(8.))
                                        .px(px(12.))
                                        .py(px(6.))
                                        .rounded_full()
                                        .bg(rgb(crate::theme::surface()))
                                        .border_1()
                                        .border_color(rgba(accent_subtle_border()))
                                        .shadow_md()
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dismiss_tag_bar_hint(true, cx);
                                        }))
                                        .child(
                                            icon("chevrons-down", 13.)
                                                .text_color(rgb(accent_fg())),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgb(text()))
                                                .child(crate::i18n::s().library.pull_down_hint),
                                        )
                                        .child(
                                            div()
                                                .px(px(7.))
                                                .py(px(2.))
                                                .rounded_full()
                                                .bg(rgba(accent_subtle_bg()))
                                                .text_size(px(11.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(accent_fg()))
                                                .child(crate::i18n::s().library.expand_now),
                                        )
                                        .child(
                                            div()
                                                .id("library-pull-hint-close")
                                                .size(px(18.))
                                                .rounded_full()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .hover(|s| s.bg(rgba(hover())))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.dismiss_tag_bar_hint(false, cx);
                                                    cx.stop_propagation();
                                                }))
                                                .child(
                                                    icon("x", 11.)
                                                        .text_color(rgb(muted_color())),
                                                ),
                                        ),
                                ),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(crate) fn library_empty_surface(&self, cx: &mut Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let query = self.search.read(cx).value().trim().to_owned();
        let hidden = self.hidden_count();
        let no_bottles = !self.demo && self.bottles.is_empty();
        let empty_scope = !self.demo && !no_bottles && self.scope_entry_count() == 0;

        let (symbol, title, detail) = if no_bottles {
            (
                "package",
                strings.library.no_bottles_title,
                strings.library.no_bottles_desc.to_owned(),
            )
        } else if empty_scope {
            (
                "folder-search",
                if self.selected_bottle.is_some() {
                    strings.library.bottle_empty_title
                } else {
                    strings.library.library_empty_title
                },
                strings.library.library_empty_desc.to_owned(),
            )
        } else if !query.is_empty() {
            (
                "search-x",
                strings.library.search_empty_title,
                crate::i18n::s().library.empty_search_detail(&query),
            )
        } else if self.tag_filter.is_active() || self.studio_filter.is_active() {
            if self.studio_filter.is_active() && self.tag_filter.is_active() {
                (
                    "building-2",
                    strings.library.filter_combo_empty_title,
                    crate::i18n::s().library.empty_studio_and_tag_detail(&self.studio_filter.display_label(), &self.tag_filter.display_label()),
                )
            } else if self.studio_filter.is_active() {
                (
                    "building-2",
                    strings.library.filter_studio_empty_title,
                    crate::i18n::s().library.empty_studio_detail(&self.studio_filter.display_label()),
                )
            } else {
                (
                    "tag",
                    strings.library.filter_tag_empty_title,
                    crate::i18n::s().library.empty_tag_detail(&self.tag_filter.display_label()),
                )
            }
        } else if hidden > 0 && !self.show_hidden() {
            (
                "search-x",
                strings.library.all_hidden_title,
                crate::i18n::s().library.empty_hidden_detail(hidden),
            )
        } else {
            (
                "search-x",
                self.empty_library_message(),
                strings.library.filter_empty_desc.to_owned(),
            )
        };

        let mut actions = row().flex_wrap().justify_center().gap(px(10.)).mt(px(4.));
        if no_bottles {
            actions = actions
                .child(
                    Button::new("empty-create-bottle")
                        .label(strings.library.create_bottle_btn)
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_preview_sheet(crate::state::PreviewSheet::CreateBottle, cx);
                        })),
                )
                .when(!self.demo, |d| {
                    d.child(
                        Button::new("empty-import-bottle")
                            .label(strings.library.import_bottles_btn)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.begin_import_scan(None, cx);
                            })),
                    )
                });
        } else if empty_scope {
            actions = actions
                .child(
                    Button::new("empty-scan-folders")
                        .label(strings.library.manage_scan_folders_btn)
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_preview_sheet(crate::state::PreviewSheet::Scan, cx);
                        })),
                )
                .when(self.selected_bottle.is_some(), |d| {
                    d.child(
                        Button::new("empty-add-program")
                            .label(strings.library.add_program_btn)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.command(crate::views::Command::AddProgram, cx);
                            })),
                    )
                });
        } else {
            if !query.is_empty() {
                actions = actions.child(
                    Button::new("empty-clear-search")
                        .label(strings.library.clear_search)
                        .primary()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.search.update(cx, |state, cx| state.clean(window, cx));
                            cx.notify();
                        })),
                );
            }
            if self.studio_filter.is_active() {
                actions = actions.child(
                    Button::new("empty-clear-studio-filter")
                        .label(strings.library.clear_studio_filter)
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_studio_filter(cx);
                        })),
                );
            }
            if self.tag_filter.is_active() {
                actions = actions.child(
                    Button::new("empty-clear-tag-filter")
                        .label(strings.library.clear_tag_filter)
                        .primary()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_tag_filter(cx);
                        })),
                );
            }
            if hidden > 0 && !self.show_hidden() {
                actions = actions.child(
                    Button::new("empty-show-hidden")
                        .label(crate::i18n::s().library.show_hidden_short(hidden))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.command(crate::views::Command::Toggle("show-hidden"), cx);
                        })),
                );
            }
        }

        col()
            .w_full()
            .min_h(px(360.))
            .items_center()
            .justify_center()
            .p(px(32.))
            .child(
                col()
                    .max_w(px(440.))
                    .items_center()
                    .gap(px(12.))
                    .p(px(28.))
                    .rounded(px(16.))
                    .bg(rgba(section_bg()))
                    .border_1()
                    .border_color(rgba(section_border()))
                    .child(
                        div()
                            .size(px(52.))
                            .rounded(px(14.))
                            .bg(rgba(accent_subtle_bg()))
                            .border_1()
                            .border_color(rgba(accent_subtle_border()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(symbol, 24.).text_color(rgb(accent_fg()))),
                    )
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::BOLD)
                            .text_center()
                            .child(title),
                    )
                    .child(
                        muted(detail)
                            .text_center()
                            .line_height(relative(1.55)),
                    )
                    .child(actions),
            )
            .into_any_element()
    }

    pub(crate) fn library_error_surface(&self, reason: &str, cx: &mut Context<Self>) -> AnyElement {
        col()
            .size_full()
            .items_center()
            .justify_center()
            .p(px(30.))
            .child(
                col()
                    .w(px(420.))
                    .gap(px(14.))
                    .p(px(22.))
                    .rounded(px(RADIUS_LG))
                    .bg(rgb(crate::theme::surface()))
                    .border_1()
                    .border_color(rgba(surface_border()))
                    .child(
                        row()
                            .gap(px(8.))
                            .child(icon("triangle-alert", 18.).text_color(rgb(danger_fg())))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::i18n::s().library.load_error_title)),
                    )
                    .child(
                        muted(reason)
                            .line_height(relative(1.5))
                            .line_clamp(4),
                    )
                    .child(
                        row().justify_end().child(
                            Button::new("library-retry")
                                .label(crate::i18n::s().common.retry)
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| this.retry_library(cx))),
                        ),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.sidebar == SidebarMode::Icons {
            self.sidebar_rail(window, cx)
        } else {
            // `Hidden` keeps the expanded rows: they slide out under the clip.
            self.sidebar_expanded(window, cx)
        }
    }

    /// Right-click on a sidebar affordance picks the presentation directly;
    /// the buttons keep their one-click behaviour.
    fn sidebar_mode_menu(
        &self,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let view = cx.entity().downgrade();
        let current = self.sidebar;
        move |mut menu, _, _| {
            let lib = &crate::i18n::s().library;
            for (label, mode) in [
                (lib.sidebar_expand, SidebarMode::Expanded),
                (lib.sidebar_icons_only, SidebarMode::Icons),
                (lib.sidebar_hide, SidebarMode::Hidden),
            ] {
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(label)
                        .checked(current == mode)
                        .on_click(move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.set_sidebar(mode, cx);
                            });
                        }),
                );
            }
            menu
        }
    }

    /// The rail's container picker: one submenu per bottle — its first item
    /// selects the container, the rest are the expanded row's bottle actions.
    fn container_menu(
        &self,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let view = cx.entity().downgrade();
        let selected = self.selected_bottle;
        let bottles: Vec<_> = self
            .bottles
            .iter()
            .enumerate()
            .map(|(index, bottle)| {
                (index, bottle.name.clone(), self.bottle_menu_entries(index, cx))
            })
            .collect();
        move |mut menu, window, cx| {
            menu = menu.scrollable(true);
            let lib = &crate::i18n::s().library;
            if bottles.is_empty() {
                menu = menu.item(PopupMenuItem::new(lib.no_bottles_menu).disabled(true));
            }
            for (index, name, items) in &bottles {
                let (index, name, items) = (*index, name.clone(), items.clone());
                let view = view.clone();
                let show_label = lib.sidebar_show_sub;
                menu = menu.submenu(name, window, cx, move |mut sub, _, _| {
                    let view = view.clone();
                    sub = sub
                        .item(
                            PopupMenuItem::new(show_label)
                                .checked(selected == Some(index))
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.select_bottle(Some(index), cx);
                                    });
                                }),
                        )
                        .separator();
                    for item in &items {
                        sub = sub.item(item.build());
                    }
                    sub
                });
            }
            menu
        }
    }

    /// The collapsed rail: every expanded entry mapped to its icon, with the
    /// name and count carried by the row's tooltip.
    fn sidebar_rail(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lib = &crate::i18n::s().library;
        let games = self.games();
        let mut entries = col().items_center().gap(px(1.)).px(px(10.));
        entries = entries.child(
            track_key_focus(
                sidebar_icon("sidebar-expand", "panel-left-open", false)
                    .tooltip(text_tip(lib.rail_expand_tip))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_sidebar(SidebarMode::Expanded, cx);
                    })),
                "sidebar-rail-expand",
                1,
                ACCENT,
                window,
                cx,
            )
            .context_menu(self.sidebar_mode_menu(cx)),
        );
        entries = entries.child(
            sidebar_icon(
                "library",
                "layout-grid",
                !self.inline_settings_active()
                    && self.preview.page == PreviewPage::Library
                    && self.selected_bottle.is_none(),
            )
            .tooltip(text_tip(format!(
                "{} · {}",
                lib.title,
                games.iter().filter(|game| !game.is_launcher).count()
            )))
            .on_click(cx.listener(|this, _, _, cx| this.select_bottle(None, cx)))
            .key_focus("sidebar-rail-library")
            .tab_index(1),
        );
        entries = entries.child(
            sidebar_icon(
                "statistics",
                "rows-3",
                !self.inline_settings_active() && self.preview.page == PreviewPage::Statistics,
            )
            .tooltip(text_tip(format!(
                "{} · {}",
                lib.statistics,
                games
                    .iter()
                    .filter(|game| !game.is_launcher && game.play_time.unwrap_or(0) > 0)
                    .count()
            )))
            .on_click(cx.listener(|this, _, _, cx| {
                this.navigate(PreviewPage::Statistics, cx);
            }))
            .key_focus("sidebar-rail-statistics")
            .tab_index(1),
        );
        entries = entries.child(
            sidebar_icon(
                "launchers",
                "store",
                !self.inline_settings_active() && self.preview.page == PreviewPage::Launchers,
            )
            .tooltip(text_tip(format!(
                "{} · {}",
                lib.launchers,
                games.iter().filter(|game| game.is_launcher).count()
            )))
            .on_click(cx.listener(|this, _, _, cx| {
                this.navigate(PreviewPage::Launchers, cx);
            }))
            .key_focus("sidebar-rail-launchers")
            .tab_index(1),
        );
        // The expanded "Bottles" caption becomes a hairline at rail width.
        entries = entries.child(div().h(px(1.)).w(px(24.)).my(px(5.)).bg(rgba(divider())));
        // One container affordance: left-click reopens the active bottle,
        // right-click lists every container so one pick selects it.
        let container_tip = match self.selected_bottle.and_then(|index| self.bottles.get(index)) {
            Some(bottle) => crate::i18n::s().library.rail_bottle_tooltip(&bottle.name),
            None => lib.rail_bottles_tip.to_owned(),
        };
        entries = entries.child(
            track_key_focus(
                sidebar_icon(
                    "containers",
                    "package",
                    !self.inline_settings_active() && self.selected_bottle.is_some(),
                )
                .tooltip(text_tip(container_tip))
                .on_click(cx.listener(|this, _, _, cx| {
                    let target = this
                        .selected_bottle
                        .or_else(|| (!this.bottles.is_empty()).then_some(0));
                    if target.is_some() {
                        this.select_bottle(target, cx);
                    }
                })),
                "sidebar-rail-containers",
                1,
                ACCENT,
                window,
                cx,
            )
            .context_menu(self.container_menu(cx)),
        );
        let rail_tags = crate::library_view::collect_tag_counts(games, self.show_hidden());
        if !rail_tags.is_empty() || self.tag_filter.is_active() {
            let tag_tip = if self.tag_filter.is_active() {
                crate::i18n::s().library.rail_tag_active_tooltip(&self.tag_filter.display_label())
            } else {
                crate::i18n::s().library.rail_tag_count_tooltip(rail_tags.len())
            };
            entries = entries.child(
                sidebar_icon("tags", "tag", self.tag_filter.is_active())
                    .tooltip(text_tip(tag_tip))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.tag_filter.is_active() {
                            this.clear_tag_filter(cx);
                        } else {
                            this.open_manage_tags_sheet(cx);
                        }
                    }))
                    .context_menu(self.tag_filter_menu(cx)),
            );
        }
        let (grad_start, grad_end) = sidebar_gradient();
        col()
            .id("sidebar-rail")
            .size_full()
            .items_center()
            .bg(gradient(grad_start, grad_end))
            .border_r_1()
            .border_color(rgba(divider()))
            // The full-width toolbar covers the traffic lights, so the rail's
            // entries start directly under it instead of below an empty band.
            .child(entries)
            .child(div().flex_1())
            .child(
                col()
                    .items_center()
                    .gap(px(6.))
                    .pb(px(8.))
                    .child(self.bridge_dot(cx))
                    .child(
                        round_button("appearance-toggle", self.appearance.icon_name())
                            .size(px(28.))
                            .tooltip(text_tip(crate::i18n::s().theme.appearance_tooltip(self.appearance.label())))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cycle_appearance(cx);
                            }))
                            .key_focus("sidebar-rail-appearance")
                            .tab_index(1),
                    )
                    .child(
                        round_button("settings", "settings-2")
                            .size(px(28.))
                            .tooltip(text_tip(crate::platform::settings_tip()))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                #[cfg(not(target_os = "linux"))]
                                this.open_settings(cx);
                                #[cfg(target_os = "linux")]
                                if this.settings {
                                    this.close_settings(Some(_window), cx);
                                } else {
                                    this.open_settings(cx);
                                }
                            }))
                            .key_focus("sidebar-rail-settings")
                            .tab_index(1),
                    ),
            )
            .context_menu(self.sidebar_mode_menu(cx))
            .into_any_element()
    }

    fn sidebar_expanded(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let lib = &crate::i18n::s().library;
        let games = self.games();
        let mut entries = col().gap(px(1.));
        entries = entries.child(
            sidebar_row(
                "library",
                lib.title,
                "layout-grid",
                games.iter().filter(|game| !game.is_launcher).count(),
                !self.inline_settings_active()
                    && self.preview.page == PreviewPage::Library
                    && self.selected_bottle.is_none(),
            )
            .on_click(cx.listener(|this, _, _, cx| this.select_bottle(None, cx)))
            .key_focus("sidebar-expanded-library")
            .tab_index(1),
        );
        entries = entries.child(
            sidebar_row(
                "statistics", lib.statistics, "rows-3",
                games.iter().filter(|game| !game.is_launcher && game.play_time.unwrap_or(0) > 0).count(),
                !self.inline_settings_active() && self.preview.page == PreviewPage::Statistics,
            ).on_click(cx.listener(|this, _, _, cx| {
                this.navigate(PreviewPage::Statistics, cx);
            }))
            .key_focus("sidebar-expanded-statistics")
            .tab_index(1),
        );
        entries = entries.child(
            sidebar_row(
                "launchers", lib.launchers, "store",
                games.iter().filter(|game| game.is_launcher).count(),
                !self.inline_settings_active() && self.preview.page == PreviewPage::Launchers,
            ).on_click(cx.listener(|this, _, _, cx| {
                this.navigate(PreviewPage::Launchers, cx);
            }))
            .key_focus("sidebar-expanded-launchers")
            .tab_index(1),
        );
        entries = entries.child(
            muted(lib.bottles_section)
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .mt(px(17.))
                .mb(px(4.))
                .px(px(13.)),
        );
        for (index, bottle) in self.bottles.iter().enumerate() {
            let count = games
                .iter()
                .filter(|game| game.bottle == index + 1 && !game.is_launcher)
                .count();
            entries = entries.child(
                track_key_focus(
                    sidebar_row(
                        ("bottle", index),
                        &bottle.name,
                        "package",
                        count,
                        !self.inline_settings_active() && self.selected_bottle == Some(index),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_bottle(Some(index), cx);
                    })),
                    ("sidebar-expanded-bottle", index),
                    1,
                    ACCENT,
                    window,
                    cx,
                )
                .context_menu(self.bottle_menu(index, cx)),
            );
        }
        let sidebar_tags = crate::library_view::collect_tag_counts(games, self.show_hidden());
        if !sidebar_tags.is_empty() || self.tag_filter.is_active() {
            entries = entries.child(
                row()
                    .mt(px(17.))
                    .mb(px(4.))
                    .px(px(13.))
                    .items_center()
                    .justify_between()
                    .child(muted(lib.tags_section).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD))
                    .when(!self.demo, |d| {
                        d.child(
                            div()
                                .id("sidebar-manage-tags")
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(4.))
                                .text_size(px(10.5))
                                .text_color(rgb(muted_color()))
                                .cursor_pointer()
                                .hover(|s| s.bg(rgba(hover())).text_color(rgb(text())))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open_manage_tags_sheet(cx);
                                }))
                                .child(lib.manage),
                        )
                    }),
            );
            for (ti, (tag, count)) in sidebar_tags.into_iter().enumerate() {
                let active = self.tag_filter.contains(&tag);
                let tag_click = tag.clone();
                let tag_menu = tag.clone();
                entries = entries.child(
                    sidebar_row(("sidebar-tag", ti), &tag, "tag", count, active)
                        .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                            this.click_tag_filter(
                                &tag_click,
                                event.modifiers().platform || event.modifiers().shift,
                                cx,
                            );
                        }))
                        .context_menu(self.tag_context_menu(tag_menu, cx)),
                );
            }
            let untagged = crate::library_view::untagged_count(games, self.show_hidden());
            if untagged > 0 || self.tag_filter == crate::library_view::TagFilter::Untagged {
                let active = self.tag_filter == crate::library_view::TagFilter::Untagged;
                entries = entries.child(
                    sidebar_row("sidebar-tag-untagged", lib.untagged, "tag-x", untagged, active)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let next = if active {
                                crate::library_view::TagFilter::All
                            } else {
                                crate::library_view::TagFilter::Untagged
                            };
                            this.set_tag_filter(next, cx);
                        })),
                );
            }
        }
        let (grad_start, grad_end) = sidebar_gradient();
        col()
            .size_full()
            .bg(gradient(grad_start, grad_end))
            .border_r_1()
            .border_color(rgba(divider()))
            // The collapse button lives in the toolbar beside the traffic
            // lights, so the rows start directly under it like the rail's.
            .child(self.flex_scroll(
                "sidebar-entries-scroll",
                entries.px(px(5.)).pt(px(5.)).pb(px(10.)),
            ))
            .child(
                col()
                    .gap(px(2.))
                    .px(px(8.))
                    .pb(px(8.))
                    .child(self.bridge_pill(cx))
                    .child(
                        row()
                            .justify_between()
                            .px(px(4.))
                            .py(px(4.))
                            .child(muted("Maltoria").text_size(px(11.)))
                            .child(
                                row()
                                    .gap(px(6.))
                                    .child(
                                        round_button(
                                            "appearance-toggle",
                                            self.appearance.icon_name(),
                                        )
                                        .size(px(28.))
                                        .tooltip(text_tip(crate::i18n::s().theme.appearance_tooltip(self.appearance.label())))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.cycle_appearance(cx);
                                        }))
                                        .key_focus("sidebar-expanded-appearance")
                                        .tab_index(1),
                                    )
                                    .child(
                                        round_button("settings", "settings-2")
                                            .size(px(28.))
                                            .tooltip(text_tip(crate::platform::settings_tip()))
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                #[cfg(not(target_os = "linux"))]
                                                this.open_settings(cx);
                                                #[cfg(target_os = "linux")]
                                                if this.settings {
                                                    this.close_settings(Some(_window), cx);
                                                } else {
                                                    this.open_settings(cx);
                                                }
                                            }))
                                            .key_focus("sidebar-expanded-settings")
                                            .tab_index(1),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
    /// The bridge indicator's (dot colour, clickable-failure) pair, shared by
    /// the expanded pill and the rail's dot.
    fn bridge_status_style(&self) -> (u32, bool) {
        match &self.bridge_status {
            BridgeStatus::Connected { .. } => (SUCCESS, false),
            BridgeStatus::Failed { .. } => (DANGER, true),
            BridgeStatus::Stopped => (DANGER, false),
            BridgeStatus::Locating | BridgeStatus::Starting => (muted_color(), false),
        }
    }
    /// The rail's bridge indicator: the pill's dot alone, still opening the
    /// recovery dialog after a failure.
    fn bridge_dot(&self, cx: &mut Context<Self>) -> AnyElement {
        let (dot, failed) = self.bridge_status_style();
        div()
            .id("bridge-status")
            .size(px(28.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .when(failed, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(rgba(hover())))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.bridge_dialog = true;
                        cx.notify();
                    }))
            })
            .tooltip(text_tip(self.bridge_status.label()))
            .child(div().size(px(6.)).rounded_full().bg(rgb(dot)))
            .into_any_element()
    }
    fn bridge_pill(&self, cx: &mut Context<Self>) -> AnyElement {
        let (dot, failed) = self.bridge_status_style();
        row()
            .id("bridge-status")
            .px(px(10.))
            .py(px(6.))
            .gap(px(7.))
            .rounded(px(RADIUS_SM))
            .when(failed, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(rgba(hover())))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.bridge_dialog = true;
                        cx.notify();
                    }))
            })
            .child(div().size(px(6.)).rounded_full().bg(rgb(dot)))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(muted_color()))
                    .child(self.bridge_status.label()),
            )
            .into_any_element()
    }
    pub(crate) fn toolbar(
        &self,
        left_padding: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Classic stays hidden in GPUI; immersive is GPUI-only. Labelled
        // segments read better than bare icons: each mode names its layout.
        let inline_settings = self.inline_settings_active();
        let inline_capture = self.inline_capture_active();
        let is_library =
            self.preview.page == PreviewPage::Library && !inline_settings && !inline_capture;
        let can_back = self.can_navigate_back();
        let back_dest = self.back_destination_label();
        let modes = self.library_mode_menu(cx);
        row()
            .h(px(52.))
            .flex_shrink_0()
            .pl(px(left_padding))
            .pr(px(18.))
            .gap(px(9.))
            .bg(rgba(toolbar_bg()))
            .border_b_1()
            .border_color(rgba(divider()))
            .when(matches!(self.sidebar, SidebarMode::Hidden), |d| {
                d.child(
                    track_key_focus(
                        round_button("sidebar-show", "panel-left")
                            .tooltip(text_tip(crate::i18n::s().library.show_sidebar_tip))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_sidebar(SidebarMode::Expanded, cx);
                            })),
                        "sidebar-show",
                        0,
                        ACCENT,
                        window,
                        cx,
                    )
                    .context_menu(self.sidebar_mode_menu(cx)),
                )
            })
            .when(matches!(self.sidebar, SidebarMode::Expanded), |d| {
                d.child(
                    track_key_focus(
                        round_button("sidebar-hide", "panel-left")
                            .tooltip(text_tip(crate::i18n::s().library.collapse_sidebar_tip))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let style = this.sidebar_collapsed_style;
                                this.set_sidebar(style, cx);
                            })),
                        "sidebar-hide",
                        0,
                        ACCENT,
                        window,
                        cx,
                    )
                    .context_menu(self.sidebar_mode_menu(cx)),
                )
            })
            .when(can_back, |d| {
                d.child(
                    round_button("toolbar-back", "chevron-left")
                        .size(px(28.))
                        .rounded(px(7.))
                        .tooltip(text_tip(crate::i18n::s().library.back_to(&back_dest, crate::platform::BACK_SHORTCUT_LABEL)))
                        .on_click(cx.listener(|this, _, _, cx| this.navigate_back(cx))),
                )
            })
            .when(inline_settings, |d| {
                let lib = &crate::i18n::s().library;
                let back_tip = if self.translation_page.active {
                    if self.translation_page.return_to_capture {
                        lib.back_to_translation
                    } else {
                        lib.back_to_settings
                    }
                } else {
                    lib.back_to_library
                };
                d.child(
                    round_button("settings-back", "chevron-left")
                        .size(px(28.))
                        .rounded(px(7.))
                        .tooltip(text_tip(back_tip))
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.translation_page.active {
                                this.close_translation_settings(window, cx);
                            } else {
                                this.close_settings(Some(window), cx);
                            }
                        })),
                )
            })
            .child(if can_back {
                row()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .items_center()
                    .child(
                        div()
                            .id("toolbar-breadcrumb-parent")
                            .px(px(5.))
                            .py(px(2.))
                            .rounded(px(5.))
                            .cursor_pointer()
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(muted_color()))
                            .hover(|s| s.bg(rgba(hover())).text_color(rgb(text())))
                            .on_click(cx.listener(|this, _, _, cx| this.navigate_back(cx)))
                            .child(back_dest),
                    )
                    .child(icon("chevron-right", 12.).text_color(rgb(muted_color())))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(self.preview_title()),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(15.))
                    .font_weight(FontWeight::BOLD)
                    .child(self.preview_title())
                    .into_any_element()
            })
            .when(!inline_settings && !inline_capture, |d| {
                d.child(
                    round_button("add", "plus")
                        .tooltip(text_tip(crate::i18n::s().library.create_bottle_btn))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_preview_sheet(crate::state::PreviewSheet::CreateBottle, cx)
                        }))
                        .key_focus("toolbar-add"),
                )
            })
            .when(is_library, |d| d.child(modes))
            .when(
                // The list owns both timeline layouts; the switch picks
                // single or staggered columns.
                is_library && (self.mode == 1 || self.mode == crate::preferences::STAGGERED_MODE),
                |d| d.child(self.timeline_layout_control(cx)),
            )
            .when(
                // Only the shelf has a cover size; the timeline and the classic
                // list draw fixed rows.
                is_library && self.mode == 0,
                |d| d.child(self.cover_size_control(cx)),
            )
            .when(!inline_settings && !inline_capture, |d| {
                d.child(self.options_menu(cx))
            })
            .when(is_library, |d| {
                d.child(
                    div()
                        .w(px(190.))
                        .flex_shrink_0()
                        .child(
                            gpui_kit::component::input::Input::new(&self.search)
                                .prefix(icon("search", 13.).text_color(rgb(muted_color())))
                                .cleanable(true),
                        ),
                )
            })
    }

    /// 单列/双列 segmented switch for the timeline family. The staggered rail
    /// is a variant of the list rather than its own picker entry, so the
    /// switch just moves the mode slot between 1 and STAGGERED_MODE.
    fn timeline_layout_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let staggered = self.mode == crate::preferences::STAGGERED_MODE;
        let mut group = row()
            .p(px(3.))
            .gap(px(2.))
            .rounded_full()
            .border_1()
            .border_color(rgba(divider()))
            .bg(rgba(hover()));
        let lib = &crate::i18n::s().library;
        for (target, icon_name, label) in [
            (1usize, "rows-3", lib.column_single),
            (
                crate::preferences::STAGGERED_MODE,
                "git-commit-vertical",
                lib.column_double,
            ),
        ] {
            let active = (target == crate::preferences::STAGGERED_MODE) == staggered;
            group = group.child(
                row()
                    .id(("timeline-layout", target))
                    .h(px(22.))
                    .px(px(9.))
                    .gap(px(5.))
                    .rounded_full()
                    .text_size(px(11.))
                    .cursor_pointer()
                    .when(active, |d| d.bg(rgba(hover_strong())))
                    .when(!active, |d| {
                        d.text_color(rgb(muted_color()))
                            .hover(|s| s.bg(rgba(hover())))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_mode(target, cx);
                    }))
                    .child(icon(icon_name, 12.))
                    .child(label)
                    .key_focus(("timeline-layout", target)),
            );
        }
        group.into_any_element()
    }

    /// Stepped cover size for the shelf. The thumb sits on the level the covers
    /// actually draw, so an automatic shelf still reads truthfully; dragging or
    /// double-clicking leaves that mode. Every level fills the row, so the
    /// shelf never leaves a gap at the edges.
    fn cover_size_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let automatic = self.cover_width <= f32::EPSILON;
        let levels = crate::library_layout::cover_levels(self.cover_span.get());
        let current = if automatic {
            self.cover_unit.get()
        } else {
            self.cover_width
        };
        let position = cover_level_index(&levels, current);
        let fraction = if levels.len() > 1 {
            position as f32 / (levels.len() - 1) as f32
        } else {
            0.
        };
        let thumb = fraction * COVER_SLIDER_WIDTH - COVER_SLIDER_THUMB / 2.;
        let label: SharedString = if automatic {
            crate::i18n::s().library.cover_auto.into()
        } else {
            format!("{:.0}", levels[position]).into()
        };
        let ticks: Vec<AnyElement> = (1..levels.len().saturating_sub(1))
            .map(|index| {
                let offset =
                    COVER_SLIDER_WIDTH * index as f32 / (levels.len() - 1) as f32;
                div()
                    .absolute()
                    .left(px(offset - 1.))
                    .top(px(COVER_SLIDER_THUMB / 2. + 0.5 + 1.))
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
        let down_levels = levels.clone();
        let moved_levels = levels.clone();
        row()
            .gap(px(8.))
            .flex_shrink_0()
            .child(
                div()
                    .id("cover-size-track")
                    .relative()
                    .w(px(COVER_SLIDER_WIDTH))
                    .h(px(COVER_SLIDER_THUMB + 6.))
                    .cursor_pointer()
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
                            .top(px(COVER_SLIDER_THUMB / 2. + 0.5))
                            .h(px(5.))
                            .w_full()
                            .rounded_full()
                            .bg(rgba(slider_track_bg())),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(COVER_SLIDER_THUMB / 2. + 0.5))
                            .h(px(5.))
                            .w(px(COVER_SLIDER_WIDTH * fraction))
                            .rounded_full()
                            .bg(rgb(ACCENT)),
                    )
                    .children(ticks)
                    .child(
                        div()
                            .absolute()
                            .left(px(thumb.clamp(
                                0.,
                                COVER_SLIDER_WIDTH - COVER_SLIDER_THUMB,
                            )))
                            .top(px(3.))
                            .size(px(COVER_SLIDER_THUMB))
                            .rounded_full()
                            .bg(rgb(0xffffff))
                            .border_1()
                            .border_color(rgba(slider_thumb_border()))
                            .shadow_xs(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            if event.click_count >= 2 {
                                this.reset_cover_width(cx);
                                return;
                            }
                            this.set_cover_width(
                                cover_slider_value(
                                    f32::from(event.position.x),
                                    down.get(),
                                    &down_levels,
                                ),
                                cx,
                            );
                        }),
                    )
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        if !event.dragging() {
                            return;
                        }
                        this.set_cover_width(
                            cover_slider_value(
                                f32::from(event.position.x),
                                moved.get(),
                                &moved_levels,
                            ),
                            cx,
                        );
                    })),
            )
            .child(muted(label).w(px(34.)).text_size(px(11.)))
            .into_any_element()
    }
    pub(crate) fn backdrop(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Per-image channels preserve continuous blending even during rapid A → B → C hovers.
        let mut layers = div().absolute().size_full().overflow_hidden().bg(rgb(bg()));
        if !self.flag("backdrop-fill") {
            return layers.into_any_element();
        }
        for i in 0..self.games().len() {
            let target = if self.hovered.is_some() && self.ambient_index == i {
                0.24_f32
            } else {
                0.0
            };
            let alpha = transition(
                ElementId::from(("ambient", i)),
                target,
                self.timing(BACKDROP_MS),
                window,
                cx,
            );
            if alpha > 0.001
                && let Some(path) = self.ambient_art(i)
            {
                layers = layers.child(
                    self.artwork_image(path)
                        .blur(self.backdrop_blur)
                        .absolute()
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .opacity(alpha),
                );
            }
        }
        layers.into_any_element()
    }
    pub(crate) fn cover(
        &self,
        index: usize,
        width: f32,
        height: f32,
        title_overlay: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovering = self.hovered == Some(index) && !self.detail_open;
        let amount = transition(
            ElementId::from(("cover-hover", index)),
            if hovering { 1_f32 } else { 0.0 },
            self.timing(HOVER_MS),
            window,
            cx,
        );
        // Frame stays fixed; only artwork expands, avoiding shelf reflow.
        let iw = width * (1. + amount * 0.025);
        let ih = height * (1. + amount * 0.025);
        let artwork = self.artwork(index);
        let title = self
            .game(index)
            .map(|game| game.title.clone())
            .unwrap_or_default();
        let (ph_start, ph_end) = placeholder_gradient();
        track_key_focus(
            div()
                .id(("cover", index))
                .relative()
                .w(px(width))
                .h(px(height))
                .flex_shrink_0()
                .rounded(px(8.))
                .overflow_hidden()
                .border_1()
                .border_color(rgba(if hovering {
                    if is_dark() { 0xffffff65 } else { 0x00000045 }
                } else {
                    surface_border()
                }))
                .cursor_pointer()
                .on_hover(cx.listener(move |this, active, _, cx| {
                    if *active {
                        this.ambient_index = index;
                        this.hovered = Some(index);
                    } else if this.hovered == Some(index) {
                        this.hovered = None;
                    }
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_detail_from(index, window, cx)
                })),
            ("cover", index),
            0,
            ACCENT,
            window,
            cx,
        )
            .child(match artwork {
                Some(path) => self.cover_image(path, cx)
                    .absolute()
                    .left(px((width - iw) / 2.))
                    .top(px((height - ih) / 2.))
                    .w(px(iw))
                    .h(px(ih))
                    .object_fit(ObjectFit::Cover)
                    .into_any_element(),
                // Same frame, same gradient language as the demo covers.
                None => div()
                    .absolute()
                    .size_full()
                    .bg(gradient(ph_start, ph_end))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon("package", 26.).text_color(rgb(muted_color())))
                    .into_any_element(),
            })
            // Overlay title is used by the classic card; the shelf draws its
            // label below the artwork instead.
            .when(title_overlay, |d| {
                d.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .w_full()
                        .h(px(85.))
                        .bg(gradient(0x00000000, 0x000000b5)),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(9.))
                        .left(px(10.))
                        .right(px(9.))
                        .text_size(px(12.))
                        .text_color(rgb(0xf4f4f6))
                        .font_weight(FontWeight::BOLD)
                        .when(self.preview_single_line(), |d| d.truncate())
                        .when(!self.preview_single_line(), |d| d.line_clamp(2))
                        // `libraryCenterWideCoverText`: wide covers get a
                        // centered label instead of the leading one.
                        .when(
                            self.flag("center-cover-text")
                                && self
                                    .game(index)
                                    .is_some_and(|game| game.landscape),
                            |d| d.text_center(),
                        )
                        .child(title),
                )
            })
            // Running and pending states are always visible; only the idle
            // affordance waits for hover, as it did before.
            .child(match self.runtime_state(index) {
                Some(RuntimeState::Running) => {
                    live_badge(self.runtime_badge_label(index, crate::i18n::s().runtime.running))
                    .absolute()
                    .top(px(9.))
                    .right(px(9.))
                    .into_any_element()
                }
                Some(state) if state.is_active() => {
                    phase_badge(self.runtime_badge_label(index, state.phase_label()))
                        .absolute()
                        .top(px(9.))
                        .right(px(9.))
                        .into_any_element()
                }
                _ => self.cover_play_circle(index, amount, cx),
            })
            .context_menu(self.game_menu(index, cx))
            .into_any_element()
    }
    /// The hover play circle. It dispatches the runtime action when there is
    /// one and stops the click so the card does not also open the details.
    fn cover_play_circle(
        &self,
        index: usize,
        amount: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.play_circle(("cover-play", index), index, amount, cx)
    }
    /// Same circle under a caller-chosen id: the immersive page draws the
    /// same game in more than one row, so `cover-play` alone would collide.
    pub(crate) fn play_circle(
        &self,
        id: impl Into<ElementId>,
        index: usize,
        amount: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let action = if self.demo {
            None
        } else {
            self.primary_action(index).action
        };
        let mut circle = row()
            .id(id)
            .absolute()
            .top(px(10. + (1. - amount) * 5.))
            .right(px(10.))
            .justify_center()
            .size(px(30.))
            .rounded_full()
            // Fades in translucent and saturates to the accent as the card's
            // hover peaks — always keeping crisp white icon contrast on accent.
            .bg(rgba(mix_rgba(
                play_circle_bg(),
                fade_rgb(ACCENT, 0.96),
                amount,
            )))
            .border_1()
            .border_color(rgba(mix_rgba(
                art_control_border(),
                fade_rgb(0xffffff, 0.24),
                amount,
            )))
            .text_color(rgb(mix_rgb(art_control_fg(), 0xffffff, amount)))
            .shadow_sm()
            .opacity(amount)
            .child(icon("play", 15.));
        if let Some(action) = action {
            let view = cx.entity().downgrade();
            circle = circle
                .cursor_pointer()
                .hover(|s| s.bg(rgb(ACCENT_HOVER)).text_color(rgb(0xffffff)))
                .on_click(move |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.perform_runtime_action(index, Some(action), cx)
                    });
                    // The card behind this control opens the details panel.
                    cx.stop_propagation();
                });
        }
        circle.into_any_element()
    }
    pub(crate) fn animated_rect(
        &self,
        index: usize,
        target: CardRect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> CardRect {
        // Identity follows the game across order/column/layout changes. Base retains
        // position AND velocity on retarget; dimensions use critical damping.
        let response = Duration::from_millis(if self.lively {
            LIVELY_SPRING_MS
        } else {
            SPRING_MS
        });
        let geometry = Spring::new(response)
            .with_epsilon(0.1)
            .with_travel(self.motion);
        let travel = geometry.with_damping(if self.lively { 0.68 } else { 0.86 });
        let rect = CardRect {
            x: spring(
                ElementId::from(("library-card-x", index)),
                target.x,
                travel,
                window,
                cx,
            ),
            y: spring(
                ElementId::from(("library-card-y", index)),
                target.y,
                travel,
                window,
                cx,
            ),
            width: spring(
                ElementId::from(("library-card-w", index)),
                target.width,
                geometry,
                window,
                cx,
            )
            .max(1.),
            height: spring(
                ElementId::from(("library-card-h", index)),
                target.height,
                geometry,
                window,
                cx,
            )
            .max(1.),
        };
        #[cfg(feature = "visual-test")]
        self.motion_samples.borrow_mut().insert(index, rect);
        rect
    }
    pub(crate) fn card_slot(
        &self,
        index: usize,
        rect: CardRect,
        child: impl IntoElement,
    ) -> Stateful<Div> {
        div()
            .id(("library-slot", index))
            .absolute()
            .left(px(rect.x))
            .top(px(rect.y))
            .w(px(rect.width))
            .h(px(rect.height))
            .child(child)
    }
    fn studio_cluster_header(
        &self,
        cluster_pos: usize,
        cluster: &crate::library_view::StudioCluster,
        top: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key_toggle = cluster.key.clone();
        let collapsed = self.collapsed_studios.contains(&cluster.key);
        let is_filtered = if cluster.is_unknown {
            self.studio_filter == crate::library_view::StudioFilter::Unknown
        } else {
            self.studio_filter.is_studio(&cluster.label)
        };
        let label = cluster.label.clone();
        let is_unknown = cluster.is_unknown;
        let lib = &crate::i18n::s().library;
        row()
            .id(("studio-cluster-header", cluster_pos))
            .absolute()
            .top(px(top))
            .left(px(18.))
            .right(px(18.))
            .h(px(34.))
            .px(px(10.))
            .gap(px(8.))
            .rounded(px(8.))
            .bg(rgba(card_bg()))
            .border_1()
            .border_color(rgba(card_border()))
            .items_center()
            .child(
                row()
                    .id(("studio-cluster-collapse", cluster_pos))
                    .flex_1()
                    .min_w_0()
                    .gap(px(8.))
                    .items_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_studio_collapsed(&key_toggle, cx);
                    }))
                    .child(
                        icon(
                            if collapsed { "chevron-right" } else { "chevron-down" },
                            14.,
                        )
                        .text_color(rgb(muted_color())),
                    )
                    .child(icon("building-2", 14.).text_color(rgb(accent_fg())))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::BOLD)
                            .child(cluster.label.clone()),
                    )
                    .child(
                        muted(crate::i18n::s().library.studio_count(cluster.indices.len()))
                            .text_size(px(11.))
                            .flex_shrink_0(),
                    ),
            )
            .child(selectable_pill(
                ("studio-cluster-filter", cluster_pos),
                if is_filtered { lib.all_studios } else { lib.only_this_studio },
                is_filtered,
                false,
                cx.listener(move |this, _, _, cx| {
                    if is_unknown {
                        this.toggle_unknown_studio_filter(cx);
                    } else {
                        this.click_studio_filter(&label, cx);
                    }
                }),
            ))
            .into_any_element()
    }

    pub(crate) fn shelf(&self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let indices = self.ordered(cx);
        if indices.is_empty() {
            return self.library_viewport(0, self.library_empty_surface(cx));
        }
        if self.group_by_studio {
            let clusters = crate::library_view::group_indices_by_studio(self.games(), &indices);
            let mut y_cursor = 14_f32;
            let mut items: Vec<AnyElement> = Vec::new();
            let mut updated_cover_metrics = false;
            for (ci, cluster) in clusters.iter().enumerate() {
                items.push(self.studio_cluster_header(ci, cluster, y_cursor, cx));
                y_cursor += 34.;
                if !self.collapsed_studios.contains(&cluster.key) {
                    let layout = LibraryLayout::shelf_with_width(
                        &cluster.indices,
                        width,
                        self.cover_width,
                        |i| self.mixed && self.game(i).is_some_and(|game| game.landscape),
                    );
                    if !updated_cover_metrics {
                        self.cover_unit.set(layout.cover_width);
                        self.cover_span.set(crate::library_layout::shelf_available(width));
                        updated_cover_metrics = true;
                    }
                    let y_shift = y_cursor + 10. - 18.;
                    for (index, mut target) in layout.cards {
                        target.y += y_shift;
                        let rect = self.animated_rect(index, target, window, cx);
                        items.push(
                            self.card_slot(
                                index,
                                rect,
                                self.cover(index, rect.width, rect.height, true, window, cx),
                            )
                            .into_any_element(),
                        );
                    }
                    y_cursor += 10. + (layout.height - 36.).max(0.) + 18.;
                } else {
                    y_cursor += 10.;
                }
            }
            if !updated_cover_metrics {
                let fallback = LibraryLayout::shelf_with_width(&indices, width, self.cover_width, |i| {
                    self.mixed && self.game(i).is_some_and(|game| game.landscape)
                });
                self.cover_unit.set(fallback.cover_width);
                self.cover_span.set(crate::library_layout::shelf_available(width));
            }
            let content = div()
                .relative()
                .w_full()
                .h(px(y_cursor.max(100.)))
                .flex_shrink_0()
                .children(items);
            return self.library_viewport(0, content);
        }
        let layout = LibraryLayout::shelf_with_width(&indices, width, self.cover_width, |i| {
            self.mixed && self.game(i).is_some_and(|game| game.landscape)
        });
        // The control reads these to show the size an automatic shelf picked
        // and which flush levels the current row width offers.
        self.cover_unit.set(layout.cover_width);
        self.cover_span.set(crate::library_layout::shelf_available(width));
        let mut content = div()
            .relative()
            .w_full()
            .h(px(layout.height.max(100.)))
            .flex_shrink_0();
        for (index, target) in layout.cards {
            let rect = self.animated_rect(index, target, window, cx);
            content = content.child(self.card_slot(
                index,
                rect,
                self.cover(index, rect.width, rect.height, true, window, cx),
            ));
        }
        self.library_viewport(0, content)
    }
    /// Focal primary run button (Detail panel, Gallery summary, Immersive hero):
    /// filled with the brand accent at rest so the primary CTA never reads as disabled.
    pub(crate) fn play_button(
        &self,
        id: impl Into<ElementId>,
        index: usize,
        demo_label: &'static str,
        cx: &mut Context<Self>,
    ) -> FocusableControl {
        self.play_button_toned(id, index, demo_label, 1., cx)
    }
    /// A play button whose accent fades in with `lit` (0 idle → 1 hot). The
    /// timeline family keeps a row of these calm: they stay legible glass until
    /// their own card is hovered, so the accent lands where the cursor is.
    pub(crate) fn play_button_toned(
        &self,
        id: impl Into<ElementId>,
        index: usize,
        demo_label: &'static str,
        lit: f32,
        cx: &mut Context<Self>,
    ) -> FocusableControl {
        let id = id.into();
        // The demo has no runtime; it keeps its own label and its notice.
        let (label, enabled, busy, action) = if self.demo {
            (demo_label, true, false, None)
        } else {
            let action = self.primary_action(index);
            (action.label, action.enabled, action.busy, action.action)
        };
        let mut button = row()
            .id(id.clone())
            .flex_shrink_0()
            .justify_center()
            .gap(px(6.))
            .px(px(16.))
            .h(px(30.))
            .rounded_full()
            .text_size(px(12.))
            .font_weight(FontWeight::BOLD);
        if busy {
            button = button
                .bg(rgba(hover_strong()))
                .text_color(rgb(subtext()))
                .child(Spinner::new().with_size(px(12.)))
                .child(label);
        } else if enabled {
            let is_destructive = matches!(action, Some(RuntimeAction::Stop | RuntimeAction::Cancel));
            let leading = match action {
                Some(RuntimeAction::Cancel) => icon("x", 13.),
                Some(RuntimeAction::Stop) => icon("square", 12.),
                _ => icon("play", 13.),
            };
            let active_rgb = if is_destructive { DANGER } else { ACCENT };
            let hover_rgb = if is_destructive { 0xff736b } else { ACCENT_HOVER };
            button = button
                .bg(rgba(mix_rgba(hover_strong(), fade_rgb(active_rgb, 1.), lit)))
                .text_color(rgb(mix_rgb(subtext(), 0xffffff, lit)))
                .cursor_pointer()
                .hover(move |s| s.bg(rgb(hover_rgb)).text_color(rgb(0xffffff)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.demo {
                        this.show_notice(crate::state::demo_notice(), cx);
                    } else {
                        this.perform_runtime_action(index, action, cx);
                    }
                }))
                .child(leading.into_any_element())
                .child(label);
        } else {
            let leading = match action {
                Some(RuntimeAction::Stop) => icon("square", 12.),
                _ => icon("play", 13.),
            };
            button = button
                .bg(rgba(card_bg()))
                .border_1()
                .border_color(rgba(card_border()))
                .text_color(rgb(muted_color()))
                .opacity(0.65)
                .child(leading.into_any_element())
                .child(label);
        }
        let mut focusable = button.key_focus(id).tab_stop(enabled && !busy);
        if lit > 0.5 && enabled && !busy {
            focusable = focusable.light_ring();
        }
        focusable
    }
    /// Ordered indices for the timeline family (list and staggered). The demo
    /// keeps its hand-tuned featured order for the default sort; every other
    /// case uses the same memoised order as the other modes.
    pub(crate) fn timeline_ordered(&self, cx: &mut Context<Self>) -> Vec<usize> {
        let games = self.games();
        let mut indices = if self.demo && self.preview.sort == 0 {
            let mut indices = self.filtered(cx);
            let featured = [
                "Northwind Signal",
                "Saltlight",
                "Quiet Frequency",
                "Emberline",
            ];
            indices.sort_by_key(|&i| {
                featured
                    .iter()
                    .position(|s| games[i].title.starts_with(s))
                    .unwrap_or(10 + i)
            });
            if self.reversed {
                indices.reverse();
            }
            indices
        } else {
            self.ordered(cx)
        };
        if self.group_by_studio {
            indices = crate::library_view::group_indices_by_studio(games, &indices)
                .into_iter()
                .flat_map(|cluster| cluster.indices)
                .collect();
        }
        indices
    }
    pub(crate) fn timeline(&self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let games = self.games();
        let indices = self.timeline_ordered(cx);
        let rail = if width > 1000. { 100. } else { 48. };
        let cw = if width > 1000. { 145. } else { 125. };
        let ch = cw * 1.5 + 26.;
        let layout = LibraryLayout::timeline(&indices, width);
        let mut content = div()
            .relative()
            .w_full()
            .h(px(layout.height.max(100.)))
            .flex_shrink_0();
        // Node → card links, collected while the rows build and painted by one
        // overlay canvas afterwards (the canvas has no hitboxes).
        let mut links = Vec::new();
        let mut previous_group: Option<String> = None;
        for (position, (index, target)) in layout.cards.into_iter().enumerate() {
            let group = self.staggered_group_label(index, position);
            let rail_label = if position == 0 || group != previous_group {
                group.clone().unwrap_or_default()
            } else {
                String::new()
            };
            previous_group = group;
            let rect = self.animated_rect(index, target, window, cx);
            let g = &games[index];
            let mut meta = row().flex_shrink_0().flex_wrap().gap(px(7.)).mt(px(3.));
            if self.demo {
                meta = meta
                    .child(icon("clock-3", 12.).text_color(rgb(muted_color())))
                    .child(muted(if position == 0 {
                        crate::i18n::s().library.demo_time_even
                    } else if position == 1 {
                        crate::i18n::s().library.demo_time_odd
                    } else {
                        crate::i18n::s().library.demo_time_evening
                    }))
                    .child(badge("package", self.bottle_name_at(g.bottle)))
                    .child(badge("clock-3", crate::i18n::s().library.demo_duration_8m));
            } else {
                if let Some(label) = crate::model::format_last_played(g.last_played) {
                    meta = meta
                        .child(icon("clock-3", 12.).text_color(rgb(muted_color())))
                        .child(muted(label));
                }
                if g.bottle > 0 {
                    meta = meta.child(badge("package", self.bottle_name_at(g.bottle)));
                }
                if let Some(label) = crate::model::format_play_time(g.play_time) {
                    meta = meta.child(badge("clock-3", label));
                }
            }
            if !g.tags.is_empty() {
                for (ti, tag) in g.tags.iter().take(2).enumerate() {
                    let tag_click = tag.clone();
                    meta = meta.child(
                        crate::controls::tag_badge_subtle(tag.clone())
                            .id(("timeline-tag", index * 16 + ti))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(hover())))
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                this.click_tag_filter(
                                    &tag_click,
                                    event.modifiers().platform || event.modifiers().shift,
                                    cx,
                                );
                                cx.stop_propagation();
                            })),
                    );
                }
                if g.tags.len() > 2 {
                    meta = meta.child(crate::controls::tag_badge_subtle(format!(
                        "+{}",
                        g.tags.len() - 2
                    )));
                }
            }
            if let Some(state) = self.runtime_state(index) {
                meta = if state == RuntimeState::Running {
                    meta.child(live_badge(self.runtime_badge_label(index, crate::i18n::s().runtime.running)))
                } else if state.is_active() {
                    meta.child(phase_badge(
                        self.runtime_badge_label(index, state.phase_label()),
                    ))
                } else {
                    meta
                };
            }
            let hover = transition(
                ElementId::from(("timeline-hover", index)),
                if self.hovered == Some(index) {
                    1_f32
                } else {
                    0.0
                },
                self.timing(HOVER_MS),
                window,
                cx,
            );
            // A hovered row grows its link from the rail node to the card
            // edge; the same curve as the staggered rail, always left→right.
            if hover > 0.005 {
                let node_y = rect.y + ch / 2.;
                links.push(connector_curve(
                    (rect.x + rail * 0.55 + 4.5, node_y),
                    (rect.x + rail + 17., node_y),
                    hover,
                ));
            }
            content = content.child(
                self.card_slot(
                    index,
                    rect,
                    row()
                        .size_full()
                        .items_start()
                        .gap(px(18.))
                        .child(
                            div()
                                .relative()
                                .w(px(rail))
                                .h(px(ch))
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(rail * 0.55))
                                        .top(px(-12.))
                                        .bottom(px(-12.))
                                        .w(px(1.))
                                        .bg(rgba(surface_border())),
                                )
                                .child(
                                    muted(rail_label)
                                        .absolute()
                                        .top(px(4.))
                                        .left_0()
                                        .max_w(px(rail * 0.55 - 8.))
                                        .truncate()
                                        .text_size(px(11.)),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(rail * 0.55 - 4.))
                                        .top(px(ch / 2. - 4.))
                                        .size(px(8.))
                                        .rounded_full()
                                        .bg(rgb(if position == 0 || self.hovered == Some(index) {
                                            ACCENT
                                        } else {
                                            timeline_dot_idle()
                                        })),
                                ),
                        )
                        .child(
                            row()
                                .id(("timeline", index))
                                .flex_1()
                                .min_w_0()
                                .h(px(ch))
                                .gap(px(19.))
                                .p(px(13.))
                                .rounded(px(16.))
                                .bg(rgba(if hover > 0.1 { hover_strong() } else { card_bg() }))
                                .border_1()
                                .border_color(rgba(card_border()))
                                .on_hover(cx.listener(move |this, active, _, cx| {
                                    this.hovered = if *active { Some(index) } else { None };
                                    this.ambient_index = index;
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .id(("timeline-art", index))
                                        .w(px(cw))
                                        .h(px(ch - 26.))
                                        .flex_shrink_0()
                                        .rounded(px(6.))
                                        .overflow_hidden()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_detail(index, cx)
                                        }))
                                        .child(match self.artwork(index) {
                                            Some(path) => self.artwork_image(path)
                                                .size_full()
                                                .object_fit(ObjectFit::Cover)
                                                .into_any_element(),
                                            None => {
                                                let (ph_start, ph_end) = placeholder_gradient();
                                                div()
                                                    .size_full()
                                                    .bg(gradient(ph_start, ph_end))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        icon("package", 26.)
                                                            .text_color(rgb(muted_color())),
                                                    )
                                                    .into_any_element()
                                            }
                                        })
                                        .context_menu(self.game_menu(index, cx)),
                                )
                                .child(
                                    track_key_focus(
                                        col()
                                            .id(("timeline-body", index))
                                            .flex_1()
                                            .min_w_0()
                                            .justify_center()
                                            .h_full()
                                            .gap(px(8.))
                                            .rounded(px(8.))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.open_detail_from(index, window, cx)
                                            })),
                                        ("timeline-body", index),
                                        0,
                                        ACCENT,
                                        window,
                                        cx,
                                    )
                                        .child(
                                            div()
                                                .text_size(px(19.))
                                                .line_height(relative(1.25))
                                                .font_weight(FontWeight::BOLD)
                                                .flex_shrink_0()
                                                .child(g.title.clone()),
                                        )
                                        .when(self.demo || g.studio.is_some(), |d| {
                                            d.child(
                                                muted(g.studio())
                                                    .flex_shrink_0()
                                                    .truncate(),
                                            )
                                        })
                                        .child(meta)
                                        .when(g.description.is_some(), |d| {
                                            d.child(
                                                div()
                                                    .mt(px(5.))
                                                    .text_size(px(13.))
                                                    .text_color(rgb(subtext()))
                                                    .line_height(relative(1.55))
                                                    .flex_1()
                                                    .min_h_0()
                                                    .max_h(px(13. * 1.55 * 3.))
                                                    .overflow_hidden()
                                                    .line_clamp(3)
                                                    // Metadata contains paragraph breaks; a card uses one
                                                    // bounded preview, while details retain the full text.
                                                    .child(
                                                        g.description
                                                            .as_deref()
                                                            .unwrap_or_default()
                                                            .split_whitespace()
                                                            .collect::<Vec<_>>()
                                                            .join(" "),
                                                    ),
                                            )
                                        }),
                                )
                                .child(
                                    col()
                                        .justify_center()
                                        .items_end()
                                        .gap(px(12.))
                                        .w(px(106.))
                                        .flex_shrink_0()
                                        .child(self.play_button_toned(
                                            ("timeline-play", index),
                                            index,
                                            crate::i18n::s().library.resume_play,
                                            hover,
                                            cx,
                                        ))
                                        .child(self.game_options(index, cx)),
                                ),
                        ),
                ),
            );
        }
        if indices.is_empty() {
            return self.library_viewport(1, self.library_empty_surface(cx));
        }
        content = content.child(
            canvas(
                move |_, _, _| links,
                move |bounds, links: Vec<ConnectorPaint>, window, _| {
                    paint_connectors(bounds.origin, &links, window)
                },
            )
            .absolute()
            .inset_0(),
        );
        self.library_viewport(1, content)
    }
    pub(crate) fn classic(&self, width: f32, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let indices = self.ordered(cx);
        if indices.is_empty() {
            return self.library_viewport(2, self.library_empty_surface(cx));
        }
        if self.group_by_studio {
            let clusters = crate::library_view::group_indices_by_studio(self.games(), &indices);
            let mut y_cursor = 14_f32;
            let mut items: Vec<AnyElement> = Vec::new();
            for (ci, cluster) in clusters.iter().enumerate() {
                items.push(self.studio_cluster_header(ci, cluster, y_cursor, cx));
                y_cursor += 34.;
                if !self.collapsed_studios.contains(&cluster.key) {
                    let layout = LibraryLayout::classic(&cluster.indices, width);
                    let y_shift = y_cursor + 10. - 18.;
                    for (i, mut target) in layout.cards {
                        target.y += y_shift;
                        let rect = self.animated_rect(i, target, window, cx);
                        let hover = transition(
                            ElementId::from(("classic-hover", i)),
                            if self.hovered == Some(i) { 1_f32 } else { 0. },
                            self.timing(HOVER_MS),
                            window,
                            cx,
                        );
                        let studio = self
                            .game(i)
                            .map(|game| game.studio.clone().unwrap_or_default());
                        let card = col()
                            .size_full()
                            .gap(px(10.))
                            .pb(px(13.))
                            .rounded(px(12.))
                            .overflow_hidden()
                            .bg(rgba(card_bg()))
                            .border_1()
                            .border_color(rgba(card_border()))
                            .child(self.cover(i, rect.width, (rect.height - 55.).max(1.), true, window, cx))
                            .child(
                                row()
                                    .px(px(12.))
                                    .justify_between()
                                    .when(self.demo || studio.is_some(), |d| {
                                        d.child(
                                            muted(studio.clone().unwrap_or_default())
                                                .flex_1()
                                                .min_w_0()
                                                .truncate(),
                                        )
                                    })
                                    .child(self.play_button_toned(("classic-play", i), i, crate::i18n::s().library.play, hover, cx)),
                            );
                        items.push(self.card_slot(i, rect, card).into_any_element());
                    }
                    y_cursor += 10. + (layout.height - 36.).max(0.) + 18.;
                } else {
                    y_cursor += 10.;
                }
            }
            let content = div()
                .relative()
                .w_full()
                .h(px(y_cursor.max(100.)))
                .flex_shrink_0()
                .children(items);
            return self.library_viewport(2, content);
        }
        let layout = LibraryLayout::classic(&indices, width);
        let mut content = div()
            .relative()
            .w_full()
            .h(px(layout.height.max(100.)))
            .flex_shrink_0();
        for (i, target) in layout.cards {
            let rect = self.animated_rect(i, target, window, cx);
            let hover = transition(
                ElementId::from(("classic-hover", i)),
                if self.hovered == Some(i) { 1_f32 } else { 0. },
                self.timing(HOVER_MS),
                window,
                cx,
            );
            // The demo always rendered the studio line, even when it was empty.
            let studio = self
                .game(i)
                .map(|game| game.studio.clone().unwrap_or_default());
            let card = col()
                .size_full()
                .gap(px(10.))
                .pb(px(13.))
                .rounded(px(12.))
                .overflow_hidden()
                .bg(rgba(card_bg()))
                .border_1()
                .border_color(rgba(card_border()))
                .child(self.cover(i, rect.width, (rect.height - 55.).max(1.), true, window, cx))
                .child(
                    row()
                        .px(px(12.))
                        .justify_between()
                        .when(self.demo || studio.is_some(), |d| {
                            d.child(
                                muted(studio.clone().unwrap_or_default())
                                    .flex_1()
                                    .min_w_0()
                                    .truncate(),
                            )
                        })
                        .child(self.play_button_toned(("classic-play", i), i, crate::i18n::s().library.play, hover, cx)),
                );
            content = content.child(self.card_slot(i, rect, card));
        }
        self.library_viewport(2, content)
    }
    /// Rating control for the detail panel: five stars with half-star steps,
    /// which maps onto the 1–10 stored value. Re-clicking the current value
    /// writes `null`, matching "unrated" in the SwiftUI menu.
    /// The five interactive stars alone, sized by `star`; callers add their
    /// own label and numeric value around it.
    pub(crate) fn rating_stars(
        &self,
        index: usize,
        star: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = self.game(index).and_then(|game| game.rating);
        let value = current.map_or(0, |rating| rating.min(10));
        let mut stars = row().gap(px(2.));
        for slot in 0..5_u8 {
            // Points covered by this star: 0, 1 (half) or 2 (full).
            let covered = value.saturating_sub(slot * 2).min(2);
            let half = |id: usize, points: u8, left: f32| {
                div()
                    .id(("rating", index * 32 + id))
                    .absolute()
                    .left(px(left))
                    .top(px(0.))
                    .w(px(star / 2.))
                    .h(px(star))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.set_rating(index, points, cx)))
            };
            stars = stars.child(
                div()
                    .relative()
                    .w(px(star))
                    .h(px(star))
                    .child(icon("star", star).text_color(rgba(star_empty())))
                    .child(
                        div()
                            .absolute()
                            .top(px(0.))
                            .left(px(0.))
                            .h(px(star))
                            .w(px(star * f32::from(covered) / 2.))
                            .overflow_hidden()
                            .child(div().w(px(star)).h(px(star)).child(icon("star", star).text_color(rgb(ACCENT)))),
                    )
                    .child(half(usize::from(slot) * 2, slot * 2 + 1, 0.))
                    .child(half(usize::from(slot) * 2 + 1, slot * 2 + 2, star / 2.)),
            );
        }
        stars.into_any_element()
    }

    pub(crate) fn rating_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let current = self.game(index).and_then(|game| game.rating);
        row()
            .gap(px(10.))
            .items_center()
            .child(muted(crate::i18n::s().library.rating))
            .child(self.rating_stars(index, 20., cx))
            .child(
                muted(current.map_or_else(|| crate::i18n::s().library.unrated.to_owned(), |rating| format!("{rating} / 10")))
                    .text_size(px(12.)),
            )
            .into_any_element()
    }

    pub(crate) fn detail_panel(
        &self,
        index: usize,
        progress: f32,
        width: f32,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(g) = self.game(index) else {
            return div().into_any_element();
        };
        // The detail tint follows the same fill/blur choice as the window backdrop.
        let backdrop = if self.flag("backdrop-fill") {
            self.ambient_art(index)
        } else {
            None
        };
        // The hero cover is a fixed 190 points wide.
        let artwork = self.artwork(index);
        let mut badges = row().flex_wrap().gap(px(8.)).mt(px(8.));
        if self.demo {
            badges = badges
                .child(badge("package", "galgame"))
                .child(badge("star", "8 / 10"));
        } else {
            if g.bottle > 0 {
                badges = badges.child(badge("package", self.bottle_name_at(g.bottle)));
            }
            if let Some(year) = g.year() {
                badges = badges.child(badge("calendar", year));
            }
            if let Some(rating) = g.rating {
                badges = badges.child(badge("star", format!("{rating} / 10")));
            }
            if let Some(play_time) = crate::model::format_play_time(g.play_time) {
                badges = badges.child(badge("clock-3", play_time));
            }
            if let Some(last_played) = crate::model::format_last_played(g.last_played) {
                badges = badges.child(badge("clock-3", last_played));
            }
        }
        if let Some(state) = self.runtime_state(index) {
            badges = if state == RuntimeState::Running {
                badges.child(live_badge(self.runtime_badge_label(index, crate::i18n::s().runtime.running)))
            } else if state.is_active() {
                badges.child(phase_badge(
                    self.runtime_badge_label(index, state.phase_label()),
                ))
            } else {
                badges
            };
        }
        let pw = (width - 100.).clamp(600., 910.).min(width - 36.);
        let ph = (height - 90.).clamp(480., 690.).min(height - 36.);
        let (ph_start, ph_end) = placeholder_gradient();
        div()
            .absolute()
            .inset_0()
            // Modal hitboxes must also block wheel events, including at scroll boundaries.
            .occlude()
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .bg(rgba(overlay()))
            .opacity(progress)
            .child(
                div()
                    .id("detail-dismiss")
                    .absolute()
                    .inset_0()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.close_detail(window, cx);
                    })),
            )
            .child({
                let trap_handle = window
                    .use_keyed_state("detail-modal-trap", cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone();
                let panel = col()
                    // Keep clicks inside the panel from reaching the dismiss layer.
                    .occlude()
                    .absolute()
                    .left(px((width - pw) / 2.))
                    .top(px((height - ph) / 2. + (1. - progress) * 20.))
                    .w(px(pw))
                    .h(px(ph))
                    .rounded(px(18.))
                    .overflow_hidden()
                    .shadow_xl()
                    .border_1()
                    .border_color(rgba(surface_border()))
                    .bg(rgb(crate::theme::surface()))
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .when_some(backdrop, |d, path| {
                                d.child(
                                    self.artwork_image(path)
                                        .blur(self.backdrop_blur)
                                        .size_full()
                                        .object_fit(ObjectFit::Cover)
                                        .opacity(if is_dark() { 0.22 } else { 0.12 }),
                                )
                            }),
                    )
                    .child(
                        row()
                            .relative()
                            .h(px(48.))
                            .px(px(16.))
                            .flex_shrink_0()
                            .justify_between()
                            .child(muted(crate::i18n::s().library.game_details).text_color(rgb(subtext())))
                            .child(row().gap(px(8.)).child(self.game_options(index, cx)).child(
                                round_button("detail-close", "x")
                                    .size(px(27.))
                                    .tooltip(text_tip(crate::i18n::s().common.close_esc))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_detail(window, cx);
                                    }))
                                    .key_focus("detail-close"),
                            )),
                    )
                    .child(
                        div().relative().flex_1().min_h_0().child(
                            self.bouncing_scroll(
                                ("detail-scroll", index),
                                &self.detail_scroll,
                                col()
                                    .w_full()
                                    .flex_shrink_0()
                                    .px(px(28.))
                                    .pb(px(28.))
                                    .gap(px(23.))
                                    .child(
                                        row()
                                            .flex_shrink_0()
                                            .items_start()
                                            .gap(px(25.))
                                            .child(match artwork {
                                                Some(path) => self.cover_image(path, cx)
                                                    .w(px(190.))
                                                    .h(px(278.))
                                                    .flex_shrink_0()
                                                    .rounded(px(9.))
                                                    .object_fit(ObjectFit::Cover)
                                                    .into_any_element(),
                                                None => div()
                                                    .w(px(190.))
                                                    .h(px(278.))
                                                    .flex_shrink_0()
                                                    .rounded(px(9.))
                                                    .bg(gradient(ph_start, ph_end))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        icon("package", 34.)
                                                            .text_color(rgb(muted_color())),
                                                    )
                                                    .into_any_element(),
                                            })
                                            .child(
                                                col()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .gap(px(12.))
                                                    .pt(px(15.))
                                                    .child(
                                                        div()
                                                            .text_size(px(27.))
                                                            .font_weight(FontWeight::BOLD)
                                                            .line_height(relative(1.2))
                                                            .child(g.title.clone()),
                                                    )
                                                    .when_some(g.studio_name(), |d, studio_name| {
                                                        let active =
                                                            self.studio_filter.is_studio(&studio_name);
                                                        let studio_click = studio_name.clone();
                                                        d.child(
                                                            row()
                                                                .id(("detail-studio-link", index))
                                                                .items_center()
                                                                .gap(px(6.))
                                                                .px(px(9.))
                                                                .py(px(3.))
                                                                .rounded_full()
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
                                                                .cursor_pointer()
                                                                .hover(|s| s.bg(rgba(hover())))
                                                                .tooltip(text_tip(crate::i18n::s().library.click_filter_studio_tip))
                                                                .on_click(cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        this.click_studio_filter(
                                                                            &studio_click,
                                                                            cx,
                                                                        );
                                                                        this.close_detail(window, cx);
                                                                    },
                                                                ))
                                                                .child(
                                                                    icon("building-2", 12.).text_color(
                                                                        rgb(if active {
                                                                            accent_fg()
                                                                        } else {
                                                                            muted_color()
                                                                        }),
                                                                    ),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(12.5))
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .text_color(rgb(if active {
                                                                            accent_fg()
                                                                        } else {
                                                                            subtext()
                                                                        }))
                                                                        .child(studio_name),
                                                                ),
                                                        )
                                                    })
                                                    .when(g.release.is_some(), |d| {
                                                        d.child(muted(
                                                            g.release.clone().unwrap_or_default(),
                                                        ))
                                                    })
                                                    .child(badges)
                                                    .when(!self.demo, |d| {
                                                        d.child(self.rating_row(index, cx))
                                                    })
                                                    .child(div().mt(px(12.)).child(
                                                        self.play_button(
                                                            "detail-play",
                                                            index,
                                                            crate::i18n::s().library.resume_play,
                                                            cx,
                                                        )
                                                        .when(self.detail_open, |b| {
                                                            b.auto_focus_key((
                                                                "detail-play-autofocus",
                                                                self.detail_open_generation,
                                                            ))
                                                        })
                                                        .when(!self.detail_open, |b| {
                                                            b.tab_stop(false)
                                                        }),
                                                    ))
                                                    .when(!self.demo, |d| {
                                                        d.child(
                                                            row()
                                                                .flex_wrap()
                                                                .gap(px(8.))
                                                                .child(
                                                                    Button::new("detail-run-settings")
                                                                        .label(crate::i18n::s().library.run_settings_ellipsis)
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.open_preview_sheet(
                                                                                    crate::state::PreviewSheet::RunSettings(index),
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        )),
                                                                )
                                                                .child(
                                                                    Button::new("detail-game-info")
                                                                        .label(crate::i18n::s().library.game_info_cover_ellipsis)
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.open_game_info(index, cx);
                                                                            },
                                                                        )),
                                                                )
                                                                .child(
                                                                    Button::new("detail-game-config")
                                                                        .label(crate::i18n::s().library.gamedb_recommended_ellipsis)
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.open_game_info(index, cx);
                                                                            },
                                                                        )),
                                                                )
                                                                .child(
                                                                    Button::new("detail-translation")
                                                                        .label(crate::i18n::s().library.game_translation_capture_ellipsis)
                                                                        .on_click(cx.listener(
                                                                            move |this, _, _, cx| {
                                                                                this.open_translation_for_game(index, cx);
                                                                            },
                                                                        )),
                                                                ),
                                                        )
                                                    }),
                                            ),
                                    )
                                    .when_some(g.description.clone(), |d, description| {
                                        d.child(
                                            col()
                                                .flex_shrink_0()
                                                .gap(px(12.))
                                                .p(px(20.))
                                                .rounded(px(12.))
                                                .bg(rgba(card_bg()))
                                                .border_1()
                                                .border_color(rgba(card_border()))
                                                .child(
                                                    row().justify_between()
                                                        .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::i18n::s().library.summary))
                                                        .when_some(g.metadata_sources.get("summary").cloned(), |row, source| {
                                                            row.child(muted(source).text_size(px(11.)))
                                                        }),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(13.))
                                                        .text_color(rgb(subtext()))
                                                        .line_height(relative(1.65))
                                                        .child(description),
                                                ),
                                        )
                                    })
                                    .child(self.detail_character_cards(index, cx))
                                    .when(!self.demo, |d| {
                                        d.child(self.detail_tags_section(index, window, cx))
                                    }),
                            ),
                        ),
                    );
                if self.detail_open {
                    panel
                        .track_focus(&trap_handle)
                        .focus_trap("detail-modal", &trap_handle)
                        .into_any_element()
                } else {
                    panel.into_any_element()
                }
            })
            .into_any_element()
    }

    fn detail_character_cards(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Some(game) = self.game(index) else {
            return div().into_any_element();
        };
        let Some(characters) = game
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.characters.as_ref())
            .filter(|characters| !characters.is_empty())
        else {
            return div().into_any_element();
        };
        let selected_character = self
            .detail_character
            .as_deref()
            .and_then(|id| characters.iter().find(|c| c.id == id));
        col()
            .gap(px(14.))
            .p(px(20.))
            .rounded(px(12.))
            .bg(rgba(card_bg()))
            .border_1()
            .border_color(rgba(card_border()))
            .child(
                row()
                    .justify_between()
                    .items_center()
                    .child(
                        row()
                            .gap(px(8.))
                            .items_center()
                            .child(icon("users", 16.).text_color(rgb(ACCENT)))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::i18n::s().library.characters_title))
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(2.))
                                    .rounded_full()
                                    .bg(rgba(badge_bg()))
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(muted_color()))
                                    .child(characters.len().to_string()),
                            ),
                    )
                    .child(muted(crate::i18n::s().library.click_character_tip).text_size(px(11.))),
            )
            .when_some(selected_character, |d, character| {
                d.child(self.character_showcase_card(
                    character,
                    characters,
                    &game.character_images,
                    false,
                    cx,
                ))
            })
            .child(self.character_cards(characters, &game.character_images, Some(index), cx))
            .into_any_element()
    }

    /// Rich character showcase card shared by the Game Detail panel and the Game Info modal.
    pub(crate) fn character_showcase_card(
        &self,
        character: &crate::bridge::dto::CharacterDto,
        characters: &[crate::bridge::dto::CharacterDto],
        images: &std::collections::BTreeMap<String, String>,
        in_game_info: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let ordered = characters_in_display_order(characters, images);
        let total = ordered.len();
        let pos = ordered
            .iter()
            .position(|c| c.id == character.id)
            .unwrap_or(0);
        let prev_id = pos.checked_sub(1).and_then(|i| ordered.get(i)).map(|c| c.id.clone());
        let next_id = ordered.get(pos + 1).map(|c| c.id.clone());

        let original_name = character
            .original_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != character.name);
        let role = character
            .role
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let voice_actor = character
            .voice_actor
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let summary = character
            .summary
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(crate::i18n::s().library.no_character_bio);

        let portrait = match images.get(&character.id) {
            Some(path) => self.artwork_image(std::path::PathBuf::from(path))
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => col()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .child(icon("user", 32.).text_color(rgb(muted_color())))
                .child(muted(crate::i18n::s().library.no_portrait).text_size(px(12.)))
                .into_any_element(),
        };

        let nav_bar = row()
            .justify_between()
            .items_center()
            .pb(px(10.))
            .border_b_1()
            .border_color(rgba(divider()))
            .child(
                Button::new("character-back")
                    .ghost()
                    .small()
                    .icon(icon("chevron-left", 14.))
                    .label(crate::i18n::s().library.character_list_btn)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if in_game_info {
                            if let Some(page) = this.game_info.as_mut() {
                                page.selected_character = None;
                            }
                        } else {
                            this.detail_character = None;
                        }
                        cx.notify();
                    })),
            )
            .when(total > 1, |d| {
                d.child(
                    row()
                        .items_center()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(rgba(divider()))
                        .bg(rgba(card_bg()))
                        .child(
                            Button::new("character-prev")
                                .ghost()
                                .xsmall()
                                .icon(icon("chevron-left", 13.))
                                .tooltip(crate::i18n::s().library.previous_character)
                                .disabled(prev_id.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(id) = prev_id.clone() {
                                        if in_game_info {
                                            if let Some(page) = this.game_info.as_mut() {
                                                page.selected_character = Some(id);
                                            }
                                        } else {
                                            this.detail_character = Some(id);
                                        }
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(div().w(px(1.)).h(px(14.)).bg(rgba(divider())))
                        .child(
                            muted(format!("{} / {}", pos + 1, total))
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .px(px(8.)),
                        )
                        .child(div().w(px(1.)).h(px(14.)).bg(rgba(divider())))
                        .child(
                            Button::new("character-next")
                                .ghost()
                                .xsmall()
                                .icon(icon("chevron-right", 13.))
                                .tooltip(crate::i18n::s().library.next_character)
                                .disabled(next_id.is_none())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(id) = next_id.clone() {
                                        if in_game_info {
                                            if let Some(page) = this.game_info.as_mut() {
                                                page.selected_character = Some(id);
                                            }
                                        } else {
                                            this.detail_character = Some(id);
                                        }
                                        cx.notify();
                                    }
                                })),
                        ),
                )
            });

        col()
            .id("character-showcase")
            .gap(px(16.))
            .p(px(18.))
            .rounded(px(14.))
            .bg(rgba(section_bg()))
            .border_1()
            .border_color(rgba(surface_border()))
            .child(nav_bar)
            .child(
                row()
                    .items_start()
                    .gap(px(22.))
                    .child(
                        col()
                            .w(px(180.))
                            .flex_shrink_0()
                            .gap(px(10.))
                            .child(
                                div()
                                    .w(px(180.))
                                    .h(px(246.))
                                    .rounded(px(12.))
                                    .overflow_hidden()
                                    .bg(rgba(portrait_bg()))
                                    .border_1()
                                    .border_color(rgba(surface_border()))
                                    .p(px(6.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(portrait),
                            )
                            .when_some(voice_actor, |d, actor| {
                                d.child(
                                    row()
                                        .w_full()
                                        .gap(px(7.))
                                        .px(px(10.))
                                        .py(px(7.))
                                        .rounded(px(8.))
                                        .bg(rgba(badge_bg()))
                                        .border_1()
                                        .border_color(rgba(card_border()))
                                        .child(icon("mic", 13.).text_color(rgb(ACCENT)))
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(rgb(ACCENT))
                                                .child("CV"),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgb(text()))
                                                .child(actor.to_owned()),
                                        ),
                                )
                            }),
                    )
                    .child(
                        col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(12.))
                            .child(
                                col()
                                    .gap(px(6.))
                                    .child(
                                        row()
                                            .flex_wrap()
                                            .gap(px(10.))
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_size(px(22.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .line_height(relative(1.25))
                                                    .text_color(rgb(text()))
                                                    .child(character.name.clone()),
                                            )
                                            .when_some(role, |d, r| {
                                                d.child(
                                                    div()
                                                        .px(px(10.))
                                                        .py(px(3.))
                                                        .rounded_full()
                                                        .bg(rgba(accent_subtle_bg()))
                                                        .border_1()
                                                        .border_color(rgba(accent_subtle_border()))
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(rgb(accent_fg()))
                                                        .child(r.to_owned()),
                                                )
                                            }),
                                    )
                                    .when_some(original_name, |d, orig| {
                                        d.child(muted(orig).text_size(px(13.)))
                                    }),
                            )
                            .child(div().w_full().h(px(1.)).bg(rgba(divider())))
                            .child(
                                col()
                                    .gap(px(8.))
                                    .child(
                                        row()
                                            .gap(px(6.))
                                            .items_center()
                                            .child(
                                                div()
                                                    .w(px(3.))
                                                    .h(px(13.))
                                                    .rounded_full()
                                                    .bg(rgb(ACCENT)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(muted_color()))
                                                    .child(crate::i18n::s().library.character_bio),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(rgb(subtext()))
                                            .line_height(relative(1.75))
                                            .child(summary.to_owned()),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn character_cards(
        &self,
        characters: &[crate::bridge::dto::CharacterDto],
        images: &std::collections::BTreeMap<String, String>,
        open_game: Option<usize>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let selected_id = if open_game.is_some() {
            self.detail_character.as_deref()
        } else {
            self.game_info
                .as_ref()
                .and_then(|page| page.selected_character.as_deref())
        };
        let mut cards = row()
            .id("detail-character-cards")
            .items_start()
            .gap(px(14.))
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .on_scroll_wheel(|event: &ScrollWheelEvent, window, cx| {
                let delta = event.delta.pixel_delta(window.line_height());
                if delta.x.abs() > delta.y.abs() {
                    cx.stop_propagation();
                }
            });
        // Match the portrait lookup below and retain provider order within each group.
        for character in characters_in_display_order(characters, images) {
            let id = character.id.clone();
            let is_selected = selected_id == Some(character.id.as_str());
            let role = character
                .role
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let original_name = character
                .original_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty() && *s != character.name);
            let voice_actor = character
                .voice_actor
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let summary = character
                .summary
                .as_deref()
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|s| !s.is_empty());

            let portrait_frame = div()
                .relative()
                .w_full()
                .h(px(CHARACTER_PORTRAIT_HEIGHT))
                .flex_shrink_0()
                .rounded(px(8.))
                .overflow_hidden()
                .bg(rgba(portrait_bg()))
                .border_1()
                .border_color(rgba(card_border()))
                .child(match images.get(&character.id) {
                    Some(path) => CharacterPortrait { path: path.into() }.into_any_element(),
                    None => character_portrait_placeholder(crate::i18n::s().library.no_avatar),
                })
                .when_some(role, |d, r| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(6.))
                            .left(px(6.))
                            .px(px(7.))
                            .py(px(2.))
                            .rounded_full()
                            .bg(rgba(role_chip_bg()))
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(role_chip_text()))
                            .child(r.to_owned()),
                    )
                });

            let mut info = col().flex_1().min_h_0().w_full().gap(px(3.));
            info = info.child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_size(px(13.))
                    .truncate()
                    .child(character.name.clone()),
            );
            if let Some(orig) = original_name {
                info = info.child(muted(orig).text_size(px(11.)).truncate());
            }
            if let Some(actor) = voice_actor {
                info = info.child(
                    row()
                        .gap(px(4.))
                        .items_center()
                        .child(icon("mic", 11.).text_color(rgb(accent_fg())))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(rgb(accent_fg()))
                                .child(actor.to_owned()),
                        ),
                );
            }
            if let Some(summary) = summary {
                info = info.child(
                    muted(summary)
                        .mt(px(2.))
                        .text_size(px(11.))
                        .line_height(px(15.))
                        .overflow_hidden()
                        .line_clamp(2),
                );
            }

            let card = col()
                .id(format!("detail-character:{}", character.id))
                .w(px(CHARACTER_CARD_WIDTH))
                .h(px(272.))
                .flex_shrink_0()
                .gap(px(8.))
                .p(px(CHARACTER_CARD_PADDING))
                .overflow_hidden()
                .rounded(px(12.))
                .border_1()
                .border_color(rgba(if is_selected {
                    if is_dark() { 0xff891baa } else { 0x00000038 }
                } else {
                    card_border()
                }))
                .bg(rgba(if is_selected { selected_bg() } else { card_bg() }))
                .cursor_pointer()
                .hover(|style| {
                    style.bg(rgba(if is_selected {
                        selected_bg()
                    } else {
                        hover_strong()
                    }))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if open_game.is_some() {
                        if this.detail_character.as_deref() == Some(id.as_str()) {
                            this.detail_character = None;
                        } else {
                            this.detail_character = Some(id.clone());
                        }
                    }
                    if let Some(page) = this.game_info.as_mut() {
                        page.selected_character = Some(id.clone());
                    }
                    cx.notify();
                }))
                .child(portrait_frame)
                .child(info);
            cards = cards.child(card);
        }
        cards.into_any_element()
    }

    fn tag_filter_menu(
        &self,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let view = cx.entity().downgrade();
        let tags = crate::library_view::collect_tag_counts(self.games(), self.show_hidden());
        let untagged = crate::library_view::untagged_count(self.games(), self.show_hidden());
        let current = self.tag_filter.clone();
        move |mut menu, _, _| {
            menu = menu.scrollable(true);
            let v = view.clone();
            menu = menu.item(
                PopupMenuItem::new(crate::i18n::s().library.all_games)
                    .checked(!current.is_active())
                    .on_click(move |_, _, cx| {
                        let _ = v.update(cx, |this, cx| this.clear_tag_filter(cx));
                    }),
            );
            if !tags.is_empty() {
                menu = menu.separator();
            }
            for (tag, count) in &tags {
                let checked = current.contains(tag);
                let tag_click = tag.clone();
                let v = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(format!("{tag} ({count})"))
                        .checked(checked)
                        .on_click(move |_, _, cx| {
                            let _ = v.update(cx, |this, cx| {
                                this.click_tag_filter(&tag_click, false, cx);
                            });
                        }),
                );
            }
            if untagged > 0 || current == crate::library_view::TagFilter::Untagged {
                let v = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(crate::i18n::s().library.untagged_count(untagged))
                        .checked(current == crate::library_view::TagFilter::Untagged)
                        .on_click(move |_, _, cx| {
                            let _ = v.update(cx, |this, cx| {
                                this.set_tag_filter(
                                    crate::library_view::TagFilter::Untagged,
                                    cx,
                                );
                            });
                        }),
                );
            }
            menu = menu.separator();
            let v = view.clone();
            menu.item(PopupMenuItem::new(crate::i18n::s().library.manage_tags).on_click(move |_, _, cx| {
                let _ = v.update(cx, |this, cx| this.open_manage_tags_sheet(cx));
            }))
        }
    }

    fn tag_context_menu(
        &self,
        tag: String,
        cx: &Context<Self>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let view = cx.entity().downgrade();
        let active = self.tag_filter.contains(&tag);
        let is_demo = self.demo;
        move |mut menu, _, _| {
            let tag_filter = tag.clone();
            let v = view.clone();
            menu = menu.item(
                PopupMenuItem::new(if active { crate::i18n::s().library.clear_tag_item } else { crate::i18n::s().library.only_this_tag })
                    .on_click(move |_, _, cx| {
                        let _ = v.update(cx, |this, cx| {
                            this.click_tag_filter(&tag_filter, false, cx);
                        });
                    }),
            );
            if !is_demo {
                menu = menu.separator();
                let tag_rename = tag.clone();
                let v = view.clone();
                menu = menu.item(PopupMenuItem::new(crate::i18n::s().library.rename_ellipsis).on_click(move |_, _, cx| {
                    let _ = v.update(cx, |this, cx| {
                        this.open_manage_tags_sheet(cx);
                        this.begin_rename_library_tag(&tag_rename, None, cx);
                    });
                }));
                let tag_delete = tag.clone();
                let v = view.clone();
                menu = menu.item(PopupMenuItem::new(crate::i18n::s().library.delete_tag).on_click(move |_, _, cx| {
                    let _ = v.update(cx, |this, cx| {
                        this.delete_library_tag(&tag_delete, cx);
                    });
                }));
            }
            menu
        }
    }

    fn library_filter_drawer(&self, cx: &Context<Self>) -> AnyElement {
        let games = self.games();
        let show_hidden = self.show_hidden();
        let slot = self.bottle_slot();
        let tags = crate::library_view::collect_tag_counts(games, show_hidden);
        let untagged = crate::library_view::untagged_count(games, show_hidden);
        let studios = crate::library_view::collect_studio_counts(games, slot, show_hidden);
        let unknown_studios = crate::library_view::unknown_studio_count(games, slot, show_hidden);
        let has_tags = !tags.is_empty() || self.tag_filter.is_active();
        let has_studios = !studios.is_empty() || self.studio_filter.is_active();
        let active_tab = self.effective_filter_bar_tab(has_tags, has_studios);

        let mut strip = row()
            .id("library-tag-bar-scroll")
            .flex_1()
            .min_w_0()
            .items_center()
            .gap(px(6.))
            .overflow_x_scroll()
            .restrict_scroll_to_axis();

        match active_tab {
            crate::ui_state::FilterBarTab::Tags => {
                strip = strip.child(tag_chip(
                    "library-tag-all",
                    crate::i18n::s().library.all_tags,
                    !self.tag_filter.is_active(),
                    None,
                    cx.listener(|this, _, _, cx| {
                        this.clear_tag_filter(cx);
                    }),
                ));
                for (ti, (tag, count)) in tags.into_iter().enumerate() {
                    let active = self.tag_filter.contains(&tag);
                    let tag_click = tag.clone();
                    strip = strip.child(tag_chip(
                        ("library-tag-pill", ti),
                        tag,
                        active,
                        Some(count),
                        cx.listener(move |this, event: &ClickEvent, _, cx| {
                            this.click_tag_filter(
                                &tag_click,
                                event.modifiers().platform || event.modifiers().shift,
                                cx,
                            );
                        }),
                    ));
                }
                if untagged > 0 || self.tag_filter == crate::library_view::TagFilter::Untagged {
                    let active = self.tag_filter == crate::library_view::TagFilter::Untagged;
                    strip = strip.child(tag_chip(
                        "library-tag-untagged",
                        crate::i18n::s().library.untagged,
                        active,
                        Some(untagged),
                        cx.listener(move |this, _, _, cx| {
                            let next = if active {
                                crate::library_view::TagFilter::All
                            } else {
                                crate::library_view::TagFilter::Untagged
                            };
                            this.set_tag_filter(next, cx);
                        }),
                    ));
                }
            }
            crate::ui_state::FilterBarTab::Studios => {
                strip = strip.child(tag_chip(
                    "library-studio-all",
                    crate::i18n::s().library.all_studios,
                    !self.studio_filter.is_active(),
                    None,
                    cx.listener(|this, _, _, cx| {
                        this.clear_studio_filter(cx);
                    }),
                ));
                for (si, (studio, count)) in studios.into_iter().enumerate() {
                    let active = self.studio_filter.is_studio(&studio);
                    let studio_click = studio.clone();
                    strip = strip.child(tag_chip(
                        ("library-studio-pill", si),
                        studio,
                        active,
                        Some(count),
                        cx.listener(move |this, _, _, cx| {
                            this.click_studio_filter(&studio_click, cx);
                        }),
                    ));
                }
                if unknown_studios > 0
                    || self.studio_filter == crate::library_view::StudioFilter::Unknown
                {
                    let active =
                        self.studio_filter == crate::library_view::StudioFilter::Unknown;
                    strip = strip.child(tag_chip(
                        "library-studio-unknown",
                        crate::library_view::unknown_studio_label(),
                        active,
                        Some(unknown_studios),
                        cx.listener(|this, _, _, cx| {
                            this.toggle_unknown_studio_filter(cx);
                        }),
                    ));
                }
            }
        }

        let show_switcher = (has_tags && has_studios)
            || (self.tag_filter.is_active() || self.studio_filter.is_active());
        let mut bar = row()
            .id("library-filter-drawer")
            .w_full()
            .h(px(44.))
            .flex_shrink_0()
            .items_center()
            .px(px(20.))
            .gap(px(8.))
            .bg(rgba(toolbar_bg()))
            .border_b_1()
            .border_color(rgba(divider()));

        if show_switcher {
            let mut switcher = row()
                .flex_shrink_0()
                .p(px(2.))
                .gap(px(2.))
                .rounded_full()
                .bg(rgba(hover()))
                .border_1()
                .border_color(rgba(divider()));
            for (tab, icon_name, label, has_dot) in [
                (
                    crate::ui_state::FilterBarTab::Tags,
                    "tag",
                    crate::i18n::s().library.tab_tags,
                    self.tag_filter.is_active()
                        && active_tab != crate::ui_state::FilterBarTab::Tags,
                ),
                (
                    crate::ui_state::FilterBarTab::Studios,
                    "building-2",
                    crate::i18n::s().library.tab_studios,
                    self.studio_filter.is_active()
                        && active_tab != crate::ui_state::FilterBarTab::Studios,
                ),
            ] {
                let selected = active_tab == tab;
                switcher = switcher.child(
                    row()
                        .id(match tab {
                            crate::ui_state::FilterBarTab::Tags => "filter-tab-tags",
                            crate::ui_state::FilterBarTab::Studios => "filter-tab-studios",
                        })
                        .h(px(22.))
                        .px(px(9.))
                        .gap(px(4.))
                        .rounded_full()
                        .items_center()
                        .cursor_pointer()
                        .when(selected, |d| d.bg(rgba(hover_strong())))
                        .when(!selected, |d| {
                            d.text_color(rgb(muted_color()))
                                .hover(|s| s.bg(rgba(hover())))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_filter_bar_tab(tab, cx);
                        }))
                        .child(icon(icon_name, 11.))
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(if selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(label),
                        )
                        .when(has_dot, |d| {
                            d.child(div().size(px(5.)).rounded_full().bg(rgb(ACCENT)))
                        }),
                );
            }
            bar = bar.child(switcher);
        }

        bar = bar.child(strip);

        if active_tab == crate::ui_state::FilterBarTab::Tags && self.studio_filter.is_active() {
            bar = bar.child(selectable_pill(
                "library-cross-studio-clear",
                crate::i18n::s().library.chip_studio(&self.studio_filter.display_label()),
                true,
                false,
                cx.listener(|this, _, _, cx| {
                    this.clear_studio_filter(cx);
                }),
            ));
        } else if active_tab == crate::ui_state::FilterBarTab::Studios
            && self.tag_filter.is_active()
        {
            bar = bar.child(selectable_pill(
                "library-cross-tag-clear",
                crate::i18n::s().library.chip_tag(&self.tag_filter.display_label()),
                true,
                false,
                cx.listener(|this, _, _, cx| {
                    this.clear_tag_filter(cx);
                }),
            ));
        }

        if self.has_active_library_filter() {
            bar = bar.child(selectable_pill(
                "library-tag-clear",
                crate::i18n::s().library.clear_filters,
                false,
                false,
                cx.listener(|this, _, _, cx| {
                    this.clear_all_library_filters(cx);
                }),
            ));
        }

        bar = bar.child(selectable_pill(
            "library-studio-cluster-toggle",
            crate::i18n::s().library.group_by_studio,
            self.group_by_studio,
            false,
            cx.listener(|this, _, _, cx| {
                this.toggle_group_by_studio(cx);
            }),
        ));

        if !self.demo && active_tab == crate::ui_state::FilterBarTab::Tags {
            bar = bar.child(selectable_pill(
                "library-tag-manage",
                crate::i18n::s().library.manage,
                false,
                false,
                cx.listener(|this, _, _, cx| {
                    this.open_manage_tags_sheet(cx);
                }),
            ));
        }

        if self.tag_bar_mode == crate::ui_state::TagBarMode::PullDown {
            bar = bar.child(
                row()
                    .id("library-filter-drawer-close")
                    .size(px(26.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgba(card_bg()))
                    .border_1()
                    .border_color(rgba(card_border()))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(hover())))
                    .tooltip(text_tip(crate::i18n::s().library.collapse_filter_tip))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_tag_drawer_open(false, cx);
                    }))
                    .child(icon("chevron-up", 13.).text_color(rgb(muted_color()))),
            );
        }

        bar.into_any_element()
    }

    fn detail_tags_section(
        &self,
        index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(game) = self.game(index).cloned() else {
            return div().into_any_element();
        };

        if !self.detail_tags_expanded {
            let mut summary = row()
                .flex_1()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .gap(px(6.));
            if game.tags.is_empty() {
                summary = summary.child(muted(crate::i18n::s().library.no_tags).text_size(px(12.)));
            } else {
                const PREVIEW_LIMIT: usize = 4;
                for (ti, tag) in game.tags.iter().take(PREVIEW_LIMIT).enumerate() {
                    let tag_filter = tag.clone();
                    summary = summary.child(
                        div()
                            .id(("detail-tag-summary-chip", ti))
                            .h(px(22.))
                            .px(px(8.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .bg(rgba(accent_subtle_bg()))
                            .border_1()
                            .border_color(rgba(accent_subtle_border()))
                            .text_size(px(11.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(accent_fg()))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(accent_subtle_hover())))
                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                this.click_tag_filter(
                                    &tag_filter,
                                    event.modifiers().platform || event.modifiers().shift,
                                    cx,
                                );
                                this.close_detail(window, cx);
                                cx.stop_propagation();
                            }))
                            .child(tag.clone()),
                    );
                }
                if game.tags.len() > PREVIEW_LIMIT {
                    summary = summary.child(
                        muted(format!("+{}", game.tags.len() - PREVIEW_LIMIT)).text_size(px(11.)),
                    );
                }
            }

            return row()
                .id("detail-tags-collapsed")
                .flex_shrink_0()
                .items_center()
                .gap(px(10.))
                .px(px(16.))
                .py(px(11.))
                .rounded(px(12.))
                .bg(rgba(card_bg()))
                .border_1()
                .border_color(rgba(card_border()))
                .cursor_pointer()
                .hover(|s| s.bg(rgba(hover())))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.detail_tags_expanded = true;
                    cx.notify();
                }))
                .child(
                    row()
                        .flex_shrink_0()
                        .items_center()
                        .gap(px(6.))
                        .child(icon("tag", 14.).text_color(rgb(accent_fg())))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::i18n::s().library.tab_tags)),
                )
                .child(summary)
                .child(
                    row()
                        .flex_shrink_0()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            muted(if game.tags.is_empty() { crate::i18n::s().common.add } else { crate::i18n::s().common.edit })
                                .text_size(px(11.5)),
                        )
                        .child(icon("chevron-right", 14.).text_color(rgb(muted_color()))),
                )
                .into_any_element();
        }

        let library_tags: Vec<(String, usize)> =
            crate::library_view::collect_tag_counts(self.games(), true)
                .into_iter()
                .filter(|(tag, _)| !game.has_tag(tag))
                .take(12)
                .collect();

        let mut tags_row = row().flex_wrap().items_center().gap(px(6.));
        if game.tags.is_empty() {
            tags_row = tags_row.child(muted(crate::i18n::s().library.no_tags).text_size(px(12.)));
        } else {
            for (ti, tag) in game.tags.iter().enumerate() {
                let tag_filter = tag.clone();
                let tag_remove = tag.clone();
                tags_row = tags_row.child(
                    row()
                        .id(("detail-tag-chip", ti))
                        .items_center()
                        .gap(px(5.))
                        .h(px(26.))
                        .pl(px(10.))
                        .pr(px(6.))
                        .rounded_full()
                        .bg(rgba(accent_subtle_bg()))
                        .border_1()
                        .border_color(rgba(accent_subtle_border()))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(accent_subtle_hover())))
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            this.click_tag_filter(
                                &tag_filter,
                                event.modifiers().platform || event.modifiers().shift,
                                cx,
                            );
                            this.close_detail(window, cx);
                        }))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(accent_fg()))
                                .child(tag.clone()),
                        )
                        .child(
                            div()
                                .id(("detail-tag-remove", ti))
                                .size(px(16.))
                                .rounded_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(rgba(hover_strong())))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_game_tag(index, &tag_remove, cx);
                                    cx.stop_propagation();
                                }))
                                .child(icon("x", 10.).text_color(rgb(accent_fg()))),
                        ),
                );
            }
        }

        col()
            .flex_shrink_0()
            .gap(px(12.))
            .p(px(16.))
            .rounded(px(12.))
            .bg(rgba(card_bg()))
            .border_1()
            .border_color(rgba(card_border()))
            .child(
                row()
                    .id("detail-tags-header")
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.detail_tags_expanded = false;
                        cx.notify();
                    }))
                    .child(
                        row()
                            .items_center()
                            .gap(px(6.))
                            .child(icon("tag", 14.).text_color(rgb(accent_fg())))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(crate::i18n::s().library.tab_tags))
                            .child(icon("chevron-down", 14.).text_color(rgb(muted_color()))),
                    )
                    .child(
                        row()
                            .items_center()
                            .gap(px(8.))
                            .child(muted(crate::i18n::s().library.tag_quick_hint).text_size(px(11.)))
                            .child(
                                Button::new("detail-manage-tags")
                                    .label(crate::i18n::s().library.manage_all_ellipsis)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_manage_tags_sheet(cx);
                                        cx.stop_propagation();
                                    })),
                            ),
                    ),
            )
            .child(tags_row)
            .child(
                row()
                    .gap(px(8.))
                    .items_center()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.tag_input)))
                    .child(
                        Button::new("detail-add-tag-btn")
                            .label(crate::i18n::s().common.add)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.submit_tag_input(Some(window), cx);
                            })),
                    ),
            )
            .when(!library_tags.is_empty(), |d| {
                let mut quick = row().flex_wrap().items_center().gap(px(6.));
                quick = quick.child(muted(crate::i18n::s().library.quick_add_prefix).text_size(px(11.)));
                for (qi, (tag, _)) in library_tags.into_iter().enumerate() {
                    let tag_add = tag.clone();
                    quick = quick.child(tag_chip(
                        ("detail-quick-tag", qi),
                        format!("+ {tag}"),
                        false,
                        None,
                        cx.listener(move |this, _, _, cx| {
                            this.toggle_game_tag(index, &tag_add, cx);
                        }),
                    ));
                }
                d.child(quick)
            })
            .into_any_element()
    }
}
