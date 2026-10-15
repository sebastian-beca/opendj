//! A taller top bar on macOS: the hidden title bar is made as
//! tall as the app's own bar, and the traffic lights are centred in it.
//! AppKit puts them back on a resize or a trip to full screen, so this is
//! applied again after every window event.

use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindowButton};
use objc2_foundation::NSPoint;
use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::winit::window::Window;

/// Gap between the left edge of the window and the close button, in points.
const LEFT: f64 = 14.0;
/// Space from one traffic light to the next, in points.
const STEP: f64 = 20.0;

/// Centres the traffic lights in a title bar `height` points tall.
pub fn center_traffic_lights(window: &Window, height: f64) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: winit's AppKit handle is the window's content NSView, which
    // lives as long as the window, and this runs on the main thread.
    let view: Retained<NSView> =
        match unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) } {
            Some(v) => v,
            None => return,
        };
    let Some(ns_window) = view.window() else {
        return;
    };
    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|b| ns_window.standardWindowButton(b));
    let Some(close) = &buttons[0] else {
        return;
    };
    // The buttons sit in the title bar view, inside its container.
    // SAFETY: both are kept alive by the window, which `ns_window` retains.
    let Some(container) = (unsafe { close.superview() }).and_then(|v| unsafe { v.superview() })
    else {
        return;
    };
    let mut frame = container.frame();
    frame.size.height = height;
    frame.origin.y = ns_window.frame().size.height - height;
    container.setFrame(frame);
    let button_height = close.frame().size.height;
    for (i, button) in buttons.iter().enumerate() {
        if let Some(button) = button {
            button.setFrameOrigin(NSPoint::new(
                LEFT + STEP * i as f64,
                ((height - button_height) / 2.0).round(),
            ));
        }
    }
}
