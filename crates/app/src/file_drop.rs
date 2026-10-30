//! Audio files dragged in from Finder, Explorer or a Linux file manager.
//!
//! winit says that files are hovering over the window and then dropped, one
//! event per file, but not where, and Slint doesn't hand them to its drop
//! areas. So the cursor is asked for directly while they hover: the deck
//! under it lights up (`TrackDrag.files-over`), and on the drop that deck
//! asks for the files with `TrackDrag.drop-files`, which loads them like a
//! track dragged from the browser.
//!
//! Wayland sessions don't get file drops from winit yet; X11 ones do.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, DataTransfer, Timer, TimerMode};

use crate::{AppWindow, TrackDrag};

/// How often the cursor is looked up while files hover over the window.
const FOLLOW: Duration = Duration::from_millis(30);

pub struct FileDrop {
    app: slint::Weak<AppWindow>,
    /// The files of the last drop, until a deck takes them.
    paths: Rc<RefCell<Vec<PathBuf>>>,
    /// True from a drop's first file until it is handed over.
    dropping: Rc<Cell<bool>>,
    /// The cursor from the last pointer move, in logical pixels, for when
    /// the system can't be asked.
    last_cursor: Rc<Cell<Option<(f32, f32)>>>,
    follow: Timer,
    settle: Timer,
}

impl FileDrop {
    pub fn new(app: &AppWindow) -> Self {
        let paths: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
        let drag = app.global::<TrackDrag>();
        {
            let paths = paths.clone();
            let weak = app.as_weak();
            drag.on_drop_files(move |deck| {
                let Some(app) = weak.upgrade() else {
                    return;
                };
                let files = std::mem::take(&mut *paths.borrow_mut());
                if files.is_empty() {
                    return;
                }
                let mut data = DataTransfer::default();
                data.set_file_paths(files);
                app.global::<TrackDrag>().invoke_drop(deck, data);
            });
        }
        Self {
            app: app.as_weak(),
            paths,
            dropping: Rc::default(),
            last_cursor: Rc::default(),
            follow: Timer::default(),
            settle: Timer::default(),
        }
    }

    /// Follows a window event from winit.
    pub fn window_event(&self, window: &slint::Window, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(window.scale_factor() as f64);
                self.last_cursor.set(Some((p.x, p.y)));
            }
            WindowEvent::HoveredFile(_) => {
                if self.follow.running() {
                    return;
                }
                let Some(app) = self.app.upgrade() else {
                    return;
                };
                app.global::<TrackDrag>().set_files_over(true);
                place(&app, self.last_cursor.get());
                let weak = self.app.clone();
                let last = self.last_cursor.clone();
                self.follow.start(TimerMode::Repeated, FOLLOW, move || {
                    if let Some(app) = weak.upgrade() {
                        place(&app, last.get());
                    }
                });
            }
            WindowEvent::HoveredFileCancelled => {
                self.follow.stop();
                if let Some(app) = self.app.upgrade() {
                    app.global::<TrackDrag>().set_files_over(false);
                }
            }
            WindowEvent::DroppedFile(path) => {
                // A drop of several files comes as one event each, in a row:
                // the first starts a new set, and the deck gets them once
                // they're all in.
                if !self.dropping.replace(true) {
                    self.paths.borrow_mut().clear();
                }
                self.paths.borrow_mut().push(path.clone());
                self.follow.stop();
                let weak = self.app.clone();
                let last = self.last_cursor.clone();
                let dropping = self.dropping.clone();
                self.settle
                    .start(TimerMode::SingleShot, Duration::ZERO, move || {
                        dropping.set(false);
                        let Some(app) = weak.upgrade() else {
                            return;
                        };
                        place(&app, last.get());
                        let drag = app.global::<TrackDrag>();
                        drag.set_files_over(false);
                        drag.set_files_dropped(drag.get_files_dropped().wrapping_add(1));
                    });
            }
            _ => {}
        }
    }
}

/// Moves `TrackDrag`'s file cursor to where the pointer is, or to `last` if
/// the system can't say.
fn place(app: &AppWindow, last: Option<(f32, f32)>) {
    let at = app
        .window()
        .with_winit_window(|w| cursor::position(w).map(|(x, y)| (x as f32, y as f32)))
        .flatten()
        .or(last);
    let (x, y) = at.unwrap_or((-1.0, -1.0));
    let drag = app.global::<TrackDrag>();
    drag.set_files_x(x);
    drag.set_files_y(y);
}

/// Where the pointer is over the window, in logical pixels from its top left.
#[cfg(target_os = "macos")]
mod cursor {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSEvent, NSView};
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::winit::window::Window;

    pub fn position(window: &Window) -> Option<(f64, f64)> {
        let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: winit's AppKit handle is the window's content NSView, which
        // lives as long as the window, and this runs on the main thread.
        let view: Retained<NSView> =
            unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }?;
        let ns_window = view.window()?;
        let in_window = ns_window.convertPointFromScreen(NSEvent::mouseLocation());
        let p = view.convertPoint_fromView(in_window, None);
        // Points are Slint's logical pixels; AppKit counts up from the bottom
        // unless the view is flipped.
        let y = if view.isFlipped() {
            p.y
        } else {
            view.bounds().size.height - p.y
        };
        Some((p.x, y))
    }
}

#[cfg(windows)]
mod cursor {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::winit::window::Window;
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    pub fn position(window: &Window) -> Option<(f64, f64)> {
        let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        let mut p = POINT { x: 0, y: 0 };
        // SAFETY: `p` is a valid POINT and the handle is winit's live window.
        let ok = unsafe {
            GetCursorPos(&mut p) != 0 && ScreenToClient(handle.hwnd.get() as _, &mut p) != 0
        };
        let scale = window.scale_factor();
        ok.then(|| (p.x as f64 / scale, p.y as f64 / scale))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod cursor {
    use slint::winit_030::winit::raw_window_handle::{
        HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    };
    use slint::winit_030::winit::window::Window;
    use x11_dl::xlib::Xlib;

    thread_local! {
        static XLIB: Option<Xlib> = Xlib::open().ok();
    }

    /// X11 only: Wayland doesn't deliver file drops through winit.
    pub fn position(window: &Window) -> Option<(f64, f64)> {
        let RawWindowHandle::Xlib(w) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        let RawDisplayHandle::Xlib(d) = window.display_handle().ok()?.as_raw() else {
            return None;
        };
        let display = d.display?.as_ptr().cast();
        let (mut root, mut child) = (0, 0);
        let (mut root_x, mut root_y, mut x, mut y) = (0, 0, 0, 0);
        let mut mask = 0;
        // SAFETY: the display and window are winit's live ones, and every
        // out pointer is a local.
        let ok = XLIB.with(|xlib| {
            xlib.as_ref().is_some_and(|xlib| unsafe {
                (xlib.XQueryPointer)(
                    display,
                    w.window,
                    &mut root,
                    &mut child,
                    &mut root_x,
                    &mut root_y,
                    &mut x,
                    &mut y,
                    &mut mask,
                ) != 0
            })
        });
        let scale = window.scale_factor();
        ok.then(|| (x as f64 / scale, y as f64 / scale))
    }
}

#[cfg(not(any(unix, windows)))]
mod cursor {
    pub fn position(_: &slint::winit_030::winit::window::Window) -> Option<(f64, f64)> {
        None
    }
}
