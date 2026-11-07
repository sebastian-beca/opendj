//! The mini player (ui/mini.slint): a small square window of album art,
//! on top of the others, showing the track the Player bar shows with
//! Previous, Play/Pause and Next under the pointer. Opening it hides the
//! main window; its ✕, or closing it, brings the main window back. It has
//! no title bar, so it moves and resizes through winit, and stays square.
//!
//! The two windows share no globals, so a timer copies what is playing
//! from the main window into the mini player while it shows, and its
//! buttons act through the main window's controls.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use slint::winit_030::winit::window::ResizeDirection;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, LogicalSize, Model, Timer, TimerMode};

use crate::{AppWindow, Controls, MiniPlayer, Theme};

/// Opens the mini player from the main window's `mini-player` callback.
pub fn install(app: &AppWindow) {
    let weak = app.as_weak();
    let mini: Rc<std::cell::RefCell<Option<(MiniPlayer, Timer)>>> = Rc::default();
    app.on_mini_player(move || {
        let Some(app) = weak.upgrade() else { return };
        let mut slot = mini.borrow_mut();
        if slot.is_none() {
            match MiniPlayer::new() {
                Ok(m) => {
                    wire(&app, &m);
                    let timer = Timer::default();
                    let (a, w) = (app.as_weak(), m.as_weak());
                    let last = Cell::new((0.0, 0.0));
                    timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
                        if let (Some(app), Some(m)) = (a.upgrade(), w.upgrade()) {
                            if m.window().is_visible() {
                                update(&app, &m);
                                keep_square(&m, &last);
                            }
                        }
                    });
                    *slot = Some((m, timer));
                }
                Err(e) => {
                    eprintln!("can't open the mini player: {e}");
                    return;
                }
            }
        }
        let (m, _) = slot.as_ref().unwrap();
        update(&app, m);
        if m.show().is_ok() {
            let _ = app.hide();
        }
    });
}

/// The deck shown and its play control's index.
fn deck(app: &AppWindow) -> (usize, i32) {
    let d = app.get_player_deck().clamp(0, 1) as usize;
    let controls = app.global::<Controls>();
    let play = controls.invoke_resolve(format!("[Channel{}]", d + 1).into(), "play".into());
    (d, play)
}

fn set(app: &AppWindow, deck: usize, key: &str, value: f32) {
    let controls = app.global::<Controls>();
    let index = controls.invoke_resolve(format!("[Channel{}]", deck + 1).into(), key.into());
    controls.invoke_set_value(index, value);
}

fn wire(app: &AppWindow, m: &MiniPlayer) {
    let a = app.as_weak();
    m.on_play_pause(move || {
        let Some(app) = a.upgrade() else { return };
        let (d, play) = deck(&app);
        let controls = app.global::<Controls>();
        let playing = controls.invoke_value(play, controls.get_tick()) > 0.0;
        let loaded = app.get_decks().row_data(d).is_some_and(|i| i.loaded);
        // As the Player bar: starting also switches Automix on.
        if playing {
            controls.invoke_set_value(play, 0.0);
        } else {
            if loaded {
                controls.invoke_set_value(play, 1.0);
            }
            if !app.get_automix_on() && (app.get_automix_queued() > 0 || !loaded) {
                app.invoke_automix_toggle();
            }
        }
    });
    let a = app.as_weak();
    m.on_previous(move || {
        if let Some(app) = a.upgrade() {
            set(&app, deck(&app).0, "start", 1.0);
        }
    });
    let a = app.as_weak();
    m.on_next(move || {
        if let Some(app) = a.upgrade() {
            if app.get_automix_on() && app.get_automix_queued() > 0 {
                app.invoke_automix_mix_now();
            }
        }
    });
    let a = app.as_weak();
    m.on_seek(move |f| {
        if let Some(app) = a.upgrade() {
            set(&app, deck(&app).0, "playposition", f);
        }
    });
    let (a, w) = (app.as_weak(), m.as_weak());
    let back = move || {
        if let (Some(app), Some(m)) = (a.upgrade(), w.upgrade()) {
            if app.show().is_ok() {
                let _ = m.hide();
            }
        }
    };
    m.on_expand(back.clone());
    let w = m.as_weak();
    m.on_start_move(move || {
        if let Some(m) = w.upgrade() {
            m.window().with_winit_window(|win| {
                let _ = win.drag_window();
            });
        }
    });
    let w = m.as_weak();
    m.on_start_resize(move || {
        if let Some(m) = w.upgrade() {
            m.window().with_winit_window(|win| {
                let _ = win.drag_resize_window(ResizeDirection::SouthEast);
            });
        }
    });
    m.window().on_close_requested(move || {
        back();
        slint::CloseRequestResponse::HideWindow
    });
}

fn update(app: &AppWindow, m: &MiniPlayer) {
    let (d, play) = deck(app);
    let Some(info) = app.get_decks().row_data(d) else {
        return;
    };
    let controls = app.global::<Controls>();
    m.set_loaded(info.loaded);
    m.set_track_title(if info.loaded {
        info.title.clone()
    } else {
        "Nothing playing".into()
    });
    m.set_artist(if info.loaded {
        info.artist.clone()
    } else {
        Default::default()
    });
    m.set_art_image(info.cover.clone());
    m.set_has_cover(info.has_cover);
    m.set_initials(info.initials.clone());
    m.set_tint(app.global::<Theme>().invoke_deck_color(d as i32));
    m.set_playing(controls.invoke_value(play, controls.get_tick()) > 0.0);
    m.set_progress(info.progress);
    m.set_mix_from(info.automix_from);
    m.set_mix_to(info.automix_to);
    let next_in = app.get_automix_next_in();
    m.set_next_in(if next_in.is_empty() {
        Default::default()
    } else {
        format!("NEXT IN {next_in}").into()
    });
    m.set_can_next(app.get_automix_on() && app.get_automix_queued() > 0);
}

/// Brings the window back to a square after a resize: the side the DJ
/// changed sets both. `last` is the size seen the time before.
fn keep_square(m: &MiniPlayer, last: &Cell<(f32, f32)>) {
    let window = m.window();
    let size = window.size().to_logical(window.scale_factor());
    let (w, h) = (size.width, size.height);
    if (w - h).abs() < 1.0 {
        last.set((w, h));
        return;
    }
    let (lw, lh) = last.get();
    let side = if (w - lw).abs() >= (h - lh).abs() {
        w
    } else {
        h
    };
    window.set_size(LogicalSize::new(side, side));
    last.set((side, side));
}
