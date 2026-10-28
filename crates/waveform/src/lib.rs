//! GPU waveform rendering.
//!
//! Each [`WaveformView`] owns a texture. Every frame, [`WaveformView::render`]
//! draws the waveform into it with a fragment shader; the UI shows the same
//! texture as an image, so pixels never leave the GPU. A track's summary
//! (band peaks and RMS, at full resolution and at every coarser level) is
//! uploaded once as a [`GpuTrack`] that a deck's views share; per frame only
//! a small uniform block changes.
//!
//! This follows the approach of Mixxx's `allshader` renderers
//! (`src/waveform/renderers/allshader`), on wgpu instead of OpenGL.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
pub use wgpu;

pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How the waveform is coloured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
    /// One shape coloured by the balance of the bands (Mixxx's RGB).
    #[default]
    Rgb,
    /// Low, mid and high drawn as separate stacked shapes.
    Bands,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Zoomed-in view centred on the play position.
    Scrolling,
    /// The whole track, with the played part darkened.
    Overview,
}

/// What to draw this frame. All positions are in seconds.
#[derive(Clone, Debug)]
pub struct Frame {
    pub play_seconds: f64,
    /// Seconds of audio across the full width (scrolling mode only).
    pub visible_seconds: f64,
    pub duration: f64,
    /// The beat number of the track's first bar, numbered 1 on the scrolling
    /// waveform; a multiple of 4, as bar lines fall every fourth beat from 0.
    pub first_bar_beat: f64,
    /// The beat grid's sections around the view, earliest first.
    pub grid: [Option<GridSection>; GRID_SECTIONS],
    pub cue: Option<f64>,
    pub loop_range: Option<(f64, f64)>,
    /// Where a loop being marked out starts (after loop in, before loop
    /// out); drawn up to the play position.
    pub loop_marking: Option<f64>,
    pub hotcues: [Option<f64>; 8],
    pub gain: f32,
    /// Where each phrase starts and its kind, earliest first; empty to
    /// draw none. Drawn on the overview only, at most [`MAX_PHRASES`].
    pub phrases: Vec<(f64, PhraseKind)>,
}

/// Phrases the shader draws at once: far more than a track has, as a phrase
/// is at least 4 bars long and neighbours of one kind are merged.
pub const MAX_PHRASES: usize = 32;

/// A phrase's kind, which sets its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhraseKind {
    Intro,
    Build,
    Drop,
    Breakdown,
    Outro,
    Chorus,
}

/// Tempo sections the shader draws at once: enough for any view, since a
/// section is at least a few beats long.
pub const GRID_SECTIONS: usize = 4;

/// Where a stretch of steady beats starts, its tempo, and the number of its
/// first beat (beat 0 is a bar line, then every fourth).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSection {
    pub seconds: f64,
    pub bpm: f64,
    pub beat: f64,
}

/// A grid at one tempo, with a bar starting at `first_beat`; none for a
/// tempo of 0.
pub fn steady_grid(bpm: f64, first_beat: f64) -> [Option<GridSection>; GRID_SECTIONS] {
    let mut g = [None; GRID_SECTIONS];
    if bpm > 0.0 {
        g[0] = Some(GridSection {
            seconds: first_beat,
            bpm,
            beat: 0.0,
        });
    }
    g
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    size: [f32; 2],
    play_bin: f32,
    bins_per_pixel: f32,
    num_bins: u32,
    mode: u32,
    grid_count: u32,
    /// 1 while a loop is being marked out: loop_start_bin..loop_end_bin is
    /// drawn grey up to the play position.
    loop_marking: u32,
    cue_bin: f32,
    loop_start_bin: f32,
    loop_end_bin: f32,
    gain: f32,
    num_levels: u32,
    style: u32,
    scale: f32,
    /// How far the digit atlas's distance field reaches, in atlas pixels.
    spread: f32,
    low_color: [f32; 4],
    mid_color: [f32; 4],
    high_color: [f32; 4],
    background: [f32; 4],
    hotcues: [[f32; 4]; 2],
    grid_start: [f32; 4],
    grid_beat: [f32; 4],
    grid_index: [f32; 4],
    grid_bar: [f32; 4],
    /// The digit atlas: cell width and height, advance and digit height, in atlas pixels.
    digit: [f32; 4],
    level_offset: [[u32; 4]; 4],
    level_len: [[u32; 4]; 4],
    /// Where each phrase starts, in slices, and its kind (as in
    /// [`PhraseKind`]); `phrase_count` of them.
    phrase_start: [[f32; 4]; MAX_PHRASES / 4],
    phrase_kind: [[u32; 4]; MAX_PHRASES / 4],
    phrase_count: u32,
    /// Slices per minute, for the overview's minute dots; 0 for none.
    minute_bins: f32,
    _pad: [u32; 2],
}

