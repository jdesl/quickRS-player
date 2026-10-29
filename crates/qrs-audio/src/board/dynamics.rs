//! Dynamics: compressors, a two-band bass compressor, a noise gate, a clean boost.
//!
//! The compressors follow Giannoulis, Massberg and Reiss, "Digital Dynamic Range Compressor Design: A Tutorial
//! and Analysis" (JAES 60(6), 2012): the gain computer works in dB with a soft knee, and the gain reduction is
//! smoothed by their "smooth decoupled" peak detector (release then attack), which keeps pumping low and the
//! attack honest.

use super::dsp::{Biquad, Filter, OnePole, Smooth, Svf, coef_ms, db_to_gain, flush, unit10};
use super::{MAX_KNOBS, Pedal};

/// The gain computer and its smoothing.
#[derive(Clone, Copy, Debug)]
struct Comp {
    rate: f32,
    thr: f32,
    ratio: f32,
    knee: f32,
    att: f32,
    rel: f32,
    y1: f32,
    yl: f32,
}

impl Comp {
    fn new(rate: f32) -> Comp {
        Comp {
            rate,
            thr: -20.0,
            ratio: 4.0,
            knee: 6.0,
            att: 0.0,
            rel: 0.0,
            y1: 0.0,
            yl: 0.0,
        }
    }

    fn times(&mut self, attack_ms: f32, release_ms: f32) {
        self.att = (-1.0 / (self.rate * attack_ms.max(0.05) * 1e-3)).exp();
        self.rel = (-1.0 / (self.rate * release_ms.max(1.0) * 1e-3)).exp();
    }

    /// Static curve: dB of gain reduction (≥ 0) for an input level in dB.
    #[inline]
    fn reduction(&self, xg: f32) -> f32 {
        let over = xg - self.thr;
        let slope = 1.0 - 1.0 / self.ratio.max(1.0);
        if 2.0 * over < -self.knee {
            0.0
        } else if 2.0 * over.abs() <= self.knee {
            slope * (over + self.knee / 2.0).powi(2) / (2.0 * self.knee)
        } else {
            slope * over
        }
    }

    /// The gain (linear) for this sample's level.
    #[inline]
    fn gain(&mut self, x: f32) -> f32 {
        let xg = 20.0 * (x.abs() + 1e-9).log10();
        let xl = self.reduction(xg);
        self.y1 = flush(xl.max(self.rel * self.y1 + (1.0 - self.rel) * xl));
        self.yl = flush(self.att * self.yl + (1.0 - self.att) * self.y1);
        db_to_gain(-self.yl)
    }

    fn reset(&mut self) {
        self.y1 = 0.0;
        self.yl = 0.0;
    }
}

// -- Squeeze: the Dyna Comp ------------------------------------------------------------------------------------------

struct Squeeze {
    rate: f32,
    comp: Comp,
    makeup: Smooth,
    tone: OnePole,
}

pub fn squeeze(rate: f32) -> Box<dyn Pedal> {
    Box::new(Squeeze {
        rate,
        comp: Comp::new(rate),
        makeup: Smooth::new(rate, 30.0, 1.0),
        tone: OnePole::new(rate, 9000.0),
    })
}

impl Pedal for Squeeze {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let g = self.comp.gain(*v);
            *v = self.tone.lp(*v * g) * self.makeup.tick();
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        let (sustain, attack, level) = (unit10(v[0]), unit10(v[1]), v[2]);
        // an OTA compressor: the more sustain, the lower the threshold and the higher the ratio
        self.comp.thr = -10.0 - 30.0 * sustain;
        self.comp.ratio = 2.0 + 14.0 * sustain;
        self.comp.knee = 8.0;
        // the attack knob lets more of the pick through (0.5 to 25 ms); release is program-dependent-ish
        self.comp
            .times(0.5 + 24.5 * attack * attack, 120.0 + 260.0 * sustain);
        let auto = -self.comp.thr * (1.0 - 1.0 / self.comp.ratio) * 0.45;
        self.makeup.target = db_to_gain(auto + (level - 5.0) * 2.4);
        let _ = self.rate;
    }
    fn reset(&mut self) {
        self.comp.reset();
        self.tone.reset();
        self.makeup.snap();
    }
}

// -- Studio Comp -----------------------------------------------------------------------------------------------------

struct Studio {
    comp: Comp,
    makeup: Smooth,
}

pub fn studio(rate: f32) -> Box<dyn Pedal> {
    Box::new(Studio {
        comp: Comp::new(rate),
        makeup: Smooth::new(rate, 30.0, 1.0),
    })
}

impl Pedal for Studio {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let g = self.comp.gain(*v);
            *v *= g * self.makeup.tick();
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        self.comp.thr = v[0];
        self.comp.ratio = v[1].max(1.0);
        self.comp.knee = 6.0;
        self.comp.times(v[2], v[3]);
        self.makeup.target = db_to_gain(v[4]);
    }
    fn reset(&mut self) {
        self.comp.reset();
        self.makeup.snap();
    }
}

// -- Bass Multiband: two bands split by a Linkwitz-Riley crossover -------------------------------------------------

struct Multiband {
    rate: f32,
    lo: [Biquad; 2],
    hi: [Biquad; 2],
    c_lo: Comp,
    c_hi: Comp,
    makeup: Smooth,
    xover: f32,
}

pub fn multiband(rate: f32) -> Box<dyn Pedal> {
    let mut m = Multiband {
        rate,
        lo: [Biquad::default(); 2],
        hi: [Biquad::default(); 2],
        c_lo: Comp::new(rate),
        c_hi: Comp::new(rate),
        makeup: Smooth::new(rate, 30.0, 1.0),
        xover: 0.0,
    };
    m.split(400.0);
    Box::new(m)
}

