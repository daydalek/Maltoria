use gpui_kit::component::{
    button::Button,
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{prelude::*, *};

use crate::controls::*;
use crate::preferences::{picker_mode, visible_modes};
use crate::state::WhiskyApp;
use crate::theme::*;

fn preview_fill(selected: bool) -> Rgba {
    rgba(if selected {
        ACCENT << 8 | 0x66
    } else {
        muted_color() << 8 | 0x38
    })
}

fn preview_accent(selected: bool) -> Rgba {
    rgba(if selected {
        ACCENT << 8 | 0xcc
    } else {
        muted_color() << 8 | 0x55
    })
}

fn layout_preview(mode: usize, selected: bool) -> AnyElement {
    let fill = preview_fill(selected);
    let accent = preview_accent(selected);
    let content = match mode {
        1 => col()
            .size_full()
            .justify_center()
            .gap(px(6.))
            .children((0..3).map(|_| {
                row()
                    .w_full()
                    .flex_shrink_0()
                    .gap(px(6.))
                    .child(div().w(px(14.)).h(px(18.)).rounded(px(3.)).bg(fill))
                    .child(
                        col()
                            .flex_1()
                            .justify_center()
                            .gap(px(3.))
                            .child(div().h(px(4.)).w(relative(0.7)).rounded_full().bg(accent))
                            .child(div().h(px(3.)).w(relative(0.45)).rounded_full().bg(fill)),
                    )
                    .into_any_element()
            }))
            .into_any_element(),
        3 => col()
            .size_full()
            .gap(px(6.))
            .child(div().w_full().flex_1().min_h_0().rounded(px(5.)).bg(accent))
            .child(
                row()
                    .w_full()
                    .h(px(18.))
                    .flex_shrink_0()
                    .gap(px(5.))
                    .children((0..3).map(|_| {
                        div()
                            .flex_1()
                            .h_full()
                            .rounded(px(3.))
                            .bg(fill)
                            .into_any_element()
                    })),
            )
            .into_any_element(),
        4 => row()
            .size_full()
            .gap(px(6.))
            .child(
                col()
                    .w(relative(0.44))
                    .h_full()
                    .flex_shrink_0()
                    .gap(px(4.))
                    .children((0..2).map(|_| {
                        row()
                            .w_full()
                            .flex_1()
                            .min_h_0()
                            .gap(px(4.))
                            .children((0..2).map(|_| {
                                div()
                                    .flex_1()
                                    .h_full()
                                    .rounded(px(3.))
                                    .bg(fill)
                                    .into_any_element()
                            }))
                            .into_any_element()
                    })),
            )
            .child(
                col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap(px(4.))
                    .child(div().w_full().flex_1().min_h_0().rounded(px(3.)).bg(fill))
                    .child(div().h(px(4.)).w(relative(0.8)).rounded_full().bg(accent))
                    .child(div().h(px(4.)).w(relative(0.55)).rounded_full().bg(fill)),
            )
            .into_any_element(),
        5 => div()
            .relative()
            .size_full()
            .child(
                div()
                    .absolute()
                    .left(relative(0.5))
                    .top(px(-4.))
                    .bottom(px(-4.))
                    .w(px(1.))
                    .bg(accent),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(0.5))
                    .top(relative(0.5))
                    .ml(px(-2.5))
                    .mt(px(-2.5))
                    .size(px(5.))
                    .rounded_full()
                    .bg(accent),
            )
            .child(
                row()
                    .size_full()
                    .gap(px(14.))
                    .child(
                        col()
                            .flex_1()
                            .justify_center()
                            .gap(px(5.))
                            .child(div().h(px(18.)).w_full().rounded(px(4.)).bg(fill))
                            .child(div().h(px(18.)).w_full().rounded(px(4.)).bg(fill)),
                    )
                    .child(
                        col()
                            .flex_1()
                            .justify_center()
                            .child(div().h(px(26.)).w_full().rounded(px(4.)).bg(fill)),
                    ),
            )
            .into_any_element(),
        _ => col()
            .size_full()
            .gap(px(5.))
            .children((0..2).map(|_| {
                row()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .gap(px(5.))
                    .children((0..3).map(|_| {
                        div()
                            .flex_1()
                            .h_full()
                            .rounded(px(4.))
                            .bg(fill)
                            .into_any_element()
                    }))
                    .into_any_element()
            }))
            .into_any_element(),
    };
    div()
        .w_full()
        .h(px(76.))
        .flex_shrink_0()
        .rounded(px(8.))
        .overflow_hidden()
        .p(px(8.))
        .bg(rgb(bg()))
        .child(content)
        .into_any_element()
}