/// Levels the shader can address; the coarsest is 2^15 slices per texel pair,
/// far more than any overview needs.
const MAX_LEVELS: usize = 16;

/// Band colours: bass red, mids amber, highs blue.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub low: [f32; 4],
    pub mid: [f32; 4],
    pub high: [f32; 4],
    pub background: [f32; 4],
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            low: [1.0, 0.22, 0.16, 1.0],
            mid: [1.0, 0.72, 0.12, 1.0],
            high: [0.30, 0.62, 1.0, 1.0],
            background: [0.05, 0.05, 0.055, 1.0],
        }
    }
}

/// Shader and pipeline, shared by all views on one device.
pub struct WaveformRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    max_rows: u32,
    digits: DigitAtlas,
}

impl WaveformRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waveform"),
            source: wgpu::ShaderSource::Wgsl(include_str!("waveform.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("waveform"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("waveform"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("waveform"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TEXTURE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            max_rows: device.limits().max_texture_dimension_2d,
            digits: DigitAtlas::new(device, queue),
        }
    }
}

/// A track's waveform summary on the GPU, shared by every view showing it.
pub struct GpuTrack {
    id: usize,
    texture: wgpu::Texture,
    /// Slices per second in the finest level uploaded.
    bins_per_second: f64,
    level_offset: [[u32; 4]; 4],
    level_len: [[u32; 4]; 4],
    num_levels: u32,
}

impl GpuTrack {
    /// Uploads `levels` (finest first, 8 bytes per slice: band peaks then band
    /// RMS). If the finest levels don't fit the device's texture limits, which
    /// only happens for tracks of an hour or more on small GPUs, they are
    /// skipped and the next level becomes the finest.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &WaveformRenderer,
        id: usize,
        levels: &[Vec<[u8; 8]>],
        bins_per_second: f64,
    ) -> Self {
        let capacity = BIN_ROW as usize * renderer.max_rows as usize;
        let texels = |from: usize| -> usize {
            levels[from..]
                .iter()
                .take(MAX_LEVELS)
                .map(|l| 2 * l.len())
                .sum()
        };
        let base = (0..levels.len())
            .find(|&b| texels(b) <= capacity)
            .unwrap_or(levels.len());
        let used = &levels[base..levels.len().min(base + MAX_LEVELS)];
        let mut level_offset = [[0u32; 4]; 4];
        let mut level_len = [[0u32; 4]; 4];
        let mut data: Vec<[u8; 4]> = Vec::with_capacity(texels(base));
        for (i, level) in used.iter().enumerate() {
            level_offset[i / 4][i % 4] = data.len() as u32;
            level_len[i / 4][i % 4] = level.len() as u32;
            for b in level {
                data.push([b[0], b[1], b[2], b[3]]);
                data.push([b[4], b[5], b[6], b[7]]);
            }
        }
        // Pad to whole rows so the upload is one rectangle.
        let rows = data.len().div_ceil(BIN_ROW as usize).max(1);
        data.resize(rows * BIN_ROW as usize, [0; 4]);
        let texture = bins_texture(device, rows as u32);
        queue.write_texture(
            texture.as_image_copy(),
            bytemuck::cast_slice(&data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(BIN_ROW * 4),
                rows_per_image: Some(rows as u32),
            },
            wgpu::Extent3d {
                width: BIN_ROW,
                height: rows as u32,
                depth_or_array_layers: 1,
            },
        );
        Self {
            id,
            texture,
            bins_per_second: bins_per_second / 2f64.powi(base as i32),
            level_offset,
            level_len,
            num_levels: used.len() as u32,
        }
    }

    /// The id passed to [`GpuTrack::new`], to tell whether a track is already uploaded.
    pub fn id(&self) -> usize {
        self.id
    }

    fn num_bins(&self) -> u32 {
        if self.num_levels == 0 {
            0
        } else {
            self.level_len[0][0]
        }
    }
}

