//! Reverbs, all "wet only" (their tails ring out when switched off).
//!
//! - Spring: three springs, each a feedback loop around a chain of stretched first-order all-passes (the
//!   dispersion that makes a spring's echoes chirp downwards: low frequencies travel slower), a delay for the
//!   spring's length and its damping; after the parametric spring model of Välimäki, Parker and Abel (AES 2010)
//!   and Parker's efficient dispersion structures (EURASIP 2011). Dwell drives the springs' transducer harder.
//! - Plate: Jon Dattorro's plate (the "figure of eight" tank with modulated all-passes), "Effect Design Part 1",
//!   JAES 45(9), 1997, with its delay lengths scaled from 29.761 kHz.
//! - Room and Hall: eight-line feedback delay networks with a Householder feedback matrix and per-line decay set
//!   from the reverb time (Jot and Chaigne, AES 1991), a low-pass in each line for darker tails, two lines slowly
//!   modulated against metallic ringing; the room adds early reflections, the hall a pre-delay and diffusion.

use super::dsp::{Diffuser, Lfo, Line, OnePole, Smooth, flush, sweep, unit10};
use super::{MAX_KNOBS, Pedal};

// -- Spring ---------------------------------------------------------------------------------------------------------

const AP_STAGES: usize = 40;
const STRETCH: usize = 3;

/// A chain of stretched all-passes (a + z^-K) / (1 + a z^-K).
struct Chirp {
    a: f32,
    x: [[f32; STRETCH]; AP_STAGES],
    y: [[f32; STRETCH]; AP_STAGES],
    i: usize,
}

impl Chirp {
    fn new(a: f32) -> Chirp {
        Chirp {
            a,
            x: [[0.0; STRETCH]; AP_STAGES],
            y: [[0.0; STRETCH]; AP_STAGES],
            i: 0,
        }
    }
    #[inline]
    fn tick(&mut self, mut s: f32) -> f32 {
        let i = self.i;
        for k in 0..AP_STAGES {
            let xd = self.x[k][i];
            let yd = self.y[k][i];
            let y = self.a * s + xd - self.a * yd;
            self.x[k][i] = s;
            self.y[k][i] = flush(y);
            s = y;
        }
        self.i = (i + 1) % STRETCH;
        s
    }
    fn clear(&mut self) {
        self.x = [[0.0; STRETCH]; AP_STAGES];
        self.y = [[0.0; STRETCH]; AP_STAGES];
    }
}

struct OneSpring {
    chirp: Chirp,
    line: Line,
    len: f32,
    g: f32,
    lp: OnePole,
    hp: OnePole,
    end: f32,
}

struct Spring {
    rate: f32,
    springs: [OneSpring; 3],
    drive: f32,
    out_lp: OnePole,
    out_hp: OnePole,
    lfo: Lfo,
    wet: Smooth,
}

pub fn spring(rate: f32) -> Box<dyn Pedal> {
    let one = |ms: f32, a: f32| OneSpring {
        chirp: Chirp::new(a),
        line: Line::new((rate * 0.06) as usize),
        len: ms * 1e-3 * rate,
        g: 0.8,
        lp: OnePole::new(rate, 4500.0),
        hp: OnePole::new(rate, 150.0),
        end: 0.0,
    };
    Box::new(Spring {
        rate,
        springs: [one(33.0, -0.62), one(37.7, -0.58), one(41.3, -0.6)],
        drive: 1.0,
        out_lp: OnePole::new(rate, 4500.0),
        out_hp: OnePole::new(rate, 300.0),
        lfo: Lfo::default(),
        wet: Smooth::new(rate, 30.0, 0.3),
    })
}

