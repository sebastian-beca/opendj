// Waveform shader. One fragment = one pixel; the whole view is one triangle.
//
// The track's summary lives in a texture, uploaded once per track: for every
// slice two texels, band peaks (low, mid, high, all) then band RMS, each the
// square root of the level scaled to 0..1. Coarser levels (pairs of slices
// merged) follow the finest one, so any zoom reads at most a few texels per
// pixel. Each frame only the uniforms (play position, zoom, markers) change.

struct Params {
    size: vec2<f32>,          // target size in pixels
    play_bin: f32,            // play position, in finest-level slices
    bins_per_pixel: f32,
    num_bins: u32,            // slices in the finest level
    mode: u32,                // 0 = scrolling, 1 = overview
    grid_count: u32,          // tempo sections in grid_* (0 = no grid)
    loop_marking: u32,        // 1 = loop being marked out: grey, up to the play position
    cue_bin: f32,
    loop_start_bin: f32,
    loop_end_bin: f32,        // < loop_start = no loop
    gain: f32,
    num_levels: u32,
    style: u32,               // 0 = RGB (colour by frequency), 1 = stacked bands
    scale: f32,               // physical pixels per logical pixel
    spread: f32,              // how far the digit atlas's distance field reaches, in atlas pixels
    low_color: vec4<f32>,
    mid_color: vec4<f32>,
    high_color: vec4<f32>,
    background: vec4<f32>,
    hotcues: array<vec4<f32>, 2>,      // 8 hotcue positions in slices, < 0 = unset
    grid_start: vec4<f32>,             // where each tempo section starts, in slices
    grid_beat: vec4<f32>,              // its beat length in slices
    grid_index: vec4<f32>,             // its first beat's place in the bar (0 = bar line)
    grid_bar: vec4<f32>,               // the number of the bar its first beat is in (1 = the first)
    digit: vec4<f32>,                  // digit atlas: cell width, cell height, advance, digit height
    level_offset: array<vec4<u32>, 4>, // first texel of each level
    level_len: array<vec4<u32>, 4>,    // slices in each level
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var bins: texture_2d<f32>;
// Digits 0 to 9 side by side as a signed distance field (0.5 on the edge).
@group(0) @binding(2) var digits_atlas: texture_2d<f32>;
@group(0) @binding(3) var digits_sampler: sampler;
const ROW: u32 = 2048u;

// Hotcue colours, matching the pads in the skin.
const HOTCUE_COLORS = array<vec3<f32>, 8>(
    vec3<f32>(1.0, 0.27, 0.23),
    vec3<f32>(1.0, 0.62, 0.04),
    vec3<f32>(0.04, 0.52, 1.0),
    vec3<f32>(1.0, 0.84, 0.04),
    vec3<f32>(0.19, 0.82, 0.35),
    vec3<f32>(0.75, 0.35, 0.95),
    vec3<f32>(1.0, 0.22, 0.37),
    vec3<f32>(0.39, 0.82, 1.0),
);
const AMBER = vec3<f32>(0.91, 0.70, 0.23);
// A loop: Electric Lilac (#B69CFF); light grey while it is being marked out.
const LOOP_LILAC = vec3<f32>(0.714, 0.612, 1.0);
const LOOP_MARKING = vec3<f32>(0.82, 0.82, 0.84);
const CUE_ORANGE = vec3<f32>(1.0, 0.6, 0.1);

struct Slice {
    peak: vec4<f32>,
    rms: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn level_offset(l: u32) -> u32 {
    return p.level_offset[l / 4u][l % 4u];
}

fn level_len(l: u32) -> u32 {
    return p.level_len[l / 4u][l % 4u];
}

fn texel(t: u32) -> vec4<f32> {
    return textureLoad(bins, vec2<u32>(t % ROW, t / ROW), 0);
}

// Levels are stored as square roots; this returns them as linear levels.
fn load(l: u32, i: i32) -> Slice {
    if (i < 0 || u32(i) >= level_len(l)) {
        return Slice(vec4<f32>(0.0), vec4<f32>(0.0));
    }
    let t = level_offset(l) + 2u * u32(i);
    let pk = texel(t);
    let rm = texel(t + 1u);
    return Slice(pk * pk, rm * rm);
}

// The audio under the slice range [a, b) (in finest-level slices).
fn sample_range(a: f32, b: f32) -> Slice {
    let w = b - a;
    if (w <= 1.0) {
        // Zoomed in further than one slice per pixel: interpolate between
        // slice centres so the outline is smooth rather than stepped.
        let c = (a + b) * 0.5 - 0.5;
        let i = floor(c);
        let t = c - i;
        let s0 = load(0u, i32(i));
        let s1 = load(0u, i32(i) + 1);
        return Slice(mix(s0.peak, s1.peak, t), mix(s0.rms, s1.rms, t));
    }
    // Pick the level where a pixel spans one to two slices, then combine the
    // slices it touches, weighted by how much of each the pixel covers. The
    // weighting keeps peaks from popping as the waveform scrolls.
    let lvl = min(u32(floor(log2(w))), p.num_levels - 1u);
    let s = exp2(f32(lvl));
    let la = a / s;
    let lb = b / s;
    var peak = vec4<f32>(0.0);
    var power = vec4<f32>(0.0);
    var total = 0.0;
    let first = i32(floor(la));
    let last = min(i32(ceil(lb)) - 1, first + 63);
    for (var i = first; i <= last; i = i + 1) {
        let cover = clamp(min(lb, f32(i + 1)) - max(la, f32(i)), 0.0, 1.0);
        let sl = load(lvl, i);
        peak = max(peak, sl.peak * smoothstep(0.0, 0.5, cover));
        power = power + sl.rms * sl.rms * cover;
        total = total + cover;
    }
    return Slice(peak, sqrt(power / max(total, 1e-6)));
}

// The audio under the pixel at slice `bin`. Scrolling, columns are fixed to
// the track (column i covers slices i to i + 1 times bins_per_pixel), so each
// always shows the same slices and nothing shimmers, and a pixel blends the
// two columns it falls between, so the picture glides between pixels
// instead of stepping a whole pixel at a time.
fn column(bin: f32) -> Slice {
    let w = p.bins_per_pixel;
    if (p.mode != 0u) {
        return sample_range(bin - 0.5 * w, bin + 0.5 * w);
    }
    let c = bin / w - 0.5;
    let i = floor(c);
    let t = c - i;
    let s0 = sample_range(i * w, (i + 1.0) * w);
    let s1 = sample_range((i + 1.0) * w, (i + 2.0) * w);
    return Slice(mix(s0.peak, s1.peak, t), mix(s0.rms, s1.rms, t));
}

fn column_bin(x: f32) -> f32 {
    if (p.mode == 0u) {
        return p.play_bin + (x - p.size.x * 0.5) * p.bins_per_pixel;
    }
    return x * p.bins_per_pixel;
}

// Display height for a linear level: a gentle curve so quiet passages and
// breakdowns stay visible next to the drops.
fn height(v: f32) -> f32 {
    return clamp(pow(v, 0.8) * 0.94 * p.gain, 0.0, 1.0);
}

// Coverage of a pixel by a filled shape that reaches `edge` (pixels from the centre).
fn fill(edge: f32, d: f32) -> f32 {
    return clamp(edge - d + 0.5, 0.0, 1.0);
}

// Coverage of a pixel by a vertical line `half_w` pixels either side of `at`.
// On the overview, thin lines snap to the nearest pixel centre so they stay
// crisp instead of smearing across two half-lit columns; scrolling, they
// glide between pixels with the waveform.
fn line(x: f32, at: f32, half_w: f32) -> f32 {
    let c = select(at, floor(at) + 0.5, half_w <= 0.5 && p.mode == 1u);
    return clamp(half_w + 0.5 - abs(x - c), 0.0, 1.0);
}

// The x pixel of a slice position.
fn bin_x(bin: f32) -> f32 {
    if (p.mode == 0u) {
        return (bin - p.play_bin) / p.bins_per_pixel + p.size.x * 0.5;
    }
    return bin / p.bins_per_pixel;
}

fn over(base: vec3<f32>, c: vec3<f32>, a: f32) -> vec3<f32> {
    return mix(base, c, clamp(a, 0.0, 1.0));
}

// Colour of a column from the balance of its bands, like Mixxx's RGB waveform:
// bass-heavy is red, mids amber, hats and vocals blue.
fn band_color(s: Slice) -> vec3<f32> {
    let w = s.rms.xyz * s.rms.xyz + 1e-5;
    let c = (w.x * p.low_color.rgb + w.y * p.mid_color.rgb + w.z * p.high_color.rgb) / (w.x + w.y + w.z);
    // Normalise brightness so mixed colours stay as vivid as pure ones.
    return c / max(max(c.r, max(c.g, c.b)), 0.35);
}

// Coverage of pixel (x, y) by the number of the bar whose line is the last
// one at or before it: drawn just right of the line along the top. Drawn
// here rather than by the UI so it moves with the waveform; the digits come
// from a distance field, so they stay sharp between pixels.
fn bar_number(x: f32, y: f32, k: f32, sec: u32) -> f32 {
    let beat_len = p.grid_beat[sec];
    // Only where bars are far enough apart for their numbers.
    if (beat_len <= 0.0 || 4.0 * beat_len / p.bins_per_pixel < 28.0 * k) {
        return 0.0;
    }
    // Atlas pixels per screen pixel, for digits 8 points tall.
    let s = p.digit.w / (8.0 * k);
    let ay = (y - 3.0 * k) * s + p.spread;
    if (ay < 0.0 || ay > p.digit.y) {
        return 0.0;
    }
    let start = p.grid_start[sec];
    let n = floor((column_bin(x) - start) / beat_len);
    let index = i32(p.grid_index[sec]);
    let m = n - f32(((i32(n) + index) % 4 + 4) % 4);
    let at = start + m * beat_len;
    // The first section's bars run back to the start of the track; later
    // ones begin at their section.
    if (at < 0.0 || at >= f32(p.num_bins) || (sec > 0u && at < start - 0.001 * beat_len)) {
        return 0.0;
    }
    let bar = i32(p.grid_bar[sec]) + (i32(m) + index) / 4;
    if (bar < 1) {
        return 0.0;
    }
    let ax = (x - bin_x(at) - 3.0 * k) * s;
    if (ax < 0.0) {
        return 0.0;
    }
    var digits = 1;
    var place = 1;
    while (bar / (place * 10) > 0) {
        digits = digits + 1;
        place = place * 10;
    }
    let which = i32(floor(ax / p.digit.z));
    if (which >= digits) {
        return 0.0;
    }
    for (var i = 0; i < which; i = i + 1) {
        place = place / 10;
    }
    let digit = (bar / place) % 10;
    let gx = ax - f32(which) * p.digit.z + p.spread;
    let uv = vec2<f32>((f32(digit) * p.digit.x + gx) / (10.0 * p.digit.x), ay / p.digit.y);
    let field = textureSampleLevel(digits_atlas, digits_sampler, uv, 0.0).r;
    // The field's distance from the edge, in screen pixels.
    let dist = (field - 0.5) * 2.0 * p.spread / s;
    return clamp(dist + 0.5, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let x = frag.x;
    let y = frag.y;
    let k = max(p.scale, 1.0);
    let bin = column_bin(x);
    let half_h = p.size.y * 0.5;
    let d = abs(y - half_h) - 0.5;            // pixels from the centre line
    let span = half_h - 1.0;                  // tallest a peak can reach
    var color = p.background.rgb;

    // Loop region tint.
    let loop_color = select(LOOP_LILAC, LOOP_MARKING, p.loop_marking == 1u);
    if (p.loop_end_bin > p.loop_start_bin && bin >= p.loop_start_bin && bin <= p.loop_end_bin) {
        color = over(color, loop_color, 0.16);
    }

    let has_track = p.num_bins > 0u && bin >= 0.0 && bin < f32(p.num_bins);
    // The tempo section this pixel is in: the last one starting before it.
    var sec = 0u;
    for (var i = 1u; i < p.grid_count; i = i + 1u) {
        if (bin >= p.grid_start[i]) { sec = i; }
    }
    let beat_len = p.grid_beat[sec];
    let beats_visible = p.mode == 0u && p.grid_count > 0u && beat_len / p.bins_per_pixel > 4.0 * k;

    // Beat grid behind the waveform: faint beats, amber bars.
    var beat_a = 0.0;
    var bar_a = 0.0;
    if (beats_visible && has_track) {
        let n = round((bin - p.grid_start[sec]) / beat_len);
        let bx = bin_x(p.grid_start[sec] + n * beat_len);
        let in_bar = ((i32(n) + i32(p.grid_index[sec])) % 4 + 4) % 4;
        let is_bar = in_bar == 0;
        // The first section's beats run back to the start of the track.
        if (sec > 0u || p.grid_start[sec] + n * beat_len >= 0.0) {
            let a = line(x, bx, 0.5 * k);
            if (is_bar) { bar_a = a; } else { beat_a = a; }
        }
        color = over(color, vec3<f32>(1.0), beat_a * 0.10);
        color = over(color, AMBER, bar_a * 0.30);
    }

    // Faint centre line, so silence still reads as a track.
    if (has_track) {
        color = over(color, vec3<f32>(1.0), fill(0.5 * k, d) * 0.12);
    }

    // The waveform.
    if (has_track) {
        let s = column(bin);
        if (p.style == 0u) {
            let c = band_color(s);
            let peak_h = height(s.peak.w) * span;
            let rms_h = height(s.rms.w) * span;
            // Peaks as a dim translucent outline, the RMS body solid and
            // slightly brighter towards the centre.
            color = over(color, c * 0.75, fill(peak_h, d) * 0.4);
            let shade = 0.82 + 0.18 * clamp(1.0 - d / max(rms_h, 1.0), 0.0, 1.0);
            color = over(color, c * shade, fill(rms_h, d));
        } else {
            // Stacked bands: low widest at the back, each with its RMS body
            // solid and its peaks translucent.
            let lp = height(s.peak.x) * span;
            let mp = height(s.peak.y) * span * 0.85;
            let hp = height(s.peak.z) * span * 0.7;
            let lr = height(s.rms.x) * span;
            let mr = height(s.rms.y) * span * 0.85;
            let hr = height(s.rms.z) * span * 0.7;
            color = over(color, p.low_color.rgb, fill(lp, d) * 0.5);
            color = over(color, p.low_color.rgb, fill(lr, d) * 0.95);
            color = over(color, p.mid_color.rgb, fill(mp, d) * 0.5);
            color = over(color, p.mid_color.rgb, fill(mr, d) * 0.9);
            color = over(color, p.high_color.rgb, fill(hp, d) * 0.5);
            color = over(color, p.high_color.rgb, fill(hr, d) * 0.9);
        }
    }

    // Beat ticks along the top and bottom edges, over the waveform.
    if (beats_visible && has_track) {
        let edge = half_h - d;                // pixels from the nearer edge
        let tick = clamp(6.0 * k - edge + 0.5, 0.0, 1.0);
        color = over(color, vec3<f32>(0.85), beat_a * tick * 0.8);
        color = over(color, AMBER, bar_a * clamp(10.0 * k - edge + 0.5, 0.0, 1.0));
        color = over(color, vec3<f32>(1.0), bar_number(x, y, k, sec) * 0.85);
    }

    // Loop edges.
    if (p.loop_end_bin > p.loop_start_bin) {
        // No end edge while marking: the play position is the end.
        let end = select(line(x, bin_x(p.loop_end_bin), 0.75 * k), 0.0, p.loop_marking == 1u);
        let a = max(line(x, bin_x(p.loop_start_bin), 0.75 * k), end);
        color = over(color, loop_color, a * 0.9);
    }

    if (p.mode == 1u) {
        // Overview: darken what has been played.
        if (bin < p.play_bin) {
            color = color * 0.45;
        }
    }

    // Cue point: a line with flags at both edges.
    if (p.cue_bin >= 0.0) {
        let cx = bin_x(p.cue_bin);
        let edge = half_h - d;
        let flag = clamp(5.0 * k - abs(x - cx) - edge * 0.8 + 0.5, 0.0, 1.0);
        color = over(color, CUE_ORANGE, max(line(x, cx, 0.5 * k) * 0.8, flag));
    }

    // Hotcues: a coloured line with a flag at the top.
    for (var h = 0; h < 8; h = h + 1) {
        let pos = p.hotcues[h / 4][h % 4];
        if (pos >= 0.0) {
            let hx = bin_x(pos);
            let in_flag = x >= hx - 0.5 && x <= hx + 7.0 * k && y <= 7.0 * k;
            let a = max(line(x, hx, 0.5 * k), select(0.0, 1.0, in_flag));
            color = over(color, HOTCUE_COLORS[h], a * 0.95);
        }
    }

    // Play position.
    if (p.mode == 1u) {
        color = over(color, vec3<f32>(1.0), line(x, bin_x(p.play_bin), 0.75 * k));
    } else {
        let cx = p.size.x * 0.5;
        // White with a soft shadow either side, readable over any colour.
        color = over(color, vec3<f32>(0.0), clamp(1.0 - (abs(x - cx) - k) / (3.0 * k), 0.0, 1.0) * 0.35);
        color = over(color, vec3<f32>(1.0), line(x, cx, 1.0 * k));
    }

    return vec4<f32>(color, 1.0);
}
