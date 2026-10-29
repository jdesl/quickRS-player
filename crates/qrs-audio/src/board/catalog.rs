//! Every model on the board: its kind, name, the classic it's modelled after, its knobs (key, label, range,
//! default, unit), the colour of its enclosure, how its face is drawn, and the DSP that runs it. Ids and knob keys
//! are what config.json and userdata.json store, so they never change; new models and knobs go at the end of their
//! kind.

use super::{MAX_KNOBS, Pedal, amp, cab, delay, drive, dynamics, eq, filter, modfx, reverb};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Dynamics,
    Drive,
    Fuzz,
    Amp,
    Cab,
    Eq,
    Filter,
    Modulation,
    Delay,
    Reverb,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Dynamics,
        Kind::Drive,
        Kind::Fuzz,
        Kind::Amp,
        Kind::Cab,
        Kind::Eq,
        Kind::Filter,
        Kind::Modulation,
        Kind::Delay,
        Kind::Reverb,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Dynamics => "Dynamics",
            Kind::Drive => "Drive",
            Kind::Fuzz => "Fuzz",
            Kind::Amp => "Amp",
            Kind::Cab => "Cab",
            Kind::Eq => "EQ",
            Kind::Filter => "Filter",
            Kind::Modulation => "Modulation",
            Kind::Delay => "Delay",
            Kind::Reverb => "Reverb",
        }
    }

    /// The kind's colour (its tab in the browser).
    pub fn colour(self) -> [u8; 3] {
        match self {
            Kind::Dynamics => [45, 99, 184],
            Kind::Drive => [63, 138, 74],
            Kind::Fuzz => [196, 48, 60],
            Kind::Amp => [201, 166, 78],
            Kind::Cab => [143, 132, 102],
            Kind::Eq => [210, 210, 206],
            Kind::Filter => [122, 82, 199],
            Kind::Modulation => [108, 184, 226],
            Kind::Delay => [47, 107, 74],
            Kind::Reverb => [74, 106, 156],
        }
    }
}

/// How a model's face is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// A stompbox: knobs, name, LED, footswitch.
    Pedal,
    /// An amp's control panel on tolex.
    Amp,
    /// A cabinet's grille with the mic in front of a speaker.
    Cab,
    /// A graphic EQ: slider faders.
    Graphic,
    /// A rack unit: brushed metal, small knobs, a readout.
    Rack,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unit {
    /// A pedal knob's 0 to 10.
    Dial,
    Db,
    Ms,
    Hz,
    Pct,
    Sec,
    Ratio,
    /// A switch: the value is the index of one of these.
    Choice(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug)]
pub struct KnobDef {
    pub key: &'static str,
    pub label: &'static str,
    pub lo: f32,
    pub hi: f32,
    pub def: f32,
    pub unit: Unit,
    /// Turns on a log scale (frequencies, times, ratios).
    pub log: bool,
    /// Behind the panel: a control the real unit hides inside or on its back (an amp's sag, bias, tubes; a drive's
    /// clipping diodes). Shown only in the editor's Back panel / Inside section, never on the face. Back knobs come
    /// after all the front ones (D45).
    pub back: bool,
}

impl KnobDef {
    /// The value as the knob's turn, 0..1.
    pub fn to_u(&self, v: f32) -> f32 {
        let v = v.clamp(self.lo, self.hi);
        if self.hi <= self.lo {
            return 0.0;
        }
        if self.log && self.lo > 0.0 {
            (v / self.lo).ln() / (self.hi / self.lo).ln()
        } else {
            (v - self.lo) / (self.hi - self.lo)
        }
    }

    /// The value at a turn of `u` (0..1); switches snap to a position.
    pub fn from_u(&self, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        let v = if self.log && self.lo > 0.0 {
            self.lo * (self.hi / self.lo).powf(u)
        } else {
            self.lo + u * (self.hi - self.lo)
        };
        self.clamp(v)
    }

    pub fn clamp(&self, v: f32) -> f32 {
        let v = if v.is_finite() { v } else { self.def };
        match self.unit {
            Unit::Choice(c) => v.round().clamp(0.0, (c.len().max(1) - 1) as f32),
            _ => v.clamp(self.lo, self.hi),
        }
    }

    /// The value as a person reads it ("6.5", "+3.0 dB", "450 ms", "1.2 kHz", "Ribbon").
    pub fn text(&self, v: f32) -> String {
        match self.unit {
            Unit::Dial => format!("{v:.1}"),
            Unit::Db => {
                if v.abs() < 0.05 {
                    "0 dB".into()
                } else {
                    format!("{}{:.1} dB", if v > 0.0 { "+" } else { "\u{2212}" }, v.abs())
                }
            }
            Unit::Ms => {
                if v < 10.0 {
                    format!("{v:.1} ms")
                } else {
                    format!("{v:.0} ms")
                }
            }
            Unit::Hz => {
                if v >= 1000.0 {
                    format!("{:.1} kHz", v / 1000.0)
                } else {
                    format!("{v:.0} Hz")
                }
            }
            Unit::Pct => format!("{v:.0}%"),
            Unit::Sec => format!("{v:.1} s"),
            Unit::Ratio => format!("{v:.1}:1"),
            Unit::Choice(c) => c
                .get(v.round().max(0.0) as usize)
                .copied()
                .unwrap_or("")
                .to_string(),
        }
    }

    /// Values either side of a centre (EQ gains): the knob's lit ticks run from there.
    pub fn centred(&self) -> bool {
        self.lo < 0.0 && self.hi > 0.0
    }
}

