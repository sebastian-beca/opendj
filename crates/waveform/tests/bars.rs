use mixxx_waveform::{bar_labels, steady_grid, BarLabel, Frame, GridSection};

fn frame(play_seconds: f64, grid: [Option<GridSection>; 4]) -> Frame {
    Frame {
        play_seconds,
        visible_seconds: 10.0,
        duration: 300.0,
        grid,
        cue: None,
        loop_range: None,
        loop_marking: None,
        hotcues: [None; 8],
        gain: 1.0,
    }
}

#[test]
fn bars_are_numbered_from_one() {
    // 120 BPM: a bar every 2 s, the first at 1 s.
    let f = frame(5.0, steady_grid(120.0, 1.0));
    let bars = bar_labels(&f, 1000.0, 0.0, 20.0);
    assert_eq!(
        bars,
        vec![
            BarLabel { x: 0.1, bar: 1 },
            BarLabel { x: 0.3, bar: 2 },
            BarLabel { x: 0.5, bar: 3 },
            BarLabel { x: 0.7, bar: 4 },
            BarLabel { x: 0.9, bar: 5 },
        ]
    );
}

#[test]
fn bars_before_the_first_get_no_number() {
    // The grid's beat 0 is 5 s in, but two whole bars fit before it, from
    // beat -8 at 1 s: that is bar 1. The one before, at -1 s, is not shown.
    let f = frame(5.0, steady_grid(120.0, 5.0));
    let bars = bar_labels(&f, 1000.0, -8.0, 20.0);
    let numbers: Vec<i64> = bars.iter().map(|b| b.bar).collect();
    assert_eq!(numbers, vec![1, 2, 3, 4, 5]);
    assert!((bars[0].x - 0.1).abs() < 1e-9);
}

#[test]
fn numbers_carry_on_across_tempo_changes() {
    let mut grid = steady_grid(120.0, 0.0);
    // At 8 s (beat 16, bar 5) the tempo halves: bars every 4 s.
    grid[1] = Some(GridSection {
        seconds: 8.0,
        bpm: 60.0,
        beat: 16.0,
    });
    let f = frame(10.0, grid);
    let numbers: Vec<i64> = bar_labels(&f, 1000.0, 0.0, 20.0)
        .iter()
        .map(|b| b.bar)
        .collect();
    // Bar 4 at 6 s, bar 5 at 8 s, bar 6 at 12 s.
    assert_eq!(numbers, vec![4, 5, 6]);
}

#[test]
fn crowded_bars_are_not_numbered() {
    let f = frame(5.0, steady_grid(120.0, 1.0));
    assert!(bar_labels(&f, 50.0, 0.0, 20.0).is_empty());
}
