//! The Controller Editor: shows what a controller's mapping does, by
//! function, with the controller's picture. Reading a mapping means running
//! it in a sandbox (see `mixxx_controllers::probe`), which takes a moment,
//! so it happens on a thread of its own. While the editor is open, messages
//! from the controllers are watched: pressing a control finds it.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use mixxx_control::ControlRegistry;
use mixxx_controllers::layout::Layout;
use mixxx_controllers::probe::message_key;
use mixxx_controllers::view::{self, LightRow, MappingView, Row};
use mixxx_controllers::Controllers;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{
    short_port_name, AppWindow, ControllerEditor, EditorOption, EditorPanel, EditorPart, EditorRow,
};

/// How long a part stays lit after its control is pressed.
const FLASH: Duration = Duration::from_millis(350);
/// The list's row heights (FunctionRow in controller_editor.slint).
const ROW_HEIGHT: f32 = 44.0;
const HEADER_HEIGHT: f32 = 34.0;

pub struct Editor {
    app: slint::Weak<AppWindow>,
    controllers: Option<Rc<Controllers>>,
    dir: Option<PathBuf>,
    /// The mappings in the menu, by file name, and their names.
    choices: Vec<(String, String)>,
    /// The mapping being read, and where its view will come from.
    loading: Option<(String, Receiver<Result<MappingView, String>>)>,
    view: Option<MappingView>,
    /// The list narrowed to this part of the picture (or control name, when
    /// there is no picture).
    part: Option<String>,
    /// The selected row, as (tab, index into the view's rows or lights).
    selected: Option<(i32, usize)>,
    /// Model rows of the tab shown, as indexes into the view (None for a
    /// heading).
    shown: Vec<Option<usize>>,
    flash: Option<(String, Instant)>,
    scroll_serial: i32,
    was_open: bool,
    /// The last query and tab the list was filled for.
    filled_for: (String, i32),
}

impl Editor {
    pub fn install(app: &AppWindow, controllers: Option<Rc<Controllers>>) -> Rc<RefCell<Editor>> {
        let editor = Rc::new(RefCell::new(Editor {
            app: app.as_weak(),
            controllers,
            dir: mixxx_controllers::default_mappings_dir(),
            choices: Vec::new(),
            loading: None,
            view: None,
            part: None,
            selected: None,
            shown: Vec::new(),
            flash: None,
            scroll_serial: 0,
            was_open: false,
            filled_for: (String::new(), 0),
        }));
        let g = app.global::<ControllerEditor>();
        let e = editor.clone();
        g.on_show(move || e.borrow_mut().show());
        let e = editor.clone();
        g.on_show_mapping(move |file| e.borrow_mut().show_mapping(&file));
        let e = editor.clone();
        g.on_pick(move |i| e.borrow_mut().pick(i.max(0) as usize));
        let e = editor.clone();
        g.on_part_clicked(move |id| e.borrow_mut().part_clicked(&id));
        let e = editor.clone();
        g.on_row_clicked(move |tab, i| e.borrow_mut().row_clicked(tab, i.max(0) as usize));
        let e = editor.clone();
        g.on_searched(move || e.borrow_mut().fill());
        let e = editor.clone();
        g.on_clear_filter(move || {
            let mut e = e.borrow_mut();
            e.part = None;
            e.fill();
        });
        editor
    }

