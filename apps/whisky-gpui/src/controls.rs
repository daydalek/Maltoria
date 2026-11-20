//! Small shared widgets used by more than one surface.

use gpui_kit::component::Icon;
use gpui_kit::component::Sizable;
use gpui_kit::component::button::Button;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::{prelude::*, *};

use crate::theme;

/// Wraps a `Button` so it receives focus once on mount without stealing focus
/// back on subsequent re-renders, allowing keyboard Tab navigation afterward.
#[derive(gpui_kit::IntoElement)]
pub(crate) struct AutoFocusButton {
    id: ElementId,
    button: Button,
    active: bool,
}

impl AutoFocusButton {
    pub(crate) fn new(id: impl Into<ElementId>, button: Button) -> Self {
        Self {
            id: id.into(),
            button,
            active: true,
        }
    }

    pub(crate) fn when_focusable(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

impl RenderOnce for AutoFocusButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.active {
            let focused_once = window.use_keyed_state(
                (self.id.clone(), "auto-focused"),
                cx,
                |_, _| std::cell::Cell::new(false),
            );
            if !focused_once.read(cx).replace(true) {
                let button_id = self.id.clone();
                // Match `gpui-fast`'s stateless `ViewElement<Button>` ID scope so
                // `AutoFocusButton` focuses the exact `FocusHandle` created by `Button`.
                let focus_handle = window.with_id(
                    ElementId::Name(std::any::type_name::<Button>().into()),
                    |window| {
                        window
                            .use_keyed_state(button_id, cx, |_, cx| cx.focus_handle())
                            .read(cx)
                            .clone()
                    },
                );
                window.focus(&focus_handle, cx);
            }
        }
        self.button
    }
}

/// Wires a persisted `FocusHandle` and focus ring directly onto a `Stateful<Div>`,
/// preserving builder chaining (such as `.context_menu(...)`) while making the
/// element join `Tab` navigation and activate `on_click` on `Enter` / `Space`.
pub(crate) fn track_key_focus(
    el: Stateful<Div>,
    id: impl Into<ElementId>,
    tab_index: isize,
    ring_color: u32,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let focus_handle = window
        .use_keyed_state((id.into(), "focus-handle"), cx, |_, cx| {
            cx.focus_handle().tab_stop(true).tab_index(tab_index)
        })
        .read(cx)
        .clone();
    el.track_focus(&focus_handle)
        .focus(move |s| s.border_2().border_color(rgb(ring_color)))
}

/// Wraps any `Stateful<Div>` with a persisted `FocusHandle`, Tab order, and an
/// accessible focus border for keyboard navigation and `Enter`/`Space` activation.
#[derive(gpui_kit::IntoElement)]
pub(crate) struct FocusableControl {
    id: ElementId,
    inner: Stateful<Div>,
    tab_index: isize,
    tab_stop: bool,
    ring_color: u32,
    auto_focus_key: Option<ElementId>,
}

impl FocusableControl {
    pub(crate) fn new(id: impl Into<ElementId>, inner: Stateful<Div>) -> Self {
        Self {
            id: id.into(),
            inner,
            tab_index: 0,
            tab_stop: true,
            ring_color: theme::ACCENT,
            auto_focus_key: None,
        }
    }

    pub(crate) fn tab_index(mut self, tab_index: isize) -> Self {
        self.tab_index = tab_index;
        self
    }

    pub(crate) fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    /// Uses a translucent white ring suitable for solid orange primary buttons.
    pub(crate) fn light_ring(mut self) -> Self {
        self.ring_color = 0xffffff;
        self
    }

    /// Requests focus once per distinct `key` mount (e.g. a dialog or detail generation).
    pub(crate) fn auto_focus_key(mut self, key: impl Into<ElementId>) -> Self {
        self.auto_focus_key = Some(key.into());
        self
    }
}

impl Styled for FocusableControl {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl RenderOnce for FocusableControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = window
            .use_keyed_state((self.id.clone(), "focus-handle"), cx, |_, cx| {
                cx.focus_handle()
                    .tab_stop(self.tab_stop)
                    .tab_index(self.tab_index)
            })
            .read(cx)
            .clone();
        if let Some(auto_key) = self.auto_focus_key {
            let focused_once =
                window.use_keyed_state(auto_key, cx, |_, _| std::cell::Cell::new(false));
            if !focused_once.read(cx).replace(true) {
                window.focus(&focus_handle, cx);
            }
        }
        let ring_color = self.ring_color;
        self.inner
            .track_focus(&focus_handle)
            .focus(move |s| s.border_2().border_color(rgb(ring_color)))
    }
}

