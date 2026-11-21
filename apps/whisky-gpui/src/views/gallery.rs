use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{prelude::*, *};

use crate::controls::*;
use crate::gallery::GalleryLayout;
use crate::runtime::RuntimeState;
use crate::state::{PreviewSheet, WhiskyApp};
use crate::theme::*;

/// One row of the summary's info card: a muted icon and label on the left,
/// the caller's value element pinned to the right edge.
fn info_row(icon_name: &'static str, label: &'static str, value: AnyElement) -> Div {
    row()
        .h(px(30.))
        .gap(px(8.))
        .flex_shrink_0()
        .child(icon(icon_name, 13.).text_color(rgb(muted_color())))
        .child(muted(label).text_size(px(12.)))
        .child(div().flex_1().min_w_0())
        .child(value)
}

fn info_value(value: String) -> Div {
    div()
        .flex_shrink_0()
        .max_w(px(160.))
        .truncate()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .child(value)
}

fn detail_pane(content: AnyElement) -> AnyElement {
    div()
        .w_full()
        .flex_shrink_0()
        .rounded(px(16.))
        .bg(rgb(crate::theme::surface()))
        .border_1()
        .border_color(rgba(surface_border()))
        .child(content)
        .into_any_element()
}

impl WhiskyApp {
    pub(crate) fn gallery(
        &self,
        width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let games = self.games();
        let indices = self.ordered(cx);
        let Some(selected) =
            crate::gallery::selected_index(games, &indices, self.gallery_selection.as_ref())
        else {
            return self.library_viewport(4, self.library_empty_surface(cx));
        };
        let layout = GalleryLayout::new(width);
        let wall = self.gallery_wall(&indices, selected, layout, window, cx);
        let summary =
            detail_pane(self.gallery_summary(selected, layout.detail_width, !layout.stacked, cx));
        if layout.stacked {
            self.library_viewport(
                4,
                col()
                    .w_full()
                    .flex_shrink_0()
                    .p(px(20.))
                    .gap(px(20.))
                    .child(summary)
                    .child(wall),
            )
        } else {
            row()
                .size_full()
                .min_h_0()
                .items_start()
                .p(px(20.))
                .gap(px(20.))
                .child(
                    div()
                        .w(px(layout.wall_width))
                        .h_full()
                        .min_h_0()
                        .child(self.library_viewport(4, wall)),
                )
                .child(div().w(px(layout.detail_width)).h_full().min_h_0().child(
                    self.bouncing_scroll("gallery-detail", &self.gallery_detail_scroll, summary),
                ))
                .into_any_element()
        }
    }

