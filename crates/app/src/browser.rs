//! The browser: folder tree, track list and Automix / Sidelist panel.
//!
//! The collection is the saved library (see `opendj_library`): the scanned
//! music folder, or the built-in sample
//! library when there is none. The DJ's lists and folders, ratings,
//! colours and play history are kept in the library database. With the Spotify source
//! the tree lists the user's Spotify playlists, and their tracks play when the
//! collection has them (see [`crate::spotify`]).

use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use opendj_control::{ControlHandle, ControlRegistry};
use opendj_engine::key::{self, Key};
use opendj_engine::scan::{is_audio, read_track};
use opendj_engine::{deck_group, AudioEngine, Hotcue, Track, NUM_DECKS, NUM_HOTCUES};
use opendj_library::dupes::{Case, Group};
use opendj_library::health::{Check, Facts};
use opendj_library::query::{self, Context, Indexed, Query};
use opendj_library::stages::{Folders, Stage};
use opendj_library::{Library, PlaylistKind};
use slint::{ComponentHandle, DataTransfer, Model, ModelRc, SharedString, VecModel};

use opendj_library::Wanted;
use opendj_plugins::inbox::Mode;
use opendj_plugins::{ItemKind, StoreItem, StoreQuery};

mod albums;
mod analysis;
mod cleanup;
mod columns;
mod filters;
mod health;
mod info;
mod sets;
mod staging;
mod usb;

pub use filters::DEFAULT_PINNED;

use crate::analysis_queue::AnalysisQueue;
use crate::art::Art;
use crate::cover_scan::CoverScan;
use crate::files::{self, Found, Place};
use crate::inbox::Inbox;
use crate::settings::show_key;
use crate::spotify::{self, open_in_browser, Event, Request, Service, StreamTrack};
use crate::stores::{self, Stores};
use crate::waves::Waves;
use crate::{
    fmt_time, AppWindow, ColumnUi, FilesUi, Folder, HealthUi, KeyNotation, LibraryEdit, SideRow,
    StoreUi, StreamMatch, TrackDrag, TrackInfo, TrackRow,
};

/// Sources in the rail that have a tree of their own.
const COLLECTION: i32 = 0;
/// Folders on the computer (see [`crate::files`]).
const FILES: i32 = 1;
const SPOTIFY: i32 = 2;
const TIDAL: i32 = 3;
const APPLE_MUSIC: i32 = 4;
const SOUNDCLOUD: i32 = 8;
const MIXCLOUD: i32 = 11;
/// Bandcamp in the rail, with the other stores in its tree.
const STORES: i32 = 5;
/// The albums folder and the mixed sets folder (see [`albums`]); drawn
/// in the rail after Files.
const ALBUMS: i32 = 9;
const MIXED_SETS: i32 = 10;

/// Whether `source` shows store results.
fn is_stores(source: i32) -> bool {
    source == STORES
}

#[derive(Clone)]
enum Contents {
    All,
    /// Track ids in playlist order.
    List(Vec<usize>),
    /// A smart list: the tracks a search finds (see `opendj_library::query`).
    Query(String),
    /// Tracks with this grouping.
    Grouping(String),
    /// Tracks tagged with this word (a hashtag).
    Label(String),
    /// A folder shows everything in the lists below it.
    Folder,
    /// A streaming service's playlist, by its key.
    Stream(String),
    /// One of the DJ's lists (or a history session) in the library database.
    Playlist(i64),
    /// Search results from every store but Bandcamp.
    OtherStores,
    /// Search results from one store, by its index in `Stores`, or all.
    Store(Option<usize>),
    /// Tracks the DJ went to buy that haven't arrived yet.
    Wanted,
    /// A folder on the computer, in the Files source: its audio files.
    Dir(PathBuf),
    /// The collection's tracks in a folder and below it, in the Albums
    /// and Mixed Sets sources.
    Under(PathBuf),
    /// The tracks at a stage (see [`opendj_library::stages`]), or only those
    /// in one subfolder of its folder (a triage crate).
    Stage(Stage, Option<PathBuf>),
    /// Collection Health's page, or the tracks one of its checks finds (see
    /// [`opendj_library::health`]).
    Health(Option<Check>),
    /// The groups of copies of one case, under Collection Health's
    /// Duplicates (see [`opendj_library::dupes`]).
    Dupes(Case),
}

/// Whether the streaming services that are built (Spotify, TIDAL) can be
/// browsed.
/// The others in the rail say "Coming soon".
pub const STREAMING_ENABLED: bool = true;

/// Whether the stores (Bandcamp, the other stores) and the purchase inbox
/// can be used. While false, their source says "Coming soon".
pub const STORES_ENABLED: bool = false;

/// How many collection tracks the match menu offers.
const MATCH_CANDIDATES: usize = 8;

/// The sources in the rail, in order, as saved when hidden. The rail's
/// `names` and `source-names` in `library.slint` must list the same.
pub const SOURCE_NAMES: [&str; 12] = [
    "Collection",
    "Files",
    "Spotify",
    "TIDAL",
    "Apple Music",
    "Bandcamp",
    "Beatport",
    "OneLibrary",
    "SoundCloud",
    "Albums",
    "Mixed Sets",
    "Mixcloud",
];

/// Every source in the rail: the built-in ones, then add-ons' own (see
/// [`crate::sources`]).
fn all_source_names() -> Vec<String> {
    SOURCE_NAMES
        .iter()
        .map(|n| n.to_string())
        .chain(
            crate::sources::source_plugins()
                .into_iter()
                .map(|(_, info)| info.name.to_string()),
        )
        .collect()
}

/// A source's tree while another source is shown.
/// What the track menu's undo line takes back.
struct Undo {
    /// The batch of stage changes and file moves in the library.
    batch: Option<i64>,
    /// What the line says.
    text: String,
    /// Tracks to bring back into the library: merged copies.
    restore: Vec<usize>,
    /// The batch of changes to tags to take back (see
    /// [`opendj_library::cleanup`]).
    changes: Option<i64>,
}

struct Parked {
    nodes: Vec<Node>,
    current: usize,
    /// Its sort column and direction.
    sort: (i32, bool),
}

/// What the stores have found for the current search.
struct StoreState {
    stores: Stores,
    /// The search the answers are for; older answers are dropped.
    generation: u64,
    query: String,
    /// Each store's answer: results, why there are none, and a link to its
    /// own search page.
    answers: Vec<Option<Answer>>,
    wanted: Vec<Wanted>,
    /// What each row of the list is, while the stores are shown.
    rows: Vec<StoreRow>,
    /// The last download started or failed.
    download: String,
}

/// A store's results, why there are none, and a link to its own search.
type Answer = (Vec<StoreItem>, Option<String>, Option<String>);

#[derive(Clone)]
enum StoreRow {
    Item(usize, StoreItem),
    /// A store's own search page.
    Link(usize, String),
    Wanted(Wanted),
}

#[derive(Clone)]
struct Node {
    name: String,
    depth: i32,
    kind: i32,
    contents: Contents,
    expanded: bool,
    /// The DJ's own list or folder in the library database.
    playlist: Option<i64>,
    /// Shown on the history tab rather than the library tab.
    history: bool,
    /// A folder in Files whose subfolders haven't been read yet.
    lazy: bool,
}

impl Node {
    fn new(name: impl Into<String>, depth: i32, kind: i32, contents: Contents) -> Self {
        Node {
            name: name.into(),
            depth,
            kind,
            contents,
            expanded: true,
            playlist: None,
            history: false,
            lazy: false,
        }
    }

    /// A folder in Files, closed until it's opened.
    fn dir(name: impl Into<String>, path: PathBuf, depth: i32, kind: i32) -> Self {
        Node {
            expanded: false,
            lazy: true,
            ..Node::new(name, depth, kind, Contents::Dir(path))
        }
    }

    fn path(&self) -> Option<&Path> {
        match &self.contents {
            Contents::Dir(p) => Some(p),
            _ => None,
        }
    }
}

/// Reads a Files folder's subfolders into the tree below it and opens it.
fn open_dir(nodes: &mut Vec<Node>, i: usize) {
    if nodes[i].lazy {
        let depth = nodes[i].depth + 1;
        let children: Vec<Node> = nodes[i]
            .path()
            .map(files::subfolders)
            .unwrap_or_default()
            .into_iter()
            .map(|p| Node::dir(files::folder_name(&p), p, depth, KIND_FOLDER))
            .collect();
        nodes.splice(i + 1..i + 1, children);
        nodes[i].lazy = false;
    }
    nodes[i].expanded = true;
}

/// The Files tree: its starting folders, with the folders in `open` opened.
fn files_tree(open: &HashSet<PathBuf>) -> Vec<Node> {
    let mut nodes: Vec<Node> = files::roots()
        .into_iter()
        .map(|r| {
            let kind = match r.place {
                Place::Folder => KIND_FOLDER,
                Place::Drive => KIND_DRIVE,
                Place::Pinned => KIND_PINNED,
                Place::Cloud => KIND_CLOUD,
            };
            Node::dir(r.name, r.path, 0, kind)
        })
        .collect();
    let mut i = 0;
    while i < nodes.len() {
        if nodes[i].path().is_some_and(|p| open.contains(p)) {
            open_dir(&mut nodes, i);
        }
        i += 1;
    }
    nodes
}

/// Opens the Files tree down to `path` and returns its node: the deepest
/// folder on the way there that the tree shows.
fn reveal_dir(nodes: &mut Vec<Node>, path: &Path) -> Option<usize> {
    let mut at = (0..nodes.len())
        .filter(|&i| nodes[i].depth == 0)
        .filter(|&i| nodes[i].path().is_some_and(|r| path.starts_with(r)))
        .max_by_key(|&i| nodes[i].path().map_or(0, |r| r.components().count()))?;
    loop {
        if nodes[at].path() == Some(path) {
            return Some(at);
        }
        open_dir(nodes, at);
        let depth = nodes[at].depth;
        let next = nodes[at + 1..]
            .iter()
            .take_while(|n| n.depth > depth)
            .position(|n| n.depth == depth + 1 && n.path().is_some_and(|p| path.starts_with(p)));
        match next {
            Some(j) => at += 1 + j,
            None => return Some(at),
        }
    }
}

/// Files shown in the Files source that the collection doesn't have get ids
/// from here up (below the library's own ids), and stay out of the
/// collection until they're loaded or added.
const BROWSED_BASE: usize = 800_000;

fn is_browsed(t: &Track) -> bool {
    (BROWSED_BASE..opendj_library::ID_BASE).contains(&t.id)
}

fn file_path(t: &Track) -> Option<&Path> {
    match &t.source {
        opendj_engine::Source::File(p) => Some(p),
        _ => None,
    }
}

/// A tree node for one of the DJ's smart lists.
fn smart_node(p: &opendj_library::Playlist, depth: i32) -> Node {
    let mut n = Node::new(
        p.name.clone(),
        depth,
        KIND_SMART,
        Contents::Query(p.query.clone().unwrap_or_default()),
    );
    n.playlist = Some(p.id);
    n
}

/// Folder kinds the tree draws (see `Folder` in library.slint).
const KIND_FOLDER: i32 = 1;
const KIND_LIST: i32 = 2;
const KIND_SMART: i32 = 3;
const KIND_HISTORY: i32 = 5;
/// Bandcamp in the stores list, shown with its logo.
const KIND_BANDCAMP: i32 = 6;
/// A drive, and a folder pinned to the top, in Files.
const KIND_DRIVE: i32 = 7;
const KIND_PINNED: i32 = 8;
/// iCloud Drive, Google Drive, Dropbox and the like, in Files.
const KIND_CLOUD: i32 = 9;
/// The inbox and triage stages; the collection stage is drawn as kind 0.
const KIND_INBOX: i32 = 10;
const KIND_TRIAGE: i32 = 11;
/// Collection Health, and one of its checks.
const KIND_HEALTH: i32 = 12;
const KIND_CHECK: i32 = 13;
/// A set: a folder made for one gig, drawn in lilac.
const KIND_SET: i32 = 14;
/// An album's folder, and a folder of mixed sets.
const KIND_ALBUM: i32 = 15;
const KIND_MIX: i32 = 16;

/// Seconds a track must play on a deck to count as played, for the history.
const PLAYED_AFTER: f64 = 30.0;

/// Watches one deck's hot cues so changes the DJ makes are saved.
#[derive(Default)]
struct CueWatch {
    /// The track on the deck, by id, and when it arrived.
    track: Option<(usize, std::time::Instant)>,
    /// The cues as last saved or loaded.
    saved: Vec<Hotcue>,
    /// Seconds played since it was loaded, and whether that was recorded.
    played: f64,
    recorded: bool,
}

pub struct Browser {
    app: slint::Weak<AppWindow>,
    library: Vec<Arc<Track>>,
    nodes: Vec<Node>,
    /// Indices into `nodes` of the rows currently shown in the tree.
    visible_nodes: Vec<usize>,
    current_node: usize,
    search: String,
    sort_column: i32,
    ascending: bool,
    /// Indices into `library` shown in the list, in display order; `None`
    /// for a Spotify track the collection doesn't have.
    rows: Vec<Option<usize>>,
    /// For each row of a Spotify playlist, where its track is in the playlist.
    stream_rows: Vec<Option<usize>>,
    /// Ids (not indices) of the tracks on each deck.
    loaded: [Option<usize>; NUM_DECKS],
    /// The decks the layout shows: 4 in 4-Deck, else 2. "Load on a free
    /// deck" picks among these.
    decks_shown: usize,
    side: [Vec<usize>; 2],
    shown_side_tab: usize,
    /// The Linked tab: the track selected in the list (an index into
    /// `library`) and what it shows, tracks linked to it and then those
    /// played next to it in the history. `None` marks a section heading.
    linked_to: Option<usize>,
    linked: Vec<Option<usize>>,
    /// How many of `linked`'s rows (heading included) are linked tracks.
    link_rows: usize,
    engine: Rc<AudioEngine>,
    play: [ControlHandle; NUM_DECKS],
    /// The key each deck plays in, with its shift.
    deck_key: [ControlHandle; NUM_DECKS],
    /// How far, in semitones, tracks are shifted to match keys on load.
    key_match_range: ControlHandle,
    /// The key mixing rule and the reach of the key marks the rows were
    /// made with.
    key_rules: (key::Mixing, i32),
    lib: LibraryControls,
    /// Moving the selection with a controller goes through folders instead of tracks.
    folders_focused: bool,
    /// Tracks loaded with LoadSelectedTrackAndPlay, to start once they arrive.
    play_when_loaded: [Option<usize>; NUM_DECKS],
    /// The streaming services, in the order of [`STREAMS`].
    streams: Vec<Stream>,
    store: StoreState,
    inbox: Inbox,
    /// Tracks arrived from the inbox while another source was shown, so
    /// the collection's tree (its Inbox list) needs reading again.
    collection_stale: bool,
    /// The source whose tree is in `nodes`: the collection, Spotify or the stores.
    tree: i32,
    /// The other sources' trees, selections and sort orders while not shown.
    parked: HashMap<i32, Parked>,
    /// Keys in Camelot notation rather than notes.
    camelot: bool,
    /// Album art thumbnails for the list.
    art: Art,
    /// Mini waveforms for the list's Waveform column.
    waves: Waves,
    /// The list rows given a mini waveform, and the rows in view then.
    wave_rows: Vec<usize>,
    shown_rows: (i32, i32),
    /// The list as shown, kept so arriving art can fill in single rows.
    list: Rc<VecModel<TrackRow>>,
    /// The saved library, when it could be opened.
    db: Option<Library>,
    /// Where each track id is in `library`.
    index_of: HashMap<usize, usize>,
    /// Where each file is in `library`, for Files.
    path_index: HashMap<PathBuf, usize>,
    files: FilesState,
    /// Each track's text lowercased for searching, as `library` is ordered.
    indexed: Vec<Indexed>,
    /// Ids of tracks that are in the library more than once.
    duplicates: HashSet<usize>,
    /// The copies grouped by case, worked out (reading their files) the
    /// first time they're needed after the library changes.
    dupe_groups: OnceCell<Vec<Group>>,
    /// Pairs of linked tracks, both ways round.
    links: HashSet<(usize, usize)>,
    /// Tempo and key of the track playing now, for "mixes with deck", and its id.
    deck_ref: Option<(f64, Option<Key>)>,
    deck_track: Option<usize>,
    /// The sidebar shows the history sessions instead of the library tree.
    history_tab: bool,
    /// The node selected on the other tab, to go back to.
    other_tab_node: usize,
    /// This run's history session, made when the first track is played.
    session: Option<i64>,
    /// Lists tracks can be added to, as offered in the track menu.
    menu_lists: Vec<i64>,
    /// Sets, as History's "Add to Set List…" offers them.
    menu_sets: Vec<i64>,
    /// The crates each track is in, by track id.
    crates: HashMap<usize, Vec<String>>,
    /// Empty crates and entries for removed tracks.
    list_health: opendj_library::sets::ListHealth,
    /// Hot cue controls per deck: enabled and position.
    /// Each deck's hot cues: enabled, position and a saved loop's end.
    hotcues: [Vec<(ControlHandle, ControlHandle, ControlHandle)>; NUM_DECKS],
    watch: [CueWatch; NUM_DECKS],
    /// The side panel's Info tab.
    info: info::InfoState,
    /// The filter bar.
    filter: filters::FilterState,
    /// The track list's columns.
    columns: columns::ColumnState,
    /// The stage folders, as last read from the settings.
    folders: Folders,
    /// The mixed sets folder: its tracks are kept out of the collection.
    mixed_sets: Option<PathBuf>,
    /// Play Album was picked, for Automix to play the list gaplessly.
    album_request: bool,
    /// Tempo and key for tracks in triage, found in the background.
    analysis: AnalysisQueue,
    analysis_shown: analysis::Shown,
    /// Export to USB's dialog and export.
    usb: usb::State,
    phrases_seen: analysis::SeenDecks,
    /// The last stage change or merge of copies, which the track menu can
    /// take back.
    undo: Option<Undo>,
    /// Where files whose moves are waiting (file moves off) will go, by
    /// track id, so they show in the stage subfolder they're headed for.
    pending_moves: HashMap<usize, PathBuf>,
    /// What Collection Health's checks go by.
    health: Facts,
    /// Looks for covers in the background, for the "No cover" check.
    cover_scan: CoverScan,
    /// What the last Collection Health fix did.
    health_status: String,
    /// How many tracks have been taken out of the library.
    removed: usize,
    /// Merged copies' files in quarantine, or on their way there.
    quarantined: usize,
    /// The clean-up's review table, genres page and cover batch.
    cleanup: cleanup::CleanupState,
}

