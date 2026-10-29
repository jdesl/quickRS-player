//! The jam on its own: clicks and drum hits on their exact samples, the backing locked to the beat, the looper's
//! layers summing (and undoing) where they were played, Rewind fed, and none of it allocating.

use super::*;
use crate::jam::backing::BackingClip;
use crate::jam::drums::{DrumKit, DrumStyle, Inst};
use crate::jam::{RewindKeep, RewindSource};

struct Rig {
    jam: Jam,
    sh: JamShared,
    trash: rtrb::Producer<Trash>,
    _trash_rx: rtrb::Consumer<Trash>,
}

fn rig(rate: u32) -> Rig {
    let (trash, rx) = rtrb::RingBuffer::new(256);
    Rig {
        jam: Jam::new(rate, 512),
        sh: JamShared::default(),
        trash,
        _trash_rx: rx,
    }
}

impl Rig {
    fn cmd(&mut self, c: JamCmd) {
        self.jam.command(c, &mut self.trash);
    }

    /// `frames` of output in blocks of `block`, the guitar (dry = wet) from `input(t)`; returns the left channel.
    fn run(&mut self, frames: usize, block: usize, input: impl Fn(u64) -> f32) -> Vec<f32> {
        assert!(block <= 512, "the jam takes blocks of up to 512 here");
        let mut out = Vec::with_capacity(frames);
        let mut mix = vec![0.0f32; block * 2];
        let mut g = vec![0.0f32; block];
        while out.len() < frames {
            let n = block.min(frames - out.len());
            let t = self.jam.timeline();
            for (i, v) in g[..n].iter_mut().enumerate() {
                *v = input(t + i as u64);
            }
            mix[..2 * n].fill(0.0);
            self.jam.process(
                &mut mix[..2 * n],
                &g[..n],
                &g[..n],
                n,
                0,
                &self.sh,
                &mut self.trash,
            );
            out.extend(mix[..2 * n].chunks(2).map(|f| f[0]));
        }
        out
    }
}

/// A kit whose chosen instruments are a single sample of 1.0 (the rest silent): each hit is one exact sample.
fn impulse_kit(rate: u32, only: &[Inst]) -> Arc<DrumKit> {
    let mut k = DrumKit::new(rate);
    for (i, s) in k.sounds.iter_mut().enumerate() {
        *s = if only.iter().any(|&x| x as usize == i) {
            vec![1.0]
        } else {
            Vec::new()
        };
    }
    Arc::new(k)
}

fn hits(x: &[f32]) -> Vec<(usize, f32)> {
    x.iter()
        .enumerate()
        .filter(|(_, v)| v.abs() > 1e-6)
        .map(|(i, v)| (i, *v))
        .collect()
}

/// Frame of beat `b` from a start at frame 0: the first frame at or after it.
fn frame(b: f64, bpm: f64, rate: u32) -> usize {
    (b * 60.0 * rate as f64 / bpm - 1e-6).ceil() as usize
}

#[test]
fn the_click_lands_on_exact_samples() {
    for (bpm, block) in [(120.0, 256), (113.0, 333), (87.5, 97)] {
        let rate = 48000;
        let mut r = rig(rate);
        r.cmd(JamCmd::Kit(impulse_kit(rate, &[Inst::ClickAccent, Inst::Click])));
        r.cmd(JamCmd::Tempo(bpm));
        r.cmd(JamCmd::Meter(4));
        r.cmd(JamCmd::Click(ClickParams {
            on: true,
            accent: true,
            volume: 1.0,
        }));
        r.cmd(JamCmd::Start { count_in: 0 });
        let x = r.run(rate as usize * 6, block, |_| 0.0);
        let h = hits(&x);
        let beats = (6.0 * bpm / 60.0) as usize;
        assert!(h.len() >= beats, "{bpm}: {} clicks", h.len());
        for (k, &(at, v)) in h.iter().enumerate() {
            assert_eq!(at, frame(k as f64, bpm, rate), "{bpm} bpm, beat {k}");
            let want = if k % 4 == 0 { 1.0 } else { 0.7 };
            assert!((v - want).abs() < 1e-6, "beat {k}: {v}");
        }
    }
}

