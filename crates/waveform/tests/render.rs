//! Renders both waveform modes offscreen and checks the shader validates and draws.
//! Needs a Vulkan driver (a software one such as lavapipe is fine); skips otherwise.

use std::sync::Arc;

use opendj_waveform::{
    steady_grid, wgpu, Frame, GpuTrack, Mode, Palette, PhraseKind, Style, WaveformRenderer,
    WaveformView,
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
        for style in [Style::Rgb, Style::Bands, Style::Blue] {
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
                slip: None,
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
        slip: None,
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
        slip: None,
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
        slip: None,
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
        slip: None,
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

/// The Mono style, the Player's scrubber: lilac up to the play position,
/// grey after it, and a dim lilac across the next mix.
#[test]
fn draws_mono_played_rest_and_mix() {
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
        &levels(vec![[200, 200, 200, 230, 200, 200, 200, 230]; 200_000]),
        1000.0,
    ));
    // 512 wide: rows read back must be a multiple of 256 bytes.
    let (w, h) = (512u32, 28u32);
    let mut view = WaveformView::new(&device, Mode::Overview, w, h);
    view.set_style(Style::Mono);
    view.set_mix(Some((0.8, 0.95)));
    view.set_track(&device, &renderer, Some(&track));
    let frame = Frame {
        play_seconds: 50.0,
        visible_seconds: 0.0,
        duration: 200.0,
        first_bar_beat: 0.0,
        grid: steady_grid(120.0, 0.0),
        cue: Some(10.0),
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        slip: None,
        gain: 1.0,
        phrases: Vec::new(),
    };
    view.render(&device, &queue, &renderer, Some(&frame), 1.0);
    let data = read_back(&device, &queue, &view);
    let px = |x: u32| {
        let i = (((h / 2) * w + x) * 4) as usize;
        [data[i], data[i + 1], data[i + 2]]
    };
    // Played (the first quarter): lilac, blue well above red-ish grey.
    let played = px(64);
    assert!(
        played[2] > 220 && played[0] > 160,
        "played not lilac: {played:?}"
    );
    // The rest: grey, the three channels close together.
    let rest = px(256);
    assert!(
        rest[0].abs_diff(rest[2]) < 16 && rest[0] < 130,
        "rest not grey: {rest:?}"
    );
    // The mix zone, at 85 %: dim lilac, blue above red.
    let mix = px(435);
    assert!(
        mix[2] > mix[0] + 30 && mix[2] < 180,
        "mix not dim lilac: {mix:?}"
    );
    // No cue line in Mono: the column at 10 s matches one beside it.
    let cue_x = 512 * 10 / 200;
    assert_eq!(px(cue_x), px(cue_x + 8));
}

/// A live deck: the grid runs on past the play line, and lanes of marks are
/// drawn over a fainter waveform.
#[test]
fn draws_live_lanes() {
    use opendj_waveform::{Lanes, Mark, MarkKind};
    let Some((device, queue)) = gpu() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let renderer = WaveformRenderer::new(&device, &queue);
    // 8 s of silence ending at the play line.
    let track = Arc::new(GpuTrack::new(
        &device,
        &queue,
        &renderer,
        1,
        &levels(vec![[0; 8]; 8192]),
        1000.0,
    ));
    // A fixed window rewrites in place; a different length doesn't fit.
    assert!(track.rewrite(&queue, &levels(vec![[9; 8]; 8192])));
    assert!(!track.rewrite(&queue, &levels(vec![[9; 8]; 4096])));
    let (w, h) = (512u32, 40u32);
    let mut view = WaveformView::new(&device, Mode::Scrolling, w, h);
    view.set_track(&device, &renderer, Some(&track));
    view.set_live(true);
    view.set_lanes(Some(Lanes {
        count: 4,
        tinted: 0,
        // One note let go on the bottom row, one held on the top row up to
        // the play line, one coming hit past it.
        marks: vec![
            Mark {
                start: 6.0,
                end: 7.0,
                lane: 0,
                kind: MarkKind::Played,
            },
            Mark {
                start: 7.5,
                end: 8.192,
                lane: 3,
                kind: MarkKind::Held,
            },
            Mark {
                start: 9.0,
                end: 9.1,
                lane: 1,
                kind: MarkKind::Fill,
            },
        ],
        trace: 0.25,
        color: [1.0, 0.0, 0.0, 1.0],
        lit: [0.0, 0.0, 1.0, 1.0],
    }));
    let frame = Frame {
        play_seconds: 8.192,
        visible_seconds: 12.8,
        duration: 8.192,
        first_bar_beat: 0.0,
        grid: steady_grid(120.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        slip: None,
        gain: 1.0,
        phrases: Vec::new(),
    };
    view.render(&device, &queue, &renderer, Some(&frame), 1.0);
    let data = read_back(&device, &queue, &view);
    // 40 px per second; the play line at x 256.
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [data[i], data[i + 1], data[i + 2]]
    };
    // The played note, 1.8 s back (clear of the beat lines), on the bottom row (y 30..40): red.
    let p = px(256 - 72, 35);
    assert!(p[0] > 150 && p[2] < 80, "played note: {p:?}");
    // The held note, 0.4 s back on the top row (y 0..10), is lit inside.
    let p = px(256 - 16, 5);
    assert!(p[0] > 150 && p[1] > 50, "held note: {p:?}");
    // The fill, 0.85 s ahead on row 1 (y 20..30): blue.
    let p = px(256 + 34, 25);
    assert!(p[2] > 150 && p[0] < 80, "fill: {p:?}");
}