pub(crate) trait FocusableExt {
    fn key_focus(self, id: impl Into<ElementId>) -> FocusableControl;
}

impl FocusableExt for Stateful<Div> {
    fn key_focus(self, id: impl Into<ElementId>) -> FocusableControl {
        FocusableControl::new(id, self)
    }
}

pub(crate) fn row() -> Div {
    div().flex().items_center()
}
pub(crate) fn col() -> Div {
    div().flex().flex_col()
}
pub(crate) fn icon(name: &str, size: f32) -> Icon {
    Icon::default()
        .path(format!("icons/{name}.svg"))
        .size(px(size))
        .flex_shrink_0()
}
pub(crate) fn muted(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(theme::muted_color()))
        .child(text.into())
}
pub(crate) fn gradient(top: u32, bottom: u32) -> Background {
    linear_gradient(
        180.,
        linear_color_stop(rgba(top), 0.),
        linear_color_stop(rgba(bottom), 1.),
    )
}
pub(crate) fn badge(name: &str, text: impl Into<SharedString>) -> Div {
    row()
        .gap(px(5.))
        .px(px(9.))
        .py(px(3.))
        .rounded_full()
        .bg(rgba(theme::badge_bg()))
        .text_size(px(11.))
        .text_color(rgb(theme::badge_text()))
        .child(icon(name, 12.))
        .child(text.into())
}
pub(crate) fn round_button(id: &'static str, name: &str) -> Stateful<Div> {
    row()
        .id(id)
        .justify_center()
        .size(px(34.))
        .rounded_full()
        .bg(rgba(theme::round_btn_bg()))
        .border_1()
        .border_color(rgba(theme::round_btn_border()))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(theme::round_btn_hover())))
        .child(icon(name, 17.))
}

/// Running indicator: the status pill's live dot inside the existing badge
/// capsule. No new art — the dot is the bridge pill's, the capsule is `badge`.
pub(crate) fn live_badge(text: impl Into<SharedString>) -> Div {
    row()
        .flex_shrink_0()
        .gap(px(5.))
        .px(px(8.))
        .py(px(3.))
        .rounded_full()
        .bg(rgba(theme::art_chip_bg()))
        .child(div().size(px(6.)).rounded_full().bg(rgb(theme::live_dot())))
        .child(
            div()
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::live_dot()))
                .child(text.into()),
        )
}

/// Pre-launch phase: the toolkit spinner plus the phase name, same capsule.
pub(crate) fn phase_badge(label: impl Into<SharedString>) -> Div {
    row()
        .flex_shrink_0()
        .gap(px(5.))
        .px(px(8.))
        .py(px(3.))
        .rounded_full()
        .bg(rgba(theme::art_chip_bg()))
        .child(Spinner::new().with_size(px(10.)))
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(theme::text()))
                .child(label.into()),
        )
}

pub(crate) fn selectable_pill(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    disabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let id = id.into();
    let pill = row()
        .id(id.clone())
        .gap(px(5.))
        .px(px(10.))
        .py(px(5.))
        .rounded(px(7.))
        .bg(rgba(if active {
            theme::accent_subtle_bg()
        } else {
            theme::card_bg()
        }))
        .border_1()
        .border_color(rgba(if active {
            theme::accent_subtle_border()
        } else {
            theme::card_border()
        }))
        .when(active, |d| {
            d.child(icon("check", 12.).text_color(rgb(theme::accent_fg())))
        })
        .child(
            div()
                .text_size(px(12.))
                .font_weight(if active {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(if active {
                    theme::accent_fg()
                } else {
                    theme::text()
                }))
                .child(label.into()),
        );
    if disabled {
        pill.opacity(0.5).into_any_element()
    } else {
        pill.cursor_pointer()
            .hover(|s| if active { s } else { s.bg(rgba(theme::hover())) })
            .on_click(on_click)
            .key_focus(id)
            .into_any_element()
    }
}
