//! Renders both waveform modes offscreen and checks the shader validates and draws.
//! Needs a Vulkan driver (a software one such as lavapipe is fine); skips otherwise.

use std::sync::Arc;

use mixxx_waveform::{
    steady_grid, wgpu, Frame, GpuTrack, Mode, PhraseKind, Style, WaveformRenderer, WaveformView,
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
    let renderer = WaveformRenderer::new(&device, &queue);
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
                first_bar_beat: 0.0,
                grid: steady_grid(124.0, 0.1),
                cue: Some(10.0),
                loop_range: Some((28.0, 32.0)),
                loop_marking: None,
                hotcues: [Some(5.0), None, None, None, None, None, None, Some(60.0)],
                gain: 1.0,
                phrases: vec![
                    (0.0, PhraseKind::Intro),
                    (20.0, PhraseKind::Drop),
                    (60.0, PhraseKind::Outro),
                ],
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
    let renderer = WaveformRenderer::new(&device, &queue);
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
        first_bar_beat: 0.0,
        grid: steady_grid(0.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        gain: 1.0,
        phrases: Vec::new(),
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
    let renderer = WaveformRenderer::new(&device, &queue);
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
        // A quarter beat past a bar, so the playhead doesn't cover a bar line,
        // less half a pixel so the lines fall on pixel centres (scrolling,
        // lines are not snapped to pixels).
        play_seconds: 40.25 - 0.5 * 8.0 / 640.0,
        visible_seconds: 8.0,
        duration: 200.0,
        first_bar_beat: 0.0,
        grid: steady_grid(120.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        gain: 1.0,
        phrases: Vec::new(),
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

/// Bar numbers are drawn right of each bar line at the top: bar 21 at 40 s
/// (beat 80) at 120 bpm with the first bar at 0 s.
#[test]
fn draws_bar_numbers() {
    let Some((device, queue)) = gpu() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let renderer = WaveformRenderer::new(&device, &queue);
    let track = Arc::new(GpuTrack::new(
        &device,
        &queue,
        &renderer,
        1,
        &levels(vec![[0; 8]; 200_000]),
        1000.0,
    ));
    let (w, h) = (640u32, 64u32);
    let mut view = WaveformView::new(&device, Mode::Scrolling, w, h);
    view.set_track(&device, &renderer, Some(&track));
    let frame = Frame {
        // 2 s before the bar at 40 s, so its line is three quarters across.
        play_seconds: 38.0,
        visible_seconds: 8.0,
        duration: 200.0,
        first_bar_beat: 0.0,
        grid: steady_grid(120.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        gain: 1.0,
        phrases: Vec::new(),
    };
    view.render(&device, &queue, &renderer, Some(&frame), 1.0);
    let data = read_back(&device, &queue, &view);
    // White pixels in the digits' rows (below the beat ticks), right of the
    // line at x = 480: "21" is two digits, 11 columns of 2 pixels, from x = 483.
    let white = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        data[i] > 180 && data[i + 1] > 180 && data[i + 2] > 180
    };
    let lit: Vec<u32> = (470..520)
        .filter(|&x| (8..16).any(|y| white(x, y)))
        .collect();
    assert!(!lit.is_empty(), "no bar number drawn");
    assert!(
        *lit.first().unwrap() >= 482 && *lit.last().unwrap() <= 505,
        "bar number out of place: {lit:?}"
    );
    // Below the beat ticks, nothing between one bar's number and the next
    // bar line (bars are 160 pixels apart; the playhead is at 320).
    let stray: Vec<(u32, u32)> = (350..470)
        .flat_map(|x| (8..16).map(move |y| (x, y)))
        .filter(|&(x, y)| white(x, y))
        .collect();
    assert!(stray.is_empty(), "stray pixels: {stray:?}");
}

/// Phrases show as a bar along the bottom of the overview in their colours:
/// deep blue for the intro, rose for the drop, with the minute dots above it.
/// Each starts exactly on its bar, with the gap at the end of the one before.
#[test]
fn draws_phrase_bars() {
    let Some((device, queue)) = gpu() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let renderer = WaveformRenderer::new(&device, &queue);
    let track = Arc::new(GpuTrack::new(
        &device,
        &queue,
        &renderer,
        1,
        &levels(vec![[0; 8]; 100_000]),
        1000.0,
    ));
    let (w, h) = (512u32, 40u32);
    let mut view = WaveformView::new(&device, Mode::Overview, w, h);
    view.set_track(&device, &renderer, Some(&track));
    let frame = Frame {
        play_seconds: 0.0,
        visible_seconds: 8.0,
        duration: 100.0,
        first_bar_beat: 0.0,
        grid: steady_grid(120.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        gain: 1.0,
        phrases: vec![(0.0, PhraseKind::Intro), (50.0, PhraseKind::Drop)],
    };
    view.render(&device, &queue, &renderer, Some(&frame), 1.0);
    let data = read_back(&device, &queue, &view);
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [data[i], data[i + 1], data[i + 2]]
    };
    let intro = px(128, h - 2);
    let drop = px(384, h - 2);
    assert!(
        drop[0] > 150 && drop[0] > drop[2] + 60,
        "drop is rose: {drop:?}"
    );
    assert!(
        intro[2] > intro[0] + 50 && intro[0] < 60,
        "intro is a deep blue: {intro:?}"
    );
    // The drop starts right on its bar; the gap comes out of the intro's end.
    for x in [254, 255] {
        let gap = px(x, h - 2);
        assert!(gap[0] < drop[0] / 2, "gap before the drop at {x}: {gap:?}");
    }
    let start = px(256, h - 2);
    assert!(
        start[0] > 150,
        "drop colour from its first pixel: {start:?}"
    );
    let above = px(384, h - 10);
    assert!(above[2] < 40, "nothing above the bar but dots: {above:?}");
    // The minute dot at 60 s sits between the waveform and the bar.
    let x60 = (60.0 / 100.0 * w as f64) as u32;
    let dot = (0..4).map(|dx| px(x60 - 2 + dx, h - 9)[0]).max().unwrap();
    assert!(dot > 120, "minute dot above the bar: {dot}");
}