#[test]
fn a_tempo_change_carries_on_from_the_beat_it_is_on() {
    let rate = 48000;
    let mut r = rig(rate);
    r.cmd(JamCmd::Kit(impulse_kit(rate, &[Inst::ClickAccent, Inst::Click])));
    r.cmd(JamCmd::Click(ClickParams {
        on: true,
        accent: false,
        volume: 1.0,
    }));
    r.cmd(JamCmd::Start { count_in: 0 });
    // 120 bpm for 30000 frames (beat 1.25), then 60 bpm: beat 2 is 0.75 beats (36000 frames) on
    let a = r.run(30000, 500, |_| 0.0);
    r.cmd(JamCmd::Tempo(60.0));
    let b = r.run(100000, 500, |_| 0.0);
    let all: Vec<f32> = a.into_iter().chain(b).collect();
    let h: Vec<usize> = hits(&all).iter().map(|h| h.0).collect();
    assert_eq!(h[..4], [0, 24000, 66000, 114000]);
}

#[test]
fn a_count_in_clicks_a_bar_then_the_drums_come_in() {
    let rate = 8000;
    let mut r = rig(rate);
    r.cmd(JamCmd::Kit(impulse_kit(
        rate,
        &[Inst::ClickAccent, Inst::Click, Inst::Kick],
    )));
    r.cmd(JamCmd::Click(ClickParams {
        on: false,
        accent: true,
        volume: 1.0,
    }));
    r.cmd(JamCmd::Drums(DrumParams {
        on: true,
        style: DrumStyle::Rock,
        volume: 1.0,
        ..Default::default()
    }));
    r.cmd(JamCmd::Start { count_in: 1 });
    let x = r.run(8000 * 4, 64, |_| 0.0);
    let h = hits(&x);
    // 120 bpm at 8 kHz: 4000 frames a beat; four clicks, then the kick on bar 0's one
    let pos: Vec<usize> = h.iter().map(|h| h.0).collect();
    assert_eq!(pos[..5], [0, 4000, 8000, 12000, 16000]);
    assert!((h[0].1 - 1.0).abs() < 1e-6 && (h[1].1 - 0.7).abs() < 1e-6);
    assert!((h[4].1 - 0.8 * KIT_LEVEL).abs() < 1e-6, "the kick: {}", h[4].1);
}

#[test]
fn drum_patterns_play_on_their_ticks() {
    let rate = 48000;
    let bpm = 100.0;
    let groove = |style: DrumStyle, inst: Inst, fill_every: u32| {
        let mut r = rig(rate);
        r.cmd(JamCmd::Kit(impulse_kit(rate, &[inst])));
        r.cmd(JamCmd::Tempo(bpm));
        r.cmd(JamCmd::Click(ClickParams {
            on: false,
            ..Default::default()
        }));
        r.cmd(JamCmd::Drums(DrumParams {
            on: true,
            style,
            volume: 1.0,
            fill_every,
            ..Default::default()
        }));
        r.cmd(JamCmd::Start { count_in: 0 });
        // three bars
        let x = r.run(frame(12.0, bpm, rate), 300, |_| 0.0);
        hits(&x).iter().map(|h| h.0).collect::<Vec<_>>()
    };
    let at = |beats: &[f64]| beats.iter().map(|&b| frame(b, bpm, rate)).collect::<Vec<_>>();
    // rock: kick on 1, the "and" of 3, and 4 ("x.......x.x....."); snare on 2 and 4
    assert_eq!(
        groove(DrumStyle::Rock, Inst::Kick, 0),
        at(&[0.0, 2.0, 2.5, 4.0, 6.0, 6.5, 8.0, 10.0, 10.5])
    );
    assert_eq!(
        groove(DrumStyle::Rock, Inst::Snare, 0),
        at(&[1.0, 3.0, 5.0, 7.0, 9.0, 11.0])
    );
    // half-time: the snare on 3 only
    assert_eq!(groove(DrumStyle::HalfTime, Inst::Snare, 0), at(&[2.0, 6.0, 10.0]));
    // shuffle: hats on the beat and the third triplet
    let sh = groove(DrumStyle::Shuffle, Inst::HatClosed, 0);
    assert_eq!(sh[..4], at(&[0.0, 2.0 / 3.0, 1.0, 1.0 + 2.0 / 3.0])[..]);
    // funk: sixteenth hats, 15 of 16 a bar
    let funk = groove(DrumStyle::Funk, Inst::HatClosed, 0);
    assert_eq!(funk.iter().filter(|&&f| f < frame(4.0, bpm, rate)).count(), 15);
    assert_eq!(funk[1], frame(0.25, bpm, rate));
    // metal: sixteenth kicks
    assert_eq!(groove(DrumStyle::Metal, Inst::Kick, 0).len(), 48);
    // a fill every 2 bars: toms in bar 1's last beat, a crash on bar 2's one
    let toms = groove(DrumStyle::Rock, Inst::TomLow, 2);
    assert_eq!(toms, at(&[7.5, 7.75]));
    assert_eq!(groove(DrumStyle::Rock, Inst::Crash, 2), at(&[8.0]));
    assert!(groove(DrumStyle::Rock, Inst::TomLow, 0).is_empty());
}