impl Multiband {
    fn split(&mut self, f: f32) {
        if (f - self.xover).abs() < 0.5 {
            return;
        }
        self.xover = f;
        // LR4: two Butterworth 2nd-order sections each side; the two outputs sum flat
        for b in &mut self.lo {
            b.set(
                self.rate,
                Filter::LowPass {
                    f,
                    q: std::f32::consts::FRAC_1_SQRT_2,
                },
            );
        }
        for b in &mut self.hi {
            b.set(
                self.rate,
                Filter::HighPass {
                    f,
                    q: std::f32::consts::FRAC_1_SQRT_2,
                },
            );
        }
    }
}

impl Pedal for Multiband {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let l = {
                let t0 = self.lo[0].tick(*v);
                self.lo[1].tick(t0)
            };
            let h = {
                let t0 = self.hi[0].tick(*v);
                self.hi[1].tick(t0)
            };
            let gl = self.c_lo.gain(l);
            let gh = self.c_hi.gain(h);
            *v = (l * gl + h * gh) * self.makeup.tick();
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        let (amount, speed, level) = (unit10(v[0]), unit10(v[2]), v[3]);
        self.split(v[1]);
        // the low band holds hard, the top lighter so the pick and fret noise stay alive
        self.c_lo.thr = -14.0 - 22.0 * amount;
        self.c_lo.ratio = 2.0 + 8.0 * amount;
        self.c_lo.knee = 8.0;
        self.c_hi.thr = -12.0 - 14.0 * amount;
        self.c_hi.ratio = 1.5 + 2.5 * amount;
        self.c_hi.knee = 8.0;
        let (a, r) = (25.0 - 22.0 * speed, 420.0 - 340.0 * speed);
        self.c_lo.times(a * 1.5, r * 1.3);
        self.c_hi.times(a * 0.5, r * 0.7);
        let auto = -self.c_lo.thr * (1.0 - 1.0 / self.c_lo.ratio) * 0.3;
        self.makeup.target = db_to_gain(auto + (level - 5.0) * 2.4);
    }
    fn reset(&mut self) {
        for b in self.lo.iter_mut().chain(self.hi.iter_mut()) {
            b.reset();
        }
        self.c_lo.reset();
        self.c_hi.reset();
        self.makeup.snap();
    }
}

// -- Noise Gate ------------------------------------------------------------------------------------------------------

struct Gate {
    rate: f32,
    env: f32,
    env_c: f32,
    open_thr: f32,
    close_thr: f32,
    hold: usize,
    held: usize,
    open: bool,
    gain: f32,
    att_c: f32,
    rel_c: f32,
    key: Svf,
}

pub fn gate(rate: f32) -> Box<dyn Pedal> {
    Box::new(Gate {
        rate,
        env: 0.0,
        env_c: coef_ms(rate, 2.0),
        open_thr: db_to_gain(-56.0),
        close_thr: db_to_gain(-62.0),
        hold: (rate * 0.03) as usize,
        held: 0,
        open: false,
        gain: 0.0,
        att_c: coef_ms(rate, 0.6),
        rel_c: coef_ms(rate, 80.0),
        key: Svf::new(rate, 800.0, 0.5),
    })
}

impl Pedal for Gate {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            // the key ignores the lowest hum: a band-pass around the guitar's body
            let k = self.key.tick(*v).bp;
            let a = k.abs().max(v.abs() * 0.5);
            self.env = flush(self.env + (a - self.env) * if a > self.env { 0.5 } else { self.env_c });
            if self.env > self.open_thr {
                self.open = true;
                self.held = self.hold;
            } else if self.env < self.close_thr {
                if self.held > 0 {
                    self.held -= 1;
                } else {
                    self.open = false;
                }
            }
            let target = if self.open { 1.0 } else { 0.0 };
            let c = if target > self.gain {
                self.att_c
            } else {
                self.rel_c
            };
            self.gain = flush(self.gain + (target - self.gain) * c);
            *v *= self.gain;
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        self.open_thr = db_to_gain(v[0]);
        self.close_thr = db_to_gain(v[0] - 6.0);
        self.rel_c = coef_ms(self.rate, v[1]);
    }
    fn reset(&mut self) {
        self.env = 0.0;
        self.gain = 0.0;
        self.open = false;
        self.key.reset();
    }
}

// -- Clean Boost -----------------------------------------------------------------------------------------------------

struct Boost {
    rate: f32,
    gain: Smooth,
    shelf: Biquad,
    bright: f32,
}

pub fn boost(rate: f32) -> Box<dyn Pedal> {
    Box::new(Boost {
        rate,
        gain: Smooth::new(rate, 30.0, 1.0),
        shelf: Biquad::new(rate, Filter::HighShelf { f: 2200.0, db: 0.0 }),
        bright: -1.0,
    })
}

impl Pedal for Boost {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let s = self.shelf.tick(*v) * self.gain.tick();
            // a JFET preamp's gentle rounding of the very tops
            *v = if s.abs() < 0.6 {
                s
            } else {
                (0.6 + 0.4 * ((s.abs() - 0.6) / 0.4).tanh()).copysign(s)
            };
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        self.gain.target = db_to_gain(v[0]);
        let b = unit10(v[1]) * 6.0;
        if (b - self.bright).abs() > 1e-3 {
            self.bright = b;
            self.shelf.set(self.rate, Filter::HighShelf { f: 2200.0, db: b });
        }
    }
    fn reset(&mut self) {
        self.shelf.reset();
        self.gain.snap();
    }
}