/// The Files source: the folder listed and the thread reading it.
struct FilesState {
    reader: files::Reader,
    /// The folder listed, its audio files in order, and the listing's
    /// generation (older answers are for folders since left).
    dir: Option<PathBuf>,
    paths: Vec<PathBuf>,
    generation: u64,
    /// Tags are still being read.
    reading: bool,
    /// Listings include the files in subfolders.
    recursive: bool,
    /// The next id for a file the collection doesn't have.
    next_id: usize,
    /// Tracks added so far from the folder being added to the collection.
    added: usize,
}

/// The rail sources that are streaming services: their source, name, and
/// how matches set by hand are saved for them.
const STREAMS: [(i32, &str, &str); 5] = [
    (SPOTIFY, "Spotify", "spotify"),
    (TIDAL, "TIDAL", "tidal"),
    (APPLE_MUSIC, "Apple Music", "applemusic"),
    (SOUNDCLOUD, "SoundCloud", "soundcloud"),
    (MIXCLOUD, "Mixcloud", "mixcloud"),
];

/// A streaming service: its worker, and what it has sent.
struct Stream {
    source: i32,
    name: &'static str,
    id: &'static str,
    service: Service,
    state: StreamState,
    user: String,
    /// Progress or an error to show while signing in, or what copying a
    /// playlist did.
    status: String,
}

#[derive(Default)]
struct StreamState {
    connected: bool,
    playlists: Vec<spotify::Playlist>,
    tracks: HashMap<String, Vec<StreamTrack>>,
    loading: HashSet<String>,
    errors: HashMap<String, String>,
    /// Matches the DJ set by hand, by [`StreamTrack::match_key`]: the
    /// collection track's id, or None for none of them.
    manual: HashMap<String, Option<usize>>,
    /// What the match menu offers, by index into the library.
    candidates: Vec<usize>,
}

/// What the list shows of a track, from the collection or Spotify.
struct RowData<'a> {
    title: &'a str,
    artist: String,
    album: &'a str,
    genre: &'a str,
    bpm: f64,
    key: &'a str,
    duration: f64,
    rating: u8,
    local: Option<usize>,
    /// A Spotify track: where it is in its playlist.
    stream: Option<usize>,
}

/// Mixxx's library controls, which controller mappings use to browse and load.
pub(crate) struct LibraryControls {
    /// Steps up (negative) or down (positive) through the focused list.
    steps: Vec<(ControlHandle, f64)>,
    focus: [ControlHandle; 3],
    go_to_item: ControlHandle,
    load_first_stopped: ControlHandle,
    load: [ControlHandle; NUM_DECKS],
    load_and_play: [ControlHandle; NUM_DECKS],
    add_to_automix: [ControlHandle; 2],
    /// Opens the Linked tab (the DDJ-FLX4's SHIFT + left LOAD).
    show_linked: ControlHandle,
    folder_steps: [(ControlHandle, f64); 3],
    /// Opens or closes the folder picked in the folders pane.
    toggle_folder: ControlHandle,
    /// Which pane the browse knob moves in, as Mixxx's `focused_widget`:
    /// 2 the folders, 3 the track list.
    focused_widget: ControlHandle,
}

impl LibraryControls {
    pub(crate) fn create(reg: &ControlRegistry) -> Self {
        let lib = |k: &str| reg.free("[Library]", k);
        let playlist = |k: &str| reg.free("[Playlist]", k);
        Self {
            steps: vec![
                (lib("MoveVertical"), 1.0),
                (lib("ScrollVertical"), 1.0),
                (lib("MoveDown"), 1.0),
                (lib("MoveUp"), -1.0),
                (playlist("SelectTrackKnob"), 1.0),
                (playlist("SelectNextTrack"), 1.0),
                (playlist("SelectPrevTrack"), -1.0),
            ],
            focus: [
                lib("MoveFocusForward"),
                lib("MoveFocusBackward"),
                lib("MoveFocus"),
            ],
            go_to_item: lib("GoToItem"),
            load_first_stopped: playlist("LoadSelectedIntoFirstStopped"),
            load: std::array::from_fn(|d| reg.free(&deck_group(d), "LoadSelectedTrack")),
            load_and_play: std::array::from_fn(|d| {
                reg.free(&deck_group(d), "LoadSelectedTrackAndPlay")
            }),
            add_to_automix: [lib("AutoDjAddBottom"), lib("AutoDjAddTop")],
            show_linked: lib("ShowLinked"),
            folder_steps: [
                (playlist("SelectNextPlaylist"), 1.0),
                (playlist("SelectPrevPlaylist"), -1.0),
                (playlist("SelectPlaylist"), 1.0),
            ],
            toggle_folder: playlist("ToggleSelectedSidebarItem"),
            focused_widget: lib("focused_widget"),
        }
    }
}

/// Takes the presses or steps a controller has accumulated in `c`.
fn take(c: &ControlHandle) -> f64 {
    let v = c.get();
    if v != 0.0 {
        c.set(c.get() - v);
    }
    v
}

/// Automix's view of the browser: its Automix list, and the engine to load
/// tracks with.
pub struct AutomixHost<'a> {
    pub browser: &'a mut Browser,
    pub engine: &'a AudioEngine,
}

impl crate::automix::Host for AutomixHost<'_> {
    fn take_next(&mut self) -> Option<Arc<Track>> {
        self.browser.take_automix_next()
    }

    fn queued(&self) -> usize {
        self.browser.automix_len()
    }

    fn load(&mut self, deck: usize, track: Arc<Track>) {
        self.engine.load_track(deck, track);
    }
}

const ROW_PAYLOAD: &str = "opendj-row:";
/// Rows of a side panel tab: `opendj-side:<tab>:<rows>`.
const SIDE_PAYLOAD: &str = "opendj-side:";
/// The track on a deck, picked up by its cover: `opendj-deck:<deck>`.
const DECK_PAYLOAD: &str = "opendj-deck:";

/// Where a drag from inside the app comes from.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Dragged {
    /// Rows of the track list, first to last.
    Rows(usize, usize),
    /// Rows of a side panel tab, first to last.
    Side(usize, usize, usize),
    /// The track loaded on a deck.
    Deck(usize),
}

/// One row, or a shift-picked range written as `first-last`.
fn parse_rows(rows: &str) -> Option<(usize, usize)> {
    match rows.split_once('-') {
        Some((first, last)) => {
            let (first, last) = (first.parse().ok()?, last.parse().ok()?);
            (first <= last).then_some((first, last))
        }
        None => rows.parse().ok().map(|row| (row, row)),
    }
}

fn parse_dragged(text: &str) -> Option<Dragged> {
    if let Some(rows) = text.strip_prefix(ROW_PAYLOAD) {
        let (first, last) = parse_rows(rows)?;
        Some(Dragged::Rows(first, last))
    } else if let Some(rest) = text.strip_prefix(SIDE_PAYLOAD) {
        let (tab, rows) = rest.split_once(':')?;
        let (first, last) = parse_rows(rows)?;
        Some(Dragged::Side(tab.parse().ok()?, first, last))
    } else {
        let deck = text.strip_prefix(DECK_PAYLOAD)?.parse().ok()?;
        (deck < NUM_DECKS).then_some(Dragged::Deck(deck))
    }
}

/// What a drag from inside the app carries, if it is one.
fn dragged(data: &DataTransfer) -> Option<Dragged> {
    parse_dragged(&data.plain_text().ok()?)
}

/// Moves `list[first..=last]` to go in before `at` (an index into the list
/// as it was), keeping their order.
fn move_block<T>(list: &mut Vec<T>, first: usize, last: usize, at: usize) {
    let len = list.len();
    if first >= len {
        return;
    }
    let last = last.min(len - 1);
    let at = at.min(len);
    if (first..=last + 1).contains(&at) {
        return;
    }
    let block: Vec<T> = list.drain(first..=last).collect();
    let at = if at > last { at - block.len() } else { at };
    list.splice(at..at, block);
}

/// A file's name without its extension.
pub fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn dropped_file(data: &DataTransfer) -> Option<PathBuf> {
    data.file_paths()
        .ok()?
        .find(|p| is_audio(p))
        .map(Path::to_path_buf)
}

fn pick(library_len: usize, seed: usize, count: usize) -> Vec<usize> {
    (0..count)
        .map(|i| (seed * 31 + i * 17 + i * i * 7) % library_len)
        .collect::<Vec<_>>()
        .into_iter()
        .fold(Vec::new(), |mut v, x| {
            if !v.contains(&x) {
                v.push(x);
            }
            v
        })
}

/// Lists of the library's groupings and tags: one per grouping and one per
/// hashtag, each a smart list of the tracks that carry it.
fn imported_tree(library: &[Arc<Track>]) -> Vec<Node> {
    let mut groupings: Vec<&str> = Vec::new();
    let mut labels: HashMap<&str, usize> = HashMap::new();
    for t in library {
        let g = t.info.grouping.as_str();
        if !g.is_empty() && !groupings.contains(&g) {
            groupings.push(g);
        }
        for l in &t.info.labels {
            *labels.entry(l.as_str()).or_default() += 1;
        }
    }
    if groupings.is_empty() && labels.is_empty() {
        return Vec::new();
    }
    // "11 YOGA" sorts after "3 JNY": by the leading number, then by name.
    groupings.sort_by_key(|g| {
        let n: u32 = g
            .split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(u32::MAX);
        (n, g.to_string())
    });
    let mut labels: Vec<(&str, usize)> = labels.into_iter().collect();
    labels.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let node = |name: String, depth, kind, contents| Node::new(name, depth, kind, contents);
    let mut v = vec![node("Groupings and Tags".into(), 0, 1, Contents::Folder)];
    if !groupings.is_empty() {
        v.push(node("Groupings".into(), 1, 1, Contents::Folder));
        v.extend(
            groupings
                .iter()
                .map(|g| node(g.to_string(), 2, 3, Contents::Grouping(g.to_string()))),
        );
    }
    if !labels.is_empty() {
        v.push(node("Tags".into(), 1, 1, Contents::Folder));
        v.extend(
            labels
                .iter()
                .map(|(l, _)| node(l.to_string(), 2, 3, Contents::Label(l.to_string()))),
        );
    }
    // Tags start folded: there can be many.
    if let Some(tags) = v.iter_mut().find(|n| n.name == "Tags" && n.depth == 1) {
        tags.expanded = false;
    }
    v
}

fn sample_tree(n: usize) -> Vec<Node> {
    let node = |name: &str, depth, kind, contents| Node::new(name, depth, kind, contents);
    let list = |name: &str, depth, seed| {
        node(name, depth, 2, Contents::List(pick(n, seed, 12 + seed % 9)))
    };
    let mut v = vec![
        node("Collection", 0, 0, Contents::All),
        node("My Lists", 0, 1, Contents::Folder),
        list("Warm-up", 1, 1),
        list("Peak Time", 1, 2),
        list("Closing", 1, 3),
        node("Sets", 1, 1, Contents::Folder),
        list("Friday 10-09-26", 2, 4),
        list("Saturday 10-10-26", 2, 5),
        list("Sunday Sessions", 2, 6),
        node("Backing", 1, 1, Contents::Folder),
        list("Ambient", 2, 7),
        list("Edits", 2, 8),
        node("Smart Lists", 0, 1, Contents::Folder),
        node("Mixes with deck", 1, 3, Contents::Query("~deck".into())),
        node("120-124 BPM", 1, 3, Contents::Query("bpm:120-124".into())),
        node("125-128 BPM", 1, 3, Contents::Query("bpm:125-128".into())),
        node("Top rated", 1, 3, Contents::Query("rating:>=4".into())),
    ];
    // Start with "Backing" collapsed so the tree shows both states.
    v[9].expanded = false;
    v
}

