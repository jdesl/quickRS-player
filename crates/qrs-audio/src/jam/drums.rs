//! The jam's drum kit and grooves. New for quickRS (P22.4).
//!
//! Every sound is synthesised on the control thread into a short mono clip (no sample files): a kick (a sine
//! swept down, a beater click), a snare (two drum-head modes and filtered noise), hats, ride and crash (six square
//! waves at inharmonic ratios, as the 808 does, high-passed, with noise), toms, a side stick and the metronome's
//! two clicks. Grooves are written as step strings (`x` hit, `X` accent, `o` soft, `g` ghost, `.` rest) and
//! compiled into a sorted hit list the audio thread reads without allocating.

use crate::synth::Rng;

/// The kit's instruments (indexes into `DrumKit::sounds`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Inst {
    Kick = 0,
    Snare,
    Stick,
    HatClosed,
    HatOpen,
    Ride,
    Crash,
    TomHigh,
    TomLow,
    ClickAccent,
    Click,
}

pub const N_INST: usize = 11;

impl Inst {
    /// The level knob it follows: 0 kick, 1 snare (and stick, toms), 2 hats (and cymbals), 3 the click.
    pub fn group(self) -> usize {
        match self {
            Inst::Kick => 0,
            Inst::Snare | Inst::Stick | Inst::TomHigh | Inst::TomLow => 1,
            Inst::HatClosed | Inst::HatOpen | Inst::Ride | Inst::Crash => 2,
            Inst::ClickAccent | Inst::Click => 3,
        }
    }

    /// Left-right placement (-1..1), a drummer's view kept narrow.
    pub fn pan(self) -> f32 {
        match self {
            Inst::HatClosed | Inst::HatOpen => -0.3,
            Inst::Ride => 0.35,
            Inst::Crash => -0.2,
            Inst::TomHigh => -0.15,
            Inst::TomLow => 0.25,
            _ => 0.0,
        }
    }
}

/// The drum loops on offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum DrumStyle {
    #[default]
    Rock,
    Indie,
    Funk,
    Shuffle,
    HalfTime,
    Ballad,
    Metal,
}

impl DrumStyle {
    pub const ALL: [DrumStyle; 7] = [
        DrumStyle::Rock,
        DrumStyle::Indie,
        DrumStyle::Funk,
        DrumStyle::Shuffle,
        DrumStyle::HalfTime,
        DrumStyle::Ballad,
        DrumStyle::Metal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DrumStyle::Rock => "Rock",
            DrumStyle::Indie => "Indie",
            DrumStyle::Funk => "Funk",
            DrumStyle::Shuffle => "Shuffle",
            DrumStyle::HalfTime => "Half-time",
            DrumStyle::Ballad => "Ballad",
            DrumStyle::Metal => "Metal",
        }
    }

    /// A stable id for settings files.
    pub fn id(self) -> &'static str {
        match self {
            DrumStyle::Rock => "rock",
            DrumStyle::Indie => "indie",
            DrumStyle::Funk => "funk",
            DrumStyle::Shuffle => "shuffle",
            DrumStyle::HalfTime => "halftime",
            DrumStyle::Ballad => "ballad",
            DrumStyle::Metal => "metal",
        }
    }

    pub fn from_id(id: &str) -> Option<DrumStyle> {
        DrumStyle::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn index(self) -> usize {
        DrumStyle::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    /// The groove and its fill (the fill's last two beats replace the groove's in a fill bar).
    fn source(self) -> (Groove, Groove) {
        match self {
            DrumStyle::Rock => (
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.......x.x....."),
                        (Inst::Snare, "....X.......X..."),
                        (Inst::HatClosed, "x.o.x.o.x.o.x.o."),
                    ],
                },
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.......x......."),
                        (Inst::Snare, "....X...xoxo...."),
                        (Inst::TomHigh, "............xx.."),
                        (Inst::TomLow, "..............xx"),
                        (Inst::HatClosed, "x.o.x.o........."),
                    ],
                },
            ),
            DrumStyle::Indie => (
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x...x...x...x..."),
                        (Inst::Snare, "....X.......X..."),
                        (Inst::HatClosed, "x.....o.x.....o."),
                        (Inst::HatOpen, "..x.......x....."),
                    ],
                },
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x...x...x...x..."),
                        (Inst::Snare, "....X...x.x.xxxx"),
                        (Inst::HatClosed, "x.....o........."),
                        (Inst::HatOpen, "..x............."),
                    ],
                },
            ),
            DrumStyle::Funk => (
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.x....x..x..x.."),
                        (Inst::Snare, "....X..g.g..X..g"),
                        (Inst::HatClosed, "xoxoxoxoxoxoxo.o"),
                        (Inst::HatOpen, "..............x."),
                    ],
                },
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.x....x...x...x"),
                        (Inst::Snare, "....X..g.xgxX.xx"),
                        (Inst::HatClosed, "xoxoxoxo........"),
                    ],
                },
            ),
            DrumStyle::Shuffle => (
                Groove {
                    steps: 12,
                    rows: &[
                        (Inst::Kick, "x.....x.x..."),
                        (Inst::Snare, "...X.g...X.g"),
                        (Inst::HatClosed, "x.ox.ox.ox.o"),
                    ],
                },
                Groove {
                    steps: 12,
                    rows: &[
                        (Inst::Kick, "x.....x....."),
                        (Inst::Snare, "...X.gxoxx.x"),
                        (Inst::TomLow, "..........x."),
                        (Inst::HatClosed, "x.ox.o......"),
                    ],
                },
            ),
            DrumStyle::HalfTime => (
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.....x...x....."),
                        (Inst::Snare, "........X......."),
                        (Inst::HatClosed, "x.o.x.o.x.o.x.o."),
                    ],
                },
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.....x........."),
                        (Inst::Snare, "........X...x.x."),
                        (Inst::TomHigh, ".............x.."),
                        (Inst::TomLow, "...............x"),
                        (Inst::HatClosed, "x.o.x.o........."),
                    ],
                },
            ),
            DrumStyle::Ballad => (
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.......x.....o."),
                        (Inst::Stick, "....x.......x..."),
                        (Inst::HatClosed, "o.o.o.o.o.o.o.o."),
                    ],
                },
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "x.......x......."),
                        (Inst::Stick, "....x..........."),
                        (Inst::Snare, "........o.o.x.x."),
                        (Inst::HatClosed, "o.o.o.o........."),
                    ],
                },
            ),
            DrumStyle::Metal => (
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "xxxxxxxxxxxxxxxx"),
                        (Inst::Snare, "....X.......X..."),
                        (Inst::Ride, "x...x...x...x..."),
                    ],
                },
                Groove {
                    steps: 16,
                    rows: &[
                        (Inst::Kick, "xxxxxxxxxxxxxxxx"),
                        (Inst::Snare, "....X...xxxx...."),
                        (Inst::TomHigh, "............xx.."),
                        (Inst::TomLow, "..............xx"),
                        (Inst::Ride, "x...x..........."),
                    ],
                },
            ),
        }
    }
}