#[test]
fn following_the_guitar_drops_the_ghosts_when_it_is_quiet() {
    let rate = 48000;
    let count = |level: f32| {
        let mut r = rig(rate);
        r.cmd(JamCmd::Kit(impulse_kit(rate, &[Inst::Snare])));
        r.cmd(JamCmd::Click(ClickParams {
            on: false,
            ..Default::default()
        }));
        r.cmd(JamCmd::Drums(DrumParams {
            on: true,
            style: DrumStyle::Funk,
            volume: 1.0,
            follow: true,
            ..Default::default()
        }));
        // the guitar playing for two seconds first, so the band has caught up
        r.run(96000, 480, |t| level * ((t as f32) * 0.05).sin());
        r.cmd(JamCmd::Start { count_in: 0 });
        let x = r.run(frame(4.0, 120.0, rate), 480, |t| {
            level * ((t as f32) * 0.05).sin()
        });
        let h = hits(&x);
        let loudest = h.iter().fold(0.0f32, |m, h| m.max(h.1));
        (h.len(), loudest, r.sh.intensity.get())
    };
    let (quiet, soft, iq) = count(0.001);
    let (loud, hard, il) = count(0.3);
    assert!(iq < 0.2 && il > 0.9, "intensity {iq} / {il}");
    assert_eq!(quiet, 2, "backbeat only");
    assert_eq!(loud, 5, "with the ghosts");
    assert!(hard > soft * 1.8, "louder: {soft} -> {hard}");
}

#[test]
fn the_backing_plays_locked_to_the_beat() {
    let rate = 8000;
    let bpm = 120.0;
    let mut r = rig(rate);
    r.cmd(JamCmd::Tempo(bpm));
    // a 2-beat clip whose left channel is its own frame index
    let n = 8000usize;
    let clip = BackingClip {
        rate,
        bpm,
        beats_per_bar: 4,
        beats: 2,
        samples: (0..n).flat_map(|i| [i as f32, 0.0]).collect(),
    };
    r.cmd(JamCmd::Backing(Some(Arc::new(clip))));
    r.cmd(JamCmd::BackingGain(1.0));
    r.cmd(JamCmd::Click(ClickParams {
        on: false,
        ..Default::default()
    }));
    r.cmd(JamCmd::Start { count_in: 1 });
    let x = r.run(40000, 128, |_| 0.0);
    // silent through the count-in bar (16000 frames), then the clip from its start, faded in over 50 ms
    assert!(x[..16000].iter().all(|v| *v == 0.0));
    for t in [16400usize, 20000, 23999, 24000, 31000, 39999] {
        let want = ((t - 16000) % n) as f32;
        assert!((x[t] - want).abs() < 1e-3, "frame {t}: {} vs {want}", x[t]);
    }
    // a clip at another tempo fades out
    r.cmd(JamCmd::Tempo(100.0));
    let y = r.run(4000, 128, |_| 0.0);
    assert!(y[1000..].iter().all(|v| *v == 0.0));
}

/// A rig for re-amping: doubles the signal.
struct Double;
impl ToneProcessor for Double {
    fn process(&mut self, x: &mut [f32]) {
        for v in x {
            *v *= 2.0;
        }
    }
    fn reset(&mut self) {}
}

fn looper_rig(rate: u32, seconds: f64) -> Rig {
    let mut r = rig(rate);
    r.cmd(JamCmd::LoopStore(Some(Box::new(LoopStore::new(
        rate,
        LooperSettings {
            max_seconds: seconds,
            max_layers: 4,
        },
    )))));
    r
}

