use std::cell::Cell;

use super::*;

/// A capture ring stand-in: sample i is i as f32 / 1000; it keeps the last `window` samples.
struct Ring {
    written: Cell<u64>,
    window: u64,
}

impl SampleSource for Ring {
    fn rate(&self) -> u32 {
        1000
    }
    fn written(&self) -> u64 {
        self.written.get()
    }
    fn read(&self, start: u64, out: &mut [f32]) -> bool {
        let w = self.written.get();
        if start + out.len() as u64 > w || start + self.window < w {
            return false;
        }
        for (i, o) in out.iter_mut().enumerate() {
            *o = (start + i as u64) as f32 / 1000.0;
        }
        true
    }
    fn latest(&self, _: &mut [f32]) -> bool {
        false
    }
}

#[test]
fn the_take_is_copied_frame_by_frame_without_gaps() {
    let ring = Ring {
        written: Cell::new(500),
        window: 400,
    };
    let mut cap = Capture::new(500, 1000, 10.0);
    assert_eq!(cap.pull(&ring), 0, "nothing new yet");
    for k in 1..=5u64 {
        ring.written.set(500 + 37 * k);
        assert_eq!(cap.pull(&ring), 37);
    }
    assert_eq!(cap.samples.len(), 185);
    assert_eq!(cap.lost, 0);
    // every sample once, in order, from the take's start
    for (i, s) in cap.samples.iter().enumerate() {
        assert_eq!(*s, (500 + i) as f32 / 1000.0, "{i}");
    }
}

#[test]
fn a_frame_too_long_for_the_ring_leaves_silence_not_a_shift() {
    let ring = Ring {
        written: Cell::new(0),
        window: 100,
    };
    let mut cap = Capture::new(0, 1000, 10.0);
    ring.written.set(50);
    cap.pull(&ring);
    // 300 samples in one frame: the ring only has the last 100
    ring.written.set(350);
    assert_eq!(cap.pull(&ring), 300);
    assert_eq!(
        cap.samples.len(),
        350,
        "the take keeps its length: later samples stay where they belong"
    );
    assert!(cap.lost >= 200 && cap.lost < 300, "{}", cap.lost);
    let lost = cap.lost as usize;
    assert!(cap.samples[50..50 + lost].iter().all(|s| *s == 0.0));
    assert_eq!(cap.samples[349], 349.0 / 1000.0);
    assert_eq!(cap.samples[50 + lost], (50 + lost) as f32 / 1000.0);
}

#[test]
fn a_take_stops_at_its_room() {
    let ring = Ring {
        written: Cell::new(0),
        window: 1 << 20,
    };
    let mut cap = Capture::new(0, 1000, 1.0); // room for 2 s + 1 s
    ring.written.set(10_000);
    cap.pull(&ring);
    assert_eq!(cap.samples.len(), 3000);
    assert_eq!(cap.pull(&ring), 0);
}

#[test]
fn the_take_is_placed_in_the_chart_as_the_scorer_places_plucks() {
    // a pluck at ring index 4800 (the take from 0, 48 kHz) heard at chart time 12.1, at 85%: sample 0 was at
    // 12.1 - 0.1 s * 0.85
    let t0 = t0_from_pluck(4800, 0, 48000, 12.1, 0.85);
    assert!((t0 - (12.1 - 0.085)).abs() < 1e-9, "{t0}");
    // the plucks' median wins over the frame's guess; one odd pluck doesn't move it
    let mut cap = Capture::new(1000, 48000, 10.0);
    cap.frame(9.0);
    assert_eq!(
        cap.timing(1.0).chart_time(0.0),
        9.0,
        "no plucks: the frame's guess"
    );
    for (idx, chart) in [
        (1000 + 48000, 11.0),
        (1000 + 96000, 12.0),
        (1000 + 144000, 13.5),
        (1000 + 192000, 14.0),
    ] {
        cap.pluck(idx, chart, 1.0);
    }
    let tm = cap.timing(1.0);
    assert!((tm.chart_time(0.0) - 10.0).abs() < 1e-9, "{:?}", tm);
    assert!((tm.chart_time(48000.0) - 11.0).abs() < 1e-9);
    // a pluck before the take started (the count-in) doesn't count
    let mut early = Capture::new(1000, 48000, 10.0);
    early.pluck(10, 3.0, 1.0);
    early.frame(5.0);
    assert_eq!(early.timing(1.0).chart_time(0.0), 5.0);
    // the frame's guess: the now-line at 20.0, guitar timing +0.01, 12 ms of input latency at half speed, the newest
    // sample 480 after the take's start at 48 kHz
    let t0 = t0_from_frame(20.0, 0.01, 0.012, 0.5, 1481, 1000, 48000);
    assert!(
        (t0 - (20.0 + 0.01 - 0.006 - 480.0 / 48000.0 * 0.5)).abs() < 1e-9,
        "{t0}"
    );
}

