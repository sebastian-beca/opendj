//! Plain names for engine controls, grouped the way a DJ looks for them:
//! `[Channel1],hotcue_2_activate` is "Hot cue 2" on "Deck 1" under
//! "Hot cues". Used by the Controller Editor to say what a mapping does.

/// The categories, in the order the editor lists them.
pub const CATEGORIES: [&str; 20] = [
    "Transport",
    "Jog & scratch",
    "Cues",
    "Hot cues",
    "Loops",
    "Beat jump",
    "Tempo & sync",
    "Key",
    "Mixer",
    "EQ & filter",
    "Effects",
    "Stems",
    "Samplers",
    "Pads",
    "Library",
    "Automix",
    "Mic & aux",
    "Recording",
    "View",
    "Controller",
];

/// What a control does, in words.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Function {
    /// One of [`CATEGORIES`].
    pub category: &'static str,
    pub name: String,
    /// "Deck 1", "FX 2", "Main": what it acts on.
    pub target: String,
}

impl Function {
    pub fn new(category: &'static str, name: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            category,
            name: name.into(),
            target: target.into(),
        }
    }

    /// The category's place in [`CATEGORIES`] (unknown ones last).
    pub fn order(&self) -> usize {
        CATEGORIES
            .iter()
            .position(|c| *c == self.category)
            .unwrap_or(CATEGORIES.len())
    }
}

/// What a group is, parsed from Mixxx's group names.
#[derive(Clone, Debug, PartialEq)]
enum Group {
    Deck(u32),
    Stem(u32, u32),
    Sampler(u32),
    Preview,
    Eq(u32),
    QuickFx(u32),
    StemFx(u32, u32),
    Sculpt(u32),
    FxUnit(u32),
    FxSlot(u32, u32),
    Main,
    Library,
    AutoDj,
    Skin,
    Mic(u32),
    Aux(u32),
    Recording,
    Controller,
    Mixer,
    Clock,
    Other(String),
}

fn num(s: &str) -> Option<u32> {
    if s.is_empty() {
        return Some(1);
    }
    s.parse().ok()
}

/// "Channel1" → 1, "Channel2_Stem3" → (2, Some(3)).
fn channel(s: &str) -> Option<(u32, Option<u32>)> {
    let rest = s.strip_prefix("Channel")?;
    match rest.split_once("_Stem") {
        // "[Channel1_Stem]": a script's name for the deck's stems as a whole.
        Some((d, "")) => Some((d.parse().ok()?, None)),
        Some((d, st)) => Some((d.parse().ok()?, Some(st.parse().ok()?))),
        None => Some((rest.parse().ok()?, None)),
    }
}

fn parse_group(group: &str) -> Group {
    let inner = group.trim_start_matches('[').trim_end_matches(']');
    if let Some((d, stem)) = channel(inner) {
        return match stem {
            Some(s) => Group::Stem(d, s),
            None => Group::Deck(d),
        };
    }
    if let Some(n) = inner.strip_prefix("Sampler").and_then(num) {
        return Group::Sampler(n);
    }
    if inner.starts_with("PreviewDeck") {
        return Group::Preview;
    }
    // "[EqualizerRack1_[Channel1]_Effect1]"
    if let Some(rest) = group.strip_prefix("[EqualizerRack1_[") {
        if let Some((ch, _)) = rest.split_once(']') {
            if let Some((d, _)) = channel(ch) {
                return Group::Eq(d);
            }
        }
    }
    if let Some(rest) = group.strip_prefix("[QuickEffectRack1_[") {
        if let Some((ch, _)) = rest.split_once(']') {
            return match channel(ch) {
                Some((d, Some(s))) => Group::StemFx(d, s),
                Some((d, None)) => Group::QuickFx(d),
                None => Group::Other(inner.into()),
            };
        }
    }
    if let Some(rest) = group.strip_prefix("[Sculpt_[") {
        if let Some((d, _)) = rest.split_once(']').and_then(|(c, _)| channel(c)) {
            return Group::Sculpt(d);
        }
    }
    if let Some(rest) = inner.strip_prefix("EffectRack1_EffectUnit") {
        return match rest.split_once("_Effect") {
            Some((u, e)) => match (u.parse(), e.parse()) {
                (Ok(u), Ok(e)) => Group::FxSlot(u, e),
                _ => Group::Other(inner.into()),
            },
            None => rest
                .parse()
                .map_or(Group::Other(inner.into()), Group::FxUnit),
        };
    }
    if let Some(n) = inner.strip_prefix("Microphone").and_then(num) {
        return Group::Mic(n);
    }
    if let Some(n) = inner.strip_prefix("Auxiliary").and_then(num) {
        return Group::Aux(n);
    }
    match inner {
        "Master" | "Main" | "Headphone" => Group::Main,
        "Library" | "Playlist" => Group::Library,
        "AutoDJ" => Group::AutoDj,
        "Skin" => Group::Skin,
        "Recording" => Group::Recording,
        "Controller" | "PadMode" => Group::Controller,
        "Mixer" => Group::Mixer,
        "InternalClock" => Group::Clock,
        _ => Group::Other(inner.into()),
    }
}

