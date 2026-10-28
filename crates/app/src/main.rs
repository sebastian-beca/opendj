//! openDJ: the Slint UI wired to the control registry, the audio engine
//! and the GPU waveform renderer.

// No console window behind the app on Windows (debug builds keep it, for
// the log).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use opendj_control::{ControlHandle, ControlRegistry};
use opendj_engine::cover::Cover;
use opendj_engine::key::Key;
use opendj_engine::phrases;
use opendj_engine::scan::scan;
use opendj_engine::stems;
use opendj_engine::synth::SAMPLE_LIBRARY_SIZE;
use opendj_engine::{
    deck_group, default_output_device, effect_group, effect_name, output_devices, sample_library,
    AudioEngine, AudioSetup, Role, Route, Source, Track, VisualPositions, Waveform,
    AVAILABLE_EFFECTS, COLOR_FX, EFFECTS_PER_UNIT, NUM_DECKS,
};
use opendj_library::Library;
use opendj_waveform::{
    steady_grid, wgpu, Frame, GpuTrack, GridSection, Mode, PhraseKind, Style, WaveformRenderer,
    WaveformView, GRID_SECTIONS,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use smooth::SmoothClock;

slint::include_modules!();

mod analysis_queue;
mod art;
mod automix;
mod browser;
mod controller_editor;
mod cover_scan;
mod coverart;
mod diagnostics;
mod files;
mod inbox;
mod lookup;
#[cfg(target_os = "macos")]
mod mac_menu;
#[cfg(target_os = "macos")]
mod mac_titlebar;
mod prefs;
mod sampler;
mod settings;
mod shortcuts;
mod smooth;
mod sources;
mod spotify;
mod stores;
mod ui_font;
mod update;
mod waves;

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
        let app = app.as_weak();
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
            if let Some(app) = app.upgrade() {
                app.global::<AudioSettings>()
                    .set_booth(setup.routes.iter().any(|r| r.role == Role::Booth));
            }
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

/// The 4-Deck layout's value of `layout` (0 is 2-Deck, 1 Simple).
const FOUR_DECK_LAYOUT: i32 = 2;

/// Seconds of audio shown across the scrolling waveform at normal speed.
/// Dragging the waveform (`Waveform` in main.slint) uses the same figure.
const VISIBLE_SECONDS: f64 = 10.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("--list-controls") {
        list_controls();
        return Ok(());
    }
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
    let library_path = settings::config_dir().map(|d| opendj_library::default_path(&d));
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
    // 2-Deck, Simple or 4-Deck, as last picked.
    app.set_layout(settings::number("layout", 0.0).clamp(0.0, FOUR_DECK_LAYOUT as f64) as i32);
    // `[Skin],layout` lets controllers switch it too (the DDJ-FLX4's
    // SHIFT + right LOAD, VIEW): 0 2-Deck, 1 Simple, 2 4-Deck. VIEW toggles,
    // so from 4-Deck it goes to 2-Deck.
    let layout_control = registry.create(
        "[Skin]",
        "layout",
        app.get_layout() as f64,
        0.0,
        FOUR_DECK_LAYOUT as f64,
        opendj_control::Behavior::Toggle,
    );
    // The deck 3 and 4 switches for two-deck controllers (see
    // opendj_controllers::LAYER_KEYS): clicking a strip's deck number in
    // 4-Deck moves that side over. Outside 4-Deck they go back to decks 1
    // and 2, which are the ones showing.
    let layers =
        opendj_controllers::LAYER_KEYS.map(|k| registry.toggle(opendj_controllers::LAYER_GROUP, k));
    let reset_layers = move |layout: i32| {
        if layout != FOUR_DECK_LAYOUT {
            layers.iter().for_each(|l| l.set_bool(false));
        }
    };
    reset_layers(app.get_layout());
    {
        let layout_control = layout_control.clone();
        let reset_layers = reset_layers.clone();
        app.on_layout_changed(move |layout| {
            layout_control.set(layout as f64);
            settings::set("layout", serde_json::json!(layout));
            reset_layers(layout);
        });
    }
    // The large waveforms, shown or hidden from the top bar.
    app.set_waveforms_visible(settings::flag("waveforms_visible", true));
    app.on_waveforms_visible_changed(|on| {
        settings::set("waveforms_visible", serde_json::json!(on))
    });
    // The FX bar, likewise.
    app.set_fx_bar_visible(settings::flag("fx_bar_visible", true));
    app.on_fx_bar_visible_changed(|on| settings::set("fx_bar_visible", serde_json::json!(on)));
    // Phrase bars on the deck overviews, shown or hidden from an
    // overview's right-click menu.
    let phrase_view = app.global::<PhraseView>();
    phrase_view.set_visible(settings::flag("phrases_visible", true));
    {
        let app_weak = app.as_weak();
        phrase_view.on_changed(move |on| {
            settings::set("phrases_visible", serde_json::json!(on));
            if let Some(app) = app_weak.upgrade() {
                app.window().request_redraw();
            }
        });
    }
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

    let controls_version = bind_controls(&app, registry.clone());
    app.global::<EffectList>()
        .set_names(ModelRc::new(VecModel::from(
            AVAILABLE_EFFECTS
                .iter()
                .map(|&n| SharedString::from(n))
                .collect::<Vec<_>>(),
        )));
    app.global::<EffectList>()
        .set_color_fx(ModelRc::new(VecModel::from(
            COLOR_FX
                .iter()
                .map(|&n| SharedString::from(n))
                .collect::<Vec<_>>(),
        )));
    app.global::<EffectList>()
        .set_mic_unit(opendj_engine::MIC_FX_UNIT as i32);
    {
        let engine = engine.clone();
        app.global::<DeckActions>()
            .on_prepare_hq_stems(move |deck| {
                let loaded = engine.loaded_tracks();
                let track = loaded.read().unwrap().get(deck as usize).cloned().flatten();
                if let Some(t) = track {
                    engine.prepare_hq_stems(&t);
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
    let sampler = sampler::Sampler::install(&app, engine.clone(), browser.clone(), &registry);
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

    // Keyboard shortcuts, each an openDJscript line.
    {
        let browser = browser.clone();
        let registry = registry.clone();
        app.on_shortcut(move |text, cmd, alt, shift, master, first, last| {
            let Some(key) = shortcuts::key_name(&text, cmd, alt, shift) else {
                return false;
            };
            let Some(script) = shortcuts::script_for(&key) else {
                return false;
            };
            let mut b = browser.borrow_mut();
            let mut host = shortcuts::AppHost {
                registry: &registry,
                browser: &mut b,
                picked: (first, last),
                master: master.clamp(0, 3) as u8 + 1,
            };
            if let Err(e) = opendj_script::run(&script, &mut host) {
                eprintln!("{key} ({script}): {e}");
            }
            true
        });
    }

    // MIDI controllers, driven by Mixxx's mappings. Started after the browser
    // so the library controls the mappings use already exist.
    let controllers = match opendj_controllers::default_mappings_dir() {
        Some(dir) => Some(Rc::new(opendj_controllers::Controllers::start(
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
    // music folder, unless the DJ turned that off. The browser then shows the
    // library as saved.
    let (scan_tx, scan_rx) = std::sync::mpsc::channel();
    if settings::scan_on_start() {
        sync_library(library_path.clone(), scan_tx.clone());
    }
    let controller_editor = controller_editor::Editor::install(&app, controllers.clone());
    prefs::apply_saved(&registry);
    let _levels = prefs::remember_levels(registry.clone());
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
        prefs::InputHooks {
            changed: {
                let engine = engine.clone();
                Box::new(move || engine.set_inputs(&settings::audio_setup()))
            },
            status: {
                let engine = engine.clone();
                Box::new(move || engine.input_status())
            },
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
                    let renderer = WaveformRenderer::new(device, queue);
                    let make = |mode| {
                        let mut v = WaveformView::new(device, mode, 1, 1);
                        v.set_style(style);
                        v
                    };
                    *gpu.borrow_mut() = Some(Gpu {
                        device: device.clone(),
                        queue: queue.clone(),
                        renderer,
                        scrolling: std::array::from_fn(|_| make(Mode::Scrolling)),
                        overview: std::array::from_fn(|_| make(Mode::Overview)),
                        tracks: Default::default(),
                        phrases: Default::default(),
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

    // The overview's tooltip: the phrase under the pointer.
    {
        let gpu = gpu.clone();
        app.global::<PhraseView>().on_describe(move |d, seconds| {
            let gpu = gpu.try_borrow().ok();
            let cached = gpu
                .as_ref()
                .and_then(|g| g.as_ref()?.phrases.get(d as usize)?.as_ref());
            cached
                .and_then(|(t, p)| phrase_tip(p, t.duration, t.bpm, seconds as f64))
                .unwrap_or_default()
        });
    }

    // A newer release: checked once per start, offered while nothing plays.
    update::install_model_fetch();
    let update_check = update::check();
    let update: Rc<RefCell<Option<update::Release>>> = Default::default();
    let installing: Rc<RefCell<Option<std::sync::mpsc::Receiver<update::Progress>>>> =
        Default::default();
    // Installed and waiting for the decks to stop before restarting.
    let update_ready = Cell::new(false);
    let play_controls: Vec<ControlHandle> = (0..NUM_DECKS)
        .filter_map(|d| registry.get(&deck_group(d), "play"))
        .collect();
    {
        let update = update.clone();
        let installing = installing.clone();
        let weak = app.as_weak();
        app.on_update_download(move || {
            let Some(release) = update.borrow().clone() else {
                return;
            };
            if release.asset.is_none() {
                spotify::open_in_browser(&release.url);
            } else if installing.borrow().is_none() {
                if let Some(app) = weak.upgrade() {
                    app.set_update_status("Starting…".into());
                }
                *installing.borrow_mut() = Some(update::install(release));
            }
        });
    }
    {
        let update = update.clone();
        let weak = app.as_weak();
        app.on_update_dismiss(move || {
            if let Some(r) = update.borrow_mut().take() {
                update::dismiss(&r.version);
            }
            if let Some(app) = weak.upgrade() {
                app.set_update_version("".into());
            }
        });
    }

    let start = Instant::now();
    let timer = slint::Timer::default();
    let covers: RefCell<[CoverImage; NUM_DECKS]> = Default::default();
    let last_tick = Cell::new(Instant::now());
    // When the alert on screen goes away.
    let alert_until: Cell<Option<Instant>> = Cell::new(None);
    let alert_engine = engine.clone();
    let layout_version = Cell::new(layout_control.version());
    // Switched off or on from the crossfader's menu, a controller or a
    // script: remembered for next time, as the Settings switch is.
    let xfader_enabled = registry.create(
        "[Mixer]",
        "crossfader_enabled",
        1.0,
        0.0,
        1.0,
        opendj_control::Behavior::Toggle,
    );
    let xfader_version = Cell::new(xfader_enabled.version());
    // What the screen last showed, to redraw only when something changed.
    let shown_controls = Cell::new(u64::MAX);
    let shown_tracks: RefCell<[Option<Arc<Track>>; NUM_DECKS]> = Default::default();
    let shown_positions: Cell<[f64; NUM_DECKS]> = Cell::new([f64::NAN; NUM_DECKS]);
    let deck_rows = Rc::new(VecModel::from(vec![DeckInfo::default(); NUM_DECKS]));
    app.set_decks(ModelRc::from(deck_rows.clone()));
    let positions = engine.visual_positions();
    {
        let app_weak = app.as_weak();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(16),
            move || {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                // The skin reads controls through `tick`; moving it makes
                // every one of them look again, so only when one changed.
                let version = controls_version();
                let controls_changed = shown_controls.replace(version) != version;
                if controls_changed {
                    app.global::<Controls>()
                        .set_tick(app.global::<Controls>().get_tick().wrapping_add(1));
                }
                app.global::<Controls>()
                    .set_blink(start.elapsed().as_millis() / 500 % 2 == 1);
                // Only when a controller moved it since the last look.
                let layout = layout_control.get() as i32;
                if layout_version.replace(layout_control.version()) != layout_control.version()
                    && layout != app.get_layout()
                {
                    app.set_layout(layout);
                    settings::set("layout", serde_json::json!(layout));
                    reset_layers(layout);
                }
                if xfader_version.replace(xfader_enabled.version()) != xfader_enabled.version() {
                    let on = xfader_enabled.get_bool();
                    if settings::flag("xfader_enabled", true) != on {
                        settings::set("xfader_enabled", serde_json::json!(on));
                    }
                }
                // The newest alert shows for a while; a click closes it.
                if let Some(message) = alert_engine.take_alerts().pop() {
                    app.set_alert(message.into());
                    alert_until.set(Some(Instant::now() + ALERT_SECONDS));
                } else if alert_until.get().is_some_and(|t| Instant::now() >= t) {
                    alert_until.set(None);
                    app.set_alert("".into());
                }
                if let Ok(release) = update_check.try_recv() {
                    app.set_update_action(
                        if release.asset.is_some() {
                            "Install & restart"
                        } else {
                            "Download"
                        }
                        .into(),
                    );
                    *update.borrow_mut() = Some(release);
                }
                let progress = installing
                    .borrow()
                    .as_ref()
                    .and_then(|rx| rx.try_recv().ok());
                match progress {
                    Some(update::Progress::Status(s)) => app.set_update_status(s.into()),
                    Some(update::Progress::Done) => update_ready.set(true),
                    Some(update::Progress::Failed(message)) => {
                        installing.take();
                        // The button opens the release page from now on.
                        if let Some(r) = update.borrow_mut().as_mut() {
                            r.asset = None;
                        }
                        app.set_update_status("".into());
                        app.set_update_action("Download".into());
                        app.set_alert(message.into());
                        alert_until.set(Some(Instant::now() + ALERT_SECONDS));
                    }
                    None => {}
                }
                let playing = play_controls.iter().any(|c| c.get() > 0.5);
                if update_ready.get() {
                    if playing {
                        app.set_update_status("Restarts when the decks stop".into());
                    } else {
                        update_ready.set(false);
                        update::relaunch();
                        let _ = slint::quit_event_loop();
                    }
                }
                // Never pops up mid-set; an install under way stays in view.
                let shown = match &*update.borrow() {
                    Some(r) if !playing || installing.borrow().is_some() => r.version.clone(),
                    _ => String::new(),
                };
                if app.get_update_version() != shown.as_str() {
                    app.set_update_version(shown.into());
                }
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
                browser.borrow_mut().poll_waves();
                browser.borrow_mut().poll_analysis();
                browser.borrow_mut().poll_usb_export();
                browser.borrow_mut().poll_covers();
                browser.borrow_mut().poll_cleanup();
                browser.borrow_mut().poll_files();
                sampler.borrow_mut().poll();
                prefs.borrow_mut().poll();
                controller_editor.borrow_mut().poll();
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
                b.set_decks_shown(if app.get_layout() == FOUR_DECK_LAYOUT {
                    4
                } else {
                    2
                });
                b.set_loaded(
                    &app,
                    std::array::from_fn(|d| tracks[d].as_ref().map(|t| t.id)),
                );
                b.learn_deck_phrases(&tracks);
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
                // Row by row and only when it differs: a whole new model
                // would make everything on the decks lay out and draw again.
                for (d, track) in tracks.iter().enumerate() {
                    let track = track.as_deref();
                    let mut info = decks.info(d, track, camelot);
                    if let Some(image) = covers.borrow_mut()[d].get(track) {
                        info.cover = image;
                        info.has_cover = true;
                    }
                    if !deck_rows
                        .row_data(d)
                        .is_some_and(|old| same_deck_info(&old, &info))
                    {
                        deck_rows.set_row_data(d, info);
                    }
                }
                let t = start.elapsed().as_secs();
                let clock = format!("{:02}:{:02}", t / 60 % 60, t % 60);
                if app.get_clock() != clock.as_str() {
                    app.set_clock(clock.into());
                }
                // The waveforms are drawn outside Slint, so it can't tell
                // when they change: redraw when a deck moved, a track or its
                // waveform changed, or a control did (cues, loops, zoom).
                // While a deck plays, each frame asks for the next itself.
                let now = Instant::now();
                let at: [f64; NUM_DECKS] = std::array::from_fn(|d| positions.position(d, now).0);
                let moved = at != shown_positions.replace(at);
                let mut shown = shown_tracks.borrow_mut();
                let loaded_changed = shown.iter().zip(tracks.iter()).any(|(a, b)| match (a, b) {
                    (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                    (None, None) => false,
                    _ => true,
                });
                if loaded_changed {
                    *shown = tracks.clone();
                }
                if controls_changed || moved || loaded_changed {
                    app.window().request_redraw();
                }
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
                        let prefs = app.global::<Preferences>();
                        if !prefs.get_open() {
                            prefs.invoke_show(prefs.get_page());
                        }
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
/// in the music folders, and the library of any app the DJ imports from whose
/// files have changed since last time. The tracks as saved are then sent to `tx`,
/// for the browser to show.
fn sync_library(library_path: Option<PathBuf>, tx: std::sync::mpsc::Sender<Vec<Arc<Track>>>) {
    let music: Vec<PathBuf> = settings::music_folders()
        .into_iter()
        .filter(|d| d.is_dir())
        .collect();
    let folders = settings::stage_folders();
    // The music folders and the stage folders, each folder once: one inside
    // another (Downloads/Soulseek, a stage folder in a music folder) is
    // scanned with it.
    let mut wanted: Vec<PathBuf> = Vec::new();
    for dir in music.iter().cloned().chain(
        folders
            .all()
            .into_iter()
            .filter(|d| d.is_dir())
            .map(Path::to_path_buf),
    ) {
        if !wanted.contains(&dir) {
            wanted.push(dir);
        }
    }
    let dirs: Vec<PathBuf> = wanted
        .iter()
        .filter(|dir| !wanted.iter().any(|d| d != *dir && dir.starts_with(d)))
        .cloned()
        .collect();
    let move_files = settings::move_files();
    let importers = stores::enabled_importers();
    // Imports the DJ removed in Settings, to take out before anything else.
    let removed: Vec<String> = stores::importers()
        .iter()
        .map(|i| i.id().to_string())
        .filter(|id| settings::flag(&settings::import_remove_key(id), false))
        .collect();
    if dirs.is_empty() && importers.is_empty() && removed.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let mut scanned = Vec::new();
        for dir in &dirs {
            let next = SAMPLE_LIBRARY_SIZE + scanned.len();
            scanned.extend(scan(dir, next));
        }
        let synced = library_path
            .as_deref()
            .map(|path| -> Result<Vec<Track>, String> {
                let mut db = Library::open(path).map_err(|e| e.to_string())?;
                for id in &removed {
                    // Files in the music folders stay, as a scan would bring them in again.
                    let keep = |p: &Path| music.iter().any(|m| p.starts_with(m));
                    match db.remove_import(id, keep) {
                        Ok(r) => eprintln!(
                            "{id}: import removed: {} lists, {} tracks; {} tracks kept",
                            r.lists, r.tracks, r.kept
                        ),
                        Err(e) => eprintln!("can't remove the {id} import: {e}"),
                    }
                    settings::set(&settings::import_remove_key(id), serde_json::json!(false));
                }
                let scanned: Vec<Track> = scanned.iter().map(|t| (**t).clone()).collect();
                let report = db.sync(&scanned).map_err(|e| e.to_string())?;
                if report.added + report.updated > 0 {
                    eprintln!(
                        "library: {} tracks added, {} updated",
                        report.added, report.updated
                    );
                }
                for importer in &importers {
                    let Some(database) = settings::import_database(importer.as_ref()) else {
                        continue;
                    };
                    let groups = settings::import_groups(importer.as_ref());
                    let theirs = settings::import_theirs_win(importer.id());
                    // Changing what comes in, or whose tags win, brings it in again.
                    let stamp = format!(
                        "{}|{}|{theirs}",
                        importer.stamp(&database),
                        groups.join(",")
                    );
                    match db.import_with(importer.id(), importer.title(), &stamp, theirs, || {
                        importer.read(&database, &groups)
                    }) {
                        Ok(Some(r)) => eprintln!(
                            "{}: {} tracks added, {} updated; {} lists, folders and \
                             sessions ({} entries without a file here left out)",
                            importer.id(),
                            r.added,
                            r.updated,
                            r.lists,
                            r.unmatched
                        ),
                        Ok(None) => {}
                        Err(e) => eprintln!("can't import {}: {e}", database.display()),
                    }
                }
                match db.assign_stages(&folders) {
                    Ok(0) => {}
                    Ok(n) => eprintln!("library: {n} tracks given a stage by their folder"),
                    Err(e) => eprintln!("can't set stages: {e}"),
                }
                if move_files {
                    match db.apply_moves() {
                        Ok(r) => {
                            if r.moved > 0 {
                                eprintln!("library: {} files moved between stage folders", r.moved);
                            }
                            for (path, why) in r.failed {
                                eprintln!("can't move {}: {why}", path.display());
                            }
                        }
                        Err(e) => eprintln!("can't move files: {e}"),
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

/// Prints every control openDJ has, aliases included, as sorted `[Group],key`
/// lines. The website's controller pages use this list to mark mapping lines
/// that don't do anything in openDJ yet (website/book/supported-controls.txt).
fn list_controls() {
    let registry = ControlRegistry::new();
    opendj_engine::create_controls(&registry);
    browser::LibraryControls::create(&registry);
    automix::AutoMix::new(&registry);
    let mut keys = registry.keys();
    keys.extend(registry.alias_keys());
    keys.sort();
    for k in keys {
        println!("{},{}", k.group, k.item);
    }
}

/// Connects the `Controls` global used by every skin widget to the registry.
/// Names are resolved to an index once; per-frame reads are atomic loads.
/// Resolved controls, indexed by the number handed to the UI.
#[derive(Default)]
struct Resolved {
    handles: Vec<ControlHandle>,
    by_name: HashMap<(String, String), i32>,
}

/// Hooks the skin's controls up to the registry. Returns a count that goes
/// up whenever any control the skin shows changes, so the UI only looks
/// again (and redraws) when something did.
fn bind_controls(app: &AppWindow, registry: Arc<ControlRegistry>) -> impl Fn() -> u64 {
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
    move || {
        let h = handles.borrow();
        // Each version only goes up, so their sum moves when any one does;
        // a newly resolved control adds to it too.
        h.handles.iter().map(|c| c.version() as u64).sum::<u64>() + h.handles.len() as u64
    }
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

/// How long an alert stays on screen.
const ALERT_SECONDS: Duration = Duration::from_secs(8);

/// A deck's playing speed from its tempo fader (`rate`), range and
/// direction, as the engine works it out.
pub(crate) fn speed(rate: &ControlHandle, range: &ControlHandle, dir: &ControlHandle) -> f64 {
    let dir = if dir.get() < 0.0 { -1.0 } else { 1.0 };
    1.0 + rate.get() * range.get() * dir
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
    rate_dir: ControlHandle,
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
                rate_dir: c("rate_dir"),
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
        let speed = speed(&h.rate, &h.rate_range, &h.rate_dir);
        let base = file.playing(0.0, speed, h.keylock.get_bool());
        // The deck it pairs with: 1 with 2, 3 with 4.
        let other = Key::from_mixxx_number(self.0[deck ^ 1].key.get());
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
        let rate = format!(
            "{:+.1}%",
            (speed(&h.rate, &h.rate_range, &h.rate_dir) - 1.0) * 100.0
        )
        .into();
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
            stem_status: match &t.source {
                Source::File(path) => match stems::separation(path) {
                    Some(stems::Separation::Failed(e)) => e.into(),
                    _ => "".into(),
                },
                _ => "".into(),
            },
            stem_hq: stems.is_some_and(|s| s.playing_quality() == stems::Quality::High),
            stem_preparing: match &t.source {
                // Running, or done and the stems still loading.
                Source::File(path) => {
                    matches!(
                        stems::separation(path),
                        Some(stems::Separation::Running | stems::Separation::Done)
                    ) && !stems.is_some_and(|s| s.playing_quality() == stems::Quality::High)
                }
                _ => false,
            },
            can_prepare_hq: match &t.source {
                Source::File(path) => {
                    !stems.is_some_and(|s| s.playing_quality() == stems::Quality::High)
                        && !matches!(
                            stems::separation(path),
                            Some(stems::Separation::Running | stems::Separation::Done)
                        )
                }
                _ => false,
            },
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

    fn frame(&self, deck: usize, t: &Track, play_seconds: f64, phrases: Phrases) -> Frame {
        let h = &self.0[deck];
        Frame {
            play_seconds,
            // Show more of the track when it plays faster, so synced decks'
            // beats are the same distance apart on screen.
            visible_seconds: VISIBLE_SECONDS * speed(&h.rate, &h.rate_range, &h.rate_dir),
            duration: t.duration,
            first_bar_beat: first_bar_beat(t),
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
            phrases,
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

/// Whether two deck infos show the same. Their lists are new models each
/// time, which never compare equal, so they are compared by their rows; and
/// Slint never finds two empty images equal, so those are compared by size.
fn same_deck_info(old: &DeckInfo, new: &DeckInfo) -> bool {
    fn same<T: Clone + PartialEq + 'static>(a: &ModelRc<T>, b: &ModelRc<T>) -> bool {
        a.row_count() == b.row_count() && a.iter().eq(b.iter())
    }
    let lists_same = same(&old.key_menu, &new.key_menu)
        && same(&old.key_menu_all, &new.key_menu_all)
        && same(&old.stem_names, &new.stem_names)
        && same(&old.stem_colors, &new.stem_colors)
        && same(&old.effects, &new.effects);
    let no_cover = |i: &slint::Image| i.size() == Default::default();
    let covers_same = old.cover == new.cover || (no_cover(&old.cover) && no_cover(&new.cover));
    // Everything else, with the lists and covers (compared above) the same.
    let blank = slint::Image::from_rgba8(slint::SharedPixelBuffer::new(1, 1));
    let rest = |i: &DeckInfo| DeckInfo {
        key_menu: Default::default(),
        key_menu_all: Default::default(),
        stem_names: Default::default(),
        stem_colors: Default::default(),
        effects: Default::default(),
        cover: blank.clone(),
        ..i.clone()
    };
    lists_same && covers_same && rest(old) == rest(new)
}

/// Where each phrase starts and its kind, as the waveforms draw them.
type Phrases = Vec<(f64, PhraseKind)>;

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
    /// Each deck's phrases and the track they were found for, found again
    /// when the track changes (a new analysis or a grid edit replaces it).
    phrases: [Option<(Arc<Track>, Phrases)>; NUM_DECKS],
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
        let four = app.get_layout() == FOUR_DECK_LAYOUT;
        let show_phrases = app.global::<PhraseView>().get_visible();
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
            let phrases = match track {
                Some(t) if show_phrases => self.phrases(d, t),
                _ => Vec::new(),
            };
            let frame = track.map(|t| decks.frame(d, t, play_seconds, phrases));
            // The jog marker turns with the same smoothed clock as the
            // waveform, set here as each frame is drawn rather than on the UI
            // timer, whose ticks beat against the display. A 33 rpm record
            // turns once every 1.8 s.
            let spin = (play_seconds / 1.8).rem_euclid(1.0) as f32 * 360.0;
            let spin = if track.is_some() { spin } else { 0.0 };
            match d {
                0 => app.set_spin_1(spin),
                1 => app.set_spin_2(spin),
                // Decks 3 and 4 have no jog.
                _ => {}
            }
            // Decks 3 and 4 show only in 4-Deck.
            if d >= 2 && !four {
                continue;
            }
            // Phrases show only on the deck's overview.
            let scrolling_frame = frame.as_ref().map(|f| Frame {
                phrases: Vec::new(),
                ..f.clone()
            });
            for (view, frame) in [
                (&mut self.scrolling[d], scrolling_frame.as_ref()),
                (&mut self.overview[d], frame.as_ref()),
            ] {
                view.set_track(&self.device, &self.renderer, gpu_track.as_ref());
                view.render(&self.device, &self.queue, &self.renderer, frame, scale);
            }
        }
        if changed {
            let img = |v: &WaveformView| {
                slint::Image::try_from(v.texture()).expect("import waveform texture")
            };
            app.set_waveform_1(img(&self.scrolling[0]));
            app.set_waveform_2(img(&self.scrolling[1]));
            app.set_waveform_3(img(&self.scrolling[2]));
            app.set_waveform_4(img(&self.scrolling[3]));
            app.set_overview_1(img(&self.overview[0]));
            app.set_overview_2(img(&self.overview[1]));
            app.set_overview_3(img(&self.overview[2]));
            app.set_overview_4(img(&self.overview[3]));
            self.images_set = true;
        }
    }
}

impl Gpu {
    /// Deck `d`'s phrases for the waveforms, found once per track: the
    /// library's until its waveform is ready, then found from the waveform
    /// and the grid as they are now.
    fn phrases(&mut self, d: usize, t: &Arc<Track>) -> Phrases {
        match &self.phrases[d] {
            Some((cached, p)) if Arc::ptr_eq(cached, t) => p.clone(),
            _ => {
                let found = if phrases::waveform_ready(t) {
                    phrases::detect(t)
                } else {
                    phrases::stored(t)
                };
                let p: Vec<_> = found
                    .into_iter()
                    .map(|p| {
                        let kind = match p.kind {
                            phrases::PhraseKind::Intro => PhraseKind::Intro,
                            phrases::PhraseKind::Build => PhraseKind::Build,
                            phrases::PhraseKind::Drop => PhraseKind::Drop,
                            phrases::PhraseKind::Breakdown => PhraseKind::Breakdown,
                            phrases::PhraseKind::Outro => PhraseKind::Outro,
                            phrases::PhraseKind::Chorus => PhraseKind::Chorus,
                        };
                        (p.start, kind)
                    })
                    .collect();
                self.phrases[d] = Some((t.clone(), p.clone()));
                p
            }
        }
    }
}

/// The tooltip for the phrase at `seconds`: its kind, then its length in
/// bars and where it starts and ends.
fn phrase_tip(phrases: &Phrases, duration: f64, bpm: f64, seconds: f64) -> Option<PhraseTip> {
    let i = phrases.iter().rposition(|(start, _)| *start <= seconds)?;
    let (start, kind) = phrases[i];
    let end = phrases.get(i + 1).map_or(duration, |(s, _)| *s);
    let title = match kind {
        PhraseKind::Intro => "INTRO",
        PhraseKind::Build => "BUILD-UP",
        PhraseKind::Drop => "DROP",
        PhraseKind::Breakdown => "BREAKDOWN",
        PhraseKind::Outro => "OUTRO",
        PhraseKind::Chorus => "CHORUS",
    };
    let mmss = |s: f64| {
        let s = s.max(0.0).round() as u64;
        format!("{}:{:02}", s / 60, s % 60)
    };
    let range = format!("{} – {}", mmss(start), mmss(end));
    let bars = ((end - start) * bpm / 240.0).round();
    let body = if bpm > 0.0 && bars >= 1.0 {
        format!("{bars} bars · {range}")
    } else {
        range
    };
    Some(PhraseTip {
        title: title.into(),
        body: body.into(),
    })
}

/// The beat number of the track's first bar: the first bar line at or
/// after its start, on the grid the waveform draws (bar lines every fourth
/// beat from beat 0).
fn first_bar_beat(t: &Track) -> f64 {
    let at_start = if t.grid.is_empty() {
        if t.bpm <= 0.0 {
            return 0.0;
        }
        -t.first_beat * t.bpm / 60.0
    } else {
        t.grid.beat_at(0.0)
    };
    // A hair of tolerance so a bar right at the start is bar 1.
    (at_start / 4.0 - 1e-6).ceil() * 4.0
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