    fn gallery_wall(
        &self,
        indices: &[usize],
        selected: usize,
        layout: GalleryLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut parts: Vec<String> = Vec::new();
        if self.selected_bottle.is_some() {
            parts.push(self.bottle_name().to_owned());
        }
        if self.studio_filter.is_active() {
            parts.push(format!("厂商：{}", self.studio_filter.display_label()));
        }
        if self.tag_filter.is_active() {
            parts.push(self.tag_filter.display_label());
        }
        let heading: SharedString = if parts.is_empty() {
            if self.group_by_studio {
                "按厂商聚类".into()
            } else {
                "全部游戏".into()
            }
        } else {
            parts.join(" · ").into()
        };
        let mut grid = col().w_full().flex_shrink_0().gap(px(if self.group_by_studio { 18. } else { 12. }));
        if self.group_by_studio {
            for (ci, cluster) in crate::library_view::group_indices_by_studio(self.games(), indices)
                .into_iter()
                .enumerate()
            {
                let mut cluster_col = col().w_full().flex_shrink_0().gap(px(10.));
                let is_filtered = if cluster.is_unknown {
                    self.studio_filter == crate::library_view::StudioFilter::Unknown
                } else {
                    self.studio_filter.is_studio(&cluster.label)
                };
                let filter_label = if is_filtered { "全部厂商" } else { "仅看此厂商" };
                let cluster_label = cluster.label.clone();
                let is_unknown = cluster.is_unknown;
                cluster_col = cluster_col.child(
                    row()
                        .items_center()
                        .gap(px(8.))
                        .child(icon("building-2", 14.).text_color(rgb(accent_fg())))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .child(cluster.label.clone()),
                        )
                        .child(muted(format!("{} 款", cluster.indices.len())).text_size(px(11.)))
                        .child(div().flex_1())
                        .child(crate::controls::selectable_pill(
                            ("gallery-studio-filter", ci),
                            filter_label,
                            is_filtered,
                            false,
                            cx.listener(move |this, _, _, cx| {
                                if is_unknown {
                                    this.toggle_unknown_studio_filter(cx);
                                } else {
                                    this.click_studio_filter(&cluster_label, cx);
                                }
                            }),
                        )),
                );
                for strip_indices in cluster.indices.chunks(layout.columns) {
                    let mut strip = row().items_start().gap(px(12.));
                    for &index in strip_indices {
                        strip = strip.child(self.gallery_card(
                            index,
                            index == selected,
                            layout.card_width,
                            layout.card_height,
                            window,
                            cx,
                        ));
                    }
                    cluster_col = cluster_col.child(strip);
                }
                grid = grid.child(cluster_col);
            }
        } else {
            for strip_indices in indices.chunks(layout.columns) {
                let mut strip = row().items_start().gap(px(12.));
                for &index in strip_indices {
                    strip = strip.child(self.gallery_card(
                        index,
                        index == selected,
                        layout.card_width,
                        layout.card_height,
                        window,
                        cx,
                    ));
                }
                grid = grid.child(strip);
            }
        }
        col()
            .w_full()
            .flex_shrink_0()
            .gap(px(14.))
            .child(
                row()
                    .items_baseline()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(17.))
                            .font_weight(FontWeight::BOLD)
                            .child(heading),
                    )
                    .child(muted(indices.len().to_string()).text_size(px(11.))),
            )
            .child(grid)
            .into_any_element()
    }

    fn gallery_card(
        &self,
        index: usize,
        selected: bool,
        width: f32,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(game) = self.game(index) else {
            return div().into_any_element();
        };
        let title = game.title.clone();
        let artwork = self.artwork(index);
        let (ph_start, ph_end) = placeholder_gradient();
        track_key_focus(
            div()
                .id(("gallery-card", index))
                .relative()
                .w(px(width))
                .h(px(height))
                .flex_shrink_0()
                .rounded(px(10.))
                .overflow_hidden()
                .border_2()
                .border_color(if selected {
                    rgb(ACCENT).into()
                } else {
                    rgba(surface_border())
                })
                .cursor_pointer()
                .on_hover(cx.listener(move |this, active, _, cx| {
                    if *active {
                        this.hovered = Some(index);
                    } else if this.hovered == Some(index) {
                        this.hovered = None;
                    }
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_gallery_game(index, cx);
                })),
            ("gallery-card", index),
            0,
            ACCENT,
            window,
            cx,
        )
            .child(match artwork {
                Some(path) => self.cover_image(path, cx)
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .into_any_element(),
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
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(30.))
                    .bg(gradient(0x00000000, 0x000000a8)),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(5.))
                    .left(px(8.))
                    .right(px(8.))
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xf4f4f6))
                    .truncate()
                    .child(title.clone()),
            )
            .child(match self.runtime_state(index) {
                Some(RuntimeState::Running) => {
                    live_badge(self.runtime_badge_label(index, "运行中"))
                        .absolute()
                        .top(px(7.))
                        .right(px(7.))
                        .into_any_element()
                }
                Some(state) if state.is_active() => {
                    phase_badge(self.runtime_badge_label(index, state.phase_label()))
                        .absolute()
                        .top(px(7.))
                        .right(px(7.))
                        .into_any_element()
                }
                _ => div().into_any_element(),
            })
            .when(selected, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(7.))
                        .left(px(7.))
                        .size(px(18.))
                        .rounded_full()
                        .bg(rgb(ACCENT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("check", 11.).text_color(rgb(0xffffff))),
                )
            })
            .tooltip(move |_, cx| cx.new(|_| Tooltip::new(title.clone())).into())
            .context_menu(self.game_menu(index, cx))
            .into_any_element()
    }

    /// `fill` stretches the column to the pane's full height so the action
    /// block pins to the bottom; the stacked layout keeps natural height.
    fn gallery_summary(
        &self,
        index: usize,
        width: f32,
        fill: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(game) = self.game(index) else {
            return div().into_any_element();
        };
        let hero_height = ((width - 32.) * 0.60).clamp(150., 300.);
        let artwork = self.artwork(index);
        let (ph_start, ph_end) = placeholder_gradient();
        let bottle = if game.bottle > 0 {
            let name = self.bottle_name_at(game.bottle);
            if name.is_empty() {
                "未关联容器".to_owned()
            } else {
                name.to_owned()
            }
        } else {
            "未关联容器".to_owned()
        };
        let last_played =
            crate::model::format_last_played(game.last_session_ended_at.or(game.last_played))
                .unwrap_or_else(|| "尚无游玩记录".to_owned());
        let week_seconds = {
            let cutoff = chrono::Local::now().date_naive() - chrono::Duration::days(7);
            game.daily_play_time
                .iter()
                .filter(|(day, _)| {
                    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
                        .is_ok_and(|date| date > cutoff)
                })
                .map(|(_, secs)| *secs)
                .sum::<f64>() as u64
        };
        let week_play = (week_seconds > 0)
            .then(|| crate::model::format_play_time(Some(week_seconds)))
            .flatten();
        let install_dir = game.game_directory();
        let mut badges = row().flex_shrink_0().flex_wrap().gap(px(7.));
        badges = badges.child(badge("package", "Wine"));
        if let Some(state) = self.runtime_state(index) {
            if state == RuntimeState::Running {
                badges = badges.child(live_badge(self.runtime_badge_label(index, "运行中")));
            } else if state.is_active() {
                badges = badges.child(phase_badge(
                    self.runtime_badge_label(index, state.phase_label()),
                ));
            }
        }
        col()
            .w_full()
            .when(fill, |d| d.h_full())
            .when(!fill, |d| d.flex_shrink_0())
            .p(px(16.))
            .gap(px(12.))
            .child(
                div()
                    .w_full()
                    .h(px(hero_height))
                    .flex_shrink_0()
                    .rounded(px(12.))
                    .overflow_hidden()
                    .bg(rgba(card_bg()))
                    .child(match artwork {
                        Some(path) => self.cover_image(path, cx)
                            .size_full()
                            .object_fit(if game.landscape {
                                ObjectFit::Cover
                            } else {
                                ObjectFit::Contain
                            })
                            .into_any_element(),
                        None => div()
                            .size_full()
                            .bg(gradient(ph_start, ph_end))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("package", 32.).text_color(rgb(muted_color())))
                            .into_any_element(),
                    }),
            )
            .child(
                div()
                    .text_size(px(28.))
                    .line_height(relative(1.15))
                    .font_weight(FontWeight::BOLD)
                    .line_clamp(3)
                    .child(game.title.clone()),
            )
            .when(game.studio_name().is_some() || game.year().is_some(), |d| {
                let studio_opt = game.studio_name();
                let year_opt = game.year().map(str::to_owned);
                let mut meta_row = row().flex_wrap().items_center().gap(px(6.));
                if let Some(studio_name) = studio_opt {
                    let active = self.studio_filter.is_studio(&studio_name);
                    let studio_click = studio_name.clone();
                    meta_row = meta_row.child(
                        row()
                            .id(("gallery-studio-link", index))
                            .items_center()
                            .gap(px(4.))
                            .px(px(7.))
                            .py(px(2.))
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
                            .tooltip(move |_, cx| {
                                cx.new(|_| Tooltip::new("点击按此厂商筛选")).into()
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.click_studio_filter(&studio_click, cx);
                            }))
                            .child(icon("building-2", 11.).text_color(rgb(if active {
                                accent_fg()
                            } else {
                                muted_color()
                            })))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(rgb(if active {
                                        accent_fg()
                                    } else {
                                        muted_color()
                                    }))
                                    .child(studio_name),
                            ),
                    );
                }
                if let Some(year) = year_opt {
                    meta_row = meta_row.child(muted(year));
                }
                d.child(meta_row)
            })
            .child(badges)
            .when(!self.demo && (!game.tags.is_empty() || !game.is_launcher), |d| {
                let mut tag_row = row().flex_shrink_0().flex_wrap().items_center().gap(px(6.));
                for (ti, tag) in game.tags.iter().enumerate() {
                    let tag_click = tag.clone();
                    let active = self.tag_filter.contains(&tag_click);
                    tag_row = tag_row.child(crate::controls::tag_chip(
                        ("gallery-summary-tag", index * 64 + ti),
                        tag.clone(),
                        active,
                        None,
                        cx.listener(move |this, event: &ClickEvent, _, cx| {
                            this.click_tag_filter(
                                &tag_click,
                                event.modifiers().platform || event.modifiers().shift,
                                cx,
                            );
                        }),
                    ));
                }
                tag_row = tag_row.child(crate::controls::selectable_pill(
                    ("gallery-edit-tags", index),
                    if game.tags.is_empty() {
                        "+ 标签"
                    } else {
                        "编辑标签"
                    },
                    false,
                    false,
                    cx.listener(move |this, _, _, cx| {
                        this.open_game_tags_sheet(index, cx);
                    }),
                ));
                d.child(tag_row)
            })
            .when_some(game.description.clone(), |d, description| {
                d.child(
                    div()
                        .w_full()
                        .flex_shrink_0()
                        .rounded(px(10.))
                        .bg(rgba(section_bg()))
                        .border_1()
                        .border_color(rgba(section_border()))
                        .px(px(10.))
                        .py(px(8.))
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(rgb(subtext()))
                        .line_clamp(5)
                        .child(description),
                )
            })
            .child({
                // Grouped info card: icon + label on the left, value on the
                // right, hairlines between rows.
                let mut info_rows: Vec<AnyElement> = vec![
                    info_row("star", "评分", {
                        let value = game.rating.map_or_else(
                            || "未评分".to_owned(),
                            |rating| format!("{} / 10", rating.min(10)),
                        );
                        row()
                            .flex_shrink_0()
                            .items_center()
                            .gap(px(8.))
                            .child(self.rating_stars(index, 14., cx))
                            .child(muted(value).text_size(px(12.)))
                            .into_any_element()
                    })
                    .into_any_element(),
                    info_row("container", "运行环境", info_value(bottle).into_any_element())
                        .into_any_element(),
                    info_row("clock", "上次游玩", info_value(last_played).into_any_element())
                        .into_any_element(),
                ];
                if let Some(play_time) = crate::model::format_play_time(game.play_time) {
                    info_rows.push(
                        info_row("hourglass", "累计游玩", info_value(play_time).into_any_element())
                            .into_any_element(),
                    );
                }
                if let Some(play_time) = week_play {
                    info_rows.push(
                        info_row(
                            "calendar-clock",
                            "近 7 天游玩",
                            info_value(play_time).into_any_element(),
                        )
                        .into_any_element(),
                    );
                }
                if let Some(dir) = install_dir {
                    info_rows.push(
                        row()
                            .id(("gallery-install-dir", index))
                            .h(px(30.))
                            .gap(px(8.))
                            .flex_shrink_0()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(hover())))
                            .tooltip(move |_, cx| {
                                cx.new(|_| Tooltip::new("在访达中打开")).into()
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.command(
                                    crate::views::Command::OpenGameDirectory(index),
                                    cx,
                                );
                            }))
                            .child(icon("folder", 13.).text_color(rgb(muted_color())))
                            .child(muted("安装位置").text_size(px(12.)))
                            .child(div().flex_1().min_w_0())
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .max_w(px(160.))
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(rgb(muted_color()))
                                    .child(dir.to_string_lossy().to_string()),
                            )
                            .child(icon("chevron-right", 11.).text_color(rgb(muted_color())))
                            .into_any_element(),
                    );
                }
                let mut card = col()
                    .w_full()
                    .flex_shrink_0()
                    .rounded(px(10.))
                    .bg(rgba(section_bg()))
                    .border_1()
                    .border_color(rgba(section_border()))
                    .px(px(10.));
                for (row_index, info) in info_rows.into_iter().enumerate() {
                    if row_index > 0 {
                        card = card.child(div().h(px(1.)).w_full().bg(rgba(divider())));
                    }
                    card = card.child(info);
                }
                card
            })
            .when(fill, |d| {
                d.child(div().flex_1().min_h(px(8.))).child(
                    div().w_full().h(px(1.)).bg(rgba(divider())),
                )
            })
            .child(
                self.play_button(("gallery-play", index), index, "启动游戏", cx)
                    .w_full()
                    .h(px(44.)),
            )
            .child(gallery_action_pill(
                ("gallery-settings", index),
                "sliders-horizontal",
                "游戏设置",
                cx.listener(move |this, _, _, cx| {
                    this.open_preview_sheet(PreviewSheet::RunSettings(index), cx);
                }),
            ))
            .child(gallery_action_pill(
                ("gallery-translation", index),
                "languages",
                "游戏翻译与捕获",
                cx.listener(move |this, _, _, cx| {
                    this.open_translation_for_game(index, cx);
                }),
            ))
            .child(
                row()
                    .id(("gallery-details-link", index))
                    .w_full()
                    .h(px(30.))
                    .flex_shrink_0()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(hover())))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_detail_from(index, window, cx);
                    }))
                    .child(muted("查看完整详情"))
                    .key_focus(("gallery-details-link", index)),
            )
            .into_any_element()
    }
}

fn gallery_action_pill(
    id: impl Into<ElementId>,
    icon_name: &str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> FocusableControl {
    let id = id.into();
    row()
        .id(id.clone())
        .w_full()
        .h(px(44.))
        .flex_shrink_0()
        .justify_center()
        .gap(px(6.))
        .rounded_full()
        .border_1()
        .border_color(rgba(surface_border()))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(hover())))
        .on_click(on_click)
        .child(icon(icon_name, 15.))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .child(label),
        )
        .key_focus(id)
}