/// "hotcue_12_activate" with prefix "hotcue_" and suffix "_activate" → "12".
fn middle<'a>(key: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
    key.strip_prefix(prefix)?.strip_suffix(suffix)
}

/// Beat counts as a DJ reads them: "0.25" → "1/4".
fn beats(n: &str) -> String {
    match n {
        "0.03125" => "1/32".into(),
        "0.0625" => "1/16".into(),
        "0.125" => "1/8".into(),
        "0.25" => "1/4".into(),
        "0.5" => "1/2".into(),
        _ => n.to_string(),
    }
}

fn plural(n: &str) -> String {
    let b = beats(n);
    if b == "1" || b.contains('/') {
        format!("{b} beat")
    } else {
        format!("{b} beats")
    }
}

/// "loop_halve" → "Loop halve"; "MoveFocusForward" → "Move focus forward".
pub fn humanize(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if c == '_' || c == '.' || c == '-' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !out.ends_with(' ') {
            out.push(' ');
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        out.push(c);
    }
    let lower = out.trim().to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// A deck control (`[ChannelN]`, also samplers and the preview deck).
fn deck_function(key: &str, target: &str) -> Function {
    let f = |c: &'static str, n: &str| Function::new(c, n, target);
    if let Some(n) = middle(key, "hotcue_", "_activate") {
        return f("Hot cues", &format!("Hot cue {n}"));
    }
    if let Some(n) = middle(key, "hotcue_", "_clear") {
        return f("Hot cues", &format!("Clear hot cue {n}"));
    }
    if let Some(n) = middle(key, "hotcue_", "_set") {
        return f("Hot cues", &format!("Set hot cue {n}"));
    }
    if let Some(n) = middle(key, "hotcue_", "_gotoandplay") {
        return f("Hot cues", &format!("Play from hot cue {n}"));
    }
    if let Some(n) = middle(key, "hotcue_", "_gotoandstop") {
        return f("Hot cues", &format!("Stop at hot cue {n}"));
    }
    for suffix in ["_status", "_enabled"] {
        if let Some(n) = middle(key, "hotcue_", suffix) {
            return f("Hot cues", &format!("Hot cue {n}"));
        }
    }
    if let Some(base) = key.strip_suffix("_indicator") {
        return deck_function(base, target);
    }
    if let Some(n) = middle(key, "hotcue_", "_goto") {
        return f("Hot cues", &format!("Go to hot cue {n}"));
    }
    if let Some(n) = middle(key, "beatlooproll_", "_activate") {
        return f("Loops", &format!("Loop roll {}", plural(n)));
    }
    for suffix in ["_toggle", "_activate", "_enabled"] {
        if let Some(n) = middle(key, "beatloop_", suffix) {
            if n.parse::<f64>().is_ok() {
                return f("Loops", &format!("Loop {}", plural(n)));
            }
        }
    }
    if let Some(n) = middle(key, "beatjump_", "_forward") {
        return f("Beat jump", &format!("Jump {} forward", plural(n)));
    }
    if let Some(n) = middle(key, "beatjump_", "_backward") {
        return f("Beat jump", &format!("Jump {} back", plural(n)));
    }
    if let Some(n) = middle(key, "loop_move_", "_forward") {
        return f("Loops", &format!("Move loop {} forward", plural(n)));
    }
    if let Some(n) = middle(key, "loop_move_", "_backward") {
        return f("Loops", &format!("Move loop {} back", plural(n)));
    }
    if let Some(n) = middle(key, "keyboard_", "_activate") {
        return f("Key", &format!("Keyboard pad {n}"));
    }
    let (c, n): (&'static str, &str) = match key {
        "play" => ("Transport", "Play / pause"),
        "play_stutter" => ("Transport", "Stutter play"),
        "stop" => ("Transport", "Stop"),
        "start" => ("Transport", "Jump to start"),
        "start_play" => ("Transport", "Play from start"),
        "start_stop" => ("Transport", "Back to start and stop"),
        "end" => ("Transport", "Jump to end"),
        "reverse" => ("Transport", "Reverse"),
        "reverseroll" => ("Transport", "Reverse roll (censor)"),
        "fwd" => ("Transport", "Fast forward"),
        "back" => ("Transport", "Rewind"),
        "eject" => ("Transport", "Eject"),
        "slip_enabled" => ("Transport", "Slip mode"),
        "repeat" => ("Transport", "Repeat"),
        "playposition" => ("Transport", "Position"),
        "brake" => ("Transport", "Brake"),
        "rate_search" => ("Transport", "Fast search"),
        "LoadSelectedTrack" => ("Library", "Load selected track"),
        "LoadSelectedTrackAndPlay" => ("Library", "Load and play"),
        "jog" => ("Jog & scratch", "Jog nudge"),
        "jog_bend" => ("Jog & scratch", "Jog rim bend"),
        "scratch2" => ("Jog & scratch", "Scratch"),
        "scratch2_enable" => ("Jog & scratch", "Scratch (platter touch)"),
        "wheel" => ("Jog & scratch", "Jog wheel"),
        "cue_default" => ("Cues", "Cue"),
        "cue_set" => ("Cues", "Set cue"),
        "cue_goto" => ("Cues", "Go to cue"),
        "cue_gotoandplay" => ("Cues", "Play from cue"),
        "cue_gotoandstop" => ("Cues", "Stop at cue"),
        "cue_play" => ("Cues", "Cue and play"),
        "cue_preview" => ("Cues", "Preview cue"),
        "cue_cdj" => ("Cues", "Cue (CDJ)"),
        "intro_start_activate" => ("Cues", "Intro start"),
        "intro_end_activate" => ("Cues", "Intro end"),
        "outro_start_activate" => ("Cues", "Outro start"),
        "outro_end_activate" => ("Cues", "Outro end"),
        "intro_start_clear" => ("Cues", "Clear intro start"),
        "intro_end_clear" => ("Cues", "Clear intro end"),
        "outro_start_clear" => ("Cues", "Clear outro start"),
        "outro_end_clear" => ("Cues", "Clear outro end"),
        "loop_in" => ("Loops", "Loop in"),
        "loop_out" => ("Loops", "Loop out"),
        "loop_in_goto" => ("Loops", "Go to loop in"),
        "loop_out_goto" => ("Loops", "Go to loop out"),
        "reloop_toggle" => ("Loops", "Reloop / exit"),
        "reloop_andstop" => ("Loops", "Reloop and stop"),
        "reloop_exit" => ("Loops", "Reloop / exit"),
        "loop_halve" | "beatloop_halve" => ("Loops", "Halve loop"),
        "loop_double" | "beatloop_double" => ("Loops", "Double loop"),
        "beatloop_activate" => ("Loops", "Loop (current size)"),
        "beatloop_toggle" => ("Loops", "Loop on / off"),
        "beatloop_size" => ("Loops", "Loop size"),
        "beatlooproll_activate" => ("Loops", "Loop roll (current size)"),
        "loop_enabled" => ("Loops", "Loop on"),
        "loop_remove" => ("Loops", "Remove loop"),
        "loop_start_position" => ("Loops", "Loop start"),
        "loop_end_position" => ("Loops", "Loop end"),
        "beatjump_forward" => ("Beat jump", "Beat jump forward"),
        "beatjump_backward" => ("Beat jump", "Beat jump back"),
        "beatjump_size" => ("Beat jump", "Beat jump size"),
        "beatjump" => ("Beat jump", "Beat jump"),
        "rate" => ("Tempo & sync", "Tempo"),
        "rate_perm_up" => ("Tempo & sync", "Tempo up"),
        "rate_perm_down" => ("Tempo & sync", "Tempo down"),
        "rate_perm_up_small" => ("Tempo & sync", "Tempo up a little"),
        "rate_perm_down_small" => ("Tempo & sync", "Tempo down a little"),
        "rate_temp_up" | "rate_temp_up_small" => ("Tempo & sync", "Nudge faster"),
        "rate_temp_down" | "rate_temp_down_small" => ("Tempo & sync", "Nudge slower"),
        "rateRange" => ("Tempo & sync", "Tempo range"),
        "rate_dir" => ("Tempo & sync", "Tempo direction"),
        "rate_set_default" => ("Tempo & sync", "Reset tempo"),
        "sync_enabled" => ("Tempo & sync", "Sync"),
        "beatsync" | "beatsync_tempo" => ("Tempo & sync", "Sync once"),
        "beatsync_phase" => ("Tempo & sync", "Sync phase"),
        "sync_leader" | "sync_master" => ("Tempo & sync", "Sync leader"),
        "bpm_tap" => ("Tempo & sync", "Tap tempo"),
        "bpm" => ("Tempo & sync", "BPM"),
        "quantize" => ("Tempo & sync", "Quantize"),
        "beats_translate_curpos" => ("Tempo & sync", "Move grid to playhead"),
        "beats_translate_earlier" => ("Tempo & sync", "Move grid earlier"),
        "beats_translate_later" => ("Tempo & sync", "Move grid later"),
        "keylock" => ("Key", "Key lock"),
        "pitch" | "pitch_adjust" => ("Key", "Key shift"),
        "pitch_up" | "pitch_adjust_up" => ("Key", "Key up"),
        "pitch_down" | "pitch_adjust_down" => ("Key", "Key down"),
        "pitch_up_small" => ("Key", "Key up a little"),
        "pitch_down_small" => ("Key", "Key down a little"),
        "pitch_set_default" | "pitch_adjust_set_default" | "reset_key" => ("Key", "Reset key"),
        "sync_key" => ("Key", "Match key"),
        "volume" => ("Mixer", "Channel fader"),
        "volume_set_default" => ("Mixer", "Reset channel fader"),
        "pregain" => ("Mixer", "Gain"),
        "pregain_set_default" => ("Mixer", "Reset gain"),
        "pregain_set_one" => ("Mixer", "Reset gain"),
        "pfl" => ("Mixer", "Headphone cue"),
        "orientation" => ("Mixer", "Crossfader side"),
        "mute" => ("Mixer", "Mute"),
        "talkover" => ("Mic & aux", "Talk over"),
        "pad_panel" => ("Pads", "Pad mode"),
        "pad_tab" => ("Pads", "Pad tab"),
        k if k.starts_with("pad_slot_") => ("Pads", "Pad tab page"),
        "waveform_zoom" => ("View", "Waveform zoom"),
        "waveform_zoom_up" => ("View", "Zoom waveform in"),
        "waveform_zoom_down" => ("View", "Zoom waveform out"),
        "waveform_zoom_set_default" => ("View", "Reset waveform zoom"),
        _ => ("Controller", ""),
    };
    if n.is_empty() {
        return f(guess_category(key), &humanize(key));
    }
    f(c, n)
}

/// For controls the tables above don't name: a category from words in it.
fn guess_category(key: &str) -> &'static str {
    let k = key.to_lowercase();
    let has = |w: &str| k.contains(w);
    if has("hotcue") {
        "Hot cues"
    } else if has("loop") {
        "Loops"
    } else if has("beatjump") {
        "Beat jump"
    } else if has("cue") || has("intro") || has("outro") {
        "Cues"
    } else if has("rate") || has("sync") || has("bpm") || has("beats") {
        "Tempo & sync"
    } else if has("pitch") || has("key") {
        "Key"
    } else if has("scratch") || has("jog") {
        "Jog & scratch"
    } else if has("waveform") || has("zoom") {
        "View"
    } else {
        "Controller"
    }
}

