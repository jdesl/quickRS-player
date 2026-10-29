//! Filters: the wah, the envelope filter, lo-fi boxes and the acoustic simulator.
//!
//! The wah and the envelope filter sweep a trapezoidal state-variable filter (which stays stable however fast it
//! moves), recomputed every 8 samples. The Cry Baby's sweep runs about 400 Hz to 2.3 kHz with a resonant peak a
//! few dB over the input and the lows kept at a quarter, which is what makes it talk rather than just thin out.

use super::dsp::{Biquad, Filter, Line, OnePole, Smooth, Svf, coef_ms, db_to_gain, flush, sweep, unit10};
use super::{MAX_KNOBS, Pedal};

const STEP: usize = 8;

// -- Wah ------------------------------------------------------------------------------------------------------------

struct Wah {
    rate: f32,
    svf: Svf,
    pos: Smooth,
    manual: f32,
    auto: bool,
    sens: f32,
    env: f32,
    att: f32,
    rel: f32,
    count: usize,
    pos_now: f32,
}

pub fn wah(rate: f32) -> Box<dyn Pedal> {
    Box::new(Wah {
        rate,
        svf: Svf::new(rate, 800.0, 4.0),
        pos: Smooth::new(rate, 15.0, 0.5),
        manual: 0.5,
        auto: false,
        sens: 0.5,
        env: 0.0,
        att: coef_ms(rate, 4.0),
        rel: coef_ms(rate, 120.0),
        count: 0,
        pos_now: 0.5,
    })
}

impl Pedal for Wah {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let a = v.abs();
            self.env = flush(self.env + (a - self.env) * if a > self.env { self.att } else { self.rel });
            if self.count == 0 {
                self.pos.target = if self.auto {
                    (self.env * sweep(2.0, 40.0, self.sens)).clamp(0.0, 1.0)
                } else {
                    self.manual
                };
                self.pos_now = self.pos.skip(STEP);
                let p = self.pos_now;
                // narrower towards the toe, like the inductor's Q rising
                self.svf.set(self.rate, sweep(420.0, 2300.0, p), 3.0 + 3.5 * p);
                self.count = STEP;
            }
            self.count -= 1;
            let o = self.svf.tick(*v);
            // the band-pass is Q times the input at its centre: normalise it, then a +5 dB peak over the kept lows
            *v = o.lp * 0.35 + o.bp / (3.0 + 3.5 * self.pos_now) * 2.5;
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        self.manual = unit10(v[0]);
        self.auto = v[1] > 0.5;
        self.sens = unit10(v[2]);
        // speed: how fast the auto wah follows (slow: a lazy sweep, fast: a quack)
        self.rel = coef_ms(self.rate, sweep(400.0, 40.0, unit10(v[3])));
        self.att = coef_ms(self.rate, sweep(25.0, 2.0, unit10(v[3])));
    }
    fn reset(&mut self) {
        self.svf.reset();
        self.env = 0.0;
        self.pos.snap();
    }
}

// -- Envelope filter --------------------------------------------------------------------------------------------------

struct Envelope {
    rate: f32,
    svf: Svf,
    env: f32,
    att: f32,
    rel: f32,
    sens: f32,
    q: f32,
    bandpass: bool,
    count: usize,
}

pub fn envelope(rate: f32) -> Box<dyn Pedal> {
    Box::new(Envelope {
        rate,
        svf: Svf::new(rate, 300.0, 3.0),
        env: 0.0,
        att: coef_ms(rate, 8.0),
        rel: coef_ms(rate, 120.0),
        sens: 0.5,
        q: 3.0,
        bandpass: true,
        count: 0,
    })
}

impl Pedal for Envelope {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let a = v.abs();
            self.env = flush(self.env + (a - self.env) * if a > self.env { self.att } else { self.rel });
            if self.count == 0 {
                let u = (self.env * sweep(3.0, 60.0, self.sens)).clamp(0.0, 1.0);
                self.svf.set(self.rate, sweep(220.0, 3500.0, u), self.q);
                self.count = STEP;
            }
            self.count -= 1;
            let o = self.svf.tick(*v);
            *v = if self.bandpass {
                o.bp / self.q * 3.5 + o.lp * 0.45
            } else {
                o.lp * 1.8 / self.q.sqrt()
            };
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        self.sens = unit10(v[0]);
        self.q = 1.0 + 9.0 * unit10(v[1]);
        self.att = coef_ms(self.rate, sweep(1.0, 60.0, unit10(v[2])));
        self.rel = coef_ms(self.rate, sweep(30.0, 600.0, unit10(v[3])));
        self.bandpass = v[4] > 0.5;
    }
    fn reset(&mut self) {
        self.svf.reset();
        self.env = 0.0;
    }
}

// -- Lo-Fi ----------------------------------------------------------------------------------------------------------

struct LoFi {
    rate: f32,
    kind: i32,
    f: [Biquad; 4],
    mix: Smooth,
    drive: f32,
    hold: f32,
    held: f32,
    phase: f32,
    bits: f32,
}