fn a(p: u64) -> f32 {
    0.001 * (p % 97) as f32
}

#[test]
fn looper_record_overdub_undo_yields_the_right_sum() {
    for delay in [0u64, 64] {
        let rate = 8000;
        let mut r = looper_rig(rate, 2.0);
        r.jam.delay = delay;
        assert_eq!(
            LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
            LoopState::Off
        );
        r.run(10, 10, |_| 0.0);
        assert_eq!(
            LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
            LoopState::Empty
        );
        // the first take: A played over heard frames 10..1010 (arriving `delay` later)
        let t0 = r.jam.timeline();
        r.cmd(JamCmd::Loop(LoopAction::Press));
        let input = move |t: u64| {
            if t >= t0 + delay && t < t0 + 1000 + delay {
                a(t - delay - t0)
            } else {
                0.0
            }
        };
        r.run(1000, 100, input);
        assert_eq!(
            LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
            LoopState::Recording
        );
        r.cmd(JamCmd::Loop(LoopAction::Press));
        let x = r.run(2000, 100, input);
        assert_eq!(r.sh.loop_len.load(Ordering::Relaxed), 1000);
        assert_eq!(
            LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
            LoopState::Playing
        );
        for (i, v) in x.iter().enumerate() {
            let want = a((i % 1000) as u64);
            assert!((v - want).abs() < 2e-4, "delay {delay}, frame {i}: {v} vs {want}");
        }
        // an overdub of B (0.05) over the loop's second half
        r.run(500, 100, |_| 0.0);
        let t1 = r.jam.timeline();
        r.cmd(JamCmd::Loop(LoopAction::Press));
        assert_eq!(
            LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
            LoopState::Playing
        );
        let b_in = move |t: u64| {
            if t >= t1 + delay && t < t1 + 500 + delay {
                0.05
            } else {
                0.0
            }
        };
        r.run(500, 100, b_in);
        r.cmd(JamCmd::Loop(LoopAction::Press));
        r.run(1000, 100, b_in);
        assert_eq!(r.sh.layers.load(Ordering::Relaxed), 2);
        let x = r.run(1000, 100, |_| 0.0);
        let origin = (t1 - 500) as usize; // a loop start (the loop is 1000 long and t1 is half-way)
        let start = r.jam.timeline() as usize - 1000;
        for (i, v) in x.iter().enumerate() {
            let p = (start + i - origin) % 1000;
            let want = a(p as u64) + if p >= 500 { 0.05 } else { 0.0 };
            assert!((v - want).abs() < 3e-4, "delay {delay}, pos {p}: {v} vs {want}");
        }
        // undo the overdub, redo it, clear
        r.cmd(JamCmd::Loop(LoopAction::Undo));
        let x = r.run(1000, 100, |_| 0.0);
        assert_eq!(r.sh.layers.load(Ordering::Relaxed), 1);
        assert_eq!(r.sh.redo.load(Ordering::Relaxed), 1);
        let start = r.jam.timeline() as usize - 1000;
        for (i, v) in x.iter().enumerate() {
            let p = (start + i - origin) % 1000;
            assert!((v - a(p as u64)).abs() < 2e-4, "undone, pos {p}");
        }
        r.cmd(JamCmd::Loop(LoopAction::Redo));
        let x = r.run(1000, 100, |_| 0.0);
        let start = r.jam.timeline() as usize - 1000;
        let p = (start + 700 - origin) % 1000;
        assert!((x[700] - a(p as u64) - if p >= 500 { 0.05 } else { 0.0 }).abs() < 3e-4);
        // the first take can't be undone (that's Clear)
        r.cmd(JamCmd::Loop(LoopAction::Undo));
        r.cmd(JamCmd::Loop(LoopAction::Undo));
        r.run(10, 10, |_| 0.0);
        assert_eq!(r.sh.layers.load(Ordering::Relaxed), 1);
        r.cmd(JamCmd::Loop(LoopAction::Clear));
        let x = r.run(1000, 100, |_| 0.0);
        assert!(x.iter().all(|v| *v == 0.0));
        assert_eq!(
            LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
            LoopState::Empty
        );
    }
}

