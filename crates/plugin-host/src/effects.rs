//! What a helper runs. Real plugin formats (CLAP, then VST3 and AU) will
//! implement [`Effect`] too; for now there are test effects that misbehave
//! on purpose, so the isolation can be tested without third-party plugins.

use crate::client::Transport;
use crate::shm::CHANNELS;

/// One loaded effect, inside the helper.
pub trait Effect {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        transport: &Transport,
    );
}

/// Ids [`load`] knows.
pub const TEST_EFFECTS: [&str; 4] = ["test-gain", "test-crash", "test-hang", "test-slow"];

/// Loads effect `id`, or says why not.
pub fn load(id: &str, _sample_rate: u32) -> Result<Box<dyn Effect>, String> {
    match id {
        "test-gain" => Ok(Box::new(Gain)),
        "test-crash" => Ok(Box::new(Crash { blocks: 0 })),
        "test-hang" => Ok(Box::new(Hang)),
        "test-slow" => Ok(Box::new(Slow)),
        _ => Err(format!("unknown effect {id:?}")),
    }
}

/// Multiplies by parameter 0.
struct Gain;

impl Effect for Gain {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        _: &Transport,
    ) {
        let g = params[0];
        for (i, o) in input.iter().zip(output) {
            for (x, y) in i.iter().zip(o.iter_mut()) {
                *y = x * g;
            }
        }
    }
}

/// Passes audio, then aborts once it has done parameter 0's number of
/// blocks (when that is above 0), like a plugin with a memory bug.
struct Crash {
    blocks: u32,
}

impl Effect for Crash {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
    ) {
        if params[0] > 0.0 && self.blocks as f32 >= params[0] {
            std::process::abort();
        }
        self.blocks += 1;
        Gain.process(input, output, &[1.0], t);
    }
}

/// Passes audio until parameter 0 goes above 0.5, then never returns.
struct Hang;

impl Effect for Hang {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
    ) {
        if params[0] > 0.5 {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Gain.process(input, output, &[1.0], t);
    }
}

/// Passes audio after sleeping parameter 0 milliseconds per block.
struct Slow;

impl Effect for Slow {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        t: &Transport,
    ) {
        if params[0] > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f32(params[0] / 1000.0));
        }
        Gain.process(input, output, &[1.0], t);
    }
}