impl Browser {
    pub fn install(
        app: &AppWindow,
        library: Vec<Arc<Track>>,
        db: Option<Library>,
        library_path: Option<PathBuf>,
        engine: Rc<AudioEngine>,
        registry: &ControlRegistry,
    ) -> Rc<RefCell<Self>> {
        let inbox_settings = crate::settings::inbox();
        let (inbox_on, inbox_mode) = (inbox_settings.on, inbox_settings.mode);
        let stores = Stores::start(stores::configured());
        let store_count = stores.stores.len();
        let nodes = sample_tree(library.len());
        let b = Rc::new(RefCell::new(Self {
            app: app.as_weak(),
            library: Vec::new(),
            nodes,
            visible_nodes: Vec::new(),
            current_node: 0,
            search: String::new(),
            sort_column: 1,
            ascending: true,
            rows: Vec::new(),
            stream_rows: Vec::new(),
            loaded: [None; NUM_DECKS],
            decks_shown: 2,
            side: [Vec::new(), Vec::new()],
            shown_side_tab: 0,
            linked_to: None,
            linked: Vec::new(),
            link_rows: 0,
            engine,
            play: std::array::from_fn(|d| {
                registry.get(&deck_group(d), "play").expect("play control")
            }),
            deck_key: std::array::from_fn(|d| {
                registry.get(&deck_group(d), "key").expect("key control")
            }),
            key_match_range: registry
                .get("[Controls]", "key_match_on_load")
                .expect("key matching control"),
            key_rules: (key::Mixing::Harmonic, 1),
            lib: LibraryControls::create(registry),
            folders_focused: false,
            play_when_loaded: [None; NUM_DECKS],
            streams: STREAMS
                .iter()
                .map(|&(source, name, id)| Stream {
                    source,
                    name,
                    id,
                    service: match source {
                        TIDAL => crate::tidal::start(),
                        MIXCLOUD => crate::mixcloud::start(),
                        SOUNDCLOUD => crate::soundcloud::start(),
                        APPLE_MUSIC => crate::apple_music::start(),
                        _ => Service::spotify(),
                    },
                    state: StreamState::default(),
                    user: String::new(),
                    status: String::new(),
                })
                .collect(),
            store: StoreState {
                stores,
                generation: 0,
                query: String::new(),
                answers: vec![None; store_count],
                wanted: Vec::new(),
                rows: Vec::new(),
                download: String::new(),
            },
            inbox: Inbox::start(
                inbox_settings,
                library_path,
                opendj_engine::scan::default_music_dir(),
                stores::configured(),
            ),
            collection_stale: false,
            tree: COLLECTION,
            parked: HashMap::new(),
            camelot: crate::settings::camelot_keys(),
            art: Art::start(),
            waves: Waves::start(),
            wave_rows: Vec::new(),
            shown_rows: (0, 0),
            list: Rc::new(VecModel::default()),
            db,
            index_of: HashMap::new(),
            path_index: HashMap::new(),
            files: FilesState {
                reader: files::Reader::start(),
                dir: None,
                paths: Vec::new(),
                generation: 0,
                reading: false,
                recursive: files::recursive(),
                next_id: BROWSED_BASE,
                added: 0,
            },
            indexed: Vec::new(),
            duplicates: HashSet::new(),
            dupe_groups: OnceCell::new(),
            links: HashSet::new(),
            deck_ref: None,
            deck_track: None,
            history_tab: false,
            other_tab_node: 0,
            session: None,
            menu_lists: Vec::new(),
            menu_sets: Vec::new(),
            crates: HashMap::new(),
            list_health: Default::default(),
            hotcues: std::array::from_fn(|d| {
                (1..=NUM_HOTCUES)
                    .map(|n| {
                        let c = |k: &str| {
                            registry
                                .get(&deck_group(d), &format!("hotcue_{n}_{k}"))
                                .expect("hotcue control")
                        };
                        (c("enabled"), c("position"), c("endposition"))
                    })
                    .collect()
            }),
            watch: Default::default(),
            info: info::InfoState::new(registry),
            filter: filters::FilterState::new(),
            columns: columns::ColumnState::new(&crate::settings::field_names()),
            folders: crate::settings::stage_folders(),
            mixed_sets: crate::settings::mixed_sets_folder(),
            album_request: false,
            analysis: AnalysisQueue::start(),
            analysis_shown: Default::default(),
            usb: Default::default(),
            phrases_seen: Default::default(),
            undo: None,
            pending_moves: HashMap::new(),
            health: Facts::default(),
            cover_scan: CoverScan::start(),
            health_status: String::new(),
            removed: 0,
            quarantined: 0,
            cleanup: Default::default(),
        }));
        b.borrow_mut().set_library(library);
        app.global::<KeyNotation>().set_camelot(b.borrow().camelot);
        {
            let ui = app.global::<StoreUi>();
            ui.set_streaming(STREAMING_ENABLED);
            ui.set_stores(STORES_ENABLED);
            ui.set_inbox_on(inbox_on);
            ui.set_inbox_mode(match inbox_mode {
                Mode::Move => 0,
                Mode::Copy => 1,
                Mode::Leave => 2,
            });
            if b.borrow().inbox.dir.is_none() {
                ui.set_inbox_status("No folder to watch.".into());
            }
        }
        b.borrow_mut().refresh_tree();
        b.borrow_mut().refresh_rows();
        b.borrow_mut().refresh_side();

        let weak = Rc::downgrade(&b);
        let with = move |f: Box<dyn Fn(&mut Browser)>| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        app.on_select_folder(move |i| {
            w(Box::new(move |b| {
                if let Some(&n) = b.visible_nodes.get(i as usize) {
                    b.current_node = n;
                    b.health_status.clear();
                    b.close_cleanup();
                    if let Some(app) = b.app.upgrade() {
                        app.set_current_track(-1);
                    }
                    b.refresh_tree();
                    b.refresh_rows();
                    b.fetch_current_playlist();
                }
            }))
        });
        let w = with.clone();
        app.on_toggle_folder(move |i| {
            w(Box::new(move |b| {
                if let Some(&n) = b.visible_nodes.get(i as usize) {
                    if b.nodes[n].lazy {
                        open_dir(&mut b.nodes, n);
                    } else {
                        b.nodes[n].expanded = !b.nodes[n].expanded;
                    }
                    b.refresh_tree();
                }
            }))
        });
        let w = with.clone();
        app.on_sort(move |c| {
            w(Box::new(move |b| {
                if b.sort_column == c {
                    b.ascending = !b.ascending;
                } else {
                    b.sort_column = c;
                    b.ascending = true;
                }
                b.refresh_rows_keeping_current();
            }))
        });
        let w = with.clone();
        app.on_search(move |text| {
            let text = text.to_string();
            w(Box::new(move |b| {
                b.search = text.clone();
                if is_stores(b.tree) {
                    b.search_stores(StoreQuery::text(&text));
                }
                b.refresh_rows();
            }))
        });
        let w = with.clone();
        app.on_activate_track(move |row| {
            w(Box::new(move |b| {
                if is_stores(b.tree) {
                    b.activate_store_row(row as usize);
                } else if let Some(&Some(id)) = b.rows.get(row as usize) {
                    b.adopt(&[id]);
                    b.load_to_free_deck(id);
                } else if let Some(t) = b.stream_track(row as usize) {
                    // Not in the collection: it plays on the service's page.
                    open_in_browser(&t.page);
                }
            }))
        });
        let w = with.clone();
        app.on_load_track(move |deck, row| {
            w(Box::new(move |b| {
                if let Some(&Some(id)) = b.rows.get(row as usize) {
                    if b.playable(id) {
                        b.adopt(&[id]);
                        b.engine.load_track(deck as usize, b.library[id].clone());
                    }
                }
            }))
        });
        // Drag and drop onto a deck. The payload only names the list row, so
        // building it never touches the browser while the list is refreshing.
        let drag = app.global::<TrackDrag>();
        drag.on_payload(|first, last| {
            let rows = if last > first {
                format!("{first}-{last}")
            } else {
                first.to_string()
            };
            DataTransfer::from(SharedString::from(format!("{ROW_PAYLOAD}{rows}")))
        });
        drag.on_side_payload(|tab, first, last| {
            let rows = if last > first {
                format!("{first}-{last}")
            } else {
                first.to_string()
            };
            DataTransfer::from(SharedString::from(format!("{SIDE_PAYLOAD}{tab}:{rows}")))
        });
        drag.on_deck_payload(|deck| {
            DataTransfer::from(SharedString::from(format!("{DECK_PAYLOAD}{deck}")))
        });
        drag.on_accepts(|data| dragged(&data).is_some() || dropped_file(&data).is_some());
        let w = with.clone();
        drag.on_drop(move |deck, data| w(Box::new(move |b| b.load_dropped(deck as usize, &data))));
        let w = with.clone();
        drag.on_drop_side(move |index, data| {
            w(Box::new(move |b| {
                let tab = b.side_tab();
                let at = index.max(0) as usize;
                // Rows dragged within their own list move there.
                if let Some(Dragged::Side(from, first, last)) = dragged(&data) {
                    if from == tab {
                        if tab != LINKED_TAB {
                            move_block(&mut b.side[tab], first, last, at);
                            b.refresh_side();
                        }
                        return;
                    }
                }
                let ids = b.dropped_tracks(&data);
                if ids.is_empty() {
                    return;
                }
                if tab == LINKED_TAB {
                    for id in ids {
                        b.link(id);
                    }
                    return;
                }
                let list = &mut b.side[tab];
                let at = at.min(list.len());
                list.splice(at..at, ids);
                b.refresh_side();
            }))
        });
        let w = with.clone();
        app.global::<LibraryEdit>()
            .on_add_to_automix(move |first, last| {
                w(Box::new(move |b| {
                    let ids = b.playable_rows(first.max(0) as usize, last.max(0) as usize);
                    if !ids.is_empty() {
                        b.side[0].extend(ids);
                        b.refresh_side();
                    }
                }))
            });
        let w = with.clone();
        app.on_side_activate(move |row| {
            w(Box::new(move |b| {
                if let Some(id) = b.side_id(row as usize) {
                    b.load_to_free_deck(id);
                }
            }))
        });
        let w = with.clone();
        app.on_side_remove(move |first, last| {
            w(Box::new(move |b| {
                let tab = b.side_tab();
                let first = first.max(0) as usize;
                let last = last.max(0) as usize;
                if tab == LINKED_TAB {
                    // From the bottom up, so the rows above stay put.
                    for row in (first..=last).rev() {
                        b.unlink_row(row);
                    }
                } else if first < b.side[tab].len() {
                    let end = (last + 1).min(b.side[tab].len());
                    b.side[tab].drain(first..end);
                    b.refresh_side();
                }
            }))
        });
        let w = with.clone();
        app.on_select_source(move |s| w(Box::new(move |b| b.select_source(s))));
        // The sources the rail shows, by name so new ones appear by default.
        let hidden = crate::settings::hidden_sources();
        let names = all_source_names();
        let shown: Vec<bool> = names
            .iter()
            .map(|n| !hidden.iter().any(|h| h == n))
            .collect();
        app.set_sources_shown(std::rc::Rc::new(slint::VecModel::from(shown)).into());
        let weak = app.as_weak();
        app.on_source_shown_changed(move |_, _| {
            let Some(app) = weak.upgrade() else { return };
            let shown = app.get_sources_shown();
            let hidden = names
                .iter()
                .zip(shown.iter())
                .filter(|(_, on)| !on)
                .map(|(n, _)| n.to_string())
                .collect();
            crate::settings::set_hidden_sources(hidden);
        });
        let ui = app.global::<StoreUi>();
        let w = with.clone();
        ui.on_find(move |row| w(Box::new(move |b| b.find_in_stores(row as usize))));
        let w = with.clone();
        ui.on_set_inbox(move |on| {
            w(Box::new(move |b| {
                crate::settings::set_inbox_on(on);
                b.inbox.send(crate::inbox::Request::SetOn(on));
                if let Some(app) = b.app.upgrade() {
                    app.global::<StoreUi>().set_inbox_on(on);
                }
            }))
        });
        let w = with.clone();
        ui.on_set_inbox_mode(move |m| {
            w(Box::new(move |b| {
                let mode = match m {
                    1 => Mode::Copy,
                    2 => Mode::Leave,
                    _ => Mode::Move,
                };
                crate::settings::set_inbox_mode(mode);
                b.inbox.send(crate::inbox::Request::SetMode(mode));
                if let Some(app) = b.app.upgrade() {
                    app.global::<StoreUi>().set_inbox_mode(m);
                }
            }))
        });
        let w = with.clone();
        app.on_spotify_connect(move |id| {
            let id = id.to_string();
            w(Box::new(move |b| {
                let Some(st) = b.shown_stream_mut() else {
                    return;
                };
                st.status = format!("Opening {} in your browser…", st.name);
                // SoundCloud's and Apple Music's forms send more after the
                // client ID (or Team ID); only that stays in the box.
                st.service.client_id = id.split_whitespace().next().unwrap_or_default().into();
                st.service.send(Request::Connect(id.clone()));
                b.show_stream();
            }))
        });
        let w = with.clone();
        app.on_spotify_disconnect(move || {
            w(Box::new(|b| {
                if let Some(st) = b.shown_stream_mut() {
                    st.service.send(Request::Disconnect);
                }
            }))
        });
        let w = with.clone();
        app.on_spotify_copy(move || w(Box::new(|b| b.copy_stream_playlist())));
        {
            let ui = app.global::<StreamMatch>();
            let w = with.clone();
            ui.on_open(move |row| w(Box::new(move |b| b.open_match(row as usize))));
            let w = with.clone();
            ui.on_search(move |text| {
                let text = text.to_string();
                w(Box::new(move |b| b.search_match(&text)))
            });
            let w = with.clone();
            ui.on_pick(move |row, c| {
                w(Box::new(move |b| {
                    let pick = b
                        .shown_stream()
                        .and_then(|st| st.state.candidates.get(c as usize).copied());
                    if pick.is_some() {
                        b.set_match(row as usize, pick, false);
                    }
                }))
            });
            let w = with.clone();
            ui.on_none(move |row| w(Box::new(move |b| b.set_match(row as usize, None, false))));
            let w = with.clone();
            ui.on_auto(move |row| w(Box::new(move |b| b.set_match(row as usize, None, true))));
        }
        Self::install_editing(app, &b);
        Self::install_files(app, &b);
        Self::install_info(app, &b);
        Self::install_filters(app, &b);
        Self::install_columns(app, &b);
        b
    }

    /// Hooks up the Files source's menus and its subfolders switch.
    fn install_files(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let ui = app.global::<FilesUi>();
        ui.set_recursive(b.borrow().files.recursive);
        ui.set_file_manager(files::file_manager().into());
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        ui.on_set_recursive(move |on| {
            w(&|b| {
                b.files.recursive = on;
                files::set_recursive(on);
                b.files.dir = None;
                b.list_current_dir();
            })
        });
        let w = with.clone();
        ui.on_pin(move |i| w(&|b| b.toggle_pin(i as usize)));
        let w = with.clone();
        ui.on_add_folder(move |i| {
            w(&|b| {
                if let Some(path) = b.dir_at(i as usize) {
                    b.set_files_status(format!(
                        "Adding {} to the collection…",
                        files::folder_name(&path)
                    ));
                    b.files.reader.add(&path);
                }
            })
        });
        let w = with.clone();
        ui.on_reveal_folder(move |i| {
            w(&|b| {
                if let Some(path) = b.dir_at(i as usize) {
                    files::reveal(&path);
                }
            })
        });
        let w = with.clone();
        ui.on_add_track(move |row| {
            w(&|b| {
                if let Some(&Some(id)) = b.rows.get(row as usize) {
                    b.adopt(&[id]);
                    b.refresh_rows();
                }
            })
        });
        let w = with.clone();
        ui.on_reveal_track(move |row| {
            w(&|b| {
                if let Some(path) = b
                    .rows
                    .get(row as usize)
                    .copied()
                    .flatten()
                    .and_then(|id| file_path(&b.library[id]))
                {
                    files::reveal(path);
                }
            })
        });
        let w = with.clone();
        ui.on_in_collection(move |row| {
            let found = std::cell::Cell::new(false);
            w(&|b| {
                found.set(
                    b.rows
                        .get(row as usize)
                        .copied()
                        .flatten()
                        .is_some_and(|id| !is_browsed(&b.library[id])),
                )
            });
            found.get()
        });
        let w = with.clone();
        ui.on_has_file(move |row| {
            let found = std::cell::Cell::new(false);
            w(&|b| {
                found.set(
                    b.rows
                        .get(row as usize)
                        .copied()
                        .flatten()
                        .is_some_and(|id| file_path(&b.library[id]).is_some()),
                )
            });
            found.get()
        });
    }

    /// Hooks up making, naming and deleting lists, adding tracks to them,
    /// ratings and colours, and the history tab.
    fn install_editing(app: &AppWindow, b: &Rc<RefCell<Self>>) {
        let edit = app.global::<LibraryEdit>();
        let weak = Rc::downgrade(b);
        let with = move |f: &dyn Fn(&mut Browser)| {
            if let Some(b) = weak.upgrade() {
                f(&mut b.borrow_mut());
            }
        };
        let w = with.clone();
        edit.on_create(move |kind| {
            let row = std::cell::Cell::new(-1);
            w(&|b| {
                row.set(if kind == 2 {
                    b.create_set()
                } else {
                    b.create_list(kind == 1)
                })
            });
            row.get()
        });
        let w = with.clone();
        edit.on_rename(move |i, name| w(&|b| b.rename_node(i as usize, &name)));
        let w = with.clone();
        edit.on_delete(move |i| w(&|b| b.delete_node(i as usize)));
        edit.on_accepts_drop(|_, data| dragged(&data).is_some());
        let w = with.clone();
        edit.on_drop(move |i, data| {
            w(&|b| {
                if dragged(&data).is_some() {
                    let ids = b.dropped_tracks(&data);
                    if let Some(list) = b.playlist_at(i as usize) {
                        b.add_to_playlist(&ids, list);
                    } else if let Some((stage, dir)) = b.stage_at(i as usize) {
                        b.change_stage(&ids, stage, dir.as_deref());
                    }
                }
            })
        });
        let w = with.clone();
        edit.on_set_rating(move |row, n| w(&|b| b.set_rating(row as usize, n.clamp(0, 5) as u8)));
        let w = with.clone();
        edit.on_add_to_list(move |first, last, list| {
            w(&|b| {
                if let Some(&id) = b.menu_lists.get(list as usize) {
                    b.add_rows_to_playlist(first.max(0) as usize, last.max(0) as usize, id);
                }
            })
        });
        let w = with.clone();
        edit.on_remove_from_list(move |row| w(&|b| b.remove_row_from_list(row as usize)));
        let w = with.clone();
        edit.on_add_to_set(move |first, last, set| {
            w(&|b| {
                b.add_rows_to_set(
                    first.max(0) as usize,
                    last.max(0) as usize,
                    set.max(0) as usize,
                )
            })
        });
        let w = with.clone();
        edit.on_show_history(move |on| w(&|b| b.show_history(on)));
        let w = with.clone();
        edit.on_save_search(move |text| {
            let row = std::cell::Cell::new(-1);
            w(&|b| row.set(b.save_search(&text)));
            row.get()
        });
        let w = with.clone();
        edit.on_rule_at(move |i| {
            let rule = RefCell::new(String::new());
            w(&|b| *rule.borrow_mut() = b.rule_at(i as usize));
            rule.into_inner().into()
        });
        let w = with.clone();
        edit.on_set_rule(move |i, text| w(&|b| b.set_rule(i as usize, &text)));
        Self::install_staging(&edit, &with);
        Self::install_health(&app.global::<HealthUi>(), &with);
        Self::install_cleanup(&app.global::<crate::CleanupUi>(), &with);
        Self::install_analysis(&edit, &with);
        Self::install_albums(&edit, &with);
        Self::install_usb_export(app, &edit, &with);
    }

    /// Saves a search as a smart list at the top level (under "Smart
    /// Lists"), named after the search, and selects it. Returns its row.
    fn save_search(&mut self, text: &str) -> i32 {
        let text = query::and_text(text.trim(), &self.filter.saved_text());
        if text.is_empty() || !self.saved() || self.tree != COLLECTION || self.history_tab {
            return -1;
        }
        let Some(db) = &self.db else { return -1 };
        let name: String = text.chars().take(40).collect();
        match db.create_smart_list(&name, None, &text) {
            Ok(id) => {
                self.search.clear();
                self.filter.clear_all();
                self.refresh_filter_ui();
                self.reload_tree(Some(id));
                self.visible_nodes
                    .iter()
                    .position(|&n| n == self.current_node)
                    .map_or(-1, |p| p as i32)
            }
            Err(e) => {
                eprintln!("can't save the smart list: {e}");
                -1
            }
        }
    }

    /// The search of the smart list at `row` of the tree.
    fn rule_at(&self, row: usize) -> String {
        match self.visible_nodes.get(row).and_then(|&n| self.nodes.get(n)) {
            Some(Node {
                contents: Contents::Query(q),
                ..
            }) => q.clone(),
            _ => String::new(),
        }
    }

    fn set_rule(&mut self, row: usize, text: &str) {
        let (Some(id), Some(db)) = (self.playlist_at(row), &self.db) else {
            return;
        };
        if let Err(e) = db.set_query(id, text) {
            eprintln!("can't change the smart list: {e}");
        }
        self.reload_tree(None);
    }

    /// Shows the tree of the source picked in the rail. Sources without a
    /// tree of their own keep the current one underneath their placeholder.
    fn select_source(&mut self, source: i32) {
        let usable = match source {
            COLLECTION | FILES | ALBUMS | MIXED_SETS => true,
            SPOTIFY | TIDAL | APPLE_MUSIC | SOUNDCLOUD | MIXCLOUD => STREAMING_ENABLED,
            STORES => STORES_ENABLED,
            _ => false,
        };
        if !usable || source == self.tree {
            return;
        }
        let leaving = Parked {
            nodes: std::mem::take(&mut self.nodes),
            current: self.current_node,
            sort: (self.sort_column, self.ascending),
        };
        self.parked.insert(self.tree, leaving);
        // Spotify playlists and store results start in their own order.
        let back = self.parked.remove(&source).unwrap_or(Parked {
            nodes: Vec::new(),
            current: 0,
            sort: (-1, true),
        });
        self.tree = source;
        (self.nodes, self.current_node) = (back.nodes, back.current);
        (self.sort_column, self.ascending) = back.sort;
        match source {
            FILES => self.rebuild_files_tree(),
            ALBUMS | MIXED_SETS => self.rebuild_folder_tree(source),
            SPOTIFY | TIDAL | APPLE_MUSIC | SOUNDCLOUD | MIXCLOUD => {
                self.rebuild_stream_tree(source)
            }
            STORES => self.rebuild_stores_tree(),
            _ => {}
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
            app.global::<StoreUi>().set_source(source);
        }
        self.show_stream();
        if source == COLLECTION && self.collection_stale {
            self.collection_stale = false;
            self.reload_tree(None);
        }
        self.refresh_tree();
        self.refresh_rows();
        self.fetch_current_playlist();
        if is_stores(source) && self.store.query != self.search {
            self.search_stores(StoreQuery::text(&self.search.clone()));
        }
    }