#[test]
fn an_overdub_going_round_twice_adds_to_itself_and_the_loop_keeps_its_length() {
    let rate = 8000;
    let mut r = looper_rig(rate, 2.0);
    r.cmd(JamCmd::Loop(LoopAction::Record));
    r.run(400, 100, |_| 0.1);
    r.cmd(JamCmd::Loop(LoopAction::Play));
    r.cmd(JamCmd::Loop(LoopAction::Overdub));
    r.run(800, 100, |_| 0.02); // two times round
    r.cmd(JamCmd::Loop(LoopAction::Play));
    let x = r.run(400, 100, |_| 0.0);
    assert_eq!(r.sh.loop_len.load(Ordering::Relaxed), 400);
    assert!(x.iter().all(|v| (v - 0.14).abs() < 3e-4), "{:?}", &x[..4]);
    // stop and play again: from the top
    r.cmd(JamCmd::Loop(LoopAction::Stop));
    let y = r.run(100, 100, |_| 0.0);
    assert!(y.iter().all(|v| *v == 0.0));
    r.cmd(JamCmd::Loop(LoopAction::Play));
    assert!((r.run(1, 1, |_| 0.0)[0] - 0.14).abs() < 3e-4);
}

#[test]
fn a_loop_locked_to_the_click_snaps_to_bars() {
    let rate = 8000; // 120 bpm: 4000 frames a beat, 16000 a bar
    let mut r = looper_rig(rate, 10.0);
    r.cmd(JamCmd::Kit(impulse_kit(rate, &[])));
    r.cmd(JamCmd::Click(ClickParams {
        on: true,
        accent: true,
        volume: 0.0,
    }));
    r.cmd(JamCmd::Start { count_in: 0 });
    // pressed a little after bar 0's line: the take starts on it
    r.run(300, 100, |_| 0.0);
    r.cmd(JamCmd::Loop(LoopAction::Press));
    r.run(100, 100, |_| 0.2);
    assert_eq!(
        LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
        LoopState::Recording
    );
    // closed a little late, at 2.06 bars: the loop is two bars and plays on from where it is
    r.run(33000 - 400, 100, |_| 0.2);
    r.cmd(JamCmd::Loop(LoopAction::Press));
    r.run(100, 100, |_| 0.0);
    assert_eq!(r.sh.loop_len.load(Ordering::Relaxed), 32000);
    assert_eq!(r.sh.loop_pos.load(Ordering::Relaxed), 33000 - 32000);
    // the first 300 frames went by before the press: not recorded (silent); the rest is the take
    let x = r.run(32000, 100, |_| 0.0);
    let p0 = 33100 - 32000;
    for (i, v) in x.iter().enumerate() {
        let p = (p0 + i) % 32000;
        let want = if p >= 300 { 0.2 } else { 0.0 };
        assert!((v - want).abs() < 2e-4, "pos {p}: {v}");
    }
    // cleared, then pressed well into a bar: armed until the next bar line
    r.cmd(JamCmd::Loop(LoopAction::Clear));
    let now = r.jam.timeline();
    let into = now % 16000;
    r.run((16000 + 6000 - into) as usize % 16000, 100, |_| 0.0);
    r.cmd(JamCmd::Loop(LoopAction::Press));
    r.run(100, 100, |_| 0.0);
    assert_eq!(
        LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
        LoopState::Armed
    );
    // the first take stops itself at the longest loop (10 s: 5 bars quantised down)
    r.run(16000 + 81000, 500, |_| 0.1);
    assert_eq!(
        LoopState::from_u32(r.sh.loop_state.load(Ordering::Relaxed)),
        LoopState::Playing
    );
    assert_eq!(r.sh.loop_len.load(Ordering::Relaxed), 80000);
}

#[test]
fn re_amp_plays_the_dry_layers_through_the_rig() {
    let rate = 8000;
    let mut r = looper_rig(rate, 1.0);
    r.cmd(JamCmd::Loop(LoopAction::Record));
    // dry and wet are the same input here; the re-amp rig doubles it
    r.run(200, 50, |_| 0.1);
    r.cmd(JamCmd::Loop(LoopAction::Play));
    let x = r.run(200, 50, |_| 0.0);
    assert!(x.iter().all(|v| (v - 0.1).abs() < 2e-4));
    r.cmd(JamCmd::ReampTone(Some(Box::new(Double))));
    r.cmd(JamCmd::Reamp(true));
    let x = r.run(200, 50, |_| 0.0);
    assert!(x.iter().all(|v| (v - 0.2).abs() < 4e-4), "{}", x[0]);
}

