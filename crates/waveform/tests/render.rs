//! Renders both waveform modes offscreen and checks the shader validates and draws.
//! Needs a Vulkan driver (a software one such as lavapipe is fine); skips otherwise.

use std::sync::Arc;

use mixxx_waveform::{
    steady_grid, wgpu, Frame, GpuTrack, Mode, Style, WaveformRenderer, WaveformView,
};

/// Builds the coarser levels the way the analyser does (peaks by maximum;
/// RMS approximated by maximum too, which is enough here).
fn levels(base: Vec<[u8; 8]>) -> Vec<Vec<[u8; 8]>> {
    let mut levels = vec![base];
    while levels.last().unwrap().len() > 1 {
        let next = levels
            .last()
            .unwrap()
            .chunks(2)
            .map(|c| std::array::from_fn(|k| c.iter().map(|b| b[k]).max().unwrap()))
            .collect();
        levels.push(next);
    }
    levels
}

fn gpu() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
        ..Default::default()
    }))
    .unwrap();
    device.on_uncaptured_error(Arc::new(|e| panic!("wgpu error: {e}")));
    Some((device, queue))
}

/// Copies a view's pixels back to the CPU (RGBA rows).
fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, view: &WaveformView) -> Vec<u8> {
    let (w, h) = view.size();
    let bytes_per_row = w * 4;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (bytes_per_row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        view.texture().as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap().to_vec();
    data
}

#[test]
fn renders_scrolling_and_overview() {
    let Some((device, queue)) = gpu() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let renderer = WaveformRenderer::new(&device);
    let base = (0..100_000)
        .map(|i| [(i % 255) as u8, 120, 60, 200, (i % 200) as u8, 80, 40, 150])
        .collect();
    let track = Arc::new(GpuTrack::new(
        &device,
        &queue,
        &renderer,
        1,
        &levels(base),
        1000.0,
    ));
    for mode in [Mode::Scrolling, Mode::Overview] {
        for style in [Style::Rgb, Style::Bands] {
            let mut view = WaveformView::new(&device, mode, 640, 96);
            view.set_style(style);
            view.set_track(&device, &renderer, Some(&track));
            let frame = Frame {
                play_seconds: 30.0,
                visible_seconds: 8.0,
                duration: 100.0,
                grid: steady_grid(124.0, 0.1),
                cue: Some(10.0),
                loop_range: Some((28.0, 32.0)),
                loop_marking: None,
                hotcues: [Some(5.0), None, None, None, None, None, None, Some(60.0)],
                gain: 1.0,
            };
            view.render(&device, &queue, &renderer, Some(&frame), 2.0);
            let marking = Frame {
                loop_range: None,
                loop_marking: Some(28.0),
                ..frame
            };
            view.render(&device, &queue, &renderer, Some(&marking), 2.0);
            view.render(&device, &queue, &renderer, None, 1.0);
        }
    }
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

/// An hour-long mix doesn't fit at full resolution within the smallest
/// texture limits; the finest level is dropped and it still draws.
#[test]
fn long_tracks_fall_back_to_a_coarser_level() {
    let Some((device, queue)) = gpu() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let renderer = WaveformRenderer::new(&device);
    let seconds = 3600.0;
    let base = vec![[200u8, 100, 50, 220, 150, 70, 30, 160]; (seconds * 1000.0) as usize];
    let track = Arc::new(GpuTrack::new(
        &device,
        &queue,
        &renderer,
        1,
        &levels(base),
        1000.0,
    ));
    let mut view = WaveformView::new(&device, Mode::Scrolling, 320, 64);
    view.set_track(&device, &renderer, Some(&track));
    let frame = Frame {
        play_seconds: 3000.0,
        visible_seconds: 8.0,
        duration: seconds,
        grid: steady_grid(0.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        gain: 1.0,
    };
    view.render(&device, &queue, &renderer, Some(&frame), 1.0);
    let data = read_back(&device, &queue, &view);
    // Centre row, a quarter of the way across: inside the waveform body.
    let i = (32 * 320 + 80) * 4;
    assert!(
        data[i] > 100,
        "waveform drawn at 50 minutes: {:?}",
        &data[i..i + 4]
    );
}

/// Reads back one row of the scrolling view and checks the beat grid lines are drawn.
#[test]
fn draws_beat_grid() {
    let Some((device, queue)) = gpu() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let renderer = WaveformRenderer::new(&device);
    // Silent track, so only the grid and markers show.
    let track = Arc::new(GpuTrack::new(
        &device,
        &queue,
        &renderer,
        1,
        &levels(vec![[0; 8]; 200_000]),
        1000.0,
    ));
    let (w, h) = (640u32, 32u32);
    let mut view = WaveformView::new(&device, Mode::Scrolling, w, h);
    view.set_track(&device, &renderer, Some(&track));
    let frame = Frame {
        // A quarter beat past a bar, so the playhead doesn't cover a bar line.
        play_seconds: 40.25,
        visible_seconds: 8.0,
        duration: 200.0,
        grid: steady_grid(120.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        gain: 1.0,
    };
    view.render(&device, &queue, &renderer, Some(&frame), 1.0);
    let data = read_back(&device, &queue, &view);
    let bytes_per_row = w * 4;
    let row = &data[(bytes_per_row * 2) as usize..(bytes_per_row * 3) as usize];
    // 8 s visible at 120 bpm is 16 beats, so 4 bars; count amber columns (red high, blue low).
    let amber = (0..w as usize)
        .filter(|&x| row[x * 4] > 150 && row[x * 4 + 2] < 120)
        .collect::<Vec<_>>();
    let mut lines = 0;
    for (i, x) in amber.iter().enumerate() {
        if i == 0 || *x > amber[i - 1] + 1 {
            lines += 1;
        }
    }
    assert!(
        (4..=5).contains(&lines),
        "expected 4 or 5 bar lines, got {lines} at {amber:?}"
    );
}
