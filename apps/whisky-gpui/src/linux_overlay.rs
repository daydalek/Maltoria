use std::time::{Duration, Instant};

use gpui_kit::{prelude::*, *};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde_json::json;
use x11rb::connection::Connection;
use x11rb::protocol::shape;
use x11rb::protocol::shape::ConnectionExt as _;
use x11rb::protocol::xproto::{
    ClipOrdering, ConfigureWindowAux, ConnectionExt as _, KeyButMask, StackMode,
};
use x11rb::rust_connection::RustConnection;

use crate::capture::{OverlaySettings, Subtitle};
use crate::state::WhiskyApp;

pub(crate) fn start(app: &Entity<WhiskyApp>, cx: &mut App) {
    app.update(cx, |_, cx| {
        cx.spawn(async move |app, cx| {
            let mut overlay = OverlayController::default();
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                if app.update(cx, |app, cx| overlay.sync(app, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    });
}

#[derive(Default)]
struct OverlayController {
    window: Option<WindowHandle<SubtitleWindow>>,
    last_poll: Option<Instant>,
    geometry: Option<(OverlaySettings, Instant)>,
    failed: bool,
    #[cfg(debug_assertions)]
    fixture_started: bool,
    #[cfg(debug_assertions)]
    fixture_settings: Option<String>,
}

impl OverlayController {
    fn sync(&mut self, app: &mut WhiskyApp, cx: &mut Context<WhiskyApp>) {
        if !app.service_available("translation.overlay.read") {
            self.close(cx);
            return;
        }
        let configuring = app.capture.is_pending("translation.overlay.configure");
        let polling = app.capture.overlay.as_ref().is_none_or(|s| s.visible)
            || app.capture.active_game.is_some();
        if polling
            && self.geometry.is_none()
            && !configuring
            && !app.capture.is_pending("translation.overlay.read")
            && self
                .last_poll
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(500))
        {
            self.last_poll = Some(Instant::now());
            let _ = app
                .capture
                .request(&app.bridge, "translation.overlay.read", json!({}));
        }
        #[cfg(debug_assertions)]
        if !self.fixture_started
            && app.capture.overlay.is_some()
            && std::env::var_os("WHISKY_OVERLAY_TEST_SUBTITLE").is_some()
        {
            self.fixture_started = true;
            let _ = app.capture.request(
                &app.bridge,
                "translation.overlay.configure",
                json!({"visible":true}),
            );
        }
        #[cfg(debug_assertions)]
        if app.capture.overlay.is_some() && !configuring {
            if let Some(contents) = std::env::var_os("WHISKY_OVERLAY_TEST_SETTINGS")
                .and_then(|path| std::fs::read_to_string(path).ok())
            {
                if self.fixture_settings.as_ref() != Some(&contents) {
                    if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&contents) {
                        let _ = app.capture.request(
                            &app.bridge,
                            "translation.overlay.configure",
                            settings,
                        );
                        self.fixture_settings = Some(contents);
                    }
                }
            }
        }
        let Some(mut settings) = app.capture.overlay.clone() else {
            return;
        };
        if let Some((geometry, changed)) = &self.geometry {
            copy_geometry(&mut settings, geometry);
            if changed.elapsed() >= Duration::from_millis(300) && !configuring {
                let _ = app.capture.request(&app.bridge, "translation.overlay.configure", json!({
                    "x":settings.x, "y":settings.y, "width":settings.width, "height":settings.height
                }));
                self.geometry = None;
            }
        }
        if !settings.visible {
            self.close(cx);
            self.failed = false;
            return;
        }
        let mut subtitle = app
            .capture
            .snapshot
            .as_ref()
            .map(|s| s.subtitle.clone())
            .unwrap_or_default();
        #[cfg(debug_assertions)]
        if let Ok(source) = std::env::var("WHISKY_OVERLAY_TEST_SUBTITLE") {
            subtitle.source = source;
            subtitle.translation = "A subtitle above the game — translation ready.".into();
            subtitle.status = "completed".into();
        }
        if self.window.is_none() && !self.failed {
            let screen = cx
                .primary_display()
                .map(|d| d.bounds())
                .unwrap_or(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1440.), px(900.)),
                ));
            let bounds = initial_bounds(&settings, screen);
            let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
            let options = |kind| WindowOptions {
                kind,
                focus: false,
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Transparent,
                app_id: Some("com.daydalek.Maltoria.subtitles".into()),
                window_min_size: Some(size(px(320.), px(100.))),
                ..Default::default()
            };
            let kind = if wayland {
                WindowKind::LayerShell(layer_shell::LayerShellOptions {
                    namespace: "maltoria-subtitles".into(),
                    layer: layer_shell::Layer::Overlay,
                    anchor: layer_shell::Anchor::BOTTOM,
                    margin: Some((px(0.), px(0.), px(40.), px(0.))),
                    keyboard_interactivity: layer_shell::KeyboardInteractivity::None,
                    ..Default::default()
                })
            } else {
                WindowKind::PopUp
            };
            let make = |window: &mut Window, cx: &mut App| {
                window.set_window_title("Maltoria subtitles");
                cx.new(|_| SubtitleWindow::new(settings.clone(), subtitle.clone(), bounds, window))
            };
            let opened = cx.open_window(options(kind), make);
            let opened = if wayland && opened.is_err() {
                cx.open_window(options(WindowKind::Floating), make)
            } else {
                opened
            };
            match opened {
                Ok(handle) => self.window = Some(handle),
                Err(error) => {
                    app.capture.error = Some(format!("Could not open subtitle overlay: {error}"));
                    self.failed = true;
                }
            }
        }
        if let Some(handle) = self.window {
            match handle.update(cx, |view, window, cx| {
                view.sync(settings, subtitle, window, cx)
            }) {
                Ok(Some(geometry)) => {
                    if let Some(settings) = &mut app.capture.overlay {
                        copy_geometry(settings, &geometry);
                    }
                    self.geometry = Some((geometry, Instant::now()));
                }
                Err(_) => self.window = None,
                _ => {}
            }
        }
    }

    fn close(&mut self, cx: &mut App) {
        if let Some(window) = self.window.take() {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        }
    }
}