const EQ_BANDS: [&str; 3] = ["Low", "Mid", "High"];

/// Says what `[group],key` does.
pub fn describe(group: &str, key: &str) -> Function {
    let other = |c: &'static str, target: &str| Function::new(c, humanize(key), target);
    match parse_group(group) {
        Group::Deck(d) => deck_function(key, &format!("Deck {d}")),
        Group::Preview => deck_function(key, "Preview"),
        Group::Sampler(n) => {
            let target = format!("Sampler {n}");
            let mut f = deck_function(key, &target);
            if f.category != "Hot cues" {
                f.category = "Samplers";
            }
            f.name = match key {
                "cue_gotoandplay" | "start_play" => "Play from start".into(),
                "LoadSelectedTrack" => "Load selected track".into(),
                _ => f.name,
            };
            f
        }
        Group::Stem(d, s) => {
            let target = format!("Deck {d}");
            let name = match key {
                "volume" => format!("Stem {s} volume"),
                "mute" => format!("Stem {s} mute"),
                "volume_set_default" => format!("Reset stem {s} volume"),
                _ => format!("Stem {s} {}", humanize(key).to_lowercase()),
            };
            Function::new("Stems", name, target)
        }
        Group::StemFx(d, s) => {
            let name = match key {
                "super1" => format!("Stem {s} FX knob"),
                "enabled" => format!("Stem {s} FX on"),
                _ => format!("Stem {s} FX {}", humanize(key).to_lowercase()),
            };
            Function::new("Stems", name, format!("Deck {d}"))
        }
        Group::Eq(d) => {
            let target = format!("Deck {d}");
            let band = |p: &str| {
                p.parse::<usize>()
                    .ok()
                    .and_then(|i| EQ_BANDS.get(i.wrapping_sub(1)))
                    .copied()
            };
            if let Some(b) = key.strip_prefix("button_parameter").and_then(band) {
                return Function::new("EQ & filter", format!("Kill {}", b.to_lowercase()), target);
            }
            if let Some(rest) = key.strip_prefix("parameter") {
                let (p, reset) = match rest.split_once('_') {
                    Some((p, _)) => (p, true),
                    None => (rest, false),
                };
                if let Some(b) = band(p) {
                    let name = if reset {
                        format!("Reset {} EQ", b.to_lowercase())
                    } else {
                        format!("{b} EQ")
                    };
                    return Function::new("EQ & filter", name, target);
                }
            }
            other("EQ & filter", &target)
        }
        Group::QuickFx(d) => {
            let target = format!("Deck {d}");
            let name = match key {
                "super1" => "Filter",
                "super1_set_default" => "Reset filter",
                "enabled" => "Filter on",
                "chain_preset_selector" | "next_chain" | "next_chain_preset" => {
                    "Next filter effect"
                }
                "prev_chain" | "prev_chain_preset" => "Previous filter effect",
                _ => return other("EQ & filter", &target),
            };
            Function::new("EQ & filter", name, target)
        }
        Group::Sculpt(d) => Function::new(
            "EQ & filter",
            format!("Sculpt {}", humanize(key).to_lowercase()),
            format!("Deck {d}"),
        ),
        Group::FxUnit(u) => {
            let target = format!("FX {u}");
            if let Some(g) = middle(key, "group_", "_enable") {
                let to = match parse_group(g) {
                    Group::Deck(d) => format!("Deck {d}"),
                    Group::Main => g.trim_matches(['[', ']']).to_string(),
                    Group::Sampler(n) => format!("Sampler {n}"),
                    _ => g.trim_matches(['[', ']']).to_string(),
                };
                return Function::new("Effects", format!("Assign to {to}"), target);
            }
            let name = match key {
                "mix" => "Wet / dry".to_string(),
                "super1" => "Super knob".into(),
                "enabled" => "Unit on".into(),
                "next_chain" | "next_chain_preset" => "Next chain".into(),
                "prev_chain" | "prev_chain_preset" => "Previous chain".into(),
                "chain_selector" | "chain_preset_selector" => "Choose chain".into(),
                "focused_effect" => "Focused effect".into(),
                "mix_mode" => "Mix mode".into(),
                "clear" => "Clear unit".into(),
                "beats" => "Effect length".into(),
                "bpm_tap" => "Tap effect tempo".into(),
                "release" => "Release".into(),
                _ => humanize(key),
            };
            Function::new("Effects", name, target)
        }
        Group::FxSlot(u, e) => {
            let target = format!("FX {u}");
            let name = if let Some(p) = key.strip_prefix("button_parameter") {
                format!("Effect {e} button {p}")
            } else if let Some(p) = key.strip_prefix("parameter") {
                format!("Effect {e} parameter {p}")
            } else {
                match key {
                    "enabled" => format!("Effect {e} on"),
                    "meta" => format!("Effect {e} knob"),
                    "next_effect" => format!("Effect {e} next"),
                    "prev_effect" => format!("Effect {e} previous"),
                    "effect_selector" => format!("Effect {e} choose"),
                    "clear" => format!("Effect {e} clear"),
                    _ => format!("Effect {e} {}", humanize(key).to_lowercase()),
                }
            };
            Function::new("Effects", name, target)
        }
        Group::Main => {
            let (c, n): (&'static str, &str) = match key {
                "crossfader" => ("Mixer", "Crossfader"),
                "crossfader_set_default" => ("Mixer", "Reset crossfader"),
                "gain" => ("Mixer", "Main level"),
                "headGain" => ("Mixer", "Headphone level"),
                "headMix" => ("Mixer", "Headphone mix"),
                "headSplit" => ("Mixer", "Split cue"),
                "balance" => ("Mixer", "Balance"),
                "booth_gain" | "boothGain" => ("Mixer", "Booth level"),
                "talkover_ducking" | "duckStrength" => ("Mic & aux", "Talk-over ducking"),
                "maximize_library" => ("View", "Maximise library"),
                _ => ("Mixer", ""),
            };
            let n = if n.is_empty() {
                humanize(key)
            } else {
                n.into()
            };
            Function::new(c, n, "Main")
        }
        Group::Library => {
            let n = match key {
                "MoveVertical" | "ScrollVertical" | "SelectTrackKnob" => "Scroll list",
                "MoveUp" | "SelectPrevTrack" => "Up",
                "MoveDown" | "SelectNextTrack" => "Down",
                "MoveLeft" => "Left",
                "MoveRight" => "Right",
                "MoveHorizontal" => "Left / right",
                "MoveFocus" | "MoveFocusForward" => "Next panel",
                "MoveFocusBackward" => "Previous panel",
                "GoToItem" => "Open or load item",
                "SelectPlaylist" => "Scroll playlists",
                "SelectNextPlaylist" => "Next playlist",
                "SelectPrevPlaylist" => "Previous playlist",
                "ToggleSelectedSidebarItem" => "Open or close folder",
                "AutoDjAddBottom" => "Add to Automix",
                "AutoDjAddTop" => "Add to top of Automix",
                "ShowLinked" => "Show linked tracks",
                "LoadSelectedIntoFirstStopped" => "Load into a stopped deck",
                "sort_column_toggle" | "sort_column" => "Sort by column",
                "font_size_increment" => "Bigger text",
                "font_size_decrement" => "Smaller text",
                _ => "",
            };
            let n = if n.is_empty() {
                humanize(key)
            } else {
                n.into()
            };
            Function::new("Library", n, "Library")
        }
        Group::AutoDj => {
            let n = match key {
                "enabled" => "Automix on".into(),
                "fade_now" => "Fade now".into(),
                "skip_next" => "Skip next".into(),
                _ => humanize(key),
            };
            Function::new("Automix", n, "Automix")
        }
        Group::Skin => {
            let n = match key {
                "layout" => "Switch layout".into(),
                _ => humanize(key),
            };
            Function::new("View", n, "View")
        }
        Group::Mic(n) | Group::Aux(n) => {
            let target = match parse_group(group) {
                Group::Mic(_) => format!("Mic {n}"),
                _ => format!("Aux {n}"),
            };
            let name = match key {
                "talkover" => "Talk over".into(),
                "pregain" => "Gain".into(),
                "volume" => "Level".into(),
                "pfl" => "Headphone cue".into(),
                "main_mix" | "master" => "To main mix".into(),
                _ => humanize(key),
            };
            Function::new("Mic & aux", name, target)
        }
        Group::Recording => Function::new(
            "Recording",
            match key {
                "toggle_recording" => "Record".into(),
                _ => humanize(key),
            },
            "Recording",
        ),
        Group::Controller => {
            let n = match key {
                "deck3" => "Switch left side to deck 3".into(),
                "deck4" => "Switch right side to deck 4".into(),
                _ => humanize(key),
            };
            Function::new("Controller", n, "Controller")
        }
        Group::Mixer => {
            let n = match key {
                "knob_mode" => "Mixer knob mode".into(),
                _ => humanize(key),
            };
            Function::new("Mixer", n, "Mixer")
        }
        Group::Clock => Function::new(
            "Tempo & sync",
            format!("Clock {}", humanize(key).to_lowercase()),
            "Main",
        ),
        Group::Other(g) => Function::new(guess_category(key), humanize(key), g),
    }
}

