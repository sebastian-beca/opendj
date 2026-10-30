//! Sources in the rail that come from add-ons (see
//! [`opendj_plugins::source`]): saved searches on the left, what they found
//! in the list, and downloads that join the collection when they finish.
//!
//! Each source runs on a thread of its own, which also asks how searches
//! and downloads are getting on about once a second, so a slow network
//! never holds up the UI or the audio.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use opendj_engine::Track;
use opendj_plugins::inbox::safe_name;
use opendj_plugins::source::{
    megabytes, DownloadUpdate, Found, Prefer, Progress, QueryKind, SearchSource, Snapshot,
    SourceInfo, SourceQuery, DOWNLOAD_FOLDER,
};
use opendj_plugins::{Plugin, PluginConfig, PluginError};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::{AppWindow, FoundRow, QueueLine, SearchItem, SourceTab, SourceUi, StoreUi};

/// Where plugin sources start in the rail, after the built-in ones.
pub const FIRST_SOURCE: i32 = crate::browser::SOURCE_NAMES.len() as i32;

/// How often searches and downloads are asked how they're getting on.
const TICK: Duration = Duration::from_millis(900);

/// A square picture's side length and RGBA pixels.
type Thumbnail = (u32, Vec<u8>);

/// How big a result's cover is fetched, in pixels (shown at 40).
const COVER_SIDE: u32 = 96;

/// Looks in a row (about a second apart) a playlist entry's results must
/// stay the same before its best copy is downloaded by itself.
const SETTLED: u32 = 3;

/// Formats that count as lossless for "Lossless only".
const LOSSLESS: [&str; 5] = ["FLAC", "WAV", "AIFF", "ALAC", "APE"];

/// The plugins with a source of their own, and what each is called.
pub fn source_plugins() -> Vec<(Box<dyn Plugin>, SourceInfo)> {
    crate::stores::configured()
        .into_iter()
        .filter_map(|mut p| {
            let info = p.source()?.info();
            Some((p, info))
        })
        .collect()
}

enum Request {
    Start(u64, SourceQuery),
    Stop(u64),
    OpenFolder(u64, Box<Found>),
    Download(u64, Vec<Found>, PathBuf),
    Retry(String),
    Cancel(String),
    /// The DJ changed the plugin's settings.
    Configure(PluginConfig),
}

enum Event {
    /// What search `local` has found so far.
    Found(usize, u64, Snapshot),
    /// Search `local` couldn't start or went wrong.
    SearchFailed(usize, u64, String),
    /// A folder's files, read when the DJ opened it.
    Folder(usize, u64, String, Vec<Found>),
    Download(usize, DownloadUpdate),
    /// A finished download joined the library.
    Imported(Box<Track>),
}

