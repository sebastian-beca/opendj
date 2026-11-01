//! The block of memory openDJ and a helper share: a header of atomics, then
//! the input and output audio, one channel after the other.
//!
//! Who may touch what, and when:
//!
//! - openDJ writes the input, the frame count and the transport, then bumps
//!   `seq_in`. It only does so while the helper is idle (`seq_out == seq_in`).
//! - The helper reads them after it sees the new `seq_in`, writes the output,
//!   then sets `seq_out` to that number.
//! - openDJ reads the output only once `seq_out` has caught up.
//!
//! The release/acquire pairs on the two counters order every other access, so
//! the audio buffers themselves need no locks.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use memmap2::MmapMut;

/// "openDJ P" in ASCII, so a helper can tell it was handed the right file.
pub const MAGIC: u64 = 0x6f70_656e_444a_2050;
/// Bumped whenever the layout below changes.
pub const VERSION: u32 = 1;
/// Two channels: decks and the main mix are stereo.
pub const CHANNELS: usize = 2;
/// Parameters a slot can set without a round trip.
pub const MAX_PARAMS: usize = 32;

/// Shared state, at the start of the mapping.
#[repr(C)]
pub struct Header {
    pub magic: AtomicU64,
    pub version: AtomicU32,
    pub max_frames: AtomicU32,
    pub sample_rate: AtomicU32,
    /// Frames in the block `seq_in` names.
    pub frames: AtomicU32,
    /// Block openDJ last handed over.
    pub seq_in: AtomicU64,
    /// Block the helper last finished.
    pub seq_out: AtomicU64,
    /// The routed deck's tempo, as `f64` bits.
    pub bpm: AtomicU64,
    /// Beat position at the start of the block, as `f64` bits.
    pub beat: AtomicU64,
    pub playing: AtomicU32,
    /// How long the helper took over the last block, in nanoseconds.
    pub process_ns: AtomicU64,
    /// Parameter values, as `f32` bits.
    pub params: [AtomicU32; MAX_PARAMS],
}

/// One shared mapping, from either side.
pub struct Shm {
    map: MmapMut,
    max_frames: usize,
}

// The mapping is plain memory, and the protocol above says who may write it.
unsafe impl Send for Shm {}
unsafe impl Sync for Shm {}

/// Bytes a mapping for `max_frames` takes.
pub fn size_for(max_frames: usize) -> usize {
    std::mem::size_of::<Header>() + 2 * CHANNELS * max_frames * std::mem::size_of::<f32>()
}

impl Shm {
    /// Creates the file at `path` and maps it (openDJ's side).
    pub fn create(path: &Path, max_frames: usize, sample_rate: u32) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        file.set_len(size_for(max_frames) as u64)?;
        let shm = Self::map(&file, max_frames)?;
        let h = shm.header();
        h.max_frames.store(max_frames as u32, Ordering::Relaxed);
        h.sample_rate.store(sample_rate, Ordering::Relaxed);
        h.version.store(VERSION, Ordering::Relaxed);
        h.magic.store(MAGIC, Ordering::Release);
        Ok(shm)
    }

    /// Maps a file openDJ created (the helper's side).
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let len = file.metadata()?.len() as usize;
        if len < std::mem::size_of::<Header>() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "too short"));
        }
        // Read the header first to learn the buffer size.
        let shm = Self::map(&file, 0)?;
        let h = shm.header();
        if h.magic.load(Ordering::Acquire) != MAGIC || h.version.load(Ordering::Relaxed) != VERSION
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not an openDJ plugin block, or another version",
            ));
        }
        let max_frames = h.max_frames.load(Ordering::Relaxed) as usize;
        if len < size_for(max_frames) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "too short"));
        }
        Ok(Self {
            map: shm.map,
            max_frames,
        })
    }

    fn map(file: &File, max_frames: usize) -> io::Result<Self> {
        // SAFETY: the file is ours (openDJ creates it with create_new, the
        // helper is handed its path); nothing else resizes it.
        let map = unsafe { MmapMut::map_mut(file)? };
        Ok(Self { map, max_frames })
    }

    pub fn header(&self) -> &Header {
        // SAFETY: the mapping is page-aligned and at least a Header long, and
        // every field is an atomic, so shared references are fine.
        unsafe { &*(self.map.as_ptr() as *const Header) }
    }

    pub fn max_frames(&self) -> usize {
        self.max_frames
    }

    fn channel(&self, output: bool, ch: usize) -> *mut f32 {
        assert!(ch < CHANNELS);
        let i = (output as usize) * CHANNELS + ch;
        // SAFETY: in bounds by `size_for`; the header size is a multiple of 8.
        unsafe {
            (self.map.as_ptr().add(std::mem::size_of::<Header>()) as *mut f32)
                .add(i * self.max_frames)
        }
    }

    /// Copies `data` into input (or output) channel `ch`.
    pub fn write(&self, output: bool, ch: usize, data: &[f32]) {
        assert!(data.len() <= self.max_frames);
        // SAFETY: in bounds; only the side the protocol allows writes now.
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.channel(output, ch), data.len())
        }
    }

    /// Copies input (or output) channel `ch` into `data`.
    pub fn read(&self, output: bool, ch: usize, data: &mut [f32]) {
        assert!(data.len() <= self.max_frames);
        // SAFETY: as in `write`.
        unsafe {
            std::ptr::copy_nonoverlapping(self.channel(output, ch), data.as_mut_ptr(), data.len())
        }
    }
}

/// Stores an `f32` in an atomic as its bits.
pub fn store_f32(a: &AtomicU32, v: f32) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

pub fn load_f32(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}

pub fn store_f64(a: &AtomicU64, v: f64) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

pub fn load_f64(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Ordering::Relaxed))
}