/// Like [`describe`], using the value a press wrote where that says more:
/// `beatjump` -4 is "Jump 4 beats back", `pitch_adjust` 2 is "Key +2".
pub fn describe_value(group: &str, key: &str, value: f64) -> Function {
    let mut f = describe(group, key);
    let n = |v: f64| {
        let s = format!("{}", v.abs());
        s.trim_end_matches(".0").to_string()
    };
    match key {
        "beatjump" if value != 0.0 => {
            let dir = if value > 0.0 { "forward" } else { "back" };
            f.name = format!("Jump {} {dir}", plural(&n(value)));
        }
        "beatjump_size" if value > 0.0 => f.name = format!("Beat jump size {}", beats(&n(value))),
        "beatloop_size" if value > 0.0 => f.name = format!("Loop size {}", beats(&n(value))),
        "pitch_adjust" | "pitch" => {
            f.name = if value == 0.0 {
                "Key back to original".into()
            } else {
                format!("Key {}{}", if value > 0.0 { "+" } else { "−" }, n(value))
            };
        }
        _ => {}
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_common_controls() {
        let d = |g: &str, k: &str| {
            let f = describe(g, k);
            (f.category, f.name, f.target)
        };
        assert_eq!(
            d("[Channel1]", "play"),
            ("Transport", "Play / pause".into(), "Deck 1".into())
        );
        assert_eq!(
            d("[Channel2]", "hotcue_3_clear"),
            ("Hot cues", "Clear hot cue 3".into(), "Deck 2".into())
        );
        assert_eq!(
            d("[Channel1]", "beatloop_0.25_toggle"),
            ("Loops", "Loop 1/4 beat".into(), "Deck 1".into())
        );
        assert_eq!(
            d("[Channel1]", "beatjump_8_forward"),
            ("Beat jump", "Jump 8 beats forward".into(), "Deck 1".into())
        );
        assert_eq!(
            d("[EqualizerRack1_[Channel1]_Effect1]", "parameter3"),
            ("EQ & filter", "High EQ".into(), "Deck 1".into())
        );
        assert_eq!(
            d("[EqualizerRack1_[Channel2]_Effect1]", "button_parameter1"),
            ("EQ & filter", "Kill low".into(), "Deck 2".into())
        );
        assert_eq!(
            d("[QuickEffectRack1_[Channel2]]", "super1"),
            ("EQ & filter", "Filter".into(), "Deck 2".into())
        );
        assert_eq!(
            d("[EffectRack1_EffectUnit2]", "group_[Channel1]_enable"),
            ("Effects", "Assign to Deck 1".into(), "FX 2".into())
        );
        assert_eq!(
            d("[EffectRack1_EffectUnit1_Effect3]", "meta"),
            ("Effects", "Effect 3 knob".into(), "FX 1".into())
        );
        assert_eq!(
            d("[Channel1_Stem2]", "mute"),
            ("Stems", "Stem 2 mute".into(), "Deck 1".into())
        );
        assert_eq!(
            d("[Master]", "crossfader"),
            ("Mixer", "Crossfader".into(), "Main".into())
        );
        assert_eq!(
            d("[Library]", "MoveVertical"),
            ("Library", "Scroll list".into(), "Library".into())
        );
        assert_eq!(
            d("[Sampler3]", "cue_gotoandplay"),
            ("Samplers", "Play from start".into(), "Sampler 3".into())
        );
    }

    #[test]
    fn uses_written_values() {
        assert_eq!(
            describe_value("[Channel1]", "beatjump", -4.0).name,
            "Jump 4 beats back"
        );
        assert_eq!(
            describe_value("[Channel1]", "beatjump", 0.5).name,
            "Jump 1/2 beat forward"
        );
        assert_eq!(
            describe_value("[Channel1]", "pitch_adjust", 2.0).name,
            "Key +2"
        );
    }

    #[test]
    fn humanizes_names() {
        assert_eq!(humanize("MoveFocusForward"), "Move focus forward");
        assert_eq!(humanize("loop_halve"), "Loop halve");
        assert_eq!(humanize("cueLoopCallLeft"), "Cue loop call left");
    }
}
