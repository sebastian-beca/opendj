//! The Player bar's transport for everything outside the main window that
//! plays the music the bar shows: the mini player, and the system's media
//! keys and Now Playing (see [`crate::media_controls`]). Each acts as the
//! bar's button of the same name does.

use slint::{ComponentHandle, Model};

use crate::{AppWindow, Controls};

/// The deck the Player bar shows (Automix's live deck, else the master
/// deck) and its play control's index.
pub fn deck(app: &AppWindow) -> (usize, i32) {
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

/// Whether the deck the bar shows is playing.
pub fn playing(app: &AppWindow) -> bool {
    let (_, play) = deck(app);
    let controls = app.global::<Controls>();
    controls.invoke_value(play, controls.get_tick()) > 0.0
}

/// Play/Pause. As on the bar, starting also switches Automix on, so the
/// next track in Up Next mixes in when this one ends.
pub fn play_pause(app: &AppWindow) {
    if playing(app) {
        pause(app);
    } else {
        play(app);
    }
}

/// Plays, if it isn't already.
pub fn play(app: &AppWindow) {
    if playing(app) {
        return;
    }
    let (d, play) = deck(app);
    let loaded = app.get_decks().row_data(d).is_some_and(|i| i.loaded);
    if loaded {
        app.global::<Controls>().invoke_set_value(play, 1.0);
    }
    if !app.get_automix_on() && (app.get_automix_queued() > 0 || !loaded) {
        app.invoke_automix_toggle();
    }
}

/// Pauses, if it is playing.
pub fn pause(app: &AppWindow) {
    if playing(app) {
        let (_, play) = deck(app);
        app.global::<Controls>().invoke_set_value(play, 0.0);
    }
}

/// Previous: back to the start of the track.
pub fn previous(app: &AppWindow) {
    set(app, deck(app).0, "start", 1.0);
}

/// Whether Next has a track to mix into.
pub fn can_next(app: &AppWindow) -> bool {
    app.get_automix_on() && app.get_automix_queued() > 0
}

/// Next: on to the next track in Up Next now, with a cut in Player as on
/// its bar, else mixing in as Automix's Mix now does.
pub fn next(app: &AppWindow) {
    if !can_next(app) {
        return;
    }
    if app.get_layout() == crate::PLAYER_LAYOUT {
        app.invoke_automix_cut_now();
    } else {
        app.invoke_automix_mix_now();
    }
}

/// Moves to `fraction` (0 to 1) of the track.
pub fn seek(app: &AppWindow, fraction: f32) {
    set(app, deck(app).0, "playposition", fraction.clamp(0.0, 1.0));
}
