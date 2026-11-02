//! A tiny CLAP effect for the plugin-host tests: a stereo gain with one
//! parameter ("Gain", 0 to 2, default 1) and a reported latency of 64 frames,
//! plus a second parameter ("Tempo out") that copies the host's tempo into the
//! left channel's first sample, so tests can check the transport arrives.
//!
//! It is a real `.clap` binary, loaded through the same path as third-party
//! plugins.

use std::sync::atomic::{AtomicU64, Ordering};

use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPorts,
    PluginAudioPortsImpl,
};
use clack_extensions::latency::{PluginLatency, PluginLatencyImpl};
use clack_extensions::params::{
    ParamDisplayWriter, ParamInfo, ParamInfoFlags, ParamInfoWriter, PluginAudioProcessorParams,
    PluginMainThreadParams, PluginParams,
};
use clack_plugin::events::event_types::{ParamValueEvent, TransportEvent};
use clack_plugin::prelude::*;
use clack_plugin::utils::Cookie;

const GAIN: u32 = 1;
const TEMPO_OUT: u32 = 2;

pub struct TestGain;

pub struct Shared {
    gain: AtomicU64,
    tempo_out: AtomicU64,
}

impl Shared {
    fn apply(&self, events: &InputEvents) {
        for e in events {
            if let Some(p) = e.as_event::<ParamValueEvent>() {
                match p.param_id().map(|i| i.get()) {
                    Some(GAIN) => self.gain.store(p.value().to_bits(), Ordering::Relaxed),
                    Some(TEMPO_OUT) => self.tempo_out.store(p.value().to_bits(), Ordering::Relaxed),
                    _ => {}
                }
            }
        }
    }

    fn get(&self, id: u32) -> Option<f64> {
        match id {
            GAIN => Some(f64::from_bits(self.gain.load(Ordering::Relaxed))),
            TEMPO_OUT => Some(f64::from_bits(self.tempo_out.load(Ordering::Relaxed))),
            _ => None,
        }
    }
}

impl<'a> PluginShared<'a> for Shared {}

pub struct MainThread<'a> {
    shared: &'a Shared,
}

impl<'a> PluginMainThread<'a, Shared> for MainThread<'a> {}

impl Plugin for TestGain {
    type AudioProcessor<'a> = Processor<'a>;
    type Shared<'a> = Shared;
    type MainThread<'a> = MainThread<'a>;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _shared: Option<&Shared>) {
        builder
            .register::<PluginAudioPorts>()
            .register::<PluginParams>()
            .register::<PluginLatency>();
    }
}

impl DefaultPluginFactory for TestGain {
    fn get_descriptor() -> PluginDescriptor {
        use clack_plugin::plugin::features::{AUDIO_EFFECT, STEREO};
        PluginDescriptor::new("app.opendj.test-gain", "openDJ Test Gain")
            .with_vendor("openDJ")
            .with_features([AUDIO_EFFECT, STEREO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Shared, PluginError> {
        Ok(Shared {
            gain: AtomicU64::new(1.0f64.to_bits()),
            tempo_out: AtomicU64::new(0.0f64.to_bits()),
        })
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        shared: &'a Shared,
    ) -> Result<MainThread<'a>, PluginError> {
        Ok(MainThread { shared })
    }
}

pub struct Processor<'a> {
    shared: &'a Shared,
}

impl<'a> PluginAudioProcessor<'a, Shared, MainThread<'a>> for Processor<'a> {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main: &MainThread<'a>,
        shared: &'a Shared,
        _config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(Self { shared })
    }

    fn process(
        &mut self,
        process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        self.shared.apply(events.input);
        let gain = self.shared.get(GAIN).unwrap_or(1.0) as f32;
        let tempo = process.transport.map(|t: &TransportEvent| t.tempo);
        let tempo_out = self.shared.get(TEMPO_OUT).unwrap_or(0.0) > 0.5;
        for mut port in &mut audio {
            let Some(pairs) = port.channels()?.into_f32() else {
                continue;
            };
            for (ch, pair) in pairs.into_iter().enumerate() {
                let out: &mut [f32] = match pair {
                    ChannelPair::InputOnly(_) => continue,
                    ChannelPair::OutputOnly(buf) => {
                        buf.fill(0.0);
                        continue;
                    }
                    ChannelPair::InputOutput(i, o) => {
                        for (x, y) in i.iter().zip(o.iter_mut()) {
                            *y = x * gain;
                        }
                        o
                    }
                    ChannelPair::InPlace(buf) => {
                        for s in buf.iter_mut() {
                            *s *= gain;
                        }
                        buf
                    }
                };
                if tempo_out && ch == 0 && !out.is_empty() {
                    out[0] = tempo.unwrap_or(-1.0) as f32;
                }
            }
        }
        Ok(ProcessStatus::Continue)
    }
}

impl PluginAudioProcessorParams for Processor<'_> {
    fn flush(&mut self, input: &InputEvents, _output: &mut OutputEvents) {
        self.shared.apply(input);
    }
}

impl PluginAudioPortsImpl for MainThread<'_> {
    fn count(&self, _is_input: bool) -> u32 {
        1
    }

    fn get(&self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        if index == 0 {
            writer.set(&AudioPortInfo {
                id: ClapId::new(0),
                name: if is_input { b"In" } else { b"Out" },
                channel_count: 2,
                flags: AudioPortFlags::IS_MAIN,
                port_type: Some(AudioPortType::STEREO),
                in_place_pair: None,
            });
        }
    }
}

impl PluginMainThreadParams for MainThread<'_> {
    fn count(&self) -> u32 {
        2
    }

    fn get_info(&self, index: u32, info: &mut ParamInfoWriter) {
        let (id, name, max, default): (u32, &[u8], f64, f64) = match index {
            0 => (GAIN, b"Gain", 2.0, 1.0),
            1 => (TEMPO_OUT, b"Tempo out", 1.0, 0.0),
            _ => return,
        };
        info.set(&ParamInfo {
            id: ClapId::new(id),
            flags: ParamInfoFlags::IS_AUTOMATABLE,
            cookie: Cookie::empty(),
            name,
            module: b"",
            min_value: 0.0,
            max_value: max,
            default_value: default,
        });
    }

    fn get_value(&self, id: ClapId) -> Option<f64> {
        self.shared.get(id.get())
    }

    fn value_to_text(
        &self,
        _id: ClapId,
        value: f64,
        writer: &mut ParamDisplayWriter,
    ) -> std::fmt::Result {
        use std::fmt::Write;
        write!(writer, "{value:.2}")
    }

    fn text_to_value(&self, _id: ClapId, text: &std::ffi::CStr) -> Option<f64> {
        text.to_str().ok()?.trim().parse().ok()
    }

    fn flush(&self, input: &InputEvents, _output: &mut OutputEvents) {
        self.shared.apply(input);
    }
}

impl PluginLatencyImpl for MainThread<'_> {
    fn get(&self) -> u32 {
        64
    }
}

clack_export_entry!(SinglePluginEntry<TestGain>);