    /// The stores tree: Bandcamp, the other stores (together and each),
    /// then the wanted list.
    fn rebuild_stores_tree(&mut self) {
        let stores = &self.store.stores.stores;
        let node = |i: usize| {
            let s = &stores[i];
            let kind = if s.name == "Bandcamp" {
                KIND_BANDCAMP
            } else {
                KIND_LIST
            };
            Node::new(s.name.clone(), 0, kind, Contents::Store(Some(i)))
        };
        let (bandcamp, others): (Vec<usize>, Vec<usize>) =
            (0..stores.len()).partition(|&i| stores[i].name == "Bandcamp");
        let mut nodes: Vec<Node> = bandcamp.into_iter().map(node).collect();
        if !others.is_empty() {
            nodes.push(Node::new("All other stores", 0, 0, Contents::OtherStores));
            nodes.extend(others.into_iter().map(node));
        }
        nodes.push(Node::new("Wanted", 0, 4, Contents::Wanted));
        self.current_node = self.current_node.min(nodes.len() - 1);
        self.nodes = nodes;
        self.store.wanted = self
            .db
            .as_ref()
            .and_then(|db| db.wanted().ok())
            .unwrap_or_default();
    }

    /// Asks the stores, unless they're already answering this.
    fn search_stores(&mut self, query: StoreQuery) {
        if query.text == self.store.query && self.store.generation > 0 {
            return;
        }
        self.store.query = query.text.clone();
        self.store.download.clear();
        self.store.answers = vec![None; self.store.stores.stores.len()];
        self.store.generation = self.store.stores.search(query);
    }

    /// "Find in stores" on a track: show the stores, searching for it.
    fn find_in_stores(&mut self, row: usize) {
        let Some(&Some(id)) = self.rows.get(row) else {
            return;
        };
        let t = self.library[id].clone();
        let query = StoreQuery::track(&t.artist, &t.title);
        self.search = query.text.clone();
        if let Some(app) = self.app.upgrade() {
            app.global::<StoreUi>()
                .set_search_text(query.text.clone().into());
        }
        self.store.query.clear();
        self.store.generation = 0;
        if is_stores(self.tree) {
            self.search_stores(query);
            self.refresh_rows();
        } else {
            self.search_stores(query);
            self.select_source(STORES);
        }
    }

    /// A store row's button: open its page (and remember the track as
    /// wanted), open a store's search page, or download into the inbox.
    fn activate_store_row(&mut self, row: usize) {
        let Some(r) = self.store.rows.get(row).cloned() else {
            return;
        };
        match r {
            StoreRow::Item(store, item) => {
                if let Some(url) = &item.page_url {
                    open_in_browser(url);
                    if item.kind == ItemKind::Track {
                        if let Some(db) = &self.db {
                            let _ = db.add_wanted(&item.store, &item.artist, &item.title, url);
                            self.store.wanted = db.wanted().unwrap_or_default();
                        }
                    }
                } else if item.download.is_some() {
                    match &self.inbox.dir {
                        Some(dir) => self.store.stores.download(store, item, dir.clone()),
                        None => {
                            self.store.download = "There's no inbox folder to download into.".into()
                        }
                    }
                }
            }
            StoreRow::Link(_, url) => {
                open_in_browser(&url);
            }
            StoreRow::Wanted(w) => {
                open_in_browser(&w.page_url);
            }
        }
        self.refresh_rows();
    }

    /// Takes in what the stores and the inbox have done. Called every frame.
    pub fn poll_stores(&mut self, app: &AppWindow) {
        let mut changed = false;
        for event in self.store.stores.poll() {
            match event {
                stores::Event::Results {
                    generation,
                    store,
                    items,
                    error,
                    link,
                } if generation == self.store.generation => {
                    if let Some(a) = self.store.answers.get_mut(store) {
                        *a = Some((items, error, link));
                        changed = true;
                    }
                }
                stores::Event::Results { .. } => {}
                stores::Event::Download(message) => {
                    self.store.download = message;
                    changed = true;
                }
            }
        }
        for event in self.inbox.poll() {
            match event {
                crate::inbox::Event::Status(s) => {
                    app.global::<StoreUi>().set_inbox_status(s.into())
                }
                crate::inbox::Event::Imported(track) => self.add_imported(*track),
            }
        }
        if changed && is_stores(self.tree) {
            self.refresh_rows();
        }
    }

    /// A track the inbox added to the library joins the list in memory.
    pub fn add_imported(&mut self, track: Track) {
        if self.index_of.contains_key(&track.id) {
            return;
        }
        // A file shown in Files makes way for the collection's copy.
        if let Some(i) = file_path(&track).and_then(|p| self.path_index.get(p).copied()) {
            if is_browsed(&self.library[i]) {
                self.index_of.remove(&self.library[i].id);
                self.index_of.insert(track.id, i);
                self.indexed[i] = Indexed::new(&track);
                self.library[i] = Arc::new(track);
                self.collection_stale = true;
                return;
            }
        }
        // Read its row back so it has everything the library knows.
        self.push_track(track);
        if let Some(db) = &self.db {
            self.store.wanted = db.wanted().unwrap_or_default();
        }
        if self.tree == COLLECTION {
            self.reload_tree(None);
        } else {
            self.collection_stale = true;
            if is_stores(self.tree) {
                self.refresh_rows();
            }
        }
    }