/// One on-screen waveform: a texture the UI shows, drawn from a [`GpuTrack`].
pub struct WaveformView {
    mode: Mode,
    style: Style,
    palette: Palette,
    texture: wgpu::Texture,
    size: (u32, u32),
    params: wgpu::Buffer,
    track: Option<(Arc<GpuTrack>, wgpu::BindGroup)>,
    empty: wgpu::Texture,
}

impl WaveformView {
    pub fn new(device: &wgpu::Device, mode: Mode, width: u32, height: u32) -> Self {
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let empty = bins_texture(device, 1);
        Self {
            mode,
            style: Style::default(),
            palette: Palette::default(),
            texture: create_texture(device, width, height),
            size: (width.max(1), height.max(1)),
            params,
            track: None,
            empty,
        }
    }

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    /// The texture the UI displays. Cheap to clone (reference counted).
    pub fn texture(&self) -> wgpu::Texture {
        self.texture.clone()
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Recreates the texture at a new size. Returns true if it changed, in which
    /// case the UI must be handed the new [`texture`](Self::texture).
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let (w, h) = (width.clamp(1, 8192), height.clamp(1, 8192));
        if (w, h) == self.size {
            return false;
        }
        self.texture = create_texture(device, w, h);
        self.size = (w, h);
        true
    }

    /// Shows `track`, or nothing.
    pub fn set_track(
        &mut self,
        device: &wgpu::Device,
        renderer: &WaveformRenderer,
        track: Option<&Arc<GpuTrack>>,
    ) {
        match track {
            None => self.track = None,
            Some(t) => {
                if matches!(&self.track, Some((cur, _)) if Arc::ptr_eq(cur, t)) {
                    return;
                }
                let bind = bind_group(device, renderer, &self.params, &t.texture);
                self.track = Some((t.clone(), bind));
            }
        }
    }

