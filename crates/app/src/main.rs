//! openDJ: the Slint UI wired to the control registry, the audio engine
//! and the GPU waveform renderer.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mixxx_control::{ControlHandle, ControlRegistry};
use mixxx_engine::cover::Cover;
use mixxx_engine::key::Key;
use mixxx_engine::scan::scan;
use mixxx_engine::stems;
use mixxx_engine::synth::SAMPLE_LIBRARY_SIZE;
use mixxx_engine::{
    deck_group, default_output_device, effect_group, effect_name, output_devices, sample_library,
    AudioEngine, AudioSetup, Role, Route, Source, Track, VisualPositions, Waveform,
    AVAILABLE_EFFECTS, EFFECTS_PER_UNIT, NUM_DECKS,
};
use mixxx_library::Library;
use mixxx_waveform::{
    steady_grid, wgpu, Frame, GpuTrack, GridSection, Mode, Style, WaveformRenderer, WaveformView,
    GRID_SECTIONS,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smooth::SmoothClock;

slint::include_modules!();

mod art;
mod automix;
mod browser;
mod coverart;
mod diagnostics;
mod files;
mod inbox;
#[cfg(target_os = "macos")]
mod mac_menu;
#[cfg(target_os = "macos")]
mod mac_titlebar;
mod prefs;
mod settings;
mod smooth;
mod sources;
mod spotify;
mod stores;
mod ui_font;

/// Fills the master and headphone output menus and switches devices when
/// one is picked. The menus list the devices present when they open.
/// The settings page's output table: shows the routes in use and applies
/// changes to them.
fn install_audio_settings(app: &AppWindow, engine: Rc<AudioEngine>) {
    let devices = Rc::new(RefCell::new(output_devices()));
    let default_device = Rc::new(RefCell::new(default_output_device()));
    // Kept between updates and changed row by row: replacing a list would
    // rebuild its rows and close a menu that is opening.
    let rows_model: Rc<VecModel<OutputRow>> = Default::default();
    let names_model: Rc<VecModel<SharedString>> = Default::default();
    {
        let g = app.global::<AudioSettings>();
        g.set_rows(ModelRc::from(rows_model.clone()));
        g.set_devices(ModelRc::from(names_model.clone()));
    }
    let show = {
        let default_device = default_device.clone();
        let engine = engine.clone();
        let devices = devices.clone();
        move || {
            let setup = settings::audio_setup();
            let devices = devices.borrow();
            let default_channels = default_device
                .borrow()
                .as_ref()
                .and_then(|name| devices.iter().find(|d| d.name == *name))
                .map_or(2, |d| d.channels);
            let status = engine.route_status();
            let rows: Vec<OutputRow> = setup
                .routes
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let found = devices.iter().position(|d| d.name == r.device);
                    let channels = if r.device.is_empty() {
                        default_channels
                    } else {
                        found.map_or(2, |k| devices[k].channels)
                    };
                    let pairs = (channels / 2).max(1).max(r.pair + 1);
                    let status = status.get(i).cloned().unwrap_or_default();
                    OutputRow {
                        role: Role::ALL.iter().position(|&x| x == r.role).unwrap_or(0) as i32,
                        device: if r.device.is_empty() {
                            0
                        } else {
                            found.map_or(-1, |k| k as i32 + 1)
                        },
                        device_name: if r.device.is_empty() {
                            "System default".into()
                        } else if found.is_some() {
                            r.device.as_str().into()
                        } else {
                            format!("{} (not connected)", r.device).into()
                        },
                        pair: r.pair as i32,
                        pairs: ModelRc::new(VecModel::from(
                            (0..pairs)
                                .map(|p| {
                                    SharedString::from(format!("{} & {}", 2 * p + 1, 2 * p + 2))
                                })
                                .collect::<Vec<_>>(),
                        )),
                        ok: status.starts_with(|c: char| c.is_ascii_digit()),
                        status: status.into(),
                    }
                })
                .collect();
            let names: Vec<SharedString> = std::iter::once("System default".to_string())
                .chain(devices.iter().map(|d| d.name.clone()))
                .map(Into::into)
                .collect();
            if names_model.iter().ne(names.iter().cloned()) {
                names_model.set_vec(names);
            }
            if rows_model.row_count() != rows.len() {
                rows_model.set_vec(rows);
            } else {
                for (i, row) in rows.into_iter().enumerate() {
                    let old = rows_model.row_data(i).unwrap();
                    let same = old.role == row.role
                        && old.device == row.device
                        && old.device_name == row.device_name
                        && old.pair == row.pair
                        && old.status == row.status
                        && old.ok == row.ok
                        && old.pairs.iter().eq(row.pairs.iter());
                    if !same {
                        rows_model.set_row_data(i, row);
                    }
                }
            }
        }
    };
    show();
    let show = Rc::new(show);
    let g = app.global::<AudioSettings>();
    g.on_refresh({
        let devices = devices.clone();
        let show = show.clone();
        let default_device = default_device.clone();
        move || {
            *devices.borrow_mut() = output_devices();
            *default_device.borrow_mut() = default_output_device();
            show();
        }
    });
    let change = {
        let show = show.clone();
        move |edit: &dyn Fn(&mut AudioSetup)| {
            let mut setup = settings::audio_setup();
            edit(&mut setup);
            settings::set_audio_setup(&setup);
            engine.set_outputs(&setup);
            show();
        }
    };
    let change = Rc::new(change);
    g.on_set_role({
        let change = change.clone();
        move |row, role| {
            change(&|s| {
                if let (Some(r), Some(&role)) =
                    (s.routes.get_mut(row as usize), Role::ALL.get(role as usize))
                {
                    r.role = role;
                }
            })
        }
    });
    g.on_set_device({
        let change = change.clone();
        let devices = devices.clone();
        move |row, device| {
            let name = match device {
                0 => String::new(),
                d => devices
                    .borrow()
                    .get(d as usize - 1)
                    .map(|d| d.name.clone())
                    .unwrap_or_default(),
            };
            change(&|s| {
                if let Some(r) = s.routes.get_mut(row as usize) {
                    r.device = name.clone();
                    r.pair = 0;
                }
            })
        }
    });
    g.on_set_pair({
        let change = change.clone();
        move |row, pair| {
            change(&|s| {
                if let Some(r) = s.routes.get_mut(row as usize) {
                    r.pair = pair.max(0) as u16;
                }
            })
        }
    });
    g.on_remove({
        let change = change.clone();
        move |row| {
            change(&|s| {
                if (row as usize) < s.routes.len() {
                    s.routes.remove(row as usize);
                }
            })
        }
    });
    g.on_add(move || {
        let devices = devices.clone();
        change(&|s| {
            // The first mix not routed yet, on the first route's device, on
            // the first pair it leaves free.
            let role = Role::ALL
                .into_iter()
                .find(|role| s.routes.iter().all(|r| r.role != *role))
                .unwrap_or(Role::Booth);
            let device = s
                .routes
                .first()
                .map(|r| r.device.clone())
                .unwrap_or_default();
            let channels = devices
                .borrow()
                .iter()
                .find(|d| d.name == device)
                .map_or(2, |d| d.channels);
            let pair = (0..(channels / 2).max(1))
                .find(|p| !s.routes.iter().any(|r| r.device == device && r.pair == *p))
                .unwrap_or(0);
            s.routes.push(Route { role, device, pair });
        })
    });
}