fn copy_geometry(to: &mut OverlaySettings, from: &OverlaySettings) {
    to.x = from.x;
    to.y = from.y;
    to.width = from.width;
    to.height = from.height;
}

fn initial_bounds(settings: &OverlaySettings, screen: Bounds<Pixels>) -> Bounds<Pixels> {
    let width = settings.width.min(f32::from(screen.size.width) as f64) as f32;
    let height = settings.height.min(f32::from(screen.size.height) as f64) as f32;
    let left = f32::from(screen.origin.x);
    let top = f32::from(screen.origin.y);
    let right = left + f32::from(screen.size.width);
    let bottom = top + f32::from(screen.size.height);
    let x = (settings
        .x
        .map(|x| x as f32)
        .unwrap_or((left + right - width) / 2.))
    .clamp(left, right - width);
    let y = (settings
        .y
        .map(|y| y as f32)
        .unwrap_or(bottom - height - 40.))
    .clamp(top, bottom - height);
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn status_text(subtitle: &Subtitle) -> String {
    match subtitle.status.as_str() {
        "pending" => "Translating…".into(),
        "failed" => subtitle
            .error
            .clone()
            .unwrap_or_else(|| "Translation failed".into()),
        "cancelled" => "Translation cancelled".into(),
        _ => String::new(),
    }
}

struct SubtitleWindow {
    settings: OverlaySettings,
    subtitle: Subtitle,
    x11: Option<X11Overlay>,
}

impl SubtitleWindow {
    fn new(
        settings: OverlaySettings,
        subtitle: Subtitle,
        bounds: Bounds<Pixels>,
        window: &Window,
    ) -> Self {
        let x11 = X11Overlay::new(window, bounds);
        Self {
            settings,
            subtitle,
            x11,
        }
    }

    fn sync(
        &mut self,
        settings: OverlaySettings,
        subtitle: Subtitle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<OverlaySettings> {
        let changed = self.settings != settings || self.subtitle != subtitle;
        if self.x11.is_none()
            && (self.settings.width != settings.width || self.settings.height != settings.height)
        {
            window.resize(size(px(settings.width as f32), px(settings.height as f32)));
        }
        let geometry = self
            .x11
            .as_mut()
            .and_then(|x| x.sync(&settings, &self.settings, changed));
        self.settings = settings;
        if let Some(geometry) = &geometry {
            copy_geometry(&mut self.settings, geometry);
        }
        self.subtitle = subtitle;
        if changed || geometry.is_some() {
            cx.notify();
        }
        geometry
    }
}

impl Render for SubtitleWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let settings = &self.settings;
        let opacity = if settings.click_through {
            settings.opacity
        } else {
            settings.opacity.max(0.02)
        };
        let color = |hex: &str| {
            rgb(u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0xFFFFFF))
        };
        div()
            .size_full()
            .overflow_hidden()
            .rounded(px(12.))
            .bg(rgba((opacity * 255.).round() as u32))
            .when(!settings.click_through, |d| {
                d.border_1().border_color(rgba(0xFFFFFF59))
            })
            .px(px(18.))
            .py(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .font_family("Noto Sans CJK SC")
            .when(settings.show_source, |d| {
                d.child(
                    div()
                        .text_size(px((settings.font_size * 0.75).max(12.) as f32))
                        .text_color(color(&settings.source_text_color))
                        .child(self.subtitle.source.chars().take(4096).collect::<String>()),
                )
            })
            .when(
                settings.show_translation && !self.subtitle.translation.is_empty(),
                |d| {
                    d.child(
                        div()
                            .text_size(px(settings.font_size as f32))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(color(&settings.text_color))
                            .child(
                                self.subtitle
                                    .translation
                                    .chars()
                                    .take(4096)
                                    .collect::<String>(),
                            ),
                    )
                },
            )
            .when(!status_text(&self.subtitle).is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(13.))
                        .text_color(rgb(0xAAAAAA))
                        .child(status_text(&self.subtitle)),
                )
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy)]
struct Gesture {
    pointer: (i32, i32),
    frame: Frame,
    edges: u8,
}