/// One source's thread: searches by the app's own numbers, mapped to the
/// add-on's ids once it has started them.
fn run(
    mut plugin: Box<dyn Plugin>,
    index: usize,
    library: Option<PathBuf>,
    rx: Receiver<Request>,
    tx: Sender<Event>,
) {
    let Some(name) = plugin.source().map(|s| s.info().name) else {
        return;
    };
    let mut ids: HashMap<u64, String> = HashMap::new();
    let mut active: HashSet<u64> = HashSet::new();
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(TICK) {
            Ok(Request::Configure(config)) => plugin.configure(&config),
            Ok(request) => {
                let source = plugin.source().expect("a source");
                if !handle(source, index, request, &mut ids, &mut active, &tx) {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if last.elapsed() < TICK {
            continue;
        }
        last = Instant::now();
        let source = plugin.source().expect("a source");
        for local in active.clone() {
            let Some(id) = ids.get(&local) else { continue };
            let event = match source.results(id) {
                Ok(snap) => {
                    if snap.done {
                        active.remove(&local);
                    }
                    Event::Found(index, local, snap)
                }
                Err(e) => {
                    active.remove(&local);
                    Event::SearchFailed(index, local, e.to_string())
                }
            };
            if tx.send(event).is_err() {
                return;
            }
        }
        for mut update in source.progress() {
            if let Progress::Done(paths) = &update.progress {
                for path in paths {
                    let Some(library) = &library else { break };
                    match crate::inbox::add_download(library, path, name) {
                        Ok(track) => {
                            let _ = tx.send(Event::Imported(Box::new(track)));
                        }
                        Err(e) => eprintln!("{name}: {}: {e}", path.display()),
                    }
                }
                update.progress = Progress::Done(paths.clone());
            }
            if tx.send(Event::Download(index, update)).is_err() {
                return;
            }
        }
    }
}

/// Does one request; false when the app has gone.
fn handle(
    source: &mut dyn SearchSource,
    index: usize,
    request: Request,
    ids: &mut HashMap<u64, String>,
    active: &mut HashSet<u64>,
    tx: &Sender<Event>,
) -> bool {
    let event = match request {
        Request::Start(local, query) => match source.start(&query) {
            Ok(id) => {
                ids.insert(local, id);
                active.insert(local);
                None
            }
            Err(e) => Some(Event::SearchFailed(index, local, e.to_string())),
        },
        Request::Stop(local) => {
            active.remove(&local);
            if let Some(id) = ids.remove(&local) {
                source.stop(&id);
            }
            None
        }
        Request::OpenFolder(local, folder) => {
            let folder = *folder;
            let id = ids.get(&local).cloned().unwrap_or_default();
            match source.open_folder(&id, &folder) {
                Ok(tracks) => Some(Event::Folder(index, local, folder.key, tracks)),
                Err(e) => {
                    eprintln!("can't open {}: {e}", folder.title);
                    None
                }
            }
        }
        Request::Download(local, items, dir) => {
            let id = ids.get(&local).cloned().unwrap_or_default();
            let _ = std::fs::create_dir_all(&dir);
            match source.download(&id, &items, &dir) {
                Ok(()) => None,
                Err(e) => {
                    for item in items {
                        let failed = DownloadUpdate {
                            key: item.key,
                            progress: Progress::Failed(e.to_string()),
                        };
                        if tx.send(Event::Download(index, failed)).is_err() {
                            return false;
                        }
                    }
                    None
                }
            }
        }
        Request::Configure(_) => None,
        Request::Cancel(key) => {
            if let Err(e) = source.cancel(&key) {
                eprintln!("can't cancel {key}: {e}");
            }
            None
        }
        Request::Retry(key) => match source.retry(&key) {
            Ok(()) => None,
            Err(e) => Some(Event::Download(
                index,
                DownloadUpdate {
                    key,
                    progress: Progress::Failed(match e {
                        PluginError::NotSupported => "Can't try again.".into(),
                        e => e.to_string(),
                    }),
                },
            )),
        },
    };
    event.is_none_or(|e| tx.send(e).is_ok())
}

/// A saved search and what it has found.
struct Search {
    local: u64,
    query: SourceQuery,
    /// When the DJ added it (Unix milliseconds); the newest is listed first.
    added: u64,
    items: Vec<Found>,
    done: bool,
    note: String,
    /// What the source calls it, e.g. a playlist's name; empty for none.
    title: String,
    error: Option<String>,
    /// Folders the DJ has opened, by key, and playlist entries showing all
    /// their copies, by [`song_key`].
    open: HashSet<String>,
    /// Download each playlist entry's best copy once its results settle,
    /// and its next copy when one fails. For this run only, not saved.
    auto: bool,
    /// Each entry's results at the last looks, and how many looks in a
    /// row they've stayed the same.
    settling: HashMap<String, (Vec<String>, u32)>,
    /// Entries kept from before "Search again", which isn't looking for
    /// them.
    kept: Vec<Found>,
}

impl Search {
    /// What the search is listed by: the source's name for it, else the
    /// query.
    fn name(&self) -> &str {
        if self.title.is_empty() {
            &self.query.text
        } else {
            &self.title
        }
    }

    fn find(&self, key: &str) -> Option<&Found> {
        self.items.iter().find_map(|f| {
            if f.key == key {
                Some(f)
            } else {
                f.tracks.iter().find(|t| t.key == key)
            }
        })
    }

    /// The folder a file is in, if it's in one.
    fn parent(&self, key: &str) -> Option<&Found> {
        self.items
            .iter()
            .find(|f| f.folder && f.tracks.iter().any(|t| t.key == key))
    }
}

struct Download {
    key: String,
    /// The search it came from, by the app's number.
    local: u64,
    label: String,
    progress: Progress,
}

/// Now, in Unix milliseconds.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// One plugin source: its searches and downloads.
struct Source {
    /// The plugin's id, as its settings are saved under.
    id: &'static str,
    info: SourceInfo,
    requests: Sender<Request>,
    searches: Vec<Search>,
    downloads: Vec<Download>,
}

impl Source {
    fn download(&self, key: &str) -> Option<&Download> {
        self.downloads.iter().rev().find(|d| d.key == key)
    }

    fn save(&self) {
        let saved: Vec<_> = self
            .searches
            .iter()
            .map(|s| {
                (
                    s.query.text.clone(),
                    s.query.kind.name().to_string(),
                    s.query.prefer.name().to_string(),
                    s.added,
                )
            })
            .collect();
        crate::settings::set_source_searches(self.info.name, &saved);
    }
}

/// What a row of the list is.
#[derive(Clone, PartialEq)]
enum Row {
    /// "Albums", "Tracks", or a playlist entry (by its group), whose other
    /// copies show under it when it's open.
    Heading(Option<String>),
    /// A result, by its key.
    Item(String),
    /// A download in the Downloads list, by its index.
    Download(usize),
}

pub struct Sources {
    app: slint::Weak<AppWindow>,
    /// Results' covers fetched so far, by address, and those asked for.
    covers: HashMap<String, slint::Image>,
    asked: HashSet<String>,
    cover_requests: Sender<String>,
    cover_events: Receiver<(String, Option<Thumbnail>)>,
    sources: Vec<Source>,
    events: Receiver<Event>,
    next_local: u64,
    /// The source shown, if a plugin source is picked in the rail.
    shown: Option<usize>,
    /// The search shown, or the Downloads list.
    search: Option<usize>,
    /// 0 = everything, 1 = single tracks, 2 = albums.
    filter: i32,
    lossless_only: bool,
    /// Ticked results, by key.
    checked: HashSet<String>,
    /// The row ticked last, for shift-click.
    anchor: Option<usize>,
    rows: Vec<Row>,
    /// The music folder; downloads go to `Downloaded/<source>/` in it
    /// unless the source's settings name a folder.
    music: Option<PathBuf>,
    list: Rc<VecModel<FoundRow>>,
    /// A copy of each source for the UI thread, to describe queries
    /// while the DJ types without waiting on the source's own thread.
    describers: Vec<Box<dyn Plugin>>,
}

impl Sources {
    /// Starts each source with its saved searches, and wires up the UI.
    pub fn install(app: &AppWindow, library: Option<PathBuf>) -> Rc<std::cell::RefCell<Self>> {
        let (tx, events) = channel();
        let sources: Vec<Source> = source_plugins()
            .into_iter()
            .enumerate()
            .map(|(index, (plugin, info))| {
                let id = plugin.manifest().id;
                let (requests, rx) = channel();
                let (tx, library) = (tx.clone(), library.clone());
                std::thread::Builder::new()
                    .name(format!("source {}", info.name))
                    .spawn(move || run(plugin, index, library, rx, tx))
                    .expect("spawn source thread");
                Source {
                    id,
                    info,
                    requests,
                    searches: Vec::new(),
                    downloads: Vec::new(),
                }
            })
            .collect();
        let list = Rc::new(VecModel::default());
        let (cover_requests, wanted) = channel::<String>();
        let (covers_tx, cover_events) = channel();
        std::thread::Builder::new()
            .name("source covers".into())
            .spawn(move || {
                for url in wanted {
                    let thumb = crate::coverart::thumbnail(&url, COVER_SIDE);
                    if covers_tx.send((url, thumb)).is_err() {
                        return;
                    }
                }
            })
            .expect("spawn source covers thread");
        let mut me = Self {
            app: app.as_weak(),
            covers: HashMap::new(),
            asked: HashSet::new(),
            cover_requests,
            cover_events,
            sources,
            events,
            next_local: 0,
            shown: None,
            search: None,
            filter: 0,
            lossless_only: false,
            checked: HashSet::new(),
            anchor: None,
            rows: Vec::new(),
            music: opendj_engine::scan::default_music_dir(),
            list: list.clone(),
            describers: source_plugins().into_iter().map(|(p, _)| p).collect(),
        };
        // Saved searches run again when the app starts.
        for s in 0..me.sources.len() {
            let saved = crate::settings::source_searches(me.sources[s].info.name);
            for (text, kind, prefer, added) in saved {
                let query = SourceQuery {
                    text,
                    kind: QueryKind::from_name(&kind),
                    prefer: Prefer::from_name(&prefer),
                    skip: Vec::new(),
                };
                me.start(s, query, added, false);
            }
        }
        let ui = app.global::<SourceUi>();
        let tabs: Vec<SourceTab> = me
            .sources
            .iter()
            .map(|s| SourceTab {
                name: s.info.name.into(),
                icon: slint::Image::load_from_svg_data(s.info.icon_svg).unwrap_or_default(),
                hint: s.info.hint.into(),
            })
            .collect();
        ui.set_sources(ModelRc::new(VecModel::from(tabs)));
        ui.set_rows(list.into());
        let me = Rc::new(std::cell::RefCell::new(me));
        let weak = Rc::downgrade(&me);
        let with = move |f: Box<dyn Fn(&mut Sources)>| {
            if let Some(s) = weak.upgrade() {
                f(&mut s.borrow_mut());
            }
        };
        let w = with.clone();
        ui.on_describe(move |text, kind, prefer| {
            let query = query(&text, kind, prefer);
            w(Box::new(move |s| s.describe(&query)))
        });
        let w = with.clone();
        ui.on_search(move |text, kind, prefer, auto| {
            let query = query(&text, kind, prefer);
            w(Box::new(move |s| s.add_search(query.clone(), auto)))
        });
        let w = with.clone();
        ui.on_select_search(move |i| w(Box::new(move |s| s.select(i))));
        let w = with.clone();
        ui.on_remove_search(move |i| w(Box::new(move |s| s.remove(i))));
        let w = with.clone();
        ui.on_clear_searches(move || w(Box::new(|s| s.clear_searches())));
        let w = with.clone();
        ui.on_search_again(move |i| w(Box::new(move |s| s.again(i))));
        let w = with.clone();
        ui.on_check(move |row, shift| w(Box::new(move |s| s.check(row as usize, shift))));
        let w = with.clone();
        ui.on_toggle_open(move |row| w(Box::new(move |s| s.toggle_open(row as usize))));
        let w = with.clone();
        ui.on_download(move |row| w(Box::new(move |s| s.download_row(row as usize))));
        let w = with.clone();
        ui.on_remove_download(move |row| w(Box::new(move |s| s.remove_download(row as usize))));
        let w = with.clone();
        ui.on_retry_failed(move || w(Box::new(|s| s.retry_failed())));
        let w = with.clone();
        ui.on_clear_finished(move || w(Box::new(|s| s.clear_finished())));
        let w = with.clone();
        ui.on_cancel_downloads(move || w(Box::new(|s| s.cancel_downloads())));
        let w = with.clone();
        ui.on_download_selected(move || w(Box::new(|s| s.download_selected())));
        let w = with.clone();
        ui.on_pick_best(move || w(Box::new(|s| s.pick_best())));
        let w = with.clone();
        ui.on_clear_selection(move || {
            w(Box::new(|s| {
                s.checked.clear();
                s.refresh();
            }))
        });
        let w = with.clone();
        ui.on_set_filter(move |f| {
            w(Box::new(move |s| {
                s.filter = f;
                s.refresh();
            }))
        });
        let w = with.clone();
        ui.on_set_lossless_only(move |on| {
            w(Box::new(move |s| {
                s.lossless_only = on;
                s.refresh();
            }))
        });
        me
    }

    /// Takes in what the sources have done, and follows the rail. Called
    /// every frame; returns tracks that joined the library.
    /// The DJ changed plugin `id`'s settings: its source hears of it.
    pub fn configure(&mut self, id: &str) {
        let config = crate::settings::plugin_config(id);
        for source in self.sources.iter().filter(|s| s.id == id) {
            let _ = source.requests.send(Request::Configure(config.clone()));
        }
        for p in self.describers.iter_mut().filter(|p| p.manifest().id == id) {
            p.configure(&config);
        }
    }

    pub fn poll(&mut self) -> Vec<Track> {
        let mut imported = Vec::new();
        let mut changed = false;
        for (url, thumb) in self.cover_events.try_iter().collect::<Vec<_>>() {
            if let Some((side, rgba)) = thumb {
                let pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    &rgba, side, side,
                );
                self.covers.insert(url, slint::Image::from_rgba8(pixels));
                changed = true;
            }
        }
        for event in self.events.try_iter().collect::<Vec<_>>() {
            changed = true;
            match event {
                Event::Found(s, local, snap) => {
                    if let Some(search) = self.search_mut(s, local) {
                        // Results mean it's working, whatever went wrong
                        // before.
                        search.error = None;
                        // Opened folders keep the files read for them.
                        let mut items = snap.items;
                        for item in items.iter_mut().filter(|i| i.folder) {
                            if let Some(old) = search.items.iter().find(|o| o.key == item.key) {
                                if old.tracks.len() > item.tracks.len() {
                                    item.tracks = old.tracks.clone();
                                }
                            }
                        }
                        if !search.kept.is_empty() {
                            items = with_kept(&search.kept, items);
                        }
                        if search.auto {
                            settle(&mut search.settling, &items);
                        }
                        search.items = items;
                        search.done = snap.done;
                        search.note = snap.note;
                        if !snap.title.is_empty() {
                            search.title = snap.title;
                        }
                    }
                }
                Event::SearchFailed(s, local, error) => {
                    if let Some(search) = self.search_mut(s, local) {
                        search.done = true;
                        search.error = Some(error);
                    }
                }
                Event::Folder(s, local, key, tracks) => {
                    if let Some(search) = self.search_mut(s, local) {
                        if let Some(f) = search.items.iter_mut().find(|f| f.key == key) {
                            f.tracks = tracks;
                        }
                    }
                }
                Event::Download(s, update) => {
                    if let Some(d) = self.sources[s]
                        .downloads
                        .iter_mut()
                        .rev()
                        .find(|d| d.key == update.key)
                    {
                        d.progress = update.progress;
                    }
                }
                Event::Imported(track) => imported.push(*track),
            }
        }
        if changed {
            for s in 0..self.sources.len() {
                for i in 0..self.sources[s].searches.len() {
                    self.auto_download(s, i);
                }
            }
        }
        if let Some(app) = self.app.upgrade() {
            let picked = app.global::<StoreUi>().get_source();
            let shown = (picked >= FIRST_SOURCE)
                .then(|| (picked - FIRST_SOURCE) as usize)
                .filter(|&s| s < self.sources.len());
            if shown != self.shown {
                self.shown = shown;
                self.search = shown
                    .filter(|&s| !self.sources[s].searches.is_empty())
                    .map(|_| 0);
                self.checked.clear();
                changed = true;
            }
        }
        if changed {
            self.refresh();
        }
        imported
    }

    fn search_mut(&mut self, s: usize, local: u64) -> Option<&mut Search> {
        self.sources
            .get_mut(s)?
            .searches
            .iter_mut()
            .find(|x| x.local == local)
    }

    /// Starts a search and lists it by when it was added, newest first;
    /// returns its place in the list.
    fn start(&mut self, s: usize, query: SourceQuery, added: u64, auto: bool) -> usize {
        self.next_local += 1;
        let local = self.next_local;
        let _ = self.sources[s]
            .requests
            .send(Request::Start(local, query.clone()));
        let searches = &mut self.sources[s].searches;
        let at = place(searches, added);
        searches.insert(
            at,
            Search {
                local,
                query,
                added,
                items: Vec::new(),
                done: false,
                note: String::new(),
                title: String::new(),
                error: None,
                open: HashSet::new(),
                auto,
                settling: HashMap::new(),
                kept: Vec::new(),
            },
        );
        at
    }

    fn describe(&mut self, query: &SourceQuery) {
        let Some(s) = self.shown else { return };
        let lines = self
            .describers
            .get_mut(s)
            .and_then(|p| p.source().map(|src| src.describe(query)))
            .unwrap_or_default();
        if let Some(app) = self.app.upgrade() {
            let lines: Vec<SharedString> = lines.into_iter().map(Into::into).collect();
            app.global::<SourceUi>()
                .set_preview(ModelRc::new(VecModel::from(lines)));
        }
    }

    fn add_search(&mut self, query: SourceQuery, auto: bool) {
        let Some(s) = self.shown else { return };
        if query.text.trim().is_empty() {
            return;
        }
        let at = self.start(s, query, now(), auto);
        self.sources[s].save();
        self.search = Some(at);
        self.checked.clear();
        self.refresh();
    }

    /// The search at a row of the panel; -1 is Downloads.
    fn select(&mut self, i: i32) {
        self.search = usize::try_from(i).ok();
        self.checked.clear();
        self.anchor = None;
        self.refresh();
    }

    fn remove(&mut self, i: i32) {
        let (Some(s), Ok(i)) = (self.shown, usize::try_from(i)) else {
            return;
        };
        let source = &mut self.sources[s];
        if i >= source.searches.len() {
            return;
        }
        let gone = source.searches.remove(i);
        let _ = source.requests.send(Request::Stop(gone.local));
        source.save();
        self.search = match self.search {
            Some(x) if x == i => (!source.searches.is_empty()).then_some(0),
            Some(x) if x > i => Some(x - 1),
            other => other,
        };
        self.refresh();
    }

    /// Removes every search of the source shown. Downloads stay.
    fn clear_searches(&mut self) {
        let Some(s) = self.shown else { return };
        let source = &mut self.sources[s];
        for gone in source.searches.drain(..) {
            let _ = source.requests.send(Request::Stop(gone.local));
        }
        source.save();
        self.search = None;
        self.checked.clear();
        self.refresh();
    }

    /// Runs a search again from scratch, in its place in the list.
    fn again(&mut self, i: i32) {
        let (Some(s), Ok(i)) = (self.shown, usize::try_from(i)) else {
            return;
        };
        if i >= self.sources[s].searches.len() {
            return;
        }
        self.next_local += 1;
        let local = self.next_local;
        let source = &mut self.sources[s];
        // A playlist's entries already found or downloaded stay as they
        // are; only the rest are searched for.
        let skip = found_entries(source, &source.searches[i]);
        let old = std::mem::replace(&mut source.searches[i].local, local);
        let _ = source.requests.send(Request::Stop(old));
        let search = &mut source.searches[i];
        search.kept = std::mem::take(&mut search.items)
            .into_iter()
            .filter(|f| skip.contains(&f.group))
            .collect();
        search.items = search.kept.clone();
        (search.done, search.note, search.error) = (false, String::new(), None);
        search.settling.clear();
        let query = SourceQuery {
            skip: skip.into_iter().collect(),
            ..search.query.clone()
        };
        let _ = source.requests.send(Request::Start(local, query));
        // Downloads from before stay with the new search's results.
        for d in source.downloads.iter_mut().filter(|d| d.local == old) {
            d.local = local;
        }
        self.refresh();
    }

    fn current(&self) -> Option<(usize, &Search)> {
        let s = self.shown?;
        Some((s, self.sources[s].searches.get(self.search?)?))
    }

    fn check(&mut self, row: usize, shift: bool) {
        let Some(Row::Item(key)) = self.rows.get(row).cloned() else {
            return;
        };
        let on = !self.checked.contains(&key);
        let range = match (shift, self.anchor) {
            (true, Some(a)) => a.min(row)..=a.max(row),
            _ => row..=row,
        };
        for r in range {
            if let Some(Row::Item(k)) = self.rows.get(r) {
                if on {
                    self.checked.insert(k.clone());
                } else {
                    self.checked.remove(k);
                }
            }
        }
        self.anchor = Some(row);
        self.refresh();
    }

    fn toggle_open(&mut self, row: usize) {
        let key = match self.rows.get(row).cloned() {
            Some(Row::Item(key)) => key,
            Some(Row::Heading(Some(group))) => song_key(&group),
            _ => return,
        };
        let Some(s) = self.shown else { return };
        let Some(i) = self.search else { return };
        let source = &mut self.sources[s];
        let search = &mut source.searches[i];
        if !search.open.remove(&key) {
            search.open.insert(key.clone());
            if let Some(f) = search.find(&key) {
                if f.folder && f.tracks.len() < f.track_count {
                    let _ = source
                        .requests
                        .send(Request::OpenFolder(search.local, Box::new(f.clone())));
                }
            }
        }
        self.refresh();
    }

    /// The button on a row: download it, or try a failed one again.
    fn download_row(&mut self, row: usize) {
        let Some(s) = self.shown else { return };
        let key = match self.rows.get(row) {
            Some(Row::Item(k)) => k.clone(),
            Some(Row::Download(d)) => self.sources[s].downloads[*d].key.clone(),
            _ => return,
        };
        match self.sources[s].download(&key).map(|d| &d.progress) {
            Some(Progress::Failed(_)) => {
                let source = &mut self.sources[s];
                if let Some(d) = source.downloads.iter_mut().rev().find(|d| d.key == key) {
                    d.progress = Progress::Queued;
                }
                let _ = source.requests.send(Request::Retry(key));
                self.refresh();
            }
            Some(_) => {}
            None => self.download_keys(vec![key]),
        }
    }

    /// Takes a row off the Downloads list, stopping it if it's still going.
    /// Files already downloaded stay.
    fn remove_download(&mut self, row: usize) {
        let (Some(s), Some(Row::Download(d))) = (self.shown, self.rows.get(row).cloned()) else {
            return;
        };
        let source = &mut self.sources[s];
        if d >= source.downloads.len() {
            return;
        }
        let gone = source.downloads.remove(d);
        if is_active(&gone.progress) {
            let _ = source.requests.send(Request::Cancel(gone.key));
        }
        self.refresh();
    }

    /// Tries every failed download of the source shown again.
    fn retry_failed(&mut self) {
        let Some(s) = self.shown else { return };
        let source = &mut self.sources[s];
        for d in &mut source.downloads {
            if matches!(d.progress, Progress::Failed(_)) {
                d.progress = Progress::Queued;
                let _ = source.requests.send(Request::Retry(d.key.clone()));
            }
        }
        self.refresh();
    }

    /// Takes finished and failed downloads off the list.
    fn clear_finished(&mut self) {
        let Some(s) = self.shown else { return };
        self.sources[s].downloads.retain(|d| is_active(&d.progress));
        self.refresh();
    }

    /// Stops every download still going, and takes them off the list.
    fn cancel_downloads(&mut self) {
        let Some(s) = self.shown else { return };
        let source = &mut self.sources[s];
        for d in source.downloads.iter().filter(|d| is_active(&d.progress)) {
            let _ = source.requests.send(Request::Cancel(d.key.clone()));
        }
        source.downloads.retain(|d| !is_active(&d.progress));
        self.refresh();
    }

    /// Ticks the first (best) copy found for each song of a playlist.
    fn pick_best(&mut self) {
        let Some((_, search)) = self.current() else {
            return;
        };
        let mut seen = HashSet::new();
        let best: Vec<String> = search
            .items
            .iter()
            .filter(|f| !f.group.is_empty() && seen.insert(f.group.clone()))
            .map(|f| f.key.clone())
            .collect();
        self.checked.extend(best);
        self.refresh();
    }

    /// For a search set to download by itself: each playlist entry whose
    /// results have settled and has no copy downloading or downloaded gets
    /// its best copy not yet tried, so a failed copy makes way for the next.
    fn auto_download(&mut self, s: usize, i: usize) {
        let source = &self.sources[s];
        let search = &source.searches[i];
        if !search.auto {
            return;
        }
        let mut keys = Vec::new();
        for group in entries(&search.items) {
            let settled = search.done
                || search
                    .settling
                    .get(group)
                    .is_some_and(|(_, looks)| *looks >= SETTLED);
            let copies: Vec<&Found> = search
                .items
                .iter()
                .filter(|f| f.group == group && !f.folder)
                .collect();
            let going = copies.iter().any(|f| {
                source
                    .download(&f.key)
                    .is_some_and(|d| !matches!(d.progress, Progress::Failed(_)))
            });
            if !settled || going {
                continue;
            }
            if let Some(next) = copies.iter().find(|f| source.download(&f.key).is_none()) {
                keys.push(next.key.clone());
            }
        }
        if !keys.is_empty() {
            self.download_in(s, i, keys);
        }
    }

    fn download_selected(&mut self) {
        let keys = self.checked.drain().collect();
        self.download_keys(keys);
    }

    /// Downloads results of the shown search: folders whole, and files
    /// that aren't in a folder also being downloaded.
    fn download_keys(&mut self, keys: Vec<String>) {
        if let (Some(s), Some(i)) = (self.shown, self.search) {
            self.download_in(s, i, keys);
        }
    }

    /// [`Self::download_keys`] for search `i` of source `s`, shown or not.
    fn download_in(&mut self, s: usize, i: usize, keys: Vec<String>) {
        let Some(dir) = download_dir(
            self.sources[s].id,
            self.sources[s].info.name,
            self.music.as_deref(),
        ) else {
            return;
        };
        let source = &mut self.sources[s];
        let search = &source.searches[i];
        let wanted: HashSet<&String> = keys.iter().collect();
        let mut items: Vec<Found> = Vec::new();
        for key in &keys {
            let Some(found) = search.find(key) else {
                continue;
            };
            let in_folder = search
                .parent(key)
                .is_some_and(|p| wanted.contains(&p.key) || source.download(&p.key).is_some());
            let busy = source
                .download(key)
                .is_some_and(|d| !matches!(d.progress, Progress::Failed(_)));
            if !in_folder && !busy {
                items.push(found.clone());
            }
        }
        if items.is_empty() {
            return;
        }
        // Keep the order they're listed in (when they're listed).
        let order: Vec<String> = self
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Item(k) => Some(k.clone()),
                _ => None,
            })
            .collect();
        items.sort_by_key(|f| order.iter().position(|k| *k == f.key));
        for f in &items {
            source.downloads.push(Download {
                key: f.key.clone(),
                local: search.local,
                label: label(f),
                progress: Progress::Queued,
            });
        }
        let _ = source
            .requests
            .send(Request::Download(search.local, items, dir));
        self.refresh();
    }

    /// Puts the panel, the list and the queue on screen.
    fn refresh(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<SourceUi>();
        let Some(s) = self.shown else {
            self.rows.clear();
            self.list.set_vec(Vec::new());
            return;
        };
        let source = &self.sources[s];
        let items: Vec<SearchItem> = source
            .searches
            .iter()
            .map(|search| SearchItem {
                name: search.name().into(),
                sub: search_line(search).into(),
                kind: if search.items.iter().any(|f| !f.group.is_empty()) {
                    QueryKind::ALL
                        .iter()
                        .position(|k| *k == QueryKind::Playlist)
                        .unwrap_or(0) as i32
                } else {
                    QueryKind::ALL
                        .iter()
                        .position(|k| *k == search.query.kind)
                        .unwrap_or(0) as i32
                },
                busy: !search.done,
            })
            .collect();
        ui.set_searches(ModelRc::new(VecModel::from(items)));
        ui.set_current_search(self.search.map_or(-1, |i| i as i32));
        ui.set_queue_shown(!source.downloads.is_empty());
        ui.set_grouped(
            self.current()
                .is_some_and(|(_, search)| search.items.iter().any(|f| !f.group.is_empty())),
        );
        ui.set_filter(self.filter);
        ui.set_lossless_only(self.lossless_only);
        let count =
            |f: fn(&Progress) -> bool| source.downloads.iter().filter(|d| f(&d.progress)).count();
        let active = count(is_active);
        let done = count(|p| matches!(p, Progress::Done(_)));
        let failed = count(|p| matches!(p, Progress::Failed(_)));
        ui.set_active_count(active as i32);
        ui.set_finished_count((done + failed) as i32);
        ui.set_failed_count(failed as i32);
        ui.set_downloads_line(
            match (active, failed) {
                (0, 0) if done == 0 => String::new(),
                (0, 0) => format!("{done} done"),
                (0, f) => format!("{f} failed"),
                (n, _) => format!("{n} active"),
            }
            .into(),
        );
        let queue: Vec<QueueLine> = source
            .downloads
            .iter()
            .filter(|d| matches!(d.progress, Progress::Running(_) | Progress::Queued))
            .take(3)
            .map(|d| QueueLine {
                text: d.label.as_str().into(),
                progress: match d.progress {
                    Progress::Running(p) => p,
                    _ => 0.0,
                },
            })
            .collect();
        ui.set_queue(ModelRc::new(VecModel::from(queue)));
        ui.set_queue_title(format!("{done} of {}", source.downloads.len()).into());

        let (rows, list) = match self.current() {
            Some((_, search)) => self.result_rows(source, search),
            None => self.download_rows(source),
        };
        // Covers of what's listed are fetched once each, in the background.
        let wanted: Vec<String> = match self.current() {
            Some((_, search)) => search.items.iter().map(|f| f.cover_url.clone()).collect(),
            None => Vec::new(),
        };
        for url in wanted {
            if !url.is_empty() && self.asked.insert(url.clone()) {
                let _ = self.cover_requests.send(url);
            }
        }
        let (title, summary, notice) = match self.current() {
            Some((_, search)) => (
                search.name().to_string(),
                search_line(search),
                match (&search.error, search.done, search.items.is_empty()) {
                    (Some(e), _, _) => e.clone(),
                    (None, false, true) => "Searching…".into(),
                    (None, true, true) => "Nothing found.".into(),
                    _ if list.is_empty() => "Nothing matches the filters.".into(),
                    _ => String::new(),
                },
            ),
            None => (
                "Downloads".into(),
                ui.get_downloads_line().to_string(),
                if list.is_empty() {
                    "Nothing downloaded yet.".into()
                } else {
                    String::new()
                },
            ),
        };
        ui.set_title(title.into());
        ui.set_summary(summary.into());
        ui.set_notice(notice.into());
        let picked: Vec<&Found> = self
            .current()
            .map(|(_, search)| self.checked.iter().filter_map(|k| search.find(k)).collect())
            .unwrap_or_default();
        ui.set_selected_count(picked.len() as i32);
        let bytes: u64 = picked.iter().map(|f| f.size).sum();
        ui.set_selected_summary(megabytes(bytes).into());
        self.rows = rows;
        // Rows update in place so the list keeps its scroll position.
        if self.list.row_count() == list.len() {
            for (i, r) in list.into_iter().enumerate() {
                if self.list.row_data(i).as_ref() != Some(&r) {
                    self.list.set_row_data(i, r);
                }
            }
        } else {
            self.list.set_vec(list);
        }
    }

    fn result_rows(&self, source: &Source, search: &Search) -> (Vec<Row>, Vec<FoundRow>) {
        let mut rows = Vec::new();
        let mut list = Vec::new();
        let keep = |f: &Found| {
            !self.lossless_only
                || LOSSLESS.contains(&f.format.to_uppercase().as_str())
                || f.format.is_empty()
        };
        let push = |rows: &mut Vec<Row>, list: &mut Vec<FoundRow>, f: &Found, depth: i32| {
            let parent_checked = search
                .parent(&f.key)
                .is_some_and(|p| self.checked.contains(&p.key));
            let own = source.download(&f.key);
            let inherited = || search.parent(&f.key).and_then(|p| source.download(&p.key));
            rows.push(Row::Item(f.key.clone()));
            list.push(found_row(
                f,
                self.covers.get(&f.cover_url),
                depth,
                self.checked.contains(&f.key) || parent_checked,
                search.open.contains(&f.key),
                own.or_else(inherited).map(|d| &d.progress),
            ));
        };
        let heading = |rows: &mut Vec<Row>, list: &mut Vec<FoundRow>, text: String| {
            rows.push(Row::Heading(None));
            list.push(FoundRow {
                kind: 0,
                title: text.into(),
                ..Default::default()
            });
        };
        let grouped = search.items.iter().any(|f| !f.group.is_empty());
        if grouped {
            // A playlist: each entry shows one copy, the one downloading or
            // downloaded if there is one, with the others under it when open.
            for group in entries(&search.items) {
                let copies: Vec<&Found> = search
                    .items
                    .iter()
                    .filter(|f| f.group == group && keep(f))
                    .collect();
                if copies.is_empty() {
                    continue;
                }
                let tried = |f: &&Found| source.download(&f.key).map(|d| &d.progress);
                let lead = copies
                    .iter()
                    .position(|f| tried(f).is_some_and(|p| !matches!(p, Progress::Failed(_))))
                    .or_else(|| copies.iter().position(|f| tried(f).is_none()))
                    .unwrap_or(0);
                let open = search.open.contains(&song_key(group));
                let more = copies.len() - 1;
                rows.push(Row::Heading(Some(group.to_string())));
                list.push(FoundRow {
                    kind: 0,
                    title: group.into(),
                    open,
                    note: match more {
                        0 => String::new(),
                        n => plural(n, "other copy", "other copies"),
                    }
                    .into(),
                    ..Default::default()
                });
                push(&mut rows, &mut list, copies[lead], 0);
                if open {
                    for (_, f) in copies.iter().enumerate().filter(|(n, _)| *n != lead) {
                        push(&mut rows, &mut list, f, 1);
                    }
                }
            }
            return (rows, list);
        }
        let folders: Vec<&Found> = search
            .items
            .iter()
            .filter(|f| f.folder && keep(f))
            .collect();
        let files: Vec<&Found> = search
            .items
            .iter()
            .filter(|f| !f.folder && keep(f))
            .collect();
        if self.filter != 1 && !folders.is_empty() {
            if self.filter == 0 {
                heading(&mut rows, &mut list, "Albums".into());
            }
            for f in folders {
                push(&mut rows, &mut list, f, 0);
                if search.open.contains(&f.key) {
                    for t in &f.tracks {
                        push(&mut rows, &mut list, t, 1);
                    }
                }
            }
        }
        if self.filter != 2 && !files.is_empty() {
            if self.filter == 0 && !rows.is_empty() {
                heading(&mut rows, &mut list, "Tracks".into());
            }
            for f in files {
                push(&mut rows, &mut list, f, 0);
            }
        }
        (rows, list)
    }

    fn download_rows(&self, source: &Source) -> (Vec<Row>, Vec<FoundRow>) {
        let mut rows = Vec::new();
        let mut list = Vec::new();
        for (i, d) in source.downloads.iter().enumerate().rev() {
            let found = source
                .searches
                .iter()
                .find(|s| s.local == d.local)
                .and_then(|s| s.find(&d.key))
                .cloned()
                .unwrap_or_else(|| Found {
                    key: d.key.clone(),
                    title: d.label.clone(),
                    ..Default::default()
                });
            rows.push(Row::Download(i));
            list.push(found_row(
                &found,
                self.covers.get(&found.cover_url),
                0,
                false,
                false,
                Some(&d.progress),
            ));
        }
        (rows, list)
    }
}