/// The top bar's height in points, as in main.slint; the macOS title bar
/// is made this tall.
#[cfg(target_os = "macos")]
const TOP_BAR_HEIGHT: f64 = 52.0;

/// Seconds of audio shown across the scrolling waveform at normal speed.
const VISIBLE_SECONDS: f64 = 10.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    diagnostics::install();
    let backend =
        slint::BackendSelector::new().require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default());
    // On macOS the app draws under a hidden title bar: only
    // the traffic lights stay, over the top bar.
    #[cfg(target_os = "macos")]
    let backend = backend.with_winit_window_attributes_hook(|attributes| {
        use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
        attributes
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
    });
    backend.select()?;
    // San Francisco on macOS; registered before any window.
    let ui_family = ui_font::install();

    let registry = ControlRegistry::new();
    let engine = Rc::new(AudioEngine::start(
        registry.clone(),
        &settings::audio_setup(),
    ));
    engine.recorder().configure(settings::recording());
    let recorder = engine.recorder().clone();
    // The saved library, with the tracks it had last time; the built-in
    // samples until there is anything in it.
    let library_path = settings::config_dir().map(|d| mixxx_library::default_path(&d));
    let db = library_path
        .as_deref()
        .and_then(|p| match Library::open(p) {
            Ok(db) => Some(db),
            Err(e) => {
                eprintln!("can't open the library at {}: {e}", p.display());
                None
            }
        });
    let saved: Vec<Arc<Track>> = db
        .as_ref()
        .and_then(|db| db.all_tracks(Path::exists).ok())
        .unwrap_or_default()
        .into_iter()
        .map(Arc::new)
        .collect();
    let samples = saved.is_empty();
    let library = if samples { sample_library() } else { saved };
    let app = AppWindow::new()?;
    #[cfg(target_os = "macos")]
    {
        use slint::winit_030::{EventResult, WinitWindowAccessor};
        app.set_traffic_light_inset(76.0);
        // The traffic lights, centred in the top bar after anything that
        // might have moved them back.
        app.window().on_winit_window_event(|window, event| {
            use slint::winit_030::winit::event::WindowEvent;
            if !matches!(
                event,
                WindowEvent::CursorMoved { .. } | WindowEvent::RedrawRequested
            ) {
                window
                    .with_winit_window(|w| mac_titlebar::center_traffic_lights(w, TOP_BAR_HEIGHT));
            }
            EventResult::Propagate
        });
    }
    // Pro or Player, as last picked.
    app.set_layout(settings::number("layout", 0.0).clamp(0.0, 1.0) as i32);
    app.on_layout_changed(|layout| settings::set("layout", serde_json::json!(layout)));
    {
        use slint::winit_030::WinitWindowAccessor;
        let weak = app.as_weak();
        app.on_drag_window(move || {
            if let Some(app) = weak.upgrade() {
                app.window().with_winit_window(|w| w.drag_window().ok());
            }
        });
        let weak = app.as_weak();
        app.on_zoom_window(move || {
            if let Some(app) = weak.upgrade() {
                app.window()
                    .with_winit_window(|w| w.set_maximized(!w.is_maximized()));
            }
        });
    }
    if let Some(family) = ui_family {
        app.global::<Theme>().set_font_family(family.into());
    }
    install_audio_settings(&app, engine.clone());

    bind_controls(&app, registry.clone());
    app.global::<EffectList>()
        .set_names(ModelRc::new(VecModel::from(
            AVAILABLE_EFFECTS
                .iter()
                .map(|&n| SharedString::from(n))
                .collect::<Vec<_>>(),
        )));
    {
        let engine = engine.clone();
        app.global::<DeckActions>().on_separate_stems(move |deck| {
            let loaded = engine.loaded_tracks();
            let track = loaded.read().unwrap().get(deck as usize).cloned().flatten();
            if let Some(t) = track {
                engine.separate_stems(&t);
            }
        });
    }
    let browser = browser::Browser::install(
        &app,
        library.clone(),
        db,
        library_path.clone(),
        engine.clone(),
        &registry,
    );
    let sources = sources::Sources::install(&app, library_path.clone());
    let automix = Rc::new(RefCell::new(automix::AutoMix::new(&registry)));
    {
        let a = automix.clone();
        app.on_automix_toggle(move || a.borrow_mut().toggle());
        let a = automix.clone();
        app.on_automix_mix_now(move || a.borrow_mut().mix_now());
        let a = automix.clone();
        app.on_automix_skip(move || a.borrow_mut().skip());
        let a = automix.clone();
        app.on_automix_set_length(move |i| a.borrow_mut().set_length(i.max(0) as usize));
        let a = automix.clone();
        app.on_automix_set_style(move |i| a.borrow_mut().set_style(i.max(0) as usize));
    }

    // MIDI controllers, driven by Mixxx's mappings. Started after the browser
    // so the library controls the mappings use already exist.
    let controllers = match mixxx_controllers::default_mappings_dir() {
        Some(dir) => Some(Rc::new(mixxx_controllers::Controllers::start(
            registry.clone(),
            dir,
            settings::controller_mappings(),
        ))),
        None => {
            eprintln!("controller mappings not found; MIDI controllers are disabled");
            None
        }
    };

    // Start with two sample tracks loaded so the screen is not empty.
    if samples {
        engine.load_track(0, library[0].clone());
        engine.load_track(1, library[4].clone());
    }

    // Bring the library up to date in the background with new files in the
    // music folder. The browser then shows the library as saved.
    let (scan_tx, scan_rx) = std::sync::mpsc::channel();
    sync_library(library_path.clone(), scan_tx.clone());
    prefs::apply_saved(&registry);
    let prefs = prefs::Prefs::install(
        &app,
        registry.clone(),
        controllers.clone(),
        engine.recorder().clone(),
        Box::new(move || sync_library(library_path.clone(), scan_tx.clone())),
        {
            let sources = sources.clone();
            Box::new(move |id| sources.borrow_mut().configure(id))
        },
        {
            let engine = engine.clone();
            Box::new(move || engine.set_outputs(&settings::audio_setup()))
        },
    );

    let decks = Rc::new(Decks::new(&registry));
    let loaded = engine.loaded_tracks();
    let visual = engine.visual_positions();

    // Capture Slint's wgpu device once it exists; all waveform rendering uses
    // it. Waveforms are drawn just before Slint renders each frame, so their
    // play position is as close as possible to when the frame is shown.
    let gpu: Rc<RefCell<Option<Gpu>>> = Rc::new(RefCell::new(None));
    {
        let gpu = gpu.clone();
        let app_weak = app.as_weak();
        let decks = decks.clone();
        let loaded = loaded.clone();
        let style = match std::env::var("OPENDJ_WAVEFORM").as_deref() {
            Ok("bands") => Style::Bands,
            _ => Style::Rgb,
        };
        app.window()
            .set_rendering_notifier(move |state, api| match (state, api) {
                (
                    slint::RenderingState::RenderingSetup,
                    slint::GraphicsAPI::WGPU30 { device, queue, .. },
                ) => {
                    let renderer = WaveformRenderer::new(device);
                    let make = |mode| {
                        let mut v = WaveformView::new(device, mode, 1, 1);
                        v.set_style(style);
                        v
                    };
                    *gpu.borrow_mut() = Some(Gpu {
                        device: device.clone(),
                        queue: queue.clone(),
                        renderer,
                        scrolling: [make(Mode::Scrolling), make(Mode::Scrolling)],
                        overview: [make(Mode::Overview), make(Mode::Overview)],
                        tracks: Default::default(),
                        clocks: Default::default(),
                        moving: false,
                        images_set: false,
                    });
                }
                (slint::RenderingState::BeforeRendering, _) => {
                    let mut gpu = gpu.borrow_mut();
                    let (Some(app), Some(gpu)) = (app_weak.upgrade(), gpu.as_mut()) else {
                        return;
                    };
                    let tracks = loaded.read().unwrap().clone();
                    gpu.render(&app, &decks, &tracks, &visual);
                }
                (slint::RenderingState::AfterRendering, _) => {
                    // While a deck moves, draw again as soon as the display
                    // can show it, rather than on the UI timer, which beats
                    // against the display's refresh and makes motion uneven.
                    let moving = gpu.borrow().as_ref().is_some_and(|g| g.moving);
                    if let (true, Some(app)) = (moving, app_weak.upgrade()) {
                        app.window().request_redraw();
                    }
                }
                _ => {}
            })?;
    }

    let start = Instant::now();
    let timer = slint::Timer::default();
    let covers: RefCell<[CoverImage; NUM_DECKS]> = Default::default();
    let last_tick = Cell::new(Instant::now());
    {
        let app_weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                app.global::<Controls>()
                    .set_tick(app.global::<Controls>().get_tick().wrapping_add(1));
                if let Ok(scanned) = scan_rx.try_recv() {
                    browser.borrow_mut().set_library(scanned);
                }
                browser.borrow_mut().poll_controllers(&app);
                browser.borrow_mut().poll_spotify(&app);
                browser.borrow_mut().poll_stores(&app);
                for track in sources.borrow_mut().poll() {
                    browser.borrow_mut().add_imported(track);
                }
                browser.borrow_mut().poll_art();
                browser.borrow_mut().poll_files();
                prefs.borrow_mut().poll();
                if let Some(c) = &controllers {
                    let names: Vec<String> = c.connected().into_iter().map(|c| c.port).collect();
                    let status = if names.is_empty() {
                        "None".to_string()
                    } else {
                        names
                            .iter()
                            .map(|n| short_port_name(n))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    };
                    let settings = app.global::<AudioSettings>();
                    if settings.get_controllers() != status.as_str() {
                        settings.set_controllers(status.into());
                    }
                }
                let tracks = loaded.read().unwrap().clone();
                let camelot = app.global::<KeyNotation>().get_camelot();
                let mut b = browser.borrow_mut();
                b.set_camelot(camelot);
                b.set_loaded(&app, [0, 1].map(|d| tracks[d].as_ref().map(|t| t.id)));
                b.learn_analysis(&tracks);
                b.keep(&tracks, 0.016);
                let mut host = browser::AutomixHost {
                    browser: &mut b,
                    engine: &engine,
                };
                let now = Instant::now();
                let dt = now.duration_since(last_tick.replace(now)).as_secs_f64();
                automix.borrow_mut().tick(&mut host, &tracks, dt.min(0.1));
                automix.borrow().show(&app, b.automix_len(), &tracks);
                drop(b);
                app.set_decks(ModelRc::new(VecModel::from(
                    (0..NUM_DECKS)
                        .map(|d| {
                            let track = tracks[d].as_deref();
                            let mut info = decks.info(d, track, camelot);
                            if let Some(image) = covers.borrow_mut()[d].get(track) {
                                info.cover = image;
                                info.has_cover = true;
                            }
                            info
                        })
                        .collect::<Vec<_>>(),
                )));
                let t = start.elapsed().as_secs();
                app.set_clock(format!("{:02}:{:02}", t / 60 % 60, t % 60).into());
                // Redraw every tick so the waveforms keep moving.
                app.window().request_redraw();
            },
        );
    }

    // Settings… (⌘,) in the macOS app menu, which only exists once the
    // event loop runs, so keep trying until it's there.
    #[cfg(target_os = "macos")]
    let _settings_menu = {
        let weak = app.as_weak();
        let timer = slint::Timer::default();
        let added = std::rc::Rc::new(std::cell::Cell::new(false));
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(100),
            move || {
                if added.get() {
                    return;
                }
                let weak = weak.clone();
                added.set(mac_menu::add_settings_item(move || {
                    if let Some(app) = weak.upgrade() {
                        app.global::<Preferences>().set_open(true);
                    }
                }));
            },
        );
        timer
    };

    app.run()?;
    // Finishes the files, with the cue sheet, if the DJ quits mid-recording.
    recorder.stop();
    Ok(())
}