impl Pedal for Spring {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            // the dwell tube drives the springs' transducer, which saturates
            let input = (*v * self.drive).tanh() * 0.5;
            let p = self.lfo.step();
            let wob = 0.25 * Lfo::sine(p);
            let mut out = 0.0;
            for (k, s) in self.springs.iter_mut().enumerate() {
                let fb = s.hp.hp(s.lp.lp(s.end)) * s.g;
                let c = s.chirp.tick(input + fb);
                s.line.push(c);
                let wobble = if k == 1 { wob } else { -wob };
                s.end = s.line.linear(s.len + wobble);
                out += c * 0.5 + s.end;
            }
            *v = self.out_lp.lp(self.out_hp.hp(out)) * self.wet.tick();
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        let (dwell, tone, mix) = (unit10(v[0]), unit10(v[1]), (v[2] / 100.0).clamp(0.0, 1.0));
        self.drive = 0.6 + 3.0 * dwell;
        let rt60 = 1.6 + 1.6 * dwell;
        for s in &mut self.springs {
            s.g = 10f32.powf(-3.0 * (s.len / self.rate) / rt60).min(0.97);
        }
        self.out_lp.set(self.rate, sweep(2200.0, 7000.0, tone));
        self.lfo.set(self.rate, 0.7);
        self.wet.target = mix * 0.85 / (0.6 + 0.8 * dwell);
    }
    fn reset(&mut self) {
        for s in &mut self.springs {
            s.chirp.clear();
            s.line.clear();
            s.lp.reset();
            s.hp.reset();
            s.end = 0.0;
        }
        self.out_lp.reset();
        self.out_hp.reset();
        self.wet.snap();
    }
    fn wet_only(&self) -> bool {
        true
    }
}

// -- Plate (Dattorro) -----------------------------------------------------------------------------------------------

struct Plate {
    rate: f32,
    k: f32,
    pre: Line,
    pre_d: usize,
    bw: f32,
    bw_y: f32,
    inp: [Diffuser; 4],
    // the tank: left then right half
    l_ap1: Diffuser,
    l_d1: Line,
    l_damp: f32,
    l_ap2: Diffuser,
    l_d2: Line,
    r_ap1: Diffuser,
    r_d1: Line,
    r_damp: f32,
    r_ap2: Diffuser,
    r_d2: Line,
    lens: [usize; 4],
    decay: f32,
    damping: f32,
    lfo: Lfo,
    excursion: f32,
    wet: Smooth,
}

pub fn plate(rate: f32) -> Box<dyn Pedal> {
    let k = rate / 29761.0;
    let n = |s: f32| (s * k).round() as usize;
    let ap = |len: f32, g: f32| Diffuser::new(n(len) + 40, n(len) as f32, g);
    Box::new(Plate {
        rate,
        k,
        pre: Line::new((rate * 0.11) as usize),
        pre_d: 1,
        bw: 0.9995,
        bw_y: 0.0,
        inp: [
            ap(142.0, 0.75),
            ap(107.0, 0.75),
            ap(379.0, 0.625),
            ap(277.0, 0.625),
        ],
        l_ap1: ap(672.0, -0.7),
        l_d1: Line::new(n(4453.0) + 4),
        l_damp: 0.0,
        l_ap2: ap(1800.0, 0.5),
        l_d2: Line::new(n(3720.0) + 4),
        r_ap1: ap(908.0, -0.7),
        r_d1: Line::new(n(4217.0) + 4),
        r_damp: 0.0,
        r_ap2: ap(2656.0, 0.5),
        r_d2: Line::new(n(3163.0) + 4),
        lens: [n(4453.0), n(3720.0), n(4217.0), n(3163.0)],
        decay: 0.5,
        damping: 0.3,
        lfo: Lfo::default(),
        excursion: 16.0 * k,
        wet: Smooth::new(rate, 30.0, 0.25),
    })
}