    /// Draws one frame into the texture. `scale` is physical pixels per
    /// logical pixel, so lines and markers keep their size on HiDPI screens.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &WaveformRenderer,
        frame: Option<&Frame>,
        scale: f32,
    ) {
        let (w, h) = self.size;
        let track = self.track.as_ref().map(|(t, _)| t.as_ref());
        let num_bins = track.map_or(0, GpuTrack::num_bins);
        let bins_per_second = track.map_or(1.0, |t| t.bins_per_second);
        let to_bin = |s: f64| (s * bins_per_second) as f32;
        let mut params = Params {
            size: [w as f32, h as f32],
            play_bin: 0.0,
            bins_per_pixel: 1.0,
            num_bins,
            mode: if self.mode == Mode::Scrolling { 0 } else { 1 },
            grid_count: 0,
            loop_marking: 0,
            cue_bin: -1.0,
            loop_start_bin: 0.0,
            loop_end_bin: -1.0,
            gain: 1.0,
            num_levels: track.map_or(0, |t| t.num_levels),
            style: match self.style {
                Style::Rgb => 0,
                Style::Bands => 1,
            },
            scale,
            spread: DIGIT_SPREAD,
            low_color: self.palette.low,
            mid_color: self.palette.mid,
            high_color: self.palette.high,
            background: self.palette.background,
            hotcues: [[-1.0; 4]; 2],
            grid_start: [0.0; 4],
            grid_beat: [0.0; 4],
            grid_index: [0.0; 4],
            grid_bar: [0.0; 4],
            digit: renderer.digits.metrics,
            level_offset: track.map_or([[0; 4]; 4], |t| t.level_offset),
            level_len: track.map_or([[0; 4]; 4], |t| t.level_len),
            phrase_start: [[0.0; 4]; MAX_PHRASES / 4],
            phrase_kind: [[0; 4]; MAX_PHRASES / 4],
            phrase_count: 0,
            minute_bins: 0.0,
            _pad: [0; 2],
        };
        if let (Some(f), true) = (frame, num_bins > 0) {
            let bins_per_pixel = match self.mode {
                Mode::Scrolling => f.visible_seconds * bins_per_second / w as f64,
                Mode::Overview => num_bins as f64 / w as f64,
            };
            params.bins_per_pixel = bins_per_pixel as f32;
            let play_bin = f.play_seconds * bins_per_second;
            // Not rounded to whole pixels: the shader keeps each column on the
            // same slices and moves the picture smoothly between pixels.
            params.play_bin = play_bin as f32;
            for (k, section) in f.grid.iter().map_while(|g| g.as_ref()).enumerate() {
                if section.bpm <= 0.0 {
                    break;
                }
                params.grid_start[k] = to_bin(section.seconds);
                params.grid_beat[k] = to_bin(60.0 / section.bpm);
                // Only the bar position matters, which keeps the float exact.
                params.grid_index[k] = section.beat.rem_euclid(4.0) as f32;
                params.grid_bar[k] = ((section.beat - f.first_bar_beat) / 4.0).floor() as f32 + 1.0;
                params.grid_count = k as u32 + 1;
            }
            params.cue_bin = f.cue.map(to_bin).unwrap_or(-1.0);
            if let Some((a, b)) = f.loop_range {
                params.loop_start_bin = to_bin(a);
                params.loop_end_bin = to_bin(b);
            } else if let Some(a) = f.loop_marking {
                params.loop_start_bin = to_bin(a);
                params.loop_end_bin = play_bin as f32;
                params.loop_marking = 1;
            }
            params.gain = f.gain;
            for (i, hc) in f.hotcues.iter().enumerate() {
                params.hotcues[i / 4][i % 4] = hc.map(to_bin).unwrap_or(-1.0);
            }
            for (i, (seconds, kind)) in f.phrases.iter().take(MAX_PHRASES).enumerate() {
                params.phrase_start[i / 4][i % 4] = to_bin(*seconds);
                params.phrase_kind[i / 4][i % 4] = *kind as u32;
                params.phrase_count = i as u32 + 1;
            }
            if f.duration > 60.0 {
                params.minute_bins = to_bin(60.0);
            }
        }
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));

        let fallback;
        let bind = match &self.track {
            Some((_, bind)) => bind,
            None => {
                fallback = bind_group(device, renderer, &self.params, &self.empty);
                &fallback
            }
        };
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("waveform"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("waveform"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&renderer.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
    }
}

fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("waveform"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Bins per texture row; matches `ROW` in the shader. 2048 is the smallest
/// maximum texture width wgpu guarantees (WebGL2/GLES downlevel limits).
const BIN_ROW: u32 = 2048;

fn bins_texture(device: &wgpu::Device, rows: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("waveform bins"),
        size: wgpu::Extent3d {
            width: BIN_ROW,
            height: rows.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn bind_group(
    device: &wgpu::Device,
    renderer: &WaveformRenderer,
    params: &wgpu::Buffer,
    bins: &wgpu::Texture,
) -> wgpu::BindGroup {
    let view = bins.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("waveform"),
        layout: &renderer.layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&renderer.digits.view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&renderer.digits.sampler),
            },
        ],
    })
}

/// The font the bar numbers are drawn in (tabular, so every digit is as wide).
const DIGIT_FONT: &[u8] = include_bytes!("../../app/ui/fonts/InterTabular-SemiBold.otf");
/// The size the digits are rasterized at; the distance field keeps their
/// edges sharp at any size and position on screen.
const DIGIT_PX: f32 = 64.0;
/// How far the distance field reaches either side of an edge, in atlas pixels.
const DIGIT_SPREAD: f32 = 6.0;

