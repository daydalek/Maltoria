//! Windows floating subtitle overlay: an always-on-top borderless window
//! mirroring the live subtitle entries from the capture pipeline. Position
//! and styling come from the overlay settings the user edits in the capture
//! page (shared OverlaySettings shape across platforms).

#![cfg(target_os = "windows")]

use std::time::Duration;

use gpui_kit::{prelude::*, *};

use crate::capture::{OverlaySettings, SubtitleEntry};
use crate::state::WhiskyApp;

pub(crate) fn start(app: &Entity<WhiskyApp>, cx: &mut App) {
    app.update(cx, |_, cx| {
        cx.spawn(async move |app, cx| {
            let mut controller = OverlayController::default();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                if app.update(cx, |app, cx| controller.sync(app, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    });
}

#[derive(Default)]
struct OverlayController {
    window: Option<WindowHandle<OverlayView>>,
    initialized_style: bool,
}

impl OverlayController {
    fn sync(&mut self, app: &mut WhiskyApp, cx: &mut Context<WhiskyApp>) {
        if !app.service_available("translation.overlay.read") {
            self.close(cx);
            return;
        }
        let Some(settings) = app.capture.overlay.clone() else {
            return;
        };
        let entries: Vec<SubtitleEntry> = app
            .capture
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.subtitle.entries.clone())
            .unwrap_or_default();
        let capturing = app.capture.active_game.is_some()
            || app.capture.snapshot.as_ref().is_some_and(|snapshot| {
                matches!(
                    snapshot.capture_state.as_str(),
                    "selecting" | "attaching" | "attached" | "connecting"
                )
            });
        if !settings.visible || !capturing {
            self.close(cx);
            return;
        }
        if self.window.is_none() {
            let bounds = Bounds {
                origin: point(
                    px(settings.x.unwrap_or(60.0) as f32),
                    px(settings.y.unwrap_or(40.0) as f32),
                ),
                size: size(px(settings.width as f32), px(settings.height as f32)),
            };
            let make = |window: &mut Window, cx: &mut App| {
                window.set_window_title(crate::i18n::s().capture.overlay_generic_title);
                cx.new(|_| OverlayView {
                    entries: Vec::new(),
                    settings: settings.clone(),
                })
            };
            let result = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    window_min_size: Some(size(px(320.0), px(80.0))),
                    kind: WindowKind::Floating,
                    focus: false,
                    ..Default::default()
                },
                make,
            );
            match result {
                Ok(handle) => {
                    self.window = Some(handle);
                    self.initialized_style = false;
                }
                Err(_) => {
                    self.window = None;
                    return;
                }
            }
        }
        if let Some(handle) = self.window {
            if !self.initialized_style {
                self.initialized_style = true;
                let click_through = settings.click_through;
                let _ = handle.update(cx, move |_, window, _| {
                    apply_win32_style(window, click_through);
                });
            }
            let entries = entries
                .iter()
                .rev()
                .take(settings.max_entries as usize)
                .rev()
                .cloned()
                .collect::<Vec<_>>();
            let settings = settings.clone();
            let _ = handle.update(cx, move |view, _, cx| {
                if view.entries != entries || view.settings != settings {
                    view.entries = entries;
                    view.settings = settings;
                    cx.notify();
                }
            });
        }
    }

    fn close(&mut self, cx: &mut Context<WhiskyApp>) {
        if let Some(handle) = self.window.take() {
            self.initialized_style = false;
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }
}

fn apply_win32_style(window: &Window, click_through: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };
    let raw = HasWindowHandle::window_handle(window).ok().map(|handle| handle.as_raw());
    let Some(RawWindowHandle::Win32(win32)) = raw else {
        return;
    };
    let hwnd = win32.hwnd.get() as HWND;
    unsafe {
        let mut style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        style |= WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED;
        if click_through {
            style |= WS_EX_TRANSPARENT;
        } else {
            style &= !WS_EX_TRANSPARENT;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style as isize);
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub(crate) struct OverlayView {
    entries: Vec<SubtitleEntry>,
    settings: OverlaySettings,
}

fn hex_color(value: &str, fallback: u32) -> Hsla {
    let bytes = value.as_bytes();
    if bytes.len() == 7 && bytes[0] == b'#' {
        let parse = |pair: &[u8]| -> f32 {
            let text = std::str::from_utf8(pair).unwrap_or("00");
            u8::from_str_radix(text, 16).unwrap_or(0) as f32 / 255.0
        };
        rgb((parse(&bytes[1..3]) * 255.0) as u32 * 65536 + (parse(&bytes[3..5]) * 255.0) as u32 * 256 + (parse(&bytes[5..7]) * 255.0) as u32).into()
    } else {
        rgb(fallback).into()
    }
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let settings = &self.settings;
        let background_alpha = (settings.opacity as f32).clamp(0.1, 1.0);
        let translation_color = hex_color(&settings.text_color, 0xffffff);
        let source_color = hex_color(&settings.source_text_color, 0xa0a0a0);
        let font = px(settings.font_size as f32);
        let mut column = div()
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .gap_2()
            .p_4()
            .bg(hsla(0.0, 0.0, 0.0, background_alpha))
            .text_color(translation_color);
        for entry in &self.entries {
            let mut block = div().flex().flex_col().gap_1();
            if settings.show_source && !entry.source.is_empty() {
                block = block.child(
                    div()
                        .text_size(font * 0.6)
                        .text_color(source_color)
                        .child(entry.source.clone()),
                );
            }
            if settings.show_translation {
                let translated = if entry.translation.is_empty() {
                    entry.source.clone()
                } else {
                    entry.translation.clone()
                };
                block = block.child(div().text_size(font).child(translated));
            }
            column = column.child(block);
        }
        column
    }
}