impl Pedal for Plate {
    fn process(&mut self, x: &mut [f32]) {
        let k = self.k;
        let n = |s: f32| (s * k).round() as usize;
        let (t266, t2974, t1913, t1996, t1990, t187, t1066) = (
            n(266.0),
            n(2974.0),
            n(1913.0),
            n(1996.0),
            n(1990.0),
            n(187.0),
            n(1066.0),
        );
        let (t353, t3627, t1228, t2673, t2111, t335, t121) = (
            n(353.0),
            n(3627.0),
            n(1228.0),
            n(2673.0),
            n(2111.0),
            n(335.0),
            n(121.0),
        );
        for v in x.iter_mut() {
            self.pre.push(*v);
            let mut s = self.pre.at(self.pre_d);
            self.bw_y = flush(self.bw * s + (1.0 - self.bw) * self.bw_y);
            s = self.bw_y;
            for d in &mut self.inp {
                s = d.tick(s);
            }
            let p = self.lfo.step();
            let m = self.excursion * Lfo::sine(p);
            let l_end = self.r_d2.at(self.lens[3]);
            let r_end = self.l_d2.at(self.lens[1]);
            // left half
            let mut a = s + self.decay * l_end;
            a = self.l_ap1.tick_mod(a, self.l_ap1.delay + m);
            self.l_d1.push(a);
            let mut b = self.l_d1.at(self.lens[0]);
            self.l_damp = flush(b * (1.0 - self.damping) + self.l_damp * self.damping);
            b = self.l_ap2.tick(self.l_damp * self.decay);
            self.l_d2.push(b);
            // right half
            let mut c = s + self.decay * r_end;
            c = self.r_ap1.tick_mod(c, self.r_ap1.delay - m);
            self.r_d1.push(c);
            let mut d = self.r_d1.at(self.lens[2]);
            self.r_damp = flush(d * (1.0 - self.damping) + self.r_damp * self.damping);
            d = self.r_ap2.tick(self.r_damp * self.decay);
            self.r_d2.push(d);
            // Dattorro's output taps, left and right summed to mono
            let yl = self.r_d1.at(t266) + self.r_d1.at(t2974) - self.r_ap2.tap(t1913) + self.r_d2.at(t1996)
                - self.l_d1.at(t1990)
                - self.l_ap2.tap(t187)
                - self.l_d2.at(t1066);
            let yr = self.l_d1.at(t353) + self.l_d1.at(t3627) - self.l_ap2.tap(t1228) + self.l_d2.at(t2673)
                - self.r_d1.at(t2111)
                - self.r_ap2.tap(t335)
                - self.r_d2.at(t121);
            *v = 0.3 * (yl + yr) * self.wet.tick();
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        let (time, tone, pre, mix) = (v[0].max(0.2), unit10(v[1]), v[2], (v[3] / 100.0).clamp(0.0, 1.0));
        // one decay per trip round half the tank (about 0.36 s of delay at Dattorro's rate)
        let half = (672.0 + 4453.0 + 1800.0 + 3720.0) / 29761.0;
        self.decay = 10f32.powf(-3.0 * half / time).min(0.95);
        self.damping = 0.0005 + 0.65 * (1.0 - tone);
        self.pre_d = ((pre * 1e-3 * self.rate) as usize).max(1);
        self.lfo.set(self.rate, 1.0);
        self.wet.target = mix * 1.6;
    }
    fn reset(&mut self) {
        self.pre.clear();
        for d in &mut self.inp {
            d.clear();
        }
        self.l_ap1.clear();
        self.l_ap2.clear();
        self.r_ap1.clear();
        self.r_ap2.clear();
        self.l_d1.clear();
        self.l_d2.clear();
        self.r_d1.clear();
        self.r_d2.clear();
        self.l_damp = 0.0;
        self.r_damp = 0.0;
        self.bw_y = 0.0;
        self.wet.snap();
    }
    fn wet_only(&self) -> bool {
        true
    }
}

// -- Room and Hall (FDN) --------------------------------------------------------------------------------------------

const N: usize = 8;
const ROOM_MS: [f32; N] = [5.9, 7.1, 8.3, 9.7, 11.3, 12.9, 14.6, 16.7];
const HALL_MS: [f32; N] = [31.3, 36.7, 41.1, 45.9, 52.3, 57.1, 63.7, 71.9];
const EARLY: [(f32, f32); 6] = [
    (3.1, 0.5),
    (5.3, -0.42),
    (7.9, 0.36),
    (11.2, -0.3),
    (13.7, 0.25),
    (17.3, -0.2),
];

struct Fdn {
    rate: f32,
    hall: bool,
    lines: Vec<Line>,
    len: [f32; N],
    base: [f32; N],
    g: [f32; N],
    damp: [OnePole; N],
    lfo: [Lfo; 2],
    pre: Line,
    pre_d: usize,
    early: Line,
    early_taps: [(usize, f32); 6],
    diff: [Diffuser; 2],
    wet: Smooth,
    out_gain: f32,
}

fn fdn(rate: f32, hall: bool) -> Box<dyn Pedal> {
    let base = if hall { HALL_MS } else { ROOM_MS };
    let max = base.iter().fold(0.0f32, |a, b| a.max(*b)) * if hall { 1.0 } else { 1.9 };
    let ms = |m: f32| (m * 1e-3 * rate).round();
    Box::new(Fdn {
        rate,
        hall,
        lines: (0..N)
            .map(|_| Line::new((max * 1e-3 * rate) as usize + 64))
            .collect(),
        len: base.map(ms),
        base,
        g: [0.8; N],
        damp: [OnePole::new(rate, 6000.0); N],
        lfo: [Lfo::default(); 2],
        pre: Line::new((rate * 0.11) as usize),
        pre_d: 1,
        early: Line::new((rate * 0.04) as usize),
        early_taps: EARLY.map(|(m, g)| (ms(m) as usize, g)),
        diff: [
            Diffuser::new(ms(5.0) as usize + 8, ms(4.8), 0.7),
            Diffuser::new(ms(4.0) as usize + 8, ms(3.6), 0.7),
        ],
        wet: Smooth::new(rate, 30.0, 0.2),
        out_gain: 0.3,
    })
}

pub fn room(rate: f32) -> Box<dyn Pedal> {
    fdn(rate, false)
}

pub fn hall(rate: f32) -> Box<dyn Pedal> {
    fdn(rate, true)
}

impl Pedal for Fdn {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            self.pre.push(*v);
            let mut s = self.pre.at(self.pre_d);
            let mut early = 0.0;
            if self.hall {
                s = {
                    let t0 = self.diff[0].tick(s);
                    self.diff[1].tick(t0)
                };
            } else {
                self.early.push(s);
                for (d, g) in self.early_taps {
                    early += self.early.at(d) * g;
                }
            }
            let p0 = self.lfo[0].step();
            let p1 = self.lfo[1].step();
            let mut ys = [0.0f32; N];
            let mut sum = 0.0;
            for (i, y) in ys.iter_mut().enumerate() {
                *y = match i {
                    1 => self.lines[i].linear(self.len[i] + 6.0 * Lfo::sine(p0)),
                    5 => self.lines[i].linear(self.len[i] + 6.0 * Lfo::sine(p1)),
                    _ => self.lines[i].at(self.len[i] as usize),
                };
                sum += *y;
            }
            let mut out = 0.0;
            let k = 2.0 / N as f32;
            for i in 0..N {
                out += if i % 2 == 0 { ys[i] } else { -ys[i] };
                let fb = ys[i] - k * sum;
                let d = self.damp[i].lp(fb) * self.g[i];
                let inj = if i % 2 == 0 { s } else { -s };
                self.lines[i].push(inj * 0.5 + d);
            }
            *v = (out * self.out_gain + early * 0.6) * self.wet.tick();
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        let r = self.rate;
        let (rt60, tone, pre, mix) = if self.hall {
            for i in 0..N {
                self.len[i] = (self.base[i] * 1e-3 * r).round();
            }
            self.out_gain = 0.22;
            (v[0].max(0.3), unit10(v[1]), v[2], (v[3] / 100.0).clamp(0.0, 1.0))
        } else {
            let size = unit10(v[0]);
            let k = 0.6 + 1.3 * size;
            for i in 0..N {
                self.len[i] = (self.base[i] * k * 1e-3 * r).round();
            }
            for (j, (m, g)) in EARLY.iter().enumerate() {
                self.early_taps[j] = ((((m * k) * 1e-3 * r) as usize).max(1), *g);
            }
            self.out_gain = 0.3;
            (
                0.25 + 1.4 * size,
                unit10(v[1]),
                0.0,
                (v[2] / 100.0).clamp(0.0, 1.0),
            )
        };
        for i in 0..N {
            self.g[i] = 10f32.powf(-3.0 * self.len[i] / (r * rt60));
            self.damp[i].set(r, sweep(1800.0, 14000.0, tone));
        }
        self.lfo[0].set(r, 0.31);
        self.lfo[1].set(r, 0.47);
        self.pre_d = ((pre * 1e-3 * r) as usize).max(1);
        self.wet.target = mix * 1.5;
    }
    fn reset(&mut self) {
        for l in &mut self.lines {
            l.clear();
        }
        for d in &mut self.damp {
            d.reset();
        }
        for d in &mut self.diff {
            d.clear();
        }
        self.pre.clear();
        self.early.clear();
        self.wet.snap();
    }
    fn wet_only(&self) -> bool {
        true
    }
}
