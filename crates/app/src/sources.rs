//! Sources in the rail that come from add-ons (see
//! [`mixxx_plugins::source`]): saved searches on the left, what they found
//! in the list, and downloads that join the collection when they finish.
//!
//! Each source runs on a thread of its own, which also asks how searches
//! and downloads are getting on about once a second, so a slow network
//! never holds up the UI or the audio.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use mixxx_engine::Track;
use mixxx_plugins::inbox::safe_name;
use mixxx_plugins::source::{
    megabytes, DownloadUpdate, Found, Prefer, Progress, QueryKind, SearchSource, Snapshot,
    SourceInfo, SourceQuery,
};
use mixxx_plugins::{Plugin, PluginError};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::{AppWindow, FoundRow, QueueLine, SearchItem, SourceTab, SourceUi, StoreUi};

/// Where plugin sources start in the rail, after the built-in ones.
pub const FIRST_SOURCE: i32 = 10;

/// How often searches and downloads are asked how they're getting on.
const TICK: Duration = Duration::from_millis(900);

/// Formats that count as lossless for "Lossless only".
const LOSSLESS: [&str; 5] = ["FLAC", "WAV", "AIFF", "ALAC", "APE"];

/// The plugins with a source of their own, and what each is called.
pub fn source_plugins() -> Vec<(Box<dyn Plugin>, SourceInfo)> {
    crate::stores::plugins()
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
    let Some(source) = plugin.source() else {
        return;
    };
    let name = source.info().name;
    let mut ids: HashMap<u64, String> = HashMap::new();
    let mut active: HashSet<u64> = HashSet::new();
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(TICK) {
            Ok(request) => {
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
    items: Vec<Found>,
    done: bool,
    note: String,
    error: Option<String>,
    /// Folders the DJ has opened, by key.
    open: HashSet<String>,
}

impl Search {
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

/// One plugin source: its searches and downloads.
struct Source {
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
                )
            })
            .collect();
        crate::settings::set_source_searches(self.info.name, &saved);
    }
}

/// What a row of the list is.
#[derive(Clone, PartialEq)]
enum Row {
    Heading,
    /// A result, by its key.
    Item(String),
    /// A download in the Downloads list, by its index.
    Download(usize),
}