#[test]
fn draws_knob_levels() {
    use opendj_waveform::{Lanes, Mark, MarkKind};
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
        &levels(vec![[0; 8]; 8192]),
        1000.0,
    ));
    let (w, h) = (512u32, 40u32);
    let mut view = WaveformView::new(&device, Mode::Scrolling, w, h);
    view.set_track(&device, &renderer, Some(&track));
    view.set_live(true);
    // One lane: a knob at half way up in the marks' colour, then rising in
    // colour 4 (green, #7BE495).
    view.set_lanes(Some(Lanes {
        count: 1,
        tinted: 0,
        marks: vec![
            Mark {
                start: 6.0,
                end: 7.5,
                lane: 0,
                kind: MarkKind::Level {
                    from: 0.5,
                    to: 0.5,
                    color: 0,
                },
            },
            Mark {
                start: 7.5,
                end: 8.192,
                lane: 0,
                kind: MarkKind::Level {
                    from: 0.5,
                    to: 1.0,
                    color: 4,
                },
            },
        ],
        trace: 0.25,
        color: [1.0, 0.0, 0.0, 1.0],
        lit: [0.0, 0.0, 1.0, 1.0],
    }));
    let frame = Frame {
        play_seconds: 8.192,
        visible_seconds: 12.8,
        duration: 8.192,
        first_bar_beat: 0.0,
        grid: steady_grid(120.0, 0.0),
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        slip: None,
        gain: 1.0,
        phrases: Vec::new(),
    };
    view.render(&device, &queue, &renderer, Some(&frame), 1.0);
    let data = read_back(&device, &queue, &view);
    // How red a pixel is (bar numbers along the top are grey).
    let red = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        data[i].saturating_sub(data[i + 1])
    };
    // 2 s back (clear of the beat lines): the line half way up, a faint
    // fill under it, nothing over it.
    let line = (0..h).max_by_key(|&y| red(176, y)).unwrap();
    assert!((18..=22).contains(&line), "line at y {line}");
    let (on, under, over) = (red(176, line), red(176, 32), red(176, 14));
    assert!(on > 150, "line: {on}");
    assert!(under > over + 20 && under < on, "fill {under} over {over}");
    // Rising after 7.5 s: higher up near the play line, in green.
    let green = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        data[i + 1].saturating_sub(data[i + 2] / 2 + data[i] / 2)
    };
    let rising = (0..h).max_by_key(|&y| green(250, y)).unwrap();
    assert!(
        green(250, rising) > 50,
        "green line: {:?}",
        &data[((rising * w + 250) * 4) as usize..][..3]
    );
    assert!(rising < line - 5, "rising line at y {rising}");
    // Nothing past the play line.
    assert!(
        red(300, line) < 20,
        "past the play line: {}",
        red(300, line)
    );
}