pub struct ModelDef {
    pub id: &'static str,
    pub kind: Kind,
    pub name: &'static str,
    /// Up to four letters for the chain strip.
    pub short: &'static str,
    /// The classic it's modelled after.
    pub after: &'static str,
    /// One line on what it does to the sound.
    pub blurb: &'static str,
    pub colour: [u8; 3],
    pub face: Face,
    pub knobs: &'static [KnobDef],
    pub make: fn(f32) -> Box<dyn Pedal>,
}

impl ModelDef {
    pub fn defaults(&self) -> [f32; MAX_KNOBS] {
        let mut v = [0.0; MAX_KNOBS];
        for (o, k) in v.iter_mut().zip(self.knobs) {
            *o = k.def;
        }
        v
    }

    pub fn knob_index(&self, key: &str) -> Option<usize> {
        self.knobs.iter().position(|k| k.key == key)
    }

    /// The knobs on the face (all but the back-panel ones, which come last: indices are the same as `knobs`).
    pub fn front_knobs(&self) -> &'static [KnobDef] {
        &self.knobs[..self.front_len()]
    }

    /// How many knobs are on the face; knobs from here on are behind the panel.
    pub fn front_len(&self) -> usize {
        self.knobs.iter().take_while(|k| !k.back).count()
    }

    /// The knobs behind the panel, with their indices in `knobs`.
    pub fn back_knobs(&self) -> impl Iterator<Item = (usize, &'static KnobDef)> {
        let knobs: &'static [KnobDef] = self.knobs;
        knobs.iter().enumerate().skip(self.front_len())
    }

    pub fn has_back(&self) -> bool {
        self.knobs.iter().any(|k| k.back)
    }
}

const fn dial(key: &'static str, label: &'static str, def: f32) -> KnobDef {
    KnobDef {
        key,
        label,
        lo: 0.0,
        hi: 10.0,
        def,
        unit: Unit::Dial,
        log: false,
        back: false,
    }
}

const fn db(key: &'static str, label: &'static str, lo: f32, hi: f32, def: f32) -> KnobDef {
    KnobDef {
        key,
        label,
        lo,
        hi,
        def,
        unit: Unit::Db,
        log: false,
        back: false,
    }
}

const fn ms(key: &'static str, label: &'static str, lo: f32, hi: f32, def: f32) -> KnobDef {
    KnobDef {
        key,
        label,
        lo,
        hi,
        def,
        unit: Unit::Ms,
        log: true,
        back: false,
    }
}

const fn hz(key: &'static str, label: &'static str, lo: f32, hi: f32, def: f32) -> KnobDef {
    KnobDef {
        key,
        label,
        lo,
        hi,
        def,
        unit: Unit::Hz,
        log: true,
        back: false,
    }
}

const fn pct(key: &'static str, label: &'static str, def: f32) -> KnobDef {
    KnobDef {
        key,
        label,
        lo: 0.0,
        hi: 100.0,
        def,
        unit: Unit::Pct,
        log: false,
        back: false,
    }
}

const fn sec(key: &'static str, label: &'static str, lo: f32, hi: f32, def: f32) -> KnobDef {
    KnobDef {
        key,
        label,
        lo,
        hi,
        def,
        unit: Unit::Sec,
        log: true,
        back: false,
    }
}

const fn choice(key: &'static str, label: &'static str, opts: &'static [&'static str], def: f32) -> KnobDef {
    KnobDef {
        key,
        label,
        lo: 0.0,
        hi: (opts.len() - 1) as f32,
        def,
        unit: Unit::Choice(opts),
        log: false,
        back: false,
    }
}

/// The same knob, behind the panel (D45).
const fn back(k: KnobDef) -> KnobDef {
    KnobDef { back: true, ..k }
}

/// Power tubes an amp's back panel offers (`tubes`): each voices the power stage its own way.
pub const TUBES: &[&str] = &["EL34", "6L6", "KT88", "EL84", "6V6"];
pub const RECTIFIERS: &[&str] = &["Tube", "Solid"];
/// A drive's clipping diodes (`diodes`, inside the box).
pub const DIODES: &[&str] = &["Silicon", "Asymmetric", "LED", "Germanium"];
/// The tape echo's playback heads (`heads`): head 1, 2 and 3 at 1x, 2x and 3x the Time, in the order the
/// console's head selector prints them.
pub const HEADS: &[&str] = &["1", "2", "3", "1+2", "1+3", "2+3", "1+2+3"];
/// `HEADS` as bits (bit k: head k + 1).
pub const HEAD_BITS: [u8; 7] = [1, 2, 4, 3, 5, 6, 7];

/// An amp's knobs: its front panel, then its back panel (sag, bias, power tubes, rectifier, power master) at its
/// real unit's settings: sag 0..10, tube and rectifier indices into `TUBES` and `RECTIFIERS`. Bias and power master
/// sit at noon (neutral).
macro_rules! amp_knobs {
    ([$($front:expr),* $(,)?], $sag:expr, $tube:expr, $rect:expr) => {
        &[
            $($front,)*
            back(dial("sag", "Sag", $sag)),
            back(dial("bias", "Bias", 5.0)),
            back(choice("tubes", "Power tubes", TUBES, $tube)),
            back(choice("rectifier", "Rectifier", RECTIFIERS, $rect)),
            back(dial("power_master", "Power master", 5.0)),
        ]
    };
}

const fn diodes(def: f32) -> KnobDef {
    back(choice("diodes", "Diodes", DIODES, def))
}

const EQ: f32 = 15.0;
const MICS: &[&str] = &["Dynamic", "Ribbon", "Condenser"];
const CAB_KNOBS: &[KnobDef] = &[
    dial("mic", "Mic", 4.0),
    dial("distance", "Distance", 2.0),
    choice("mic_type", "Mic type", MICS, 0.0),
    hz("low_cut", "Low cut", 20.0, 300.0, 40.0),
    hz("high_cut", "High cut", 2500.0, 16000.0, 12000.0),
];