/// A groove as written: one bar of 4/4 in `steps` steps (16: sixteenths; 12: eighth-note triplets).
struct Groove {
    steps: u32,
    rows: &'static [(Inst, &'static str)],
}

/// Ticks per beat on the drum grid (sixteenths and triplets both land on it).
pub const TICKS_PER_BEAT: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Ticks into the 4/4 bar (0..48).
    pub tick: u32,
    pub inst: Inst,
    pub vel: f32,
    /// A ghost note: left out when the band plays softly (following the guitar).
    pub ghost: bool,
}

/// A groove ready for the audio thread: hits sorted by tick.
#[derive(Clone, Debug, Default)]
pub struct Pattern {
    pub hits: Vec<Hit>,
}

impl Pattern {
    fn compile(g: &Groove) -> Pattern {
        let per_step = 4 * TICKS_PER_BEAT / g.steps;
        let mut hits = Vec::new();
        for &(inst, row) in g.rows {
            for (i, c) in row.chars().enumerate() {
                let (vel, ghost) = match c {
                    'X' => (1.0, false),
                    'x' => (0.8, false),
                    'o' => (0.5, false),
                    'g' => (0.28, true),
                    _ => continue,
                };
                hits.push(Hit {
                    tick: i as u32 * per_step,
                    inst,
                    vel,
                    ghost,
                });
            }
        }
        hits.sort_by_key(|h| h.tick);
        Pattern { hits }
    }

    /// The hits on one tick (a slice of the sorted list; no allocation).
    pub fn at(&self, tick: u32) -> &[Hit] {
        let a = self.hits.partition_point(|h| h.tick < tick);
        let b = self.hits.partition_point(|h| h.tick <= tick);
        &self.hits[a..b]
    }
}

/// Every sound and groove, at one sample rate. Built off the audio thread, shared with it read-only.
pub struct DrumKit {
    pub rate: u32,
    pub sounds: [Vec<f32>; N_INST],
    /// (groove, fill) per style, in `DrumStyle::ALL` order.
    pub patterns: Vec<(Pattern, Pattern)>,
}