/// Brings the library up to date in the background: new and changed files
/// in the music folder, and the library of any importer add-on whose file
/// has changed since last time. The tracks as saved are then sent to `tx`,
/// for the browser to show.
fn sync_library(library_path: Option<PathBuf>, tx: std::sync::mpsc::Sender<Vec<Arc<Track>>>) {
    let music = settings::music_folder().filter(|d| d.is_dir());
    let importers = stores::importers();
    if music.is_none() && importers.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let scanned = music.map_or_else(Vec::new, |d| scan(&d, SAMPLE_LIBRARY_SIZE));
        let synced = library_path
            .as_deref()
            .map(|path| -> Result<Vec<Track>, String> {
                let mut db = Library::open(path).map_err(|e| e.to_string())?;
                let scanned: Vec<Track> = scanned.iter().map(|t| (**t).clone()).collect();
                let report = db.sync(&scanned).map_err(|e| e.to_string())?;
                if report.added + report.updated > 0 {
                    eprintln!(
                        "library: {} tracks added, {} updated",
                        report.added, report.updated
                    );
                }
                for importer in &importers {
                    let Some(database) = importer.database() else {
                        continue;
                    };
                    match db.import(importer.id(), &database, |p| importer.read(p)) {
                        Ok(Some(r)) => eprintln!(
                            "{}: {} tracks added, {} updated",
                            importer.id(),
                            r.added,
                            r.updated
                        ),
                        Ok(None) => {}
                        Err(e) => eprintln!("can't import {}: {e}", database.display()),
                    }
                }
                db.all_tracks(Path::exists).map_err(|e| e.to_string())
            });
        let tracks: Vec<Arc<Track>> = match synced {
            Some(Ok(tracks)) => tracks.into_iter().map(Arc::new).collect(),
            other => {
                if let Some(Err(e)) = other {
                    eprintln!("can't update the library: {e}");
                }
                // Without a library database, show what was found.
                scanned
            }
        };
        if !tracks.is_empty() {
            let _ = tx.send(tracks);
        }
    });
}