macro_rules! model {
    ($id:expr, $kind:ident, $name:expr, $short:expr, $after:expr, $blurb:expr, $rgb:expr, $face:ident, $knobs:expr, $make:path) => {
        ModelDef {
            id: $id,
            kind: Kind::$kind,
            name: $name,
            short: $short,
            after: $after,
            blurb: $blurb,
            colour: $rgb,
            face: Face::$face,
            knobs: $knobs,
            make: $make,
        }
    };
}

pub static MODELS: &[ModelDef] = &[
    // -- dynamics
    model!(
        "squeeze",
        Dynamics,
        "Squeeze",
        "COMP",
        "MXR Dyna Comp",
        "Squashes the attack and stretches the sustain: country chicken-picking, funk, clean leads.",
        [200, 32, 42],
        Pedal,
        &[
            dial("sustain", "Sustain", 5.0),
            dial("attack", "Attack", 5.0),
            dial("level", "Level", 5.0)
        ],
        dynamics::squeeze
    ),
    model!(
        "studio_comp",
        Dynamics,
        "Needle Comp",
        "SCMP",
        "a studio rack compressor",
        "A clean feed-forward compressor with every control: evens out strumming without colouring it.",
        [154, 161, 169],
        Rack,
        &[
            db("threshold", "Threshold", -40.0, 0.0, -18.0),
            KnobDef {
                key: "ratio",
                label: "Ratio",
                lo: 1.0,
                hi: 20.0,
                def: 4.0,
                unit: Unit::Ratio,
                log: true,
                back: false
            },
            ms("attack", "Attack", 0.1, 100.0, 10.0),
            ms("release", "Release", 20.0, 1000.0, 150.0),
            db("makeup", "Makeup", 0.0, 24.0, 4.0)
        ],
        dynamics::studio
    ),
    model!(
        "bass_comp",
        Dynamics,
        "Low Lock",
        "MBC",
        "a multiband bass compressor",
        "Holds the low end steady and lets the pick attack through: the bass part's best friend.",
        [57, 66, 77],
        Pedal,
        &[
            dial("compress", "Compress", 5.0),
            hz("crossover", "Crossover", 100.0, 1000.0, 400.0),
            dial("speed", "Speed", 5.0),
            dial("level", "Level", 5.0)
        ],
        dynamics::multiband
    ),
    model!(
        "gate",
        Dynamics,
        "Shush",
        "GATE",
        "Boss NS-2",
        "Silences hiss and hum between notes; set it just above the noise with the guitar still.",
        [237, 237, 234],
        Pedal,
        &[
            db("threshold", "Threshold", -80.0, -20.0, -56.0),
            ms("release", "Release", 10.0, 500.0, 80.0)
        ],
        dynamics::gate
    ),
    model!(
        "boost",
        Dynamics,
        "Echo Pre",
        "BST",
        "Xotic EP Booster",
        "Up to 20 dB of clean gain with a little sparkle: pushes an amp into breakup for solos.",
        [217, 211, 196],
        Pedal,
        &[db("gain", "Gain", 0.0, 20.0, 6.0), dial("bright", "Bright", 5.0)],
        dynamics::boost
    ),
    // -- drive
    model!(
        "green",
        Drive,
        "Mean Green",
        "TS",
        "Ibanez TS808",
        "Mid-hump overdrive that keeps the bass clean: tightens a crunchy amp, smooths a lead.",
        [63, 138, 74],
        Pedal,
        &[
            dial("drive", "Drive", 5.0),
            dial("tone", "Tone", 5.0),
            dial("level", "Level", 5.0),
            diodes(0.0)
        ],
        drive::green
    ),
    model!(
        "blues",
        Drive,
        "Delta Blue",
        "BD",
        "Boss BD-2",
        "Open, full-range overdrive that cleans up when you play softly.",
        [45, 99, 184],
        Pedal,
        &[
            dial("gain", "Gain", 5.0),
            dial("tone", "Tone", 5.0),
            dial("level", "Level", 5.0),
            diodes(1.0)
        ],
        drive::blues
    ),
    model!(
        "gold",
        Drive,
        "Gold Pegasus",
        "KLON",
        "Klon Centaur",
        "Transparent drive with the clean signal blended back in: thicker, not different.",
        [201, 165, 76],
        Pedal,
        &[
            dial("gain", "Gain", 4.0),
            dial("treble", "Treble", 5.0),
            dial("output", "Output", 6.0),
            diodes(3.0)
        ],
        drive::gold
    ),
    model!(
        "germanium",
        Drive,
        "Copper Top",
        "GE",
        "a germanium-diode overdrive",
        "Soft, warm, slightly lopsided clipping: vintage rock rhythm.",
        [180, 90, 46],
        Pedal,
        &[
            dial("gain", "Gain", 5.0),
            dial("tone", "Tone", 5.0),
            dial("level", "Level", 5.0),
            diodes(3.0)
        ],
        drive::germanium
    ),
    model!(
        "treble_boost",
        Drive,
        "Top Shelf",
        "TB",
        "Dallas Rangemaster",
        "Boosts the top end into the amp: the cutting 60s British lead sound.",
        [141, 147, 155],
        Pedal,
        &[dial("boost", "Boost", 6.0), dial("range", "Range", 5.0)],
        drive::treble_boost
    ),
    model!(
        "orange",
        Drive,
        "Tangerine Dist",
        "DS",
        "Boss DS-1",
        "Hard-clipped distortion with a scooped tone: punk, grunge, classic rock.",
        [239, 108, 26],
        Pedal,
        &[
            dial("dist", "Dist", 6.0),
            dial("tone", "Tone", 5.0),
            dial("level", "Level", 5.0),
            diodes(0.0)
        ],
        drive::orange
    ),
    model!(
        "rodent",
        Drive,
        "Rodent",
        "RAT",
        "ProCo RAT",
        "Gritty op-amp distortion; the Filter knob turns it darker.",
        [29, 30, 33],
        Pedal,
        &[
            dial("distortion", "Distortion", 6.0),
            dial("filter", "Filter", 4.0),
            dial("volume", "Volume", 5.0),
            diodes(0.0)
        ],
        drive::rodent
    ),
    model!(
        "metal",
        Drive,
        "Molten",
        "MT",
        "Boss MT-2",
        "Two stages of high gain with a powerful EQ: modern metal from a clean amp.",
        [42, 43, 46],
        Pedal,
        &[
            dial("gain", "Gain", 7.0),
            db("low", "Low", -12.0, 12.0, 3.0),
            db("mid", "Mid", -12.0, 12.0, -4.0),
            db("high", "High", -12.0, 12.0, 2.0),
            dial("level", "Level", 5.0)
        ],
        drive::metal
    ),
    model!(
        "bass_drive",
        Drive,
        "Bass Pilot",
        "BDI",
        "SansAmp Bass Driver DI",
        "Tube-style bass grit blended with the clean bass, so the low end stays solid.",
        [185, 188, 191],
        Pedal,
        &[
            dial("gain", "Gain", 4.0),
            pct("blend", "Blend", 60.0),
            dial("tone", "Tone", 5.0),
            dial("level", "Level", 5.0)
        ],
        drive::bass_drive
    ),
    // -- fuzz
    model!(
        "round_fuzz",
        Fuzz,
        "Grin Fuzz",
        "FF",
        "Dallas Arbiter Fuzz Face",
        "Thick, woolly fuzz that cleans up when you turn the guitar down: Hendrix, Gilmour.",
        [196, 48, 60],
        Pedal,
        &[dial("fuzz", "Fuzz", 8.0), dial("volume", "Volume", 5.0)],
        drive::round_fuzz
    ),
    model!(
        "big_pi",
        Fuzz,
        "Wall of Wool",
        "MUFF",
        "Electro-Harmonix Big Muff Pi",
        "Huge, sustaining fuzz with a scooped mid: Smashing Pumpkins, the White Stripes.",
        [195, 198, 202],
        Pedal,
        &[
            dial("sustain", "Sustain", 7.0),
            dial("tone", "Tone", 5.0),
            dial("volume", "Volume", 5.0)
        ],
        drive::big_pi
    ),
    model!(
        "bender",
        Fuzz,
        "Carnaby Fuzz",
        "TBND",
        "Sola Sound Tone Bender MkII",
        "Raspy, mid-heavy three-transistor fuzz: 60s British rock.",
        [107, 112, 120],
        Pedal,
        &[dial("attack", "Attack", 7.0), dial("level", "Level", 5.0)],
        drive::bender
    ),
    model!(
        "octave_fuzz",
        Fuzz,
        "Rocket Octave",
        "OCTF",
        "Tycobrahe Octavia",
        "Rectified fuzz that adds an octave up: best on the neck pickup above the 12th fret.",
        [108, 62, 166],
        Pedal,
        &[dial("fuzz", "Fuzz", 7.0), dial("volume", "Volume", 5.0)],
        drive::octave_fuzz
    ),
    // -- amps
    model!(
        "amp_clean",
        Amp,
        "Gemini Reverb",
        "AMP",
        "Fender Twin Reverb",
        "Huge clean headroom, sparkly highs, scooped mids: the clean sound most songs use.",
        [27, 27, 29],
        Amp,
        amp_knobs!(
            [
                dial("gain", "Gain", 4.0),
                dial("bass", "Bass", 5.0),
                dial("mid", "Mid", 5.0),
                dial("treble", "Treble", 6.0),
                choice("bright", "Bright", &["Off", "On"], 0.0),
                dial("master", "Master", 5.0)
            ],
            2.5,
            1.0,
            1.0
        ),
        amp::clean
    ),
    model!(
        "amp_tweed",
        Amp,
        "Tweedle Dee",
        "AMP",
        "Fender 5E3 Deluxe",
        "A small amp turned up: warm, compressed, breaking up as you dig in.",
        [199, 164, 106],
        Amp,
        amp_knobs!(
            [
                dial("volume", "Volume", 5.0),
                dial("tone", "Tone", 6.0),
                dial("master", "Master", 5.0)
            ],
            6.0,
            4.0,
            0.0
        ),
        amp::tweed
    ),
    model!(
        "amp_chime",
        Amp,
        "Dartford 30",
        "AMP",
        "Vox AC30 Top Boost",
        "Bright, chimey class A jangle that turns into a rich crunch: the Beatles, U2, Radiohead.",
        [140, 47, 43],
        Amp,
        amp_knobs!(
            [
                dial("gain", "Gain", 5.0),
                dial("treble", "Treble", 6.0),
                dial("bass", "Bass", 5.0),
                dial("cut", "Cut", 3.0),
                dial("master", "Master", 5.0)
            ],
            3.5,
            3.0,
            0.0
        ),
        amp::chime
    ),
    model!(
        "amp_crunch",
        Amp,
        "Perspex 100",
        "AMP",
        "Marshall Super Lead (Plexi)",
        "Classic rock crunch: AC/DC, Led Zeppelin, Van Halen.",
        [201, 166, 78],
        Amp,
        amp_knobs!(
            [
                dial("gain", "Gain", 6.0),
                dial("bass", "Bass", 5.0),
                dial("mid", "Mid", 6.0),
                dial("treble", "Treble", 6.0),
                dial("presence", "Presence", 5.0),
                dial("master", "Master", 5.0)
            ],
            5.0,
            0.0,
            1.0
        ),
        amp::crunch
    ),
    model!(
        "amp_lead",
        Amp,
        "Bletchley 800",
        "AMP",
        "Marshall JCM800 2203",
        "Tight, aggressive 80s rock and metal rhythm, singing leads.",
        [201, 166, 78],
        Amp,
        amp_knobs!(
            [
                dial("gain", "Gain", 6.0),
                dial("bass", "Bass", 5.0),
                dial("mid", "Mid", 6.0),
                dial("treble", "Treble", 6.0),
                dial("presence", "Presence", 5.0),
                dial("master", "Master", 5.0)
            ],
            4.0,
            0.0,
            1.0
        ),
        amp::lead
    ),
    model!(
        "amp_modern",
        Amp,
        "Petaluma Plate",
        "AMP",
        "Mesa/Boogie Dual Rectifier",
        "Saturated, tight modern metal with a deep low end: Metallica to Mastodon.",
        [174, 179, 186],
        Amp,
        amp_knobs!(
            [
                dial("gain", "Gain", 7.0),
                dial("bass", "Bass", 6.0),
                dial("mid", "Mid", 4.0),
                dial("treble", "Treble", 6.0),
                dial("presence", "Presence", 5.0),
                dial("depth", "Depth", 5.0),
                dial("master", "Master", 5.0)
            ],
            6.0,
            1.0,
            1.0
        ),
        amp::modern
    ),
    model!(
        "amp_bass",
        Amp,
        "Blue Line 300",
        "BASS",
        "Ampeg SVT",
        "The rock bass amp: warm tube lows and a sweepable mid.",
        [184, 193, 204],
        Amp,
        amp_knobs!(
            [
                dial("gain", "Gain", 4.0),
                dial("bass", "Bass", 6.0),
                dial("mid", "Mid", 5.0),
                hz("freq", "Freq", 200.0, 3000.0, 800.0),
                dial("treble", "Treble", 5.0),
                dial("master", "Master", 5.0)
            ],
            3.5,
            2.0,
            1.0
        ),
        amp::bass
    ),
    // -- cabs
    model!(
        "cab_1x10",
        Cab,
        "Bedroom 10",
        "1x10",
        "Fender Princeton 1x10",
        "Small, boxy, focused: practice amps and vintage blues.",
        [185, 171, 132],
        Cab,
        CAB_KNOBS,
        cab::c1x10
    ),
    model!(
        "cab_1x12",
        Cab,
        "Linen 12",
        "1x12",
        "Fender Deluxe 1x12",
        "Open-back and airy, with a soft low end.",
        [185, 171, 132],
        Cab,
        CAB_KNOBS,
        cab::c1x12
    ),
    model!(
        "cab_2x12",
        Cab,
        "Open 2x12",
        "2x12",
        "an open-back 2x12 combo",
        "Wide and bright: the Twin and the AC30's speakers.",
        [185, 171, 132],
        Cab,
        CAB_KNOBS,
        cab::c2x12
    ),
    model!(
        "cab_4x12",
        Cab,
        "Slant Stack 412",
        "4x12",
        "Marshall 1960A 4x12",
        "Closed-back thump and a forward upper mid: the rock and metal cab.",
        [62, 59, 51],
        Cab,
        CAB_KNOBS,
        cab::c4x12
    ),
    model!(
        "cab_8x10",
        Cab,
        "The Fridge",
        "8x10",
        "Ampeg SVT-810E",
        "The fridge: deep, punchy bass with a growly upper mid.",
        [43, 45, 49],
        Cab,
        CAB_KNOBS,
        cab::c8x10
    ),
    model!(
        "cab_1x15",
        Cab,
        "Flip-Top 15",
        "1x15",
        "a vintage bass 1x15",
        "Round, warm, old-school bass with little top.",
        [43, 45, 49],
        Cab,
        CAB_KNOBS,
        cab::c1x15
    ),
    // -- EQ
    model!(
        "eq5",
        Eq,
        "V-Shape 5",
        "EQ5",
        "a 5-band graphic EQ",
        "Five sliders for broad shaping (63 Hz to 5.7 kHz).",
        [42, 42, 42],
        Graphic,
        &[
            db("b63", "63", -EQ, EQ, 0.0),
            db("b250", "250", -EQ, EQ, 0.0),
            db("b750", "750", -EQ, EQ, 0.0),
            db("b2200", "2.2k", -EQ, EQ, 0.0),
            db("b5700", "5.7k", -EQ, EQ, 0.0),
            db("level", "Level", -EQ, EQ, 0.0)
        ],
        eq::graphic5
    ),
    model!(
        "eq8",
        Eq,
        "Graphic 8",
        "EQ8",
        "an 8-band graphic EQ",
        "Eight octave bands from 50 Hz to 6.4 kHz.",
        [237, 237, 234],
        Graphic,
        &[
            db("b50", "50", -EQ, EQ, 0.0),
            db("b100", "100", -EQ, EQ, 0.0),
            db("b200", "200", -EQ, EQ, 0.0),
            db("b400", "400", -EQ, EQ, 0.0),
            db("b800", "800", -EQ, EQ, 0.0),
            db("b1600", "1.6k", -EQ, EQ, 0.0),
            db("b3200", "3.2k", -EQ, EQ, 0.0),
            db("b6400", "6.4k", -EQ, EQ, 0.0),
            db("level", "Level", -EQ, EQ, 0.0)
        ],
        eq::graphic8
    ),
    model!(
        "bass_eq8",
        Eq,
        "Bass Graphic 8",
        "BEQ",
        "an 8-band bass graphic EQ",
        "Eight bands from 30 Hz to 16 kHz, for bass.",
        [167, 196, 222],
        Graphic,
        &[
            db("b30", "30", -EQ, EQ, 0.0),
            db("b75", "75", -EQ, EQ, 0.0),
            db("b185", "185", -EQ, EQ, 0.0),
            db("b460", "460", -EQ, EQ, 0.0),
            db("b1100", "1.1k", -EQ, EQ, 0.0),
            db("b2700", "2.7k", -EQ, EQ, 0.0),
            db("b6800", "6.8k", -EQ, EQ, 0.0),
            db("b16000", "16k", -EQ, EQ, 0.0),
            db("level", "Level", -EQ, EQ, 0.0)
        ],
        eq::bass_graphic8
    ),
    model!(
        "studio_eq",
        Eq,
        "Console EQ",
        "PEQ",
        "a 4-band parametric rack EQ",
        "Four bands with their own frequencies: surgical shaping after the cab.",
        [154, 161, 169],
        Rack,
        &[
            db("bass", "Bass", -12.0, 12.0, 0.0),
            hz("bass_freq", "Bass freq", 50.0, 500.0, 120.0),
            db("lo_mid", "Lo-mid", -12.0, 12.0, 0.0),
            hz("lo_freq", "Lo freq", 100.0, 2000.0, 500.0),
            db("hi_mid", "Hi-mid", -12.0, 12.0, 0.0),
            hz("hi_freq", "Hi freq", 500.0, 8000.0, 2000.0),
            db("treble", "Treble", -12.0, 12.0, 0.0),
            hz("treble_freq", "Treble freq", 1500.0, 16000.0, 5000.0)
        ],
        eq::studio
    ),
    // -- filters
    model!(
        "wah",
        Filter,
        "Tear Jerker",
        "WAH",
        "Dunlop Cry Baby",
        "The talking filter. Pedal: set the position; Auto: it follows how hard you pick.",
        [29, 30, 33],
        Pedal,
        &[
            dial("position", "Pedal", 5.0),
            choice("mode", "Mode", &["Pedal", "Auto"], 0.0),
            dial("sens", "Sens", 5.0),
            dial("speed", "Speed", 5.0)
        ],
        filter::wah
    ),
    model!(
        "env_filter",
        Filter,
        "Quack Box",
        "ENV",
        "Musitronics Mu-Tron III",
        "An auto-wah that opens with your pick attack: funk, quacky bass.",
        [122, 82, 199],
        Pedal,
        &[
            dial("sens", "Sens", 6.0),
            dial("res", "Res", 6.0),
            dial("attack", "Attack", 3.0),
            dial("release", "Release", 4.0),
            choice("mode", "Mode", &["Low-pass", "Band-pass"], 1.0)
        ],
        filter::envelope
    ),
    model!(
        "lofi",
        Filter,
        "Pocket Radio",
        "LOFI",
        "an old radio, a phone, a bit crusher",
        "Squeezes the sound into a small speaker, a telephone line or 8-bit crunch.",
        [226, 192, 67],
        Pedal,
        &[
            choice("type", "Type", &["Radio", "Phone", "Crush"], 0.0),
            pct("mix", "Mix", 100.0),
            dial("drive", "Drive", 3.0)
        ],
        filter::lofi
    ),
    model!(
        "acoustic",
        Filter,
        "Campfire",
        "AC",
        "Boss AC-3",
        "Makes an electric sound like a miked acoustic: body, air, no pickup honk.",
        [154, 106, 60],
        Pedal,
        &[
            dial("body", "Body", 6.0),
            dial("top", "Top", 5.0),
            dial("level", "Level", 5.0)
        ],
        filter::acoustic
    ),
    // -- modulation
    model!(
        "chorus",
        Modulation,
        "Sky Chorus",
        "CH",
        "Boss CE-2",
        "Warm, watery bucket-brigade chorus: 80s cleans, Nirvana's Come As You Are.",
        [62, 155, 214],
        Pedal,
        &[
            dial("rate", "Rate", 3.0),
            dial("depth", "Depth", 6.0),
            pct("mix", "Mix", 50.0)
        ],
        modfx::chorus
    ),
    model!(
        "flanger",
        Modulation,
        "Jet 117",
        "FL",
        "MXR 117 Flanger",
        "Jet-plane sweeps from a short, swept delay with feedback: Van Halen, Heart.",
        [90, 39, 71],
        Pedal,
        &[
            dial("rate", "Rate", 3.0),
            dial("depth", "Depth", 6.0),
            dial("regen", "Regen", 5.0),
            pct("mix", "Mix", 50.0)
        ],
        modfx::flanger
    ),
    model!(
        "phaser",
        Modulation,
        "Script Phase",
        "PH",
        "MXR Phase 90",
        "Four-stage swirl: Eddie Van Halen's brown sound, funk rhythm.",
        [224, 86, 28],
        Pedal,
        &[
            dial("speed", "Speed", 4.0),
            dial("depth", "Depth", 7.0),
            pct("mix", "Mix", 50.0)
        ],
        modfx::phaser
    ),
    model!(
        "tremolo",
        Modulation,
        "Tremolo",
        "TR",
        "Boss TR-2 and amp tremolo",
        "Pulses the volume: surf, Bo Diddley, Boulevard of Broken Dreams.",
        [20, 163, 154],
        Pedal,
        &[
            dial("speed", "Speed", 5.0),
            dial("depth", "Depth", 6.0),
            choice("wave", "Wave", &["Sine", "Triangle", "Square"], 0.0)
        ],
        modfx::tremolo
    ),
    model!(
        "vibe",
        Modulation,
        "Machine Vibe",
        "VIBE",
        "Shin-ei Uni-Vibe",
        "Throbbing, lopsided phase from a lamp and photocells: Machine Gun, Bridge of Sighs.",
        [107, 47, 58],
        Pedal,
        &[
            dial("speed", "Speed", 4.0),
            dial("intensity", "Intensity", 7.0),
            choice("mode", "Mode", &["Chorus", "Vibrato"], 0.0)
        ],
        modfx::vibe
    ),
    model!(
        "rotary",
        Modulation,
        "Rotary 122",
        "ROT",
        "Leslie 122 speaker",
        "A spinning horn and drum: slow is a lush swirl, fast a warble.",
        [92, 58, 34],
        Pedal,
        &[
            choice("speed", "Speed", &["Slow", "Fast"], 0.0),
            dial("balance", "Balance", 5.0),
            dial("depth", "Depth", 6.0),
            pct("mix", "Mix", 70.0)
        ],
        modfx::rotary
    ),
    model!(
        "octaver",
        Modulation,
        "Sub Octaver",
        "OC",
        "Boss OC-2",
        "Adds an octave below (and above): fat single-note riffs, synth bass.",
        [47, 85, 184],
        Pedal,
        &[
            dial("sub", "Sub", 6.0),
            dial("up", "Up", 0.0),
            dial("dry", "Dry", 7.0)
        ],
        modfx::octaver
    ),
    // -- delay
    model!(
        "analog_delay",
        Delay,
        "Graphite Echo",
        "AD",
        "MXR Carbon Copy",
        "Warm repeats that get darker as they fade, with a little wobble.",
        [31, 74, 51],
        Pedal,
        &[
            ms("time", "Time", 20.0, 600.0, 350.0),
            dial("repeats", "Repeats", 4.0),
            pct("mix", "Mix", 30.0),
            dial("mod", "Mod", 3.0)
        ],
        delay::analog
    ),
    model!(
        "tape_echo",
        Delay,
        "Space Tape",
        "TAPE",
        "Roland RE-201 Space Echo",
        "Tape repeats with wow and saturation; turn Repeats up for runaway echoes.",
        [59, 74, 60],
        Pedal,
        &[
            ms("time", "Time", 50.0, 800.0, 450.0),
            dial("repeats", "Repeats", 4.0),
            pct("mix", "Mix", 35.0),
            dial("wow", "Wow", 3.0),
            choice("heads", "Heads", HEADS, 0.0)
        ],
        delay::tape
    ),
    model!(
        "digital_delay",
        Delay,
        "Digital Delay",
        "DD",
        "Boss DD-3",
        "Clean, exact repeats: U2's dotted eighths, slapback, ambient leads.",
        [242, 242, 239],
        Pedal,
        &[
            ms("time", "Time", 20.0, 1000.0, 380.0),
            dial("repeats", "Repeats", 4.0),
            pct("mix", "Mix", 30.0),
            dial("tone", "Tone", 7.0)
        ],
        delay::digital
    ),
    model!(
        "mod_delay",
        Delay,
        "Mod Delay",
        "MD",
        "a modulated delay",
        "Repeats with chorus on them: lush, wide, dreamy.",
        [201, 204, 206],
        Pedal,
        &[
            ms("time", "Time", 20.0, 1000.0, 400.0),
            dial("repeats", "Repeats", 4.0),
            pct("mix", "Mix", 30.0),
            dial("rate", "Rate", 3.0),
            dial("depth", "Depth", 4.0)
        ],
        delay::modulated
    ),
    // -- reverb
    model!(
        "spring",
        Reverb,
        "'63 Spring",
        "SPR",
        "Fender 6G15 reverb unit",
        "Drippy, boingy springs: surf, rockabilly, every Fender clean.",
        [107, 74, 46],
        Pedal,
        &[
            dial("dwell", "Dwell", 6.0),
            dial("tone", "Tone", 5.0),
            pct("mix", "Mix", 30.0)
        ],
        reverb::spring
    ),
    model!(
        "plate",
        Reverb,
        "Plate 140",
        "PLT",
        "an EMT 140 plate",
        "Dense, bright and smooth: the studio reverb of 60s and 70s records.",
        [154, 161, 169],
        Rack,
        &[
            sec("time", "Time", 0.5, 6.0, 2.2),
            dial("tone", "Tone", 5.0),
            ms("predelay", "Pre-delay", 1.0, 100.0, 10.0),
            pct("mix", "Mix", 25.0)
        ],
        reverb::plate
    ),
    model!(
        "room",
        Reverb,
        "Room",
        "ROOM",
        "a small studio room",
        "A short room around the amp: makes a dry tone sound real.",
        [74, 85, 104],
        Pedal,
        &[
            dial("size", "Size", 4.0),
            dial("tone", "Tone", 5.0),
            pct("mix", "Mix", 20.0)
        ],
        reverb::room
    ),
    model!(
        "hall",
        Reverb,
        "Hall",
        "HALL",
        "a concert hall",
        "A big, long, even tail for ambient leads and ballads.",
        [40, 64, 106],
        Pedal,
        &[
            sec("time", "Time", 1.0, 8.0, 3.0),
            dial("tone", "Tone", 5.0),
            ms("predelay", "Pre-delay", 1.0, 100.0, 20.0),
            pct("mix", "Mix", 25.0)
        ],
        reverb::hall
    ),
];

