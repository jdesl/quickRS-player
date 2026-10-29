//! Speaker cabinets as filters: the speaker's resonance in its box (a bigger bump for a closed back), the low-mid
//! dip, the upper-mid presence peak and cone break-up, and the steep roll-off above 4-5 kHz that makes an amp
//! sound like an amp rather than a fizzy preamp; then the microphone: its place on the cone (edge: darker, cap:
//! brighter), its distance (proximity bass when close, a floor reflection and less air when further), its type
//! (dynamic: a presence lift; ribbon: dark and smooth; condenser: open top), and low and high cuts.

use super::dsp::{Biquad, Filter, Line, unit10};
use super::{MAX_KNOBS, Pedal};

#[derive(Clone, Copy, Debug)]
struct Voice {
    /// The box's low cut (high-pass), the speaker's resonance and how much it bumps.
    hp: f32,
    res: f32,
    res_db: f32,
    /// The low-mid dip.
    lm: f32,
    lm_db: f32,
    /// Presence peak and cone break-up.
    pres: f32,
    pres_db: f32,
    bk: f32,
    bk_db: f32,
    /// Where the top falls away.
    lp: f32,
    trim_db: f32,
}

const C1X10: Voice = Voice {
    hp: 90.0,
    res: 110.0,
    res_db: 2.0,
    lm: 650.0,
    lm_db: -2.0,
    pres: 2200.0,
    pres_db: 4.0,
    bk: 3600.0,
    bk_db: -3.0,
    lp: 5200.0,
    trim_db: 1.0,
};
const C1X12: Voice = Voice {
    hp: 80.0,
    res: 100.0,
    res_db: 2.5,
    lm: 500.0,
    lm_db: -2.0,
    pres: 2500.0,
    pres_db: 4.0,
    bk: 3800.0,
    bk_db: -3.0,
    lp: 5000.0,
    trim_db: 1.0,
};
const C2X12: Voice = Voice {
    hp: 75.0,
    res: 95.0,
    res_db: 3.0,
    lm: 450.0,
    lm_db: -2.5,
    pres: 2700.0,
    pres_db: 4.5,
    bk: 4000.0,
    bk_db: -4.0,
    lp: 5300.0,
    trim_db: 0.5,
};
const C4X12: Voice = Voice {
    hp: 75.0,
    res: 90.0,
    res_db: 4.0,
    lm: 400.0,
    lm_db: -3.0,
    pres: 2900.0,
    pres_db: 5.0,
    bk: 4300.0,
    bk_db: -5.0,
    lp: 4800.0,
    trim_db: 0.0,
};
const C8X10: Voice = Voice {
    hp: 45.0,
    res: 70.0,
    res_db: 4.0,
    lm: 300.0,
    lm_db: -2.0,
    pres: 1800.0,
    pres_db: 3.0,
    bk: 3000.0,
    bk_db: -4.0,
    lp: 4000.0,
    trim_db: 0.0,
};
const C1X15: Voice = Voice {
    hp: 40.0,
    res: 60.0,
    res_db: 4.0,
    lm: 400.0,
    lm_db: -1.0,
    pres: 1200.0,
    pres_db: 2.0,
    bk: 2200.0,
    bk_db: -3.0,
    lp: 2800.0,
    trim_db: 1.0,
};

struct Cab {
    voice: Voice,
    rate: f32,
    f: [Biquad; 12],
    /// The floor reflection: a short delay mixed in as the mic moves away.
    refl: Line,
    refl_d: usize,
    refl_g: f32,
    trim: f32,
}

fn new(voice: Voice, rate: f32) -> Box<dyn Pedal> {
    Box::new(Cab {
        voice,
        rate,
        f: [Biquad::default(); 12],
        refl: Line::new((rate * 0.004) as usize + 4),
        refl_d: 1,
        refl_g: 0.0,
        trim: 1.0,
    })
}

/// The cab's response in dB at `f` (for the big view's picture).
pub fn response_db(id: &str, v: &[f32; MAX_KNOBS], rate: f32, f: f32) -> f32 {
    let voice = match id {
        "cab_1x10" => C1X10,
        "cab_1x12" => C1X12,
        "cab_2x12" => C2X12,
        "cab_4x12" => C4X12,
        "cab_8x10" => C8X10,
        _ => C1X15,
    };
    let mut g = 1.0f32;
    for flt in filters(voice, v, rate) {
        g *= Biquad::new(rate, flt).magnitude(f, rate);
    }
    20.0 * g.max(1e-6).log10() + voice.trim_db
}

