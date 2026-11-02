//! CLAP effects inside the helper, through `clack-host`.
//!
//! The plugin instance lives on the helper's main thread; its audio
//! processor is sent to the helper's audio thread, where [`ClapEffect`] runs
//! one block per wake. Parameters arrive through the shared block as plain
//! values and are sent to the plugin as events when they change; the routed
//! deck's tempo and beat position go in as a transport event.

use std::ffi::{CStr, CString};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use clack_extensions::audio_ports::{AudioPortInfoBuffer, PluginAudioPorts};
use clack_extensions::latency::PluginLatency;
use clack_extensions::params::{ParamInfoBuffer, ParamInfoFlags, PluginParams};
use clack_host::events::event_types::{
    MidiEvent, NoteOffEvent, NoteOnEvent, ParamValueEvent, TransportEvent, TransportFlags,
};
use clack_host::events::Match;
use clack_host::events::{EventFlags, EventHeader};
use clack_host::prelude::*;
use clack_host::utils::{BeatTime, SecondsTime};

use crate::client::Transport;
use crate::descriptor::Descriptor;
use crate::effects::{Effect, Info, Loaded, MainThread, Param};
use crate::shm::{Midi, CHANNELS, MAX_EVENTS, MAX_PARAMS};

fn text(s: Option<&CStr>) -> String {
    s.map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn load_entry(file: &Path) -> Result<PluginEntry, String> {
    // SAFETY: loading a plugin runs its code; that is why this happens in a
    // helper process that openDJ can lose without harm.
    unsafe { PluginEntry::load(file) }.map_err(|e| format!("can't load {}: {e}", file.display()))
}

/// Lists the plugins in one `.clap` file. Runs in a throwaway helper
/// (`opendj-plugin-host --scan`), never in openDJ.
pub fn scan(file: &Path) -> Result<Vec<Descriptor>, String> {
    let entry = load_entry(file)?;
    let factory = entry
        .get_plugin_factory()
        .ok_or("the file has no plugin factory")?;
    Ok(factory
        .plugin_descriptors()
        .map(|d| Descriptor {
            id: text(d.id()),
            name: text(d.name()),
            vendor: text(d.vendor()),
            version: text(d.version()),
            features: d
                .features()
                .map(|f| f.to_string_lossy().into_owned())
                .collect(),
        })
        .collect())
}

struct Host;

struct HostShared {
    callback: AtomicBool,
}

impl<'a> SharedHandler<'a> for HostShared {
    fn request_restart(&self) {}
    fn request_process(&self) {}
    fn request_callback(&self) {
        self.callback.store(true, Ordering::Release);
    }
}

impl HostHandlers for Host {
    type Shared<'a> = HostShared;
    type MainThread<'a> = ();
    type AudioProcessor<'a> = ();
}

/// Keeps the instance (and the library) alive on the main thread.
struct ClapMain {
    instance: PluginInstance<Host>,
    _entry: PluginEntry,
}

impl MainThread for ClapMain {
    fn idle(&mut self) {
        let asked = self
            .instance
            .access_shared_handler(|s| s.callback.swap(false, Ordering::AcqRel));
        if asked {
            self.instance.call_on_main_thread_callback();
        }
    }
}

/// Loads plugin `id` from `file` and activates it for blocks of up to
/// `max_frames`.
pub fn load(file: &Path, id: &str, sample_rate: u32, max_frames: usize) -> Result<Loaded, String> {
    let entry = load_entry(file)?;
    let factory = entry
        .get_plugin_factory()
        .ok_or("the file has no plugin factory")?;
    let desc = factory
        .plugin_descriptors()
        .find(|d| d.id().is_some_and(|i| i.to_bytes() == id.as_bytes()))
        .ok_or_else(|| format!("{} has no plugin {id:?}", file.display()))?;
    let name = text(desc.name());
    let cid = CString::new(id).map_err(|_| "bad plugin id")?;
    let host = HostInfo::new(
        "openDJ",
        "openDJ",
        "https://opendj.app",
        env!("CARGO_PKG_VERSION"),
    )
    .map_err(|e| e.to_string())?;
    let mut instance = PluginInstance::<Host>::new(
        |_| HostShared {
            callback: AtomicBool::new(false),
        },
        |_| (),
        &entry,
        &cid,
        &host,
    )
    .map_err(|e| format!("{name} failed to start: {e}"))?;

    // A stereo output, and a stereo input unless it is an instrument,
    // which has none.
    let handle = instance.plugin_handle();
    let ports = handle
        .get_extension::<PluginAudioPorts>()
        .ok_or_else(|| format!("{name} has no audio ports"))?;
    let mut buf = AudioPortInfoBuffer::new();
    let has_input = ports.count(&handle, true) >= 1;
    for is_input in [true, false] {
        if is_input && !has_input {
            continue;
        }
        let ok = ports.count(&handle, is_input) >= 1
            && ports
                .get(&handle, 0, is_input, &mut buf)
                .is_some_and(|p| p.channel_count == 2);
        if !ok {
            return Err(format!("{name} is not a stereo effect or instrument"));
        }
    }

    // Its first parameters, as the slot's parameters.
    let mut params = Vec::new();
    if let Some(ext) = handle.get_extension::<PluginParams>() {
        let mut buf = ParamInfoBuffer::new();
        for i in 0..ext.count(&handle) {
            if params.len() == MAX_PARAMS {
                break;
            }
            let Some(p) = ext.get_info(&handle, i, &mut buf) else {
                continue;
            };
            if p.flags
                .intersects(ParamInfoFlags::IS_HIDDEN | ParamInfoFlags::IS_READONLY)
            {
                continue;
            }
            let value = ext.get_value(&handle, p.id).unwrap_or(p.default_value);
            params.push((
                p.id,
                Param {
                    name: String::from_utf8_lossy(p.name).into_owned(),
                    min: p.min_value,
                    max: p.max_value,
                    default: p.default_value,
                    value,
                },
            ));
        }
    }

    let processor = instance
        .activate(
            |_, _| (),
            PluginAudioConfiguration {
                sample_rate: sample_rate as f64,
                min_frames_count: 1,
                max_frames_count: max_frames as u32,
            },
        )
        .map_err(|e| format!("{name} failed to activate: {e}"))?;
    let handle = instance.plugin_handle();
    let latency = handle
        .get_extension::<PluginLatency>()
        .map(|l| l.get(&handle))
        .unwrap_or(0);

    let effect = ClapEffect {
        processor: processor.into(),
        input: [vec![0.0; max_frames], vec![0.0; max_frames]],
        output: [vec![0.0; max_frames], vec![0.0; max_frames]],
        in_ports: AudioPorts::with_capacity(CHANNELS, 1),
        has_input,
        out_ports: AudioPorts::with_capacity(CHANNELS, 1),
        events: EventBuffer::with_capacity(MAX_PARAMS + MAX_EVENTS + 4),
        out_events: EventBuffer::with_capacity(64),
        ids: params.iter().map(|(id, _)| *id).collect(),
        sent: params.iter().map(|(_, p)| p.value as f32).collect(),
        steady: 0,
    };
    Ok(Loaded {
        effect: Box::new(effect),
        main: Some(Box::new(ClapMain {
            instance,
            _entry: entry,
        })),
        info: Info {
            name,
            params: params.into_iter().map(|(_, p)| p).collect(),
            latency,
        },
    })
}

struct ClapEffect {
    processor: PluginAudioProcessor<Host>,
    input: [Vec<f32>; CHANNELS],
    output: [Vec<f32>; CHANNELS],
    in_ports: AudioPorts,
    /// False for an instrument, which takes notes instead of audio.
    has_input: bool,
    out_ports: AudioPorts,
    events: EventBuffer,
    out_events: EventBuffer,
    ids: Vec<ClapId>,
    /// The values last sent to the plugin.
    sent: Vec<f32>,
    steady: u64,
}

fn transport_event(t: &Transport) -> TransportEvent {
    let mut flags = TransportFlags::HAS_TEMPO
        | TransportFlags::HAS_BEATS_TIMELINE
        | TransportFlags::HAS_TIME_SIGNATURE;
    if t.playing {
        flags |= TransportFlags::IS_PLAYING;
    }
    let bar = (t.beat / 4.0).floor();
    TransportEvent {
        header: EventHeader::new_core(0, EventFlags::empty()),
        flags,
        song_pos_beats: BeatTime::from_float(t.beat),
        song_pos_seconds: SecondsTime::from_int(0),
        tempo: t.bpm,
        tempo_inc: 0.0,
        loop_start_beats: BeatTime::from_int(0),
        loop_end_beats: BeatTime::from_int(0),
        loop_start_seconds: SecondsTime::from_int(0),
        loop_end_seconds: SecondsTime::from_int(0),
        bar_start: BeatTime::from_float(bar * 4.0),
        bar_number: bar as i32,
        time_signature_numerator: 4,
        time_signature_denominator: 4,
    }
}

impl Effect for ClapEffect {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        transport: &Transport,
        notes: &[Midi],
    ) {
        let frames = input[0].len();
        // A plugin that can't start or fails a block plays dry, as a late
        // block does.
        if self.processor.ensure_processing_started().is_err() {
            for (i, o) in input.iter().zip(output) {
                o.copy_from_slice(i);
            }
            return;
        }
        self.events.clear();
        for (i, id) in self.ids.iter().enumerate() {
            let v = params[i];
            if v != self.sent[i] {
                self.sent[i] = v;
                self.events
                    .push(&ParamValueEvent::new(0, *id, Pckn::match_all(), v as f64));
            }
        }
        // Notes in CLAP's own events, which every instrument takes; other
        // MIDI (controllers, pitch bend) as it is.
        for n in notes {
            let pckn = |ch: u8, key: u8| Pckn::new(0u16, ch as u16, key as u16, Match::All);
            if let Some((ch, key, vel)) = n.as_note_on() {
                self.events
                    .push(&NoteOnEvent::new(n.frame, pckn(ch, key), vel as f64));
            } else if let Some((ch, key)) = n.as_note_off() {
                self.events
                    .push(&NoteOffEvent::new(n.frame, pckn(ch, key), 0.0));
            } else {
                self.events.push(&MidiEvent::new(n.frame, 0, n.data));
            }
        }
        for (ch, data) in input.iter().enumerate() {
            self.input[ch][..frames].copy_from_slice(data);
        }
        let transport = (transport.bpm > 0.0).then(|| transport_event(transport));
        let [in_l, in_r] = &mut self.input;
        let ports = usize::from(self.has_input);
        let inputs = self.in_ports.with_input_buffers(
            [AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_input_only(
                    [&mut in_l[..frames], &mut in_r[..frames]]
                        .into_iter()
                        .map(InputChannel::variable),
                ),
            }]
            .into_iter()
            .take(ports),
        );
        let [out_l, out_r] = &mut self.output;
        let mut outputs = self.out_ports.with_output_buffers([AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::f32_output_only(
                [&mut out_l[..frames], &mut out_r[..frames]].into_iter(),
            ),
        }]);
        self.out_events.clear();
        let ok = match self.processor.as_started_mut() {
            Ok(p) => p
                .process(
                    &inputs,
                    &mut outputs,
                    &InputEvents::from_buffer(&self.events),
                    &mut OutputEvents::from_buffer(&mut self.out_events),
                    Some(self.steady),
                    transport.as_ref(),
                )
                .is_ok(),
            Err(_) => false,
        };
        self.steady += frames as u64;
        for (ch, o) in output.into_iter().enumerate() {
            if ok {
                o.copy_from_slice(&self.output[ch][..frames]);
            } else {
                o.copy_from_slice(input[ch]);
            }
        }
    }
}