/// Where a search added at `added` goes in a list kept newest first: after
/// any added at the same time, so a list read back keeps its order.
/// The playlist entries among results, in the order they're listed.
fn entries(items: &[Found]) -> Vec<&str> {
    let mut seen = HashSet::new();
    items
        .iter()
        .filter(|f| !f.group.is_empty() && seen.insert(f.group.as_str()))
        .map(|f| f.group.as_str())
        .collect()
}

/// How an open playlist entry is kept in [`Search::open`], apart from keys.
fn song_key(group: &str) -> String {
    format!("song:{group}")
}

/// Counts how many looks in a row each entry's results stayed the same.
fn settle(settling: &mut HashMap<String, (Vec<String>, u32)>, items: &[Found]) {
    for group in entries(items) {
        let keys: Vec<String> = items
            .iter()
            .filter(|f| f.group == group)
            .map(|f| f.key.clone())
            .collect();
        let entry = settling
            .entry(group.to_string())
            .or_insert_with(|| (Vec::new(), 0));
        if entry.0 == keys {
            entry.1 += 1;
        } else {
            *entry = (keys, 1);
        }
    }
}

/// A playlist's entries that needn't be searched for again: one of their
/// copies was downloaded, or they were found and none failed.
fn found_entries(source: &Source, search: &Search) -> HashSet<String> {
    entries(&search.items)
        .into_iter()
        .filter(|group| {
            let progress: Vec<Option<&Progress>> = search
                .items
                .iter()
                .filter(|f| f.group == *group)
                .map(|f| source.download(&f.key).map(|d| &d.progress))
                .collect();
            progress
                .iter()
                .any(|p| matches!(p, Some(Progress::Done(_))))
                || !progress
                    .iter()
                    .any(|p| matches!(p, Some(Progress::Failed(_))))
        })
        .map(str::to_string)
        .collect()
}