/// The digits 0 to 9 side by side in one texture, as a signed distance
/// field: 0.5 on a glyph's edge, more inside, less outside.
struct DigitAtlas {
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Cell width and height, advance and digit height, in atlas pixels.
    metrics: [f32; 4],
}

impl DigitAtlas {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let (cell_w, cell_h, data, advance, height) = digit_field();
        let width = cell_w * 10;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("waveform digits"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: cell_h as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Rows padded to the 256 bytes a texture upload needs.
        let row = width.next_multiple_of(256);
        let mut padded = vec![0u8; row * cell_h];
        for y in 0..cell_h {
            padded[y * row..y * row + width].copy_from_slice(&data[y * width..(y + 1) * width]);
        }
        queue.write_texture(
            texture.as_image_copy(),
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row as u32),
                rows_per_image: Some(cell_h as u32),
            },
            wgpu::Extent3d {
                width: width as u32,
                height: cell_h as u32,
                depth_or_array_layers: 1,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("waveform digits"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            sampler,
            metrics: [cell_w as f32, cell_h as f32, advance, height],
        }
    }
}

/// Rasterizes the digits and turns them into a distance field: the cell
/// size, the field (rows of `10 * cell_w` bytes), the advance and the digit
/// height. Each cell has the digit's pen position `DIGIT_SPREAD` pixels in
/// from its left and its top `DIGIT_SPREAD` pixels down.
fn digit_field() -> (usize, usize, Vec<u8>, f32, f32) {
    use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
    let font = FontRef::try_from_slice(DIGIT_FONT).expect("digit font");
    let scaled = font.as_scaled(PxScale::from(DIGIT_PX));
    let glyphs: Vec<_> = ('0'..='9')
        .map(|c| {
            scaled.outline_glyph(
                scaled
                    .glyph_id(c)
                    .with_scale_and_position(DIGIT_PX, ab_glyph::point(0.0, scaled.ascent())),
            )
        })
        .collect();
    let advance = scaled.h_advance(scaled.glyph_id('0'));
    let bounds = glyphs.iter().flatten().map(|g| g.px_bounds());
    let (top, bottom) = bounds.fold((f32::MAX, f32::MIN), |(t, b), r| {
        (t.min(r.min.y), b.max(r.max.y))
    });
    let pad = DIGIT_SPREAD as usize;
    let cell_w = advance.ceil() as usize + 2 * pad;
    let cell_h = (bottom - top).ceil() as usize + 2 * pad;
    // Coverage first, digit by digit.
    let width = cell_w * 10;
    let mut cover = vec![0f32; width * cell_h];
    for (d, g) in glyphs.iter().enumerate() {
        let Some(g) = g else { continue };
        let r = g.px_bounds();
        g.draw(|x, y, c| {
            let cx = r.min.x + x as f32 + DIGIT_SPREAD;
            let cy = r.min.y - top + y as f32 + DIGIT_SPREAD;
            if cx >= 0.0 && (cx as usize) < cell_w && cy >= 0.0 && (cy as usize) < cell_h {
                cover[cy as usize * width + d * cell_w + cx as usize] = c;
            }
        });
    }
    // Then the distance to the nearest pixel on the other side of the edge,
    // within the spread, signed (inside positive) and mapped to 0..1.
    let reach = pad as i64;
    let mut field = vec![0u8; width * cell_h];
    for y in 0..cell_h as i64 {
        for x in 0..width as i64 {
            let inside = cover[y as usize * width + x as usize] >= 0.5;
            let mut best = DIGIT_SPREAD;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= width as i64 || ny >= cell_h as i64 {
                        continue;
                    }
                    if (cover[ny as usize * width + nx as usize] >= 0.5) != inside {
                        let dist = ((dx * dx + dy * dy) as f32).sqrt() - 0.5;
                        best = best.min(dist);
                    }
                }
            }
            let signed = if inside { best } else { -best };
            let v = 0.5 + signed / (2.0 * DIGIT_SPREAD);
            field[y as usize * width + x as usize] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    (cell_w, cell_h, field, advance, bottom - top)
}