/// The model with this id.
pub fn find(id: &str) -> Option<usize> {
    MODELS.iter().position(|m| m.id == id)
}

/// Models of a kind, in catalogue order.
/// The owner's build shows the real unit each model is after in brackets ("Perspex 100 (Marshall Plexi)"): the
/// `brands` feature, on in qrs-app's and qrs-cli's defaults only, so the shared builds (`--no-default-features`)
/// and the public source show only the soundalike names (D42, D58).
pub const SHOW_BRANDS: bool = cfg!(feature = "brands");

/// The real unit a model is after, short, for the brackets.
pub fn brand(id: &str) -> &'static str {
    match id {
        "squeeze" => "MXR Dyna Comp",
        "studio_comp" => "UREI 1176",
        "bass_comp" => "Darkglass Super Comp",
        "gate" => "Boss NS-2",
        "boost" => "Xotic EP Booster",
        "green" => "Ibanez TS808",
        "blues" => "Boss BD-2",
        "gold" => "Klon Centaur",
        "germanium" => "germanium overdrive",
        "treble_boost" => "Dallas Rangemaster",
        "orange" => "Boss DS-1",
        "rodent" => "ProCo RAT",
        "metal" => "Boss MT-2",
        "bass_drive" => "SansAmp Bass Driver",
        "round_fuzz" => "Fuzz Face",
        "big_pi" => "Big Muff Pi",
        "bender" => "Tone Bender MkII",
        "octave_fuzz" => "Octavia",
        "amp_clean" => "Fender Twin Reverb",
        "amp_tweed" => "Fender 5E3 Deluxe",
        "amp_chime" => "Vox AC30",
        "amp_crunch" => "Marshall Plexi",
        "amp_lead" => "Marshall JCM800",
        "amp_modern" => "Mesa Dual Rectifier",
        "amp_bass" => "Ampeg SVT",
        "cab_1x10" => "Fender Princeton 1x10",
        "cab_1x12" => "Fender Deluxe 1x12",
        "cab_2x12" => "Twin / AC30 2x12",
        "cab_4x12" => "Marshall 1960A",
        "cab_8x10" => "Ampeg SVT-810E",
        "cab_1x15" => "Ampeg B-15",
        "eq5" => "5-band graphic",
        "eq8" => "Boss GE-7 style",
        "bass_eq8" => "Boss GEB-7 style",
        "studio_eq" => "studio parametric",
        "wah" => "Dunlop Cry Baby",
        "env_filter" => "Mu-Tron III",
        "lofi" => "lo-fi",
        "acoustic" => "Boss AC-3",
        "chorus" => "Boss CE-2",
        "flanger" => "MXR 117 Flanger",
        "phaser" => "MXR Phase 90",
        "tremolo" => "Boss TR-2",
        "vibe" => "Uni-Vibe",
        "rotary" => "Leslie 122",
        "octaver" => "Boss OC-2",
        "analog_delay" => "MXR Carbon Copy",
        "tape_echo" => "Roland Space Echo",
        "digital_delay" => "Boss DD-3",
        "mod_delay" => "modulated delay",
        "spring" => "Fender 6G15",
        "plate" => "EMT 140",
        "room" => "studio room",
        "hall" => "concert hall",
        _ => "",
    }
}