    /// The list while the stores are shown: the selected store's results
    /// (or all of them), with links to each store's own search.
    fn refresh_store_rows(&mut self) {
        let ids = self.columns.ids();
        let selected = self
            .nodes
            .get(self.current_node)
            .map(|n| n.contents.clone());
        let mut rows = Vec::new();
        let mut notice = String::new();
        let mut summary = String::new();
        match selected {
            Some(Contents::Wanted) => {
                rows.extend(self.store.wanted.iter().cloned().map(StoreRow::Wanted));
                if rows.is_empty() {
                    notice =
                        "Tracks you click Buy on wait here until they arrive in the inbox.".into();
                }
            }
            Some(which @ (Contents::Store(_) | Contents::OtherStores)) => {
                let bandcamp = self
                    .store
                    .stores
                    .stores
                    .iter()
                    .position(|s| s.name == "Bandcamp");
                let shown = |i: usize| match &which {
                    Contents::Store(w) => w.is_none_or(|w| w == i),
                    _ => Some(i) != bandcamp,
                };
                let mut errors = Vec::new();
                let mut waiting = false;
                for (i, a) in self
                    .store
                    .answers
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| shown(*i))
                {
                    match a {
                        Some((items, error, _)) => {
                            rows.extend(items.iter().cloned().map(|it| StoreRow::Item(i, it)));
                            if let Some(e) = error {
                                errors.push(e.clone());
                            }
                        }
                        None => waiting = true,
                    }
                }
                // Each store's own search page, after the results.
                for (i, a) in self
                    .store
                    .answers
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| shown(*i))
                {
                    if let Some((_, _, Some(link))) = a {
                        rows.push(StoreRow::Link(i, link.clone()));
                    }
                }
                if self.store.query.is_empty() {
                    notice = "Search the stores above, or right-click a track in your collection and choose Find in stores.".into();
                } else if rows.is_empty() && waiting {
                    notice = "Searching…".into();
                } else if rows.is_empty() {
                    notice = errors
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Nothing found.".into());
                }
                summary = match (waiting && !rows.is_empty(), errors.first()) {
                    (true, _) => "Searching…".into(),
                    (false, Some(e)) if !rows.is_empty() => e.clone(),
                    _ => String::new(),
                };
            }
            _ => {}
        }
        if !self.store.download.is_empty() {
            summary = self.store.download.clone();
        }
        let names: Vec<String> = self
            .store
            .stores
            .stores
            .iter()
            .map(|s| s.name.clone())
            .collect();
        let list: Vec<TrackRow> = rows
            .iter()
            .map(|r| {
                let (title, artist, detail, duration, action) = match r {
                    StoreRow::Item(i, it) => (
                        it.title.clone(),
                        it.artist.clone(),
                        if it.release.is_empty() {
                            names[*i].clone()
                        } else {
                            format!("{} · {}", names[*i], it.release)
                        },
                        it.duration,
                        if it.download.is_some() {
                            "Get"
                        } else if it.kind == ItemKind::Track {
                            "Buy"
                        } else {
                            "Open"
                        },
                    ),
                    StoreRow::Link(i, _) => (
                        format!("Search {} for “{}”", names[*i], self.store.query),
                        String::new(),
                        names[*i].clone(),
                        0.0,
                        "Search",
                    ),
                    StoreRow::Wanted(w) => (
                        w.title.clone(),
                        w.artist.clone(),
                        w.store.clone(),
                        0.0,
                        "Buy",
                    ),
                };
                let row = TrackRow {
                    title: title.into(),
                    artist: artist.into(),
                    genre: detail.into(),
                    length: if duration > 0.0 {
                        short_time(duration)
                    } else {
                        SharedString::new()
                    },
                    deck: -1,
                    available: false,
                    action: action.into(),
                    ..Default::default()
                };
                TrackRow {
                    cells: columns::row_cells(&ids, &row, None, None),
                    ..row
                }
            })
            .collect();
        self.store.rows = rows;
        self.rows = vec![None; self.store.rows.len()];
        if let Some(app) = self.app.upgrade() {
            let current = app.get_current_track();
            app.set_folder_duration(summary.into());
            app.set_list_notice(notice.into());
            self.list.set_vec(list);
            app.set_tracks(ModelRc::from(self.list.clone()));
            app.set_sort_column(-1);
            if current >= self.rows.len() as i32 {
                app.set_current_track(-1);
            }
        }
    }

    /// Lists the Spotify playlists as tree nodes, keeping the selection.
    /// Builds the Files tree afresh (drives come and go, pins change),
    /// keeping the folders that were open and the one selected. The first
    /// time, it opens the folder used last, or the music folder.
    fn rebuild_files_tree(&mut self) {
        let open: HashSet<PathBuf> = self
            .nodes
            .iter()
            .filter(|n| n.expanded && !n.lazy)
            .filter_map(|n| n.path().map(Path::to_path_buf))
            .collect();
        let selected = match self.nodes.get(self.current_node).and_then(Node::path) {
            Some(p) => Some(p.to_path_buf()),
            None if self.nodes.is_empty() => files::last_folder(),
            None => None,
        };
        let mut nodes = files_tree(&open);
        let at = selected
            .and_then(|p| {
                nodes
                    .iter()
                    .position(|n| n.path() == Some(&p))
                    .or_else(|| reveal_dir(&mut nodes, &p))
            })
            .or_else(|| {
                nodes
                    .iter()
                    .position(|n| n.name == "Music" && n.kind == KIND_FOLDER)
            })
            .unwrap_or(0);
        self.nodes = nodes;
        self.current_node = at;
        // Opening the tree down to it shows the folder on screen.
        let mut i = at;
        while let Some(parent) = (0..i)
            .rev()
            .find(|&j| self.nodes[j].depth < self.nodes[i].depth)
        {
            self.nodes[parent].expanded = true;
            i = parent;
        }
    }

    /// The folder at a row of the Files tree.
    fn dir_at(&self, row: usize) -> Option<PathBuf> {
        let n = self.nodes.get(*self.visible_nodes.get(row)?)?;
        n.path().map(Path::to_path_buf)
    }

    /// Pins a folder to the top of the Files tree, or unpins it.
    fn toggle_pin(&mut self, row: usize) {
        let Some(&n) = self.visible_nodes.get(row) else {
            return;
        };
        let Some(path) = self.nodes[n].path().map(Path::to_path_buf) else {
            return;
        };
        let mut pinned = files::pinned();
        if self.nodes[n].kind == KIND_PINNED {
            pinned.retain(|p| *p != path);
        } else if !pinned.contains(&path) {
            pinned.push(path);
        }
        files::set_pinned(&pinned);
        self.rebuild_files_tree();
        self.refresh_tree();
    }

    fn set_files_status(&self, text: String) {
        if let Some(app) = self.app.upgrade() {
            app.global::<FilesUi>().set_status(text.into());
        }
    }

    /// Starts listing the folder selected in Files, unless it's listed.
    fn list_current_dir(&mut self) {
        if self.tree != FILES {
            return;
        }
        let Some(dir) = self.nodes.get(self.current_node).and_then(Node::path) else {
            return;
        };
        if self.files.dir.as_deref() == Some(dir) {
            return;
        }
        let dir = dir.to_path_buf();
        files::set_last_folder(&dir);
        self.files.generation = self.files.reader.list(&dir, self.files.recursive);
        self.files.dir = Some(dir);
        self.files.paths.clear();
        self.files.reading = true;
        self.refresh_rows();
    }

    /// Takes in folder listings and tags read for Files, and folders being
    /// added to the collection. Called every frame.
    pub fn poll_files(&mut self) {
        let mut changed = false;
        for found in self.files.reader.poll() {
            match found {
                Found::Listed { generation, paths } => {
                    if generation == self.files.generation {
                        self.files.paths = paths;
                        changed = true;
                    }
                }
                Found::Read {
                    generation,
                    tracks,
                    done,
                } => {
                    for t in tracks {
                        self.add_browsed(t);
                    }
                    if generation == self.files.generation {
                        changed = true;
                        if done {
                            self.files.reading = false;
                        }
                    }
                }
                Found::ToAdd {
                    known,
                    tracks,
                    done,
                } => {
                    let mut adding: Vec<usize> = known
                        .iter()
                        .filter_map(|p| self.path_index.get(p).copied())
                        .collect();
                    adding.extend(tracks.into_iter().filter_map(|t| self.add_browsed(t)));
                    let added = self.adopt(&adding);
                    self.files.added += added;
                    if done {
                        let n = std::mem::take(&mut self.files.added);
                        self.set_files_status(match n {
                            0 => "Everything there is in the collection already.".into(),
                            1 => "Added 1 track to the collection.".into(),
                            n => format!("Added {n} tracks to the collection."),
                        });
                    }
                    changed = true;
                }
            }
        }
        if changed && self.tree == FILES {
            self.refresh_rows();
        }
    }

    /// Keeps a file read for Files that the collection doesn't have, with
    /// an id of its own. Returns where it is in `library`.
    fn add_browsed(&mut self, mut t: Track) -> Option<usize> {
        let path = file_path(&t)?.to_path_buf();
        if let Some(&i) = self.path_index.get(&path) {
            return Some(i);
        }
        t.id = self.files.next_id;
        self.files.next_id += 1;
        Some(self.push_track(t))
    }

    /// Appends a track to `library` and its indexes; returns its index.
    fn push_track(&mut self, t: Track) -> usize {
        let index = self.library.len();
        self.index_of.insert(t.id, index);
        if let Some(p) = file_path(&t) {
            self.path_index.insert(p.to_path_buf(), index);
        }
        self.indexed.push(Indexed::new(&t));
        self.library.push(Arc::new(t));
        index
    }

    /// Adds files shown in Files (by index into `library`) to the
    /// collection, as loading, rating or listing them does. Returns how
    /// many joined it.
    fn adopt(&mut self, indices: &[usize]) -> usize {
        let mut pick: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| self.library.get(i).is_some_and(|t| is_browsed(t)))
            .collect();
        pick.sort_unstable();
        pick.dedup();
        let Some(db) = &mut self.db else { return 0 };
        if pick.is_empty() {
            return 0;
        }
        let tracks: Vec<Track> = pick.iter().map(|&i| (*self.library[i]).clone()).collect();
        let added = match db.add_files(&tracks) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("can't add to the library: {e}");
                return 0;
            }
        };
        let mut known = self.files.reader.collection.write().ok();
        for (&i, t) in pick.iter().zip(added) {
            self.index_of.remove(&self.library[i].id);
            self.index_of.insert(t.id, i);
            if let (Some(k), Some(p)) = (known.as_mut(), file_path(&t)) {
                k.insert(p.to_path_buf());
            }
            self.library[i] = Arc::new(t);
        }
        drop(known);
        self.collection_stale = true;
        pick.len()
    }

    /// Lists a streaming service's playlists as tree nodes, keeping the
    /// selection.
    fn rebuild_stream_tree(&mut self, source: i32) {
        let Some(st) = self.streams.iter().find(|s| s.source == source) else {
            return;
        };
        let nodes: Vec<Node> = st
            .state
            .playlists
            .iter()
            .map(|p| {
                Node::new(
                    p.name.clone(),
                    0,
                    if p.key == spotify::LIKED { 4 } else { 2 },
                    Contents::Stream(p.key.clone()),
                )
            })
            .collect();
        let parked = self.parked.entry(source).or_insert(Parked {
            nodes: Vec::new(),
            current: 0,
            sort: (-1, true),
        });
        let (nodes_ref, current) = if self.tree == source {
            (&mut self.nodes, &mut self.current_node)
        } else {
            (&mut parked.nodes, &mut parked.current)
        };
        let selected = match nodes_ref.get(*current).map(|n| &n.contents) {
            Some(Contents::Stream(k)) => Some(k.clone()),
            _ => None,
        };
        *current = selected
            .and_then(|k| {
                nodes
                    .iter()
                    .position(|n| matches!(&n.contents, Contents::Stream(x) if *x == k))
            })
            .unwrap_or(0);
        *nodes_ref = nodes;
    }

    /// The streaming service shown in the tree, if one is.
    fn shown_stream(&self) -> Option<&Stream> {
        self.streams.iter().find(|s| s.source == self.tree)
    }

    fn shown_stream_mut(&mut self) -> Option<&mut Stream> {
        let tree = self.tree;
        self.streams.iter_mut().find(|s| s.source == tree)
    }

    /// Shows the shown service's sign-in, name and status in the sidebar.
    fn show_stream(&self) {
        let (Some(app), Some(st)) = (self.app.upgrade(), self.shown_stream()) else {
            return;
        };
        app.set_spotify_connected(st.state.connected);
        app.set_spotify_user(st.user.as_str().into());
        app.set_spotify_status(st.status.as_str().into());
        app.set_spotify_client_id(st.service.client_id.as_str().into());
    }

    /// The streaming playlist selected in the tree, if a service is shown.
    fn current_playlist(&self) -> Option<&str> {
        self.shown_stream()?;
        match &self.nodes.get(self.current_node)?.contents {
            Contents::Stream(k) => Some(k),
            _ => None,
        }
    }

    fn fetch_current_playlist(&mut self) {
        self.list_current_dir();
        let Some(key) = self.current_playlist().map(str::to_string) else {
            return;
        };
        let Some(stream) = self.shown_stream_mut() else {
            return;
        };
        // What copying the last playlist did is for that playlist only.
        stream.status.clear();
        let st = &mut stream.state;
        if !st.tracks.contains_key(&key) && !st.loading.contains(&key) {
            st.loading.insert(key.clone());
            st.errors.remove(&key);
            stream.service.send(Request::Tracks(key));
        }
        self.show_stream();
    }

    /// Follows the key notation picked by clicking a deck's key, and keeps it
    /// for next time. Called every frame.
    pub fn set_camelot(&mut self, camelot: bool) {
        if camelot != self.camelot {
            self.camelot = camelot;
            crate::settings::set_camelot_keys(camelot);
            self.refresh_rows();
            self.refresh_side();
            if let Some(app) = self.app.upgrade() {
                self.refresh_info(&app);
            }
        }
    }

    /// Takes in what the Spotify worker has done. Called every frame.
    /// Puts newly found album art into the rows showing it.
    pub fn poll_art(&mut self) {
        if !self.art.poll() {
            return;
        }
        // In place, so the side panel keeps its selection.
        if let Some(app) = self.app.upgrade() {
            let side = app.get_side_rows();
            for (i, id) in self.side_ids().into_iter().enumerate() {
                let Some(id) = id else { continue };
                let Some(mut row) = side.row_data(i) else {
                    continue;
                };
                if !row.has_cover {
                    if let Some(image) = self.art.get(&self.library[id]) {
                        row.cover = image;
                        row.has_cover = true;
                        side.set_row_data(i, row);
                    }
                }
            }
        }
        for (i, local) in self.rows.iter().enumerate() {
            let Some(id) = *local else { continue };
            let Some(mut row) = self.list.row_data(i) else {
                continue;
            };
            if row.has_cover {
                continue;
            }
            if let Some(image) = self.art.get(&self.library[id]) {
                row.cover = image;
                row.has_cover = true;
                self.list.set_row_data(i, row);
            }
        }
    }

    /// Gives the rows in view (and a screenful either side) their mini
    /// waveforms, and takes them from rows scrolled well away, so only a
    /// few small images are ever kept. Overviews not read yet are asked
    /// for; [`Browser::poll_waves`] fills them in as they arrive.
    fn fill_waves(&mut self) {
        let shown = self.columns.layout.has(columns::WAVE);
        let (first, count) = self.shown_rows;
        let count = count.max(1) as usize;
        let from = (first.max(0) as usize).saturating_sub(count);
        let to = (first.max(0) as usize + 2 * count).min(self.rows.len());
        let list = &self.list;
        self.wave_rows.retain(|&i| {
            let keep = shown && (from..to).contains(&i);
            if !keep {
                if let Some(mut row) = list.row_data(i) {
                    row.wave = Default::default();
                    row.has_wave = false;
                    list.set_row_data(i, row);
                }
            }
            keep
        });
        if !shown {
            return;
        }
        for i in from..to {
            let Some(id) = self.rows[i] else { continue };
            if self.wave_rows.contains(&i) {
                continue;
            }
            let Some(image) = self.waves.get(&self.library[id]) else {
                continue;
            };
            if let Some(mut row) = list.row_data(i) {
                row.wave = image;
                row.has_wave = true;
                list.set_row_data(i, row);
                self.wave_rows.push(i);
            }
        }
    }

    /// Follows the list's scrolling and puts newly read mini waveforms
    /// into the rows showing them. Called every frame; does nothing while
    /// the list is still and nothing has arrived.
    pub fn poll_waves(&mut self) {
        let arrived = self.waves.poll();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let ui = app.global::<ColumnUi>();
        let shown = (ui.get_first_shown(), ui.get_shown_count());
        if arrived || shown != self.shown_rows {
            self.shown_rows = shown;
            self.fill_waves();
        }
        if arrived && !app.global::<TrackInfo>().get_has_wave() {
            self.fill_info_wave(&app);
        }
    }

    /// Takes in what the streaming services' workers have done. Called
    /// every frame.
    pub fn poll_streams(&mut self) {
        for i in 0..self.streams.len() {
            let source = self.streams[i].source;
            for event in self.streams[i].service.poll() {
                let shown = self.tree == source;
                let st = &mut self.streams[i];
                match event {
                    Event::Status(s) => st.status = s,
                    Event::Connected { user, playlists } => {
                        st.state = StreamState {
                            connected: true,
                            playlists,
                            manual: self
                                .db
                                .as_ref()
                                .and_then(|db| db.stream_matches(st.id).ok())
                                .unwrap_or_default(),
                            ..Default::default()
                        };
                        st.user = user;
                        st.status.clear();
                        self.rebuild_stream_tree(source);
                        if shown {
                            self.refresh_tree();
                            self.refresh_rows();
                            self.fetch_current_playlist();
                        }
                    }
                    Event::Tracks { key, tracks } => {
                        st.state.loading.remove(&key);
                        st.state.tracks.insert(key.clone(), tracks);
                        if shown && self.current_playlist() == Some(key.as_str()) {
                            self.refresh_rows();
                        }
                    }
                    Event::TracksFailed { key, error } => {
                        st.state.loading.remove(&key);
                        st.state.errors.insert(key.clone(), error);
                        if shown && self.current_playlist() == Some(key.as_str()) {
                            self.refresh_rows();
                        }
                    }
                    Event::Disconnected => {
                        st.state = StreamState::default();
                        st.user.clear();
                        st.status.clear();
                        self.rebuild_stream_tree(source);
                        if shown {
                            self.refresh_tree();
                            self.refresh_rows();
                        }
                    }
                }
                if shown {
                    self.show_stream();
                }
            }
        }
    }

    /// Replaces the collection, e.g. when the library has been brought up
    /// to date with the music folder.
    pub fn set_library(&mut self, mut library: Vec<Arc<Track>>) {
        let paths: HashSet<PathBuf> = library
            .iter()
            .filter_map(|t| file_path(t).map(Path::to_path_buf))
            .collect();
        // Files shown in Files stay, unless the collection has them now.
        library.extend(
            self.library
                .iter()
                .filter(|t| is_browsed(t) && file_path(t).is_some_and(|p| !paths.contains(p)))
                .cloned(),
        );
        if let Ok(mut known) = self.files.reader.collection.write() {
            *known = paths;
        }
        self.path_index = library
            .iter()
            .enumerate()
            .filter_map(|(i, t)| Some((file_path(t)?.to_path_buf(), i)))
            .collect();
        self.index_of = library.iter().enumerate().map(|(i, t)| (t.id, i)).collect();
        self.indexed = library.iter().map(|t| Indexed::new(t)).collect();
        self.library = library;
        self.side = [Vec::new(), Vec::new()];
        self.folders = crate::settings::stage_folders();
        self.mixed_sets = crate::settings::mixed_sets_folder();
        self.read_pending_moves();
        self.queue_triage_analysis();
        self.refresh_health();
        self.refresh_dupes();
        let tree = self.collection_tree();
        if self.tree == COLLECTION {
            self.nodes = tree;
            self.current_node = self.first_node(self.history_tab);
            self.other_tab_node = self.first_node(!self.history_tab);
        } else {
            let sort = self.parked.get(&COLLECTION).map_or((1, true), |p| p.sort);
            self.parked.insert(
                COLLECTION,
                Parked {
                    nodes: tree,
                    current: 0,
                    sort,
                },
            );
        }
        if matches!(self.tree, ALBUMS | MIXED_SETS) {
            self.rebuild_folder_tree(self.tree);
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
        self.refresh_side();
    }

    /// Takes the first track off the Automix list, as Mixxx's Auto DJ does
    /// when it loads one.
    pub fn take_automix_next(&mut self) -> Option<Arc<Track>> {
        if self.side[0].is_empty() {
            return None;
        }
        let id = self.side[0].remove(0);
        self.refresh_side();
        self.library.get(id).cloned()
    }

    pub fn automix_len(&self) -> usize {
        self.side[0].len()
    }

    /// Whether the collection is the saved library rather than the samples.
    fn saved(&self) -> bool {
        self.db.is_some()
            && self
                .library
                .first()
                .is_some_and(|t| opendj_library::row_id(t.id).is_some())
    }

    /// The collection's tree: everything, the DJ's lists and folders,
    /// groupings and tags, smart lists, and (on the history
    /// tab) the history sessions.
    fn collection_tree(&self) -> Vec<Node> {
        if !self.saved() {
            let mut tree = sample_tree(self.library.len());
            let imported = imported_tree(&self.library);
            if !imported.is_empty() {
                let smart = tree
                    .iter()
                    .position(|n| n.name == "Smart Lists")
                    .unwrap_or(1);
                tree.splice(1..smart, imported);
            }
            return tree;
        }
        let mut tree = if self.folders.is_set() {
            let mut t = staging::stage_nodes(&self.folders);
            t.push(Node::new("All Tracks", 0, KIND_LIST, Contents::All));
            t
        } else {
            vec![Node::new("Collection", 0, 0, Contents::All)]
        };
        tree.extend(health::health_nodes());
        let playlists = self
            .db
            .as_ref()
            .and_then(|db| db.playlists().ok())
            .unwrap_or_default();
        let mut depth: HashMap<i64, i32> = HashMap::new();
        let mut sessions = Vec::new();
        let mut smart = Vec::new();
        for p in &playlists {
            let d = p.parent.and_then(|id| depth.get(&id)).map_or(0, |d| d + 1);
            depth.insert(p.id, d);
            match p.kind {
                // Smart lists at the top level go under "Smart Lists".
                PlaylistKind::Smart if p.parent.is_none() => smart.push(p),
                PlaylistKind::Smart => tree.push(smart_node(p, d)),
                PlaylistKind::History if p.parent.is_some() => sessions.push(p),
                PlaylistKind::History => {}
                PlaylistKind::Set => {
                    let mut n = Node::new(p.name.clone(), d, KIND_SET, Contents::Folder);
                    n.playlist = Some(p.id);
                    tree.push(n);
                }
                PlaylistKind::Folder | PlaylistKind::List => {
                    let folder = p.kind == PlaylistKind::Folder;
                    let mut n = Node::new(
                        p.name.clone(),
                        d,
                        if folder { KIND_FOLDER } else { KIND_LIST },
                        if folder {
                            Contents::Folder
                        } else {
                            Contents::Playlist(p.id)
                        },
                    );
                    n.playlist = Some(p.id);
                    tree.push(n);
                }
            }
        }
        tree.extend(imported_tree(&self.library));
        if !smart.is_empty() {
            tree.push(Node::new("Smart Lists", 0, KIND_FOLDER, Contents::Folder));
            tree.extend(smart.into_iter().map(|p| smart_node(p, 1)));
        }
        // Newest session first, sessions brought in from other apps among
        // the DJ's own.
        sessions.sort_by_key(|p| (p.created, p.id));
        for p in sessions.iter().rev() {
            let mut n = Node::new(p.name.clone(), 0, KIND_HISTORY, Contents::Playlist(p.id));
            n.history = true;
            tree.push(n);
        }
        tree
    }

    /// The first node shown on the library tab (or the history tab).
    fn first_node(&self, history: bool) -> usize {
        // With stages, the library tab opens on the collection, not the inbox.
        let collection = (!history)
            .then(|| {
                self.nodes
                    .iter()
                    .position(|n| matches!(n.contents, Contents::Stage(Stage::Collection, None)))
            })
            .flatten();
        collection
            .or_else(|| self.nodes.iter().position(|n| n.history == history))
            .unwrap_or(0)
    }

    /// Rebuilds the tree from the database, keeping the selection where it can.
    fn reload_tree(&mut self, select: Option<i64>) {
        if self.tree != COLLECTION {
            return;
        }
        self.folders = crate::settings::stage_folders();
        self.mixed_sets = crate::settings::mixed_sets_folder();
        self.refresh_crates();
        let selected = select.or_else(|| self.nodes.get(self.current_node)?.playlist);
        let name = self.nodes.get(self.current_node).map(|n| n.name.clone());
        let expanded: HashMap<(String, i32), bool> = self
            .nodes
            .iter()
            .map(|n| ((n.name.clone(), n.depth), n.expanded))
            .collect();
        self.nodes = self.collection_tree();
        for n in &mut self.nodes {
            if let Some(&e) = expanded.get(&(n.name.clone(), n.depth)) {
                n.expanded = e;
            }
        }
        self.current_node = selected
            .and_then(|id| self.nodes.iter().position(|n| n.playlist == Some(id)))
            .or_else(|| {
                self.nodes
                    .iter()
                    .position(|n| Some(&n.name) == name.as_ref() && n.history == self.history_tab)
            })
            .unwrap_or_else(|| self.first_node(self.history_tab));
        // Open the folders above the selection.
        let mut depth = self.nodes.get(self.current_node).map_or(0, |n| n.depth);
        for i in (0..self.current_node).rev() {
            if self.nodes[i].depth < depth {
                self.nodes[i].expanded = true;
                depth = self.nodes[i].depth;
            }
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Makes a new list or folder, inside the selected folder if it is one
    /// of the DJ's, and selects it. Returns its row in the tree.
    fn create_list(&mut self, folder: bool) -> i32 {
        let Some(db) = &self.db else { return -1 };
        if !self.saved() || self.tree != COLLECTION || self.history_tab {
            return -1;
        }
        let parent = self
            .nodes
            .get(self.current_node)
            .filter(|n| n.kind == KIND_FOLDER || n.kind == KIND_SET)
            .and_then(|n| n.playlist);
        let (name, kind) = if folder {
            ("New folder", PlaylistKind::Folder)
        } else {
            ("New list", PlaylistKind::List)
        };
        match db.create_playlist(name, parent, kind) {
            Ok(id) => {
                self.reload_tree(Some(id));
                self.visible_nodes
                    .iter()
                    .position(|&n| n == self.current_node)
                    .map_or(-1, |p| p as i32)
            }
            Err(e) => {
                eprintln!("can't make a list: {e}");
                -1
            }
        }
    }

    /// The DJ's list or folder shown at `row` of the tree.
    fn playlist_at(&self, row: usize) -> Option<i64> {
        self.nodes.get(*self.visible_nodes.get(row)?)?.playlist
    }

    fn rename_node(&mut self, row: usize, name: &str) {
        let (Some(id), Some(db)) = (self.playlist_at(row), &self.db) else {
            return;
        };
        if let Err(e) = db.rename_playlist(id, name.trim()) {
            eprintln!("can't rename the list: {e}");
        }
        self.reload_tree(None);
    }

    fn delete_node(&mut self, row: usize) {
        let (Some(id), Some(db)) = (self.playlist_at(row), &self.db) else {
            return;
        };
        if let Err(e) = db.delete_playlist(id) {
            eprintln!("can't delete the list: {e}");
        }
        let deleted_selected =
            self.nodes.get(self.current_node).and_then(|n| n.playlist) == Some(id);
        if deleted_selected {
            self.current_node = 0;
        }
        self.reload_tree(None);
    }

    /// Adds the tracks of rows `first..=last`, in order, to a playlist.
    fn add_rows_to_playlist(&mut self, first: usize, last: usize, playlist: i64) {
        let indices: Vec<usize> = (first..=last.min(self.rows.len().saturating_sub(1)))
            .filter_map(|row| self.rows.get(row).copied().flatten())
            .collect();
        self.add_to_playlist(&indices, playlist);
    }

    /// Adds library tracks (indices into `library`), in order, to a playlist.
    fn add_to_playlist(&mut self, indices: &[usize], playlist: i64) {
        if indices.is_empty() {
            return;
        }
        self.adopt(indices);
        let ids: Vec<_> = indices.iter().map(|&i| self.library[i].id).collect();
        if let Some(db) = &mut self.db {
            if let Err(e) = db.add_to_playlist(playlist, &ids) {
                eprintln!("can't add to the list: {e}");
            }
        }
        self.refresh_crates();
        if self.nodes.get(self.current_node).and_then(|n| n.playlist) == Some(playlist) {
            self.refresh_rows();
        }
    }

    /// Takes the track at `row` out of the list being shown.
    fn remove_row_from_list(&mut self, row: usize) {
        let Some(Contents::Playlist(list)) = self
            .nodes
            .get(self.current_node)
            .map(|n| n.contents.clone())
        else {
            return;
        };
        let Some(&Some(index)) = self.rows.get(row) else {
            return;
        };
        // Rows may be sorted or filtered: find which entry of the list this
        // row is (the nth time the track appears).
        let id = self.library[index].id;
        let nth = self.rows[..row]
            .iter()
            .filter(|r| r.is_some_and(|i| self.library[i].id == id))
            .count();
        let Some(db) = &mut self.db else { return };
        let entries = db.playlist_tracks(list).unwrap_or_default();
        let Some(position) = entries
            .iter()
            .enumerate()
            .filter(|(_, t)| **t == id)
            .nth(nth)
            .map(|(p, _)| p)
        else {
            return;
        };
        if let Err(e) = db.remove_from_playlist(list, position) {
            eprintln!("can't remove from the list: {e}");
        }
        self.refresh_crates();
        self.refresh_rows();
    }

    /// Changes the library's copy of the track at `row`, and saves it.
    fn change_track(&mut self, row: usize, change: impl FnOnce(&mut Track, Option<&Library>)) {
        let Some(&Some(index)) = self.rows.get(row) else {
            return;
        };
        self.adopt(&[index]);
        let mut t = (*self.library[index]).clone();
        change(&mut t, self.db.as_ref());
        self.library[index] = Arc::new(t);
        self.refresh_rows();
    }

    fn set_rating(&mut self, row: usize, rating: u8) {
        self.change_track(row, |t, db| {
            t.rating = rating;
            if let Some(Err(e)) = db.map(|db| db.set_rating(t.id, rating)) {
                eprintln!("can't save the rating: {e}");
            }
        });
    }

    /// Rates the rows picked in the track list, for a keyboard shortcut.
    pub fn rate_rows(&mut self, first: usize, last: usize, rating: u8) {
        for row in first..=last.min(self.rows.len().saturating_sub(1)) {
            self.set_rating(row, rating);
        }
    }

    /// Rates the track loaded on `deck`; false when there's none.
    pub fn rate_deck(&mut self, deck: usize, rating: u8) -> bool {
        let Some(index) = self.deck_track(deck) else {
            return false;
        };
        self.adopt(&[index]);
        let mut t = (*self.library[index]).clone();
        t.rating = rating;
        if let Some(Err(e)) = self.db.as_ref().map(|db| db.set_rating(t.id, rating)) {
            eprintln!("can't save the rating: {e}");
        }
        self.library[index] = Arc::new(t);
        self.refresh_rows();
        true
    }

    /// Switches the sidebar between the library tree and the history.
    fn show_history(&mut self, on: bool) {
        if on == self.history_tab {
            return;
        }
        self.history_tab = on;
        std::mem::swap(&mut self.current_node, &mut self.other_tab_node);
        if self
            .nodes
            .get(self.current_node)
            .is_none_or(|n| n.history != on)
        {
            self.current_node = self.first_node(on);
        }
        if let Some(app) = self.app.upgrade() {
            app.set_current_track(-1);
        }
        self.refresh_tree();
        self.refresh_rows();
    }

    /// Saves what changed on the decks: hot cues the DJ set or cleared, beat
    /// grids found by analysis or edited, and tracks played long enough to
    /// count. `dt` is the time since the last call. Called every frame.
    pub fn keep(&mut self, loaded: &[Option<Arc<Track>>], dt: f64) {
        if !self.saved() {
            return;
        }
        let mut changed = false;
        for (d, t) in loaded.iter().enumerate().take(NUM_DECKS) {
            let Some(t) = t.as_ref() else {
                self.watch[d] = CueWatch::default();
                continue;
            };
            if opendj_library::row_id(t.id).is_none() {
                continue;
            }
            let w = &mut self.watch[d];
            if w.track.map(|(id, _)| id) != Some(t.id) {
                *w = CueWatch {
                    track: Some((t.id, std::time::Instant::now())),
                    saved: t.info.hotcues.clone(),
                    ..Default::default()
                };
            }
            // The engine sets the new track's cues as it loads; wait for that.
            let settled = w
                .track
                .is_some_and(|(_, at)| at.elapsed() > std::time::Duration::from_millis(500));
            if settled {
                let pads = self.hotcues[d].len();
                let mut now: Vec<Hotcue> = self.hotcues[d]
                    .iter()
                    .enumerate()
                    .filter(|(_, (enabled, _, _))| enabled.get_bool())
                    .map(|(n, (_, pos, end))| {
                        let seconds = pos.get();
                        let end = Some(end.get()).filter(|&e| e > seconds);
                        // Keep a cue's name and colour while it stays where
                        // it was.
                        let kept = w
                            .saved
                            .iter()
                            .find(|c| c.number == n && (c.seconds - seconds).abs() < 1e-6);
                        Hotcue {
                            number: n,
                            seconds,
                            name: kept.map(|c| c.name.clone()).unwrap_or_default(),
                            end,
                            color: kept.and_then(|c| c.color),
                        }
                    })
                    .collect();
                // Cues the deck has no pad for (memory cues, and hot cues
                // past the last pad) stay as they were.
                now.extend(w.saved.iter().filter(|c| c.number >= pads).cloned());
                if now != w.saved {
                    w.saved = now.clone();
                    if let Some(db) = &mut self.db {
                        if let Err(e) = db.save_cues(t.id, &now) {
                            eprintln!("can't save the cues: {e}");
                        }
                    }
                    if let Some(&i) = self.index_of.get(&t.id) {
                        let mut e = (*self.library[i]).clone();
                        e.info.hotcues = now;
                        self.library[i] = Arc::new(e);
                    }
                }
            }
            let w = &mut self.watch[d];
            if self.play[d].get_bool() && !w.recorded {
                w.played += dt;
                if w.played >= PLAYED_AFTER {
                    w.recorded = true;
                    self.record_play(t.id);
                    changed = true;
                }
            }
        }
        if changed && self.history_tab {
            self.refresh_rows();
        }
    }

    /// Adds a played track to this session's history.
    fn record_play(&mut self, id: usize) {
        let Some(db) = &mut self.db else { return };
        if self.session.is_none() {
            let name = chrono::Local::now()
                .format("%a %e %b %Y, %H:%M")
                .to_string();
            match db.start_session(&name) {
                Ok(s) => self.session = Some(s),
                Err(e) => eprintln!("can't start the history: {e}"),
            }
            let selected = self.nodes.get(self.current_node).and_then(|n| n.playlist);
            self.reload_tree(selected);
        }
        let Some(db) = &mut self.db else { return };
        if let Err(e) = db.record_play(self.session, id) {
            eprintln!("can't record the play: {e}");
        }
        if let Some(&i) = self.index_of.get(&id) {
            let mut e = (*self.library[i]).clone();
            e.info.play_count += 1;
            self.library[i] = Arc::new(e);
        }
        let showing_session = self
            .nodes
            .get(self.current_node)
            .and_then(|n| match n.contents {
                Contents::Playlist(p) => Some(p),
                _ => None,
            })
            == self.session;
        if showing_session {
            self.refresh_rows();
        }
    }

    fn side_tab(&self) -> usize {
        self.app
            .upgrade()
            .map(|a| a.get_side_tab())
            .unwrap_or(0)
            .clamp(0, info::INFO_TAB as i32) as usize
    }

    /// Loads onto a deck that is not playing, preferring deck A.
    fn load_dropped(&mut self, deck: usize, data: &DataTransfer) {
        if deck >= NUM_DECKS {
            return;
        }
        // A deck's own cover dropped back on it changes nothing.
        if dragged(data) == Some(Dragged::Deck(deck)) {
            return;
        }
        if let Some(id) = self.dropped_track(data) {
            self.engine.load_track(deck, self.library[id].clone());
        }
    }

    /// The playable tracks of list rows `first..=last`, in order, taken into
    /// the library.
    fn playable_rows(&mut self, first: usize, last: usize) -> Vec<usize> {
        let ids: Vec<usize> = (first..=last.min(self.rows.len().saturating_sub(1)))
            .filter_map(|row| self.rows.get(row).copied().flatten())
            .filter(|&id| self.playable(id))
            .collect();
        self.adopt(&ids);
        ids
    }

    /// The library tracks a drop carries: the rows of the list it names, or
    /// an audio file from outside.
    fn dropped_tracks(&mut self, data: &DataTransfer) -> Vec<usize> {
        match dragged(data) {
            Some(Dragged::Rows(first, last)) => self.playable_rows(first, last),
            Some(Dragged::Side(tab, first, last)) => {
                let ids: Vec<usize> = self
                    .side_ids_of(tab)
                    .into_iter()
                    .skip(first)
                    .take(last + 1 - first)
                    .flatten()
                    .collect();
                self.adopt(&ids);
                ids
            }
            Some(Dragged::Deck(deck)) => self.deck_track(deck).into_iter().collect(),
            None => self.dropped_track(data).into_iter().collect(),
        }
    }

    /// The library track loaded on `deck`.
    fn deck_track(&self, deck: usize) -> Option<usize> {
        self.index_of
            .get(&self.loaded.get(deck).copied().flatten()?)
            .copied()
    }

    /// The library track a drop carries: the first of the rows it names, a
    /// deck's track, or an audio file from outside, which joins the library.
    fn dropped_track(&mut self, data: &DataTransfer) -> Option<usize> {
        if let Some(Dragged::Rows(row, _)) = dragged(data) {
            match self.rows.get(row) {
                Some(&Some(id)) if self.playable(id) => {
                    self.adopt(&[id]);
                    Some(id)
                }
                _ => None,
            }
        } else if dragged(data).is_some() {
            self.dropped_tracks(data).first().copied()
        } else if let Some(path) = dropped_file(data) {
            // A file from outside the collection joins the library so the
            // deck markers and reloading work as for any other track.
            if let Some(&i) = self.path_index.get(&path) {
                self.adopt(&[i]);
                return Some(i);
            }
            let index = self.library.len();
            let mut track = read_track(index, &path);
            if self.saved() {
                if let Some(db) = &mut self.db {
                    match db.add_file(&track) {
                        Ok(t) => track = t,
                        Err(e) => eprintln!("can't add {} to the library: {e}", path.display()),
                    }
                }
            }
            Some(if let Some(&i) = self.index_of.get(&track.id) {
                i
            } else {
                self.push_track(track)
            })
        } else {
            None
        }
    }

    /// Acts on library controls set by controller mappings: browsing,
    /// switching between folders and tracks, loading. Called every frame.
    pub fn poll_controllers(&mut self, app: &AppWindow) {
        if take(&self.lib.show_linked) > 0.0 {
            app.set_side_panel_visible(true);
            app.set_side_tab(LINKED_TAB as i32);
        }
        let mut steps: f64 = self.lib.steps.iter().map(|(c, sign)| take(c) * sign).sum();
        // Clicking a folder or a track moves the focus too.
        self.folders_focused = app.get_folders_focused();
        if self.lib.focus.iter().map(take).sum::<f64>() != 0.0 {
            self.folders_focused = !self.folders_focused;
            app.set_folders_focused(self.folders_focused);
        }
        let folder_steps: f64 = self.lib.folder_steps.iter().map(|(c, s)| take(c) * s).sum();
        let mut folder_steps = folder_steps.round() as i64;
        if self.folders_focused {
            folder_steps += steps.round() as i64;
            steps = 0.0;
        }
        if folder_steps != 0 && !self.visible_nodes.is_empty() {
            let at = self
                .visible_nodes
                .iter()
                .position(|&n| n == self.current_node)
                .unwrap_or(0) as i64;
            let to = (at + folder_steps).clamp(0, self.visible_nodes.len() as i64 - 1) as usize;
            self.current_node = self.visible_nodes[to];
            app.set_current_track(-1);
            self.refresh_tree();
            self.refresh_rows();
            self.fetch_current_playlist();
        }
        let steps = steps.round() as i64;
        if steps != 0 && !self.rows.is_empty() {
            let current = app.get_current_track() as i64;
            let to = if current < 0 {
                if steps > 0 {
                    steps - 1
                } else {
                    self.rows.len() as i64 + steps
                }
            } else {
                current + steps
            };
            app.set_current_track(to.clamp(0, self.rows.len() as i64 - 1) as i32);
        }

        let selected = usize::try_from(app.get_current_track())
            .ok()
            .and_then(|r| self.rows.get(r).copied().flatten());
        let widget = if self.folders_focused { 2.0 } else { 3.0 };
        if self.lib.focused_widget.get() != widget {
            self.lib.focused_widget.set(widget);
        }
        let go = take(&self.lib.go_to_item) > 0.0;
        if take(&self.lib.toggle_folder) > 0.0 {
            if let Some(n) = self.nodes.get_mut(self.current_node) {
                n.expanded = !n.expanded;
                self.refresh_tree();
            }
        }
        if go && self.folders_focused {
            // Like pressing the browse knob on a folder: open or close it.
            if let Some(n) = self.nodes.get_mut(self.current_node) {
                n.expanded = !n.expanded;
                self.refresh_tree();
            }
        } else if (go || take(&self.lib.load_first_stopped) > 0.0) && selected.is_some() {
            self.adopt(&[selected.unwrap()]);
            self.load_to_free_deck(selected.unwrap());
        }
        for d in 0..NUM_DECKS {
            let load = take(&self.lib.load[d]) > 0.0;
            let and_play = take(&self.lib.load_and_play[d]) > 0.0;
            if let (Some(id), true) = (selected.filter(|&i| self.playable(i)), load || and_play) {
                self.adopt(&[id]);
                if self.engine.load_track(d, self.library[id].clone()) {
                    self.play_when_loaded[d] = and_play.then_some(id);
                }
            }
        }
        let pressed: Vec<bool> = self
            .lib
            .add_to_automix
            .iter()
            .map(|c| take(c) > 0.0)
            .collect();
        for (i, pressed) in pressed.into_iter().enumerate() {
            if let (true, Some(id)) = (pressed, selected) {
                self.adopt(&[id]);
                if i == 0 {
                    self.side[0].push(id);
                } else {
                    self.side[0].insert(0, id);
                }
                self.refresh_side();
            }
        }
    }

    /// Whether a drag carries something a sampler pad takes: a track or an
    /// audio file.
    pub fn accepts_sample(data: &DataTransfer) -> bool {
        dragged(data).is_some() || dropped_file(data).is_some()
    }

    /// The audio file a drop on a sampler pad carries, with a name for the
    /// pad: a track's title, or a file's name. Tracks stay where they are in
    /// the library and files from outside don't join it.
    pub fn dropped_sample(&self, data: &DataTransfer) -> Option<(PathBuf, String)> {
        let id = match dragged(data) {
            Some(Dragged::Rows(row, _)) => self.rows.get(row).copied().flatten(),
            Some(Dragged::Side(tab, first, _)) => {
                self.side_ids_of(tab).get(first).copied().flatten()
            }
            Some(Dragged::Deck(deck)) => self.deck_track(deck),
            None => {
                let path = dropped_file(data)?;
                let name = file_stem(&path);
                return Some((path, name));
            }
        }?;
        self.sample_of(id)
    }

    /// The track selected in the list, as a sampler sample.
    pub fn selected_sample(&self, app: &AppWindow) -> Option<(PathBuf, String)> {
        let id = usize::try_from(app.get_current_track())
            .ok()
            .and_then(|r| self.rows.get(r).copied().flatten())?;
        self.sample_of(id)
    }

    fn sample_of(&self, id: usize) -> Option<(PathBuf, String)> {
        let t = self.library.get(id).filter(|_| self.playable(id))?;
        let opendj_engine::Source::File(path) = &t.source else {
            return None;
        };
        let name = if t.title.is_empty() {
            file_stem(path)
        } else {
            t.title.clone()
        };
        Some((path.clone(), name))
    }

    /// Whether the track at `index` of the library can load: its file is there.
    fn playable(&self, index: usize) -> bool {
        self.library.get(index).is_some_and(|t| !t.info.missing)
    }

    fn load_to_free_deck(&self, id: usize) {
        if !self.playable(id) {
            return;
        }
        if let Some(deck) = (0..self.decks_shown).find(|&d| !self.play[d].get_bool()) {
            self.engine.load_track(deck, self.library[id].clone());
        }
    }

    pub fn set_decks_shown(&mut self, decks: usize) {
        self.decks_shown = decks.clamp(2, NUM_DECKS);
    }

    /// Called every frame with what is on the decks; refreshes the A/B markers
    /// and the side panel when they change.
    pub fn set_loaded(&mut self, app: &AppWindow, loaded: [Option<usize>; NUM_DECKS]) {
        let tab = app.get_side_tab().clamp(0, info::INFO_TAB as i32) as usize;
        self.poll_info(app, tab);
        // The Linked tab follows the track selected in the list.
        let selected = usize::try_from(app.get_current_track())
            .ok()
            .and_then(|r| self.rows.get(r).copied().flatten());
        // Not while a track is being dragged: picking it up selects it, and
        // it is about to be linked to the track shown.
        let dragging = app.global::<TrackDrag>().get_active();
        let follow = tab == LINKED_TAB && !dragging && selected != self.linked_to;
        if follow {
            self.linked_to = selected;
        }
        if tab != self.shown_side_tab || follow {
            self.shown_side_tab = tab;
            self.refresh_side();
        }
        if loaded != self.loaded {
            self.loaded = loaded;
            self.update_reference();
            self.refresh_rows();
            self.refresh_side();
        } else if self.update_reference() {
            // Keys light up when they mix with the track playing now.
            self.refresh_rows();
            self.refresh_side();
            if self.follows_deck() {
                self.refresh_filter_ui();
            }
        }
        // Grouping or a tag field renamed in Settings.
        let names = query::field_names();
        if names != self.filter.names {
            self.field_names_changed(names);
        }
        for ((pending, now), play) in self.play_when_loaded.iter_mut().zip(loaded).zip(&self.play) {
            if pending.is_some() && *pending == now {
                *pending = None;
                play.set_bool(true);
            }
        }
    }

    /// Copies the tempo, key and beat grid found by analysing a loaded
    /// track, or a grid the DJ edited, into the library and saves them, as
    /// Mixxx stores analysis results. The list shows them too.
    pub fn learn_analysis(&mut self, loaded: &[Option<Arc<Track>>]) {
        let mut changed = false;
        let saved = self.saved();
        for t in loaded.iter().flatten() {
            // Its mini waveform, filling in as the deck analyses it, in its
            // row and in Info.
            if self.waves.learn(t) {
                let i = self.index_of.get(&t.id).copied();
                if let Some(row) = self.rows.iter().position(|r| i.is_some() && *r == i) {
                    self.wave_rows.retain(|&w| w != row);
                    self.fill_waves();
                }
                if let Some(app) = self.app.upgrade() {
                    self.fill_info_wave(&app);
                }
            }
            if t.bpm <= 0.0 {
                continue;
            }
            let Some(&i) = self.index_of.get(&t.id) else {
                continue;
            };
            let entry = &self.library[i];
            // An analysed grid arrives with the analysis; a fixed grid that
            // differs from the library's was edited on the deck.
            let analysed = !t.info.fixed_grid && !t.grid.is_empty() && t.grid != entry.grid;
            let edited = t.info.fixed_grid && t.grid != entry.grid && !t.grid.is_empty();
            let key_changed = entry.key != t.key || entry.info.key_detected != t.info.key_detected;
            let lock_changed = entry.info.grid_locked != t.info.grid_locked;
            if !(analysed || edited || lock_changed || entry.bpm != t.bpm || key_changed) {
                continue;
            }
            // An analysed grid replacing one the DJ edited or an import
            // brought: the DJ went back to the analysed grid.
            let reset = analysed && !matches!(entry.info.grid_from.as_str(), "" | "analysis");
            let mut e = (**entry).clone();
            e.bpm = t.bpm;
            e.key = t.key.clone();
            e.info.key_detected = t.info.key_detected;
            e.info.grid_locked = t.info.grid_locked;
            if analysed || edited {
                e.set_grid(t.grid.clone());
                e.info.fixed_grid = true;
                e.info.grid_analysed = analysed;
                e.info.grid_from = if analysed { "analysis" } else { "user" }.into();
                e.info.grid_unsure = t.info.grid_unsure;
            }
            if let (true, Some(db)) = (saved, &self.db) {
                let mut result = if lock_changed {
                    db.set_grid_locked(t.id, t.info.grid_locked)
                } else {
                    Ok(())
                };
                if reset {
                    result = result.and_then(|()| db.clear_grid(t.id));
                }
                result = result.and_then(|()| {
                    if edited {
                        db.save_grid(t.id, &t.grid, t.bpm)
                    } else {
                        db.save_analysis(t.id, t)
                    }
                });
                if let Err(err) = result {
                    eprintln!("can't save the analysis: {err}");
                }
            }
            self.library[i] = Arc::new(e);
            changed = true;
        }
        if changed {
            self.refresh_rows();
        }
    }

    fn refresh_tree(&mut self) {
        self.visible_nodes.clear();
        let mut hidden_below: Option<i32> = None;
        for (i, n) in self.nodes.iter().enumerate() {
            if n.history != self.history_tab {
                continue;
            }
            if let Some(d) = hidden_below {
                if n.depth > d {
                    continue;
                }
                hidden_below = None;
            }
            self.visible_nodes.push(i);
            if !n.expanded && self.has_children(i) {
                hidden_below = Some(n.depth);
            }
        }
        let folders: Vec<Folder> = self
            .visible_nodes
            .iter()
            .map(|&i| {
                let n = &self.nodes[i];
                Folder {
                    name: n.name.clone().into(),
                    depth: n.depth,
                    children: if n.lazy {
                        1
                    } else if !self.has_children(i) {
                        0
                    } else if n.expanded {
                        2
                    } else {
                        1
                    },
                    kind: n.kind,
                    editable: n.playlist.is_some() && !n.history,
                    analysable: !n.history && Self::analysable(&n.contents),
                    stage: matches!(n.contents, Contents::Stage(..)),
                    count: match &n.contents {
                        Contents::Stage(stage, dir) => self
                            .library
                            .iter()
                            .filter(|t| !t.info.missing && self.at_stage(t, *stage, dir.as_deref()))
                            .count() as i32,
                        Contents::Health(Some(check)) => self.health_count(*check) as i32,
                        Contents::Dupes(case) => self.dupe_tracks(Some(*case)).len() as i32,
                        _ => -1,
                    },
                }
            })
            .collect();
        // The track menu offers the DJ's lists, named with their folders.
        let mut lists = Vec::new();
        let mut names = Vec::new();
        let mut path: Vec<&str> = Vec::new();
        for n in self
            .nodes
            .iter()
            .filter(|n| n.playlist.is_some() && !n.history)
        {
            path.truncate(n.depth as usize);
            if n.kind == KIND_LIST {
                let mut name = path.join(" › ");
                if !name.is_empty() {
                    name.push_str(" › ");
                }
                name.push_str(&n.name);
                names.push(SharedString::from(name));
                lists.push(n.playlist.unwrap());
            }
            path.push(&n.name);
        }
        self.menu_lists = lists;
        let in_list = self
            .nodes
            .get(self.current_node)
            .is_some_and(|n| n.kind == KIND_LIST && n.playlist.is_some());
        if let Some(app) = self.app.upgrade() {
            let edit = app.global::<LibraryEdit>();
            edit.set_lists(ModelRc::new(VecModel::from(names)));
            edit.set_in_list(in_list);
            edit.set_can_save_search(self.saved() && self.tree == COLLECTION && !self.history_tab);
            let rule = match self.nodes.get(self.current_node).map(|n| &n.contents) {
                Some(Contents::Query(q)) if self.tree == COLLECTION => q.clone(),
                _ => String::new(),
            };
            edit.set_rule(rule.into());
            self.refresh_stage_ui(&edit);
            self.refresh_set_menu(&edit);
            self.refresh_health_ui();
            app.set_folders(ModelRc::new(VecModel::from(folders)));
            let current = self
                .visible_nodes
                .iter()
                .position(|&n| n == self.current_node)
                .map(|p| p as i32)
                .unwrap_or(-1);
            app.set_current_folder(current);
            let name = match self.nodes.get(self.current_node) {
                Some(n) => n.name.clone(),
                None if self.tree == FILES => "Files".into(),
                None if self.tree == ALBUMS => "Albums".into(),
                None if self.tree == MIXED_SETS => "Mixed Sets".into(),
                None if self.shown_stream().is_some() => {
                    self.shown_stream().map_or("", |st| st.name).into()
                }
                None if self.tree == STORES => "Bandcamp".into(),
                None => String::new(),
            };
            app.set_folder_name(name.into());
        }
    }

    fn has_children(&self, i: usize) -> bool {
        self.nodes.get(i + 1).is_some_and(|next| {
            next.depth > self.nodes[i].depth && next.history == self.nodes[i].history
        })
    }

    /// What searches compare tracks against: now, the track playing, and
    /// the duplicates.
    fn context(&self) -> Context<'_> {
        Context {
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs() as i64),
            deck: self.deck_ref,
            deck_track: self.deck_track,
            duplicates: Some(&self.duplicates),
            health: Some(&self.health),
            crates: Some(&self.crates),
        }
    }

    /// The tracks (indices into `library`) that `q` finds, among `within`
    /// or the whole library. Missing files are left out unless asked for.
    fn search(&self, q: &Query, within: Option<&[usize]>) -> Vec<usize> {
        let cx = self.context();
        let missing_ok = q.wants_missing();
        let test = |&t: &usize| {
            let track = &self.library[t];
            (missing_ok || !track.info.missing) && q.matches(track, &self.indexed[t], &cx)
        };
        match within {
            Some(rows) => rows.iter().copied().filter(test).collect(),
            None => (0..self.library.len())
                .filter(|&t| self.in_collection(&self.library[t]))
                .filter(test)
                .collect(),
        }
    }

    /// The tempo and key of the track playing now (the first playing deck,
    /// else the first loaded one), for "mixes with deck".
    fn playing_reference(&self) -> Option<(usize, f64, Option<Key>)> {
        let decks = (0..NUM_DECKS).filter(|&d| self.play[d].get_bool());
        let deck = decks
            .chain(0..NUM_DECKS)
            .find(|&d| self.loaded[d].is_some())?;
        let t = &self.library[*self.index_of.get(&self.loaded[deck]?)?];
        // The key it plays in, shifted or sped up, once the deck has it.
        let key = Key::from_mixxx_number(self.deck_key[deck].get()).or_else(|| Key::parse(&t.key));
        Some((t.id, t.bpm, key))
    }

    /// The rule keys mix by, and how far the marks reach: the shift tracks
    /// get on load, and at least one semitone.
    fn current_key_rules(&self) -> (key::Mixing, i32) {
        (
            key::mixing(),
            (self.key_match_range.get().round() as i32).clamp(1, 2),
        )
    }

    /// Notes the track playing now; returns whether it changed.
    fn update_reference(&mut self) -> bool {
        let r = self.playing_reference();
        let track = r.map(|(id, ..)| id);
        let reference = r.map(|(_, bpm, key)| (bpm, key));
        let rules = self.current_key_rules();
        let changed =
            track != self.deck_track || reference != self.deck_ref || rules != self.key_rules;
        self.deck_track = track;
        self.deck_ref = reference;
        self.key_rules = rules;
        changed
    }

    /// Whether a key mixes harmonically with the track playing now's.
    fn mixes_with_deck(&self, key: &str) -> bool {
        self.deck_ref
            .and_then(|(_, k)| k)
            .zip(Key::parse(key))
            .is_some_and(|(a, b)| a.compatible(b))
    }

    /// The semitones a track in `key` would be shifted to mix with the
    /// track playing now, within the marks' reach; 0 when it mixes as it is
    /// or nothing in reach does.
    fn key_shift_to_deck(&self, key: &str) -> i32 {
        let (mixing, reach) = self.key_rules;
        match self.deck_ref.and_then(|(_, k)| k).zip(Key::parse(key)) {
            Some((theirs, mine)) if !mine.mixes(theirs, mixing) => {
                mine.shift_to_mix(theirs, reach, mixing).unwrap_or(0)
            }
            _ => 0,
        }
    }

    /// Whether the list shown depends on the track playing now.
    fn follows_deck(&self) -> bool {
        self.filter.filters.follows_deck()
            || Query::parse(&self.search).uses_deck()
            || self.nodes.get(self.current_node).is_some_and(
                |n| matches!(&n.contents, Contents::Query(q) if Query::parse(q).uses_deck()),
            )
    }

    fn node_tracks(&self, i: usize) -> Vec<usize> {
        // Missing files only show in their own lists, mixed sets only in
        // theirs.
        let all = || {
            (0..self.library.len())
                .filter(|&t| !self.library[t].info.missing && self.in_collection(&self.library[t]))
                .collect::<Vec<_>>()
        };
        let Some(node) = self.nodes.get(i) else {
            return Vec::new();
        };
        match &node.contents {
            Contents::Stream(_) | Contents::Store(_) | Contents::OtherStores | Contents::Wanted => {
                Vec::new()
            }
            Contents::Playlist(id) => self
                .db
                .as_ref()
                .and_then(|db| db.playlist_tracks(*id).ok())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|t| self.index_of.get(&t).copied())
                .collect(),
            Contents::All => all(),
            Contents::Dir(dir) if self.files.dir.as_ref() == Some(dir) => self
                .files
                .paths
                .iter()
                .filter_map(|p| self.path_index.get(p).copied())
                .collect(),
            Contents::Dir(_) => Vec::new(),
            Contents::Under(dir) => self.tracks_under(dir),
            Contents::List(ids) => ids.clone(),
            Contents::Stage(stage, dir) => all()
                .into_iter()
                .filter(|&t| self.at_stage(&self.library[t], *stage, dir.as_deref()))
                .collect(),
            Contents::Query(q) => self.search(&Query::parse(q), None),
            Contents::Health(Some(Check::Duplicate)) => self.dupe_tracks(None),
            Contents::Dupes(case) => self.dupe_tracks(Some(*case)),
            Contents::Health(Some(check)) => self.health_tracks(*check),
            Contents::Health(None) => Vec::new(),
            Contents::Grouping(g) => all()
                .into_iter()
                .filter(|&t| self.library[t].info.grouping == *g)
                .collect(),
            Contents::Label(l) => all()
                .into_iter()
                .filter(|&t| self.library[t].info.labels.contains(l))
                .collect(),
            Contents::Folder => {
                let depth = self.nodes[i].depth;
                let mut ids = Vec::new();
                for j in i + 1..self.nodes.len() {
                    if self.nodes[j].depth <= depth || self.nodes[j].history != node.history {
                        break;
                    }
                    for t in self.node_tracks(j) {
                        if !ids.contains(&t) {
                            ids.push(t);
                        }
                    }
                }
                ids
            }
        }
    }

    /// The tracks of the selected node, with Spotify tracks matched to the
    /// collection.
    fn row_data(&self) -> Vec<RowData<'_>> {
        let local = |id: usize| {
            let t = &self.library[id];
            RowData {
                title: &t.title,
                artist: t.artist.clone(),
                album: &t.album,
                genre: &t.genre,
                bpm: t.bpm,
                key: &t.key,
                duration: t.duration,
                rating: t.rating,
                local: Some(id),
                stream: None,
            }
        };
        let Some(key) = self.current_playlist() else {
            return self
                .node_tracks(self.current_node)
                .into_iter()
                .map(local)
                .collect();
        };
        let Some(tracks) = self.shown_stream().and_then(|st| st.state.tracks.get(key)) else {
            return Vec::new();
        };
        tracks
            .iter()
            .zip(self.spotify_matches(tracks))
            .enumerate()
            .map(|(n, (sp, found))| {
                match found {
                    // Show what Spotify calls it, with what the collection knows.
                    Some(id) => RowData {
                        title: &sp.title,
                        artist: sp.artist(),
                        album: &sp.album,
                        duration: sp.duration,
                        stream: Some(n),
                        ..local(id)
                    },
                    None => RowData {
                        title: &sp.title,
                        artist: sp.artist(),
                        album: &sp.album,
                        genre: "",
                        bpm: 0.0,
                        key: "",
                        duration: sp.duration,
                        rating: 0,
                        local: None,
                        stream: Some(n),
                    },
                }
            })
            .collect()
    }

    /// The collection track (by index into `library`) that each Spotify
    /// track is, if any: the same ISRC first, else the same title and artist.
    fn spotify_matches(&self, tracks: &[StreamTrack]) -> Vec<Option<usize>> {
        // Files only shown in Files don't count as in the collection.
        let normalized: Vec<(String, String)> = self
            .library
            .iter()
            .map(|t| match is_browsed(t) {
                true => Default::default(),
                false => (spotify::normalize(&t.title), spotify::normalize(&t.artist)),
            })
            .collect();
        let mut by_isrc = HashMap::new();
        for (i, t) in self.library.iter().enumerate() {
            let isrc = spotify::normalize_isrc(&t.info.isrc);
            if !isrc.is_empty() && !is_browsed(t) {
                by_isrc.entry(isrc).or_insert(i);
            }
        }
        let none = HashMap::new();
        let manual = self.shown_stream().map_or(&none, |st| &st.state.manual);
        tracks
            .iter()
            .map(|sp| {
                // A match the DJ set by hand wins.
                if let Some(set) = manual.get(&sp.match_key()) {
                    return (*set).and_then(|id| self.index_of.get(&id).copied());
                }
                by_isrc
                    .get(&spotify::normalize_isrc(&sp.isrc))
                    .copied()
                    .or_else(|| {
                        normalized
                            .iter()
                            .position(|(title, artist)| spotify::matches(sp, title, artist))
                    })
            })
            .collect()
    }

    /// The Spotify track shown at `row`, if a playlist is shown.
    fn stream_track(&self, row: usize) -> Option<&StreamTrack> {
        let n = (*self.stream_rows.get(row)?)?;
        self.shown_stream()?
            .state
            .tracks
            .get(self.current_playlist()?)?
            .get(n)
    }

    /// Fills the match menu for the track at `row`: its name as the query,
    /// and the collection tracks that fit it.
    fn open_match(&mut self, row: usize) {
        let Some(sp) = self.stream_track(row) else {
            return;
        };
        let (query, manual) = (
            format!("{} {}", sp.artist(), sp.title),
            self.shown_stream()
                .is_some_and(|st| st.state.manual.contains_key(&sp.match_key())),
        );
        if let Some(app) = self.app.upgrade() {
            let ui = app.global::<StreamMatch>();
            ui.set_query(query.clone().into());
            ui.set_manual(manual);
        }
        self.search_match(&query);
    }

    /// The collection tracks whose artist and title hold every word typed,
    /// best known first.
    fn search_match(&mut self, text: &str) {
        let words: Vec<String> = text
            .split_whitespace()
            .map(spotify::normalize)
            .filter(|w| !w.is_empty())
            .collect();
        let mut found: Vec<usize> = self
            .library
            .iter()
            .enumerate()
            .filter(|(_, t)| !is_browsed(t))
            .filter(|(_, t)| {
                let hay = spotify::normalize(&format!("{} {}", t.artist, t.title));
                !words.is_empty() && words.iter().all(|w| hay.contains(w.as_str()))
            })
            .map(|(i, _)| i)
            .collect();
        found.sort_by_key(|&i| std::cmp::Reverse(self.library[i].info.play_count));
        found.truncate(MATCH_CANDIDATES);
        let lines: Vec<SharedString> = found
            .iter()
            .map(|&i| {
                let t = &self.library[i];
                format!("{} – {}", t.artist, t.title).into()
            })
            .collect();
        if let Some(st) = self.shown_stream_mut() {
            st.state.candidates = found;
        }
        if let Some(app) = self.app.upgrade() {
            app.global::<StreamMatch>()
                .set_candidates(ModelRc::new(VecModel::from(lines)));
        }
    }

    /// Saves which collection track (None: none of them) the track at `row`
    /// is, or with `forget` goes back to matching it automatically.
    fn set_match(&mut self, row: usize, track: Option<usize>, forget: bool) {
        let Some(key) = self.stream_track(row).map(StreamTrack::match_key) else {
            return;
        };
        let id = track.map(|i| self.library[i].id);
        let Some(service) = self.shown_stream().map(|st| st.id) else {
            return;
        };
        if let Some(db) = &self.db {
            if let Err(e) = db.set_stream_match(service, &key, id, forget) {
                eprintln!("can't save the match: {e}");
                return;
            }
        }
        let Some(st) = self.shown_stream_mut() else {
            return;
        };
        if forget {
            st.state.manual.remove(&key);
        } else {
            st.state.manual.insert(key, id);
        }
        self.refresh_rows();
    }

    /// Copies the streaming playlist being shown into a new list of the
    /// DJ's own, once: the tracks the collection has go in the list, the
    /// rest on the wanted list.
    fn copy_stream_playlist(&mut self) {
        let status = |b: &mut Self, s: &str| {
            if let Some(st) = b.shown_stream_mut() {
                st.status = s.to_string();
            }
            b.show_stream();
        };
        let Some(key) = self.current_playlist().map(str::to_string) else {
            return status(self, "Pick a playlist to copy.");
        };
        let Some((service, tracks, playlist)) = self.shown_stream().map(|st| {
            (
                st.name,
                st.state.tracks.get(&key).cloned(),
                st.state
                    .playlists
                    .iter()
                    .find(|p| p.key == key)
                    .map(|p| p.name.clone()),
            )
        }) else {
            return;
        };
        let Some(tracks) = tracks else {
            return status(self, "Wait for the playlist to load, then copy it.");
        };
        if !self.saved() {
            return status(self, "There's no collection to copy into.");
        }
        let name = playlist.unwrap_or_else(|| format!("{service} playlist"));
        let found = self.spotify_matches(&tracks);
        let mut have: Vec<usize> = Vec::new();
        let mut missing = 0;
        let Some(db) = &self.db else { return };
        let list = match db.create_playlist(&name, None, PlaylistKind::List) {
            Ok(id) => id,
            Err(e) => return status(self, &format!("Couldn't make the list: {e}")),
        };
        for (sp, f) in tracks.iter().zip(found) {
            match f {
                // The same track twice in a playlist goes in once.
                Some(i) if !have.contains(&i) => have.push(i),
                Some(_) => {}
                None => {
                    missing += 1;
                    let _ = db.add_wanted(service, &sp.artist(), &sp.title, &sp.page);
                }
            }
        }
        self.store.wanted = db.wanted().unwrap_or_default();
        self.add_to_playlist(&have, list);
        // The collection's tree shows the new list when it's next opened.
        self.collection_stale = true;
        status(
            self,
            &match missing {
                0 => format!("Copied all {} tracks to \"{name}\".", have.len()),
                n => format!(
                    "Copied {} {} to \"{name}\". {n} not in your collection went on the wanted list.",
                    have.len(),
                    if have.len() == 1 { "track" } else { "tracks" },
                ),
            },
        );
    }

    /// What the list says instead of rows while a playlist loads or fails.
    fn list_notice(&self) -> String {
        if matches!(self.tree, ALBUMS | MIXED_SETS) && self.nodes.is_empty() {
            return if self.tree == ALBUMS {
                "Choose your albums folder in Settings › Collection."
            } else {
                "Choose your mixed sets folder in Settings › Collection. Recordings are saved there too."
            }
            .into();
        }
        if self.tree == FILES {
            let f = &self.files;
            return if self.nodes.is_empty() {
                "No folders to show.".into()
            } else if f.dir.is_none() {
                "Pick a folder on the left.".into()
            } else if f.paths.is_empty() && f.reading {
                "Reading…".into()
            } else if f.paths.is_empty() && f.recursive {
                "No music in this folder or its subfolders.".into()
            } else if f.paths.is_empty() {
                "No music in this folder. Turn on Include subfolders to see what's below it.".into()
            } else {
                String::new()
            };
        }
        let Some(key) = self.current_playlist() else {
            return String::new();
        };
        let Some(st) = self.shown_stream().map(|st| &st.state) else {
            return String::new();
        };
        if let Some(e) = st.errors.get(key) {
            e.clone()
        } else if st.loading.contains(key) {
            "Loading…".into()
        } else if st.tracks.get(key).is_some_and(|t| t.is_empty()) {
            "This playlist is empty.".into()
        } else {
            String::new()
        }
    }

    fn refresh_rows(&mut self) {
        if is_stores(self.tree) {
            self.refresh_store_rows();
            return;
        }
        // Collection tracks are searched with the full syntax, filters
        // included; Spotify tracks the collection doesn't have, by their
        // names, and only while nothing is filtered.
        let q = Query::parse(&self.full_search());
        let cx = self.context();
        let needle = self.search.to_lowercase();
        let filtering = self.filtering();
        let mut data: Vec<RowData> = self
            .row_data()
            .into_iter()
            .filter(|r| match r.local {
                _ if q.is_empty() => true,
                Some(id) => q.matches(&self.library[id], &self.indexed[id], &cx),
                None if filtering => false,
                None => [r.title, &r.artist, r.album]
                    .iter()
                    .any(|t| t.to_lowercase().contains(&needle)),
            })
            .collect();
        // A Spotify playlist keeps its own order until a column is clicked.
        // So does a folder in Files, in file name order, and an album.
        // Copies stay in their groups, the one kept first.
        let dupes = self.dupe_view();
        let keep_order = ((self.current_playlist().is_some()
            || matches!(self.tree, FILES | ALBUMS | MIXED_SETS))
            && self.sort_column < 0)
            || dupes.is_some();
        let fade = dupes.map(|case| self.dupe_faded(case)).unwrap_or_default();
        if !keep_order {
            let library = &self.library;
            data.sort_by(|a, b| {
                use std::cmp::Ordering;
                let o = match self.sort_column {
                    columns::ARTIST => a.artist.cmp(&b.artist),
                    columns::TITLE_ID => a.title.cmp(b.title),
                    columns::ALBUM => a.album.cmp(b.album),
                    columns::GENRE => a.genre.cmp(b.genre),
                    columns::BPM => a.bpm.total_cmp(&b.bpm),
                    columns::KEY_ID => a.key.cmp(b.key),
                    columns::LENGTH => a.duration.total_cmp(&b.duration),
                    columns::RATING_ID => a.rating.cmp(&b.rating),
                    // What only the collection knows: Spotify tracks it
                    // doesn't have go last.
                    columns::CRATES => {
                        let names = |r: &RowData| {
                            r.local
                                .and_then(|i| self.crates.get(&library[i].id))
                                .map(|c| c.join(", "))
                                .unwrap_or_default()
                        };
                        let (x, y) = (names(a), names(b));
                        // Tracks in no crate last.
                        (x.is_empty(), x.to_lowercase()).cmp(&(y.is_empty(), y.to_lowercase()))
                    }
                    id => match (a.local, b.local) {
                        (Some(x), Some(y)) => columns::compare(id, &library[x], &library[y]),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => Ordering::Equal,
                    },
                };
                if self.ascending {
                    o
                } else {
                    o.reverse()
                }
            });
        }
        let art = &self.art;
        let library = &self.library;
        let ids = self.columns.ids();
        let rows: Vec<TrackRow> = data
            .iter()
            .map(|r| {
                let cover = r.local.and_then(|id| art.get(&library[id]));
                let color = r.local.and_then(|id| library[id].info.color);
                let row = TrackRow {
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                    title: r.title.into(),
                    artist: r.artist.clone().into(),
                    album: r.album.into(),
                    genre: r.genre.into(),
                    length: short_time(r.duration),
                    bpm: if r.bpm > 0.0 {
                        format!("{:.1}", r.bpm).into()
                    } else {
                        SharedString::new()
                    },
                    key: show_key(r.key, self.camelot).into(),
                    key_match: self.mixes_with_deck(r.key),
                    key_shift: self.key_shift_to_deck(r.key),
                    rating: r.rating as i32,
                    deck: r
                        .local
                        .and_then(|id| {
                            let id = self.library[id].id;
                            self.loaded.iter().position(|l| *l == Some(id))
                        })
                        .map(|d| d as i32)
                        .unwrap_or(-1),
                    available: r.local.is_some(),
                    missing: r.local.is_some_and(|id| library[id].info.missing),
                    has_color: color.is_some(),
                    color: color
                        .map(|c| {
                            slint::Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
                        })
                        .unwrap_or_default(),
                    action: SharedString::new(),
                    cells: Default::default(),
                    wave: Default::default(),
                    has_wave: false,
                    analysing: 0,
                    fade: r.local.is_some_and(|id| fade.contains(&library[id].id)),
                };
                let crates = r.local.and_then(|id| self.crates.get(&library[id].id));
                TrackRow {
                    cells: columns::row_cells(
                        &ids,
                        &row,
                        r.local.map(|id| &*library[id]),
                        crates.map(Vec::as_slice),
                    ),
                    ..row
                }
            })
            .collect();
        let total: f64 = data.iter().map(|r| r.duration).sum();
        let mut summary = total_time(total);
        if self.current_playlist().is_some() && !data.is_empty() {
            let have = data.iter().filter(|r| r.local.is_some()).count();
            summary = format!("{summary} · {have} in your collection");
        }
        let local: Vec<usize> = data.iter().filter_map(|r| r.local).collect();
        if let Some(note) = self.set_note(&local) {
            summary = format!("{summary} · {note}");
        }
        if self.tree == FILES && self.files.reading && !data.is_empty() {
            summary = format!(
                "{summary} · reading {} of {}…",
                data.len(),
                self.files.paths.len()
            );
        }
        let notice = self.list_notice();
        let streams: Vec<Option<usize>> = data.iter().map(|r| r.stream).collect();
        self.rows = data.iter().map(|r| r.local).collect();
        self.stream_rows = streams;
        if let Some(app) = self.app.upgrade() {
            let current = app.get_current_track();
            app.set_folder_duration(summary.into());
            app.set_list_notice(notice.into());
            self.list.set_vec(rows);
            self.wave_rows.clear();
            self.fill_waves();
            self.analysis_shown.marks.clear();
            self.mark_analysis_rows();
            app.set_tracks(ModelRc::from(self.list.clone()));
            // Copies are listed in their groups, whatever the sort.
            app.set_sort_column(if dupes.is_some() {
                -1
            } else {
                self.sort_column
            });
            app.set_sort_ascending(self.ascending);
            if current >= self.rows.len() as i32 {
                app.set_current_track(-1);
            }
        }
    }

    /// Lists the rows again, re-sorted, with the selected track still
    /// selected at its new place (the list then scrolls to it).
    fn refresh_rows_keeping_current(&mut self) {
        let app = match self.app.upgrade() {
            Some(app) if !is_stores(self.tree) => app,
            _ => return self.refresh_rows(),
        };
        let current = usize::try_from(app.get_current_track()).ok();
        // A collection track by its entry; any other row by its names.
        let names = |r: &TrackRow| (r.title.clone(), r.artist.clone(), r.album.clone());
        let was = current.and_then(|i| Some((*self.rows.get(i)?, self.list.row_data(i)?)));
        self.refresh_rows();
        let Some((local, row)) = was else {
            return;
        };
        let at = match local {
            Some(id) => self.rows.iter().position(|r| *r == Some(id)),
            None => (0..self.list.row_count()).find(|&i| {
                self.rows[i].is_none()
                    && self
                        .list
                        .row_data(i)
                        .is_some_and(|r| names(&r) == names(&row))
            }),
        };
        app.set_current_track(at.map_or(-1, |i| i as i32));
    }

    fn refresh_side(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let tab = self.side_tab();
        if tab == LINKED_TAB {
            self.refresh_linked(&app);
            return;
        }
        if tab == info::INFO_TAB {
            return;
        }
        let ids = &self.side[tab];
        let rows: Vec<SideRow> = ids
            .iter()
            .map(|&id| {
                let t = &self.library[id];
                let cover = self.art.get(t);
                SideRow {
                    heading: SharedString::new(),
                    title: t.title.clone().into(),
                    artist: t.artist.clone().into(),
                    bpm: if t.bpm > 0.0 {
                        format!("{:.0}", t.bpm).into()
                    } else {
                        SharedString::new()
                    },
                    key: show_key(&t.key, self.camelot).into(),
                    key_match: self.mixes_with_deck(&t.key),
                    key_shift: self.key_shift_to_deck(&t.key),
                    length: short_time(t.duration),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                }
            })
            .collect();
        let total: f64 = ids.iter().map(|&id| self.library[id].duration).sum();
        app.set_side_total(
            format!(
                "{} · {} song{}",
                short_time(total),
                ids.len(),
                if ids.len() == 1 { "" } else { "s" }
            )
            .into(),
        );
        app.set_side_rows(ModelRc::new(VecModel::from(rows)));
    }

    /// The side panel's rows as library indices, `None` for a heading.
    fn side_ids(&self) -> Vec<Option<usize>> {
        self.side_ids_of(self.side_tab())
    }

    /// The tracks of a side panel tab, `None` for the Linked tab's headings.
    fn side_ids_of(&self, tab: usize) -> Vec<Option<usize>> {
        match tab {
            LINKED_TAB => self.linked.clone(),
            tab => self
                .side
                .get(tab)
                .map(|l| l.iter().map(|&i| Some(i)).collect())
                .unwrap_or_default(),
        }
    }

    fn side_id(&self, row: usize) -> Option<usize> {
        self.side_ids().get(row).copied().flatten()
    }

    /// Fills the Linked tab for the selected track: tracks linked to it,
    /// then those played right before or after it in the history.
    fn refresh_linked(&mut self, app: &AppWindow) {
        self.linked.clear();
        let subject = self.linked_to.map(|i| &self.library[i]);
        let (mut linked, mut history) = (Vec::new(), Vec::new());
        if let (Some(t), Some(db)) = (subject, &self.db) {
            let index = |ids: Vec<usize>| -> Vec<usize> {
                ids.into_iter()
                    .filter_map(|id| self.index_of.get(&id).copied())
                    .collect()
            };
            linked = index(db.linked(t.id).unwrap_or_default());
            history = index(db.played_with(t.id).unwrap_or_default());
            history.retain(|i| !linked.contains(i));
        }
        let mut rows = Vec::new();
        for (heading, ids) in [("Linked", &linked), ("Played with", &history)] {
            if ids.is_empty() {
                continue;
            }
            self.linked.push(None);
            rows.push(SideRow {
                heading: heading.into(),
                ..Default::default()
            });
            for &i in ids {
                self.linked.push(Some(i));
                let t = &self.library[i];
                let cover = self.art.get(t);
                rows.push(SideRow {
                    heading: SharedString::new(),
                    title: t.title.clone().into(),
                    artist: t.artist.clone().into(),
                    bpm: if t.bpm > 0.0 {
                        format!("{:.0}", t.bpm).into()
                    } else {
                        SharedString::new()
                    },
                    key: show_key(&t.key, self.camelot).into(),
                    key_match: self.mixes_with_deck(&t.key),
                    key_shift: self.key_shift_to_deck(&t.key),
                    length: short_time(t.duration),
                    has_cover: cover.is_some(),
                    cover: cover.unwrap_or_default(),
                });
            }
        }
        self.link_rows = if linked.is_empty() {
            0
        } else {
            linked.len() + 1
        };
        app.set_side_total(match subject {
            Some(t) if t.artist.is_empty() => t.title.clone().into(),
            Some(t) => format!("{} - {}", t.artist, t.title).into(),
            None => SharedString::new(),
        });
        app.set_side_rows(ModelRc::new(VecModel::from(rows)));
    }

    /// Links a track dropped on the Linked tab to the selected one.
    fn link(&mut self, index: usize) {
        if let Some(subject) = self.linked_to {
            self.adopt(&[subject, index]);
        }
        let (Some(subject), Some(db)) = (self.linked_to, &self.db) else {
            return;
        };
        let _ = db.link(self.library[subject].id, self.library[index].id);
        // Picking the dragged track up selected it; go back to the one the
        // tab is showing.
        if let (Some(app), Some(row)) = (
            self.app.upgrade(),
            self.rows.iter().position(|r| *r == Some(subject)),
        ) {
            app.set_current_track(row as i32);
        }
        self.refresh_side();
    }

    /// Unlinks a row of the Linked tab; history rows stay, as they are
    /// what was played.
    fn unlink_row(&mut self, row: usize) {
        let is_link = row < self.link_rows;
        let (Some(subject), Some(Some(other)), Some(db)) =
            (self.linked_to, self.linked.get(row).copied(), &self.db)
        else {
            return;
        };
        if is_link {
            let _ = db.unlink(self.library[subject].id, self.library[other].id);
            self.refresh_side();
        }
    }
}