#[test]
fn rewind_takes_the_guitar_dry_and_wet() {
    let rate = 8000;
    let mut r = rig(rate);
    let ring = Arc::new(RewindRing::new(rate, 1.0, RewindKeep::Both));
    r.cmd(JamCmd::Rewind(Some(ring.clone())));
    r.run(20000, 128, |t| ((t as f32) * 0.01).sin() * 0.5);
    assert_eq!(ring.written(), 20000);
    let got = ring.read(19000, 20000, RewindSource::Dry).unwrap();
    for (i, v) in got.iter().enumerate() {
        let want = (((19000 + i) as f32) * 0.01).sin() * 0.5;
        assert!((v - want).abs() < 1e-4);
    }
}

#[test]
fn nothing_allocates_or_frees_on_the_audio_thread() {
    let rate = 48000;
    let mut r = looper_rig(rate, 4.0);
    // everything built first, as the engine does
    let kit = Arc::new(DrumKit::new(rate));
    let kit2 = Arc::new(DrumKit::new(rate));
    let spec = crate::jam::backing::BackingSpec::default();
    let clip = Arc::new(crate::jam::backing::render(&spec, 140.0, 4, rate));
    let clip2 = Arc::new(crate::jam::backing::render(&spec, 140.0, 4, rate));
    let ring = Arc::new(RewindRing::new(rate, 2.0, RewindKeep::Both));
    let ring2 = Arc::new(RewindRing::new(rate, 2.0, RewindKeep::Dry));
    let store = Box::new(LoopStore::new(
        rate,
        LooperSettings {
            max_seconds: 2.0,
            max_layers: 3,
        },
    ));
    let tone: Box<dyn ToneProcessor> = Box::new(Double);
    let tone2: Box<dyn ToneProcessor> = Box::new(Double);
    let mut cmds = vec![
        JamCmd::Kit(kit),
        JamCmd::Rewind(Some(ring)),
        JamCmd::Tempo(140.0),
        JamCmd::Backing(Some(clip)),
        JamCmd::ReampTone(Some(tone)),
        JamCmd::Reamp(true),
        JamCmd::Click(ClickParams::default()),
        JamCmd::Drums(DrumParams {
            on: true,
            style: DrumStyle::Metal,
            fill_every: 2,
            follow: true,
            ..Default::default()
        }),
        JamCmd::Start { count_in: 1 },
        JamCmd::Loop(LoopAction::Press),
        JamCmd::Loop(LoopAction::Press),
        JamCmd::Loop(LoopAction::Press),
        JamCmd::Drums(DrumParams {
            on: true,
            style: DrumStyle::Funk,
            fill_every: 1,
            ..Default::default()
        }),
        JamCmd::Loop(LoopAction::Press),
        JamCmd::Loop(LoopAction::Undo),
        JamCmd::Loop(LoopAction::Redo),
        JamCmd::Backing(Some(clip2)),
        JamCmd::Kit(kit2),
        JamCmd::Tempo(97.0),
        JamCmd::Loop(LoopAction::Overdub),
        JamCmd::Loop(LoopAction::Stop),
        JamCmd::Loop(LoopAction::Play),
        JamCmd::ReampTone(Some(tone2)),
        JamCmd::Rewind(Some(ring2)),
        JamCmd::Loop(LoopAction::Clear),
        JamCmd::LoopStore(Some(store)),
        JamCmd::Loop(LoopAction::Record),
        JamCmd::Backing(None),
        JamCmd::Stop,
    ];
    cmds.reverse();
    let mut mix = vec![0.0f32; 512 * 2];
    let g: Vec<f32> = (0..512).map(|i| (i as f32 * 0.03).sin() * 0.2).collect();
    crate::alloc_count::watch();
    for block in 0..2000 {
        if block % 60 == 0
            && let Some(c) = cmds.pop()
        {
            r.jam.command(c, &mut r.trash);
        }
        let n = 128 + (block * 37) % 384;
        r.jam
            .process(&mut mix[..2 * n], &g[..n], &g[..n], n, 0, &r.sh, &mut r.trash);
    }
    let calls = crate::alloc_count::calls();
    assert!(cmds.is_empty());
    assert_eq!(calls, 0, "{calls} allocator calls on the audio thread");
}