pub fn c1x10(rate: f32) -> Box<dyn Pedal> {
    new(C1X10, rate)
}
pub fn c1x12(rate: f32) -> Box<dyn Pedal> {
    new(C1X12, rate)
}
pub fn c2x12(rate: f32) -> Box<dyn Pedal> {
    new(C2X12, rate)
}
pub fn c4x12(rate: f32) -> Box<dyn Pedal> {
    new(C4X12, rate)
}
pub fn c8x10(rate: f32) -> Box<dyn Pedal> {
    new(C8X10, rate)
}
pub fn c1x15(rate: f32) -> Box<dyn Pedal> {
    new(C1X15, rate)
}

/// The speaker, box and mic as filters, for these knobs.
fn filters(c: Voice, v: &[f32; MAX_KNOBS], r: f32) -> [Filter; 12] {
    let (pos, dist, mic, low_cut, high_cut) = (unit10(v[0]), unit10(v[1]), v[2].round() as i32, v[3], v[4]);
    // the mic's type
    let (mic_low, mic_pres, mic_air) = match mic {
        1 => (2.0, -1.0, -5.0), // ribbon: warm, dark, smooth
        2 => (1.0, 0.5, 3.0),   // condenser: flat, open top
        _ => (-1.5, 3.5, 0.0),  // dynamic: the SM57's presence lift and lighter lows
    };
    [
        Filter::HighPass { f: c.hp, q: 0.65 },
        Filter::Peak {
            f: c.res,
            q: 1.2,
            db: c.res_db,
        },
        Filter::Peak {
            f: c.lm,
            q: 0.9,
            db: c.lm_db,
        },
        // the cap of the cone is brighter than its edge
        Filter::Peak {
            f: c.pres,
            q: 1.0,
            db: c.pres_db + 4.0 * pos - 1.0,
        },
        Filter::Peak {
            f: c.bk,
            q: 3.0,
            db: c.bk_db,
        },
        Filter::LowPass {
            f: c.lp * (0.78 + 0.5 * pos),
            q: 0.85,
        },
        Filter::LowPass {
            f: c.lp * (0.95 + 0.6 * pos),
            q: 0.6,
        },
        // proximity effect up close, the mic's own colour
        Filter::LowShelf {
            f: 160.0,
            db: 5.0 * (1.0 - dist) - 1.0 + mic_low,
        },
        Filter::Peak {
            f: 5000.0,
            q: 1.2,
            db: mic_pres,
        },
        Filter::HighShelf {
            f: 6000.0,
            db: mic_air - 3.0 * dist,
        },
        Filter::HighPass {
            f: low_cut.clamp(15.0, 400.0),
            q: 0.7,
        },
        Filter::LowPass {
            f: high_cut.clamp(1500.0, r * 0.45),
            q: 0.7,
        },
    ]
}

impl Pedal for Cab {
    fn process(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            let mut s = *v;
            for b in &mut self.f {
                s = b.tick(s);
            }
            self.refl.push(s);
            *v = (s + self.refl.at(self.refl_d) * self.refl_g) * self.trim;
        }
    }

    fn set(&mut self, v: &[f32; MAX_KNOBS]) {
        let (r, dist) = (self.rate, unit10(v[1]));
        let fs = filters(self.voice, v, r);
        for (b, f) in self.f.iter_mut().zip(fs) {
            b.set(r, f);
        }
        // the floor bounce: 0.3 ms close up to 2.6 ms at arm's length, getting louder with distance
        self.refl_d = ((0.3 + 2.3 * dist) * 1e-3 * r).round().max(1.0) as usize;
        self.refl_g = 0.45 * dist;
        self.trim = 10f32.powf((self.voice.trim_db - 2.0 * dist) / 20.0) / (1.0 + self.refl_g * 0.5);
    }

    fn reset(&mut self) {
        for b in &mut self.f {
            b.reset();
        }
        self.refl.clear();
    }
}