impl WhiskyApp {
    pub(crate) fn library_mode_menu(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let modes = visible_modes();
        // Hidden timeline variants report their parent entry.
        let picked = picker_mode(self.mode);
        let current = modes
            .iter()
            .find(|(index, ..)| *index == picked)
            .map(|(_, _, label, _)| *label)
            .unwrap_or(modes[0].2);
        let view = cx.entity().downgrade();
        let mode = picked;
        Button::new("library-view-menu")
            .label(format!("{current}  ⌄"))
            .rounded_full()
            .h(px(34.))
            .tooltip(strings.settings.view_picker_menu_tip)
            .dropdown_menu(move |mut menu, _, _| {
                for &(index, _, label, _) in &visible_modes() {
                    let view = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).checked(mode == index).on_click(
                        move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.set_mode(index, cx);
                            });
                        },
                    ));
                }
                menu.item(PopupMenuItem::separator()).item(
                    PopupMenuItem::new(crate::i18n::s().settings.view_picker_prefs).on_click({
                        let view = view.clone();
                        move |_, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.open_settings(cx);
                            });
                        }
                    }),
                )
            })
            .into_any_element()
    }

    pub(crate) fn library_mode_cards(&self, cx: &Context<Self>) -> AnyElement {
        let strings = crate::i18n::s();
        let modes = visible_modes();
        let mut rows = col().w_full().flex_shrink_0().gap(px(12.));
        for pair in modes.chunks(2) {
            let mut strip = row().w_full().gap(px(12.));
            for &(index, name, label, description) in pair {
                let selected = picker_mode(self.mode) == index;
                strip = strip.child(
                    col()
                        .id(("view-choice", index))
                        .flex_1()
                        .min_w_0()
                        .p(px(12.))
                        .gap(px(8.))
                        .rounded(px(12.))
                        .border_2()
                        .border_color(if selected {
                            rgb(ACCENT).into()
                        } else {
                            rgba(surface_border())
                        })
                        .bg(if selected {
                            rgba(selected_bg())
                        } else {
                            rgb(crate::theme::surface()).into()
                        })
                        .cursor_pointer()
                        .hover(|s| s.bg(rgba(hover())))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_mode(index, cx);
                        }))
                        .child(layout_preview(index, selected))
                        .child(
                            row()
                                .gap(px(8.))
                                .child(icon(name, 16.).text_color(rgb(if selected {
                                    ACCENT
                                } else {
                                    muted_color()
                                })))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::BOLD)
                                        .truncate()
                                        .child(label),
                                )
                                .child(div().flex_1())
                                .when(selected, |d| {
                                    d.child(icon("check", 14.).text_color(rgb(ACCENT)))
                                }),
                        )
                        .child(muted(description).text_size(px(11.)).line_clamp(2)),
                );
            }
            rows = rows.child(strip);
        }
        col()
            .w_full()
            .flex_shrink_0()
            .py(px(10.))
            .gap(px(10.))
            .child(
                col()
                    .gap(px(3.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(strings.settings.view_picker_heading))
                    .child(muted(strings.settings.view_picker_sub)),
            )
            .child(rows)
            .into_any_element()
    }
}