pub struct Sources {
    app: slint::Weak<AppWindow>,
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
    /// Where downloads go: `<music>/Downloaded/<source>/`.
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
                let (requests, rx) = channel();
                let (tx, library) = (tx.clone(), library.clone());
                std::thread::Builder::new()
                    .name(format!("source {}", info.name))
                    .spawn(move || run(plugin, index, library, rx, tx))
                    .expect("spawn source thread");
                Source {
                    info,
                    requests,
                    searches: Vec::new(),
                    downloads: Vec::new(),
                }
            })
            .collect();
        let list = Rc::new(VecModel::default());
        let mut me = Self {
            app: app.as_weak(),
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
            music: mixxx_engine::scan::default_music_dir(),
            list: list.clone(),
            describers: source_plugins().into_iter().map(|(p, _)| p).collect(),
        };
        // Saved searches run again when the app starts.
        for s in 0..me.sources.len() {
            let saved = crate::settings::source_searches(me.sources[s].info.name);
            for (text, kind, prefer) in saved {
                let query = SourceQuery {
                    text,
                    kind: QueryKind::from_name(&kind),
                    prefer: Prefer::from_name(&prefer),
                };
                me.start(s, query);
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
        ui.on_search(move |text, kind, prefer| {
            let query = query(&text, kind, prefer);
            w(Box::new(move |s| s.add_search(query.clone())))
        });
        let w = with.clone();
        ui.on_select_search(move |i| w(Box::new(move |s| s.select(i))));
        let w = with.clone();
        ui.on_remove_search(move |i| w(Box::new(move |s| s.remove(i))));
        let w = with.clone();
        ui.on_search_again(move |i| w(Box::new(move |s| s.again(i))));
        let w = with.clone();
        ui.on_check(move |row, shift| w(Box::new(move |s| s.check(row as usize, shift))));
        let w = with.clone();
        ui.on_toggle_open(move |row| w(Box::new(move |s| s.toggle_open(row as usize))));
        let w = with.clone();
        ui.on_download(move |row| w(Box::new(move |s| s.download_row(row as usize))));
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
    pub fn poll(&mut self) -> Vec<Track> {
        let mut imported = Vec::new();
        let mut changed = false;
        for event in self.events.try_iter().collect::<Vec<_>>() {
            changed = true;
            match event {
                Event::Found(s, local, snap) => {
                    if let Some(search) = self.search_mut(s, local) {
                        // Opened folders keep the files read for them.
                        let mut items = snap.items;
                        for item in items.iter_mut().filter(|i| i.folder) {
                            if let Some(old) = search.items.iter().find(|o| o.key == item.key) {
                                if old.tracks.len() > item.tracks.len() {
                                    item.tracks = old.tracks.clone();
                                }
                            }
                        }
                        search.items = items;
                        search.done = snap.done;
                        search.note = snap.note;
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
        if let Some(app) = self.app.upgrade() {
            let picked = app.global::<StoreUi>().get_source();
            let shown = (picked >= FIRST_SOURCE)
                .then(|| (picked - FIRST_SOURCE) as usize)
                .filter(|&s| s < self.sources.len());
            if shown != self.shown {
                self.shown = shown;
                self.search = shown.and_then(|s| self.sources[s].searches.len().checked_sub(1));
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

    fn start(&mut self, s: usize, query: SourceQuery) {
        self.next_local += 1;
        let local = self.next_local;
        let _ = self.sources[s]
            .requests
            .send(Request::Start(local, query.clone()));
        self.sources[s].searches.push(Search {
            local,
            query,
            items: Vec::new(),
            done: false,
            note: String::new(),
            error: None,
            open: HashSet::new(),
        });
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

    fn add_search(&mut self, query: SourceQuery) {
        let Some(s) = self.shown else { return };
        if query.text.trim().is_empty() {
            return;
        }
        self.start(s, query);
        self.sources[s].save();
        self.search = Some(self.sources[s].searches.len() - 1);
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
            Some(x) if x == i => source.searches.len().checked_sub(1),
            Some(x) if x > i => Some(x - 1),
            other => other,
        };
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
        let old = std::mem::replace(&mut source.searches[i].local, local);
        let _ = source.requests.send(Request::Stop(old));
        let search = &mut source.searches[i];
        (search.items, search.done, search.note, search.error) =
            (Vec::new(), false, String::new(), None);
        let _ = source
            .requests
            .send(Request::Start(local, search.query.clone()));
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
        let Some(Row::Item(key)) = self.rows.get(row).cloned() else {
            return;
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

    fn download_selected(&mut self) {
        let keys = self.checked.drain().collect();
        self.download_keys(keys);
    }

    /// Downloads results of the shown search: folders whole, and files
    /// that aren't in a folder also being downloaded.
    fn download_keys(&mut self, keys: Vec<String>) {
        let (Some(s), Some(i), Some(music)) = (self.shown, self.search, self.music.clone()) else {
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
        // Keep the order they're listed in.
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
        let dir = music
            .join("Downloaded")
            .join(safe_name(source.info.name, "Downloads"));
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
                name: search.query.text.as_str().into(),
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
        let active = count(|p| matches!(p, Progress::Queued | Progress::Running(_)));
        let done = count(|p| matches!(p, Progress::Done(_)));
        let failed = count(|p| matches!(p, Progress::Failed(_)));
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
        ui.set_queue_folder(
            format!(
                "Downloads go to Downloaded/{} in your music folder.",
                source.info.name
            )
            .into(),
        );

        let (rows, list) = match self.current() {
            Some((_, search)) => self.result_rows(source, search),
            None => self.download_rows(source),
        };
        let (title, summary, notice) = match self.current() {
            Some((_, search)) => (
                search.query.text.clone(),
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
                depth,
                self.checked.contains(&f.key) || parent_checked,
                search.open.contains(&f.key),
                own.or_else(inherited).map(|d| &d.progress),
            ));
        };
        let heading = |rows: &mut Vec<Row>, list: &mut Vec<FoundRow>, text: String| {
            rows.push(Row::Heading);
            list.push(FoundRow {
                kind: 0,
                title: text.into(),
                ..Default::default()
            });
        };
        let grouped = search.items.iter().any(|f| !f.group.is_empty());
        if grouped {
            let mut last = None;
            for f in search.items.iter().filter(|f| keep(f)) {
                if last != Some(&f.group) {
                    heading(&mut rows, &mut list, f.group.clone());
                    last = Some(&f.group);
                }
                push(&mut rows, &mut list, f, 0);
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
            list.push(found_row(&found, 0, false, false, Some(&d.progress)));
        }
        (rows, list)
    }
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
            items,
            done,
            note: String::new(),
            error: None,
            open: HashSet::new(),
        }
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
        let r = found_row(&file("k", "Kong"), 0, true, false, None);
        assert_eq!((r.kind, r.state, r.checked), (2, 0, true));
        let r = found_row(
            &Found {
                duration: 302.0,
                ..file("k", "Kong")
            },
            0,
            false,
            false,
            Some(&Progress::Running(0.5)),
        );
        assert_eq!((r.state, r.progress, r.length.as_str()), (2, 0.5, "5:02"));
        let r = found_row(
            &file("k", "Kong"),
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