/// A MIDI port's device name: ALSA ports read "DDJ-FLX4:DDJ-FLX4 MIDI 1 24:0".
fn short_port_name(port: &str) -> &str {
    port.split(':').next().unwrap_or(port)
}

/// Connects the `Controls` global used by every skin widget to the registry.
/// Names are resolved to an index once; per-frame reads are atomic loads.
/// Resolved controls, indexed by the number handed to the UI.
#[derive(Default)]
struct Resolved {
    handles: Vec<ControlHandle>,
    by_name: HashMap<(String, String), i32>,
}

fn bind_controls(app: &AppWindow, registry: Arc<ControlRegistry>) {
    let handles: Rc<RefCell<Resolved>> = Default::default();
    let controls = app.global::<Controls>();
    let get = {
        let handles = handles.clone();
        move |i: i32| -> Option<ControlHandle> { handles.borrow().handles.get(i as usize).cloned() }
    };
    {
        let handles = handles.clone();
        controls.on_resolve(move |group, key| {
            let k = (group.to_string(), key.to_string());
            let mut h = handles.borrow_mut();
            if let Some(i) = h.by_name.get(&k) {
                return *i;
            }
            let Some(handle) = registry.get(&k.0, &k.1) else {
                eprintln!("skin refers to unknown control {},{}", k.0, k.1);
                return -1;
            };
            let i = h.handles.len() as i32;
            h.handles.push(handle);
            h.by_name.insert(k, i);
            i
        });
    }
    let g = get.clone();
    controls.on_parameter(move |i, _| g(i).map(|c| c.get_parameter() as f32).unwrap_or(0.0));
    let g = get.clone();
    controls.on_value(move |i, _| g(i).map(|c| c.get() as f32).unwrap_or(0.0));
    let g = get.clone();
    controls.on_set_parameter(move |i, p| {
        if let Some(c) = g(i) {
            c.set_parameter(p as f64)
        }
    });
    let g = get.clone();
    controls.on_set_value(move |i, v| {
        if let Some(c) = g(i) {
            c.set(v as f64)
        }
    });
    let g = get.clone();
    controls.on_toggle(move |i| {
        if let Some(c) = g(i) {
            c.toggle()
        }
    });
    controls.on_reset(move |i| {
        if let Some(c) = get(i) {
            c.reset()
        }
    });
}