    /// The controllers plugged in, with the mapping each runs: its file
    /// name (unknown for a stand-in device) and its name.
    fn running(&self) -> Vec<(String, Option<String>, String)> {
        self.controllers
            .as_ref()
            .map(|c| {
                c.ports()
                    .into_iter()
                    .filter_map(|p| Some((p.name, p.mapping_file, p.running?)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The ports running the mapping shown.
    fn live_ports(&self) -> Vec<String> {
        let Some(v) = &self.view else {
            return Vec::new();
        };
        self.running()
            .into_iter()
            .filter(|(_, file, name)| file.as_ref() == Some(&v.file) || *name == v.name)
            .map(|(p, _, _)| p)
            .collect()
    }

    fn show(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let Some(c) = &self.controllers else {
            g.set_open(true);
            g.set_status("Controller mappings were not found.".into());
            return;
        };
        // The controllers plugged in first, then those with a picture, then
        // the rest by name.
        let running: Vec<(Option<String>, String)> =
            self.running().into_iter().map(|(_, f, n)| (f, n)).collect();
        let runs = |file: &str, name: &str| {
            running
                .iter()
                .any(|(f, n)| f.as_deref() == Some(file) || n == name)
        };
        let pictured = Layout::files();
        let mut choices = c.mappings();
        choices.sort_by_key(|(name, file)| {
            (
                !runs(file, name),
                !pictured.contains(file),
                name.to_lowercase(),
            )
        });
        self.choices = choices.into_iter().map(|(n, f)| (f, n)).collect();
        let names: Vec<SharedString> = self.choices.iter().map(|(_, n)| n.into()).collect();
        g.set_mappings(ModelRc::new(VecModel::from(names)));
        g.set_open(true);
        let current = self.view.as_ref().map(|v| (v.file.clone(), v.name.clone()));
        let keep = current.filter(|(f, n)| running.is_empty() || runs(f, n));
        let i = keep
            .and_then(|(f, _)| self.choices.iter().position(|(c, _)| *c == f))
            .unwrap_or(0);
        if self.loading.is_none() && (self.view.is_none() || self.chosen() != Some(i)) {
            self.pick(i);
        } else {
            g.set_chosen(i as i32);
        }
    }

    /// Opens the editor on the mapping in `file`, or as [`Self::show`] does
    /// when it isn't one of the mappings.
    fn show_mapping(&mut self, file: &str) {
        self.show();
        let Some(i) = self.choices.iter().position(|(f, _)| f == file) else {
            return;
        };
        let reading = self.loading.as_ref().is_some_and(|(f, _)| f == file);
        if !reading && self.chosen() != Some(i) {
            self.pick(i);
        }
    }

    fn chosen(&self) -> Option<usize> {
        let file = &self.view.as_ref()?.file;
        self.choices.iter().position(|(f, _)| f == file)
    }

    /// Reads the mapping at `index` in the menu.
    fn pick(&mut self, index: usize) {
        let (Some(app), Some(dir)) = (self.app.upgrade(), self.dir.clone()) else {
            return;
        };
        let Some((file, name)) = self.choices.get(index).cloned() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        g.set_chosen(index as i32);
        g.set_name(name.into());
        g.set_author("".into());
        g.set_loading(true);
        g.set_status("Reading the mapping…".into());
        self.view = None;
        self.part = None;
        self.selected = None;
        self.fill();
        self.picture();
        let (tx, rx) = mpsc::channel();
        let path = dir.join(&file);
        std::thread::Builder::new()
            .name("controller-editor".into())
            .spawn(move || {
                // A sandbox with every control the app has, so the mapping
                // runs as it would for real without touching the decks.
                let registry = ControlRegistry::new();
                mixxx_engine::create_controls(&registry);
                let _library = crate::browser::LibraryControls::create(&registry);
                let _automix = crate::automix::AutoMix::new(&registry);
                let _ = tx.send(view::build(&path, registry));
            })
            .expect("spawn controller editor thread");
        self.loading = Some((file, rx));
    }

    /// Called every frame.
    pub fn poll(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let open = g.get_open();
        if open != self.was_open {
            self.was_open = open;
            if let Some(c) = &self.controllers {
                c.set_monitor(open);
            }
        }
        if !open {
            return;
        }
        if let Some((_, rx)) = &self.loading {
            if let Ok(result) = rx.try_recv() {
                self.loading = None;
                g.set_loading(false);
                match result {
                    Ok(v) => {
                        g.set_author(v.author.clone().into());
                        g.set_status(if v.notes.is_empty() {
                            "".into()
                        } else {
                            format!("Parts of this mapping's script failed: {}", v.notes[0]).into()
                        });
                        g.set_options(ModelRc::new(VecModel::from(
                            v.options
                                .iter()
                                .map(|o| EditorOption {
                                    label: o.label.clone().into(),
                                    detail: o.description.clone().into(),
                                    default_value: o.default.clone().into(),
                                    kind: o.kind.clone().into(),
                                })
                                .collect::<Vec<_>>(),
                        )));
                        self.view = Some(v);
                    }
                    Err(e) => g.set_status(format!("Can't read this mapping: {e}").into()),
                }
                self.fill();
                self.picture();
            }
        }
        if (g.get_query().to_string(), g.get_tab()) != self.filled_for {
            self.fill();
        }
        self.hint(&g);
        self.find_pressed();
        if self
            .flash
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > FLASH)
        {
            self.flash = None;
            self.picture();
        }
    }

    /// Says how to find a control by pressing it.
    fn hint(&self, g: &ControllerEditor) {
        let port = self.live_ports().into_iter().next();
        let (live, text) = match (&port, &self.view) {
            (Some(p), _) => (
                true,
                format!("Press a control on the {} to find it", short_port_name(p)),
            ),
            (None, Some(v)) => (
                false,
                format!("Plug in the {} to find a control by pressing it", v.name),
            ),
            (None, None) => (false, String::new()),
        };
        if g.get_live() != live {
            g.set_live(live);
        }
        if g.get_find_hint() != text.as_str() {
            g.set_find_hint(text.into());
        }
    }

    /// Finds the control pressed on the controller this mapping is for.
    fn find_pressed(&mut self) {
        let Some(c) = &self.controllers else {
            return;
        };
        let seen = c.take_monitored();
        let Some(v) = &self.view else {
            return;
        };
        let ports: HashSet<String> = self.live_ports().into_iter().collect();
        let Some(key) = seen
            .iter()
            .rev()
            .filter(|(p, m)| ports.contains(p) && m.len() >= 2 && m[0] < 0xF0)
            .map(|(_, m)| message_key(m[0], m[1]))
            .find(|k| v.rows.iter().any(|r| r.keys.contains(k)))
        else {
            return;
        };
        let Some(index) = v.rows.iter().position(|r| r.keys.contains(&key)) else {
            return;
        };
        let row = &v.rows[index];
        let part = row.part.clone().unwrap_or_else(|| row.control.clone());
        let same = self.part.as_deref() == Some(part.as_str()) && self.selected == Some((0, index));
        self.flash = Some((part.clone(), Instant::now()));
        if !same {
            self.part = Some(part);
            self.selected = Some((0, index));
            if let Some(app) = self.app.upgrade() {
                let g = app.global::<ControllerEditor>();
                g.set_tab(0);
                g.set_query("".into());
            }
            self.fill();
            self.scroll_to_selected();
        }
        self.picture();
    }

    fn scroll_to_selected(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let Some((_, sel)) = self.selected else {
            return;
        };
        let mut y = 0.0;
        for s in &self.shown {
            match s {
                Some(i) if *i == sel => break,
                Some(_) => y += ROW_HEIGHT,
                None => y += HEADER_HEIGHT,
            }
        }
        let g = app.global::<ControllerEditor>();
        self.scroll_serial += 1;
        g.set_scroll_to(y);
        g.set_scroll_serial(self.scroll_serial);
    }

    fn part_clicked(&mut self, id: &str) {
        let mapped = self
            .view
            .as_ref()
            .is_some_and(|v| v.rows.iter().any(|r| r.part.as_deref() == Some(id)));
        if !mapped {
            return;
        }
        self.part = if self.part.as_deref() == Some(id) {
            None
        } else {
            Some(id.to_string())
        };
        self.selected = None;
        if let Some(app) = self.app.upgrade() {
            let g = app.global::<ControllerEditor>();
            if g.get_tab() == 2 {
                g.set_tab(0);
            }
        }
        self.fill();
        self.picture();
    }

    fn row_clicked(&mut self, tab: i32, model_index: usize) {
        let Some(Some(i)) = self.shown.get(model_index).copied() else {
            return;
        };
        self.selected = if self.selected == Some((tab, i)) {
            None
        } else {
            Some((tab, i))
        };
        self.fill();
        self.picture();
    }

    /// The part the selected row belongs to.
    fn selected_part(&self) -> Option<String> {
        let v = self.view.as_ref()?;
        match self.selected? {
            (0, i) => v.rows.get(i)?.part.clone(),
            (_, i) => v.lights.get(i)?.part.clone(),
        }
    }

    /// Fills the list for the tab, search and part shown.
    fn fill(&mut self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let query = g.get_query().to_string();
        let tab = g.get_tab();
        self.filled_for = (query.clone(), tab);
        let words: Vec<String> = query
            .to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect();
        let part = self.part.clone();
        let mut model = Vec::new();
        self.shown.clear();
        let Some(v) = &self.view else {
            g.set_rows(ModelRc::default());
            g.set_lights(ModelRc::default());
            g.set_summary("".into());
            g.set_part_filter("".into());
            return;
        };
        let on_part = |p: &Option<String>, control: &str| match &part {
            None => true,
            Some(id) => p.as_deref() == Some(id.as_str()) || (p.is_none() && control == id),
        };
        let mut category = "";
        let mut push = |header: Option<&'static str>,
                        row: EditorRow,
                        index: usize,
                        model: &mut Vec<EditorRow>,
                        shown: &mut Vec<Option<usize>>| {
            if let Some(c) = header {
                if c != category {
                    category = c;
                    model.push(EditorRow {
                        header: true,
                        function: c.to_uppercase().into(),
                        ..Default::default()
                    });
                    shown.push(None);
                }
            }
            model.push(row);
            shown.push(Some(index));
        };
        let matches = |text: &str| {
            let text = text.to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        };
        if tab == 1 {
            for (i, l) in v.lights.iter().enumerate() {
                let row = light_row(v, l, self.selected == Some((1, i)));
                if on_part(&l.part, &l.control) && matches(&search_text(&row, l.function.category))
                {
                    push(
                        Some(l.function.category),
                        row,
                        i,
                        &mut model,
                        &mut self.shown,
                    );
                }
            }
            g.set_lights(ModelRc::new(VecModel::from(model)));
        } else {
            for (i, r) in v.rows.iter().enumerate() {
                let row = function_row(r, self.selected == Some((0, i)));
                if on_part(&r.part, &r.control) && matches(&search_text(&row, r.function.category))
                {
                    push(
                        Some(r.function.category),
                        row,
                        i,
                        &mut model,
                        &mut self.shown,
                    );
                }
            }
            g.set_rows(ModelRc::new(VecModel::from(model)));
        }
        let controls: HashSet<&str> = v.rows.iter().map(|r| r.control.as_str()).collect();
        g.set_summary(
            match tab {
                0 => format!("{} functions on {} controls", v.rows.len(), controls.len()),
                1 => format!("{} lights", v.lights.len()),
                _ => format!("{} settings", v.options.len()),
            }
            .into(),
        );
        let filter = part.map(|id| {
            v.layout
                .as_ref()
                .and_then(|l| l.parts.iter().find(|p| p.id == id))
                .map_or(id.clone(), |p| p.label.clone())
        });
        g.set_part_filter(filter.unwrap_or_default().into());
    }

    /// Draws the controller: its parts, which are mapped, and which are lit.
    fn picture(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let g = app.global::<ControllerEditor>();
        let Some((v, layout)) = self
            .view
            .as_ref()
            .and_then(|v| Some((v, v.layout.as_ref()?)))
        else {
            g.set_has_picture(false);
            g.set_parts(ModelRc::default());
            g.set_panels(ModelRc::default());
            return;
        };
        let mapped: HashSet<&str> = v.rows.iter().filter_map(|r| r.part.as_deref()).collect();
        let lit = [self.part.clone(), self.selected_part()];
        let flash = self.flash.as_ref().map(|(p, _)| p.as_str());
        g.set_picture_width(layout.width);
        g.set_picture_height(layout.height);
        g.set_panels(ModelRc::new(VecModel::from(
            layout
                .panels
                .iter()
                .map(|p| EditorPanel {
                    x: p[0],
                    y: p[1],
                    w: p[2],
                    h: p[3],
                    r: p[4],
                })
                .collect::<Vec<_>>(),
        )));
        g.set_parts(ModelRc::new(VecModel::from(
            layout
                .parts
                .iter()
                .map(|p| EditorPart {
                    id: p.id.clone().into(),
                    label: p.label.clone().into(),
                    kind: p.kind.clone().into(),
                    round: p.round,
                    x: p.x,
                    y: p.y,
                    w: p.w,
                    h: p.h,
                    mapped: mapped.contains(p.id.as_str()),
                    lit: lit.iter().any(|l| l.as_deref() == Some(p.id.as_str())),
                    flash: flash == Some(p.id.as_str()),
                })
                .collect::<Vec<_>>(),
        )));
        g.set_has_picture(true);
    }
}

fn function_row(r: &Row, selected: bool) -> EditorRow {
    // The options, without "Script" (the row has a tag for that), and the
    // last part of the author's note ("PLAY/PAUSE (DECK1) - press -
    // Play/Pause" → "Play/Pause").
    let options = r.options.strip_prefix("Script").unwrap_or(&r.options);
    let options = options.trim_start_matches(", ");
    let mut detail: Vec<&str> = Vec::new();
    if !options.is_empty() {
        detail.push(options);
    }
    let note = r.description.rsplit(" - ").next().unwrap_or("").trim();
    if !note.is_empty() && !note.eq_ignore_ascii_case(&r.function.name) {
        detail.push(note);
    }
    // The script function only on the selected row: it's long and only
    // tells someone who reads the script.
    if let Some(s) = r.script.as_ref().filter(|_| selected) {
        detail.push(s);
    }
    EditorRow {
        header: false,
        function: r.function.name.clone().into(),
        target: r.function.target.clone().into(),
        control: r.control.clone().into(),
        layer: r.layer.clone().into(),
        kind: r.kind.clone().into(),
        midi: r.midi.clone().into(),
        script: r.script.clone().unwrap_or_default().into(),
        detail: detail.join(" · ").into(),
        part: r.part.clone().unwrap_or_default().into(),
        selected,
    }
}

fn light_row(v: &MappingView, l: &LightRow, selected: bool) -> EditorRow {
    let kind = l
        .part
        .as_ref()
        .and_then(|id| v.layout.as_ref()?.parts.iter().find(|p| &p.id == id))
        .map_or("button".to_string(), |p| p.kind.clone());
    // The layer of the control whose LED it is: a pad mode, a bank.
    let layer = v
        .layout
        .as_ref()
        .and_then(|layout| layout.find(l.key.0, l.key.1))
        .map(|(_, m)| m.layer.clone())
        .unwrap_or_default();
    EditorRow {
        header: false,
        function: l.function.name.clone().into(),
        target: l.function.target.clone().into(),
        control: l.control.clone().into(),
        layer: layer.into(),
        kind: kind.into(),
        midi: format!("Out: {}", l.midi).into(),
        script: SharedString::new(),
        detail: SharedString::new(),
        part: l.part.clone().unwrap_or_default().into(),
        selected,
    }
}

fn search_text(r: &EditorRow, category: &str) -> String {
    [
        category,
        &r.function,
        &r.target,
        &r.control,
        &r.layer,
        &r.midi,
        &r.script,
        &r.detail,
    ]
    .join(" ")
}