fn moved_frame(gesture: Gesture, pointer: (i32, i32), scale: f32) -> Frame {
    let (dx, dy) = (pointer.0 - gesture.pointer.0, pointer.1 - gesture.pointer.1);
    let mut frame = gesture.frame;
    if gesture.edges == 0 {
        frame.x += dx;
        frame.y += dy;
        return frame;
    }
    if gesture.edges & 3 != 0 {
        let delta = if gesture.edges & 1 != 0 { -dx } else { dx };
        frame.width = (frame.width as i32 + delta)
            .clamp((320. * scale) as i32, (3840. * scale) as i32) as u32;
        if gesture.edges & 1 != 0 {
            frame.x += gesture.frame.width as i32 - frame.width as i32;
        }
    }
    if gesture.edges & 12 != 0 {
        let delta = if gesture.edges & 4 != 0 { -dy } else { dy };
        frame.height = (frame.height as i32 + delta)
            .clamp((100. * scale) as i32, (1200. * scale) as i32) as u32;
        if gesture.edges & 4 != 0 {
            frame.y += gesture.frame.height as i32 - frame.height as i32;
        }
    }
    frame
}

struct X11Overlay {
    connection: RustConnection,
    id: u32,
    root: u32,
    scale: f32,
    gesture: Option<Gesture>,
    was_down: bool,
    raised: Instant,
    shaped: Option<bool>,
}

impl X11Overlay {
    fn new(window: &Window, bounds: Bounds<Pixels>) -> Option<Self> {
        let raw = HasWindowHandle::window_handle(window).ok()?.as_raw();
        let id = match raw {
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            RawWindowHandle::Xlib(handle) => handle.window as u32,
            _ => return None,
        };
        let (connection, screen) = x11rb::connect(None).ok()?;
        let root = connection.setup().roots[screen].root;
        let scale = window.scale_factor();
        connection
            .configure_window(
                id,
                &ConfigureWindowAux::new()
                    .x((f32::from(bounds.origin.x) * scale) as i32)
                    .y((f32::from(bounds.origin.y) * scale) as i32),
            )
            .ok()?
            .check()
            .ok()?;
        Some(Self {
            connection,
            id,
            root,
            scale,
            gesture: None,
            was_down: false,
            raised: Instant::now(),
            shaped: None,
        })
    }