impl ModelDef {
    /// The model's name as shown in lists and headers: the soundalike name, plus the real unit in brackets when
    /// `SHOW_BRANDS`.
    pub fn shown_name(&self) -> String {
        match brand(self.id) {
            b if SHOW_BRANDS && !b.is_empty() => format!("{} ({})", self.name, b),
            _ => self.name.to_string(),
        }
    }
}

pub fn of_kind(kind: Kind) -> impl Iterator<Item = (usize, &'static ModelDef)> {
    MODELS.iter().enumerate().filter(move |(_, m)| m.kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_is_sound() {
        assert_eq!(MODELS.len(), 54);
        let mut ids = std::collections::HashSet::new();
        for m in MODELS {
            assert!(ids.insert(m.id), "duplicate id {}", m.id);
            assert!(m.knobs.len() <= MAX_KNOBS, "{} has too many knobs", m.id);
            assert!(m.short.len() <= 4, "{}'s short name", m.id);
            assert!(
                m.knobs[m.front_len()..].iter().all(|k| k.back),
                "{}: back knobs come after the front ones",
                m.id
            );
            assert!(m.front_len() > 0, "{}: nothing on the face", m.id);
            if m.kind == Kind::Amp {
                for key in ["sag", "bias", "tubes", "rectifier", "power_master"] {
                    let i = m.knob_index(key).unwrap_or_else(|| panic!("{}: {key}", m.id));
                    assert!(m.knobs[i].back, "{}: {key} on the face", m.id);
                }
            }
            let mut keys = std::collections::HashSet::new();
            for k in m.knobs {
                assert!(keys.insert(k.key), "{}: knob {} twice", m.id, k.key);
                assert!(
                    k.lo < k.hi && k.def >= k.lo && k.def <= k.hi,
                    "{}: {}",
                    m.id,
                    k.key
                );
                let u = k.to_u(k.def);
                assert!(
                    (k.from_u(u) - k.def).abs() < 1e-3 * (1.0 + k.def.abs()),
                    "{}: {}",
                    m.id,
                    k.key
                );
            }
        }
        for kind in Kind::ALL {
            assert!(of_kind(kind).count() >= 4, "{kind:?}");
        }
    }

    #[test]
    fn knob_text_reads_naturally() {
        let m = &MODELS[find("tape_echo").unwrap()];
        assert_eq!(m.knobs[0].text(450.0), "450 ms");
        let eq = &MODELS[find("eq5").unwrap()];
        assert_eq!(eq.knobs[0].text(3.0), "+3.0 dB");
        assert_eq!(eq.knobs[0].text(-2.5), "\u{2212}2.5 dB");
        let cab = &MODELS[find("cab_4x12").unwrap()];
        assert_eq!(cab.knobs[2].text(1.0), "Ribbon");
        assert_eq!(cab.knobs[4].text(12000.0), "12.0 kHz");
    }
}