/// Results of a search again with the entries kept from before, listed by
/// their number in the playlist ("3. Bonobo – Kerala").
fn with_kept(kept: &[Found], items: Vec<Found>) -> Vec<Found> {
    let number = |f: &Found| -> u32 {
        let digits: String = f.group.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().unwrap_or(u32::MAX)
    };
    let mut all: Vec<Found> = kept
        .iter()
        .filter(|k| !items.iter().any(|f| f.group == k.group))
        .cloned()
        .collect();
    all.extend(items);
    all.sort_by_key(number);
    all
}

fn place(searches: &[Search], added: u64) -> usize {
    searches
        .iter()
        .position(|x| x.added < added)
        .unwrap_or(searches.len())
}

/// Where a source's downloads go: its download folder setting, else
/// `<music>/Downloaded/<source>`.
fn download_dir(id: &str, name: &str, music: Option<&Path>) -> Option<PathBuf> {
    let config = crate::settings::plugin_config(id);
    folder_or_default(config.get(DOWNLOAD_FOLDER), name, music)
}

fn folder_or_default(set: &str, name: &str, music: Option<&Path>) -> Option<PathBuf> {
    let set = set.trim();
    if set.is_empty() {
        return music.map(|m| m.join("Downloaded").join(safe_name(name, "Downloads")));
    }
    opendj_engine::paths::expand_tilde(set)
}