fn report_of(statuses: &[(f64, Status, Option<f64>)]) -> TakeReport {
    use qrs_analysis::take::{EventKind, EventResult, Neck, TimingSummary};
    let arr = qrs_formats::Arrangement::new("Lead");
    let neck = Neck::new(&arr);
    TakeReport {
        artist: String::new(),
        title: String::new(),
        part: "Lead".into(),
        from: 0.0,
        to: 10.0,
        speed: 1.0,
        rate: 48000,
        seconds: 10.0,
        events: statuses
            .iter()
            .map(|&(t, status, offset_ms)| EventResult {
                t,
                bar: 0,
                kind: EventKind::Note,
                techniques: qrs_formats::Techs::NONE,
                chord_name: String::new(),
                expected: vec![neck.pos(1, 5)],
                status,
                played: if status == Status::Wrong {
                    vec![neck.pos(1, 7)]
                } else {
                    Vec::new()
                },
                missing: Vec::new(),
                offset_ms,
                sample: None,
                note: None,
            })
            .collect(),
        neck,
        extras: Vec::new(),
        bars: Vec::new(),
        timing: TimingSummary {
            n: 0,
            mean_ms: 0.0,
            median_ms: 0.0,
            spread_ms: 0.0,
            tendency: Tendency::Unknown,
            by_kind: Vec::new(),
        },
        problems: Vec::new(),
        shift_ms: 0.0,
        plucks_found: 0,
        peak: 0.0,
        analysis_ms: 0.0,
    }
}

#[test]
fn your_tab_marks_each_chart_note_by_how_it_went() {
    let r = report_of(&[
        (1.0, Status::Hit, Some(10.0)),
        (1.5, Status::Hit, Some(150.0)),
        (2.0, Status::Hit, Some(-130.0)),
        (2.5, Status::Wrong, Some(5.0)),
        (3.0, Status::Missed, None),
        (3.5, Status::NotChecked, None),
        (9.0, Status::Hit, Some(0.0)), // no group there: left alone
    ]);
    let groups = [0.5, 1.004, 1.5, 2.0, 2.49, 3.0, 3.5];
    let m = tab_marks(&r, &groups);
    assert_eq!(
        m,
        vec![
            (1, Some(Judgement::Hit)),
            (2, Some(Judgement::Late)),
            (3, Some(Judgement::Early)),
            (4, Some(Judgement::Miss)),
            (5, Some(Judgement::Miss)),
            (6, None),
        ]
    );
}

#[test]
fn a_lesson_take_passes_by_the_reports_accuracy_at_full_speed() {
    // 9 right of 10 checked (one not checked doesn't count): 90%
    let mut v: Vec<(f64, Status, Option<f64>)> = (0..9).map(|i| (i as f64, Status::Hit, Some(0.0))).collect();
    v.push((9.0, Status::Wrong, Some(0.0)));
    v.push((9.5, Status::NotChecked, None));
    let r = report_of(&v);
    assert!((r.accuracy() - 0.9).abs() < 1e-12);
    let pass =
        |speed: f64, mark: f64| super::super::practise::take_outcome(Some(r.accuracy()), speed, mark).passed;
    assert!(pass(1.0, 0.9));
    assert!(pass(1.0, 0.85));
    assert!(!pass(1.0, 0.95));
    assert!(!pass(0.85, 0.8), "a slower take never finishes it");
    assert_eq!(
        r.passed(0.9),
        pass(1.0, 0.9),
        "the report's own rule agrees at full speed"
    );
    // nothing that could be checked: no accuracy, never a pass
    let none = report_of(&[(1.0, Status::NotChecked, None), (2.0, Status::NotChecked, None)]);
    assert_eq!(none.accuracy_checked(), None);
    assert!(!super::super::practise::take_outcome(none.accuracy_checked(), 1.0, 0.0).passed);
    assert!(!none.passed(0.0));
}

#[test]
fn listen_back_mixes_the_take_over_the_song() {
    // the take alone: on both sides, brought up to a useful level
    let take = [0.1f32, -0.1, 0.0];
    let alone = mix(&take, 1000, None, 1000);
    assert_eq!(alone.len(), 6);
    assert!((alone[0] - 0.7).abs() < 1e-6 && (alone[1] - 0.7).abs() < 1e-6);
    // over the song: the song under it, as long as the longer of the two
    let song = [0.5f32, -0.5, 0.5, -0.5, 0.5, -0.5, 0.5, -0.5];
    let both = mix(&take, 1000, Some(&song), 1000);
    assert_eq!(both.len(), 8);
    assert!((both[0] - (0.5 * SONG_UNDER + 0.7)).abs() < 1e-6);
    assert!(
        (both[7] - (-0.5 * SONG_UNDER)).abs() < 1e-6,
        "the song plays on after the take"
    );
    assert!(both.iter().all(|s| s.abs() <= 1.0));
}

#[test]
fn a_seek_ahead_in_a_take_is_told_from_playing_on() {
    // a frame of 16 ms at full speed moves 16 ms; a hitch of half a second still plays on
    assert!(!jumped(10.0, 10.016, 0.016, 1.0));
    assert!(!jumped(10.0, 10.5, 0.5, 1.0));
    // a click two bars on, or Page Down
    assert!(jumped(10.0, 13.0, 0.016, 1.0));
    // at half speed the song moves half as far: a second of chart in 0.1 s of wall time is a jump
    assert!(jumped(10.0, 11.0, 0.1, 0.5));
    assert!(!jumped(10.0, 10.05, 0.1, 0.5));
}

#[test]
fn bars_are_coloured_by_how_they_went() {
    assert_eq!(bar_tone(Some(1.0)), 0);
    assert_eq!(bar_tone(Some(0.9)), 0);
    assert_eq!(bar_tone(Some(0.75)), 1);
    assert_eq!(bar_tone(Some(0.2)), 2);
    assert_eq!(bar_tone(None), 3);
}