pub fn lofi(rate: f32) -> Box<dyn Pedal> {
    Box::new(LoFi {
        rate,
        kind: 0,
        f: [Biquad::default(); 4],
        mix: Smooth::new(rate, 30.0, 1.0),
        drive: 1.0,
        hold: 1.0,
        held: 0.0,
        phase: 0.0,
        bits: 8.0,
    })
}

impl Pedal for LoFi {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let dry = *v;
            let mut s = dry;
            if self.kind == 2 {
                // sample-and-hold at a lower rate, then fewer bits
                self.phase += 1.0;
                if self.phase >= self.hold {
                    self.phase -= self.hold;
                    let q = 2f32.powf(self.bits - 1.0);
                    self.held = (s * self.drive * q).round() / q;
                }
                s = self.held;
            } else {
                s = (s * self.drive).tanh() / self.drive.sqrt();
            }
            for b in &mut self.f {
                s = b.tick(s);
            }
            let m = self.mix.tick();
            *v = dry * (1.0 - m) + s * m;
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        self.kind = v[0].round() as i32;
        self.mix.target = (v[1] / 100.0).clamp(0.0, 1.0);
        let d = unit10(v[2]);
        let r = self.rate;
        let fs = match self.kind {
            // an old radio: 250 Hz to 3.5 kHz, a little boxy
            0 => [
                Filter::HighPass { f: 250.0, q: 0.8 },
                Filter::LowPass { f: 3500.0, q: 0.9 },
                Filter::Peak {
                    f: 1200.0,
                    q: 1.2,
                    db: 4.0,
                },
                Filter::Peak {
                    f: 1.0e4,
                    q: 1.0,
                    db: 0.0,
                },
            ],
            // a telephone line: narrower and honkier
            1 => [
                Filter::HighPass { f: 500.0, q: 1.0 },
                Filter::LowPass { f: 2700.0, q: 1.2 },
                Filter::Peak {
                    f: 1500.0,
                    q: 1.5,
                    db: 6.0,
                },
                Filter::HighPass { f: 400.0, q: 0.7 },
            ],
            // a crusher: keep the aliasing, soften the top a bit
            _ => [
                Filter::LowPass { f: 9000.0, q: 0.7 },
                Filter::HighPass { f: 40.0, q: 0.7 },
                Filter::Peak {
                    f: 1.0e4,
                    q: 1.0,
                    db: 0.0,
                },
                Filter::Peak {
                    f: 1.0e4,
                    q: 1.0,
                    db: 0.0,
                },
            ],
        };
        for (b, f) in self.f.iter_mut().zip(fs) {
            b.set(r, f);
        }
        if self.kind == 2 {
            self.hold = 1.0 + 7.0 * d;
            self.bits = 12.0 - 7.0 * d;
            self.drive = 1.0;
        } else {
            self.drive = 1.0 + 12.0 * d;
        }
    }
    fn reset(&mut self) {
        for b in &mut self.f {
            b.reset();
        }
        self.held = 0.0;
        self.mix.snap();
    }
}

// -- Acoustic simulator ---------------------------------------------------------------------------------------------

struct Acoustic {
    rate: f32,
    f: [Biquad; 6],
    body: Line,
    taps: [(usize, f32); 3],
    level: Smooth,
    hp: OnePole,
}

pub fn acoustic(rate: f32) -> Box<dyn Pedal> {
    let ms = |m: f32| (m * 1e-3 * rate).round() as usize;
    Box::new(Acoustic {
        rate,
        f: [Biquad::default(); 6],
        body: Line::new(ms(5.0) + 4),
        taps: [(ms(0.83), 0.22), (ms(1.9), -0.15), (ms(3.1), 0.1)],
        level: Smooth::new(rate, 30.0, 1.0),
        hp: OnePole::new(rate, 60.0),
    })
}

impl Pedal for Acoustic {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let mut s = self.hp.hp(*v);
            for b in &mut self.f {
                s = b.tick(s);
            }
            // a wooden box's first reflections
            self.body.push(s);
            let mut y = s;
            for (d, g) in self.taps {
                y += self.body.at(d) * g;
            }
            *v = y * self.level.tick();
        }
    }
    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        let (body, top, lvl) = (unit10(v[0]), unit10(v[1]), v[2]);
        let r = self.rate;
        let fs = [
            Filter::Peak {
                f: 115.0,
                q: 1.3,
                db: 9.0 * body,
            },
            Filter::Peak {
                f: 220.0,
                q: 1.0,
                db: 2.0 * body,
            },
            // the magnetic pickup's mud and its resonance, taken out
            Filter::Peak {
                f: 420.0,
                q: 0.9,
                db: -5.0,
            },
            Filter::Peak {
                f: 1300.0,
                q: 1.4,
                db: -4.0,
            },
            Filter::HighShelf {
                f: 3800.0,
                db: 2.0 + 9.0 * top,
            },
            Filter::LowPass { f: 15000.0, q: 0.7 },
        ];
        for (b, f) in self.f.iter_mut().zip(fs) {
            b.set(r, f);
        }
        self.level.target = db_to_gain((lvl - 5.0) * 2.4 - 2.0);
    }
    fn reset(&mut self) {
        for b in &mut self.f {
            b.reset();
        }
        self.body.clear();
        self.hp.reset();
        self.level.snap();
    }
}