/// The side panel's third tab, after Automix and Sidelist.
const LINKED_TAB: usize = 2;

fn short_time(seconds: f64) -> SharedString {
    fmt_time(seconds)
        .split('.')
        .next()
        .unwrap_or_default()
        .to_string()
        .into()
}

/// A list's total length as the header shows it: "3h 56m", or "42m".
fn total_time(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as u64;
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod total_time_tests {
    use super::total_time;

    #[test]
    fn formats_times_and_sizes() {
        assert_eq!(total_time(14_160.0), "3h 56m");
        assert_eq!(total_time(2_520.0), "42m");
        assert_eq!(total_time(3_600.0), "1h 0m");
        assert_eq!(total_time(0.0), "0m");
    }
}

#[cfg(test)]
mod drag_tests {
    use super::{move_block, parse_dragged, Dragged};

    #[test]
    fn reads_what_a_drag_carries() {
        assert_eq!(parse_dragged("opendj-row:3"), Some(Dragged::Rows(3, 3)));
        assert_eq!(parse_dragged("opendj-row:2-5"), Some(Dragged::Rows(2, 5)));
        assert_eq!(
            parse_dragged("opendj-side:0:1-2"),
            Some(Dragged::Side(0, 1, 2))
        );
        assert_eq!(
            parse_dragged("opendj-side:1:4"),
            Some(Dragged::Side(1, 4, 4))
        );
        assert_eq!(parse_dragged("opendj-deck:1"), Some(Dragged::Deck(1)));
        assert_eq!(parse_dragged("opendj-deck:9"), None);
        assert_eq!(parse_dragged("opendj-row:5-2"), None);
        assert_eq!(parse_dragged("hello"), None);
    }

    #[test]
    fn moves_rows_within_a_list() {
        let moved = |first, last, at| {
            let mut v = vec![0, 1, 2, 3, 4];
            move_block(&mut v, first, last, at);
            v
        };
        assert_eq!(moved(3, 3, 0), [3, 0, 1, 2, 4], "up to the top");
        assert_eq!(moved(0, 0, 5), [1, 2, 3, 4, 0], "down to the end");
        assert_eq!(
            moved(1, 2, 4),
            [0, 3, 1, 2, 4],
            "a picked range moves together"
        );
        assert_eq!(moved(1, 2, 2), [0, 1, 2, 3, 4], "dropped on itself");
        assert_eq!(moved(1, 2, 3), [0, 1, 2, 3, 4], "just below itself");
        assert_eq!(
            moved(4, 9, 0),
            [4, 0, 1, 2, 3],
            "range past the end is clipped"
        );
    }
}