impl DrumKit {
    pub fn new(rate: u32) -> DrumKit {
        let r = rate as f32;
        let mut rng = Rng::new(0x5eed_d2a3);
        let sounds = [
            kick(r),
            snare(r, &mut rng, 1.0),
            stick(r, &mut rng),
            hat(r, &mut rng, 0.045, 0.8),
            hat(r, &mut rng, 0.38, 0.7),
            ride(r, &mut rng),
            crash(r, &mut rng),
            tom(r, 196.0, &mut rng),
            tom(r, 131.0, &mut rng),
            click(r, 1760.0),
            click(r, 1175.0),
        ];
        let patterns = DrumStyle::ALL
            .iter()
            .map(|s| {
                let (g, f) = s.source();
                (Pattern::compile(&g), Pattern::compile(&f))
            })
            .collect();
        DrumKit {
            rate,
            sounds,
            patterns,
        }
    }

    pub fn sound(&self, i: Inst) -> &[f32] {
        &self.sounds[i as usize]
    }
}

// -- the sounds ---------------------------------------------------------------------------------------------------

fn len(r: f32, seconds: f32) -> usize {
    (r * seconds) as usize
}

fn noise(rng: &mut Rng) -> f32 {
    (rng.uniform() * 2.0 - 1.0) as f32
}

/// Normalise to a peak of `peak`, the last eighth faded out (no cut at the end).
fn norm(mut x: Vec<f32>, peak: f32) -> Vec<f32> {
    let m = x.iter().fold(1e-9f32, |m, v| m.max(v.abs()));
    let n = x.len();
    let fade = (n / 8).max(1);
    for (i, v) in x.iter_mut().enumerate() {
        let left = (n - i) as f32 / fade as f32;
        // and a 0.5 ms rise: no step at the start
        let rise = (i as f32 / 24.0).min(1.0);
        *v *= peak / m * left.min(1.0).powi(2) * rise;
    }
    x
}

/// A two-pole state-variable filter (Chamberlin), for the noise parts.
struct Svf {
    f: f32,
    q: f32,
    low: f32,
    band: f32,
}

impl Svf {
    fn new(r: f32, hz: f32, q: f32) -> Svf {
        Svf {
            f: 2.0 * (std::f32::consts::PI * (hz / r).min(0.24)).sin(),
            q: 1.0 / q,
            low: 0.0,
            band: 0.0,
        }
    }
    /// (low, band, high)
    fn tick(&mut self, x: f32) -> (f32, f32, f32) {
        self.low += self.f * self.band;
        let high = x - self.low - self.q * self.band;
        self.band += self.f * high;
        (self.low, self.band, high)
    }
}

fn kick(r: f32) -> Vec<f32> {
    let n = len(r, 0.45);
    let mut ph = 0.0f32;
    let mut lp = Svf::new(r, 3000.0, 0.7);
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            let f = 48.0 + 110.0 * (-t / 0.035).exp();
            ph += std::f32::consts::TAU * f / r;
            let body = ph.sin() * (-t / 0.28).exp();
            // the beater: a few ms of a brighter thump
            let beater = (ph * 3.1).sin() * (-t / 0.004).exp() * 0.4;
            let (low, _, _) = lp.tick(body + beater);
            low * (1.0 - (-t / 0.0007).exp())
        })
        .collect();
    norm(x, 0.95)
}

fn snare(r: f32, rng: &mut Rng, gain: f32) -> Vec<f32> {
    let n = len(r, 0.32);
    let mut bp = Svf::new(r, 4200.0, 0.8);
    let mut hp = Svf::new(r, 900.0, 0.7);
    let (mut p1, mut p2) = (0.0f32, 0.0f32);
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            p1 += std::f32::consts::TAU * (185.0 + 40.0 * (-t / 0.01).exp()) / r;
            p2 += std::f32::consts::TAU * 330.0 / r;
            let head = (p1.sin() * 0.8 + p2.sin() * 0.35) * (-t / 0.06).exp();
            let (_, band, _) = bp.tick(noise(rng));
            let (_, _, high) = hp.tick(band);
            let wires = high * (-t / 0.12).exp() * 1.6;
            (head + wires) * gain * (1.0 - (-t / 0.0005).exp())
        })
        .collect();
    norm(x, 0.9)
}

fn stick(r: f32, rng: &mut Rng) -> Vec<f32> {
    let n = len(r, 0.09);
    let mut bp = Svf::new(r, 1700.0, 4.0);
    let mut ph = 0.0f32;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            ph += std::f32::consts::TAU * 520.0 / r;
            let (_, band, _) = bp.tick(noise(rng) * (-t / 0.004).exp());
            (band * 2.0 + ph.sin() * 0.5) * (-t / 0.025).exp()
        })
        .collect();
    norm(x, 0.7)
}

/// Six square waves at the 808's cymbal ratios (inharmonic: a metallic clang), in Hz.
const METAL: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