/// Queued or running.
fn is_active(p: &Progress) -> bool {
    matches!(p, Progress::Queued | Progress::Running(_))
}

fn query(text: &str, kind: i32, prefer: i32) -> SourceQuery {
    SourceQuery {
        text: text.trim().to_string(),
        kind: QueryKind::ALL
            .get(kind as usize)
            .copied()
            .unwrap_or_default(),
        prefer: Prefer::ALL
            .get(prefer as usize)
            .copied()
            .unwrap_or_default(),
        skip: Vec::new(),
    }
}

/// "Artist – Title", or a folder's album.
fn label(f: &Found) -> String {
    let name = if f.folder && !f.album.is_empty() {
        &f.album
    } else {
        &f.title
    };
    if f.artist.is_empty() {
        name.clone()
    } else {
        format!("{} – {name}", f.artist)
    }
}

/// The line under a search in the panel.
fn search_line(search: &Search) -> String {
    if let Some(e) = &search.error {
        return e.lines().next().unwrap_or_default().to_string();
    }
    let folders = search.items.iter().filter(|f| f.folder).count();
    let files = search.items.len() - folders;
    let mut parts = Vec::new();
    if files > 0 {
        parts.push(plural(files, "track", "tracks"));
    }
    if folders > 0 {
        parts.push(plural(folders, "album", "albums"));
    }
    let found = parts.join(" · ");
    match (search.done, found.is_empty(), search.note.is_empty()) {
        (_, _, false) if search.done => search.note.clone(),
        (false, true, _) => "Searching…".into(),
        (false, false, _) => format!("Searching… {found} so far"),
        (true, true, _) => "Nothing found".into(),
        (true, false, _) => found,
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn found_row(
    f: &Found,
    cover: Option<&slint::Image>,
    depth: i32,
    checked: bool,
    open: bool,
    progress: Option<&Progress>,
) -> FoundRow {
    let (state, fraction, note) = match progress {
        None => (0, 0.0, String::new()),
        Some(Progress::Queued) => (1, 0.0, String::new()),
        Some(Progress::Running(p)) => (2, *p, String::new()),
        Some(Progress::Done(_)) => (3, 1.0, String::new()),
        Some(Progress::Failed(e)) => (4, 0.0, e.clone()),
    };
    let length = if f.duration > 0.0 {
        let s = f.duration.round() as u64;
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        String::new()
    };
    let detail = if f.folder {
        let n = f.track_count.max(f.tracks.len());
        [
            f.artist.clone(),
            f.year.clone(),
            if n > 0 {
                plural(n, "track", "tracks")
            } else {
                String::new()
            },
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
    } else {
        String::new()
    };
    FoundRow {
        kind: if f.folder { 1 } else { 2 },
        depth,
        checked,
        open,
        title: if f.folder && !f.album.is_empty() {
            f.album.as_str().into()
        } else {
            f.title.as_str().into()
        },
        artist: if f.folder {
            detail.into()
        } else {
            f.artist.as_str().into()
        },
        number: f.number.as_str().into(),
        length: length.into(),
        format: f.format.as_str().into(),
        quality: f.quality.as_str().into(),
        size: megabytes(f.size).into(),
        from: f.from.as_str().into(),
        state,
        progress: fraction,
        note: note.into(),
        cover: cover.cloned().unwrap_or_default(),
        has_cover: cover.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(key: &str, title: &str) -> Found {
        Found {
            key: key.into(),
            title: title.into(),
            artist: "Bonobo".into(),
            format: "FLAC".into(),
            ..Default::default()
        }
    }

    fn search(items: Vec<Found>, done: bool) -> Search {
        Search {
            local: 1,
            query: SourceQuery::default(),
            added: 0,
            items,
            done,
            note: String::new(),
            title: String::new(),
            error: None,
            open: HashSet::new(),
            auto: false,
            settling: HashMap::new(),
            kept: Vec::new(),
        }
    }

    /// The rail numbers add-on sources on from its own list of built-in
    /// sources. If that list and ours differ, picking an add-on's source
    /// shows its panel but its searches are neither listed nor started.
    #[test]
    fn rail_lists_the_built_in_sources() {
        let ui = include_str!("../ui/library.slint");
        let lists: Vec<Vec<&str>> = [
            "property <[string]> names: [",
            "property <[string]> source-names: [",
        ]
        .iter()
        .map(|start| {
            let from = ui.find(start).expect(start) + start.len();
            let len = ui[from..].find(']').expect("a closing ]");
            ui[from..from + len]
                .split(',')
                .map(|n| n.trim().trim_matches('"'))
                .collect()
        })
        .collect();
        for list in lists {
            assert_eq!(list, crate::browser::SOURCE_NAMES);
        }
    }

    fn copy(key: &str, group: &str) -> Found {
        Found {
            group: group.into(),
            ..file(key, "Kerala")
        }
    }

    #[test]
    fn entries_settle_when_their_results_stop_changing() {
        let mut settling = HashMap::new();
        let first = vec![copy("a", "1. Kerala")];
        settle(&mut settling, &first);
        settle(&mut settling, &first);
        assert_eq!(settling["1. Kerala"].1, 2);
        let more = vec![copy("a", "1. Kerala"), copy("b", "1. Kerala")];
        settle(&mut settling, &more);
        assert_eq!(settling["1. Kerala"].1, 1);
    }

    #[test]
    fn search_again_keeps_entries_in_playlist_order() {
        let kept = vec![copy("a", "1. Kerala"), copy("c", "10. Kong")];
        let new = vec![copy("b", "2. Cirrus"), copy("d", "11. Grains")];
        let all: Vec<String> = with_kept(&kept, new).into_iter().map(|f| f.key).collect();
        assert_eq!(all, ["a", "b", "c", "d"]);
        assert_eq!(
            entries(&[
                copy("a", "1. Kerala"),
                copy("b", "1. Kerala"),
                copy("c", "2. Kong")
            ]),
            ["1. Kerala", "2. Kong"]
        );
    }

    #[test]
    fn found_or_downloaded_entries_are_not_searched_again() {
        let (requests, _rx) = channel();
        let mut source = Source {
            id: "x",
            info: SourceInfo {
                name: "X",
                icon_svg: b"",
                hint: "",
            },
            requests,
            searches: Vec::new(),
            downloads: Vec::new(),
        };
        let progress = |key: &str, progress| Download {
            key: key.into(),
            local: 1,
            label: String::new(),
            progress,
        };
        source.downloads = vec![
            progress("a", Progress::Done(Vec::new())),
            progress("b", Progress::Failed("gone".into())),
            progress("c", Progress::Failed("gone".into())),
            progress("d", Progress::Done(Vec::new())),
        ];
        let s = search(
            vec![
                copy("a", "1. Kerala"),
                copy("b", "2. Kong"),
                copy("c", "3. Cirrus"),
                copy("d", "3. Cirrus"),
                copy("e", "4. Grains"),
            ],
            true,
        );
        let mut skip: Vec<String> = found_entries(&source, &s).into_iter().collect();
        skip.sort();
        // Kong's only copy failed, so it's searched for again.
        assert_eq!(skip, ["1. Kerala", "3. Cirrus", "4. Grains"]);
    }

    #[test]
    fn download_folders() {
        let music = Path::new("/m");
        assert_eq!(
            folder_or_default("", "Soulseek", Some(music)),
            Some(PathBuf::from("/m/Downloaded/Soulseek"))
        );
        assert_eq!(folder_or_default(" ", "Soulseek", None), None);
        assert_eq!(
            folder_or_default("/data/slsk", "Soulseek", None),
            Some(PathBuf::from("/data/slsk"))
        );
        let home = opendj_engine::paths::home_dir().unwrap();
        assert_eq!(
            folder_or_default("~/Music/In", "Soulseek", None),
            Some(home.join("Music/In"))
        );
    }

    #[test]
    fn listed_by_the_sources_name() {
        let mut s = search(vec![], true);
        s.query.text = "https://open.spotify.com/playlist/x".into();
        assert_eq!(s.name(), "https://open.spotify.com/playlist/x");
        s.title = "Psy chill".into();
        assert_eq!(s.name(), "Psy chill");
    }

    #[test]
    fn newest_searches_first() {
        let list: Vec<Search> = [30, 20, 20, 10]
            .into_iter()
            .map(|added| Search {
                added,
                ..search(vec![], true)
            })
            .collect();
        assert_eq!(place(&list, 40), 0);
        assert_eq!(place(&list, 20), 3);
        assert_eq!(place(&list, 5), 4);
        assert_eq!(place(&[], 5), 0);
    }

    #[test]
    fn lines_under_searches() {
        let album = Found {
            folder: true,
            tracks: vec![file("a1", "Migration")],
            ..file("a", "")
        };
        assert_eq!(search_line(&search(vec![], false)), "Searching…");
        assert_eq!(
            search_line(&search(vec![file("k", "Kerala")], false)),
            "Searching… 1 track so far"
        );
        assert_eq!(
            search_line(&search(vec![file("k", "Kerala"), album], true)),
            "1 track · 1 album"
        );
        assert_eq!(search_line(&search(vec![], true)), "Nothing found");
        let mut noted = search(vec![file("k", "Kerala")], true);
        noted.note = "18 of 22 found".into();
        assert_eq!(search_line(&noted), "18 of 22 found");
    }

    #[test]
    fn files_are_found_in_folders() {
        let album = Found {
            folder: true,
            album: "Migration".into(),
            tracks: vec![file("a1", "Migration"), file("a3", "Kerala")],
            ..file("a", "")
        };
        let s = search(vec![album, file("k", "Kong")], true);
        assert_eq!(s.find("a3").unwrap().title, "Kerala");
        assert_eq!(s.parent("a3").unwrap().key, "a");
        assert!(s.parent("k").is_none());
        assert_eq!(label(s.find("a").unwrap()), "Bonobo – Migration");
    }

    #[test]
    fn rows_show_download_state() {
        let r = found_row(&file("k", "Kong"), None, 0, true, false, None);
        assert_eq!((r.kind, r.state, r.checked), (2, 0, true));
        let r = found_row(
            &Found {
                duration: 302.0,
                ..file("k", "Kong")
            },
            None,
            0,
            false,
            false,
            Some(&Progress::Running(0.5)),
        );
        assert_eq!((r.state, r.progress, r.length.as_str()), (2, 0.5, "5:02"));
        let r = found_row(
            &file("k", "Kong"),
            None,
            0,
            false,
            false,
            Some(&Progress::Failed("gone".into())),
        );
        assert_eq!((r.state, r.note.as_str()), (4, "gone"));
    }

    #[test]
    fn queries_from_the_dialog() {
        let q = query("  Bonobo - Kerala ", 2, 1);
        assert_eq!(q.text, "Bonobo - Kerala");
        assert_eq!(q.kind, QueryKind::Album);
        assert_eq!(q.prefer, Prefer::Mp3);
        assert_eq!(query("x", 99, -1).kind, QueryKind::Auto);
    }
}