/// The scrolling waveform flags each phrase start: a line in its colour
/// down the waveform and a tab with its name under the bar numbers; a live
/// deck adds a strip along the top. Too short for tabs, only the lines.
#[test]
fn draws_phrase_flags() {
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
    let (w, h) = (1024u32, 120u32);
    let frame = Frame {
        play_seconds: 50.0,
        visible_seconds: 8.0,
        duration: 100.0,
        first_bar_beat: 0.0,
        grid: [None; opendj_waveform::GRID_SECTIONS],
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        slip: None,
        gain: 1.0,
        // The drop starts at x = 768 (2 s right of the middle, 128 px a second).
        phrases: vec![(40.0, PhraseKind::Build), (52.0, PhraseKind::Drop)],
    };
    let draw = |view: &mut WaveformView, frame: &Frame| {
        view.set_track(&device, &renderer, Some(&track));
        view.render(&device, &queue, &renderer, Some(frame), 1.0);
        let data = read_back(&device, &queue, view);
        let w = view.size().0;
        move |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            [data[i], data[i + 1], data[i + 2]]
        }
    };
    let mut view = WaveformView::new(&device, Mode::Scrolling, w, h);
    let px = draw(&mut view, &frame);
    // The line, rose, from top to bottom.
    for y in [2, 60, 117] {
        let c = px(768, y);
        assert!(c[0] > 120 && c[0] > c[2] + 40, "drop line at y {y}: {c:?}");
    }
    // The tab: rose behind white letters, rows 15 to 28.
    let tab: Vec<[u8; 3]> = (768..808).map(|x| px(x, 21)).collect();
    assert!(
        tab.iter().any(|c| c[0] > 150 && c[1] < 120),
        "rose tab: {tab:?}"
    );
    assert!(
        tab.iter().any(|c| c.iter().all(|&v| v > 200)),
        "white letters on the tab: {tab:?}"
    );
    // Nothing between flags: below the tab, past the line, the background
    // (off the silent track's centre line).
    let empty = px(820, 40);
    assert!(empty.iter().all(|&v| v < 40), "no band: {empty:?}");
    // No strip along the top of a track's waveform.
    let top = px(600, 1);
    assert!(top.iter().all(|&v| v < 40), "no strip on a track: {top:?}");

    // A live deck: the build's colour along the top before the drop, the
    // drop's after it.
    view.set_live(true);
    let px = draw(&mut view, &frame);
    let build = px(600, 1);
    let drop = px(900, 1);
    assert!(
        build[2] > 150 && build[0] < build[2],
        "build strip: {build:?}"
    );
    assert!(drop[0] > 150 && drop[2] < drop[0], "drop strip: {drop:?}");

    // Too short for tabs (4-Deck): the line, no tab.
    let mut thin = WaveformView::new(&device, Mode::Scrolling, w, 36);
    let px = draw(&mut thin, &frame);
    assert!(px(768, 18)[0] > 120, "line on a thin waveform");
    let beside = px(790, 20);
    assert!(beside.iter().all(|&v| v < 40), "no tab: {beside:?}");
}

/// A half overview grows from the bottom edge: the top rows stay empty for a
/// quiet track that a full overview draws round the middle, and the band
/// palettes and the Blue style draw.
#[test]
fn draws_half_overview_from_the_bottom() {
    let Some((device, queue)) = gpu() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let renderer = WaveformRenderer::new(&device, &queue);
    // Quiet: about a third of the height.
    let base = (0..10_000)
        .map(|_| [40, 40, 40, 40, 30, 30, 30, 30])
        .collect();
    let track = Arc::new(GpuTrack::new(
        &device,
        &queue,
        &renderer,
        1,
        &levels(base),
        1000.0,
    ));
    let (w, h) = (200u32, 60u32);
    let lit = |px: &[u8], y: u32| -> bool {
        let row = &px[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
        row.chunks(4)
            .filter(|c| c[0] as u32 + c[1] as u32 + c[2] as u32 > 120)
            .count()
            > w as usize / 2
    };
    for style in [Style::Bands, Style::Blue] {
        let mut full = WaveformView::new(&device, Mode::Overview, w, h);
        full.set_style(style);
        full.set_palette(Palette::bands(1));
        full.set_track(&device, &renderer, Some(&track));
        full.render(&device, &queue, &renderer, None, 1.0);
        let mut half = WaveformView::new(&device, Mode::Overview, w, h);
        half.set_style(style);
        half.set_palette(Palette::bands(1));
        half.set_half(true);
        half.set_track(&device, &renderer, Some(&track));
        half.render(&device, &queue, &renderer, None, 1.0);
        let full_px = read_back(&device, &queue, &full);
        let half_px = read_back(&device, &queue, &half);
        // Full: the middle is drawn, the bottom edge is not.
        assert!(lit(&full_px, h / 2), "{style:?} full middle");
        assert!(!lit(&full_px, h - 2), "{style:?} full bottom");
        // Half: the bottom is drawn, the middle and top are not.
        assert!(lit(&half_px, h - 3), "{style:?} half bottom");
        assert!(!lit(&half_px, 3), "{style:?} half top");
    }
}