fn metal(r: f32, i: usize, scale: f32) -> f32 {
    let t = i as f32 / r;
    METAL
        .iter()
        .map(|f| {
            let ph = (t * f * scale).fract();
            if ph < 0.5 { 1.0 } else { -1.0 }
        })
        .sum::<f32>()
        / 6.0
}

fn hat(r: f32, rng: &mut Rng, decay: f32, peak: f32) -> Vec<f32> {
    let n = len(r, decay * 5.0);
    let mut hp1 = Svf::new(r, 7000.0, 0.7);
    let mut hp2 = Svf::new(r, 7000.0, 0.7);
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            let x = metal(r, i, 1.7) * 0.7 + noise(rng) * 0.5;
            let (_, _, h) = hp1.tick(x);
            let (_, _, h) = hp2.tick(h);
            h * (-t / decay).exp()
        })
        .collect();
    norm(x, peak)
}

fn ride(r: f32, rng: &mut Rng) -> Vec<f32> {
    let n = len(r, 2.2);
    let mut hp = Svf::new(r, 3500.0, 0.7);
    let mut bell = 0.0f32;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            bell += std::f32::consts::TAU * 2350.0 / r;
            let x = metal(r, i, 3.1) * 0.6 + noise(rng) * 0.25;
            let (_, _, h) = hp.tick(x);
            h * (0.25 + 0.75 * (-t / 0.05).exp()) * (-t / 0.7).exp() + bell.sin() * 0.25 * (-t / 0.35).exp()
        })
        .collect();
    norm(x, 0.55)
}

fn crash(r: f32, rng: &mut Rng) -> Vec<f32> {
    let n = len(r, 2.5);
    let mut hp = Svf::new(r, 2500.0, 0.6);
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            let x = metal(r, i, 2.3) * 0.4 + noise(rng);
            let (_, _, h) = hp.tick(x);
            h * (-t / 0.6).exp() * (1.0 - (-t / 0.002).exp())
        })
        .collect();
    norm(x, 0.7)
}

fn tom(r: f32, f: f32, rng: &mut Rng) -> Vec<f32> {
    let n = len(r, 0.6);
    let mut ph = 0.0f32;
    let mut lp = Svf::new(r, 1200.0, 0.7);
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            ph += std::f32::consts::TAU * f * (1.0 + 0.35 * (-t / 0.05).exp()) / r;
            let (low, _, _) = lp.tick(noise(rng) * (-t / 0.01).exp());
            (ph.sin() + low * 0.6) * (-t / 0.22).exp()
        })
        .collect();
    norm(x, 0.85)
}

fn click(r: f32, f: f32) -> Vec<f32> {
    let n = len(r, 0.04);
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / r;
            (std::f32::consts::TAU * f * t).sin() * (-t / 0.008).exp() * (1.0 - (-t / 0.0004).exp())
        })
        .collect();
    norm(x, 0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_compiles_with_a_kick_on_one() {
        let kit = DrumKit::new(48000);
        assert_eq!(kit.patterns.len(), DrumStyle::ALL.len());
        for (s, (groove, fill)) in DrumStyle::ALL.iter().zip(&kit.patterns) {
            assert!(
                groove.at(0).iter().any(|h| h.inst == Inst::Kick),
                "{}: kick on one",
                s.name()
            );
            assert!(groove.hits.iter().all(|h| h.tick < 48));
            assert!(!fill.hits.is_empty());
            assert_eq!(DrumStyle::from_id(s.id()), Some(*s));
        }
        // the shuffle's hats are on the triplet grid: beat, then the third triplet (tick 8)
        let shuffle = &kit.patterns[DrumStyle::Shuffle.index()].0;
        assert!(shuffle.at(8).iter().any(|h| h.inst == Inst::HatClosed));
        assert!(shuffle.at(6).is_empty());
        // funk has ghost notes
        assert!(
            kit.patterns[DrumStyle::Funk.index()]
                .0
                .hits
                .iter()
                .any(|h| h.ghost)
        );
        // the half-time backbeat is on three
        let ht = &kit.patterns[DrumStyle::HalfTime.index()].0;
        assert!(ht.at(24).iter().any(|h| h.inst == Inst::Snare));
        assert!(!ht.at(12).iter().any(|h| h.inst == Inst::Snare));
    }

    #[test]
    fn sounds_are_bounded_and_start_quietly() {
        let kit = DrumKit::new(48000);
        for (i, s) in kit.sounds.iter().enumerate() {
            assert!(!s.is_empty());
            let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.3 && peak <= 1.0, "sound {i}: peak {peak}");
            assert!(s[0].abs() < 0.05, "sound {i} starts with a step: {}", s[0]);
            assert!(s.iter().all(|v| v.is_finite()));
            let tail = s[s.len() - 16..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(tail < 0.05, "sound {i} ends with a cut: {tail}");
        }
    }
}