#[cfg(test)]
mod time_tests {
    #[test]
    fn whole_seconds() {
        assert_eq!(super::fmt_seconds(479.96, false), "8:00");
        assert_eq!(super::fmt_seconds(0.2, true), "0:01");
        assert_eq!(super::fmt_seconds(60.0, true), "1:00");
        assert_eq!(super::fmt_seconds(-1.0, true), "0:00");
    }
}

pub(crate) fn fmt_time(seconds: f64) -> String {
    // Round to tenths first, so 479.96 s reads 8:00.0 rather than 7:60.0.
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

/// Like fmt_time with two-digit minutes, "02:00.8", as the jog shows times.
fn fmt_clock(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    format!(
        "{:02}:{:02}.{}",
        tenths / 600,
        tenths / 10 % 60,
        tenths % 10
    )
}

/// Whole seconds, as the deck header shows times: "7:59". Remaining time
/// counts down to 0:00 only when the track ends, so it rounds up.
fn fmt_seconds(seconds: f64, round_up: bool) -> String {
    let s = seconds.max(0.0);
    let s = if round_up {
        (s - 1e-6).ceil()
    } else {
        s.round()
    } as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Handles to the controls the deck displays and waveforms read every frame.
struct DeckHandles {
    playposition: ControlHandle,
    rate: ControlHandle,
    rate_range: ControlHandle,
    bpm: ControlHandle,
    cue_point: ControlHandle,
    loop_enabled: ControlHandle,
    loop_start: ControlHandle,
    loop_end: ControlHandle,
    loop_in_pending: ControlHandle,
    beatloop_size: ControlHandle,
    keylock: ControlHandle,
    /// Key shift in semitones, the key being played and the file's key.
    pitch: ControlHandle,
    key: ControlHandle,
    file_key: ControlHandle,
    hotcues: Vec<(ControlHandle, ControlHandle)>,
    /// loaded_effect of each slot of the effect unit shown under this deck.
    effects: Vec<ControlHandle>,
    /// The lists the deck shows, kept between frames: the UI rebuilds the
    /// rows of a replaced list, which would drop a click in progress.
    effect_names: Rc<VecModel<SharedString>>,
    key_menu: Rc<VecModel<KeyItem>>,
    key_menu_all: Rc<VecModel<KeyItem>>,
    stem_names: Rc<VecModel<SharedString>>,
    stem_colors: Rc<VecModel<slint::Color>>,
}

/// `model` as a list for the UI, with its rows replaced only when `items` differ.
fn kept<T: Clone + PartialEq + 'static>(model: &Rc<VecModel<T>>, items: Vec<T>) -> ModelRc<T> {
    if model.iter().ne(items.iter().cloned()) {
        model.set_vec(items);
    }
    ModelRc::from(model.clone())
}

struct Decks([DeckHandles; NUM_DECKS]);

impl Decks {
    fn new(reg: &ControlRegistry) -> Self {
        Self(std::array::from_fn(|d| {
            let g = deck_group(d);
            let c = |k: &str| {
                reg.get(&g, k)
                    .unwrap_or_else(|| panic!("missing control {g},{k}"))
            };
            DeckHandles {
                playposition: c("playposition"),
                rate: c("rate"),
                rate_range: c("rateRange"),
                bpm: c("bpm"),
                cue_point: c("cue_point"),
                loop_enabled: c("loop_enabled"),
                loop_start: c("loop_start_position"),
                loop_end: c("loop_end_position"),
                loop_in_pending: c("loop_in_pending"),
                beatloop_size: c("beatloop_size"),
                keylock: c("keylock"),
                pitch: c("pitch"),
                key: c("key"),
                file_key: c("file_key"),
                hotcues: (1..=8)
                    .map(|n| {
                        (
                            c(&format!("hotcue_{n}_enabled")),
                            c(&format!("hotcue_{n}_position")),
                        )
                    })
                    .collect(),
                effects: (0..EFFECTS_PER_UNIT)
                    .map(|s| {
                        reg.get(&effect_group(d, s), "loaded_effect")
                            .expect("effect slot control")
                    })
                    .collect(),
                effect_names: Default::default(),
                key_menu: Default::default(),
                key_menu_all: Default::default(),
                stem_names: Default::default(),
                stem_colors: Default::default(),
            }
        }))
    }

    /// The key menu's entries: the keys the deck can be shifted to, from
    /// `range` semitones down to `range` up, marking the current shift and
    /// the keys that mix with the other deck's.
    fn key_menu(&self, deck: usize, range: i32, camelot: bool) -> Vec<KeyItem> {
        let h = &self.0[deck];
        let Some(file) = Key::from_mixxx_number(h.file_key.get()) else {
            return Vec::new();
        };
        let speed = 1.0 + h.rate.get() * h.rate_range.get();
        let base = file.playing(0.0, speed, h.keylock.get_bool());
        let other = Key::from_mixxx_number(self.0[1 - deck].key.get());
        let current = h.pitch.get().round() as i32;
        (-range..=range)
            .map(|shift| {
                let key = base.transpose(shift);
                KeyItem {
                    shift,
                    text: format!(
                        "{shift:+}  {}",
                        if camelot { key.camelot() } else { key.name() }
                    )
                    .into(),
                    compatible: other.is_some_and(|o| key.compatible(o)),
                    current: shift == current,
                }
            })
            .collect()
    }

    fn info(&self, deck: usize, track: Option<&Track>, camelot: bool) -> DeckInfo {
        let h = &self.0[deck];
        let effects = kept(
            &h.effect_names,
            h.effects
                .iter()
                .map(|c| SharedString::from(effect_name(c.get())))
                .collect(),
        );
        let range = format!("±{:.0}%", h.rate_range.get() * 100.0).into();
        let rate = format!("{:+.1}%", h.rate.get() * h.rate_range.get() * 100.0).into();
        let Some(t) = track else {
            return DeckInfo {
                rate,
                range,
                effects,
                ..Default::default()
            };
        };
        let pos = h.playposition.get() * t.duration;
        let size = h.beatloop_size.get();
        // A loop made with IN and OUT shows its own length while it plays;
        // the box goes back to the beat loop size when it ends.
        let manual_beats = h
            .loop_enabled
            .get_bool()
            .then(|| t.beat_at(h.loop_end.get()) - t.beat_at(h.loop_start.get()))
            .filter(|beats| (beats - size).abs() > 0.01);
        let stems = t.audio.as_ref().and_then(|a| a.stems());
        let initials: String = t
            .artist
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect();
        DeckInfo {
            loaded: true,
            title: t.title.clone().into(),
            artist: t.artist.clone().into(),
            initials: initials.into(),
            // Filled in by the caller, which keeps the decoded images.
            cover: Default::default(),
            has_cover: false,
            bpm: if t.bpm > 0.0 {
                format!("{:.2}", h.bpm.get())
            } else {
                "--".into()
            }
            .into(),
            rate,
            range,
            grid_sections: t.grid.markers().len().max(1) as i32,
            stem_names: kept(
                &h.stem_names,
                stems.map_or(Vec::new(), |s| {
                    s.info.iter().map(|i| i.name.clone().into()).collect()
                }),
            ),
            stem_colors: kept(
                &h.stem_colors,
                stems.map_or(Vec::new(), |s| {
                    s.info
                        .iter()
                        .map(|i| slint::Color::from_argb_encoded(0xff00_0000 | i.color))
                        .collect()
                }),
            ),
            stem_status: match (&t.source, stems) {
                (Source::File(path), None) => match stems::separation(path) {
                    Some(stems::Separation::Running) => "Separating into stems…".into(),
                    Some(stems::Separation::Failed(e)) => e.into(),
                    // Done: the stems are loading.
                    Some(stems::Separation::Done) => "Loading stems…".into(),
                    None => "".into(),
                },
                _ => "".into(),
            },
            stem_failed: match (&t.source, stems) {
                (Source::File(path), None) => {
                    matches!(stems::separation(path), Some(stems::Separation::Failed(_)))
                }
                _ => false,
            },
            can_separate: matches!(t.source, Source::File(_)) && stems.is_none(),
            // The key being played, with any shift; a tag that isn't a key
            // is shown as it is.
            key: match Key::from_mixxx_number(h.key.get()) {
                Some(k) if camelot => k.camelot(),
                Some(k) => k.name(),
                None => t.key.clone(),
            }
            .into(),
            key_shift: match h.pitch.get().round() as i32 {
                0 => String::new(),
                n => format!("{n:+}"),
            }
            .into(),
            key_menu: kept(&h.key_menu, self.key_menu(deck, 3, camelot)),
            key_menu_all: kept(&h.key_menu_all, self.key_menu(deck, 6, camelot)),
            elapsed: fmt_clock(pos).into(),
            remaining: fmt_clock(t.duration - pos).into(),
            remaining_seconds: fmt_seconds(t.duration - pos, true).into(),
            total: fmt_seconds(t.duration, false).into(),
            duration: t.duration as f32,
            loop_size: loop_label(match manual_beats {
                Some(beats) => beats,
                None => size,
            }),
            progress: (pos / t.duration) as f32,
            effects,
        }
    }

    fn frame(&self, deck: usize, t: &Track, play_seconds: f64) -> Frame {
        let h = &self.0[deck];
        Frame {
            play_seconds,
            // Show more of the track when it plays faster, so synced decks'
            // beats are the same distance apart on screen.
            visible_seconds: VISIBLE_SECONDS * (1.0 + h.rate.get() * h.rate_range.get()),
            duration: t.duration,
            grid: if t.grid.is_empty() {
                steady_grid(t.bpm, t.first_beat)
            } else {
                let half = VISIBLE_SECONDS * 2.0;
                t.grid
                    .sections_in::<GRID_SECTIONS>(play_seconds - half, play_seconds + half)
                    .map(|s| s.map(|(seconds, bpm, beat)| GridSection { seconds, bpm, beat }))
            },
            cue: Some(h.cue_point.get()),
            loop_range: h
                .loop_enabled
                .get_bool()
                .then(|| (h.loop_start.get(), h.loop_end.get())),
            loop_marking: h.loop_in_pending.get_bool().then(|| h.loop_start.get()),
            hotcues: std::array::from_fn(|i| {
                let (enabled, position) = &h.hotcues[i];
                enabled.get_bool().then(|| position.get())
            }),
            gain: 1.0,
        }
    }
}

/// A deck's album art as a Slint image, made once per cover.
#[derive(Default)]
struct CoverImage(Option<(Arc<Cover>, slint::Image)>);

impl CoverImage {
    fn get(&mut self, track: Option<&Track>) -> Option<slint::Image> {
        let Some(cover) = track.and_then(|t| t.cover.clone()) else {
            self.0 = None;
            return None;
        };
        match &self.0 {
            Some((c, image)) if Arc::ptr_eq(c, &cover) => Some(image.clone()),
            _ => {
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &cover.rgba,
                    cover.width,
                    cover.height,
                );
                let image = slint::Image::from_rgba8(pixels);
                self.0 = Some((cover, image.clone()));
                Some(image)
            }
        }
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: WaveformRenderer,
    scrolling: [WaveformView; NUM_DECKS],
    overview: [WaveformView; NUM_DECKS],
    /// Each deck's track on the GPU, shared by its two views, and the
    /// waveform it was uploaded from. While a track is being analysed its
    /// waveform grows, and each new version is uploaded again.
    tracks: [Option<(Arc<GpuTrack>, Arc<Waveform>)>; NUM_DECKS],
    /// Each deck's drawn position, steadier than the engine's as read.
    clocks: [SmoothClock; NUM_DECKS],
    /// Whether a deck was moving in the last frame drawn.
    moving: bool,
    images_set: bool,
}

impl Gpu {
    fn render(
        &mut self,
        app: &AppWindow,
        decks: &Decks,
        tracks: &[Option<Arc<Track>>; NUM_DECKS],
        visual: &VisualPositions,
    ) {
        let scale = app.window().scale_factor();
        let px = |l: f32| (l * scale).round().max(1.0) as u32;
        let (ww, wh) = (px(app.get_waveform_width()), px(app.get_waveform_height()));
        let (ow, oh) = (px(app.get_overview_width()), px(app.get_overview_height()));
        let now = Instant::now();
        let mut changed = !self.images_set;
        self.moving = false;
        for (d, track) in tracks.iter().enumerate() {
            changed |= self.scrolling[d].resize(&self.device, ww, wh);
            changed |= self.overview[d].resize(&self.device, ow, oh);
            let track = track.as_ref().filter(|t| !t.waveform.is_empty());
            let gpu_track = match (track, &self.tracks[d]) {
                (None, _) => None,
                // Uploaded already, unless the waveform has grown since.
                (Some(t), Some((g, w))) if g.id() == t.id && Arc::ptr_eq(w, &t.waveform) => {
                    Some((g.clone(), w.clone()))
                }
                (Some(t), _) => Some((
                    Arc::new(GpuTrack::new(
                        &self.device,
                        &self.queue,
                        &self.renderer,
                        t.id,
                        &t.waveform.levels,
                        t.waveform.bins_per_second,
                    )),
                    t.waveform.clone(),
                )),
            };
            self.tracks[d] = gpu_track.clone();
            let gpu_track = gpu_track.map(|(g, _)| g);
            let (position, speed) = visual.position(d, now);
            let play_seconds = self.clocks[d].next(position, speed, now);
            self.moving |= track.is_some() && speed != 0.0;
            let frame = track.map(|t| decks.frame(d, t, play_seconds));
            // The jog marker turns with the same smoothed clock as the
            // waveform, set here as each frame is drawn rather than on the UI
            // timer, whose ticks beat against the display. A 33 rpm record
            // turns once every 1.8 s.
            let spin = (play_seconds / 1.8).rem_euclid(1.0) as f32 * 360.0;
            let spin = if track.is_some() { spin } else { 0.0 };
            if d == 0 {
                app.set_spin_1(spin);
            } else {
                app.set_spin_2(spin);
            }
            for view in [&mut self.scrolling[d], &mut self.overview[d]] {
                view.set_track(&self.device, &self.renderer, gpu_track.as_ref());
                view.render(
                    &self.device,
                    &self.queue,
                    &self.renderer,
                    frame.as_ref(),
                    scale,
                );
            }
        }
        if changed {
            let img = |v: &WaveformView| {
                slint::Image::try_from(v.texture()).expect("import waveform texture")
            };
            app.set_waveform_1(img(&self.scrolling[0]));
            app.set_waveform_2(img(&self.scrolling[1]));
            app.set_overview_1(img(&self.overview[0]));
            app.set_overview_2(img(&self.overview[1]));
            self.images_set = true;
        }
    }
}

/// A loop length in beats, as the loop box shows it: 4, 2.5 or 1/8.
fn loop_label(beats: f64) -> SharedString {
    if beats >= 1.0 {
        format!("{}", (beats * 100.0).round() / 100.0)
    } else {
        format!("1/{}", (1.0 / beats).round())
    }
    .into()
}