    fn sync(
        &mut self,
        settings: &OverlaySettings,
        previous: &OverlaySettings,
        changed: bool,
    ) -> Option<OverlaySettings> {
        let c = &self.connection;
        if self.shaped != Some(settings.click_through) {
            let result = if settings.click_through {
                c.shape_rectangles(
                    shape::SO::SET,
                    shape::SK::INPUT,
                    ClipOrdering::UNSORTED,
                    self.id,
                    0,
                    0,
                    &[],
                )
            } else {
                c.shape_mask(shape::SO::SET, shape::SK::INPUT, self.id, 0, 0, x11rb::NONE)
            };
            match result
                .map_err(|e| e.to_string())
                .and_then(|cookie| cookie.check().map_err(|e| e.to_string()))
            {
                Ok(_) => self.shaped = Some(settings.click_through),
                Err(error) => eprintln!("Subtitle overlay input shape failed: {error}"),
            }
        }
        if self.gesture.is_none()
            && (settings.x != previous.x
                || settings.y != previous.y
                || settings.width != previous.width
                || settings.height != previous.height)
        {
            let mut config = ConfigureWindowAux::new()
                .width((settings.width as f32 * self.scale) as u32)
                .height((settings.height as f32 * self.scale) as u32);
            if let Some(x) = settings.x {
                config = config.x((x as f32 * self.scale) as i32);
            }
            if let Some(y) = settings.y {
                config = config.y((y as f32 * self.scale) as i32);
            }
            let _ = c.configure_window(self.id, &config);
        }
        if changed || self.raised.elapsed() >= Duration::from_millis(500) {
            let _ = c.configure_window(
                self.id,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            );
            self.raised = Instant::now();
        }
        let pointer = c.query_pointer(self.root).ok()?.reply().ok()?;
        let down = pointer.mask.contains(KeyButMask::BUTTON1);
        let position = (pointer.root_x as i32, pointer.root_y as i32);
        if !settings.click_through && down && !self.was_down && pointer.child == self.id {
            let geometry = c.get_geometry(self.id).ok()?.reply().ok()?;
            let origin = c
                .translate_coordinates(self.id, self.root, 0, 0)
                .ok()?
                .reply()
                .ok()?;
            let frame = Frame {
                x: origin.dst_x as i32,
                y: origin.dst_y as i32,
                width: geometry.width as u32,
                height: geometry.height as u32,
            };
            let (x, y) = (position.0 - frame.x, position.1 - frame.y);
            let edge = (14. * self.scale) as i32;
            let edges = u8::from(x < edge)
                | (u8::from(x >= frame.width as i32 - edge) << 1)
                | (u8::from(y < edge) << 2)
                | (u8::from(y >= frame.height as i32 - edge) << 3);
            self.gesture = Some(Gesture {
                pointer: position,
                frame,
                edges,
            });
        }
        self.was_down = down;
        let mut result = None;
        if let Some(gesture) = self.gesture {
            if down && !settings.click_through {
                let frame = moved_frame(gesture, position, self.scale);
                let _ = c.configure_window(
                    self.id,
                    &ConfigureWindowAux::new()
                        .x(frame.x)
                        .y(frame.y)
                        .width(frame.width)
                        .height(frame.height),
                );
                let mut geometry = settings.clone();
                geometry.x = Some((frame.x as f32 / self.scale) as f64);
                geometry.y = Some((frame.y as f32 / self.scale) as f64);
                geometry.width = (frame.width as f32 / self.scale) as f64;
                geometry.height = (frame.height as f32 / self.scale) as f64;
                result = Some(geometry);
            } else {
                self.gesture = None;
            }
        }
        let _ = c.flush();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, Gesture, initial_bounds, moved_frame, status_text};
    use crate::capture::{OverlaySettings, Subtitle};
    use gpui_kit::{Bounds, point, px, size};

    #[test]
    fn macos_status_rules_and_empty_text() {
        let mut subtitle = Subtitle::default();
        assert_eq!(status_text(&subtitle), "");
        subtitle.status = "pending".into();
        assert_eq!(status_text(&subtitle), "Translating…");
        subtitle.status = "failed".into();
        assert_eq!(status_text(&subtitle), "Translation failed");
        subtitle.error = Some("Provider unavailable".into());
        assert_eq!(status_text(&subtitle), "Provider unavailable");
        subtitle.status = "cancelled".into();
        assert_eq!(status_text(&subtitle), "Translation cancelled");
    }

    #[test]
    fn default_position_and_disconnected_display_recovery() {
        let screen = Bounds::new(point(px(0.), px(0.)), size(px(1440.), px(900.)));
        let mut settings = OverlaySettings::default();
        let bounds = initial_bounds(&settings, screen);
        assert_eq!(bounds.origin, point(px(270.), px(680.)));
        settings.x = Some(9000.);
        settings.y = Some(-2000.);
        let bounds = initial_bounds(&settings, screen);
        assert_eq!(bounds.origin, point(px(540.), px(0.)));
    }

    #[test]
    fn drag_and_resize_edges_respect_limits() {
        let gesture = Gesture {
            pointer: (100, 100),
            frame: Frame {
                x: 40,
                y: 50,
                width: 900,
                height: 180,
            },
            edges: 0,
        };
        assert_eq!(
            moved_frame(gesture, (120, 130), 1.),
            Frame {
                x: 60,
                y: 80,
                width: 900,
                height: 180
            }
        );
        let resized = moved_frame(
            Gesture {
                edges: 3 | 12,
                ..gesture
            },
            (2000, 2000),
            1.,
        );
        assert_eq!(resized.width, 320);
        assert_eq!(resized.height, 100);
    }
}
