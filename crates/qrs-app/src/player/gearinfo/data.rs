//! The data behind `gearinfo`: one card per model, in catalogue order. "Heard on" lists famous records with the
//! real unit or, for generic models, that kind of sound.

use super::{About, Heard, KnobTip, Start};

const fn h(song: &'static str, artist: &'static str, year: u16) -> Heard {
    Heard { song, artist, year }
}

const fn t(key: &'static str, text: &'static str, sweet: (f32, f32), note: &'static str) -> KnobTip {
    KnobTip {
        key,
        text,
        sweet,
        note,
    }
}

const fn s(name: &'static str, knobs: &'static [(&'static str, f32)]) -> Start {
    Start { name, knobs }
}

// -- shared tips ------------------------------------------------------------------------------------------------------

/// The six cabs share their mic and filter controls.
const CAB_TIPS: &[KnobTip] = &[
    t(
        "mic",
        "Where the mic points on the speaker: at the centre (0) it's brightest and most cutting, towards the edge \
         (10) darker, rounder and softer.",
        (3.0, 5.0),
        "the cap edge, the classic studio spot; move out if it fizzes",
    ),
    t(
        "distance",
        "How far the mic is from the grille: close is punchy with a big low end (the proximity effect), further \
         away thinner with more air and room.",
        (1.0, 3.0),
        "close-miked, as most records do it",
    ),
    t(
        "mic_type",
        "Dynamic: punchy and mid-forward, the classic guitar mic. Ribbon: smooth and dark, tames fizzy high gain. \
         Condenser: detailed and airy, hears more of the room.",
        (0.0, 1.0),
        "dynamic for rock; ribbon for harsh high gain",
    ),
    t(
        "low_cut",
        "Trims rumble and boom below the guitar. Raise it if chords get muddy or the bass player needs the room.",
        (60.0, 100.0),
        "about 80 Hz for guitar; lower for bass",
    ),
    t(
        "high_cut",
        "Trims fizz above the speaker. Lower it if high gain sounds like a bee in a jar.",
        (6000.0, 9000.0),
        "about 7-8 kHz for distortion; higher for cleans",
    ),
];

const CAB_STARTS: &[Start] = &[
    s(
        "Classic close mic",
        &[
            ("mic", 4.0),
            ("distance", 1.5),
            ("mic_type", 0.0),
            ("low_cut", 80.0),
            ("high_cut", 8000.0),
        ],
    ),
    s(
        "Smooth ribbon",
        &[
            ("mic", 3.0),
            ("distance", 2.5),
            ("mic_type", 1.0),
            ("low_cut", 70.0),
            ("high_cut", 9000.0),
        ],
    ),
    s(
        "Bright and cutting",
        &[
            ("mic", 1.0),
            ("distance", 1.0),
            ("mic_type", 0.0),
            ("low_cut", 100.0),
            ("high_cut", 10000.0),
        ],
    ),
    s(
        "Roomy condenser",
        &[
            ("mic", 5.0),
            ("distance", 7.0),
            ("mic_type", 2.0),
            ("low_cut", 60.0),
            ("high_cut", 12000.0),
        ],
    ),
];

const BASS_CAB_STARTS: &[Start] = &[
    s(
        "Round and deep",
        &[
            ("mic", 5.0),
            ("distance", 2.0),
            ("mic_type", 0.0),
            ("low_cut", 30.0),
            ("high_cut", 6000.0),
        ],
    ),
    s(
        "Growl up front",
        &[
            ("mic", 2.0),
            ("distance", 1.0),
            ("mic_type", 0.0),
            ("low_cut", 40.0),
            ("high_cut", 9000.0),
        ],
    ),
    s(
        "Vintage thump",
        &[
            ("mic", 7.0),
            ("distance", 3.0),
            ("mic_type", 1.0),
            ("low_cut", 35.0),
            ("high_cut", 4000.0),
        ],
    ),
];

const fn band(key: &'static str, text: &'static str, note: &'static str) -> KnobTip {
    t(key, text, (-6.0, 6.0), note)
}

const EQ_LEVEL: KnobTip = t(
    "level",
    "The EQ's output. Boosting bands makes it louder; use this to bring it back level, or as a solo boost.",
    (-3.0, 3.0),
    "match bypassed and on, unless it's a boost",
);

const fn mix(text: &'static str, sweet: (f32, f32), note: &'static str) -> KnobTip {
    t("mix", text, sweet, note)
}

pub static ABOUT: &[About] = &[
    // -- dynamics ------------------------------------------------------------------------------------------------------
    About {
        id: "squeeze",
        real: "MXR Dyna Comp",
        plain: "a 70s orange stompbox compressor",
        year: 1972,
        facts: "1972 \u{b7} American \u{b7} compressor",
        what: "A simple pedal compressor that squashes the pick attack and lifts the tail of each note, so it \
               sustains longer and every note comes out the same level. It gives clean parts that glassy, \
               squeezed snap.",
        heard: &[h("Comfortably Numb", "Pink Floyd", 1979)],
        try_for: &["Country", "Funk", "Clean leads", "Pop"],
        tips: &[
            t(
                "sustain",
                "How hard it squeezes. More holds notes longer and evens out strumming, but also lifts hiss and \
                 flattens your dynamics.",
                (3.0, 6.0),
                "for cleans; 7+ for country chicken-picking",
            ),
            t(
                "attack",
                "How much of the pick's click gets through before the squeeze: low is smooth and soft, high snappy.",
                (4.0, 7.0),
                "higher for funk and country snap",
            ),
            t(
                "level",
                "The output. Set it so the pedal is as loud on as off, or a little louder for a lead.",
                (4.0, 6.0),
                "match the bypassed level",
            ),
        ],
        starts: &[
            s(
                "Country snap",
                &[("sustain", 7.0), ("attack", 7.0), ("level", 5.0)],
            ),
            s(
                "Funk rhythm",
                &[("sustain", 5.0), ("attack", 8.0), ("level", 5.0)],
            ),
            s(
                "Clean lead sustain",
                &[("sustain", 8.0), ("attack", 4.0), ("level", 5.5)],
            ),
            s(
                "Just evened out",
                &[("sustain", 3.0), ("attack", 5.0), ("level", 5.0)],
            ),
        ],
    },
    About {
        id: "studio_comp",
        real: "UREI 1176 limiting amplifier",
        plain: "a classic late-60s studio rack compressor",
        year: 1967,
        facts: "1967 \u{b7} American \u{b7} studio compressor",
        what: "A studio rack compressor with every control laid out: how loud before it acts, how much, how fast. \
               It evens out a part without the pedal-style squash, the way engineers sit a guitar in a mix.",
        heard: &[
            h("Get Lucky", "Daft Punk", 2013),
            h("Billie Jean", "Michael Jackson", 1982),
        ],
        try_for: &["Funk", "Pop", "Clean rhythm", "Mixing"],
        tips: &[
            t(
                "threshold",
                "The level where it starts working. Lower catches more of the playing; watch the curve to see how \
                 much is above the line.",
                (-24.0, -12.0),
                "so the loudest strums get 3-6 dB taken off",
            ),
            t(
                "ratio",
                "How hard it pushes back above the threshold: 2:1 is gentle, 4:1 firm, 10:1 and up a limiter.",
                (3.0, 6.0),
                "4:1 for guitar; 8:1+ to flatten funk strums",
            ),
            t(
                "attack",
                "How fast it grabs. Fast (under 5 ms) dulls the pick; slower lets the attack through for punch.",
                (5.0, 30.0),
                "about 10-20 ms keeps the pick",
            ),
            t(
                "release",
                "How fast it lets go. Too fast pumps and wobbles; too slow keeps quiet notes squashed.",
                (80.0, 300.0),
                "set so it breathes with the rhythm",
            ),
            t(
                "makeup",
                "Gain after the squeeze, to win back the level it took off.",
                (2.0, 8.0),
                "match the bypassed level",
            ),
        ],
        starts: &[
            s(
                "Gentle glue",
                &[
                    ("threshold", -16.0),
                    ("ratio", 2.5),
                    ("attack", 20.0),
                    ("release", 200.0),
                    ("makeup", 3.0),
                ],
            ),
            s(
                "Funk strum",
                &[
                    ("threshold", -24.0),
                    ("ratio", 8.0),
                    ("attack", 8.0),
                    ("release", 120.0),
                    ("makeup", 8.0),
                ],
            ),
            s(
                "Clean lead",
                &[
                    ("threshold", -22.0),
                    ("ratio", 5.0),
                    ("attack", 15.0),
                    ("release", 400.0),
                    ("makeup", 7.0),
                ],
            ),
            s(
                "Brick wall",
                &[
                    ("threshold", -30.0),
                    ("ratio", 20.0),
                    ("attack", 1.0),
                    ("release", 100.0),
                    ("makeup", 12.0),
                ],
            ),
        ],
    },
    About {
        id: "bass_comp",
        real: "Darkglass Super Symmetry-style multiband compressor",
        plain: "a modern multiband bass compressor",
        year: 2013,
        facts: "2010s \u{b7} multiband \u{b7} bass compressor",
        what: "Splits the bass in two and compresses the low end on its own, so the lows stay rock steady while the \
               pick or finger attack above still punches through. The modern rock and metal bass sound starts here.",
        heard: &[
            h("Higher Ground", "Red Hot Chili Peppers", 1989),
            h("Around the World", "Red Hot Chili Peppers", 1999),
        ],
        try_for: &["Rock bass", "Metal bass", "Funk bass", "Pop bass"],
        tips: &[
            t(
                "compress",
                "How much the low band is squeezed. More keeps every low note the same size.",
                (4.0, 7.0),
                "push it for slap and pick playing",
            ),
            t(
                "crossover",
                "Where the bass splits: below this is held steady, above it stays free. Higher holds more of the \
                 note's body.",
                (200.0, 500.0),
                "about 250-400 Hz for most basses",
            ),
            t(
                "speed",
                "How fast it acts: slow keeps the attack, fast grabs everything and smooths it flat.",
                (4.0, 6.0),
                "faster for slap",
            ),
            t(
                "level",
                "The output, to match the level with it off.",
                (4.0, 6.0),
                "match the bypassed level",
            ),
        ],
        starts: &[
            s(
                "Steady low end",
                &[
                    ("compress", 5.0),
                    ("crossover", 300.0),
                    ("speed", 5.0),
                    ("level", 5.0),
                ],
            ),
            s(
                "Slap tamer",
                &[
                    ("compress", 7.0),
                    ("crossover", 400.0),
                    ("speed", 8.0),
                    ("level", 5.5),
                ],
            ),
            s(
                "Pick punch",
                &[
                    ("compress", 6.0),
                    ("crossover", 200.0),
                    ("speed", 3.0),
                    ("level", 5.0),
                ],
            ),
            s(
                "Barely there",
                &[
                    ("compress", 3.0),
                    ("crossover", 250.0),
                    ("speed", 5.0),
                    ("level", 5.0),
                ],
            ),
        ],
    },
    About {
        id: "gate",
        real: "Boss NS-2 Noise Suppressor",
        plain: "a late-80s compact noise gate",
        year: 1987,
        facts: "1987 \u{b7} Japan \u{b7} noise gate",
        what: "Shuts the sound off when you stop playing, so the hiss and hum of high gain disappear between \
               notes. Set right, it also makes stop-start metal riffs land tight and dead silent.",
        heard: &[
            h("Master of Puppets", "Metallica", 1986),
            h("Walk", "Pantera", 1992),
        ],
        try_for: &["Metal", "High-gain rock", "Djent"],
        tips: &[
            t(
                "threshold",
                "Below this the gate closes. Too high chops off the ends of notes; too low lets the hiss through.",
                (-65.0, -50.0),
                "just above the noise with the strings muted",
            ),
            t(
                "release",
                "How fast it closes once you stop: short is tight and choppy, long lets chords fade naturally.",
                (40.0, 150.0),
                "short for riffs, longer for ringing chords",
            ),
        ],
        starts: &[
            s("Just the hiss", &[("threshold", -62.0), ("release", 120.0)]),
            s("Tight metal stops", &[("threshold", -50.0), ("release", 30.0)]),
            s("Natural fade", &[("threshold", -68.0), ("release", 300.0)]),
        ],
    },
    About {
        id: "boost",
        real: "Xotic EP Booster",
        plain: "a modern clean boost after a 70s tape-echo preamp",
        year: 2009,
        facts: "2009 \u{b7} American \u{b7} clean boost",
        what: "A small clean boost built on the preamp of an old tape echo, which players left on just for its \
               sound. It adds level, a touch of sparkle and fatness, and pushes the amp after it harder.",
        heard: &[
            h("Eruption", "Van Halen", 1978),
            h("Runnin' with the Devil", "Van Halen", 1978),
        ],
        try_for: &["Solos", "Classic rock", "Blues", "Always on"],
        tips: &[
            t(
                "gain",
                "How much louder: a few dB fattens the tone, 10 dB and more drives the amp after it into breakup.",
                (3.0, 9.0),
                "about 6 dB for a solo lift",
            ),
            t(
                "bright",
                "Adds a little top end sparkle. Turn it down with a bright amp or single coils.",
                (3.0, 6.0),
                "lower into a bright amp",
            ),
        ],
        starts: &[
            s("Always on", &[("gain", 3.0), ("bright", 5.0)]),
            s("Solo lift", &[("gain", 7.0), ("bright", 6.0)]),
            s("Push the amp", &[("gain", 14.0), ("bright", 4.0)]),
            s("Dark and fat", &[("gain", 6.0), ("bright", 1.5)]),
        ],
    },
    // -- drive ---------------------------------------------------------------------------------------------------------
    About {
        id: "green",
        real: "Ibanez TS808 Tube Screamer",
        plain: "a late-70s Japanese green overdrive",
        year: 1979,
        facts: "1979 \u{b7} Japan \u{b7} overdrive",
        what: "The green overdrive players put in front of a slightly crunchy amp. It trims the bass before \
               clipping, so the lows stay tight and the mids push forward: leads sing and rhythm parts cut.",
        heard: &[
            h("Pride and Joy", "Stevie Ray Vaughan", 1983),
            h("Texas Flood", "Stevie Ray Vaughan", 1983),
        ],
        try_for: &["Blues", "Blues-rock", "Metal boost", "Leads"],
        tips: &[
            t(
                "drive",
                "How much it clips. Low is a fat boost for a crunchy amp; high is a smooth, compressed overdrive \
                 on its own.",
                (3.0, 6.0),
                "near 0 with Level up as a metal boost",
            ),
            t(
                "tone",
                "Left: darker and smoother. Right: brighter and edgier. The pedal already trims the bass before it \
                 clips: that is the famous mid hump.",
                (4.0, 6.0),
                "lower into a bright amp",
            ),
            t(
                "level",
                "The output. Up high it pushes the amp harder too, which is half the trick.",
                (5.0, 7.0),
                "a little above unity into a crunch amp",
            ),
        ],
        starts: &[
            s(
                "Boost a crunchy amp",
                &[("drive", 2.0), ("tone", 5.5), ("level", 8.0)],
            ),
            s("Blues lead", &[("drive", 5.0), ("tone", 5.0), ("level", 6.0)]),
            s("Texas flood", &[("drive", 6.5), ("tone", 6.0), ("level", 7.0)]),
            s(
                "Tight metal boost",
                &[("drive", 0.5), ("tone", 6.0), ("level", 9.0)],
            ),
            s("Light grit", &[("drive", 3.0), ("tone", 4.5), ("level", 5.5)]),
        ],
    },
    About {
        id: "blues",
        real: "Boss BD-2 Blues Driver",
        plain: "a mid-90s blue compact overdrive",
        year: 1995,
        facts: "1995 \u{b7} Japan \u{b7} overdrive",
        what: "An open, full-range overdrive that keeps the bass and treble of your guitar and amp. It reacts \
               to how hard you pick: soft playing is nearly clean, digging in brings out the grit.",
        heard: &[],
        try_for: &["Blues", "Indie rock", "Edge of breakup", "Country"],
        tips: &[
            t(
                "gain",
                "How much it breaks up. Below the middle it cleans up with your guitar's volume; above it gets \
                 raw and fuzzy at the edges.",
                (3.0, 6.0),
                "for edge-of-breakup blues",
            ),
            t(
                "tone",
                "Brightness. This pedal is already bright, so most players keep it at or below the middle.",
                (3.5, 5.5),
                "lower with single coils",
            ),
            t(
                "level",
                "The output: match the bypassed level for rhythm, a bit more for leads.",
                (4.5, 6.5),
                "about unity",
            ),
        ],
        starts: &[
            s("Edge of breakup", &[("gain", 3.0), ("tone", 4.5), ("level", 5.5)]),
            s("Dig-in blues", &[("gain", 5.5), ("tone", 4.0), ("level", 5.5)]),
            s("Raw garage", &[("gain", 8.0), ("tone", 5.5), ("level", 5.0)]),
            s("Warm and round", &[("gain", 4.0), ("tone", 2.5), ("level", 6.0)]),
        ],
    },
    About {
        id: "gold",
        real: "Klon Centaur",
        plain: "a mid-90s boutique gold overdrive",
        year: 1994,
        facts: "1994 \u{b7} American \u{b7} transparent overdrive",
        what: "The famously expensive 'transparent' overdrive. It blends some of your clean signal back in with the \
               driven one, so the guitar sounds thicker and pushed rather than changed, with a gentle mid lift.",
        heard: &[h("Gravity", "John Mayer", 2006), h("Belief", "John Mayer", 2006)],
        try_for: &["Blues", "Classic rock", "Always on", "Leads"],
        tips: &[
            t(
                "gain",
                "Low settings are mostly clean boost with a touch of hair; up high the clipped part takes over.",
                (2.0, 5.0),
                "low gain, high output is the classic use",
            ),
            t(
                "treble",
                "A treble control after the clipping: turn up to cut through, down for a rounder lead.",
                (4.0, 6.0),
                "noon, then by ear",
            ),
            t(
                "output",
                "Lots of clean volume on tap. Up high it drives a tube amp into its own breakup.",
                (5.0, 8.0),
                "above unity to push the amp",
            ),
        ],
        starts: &[
            s("Push the amp", &[("gain", 2.0), ("treble", 5.0), ("output", 8.0)]),
            s("Thick rhythm", &[("gain", 4.5), ("treble", 5.0), ("output", 6.0)]),
            s("Singing lead", &[("gain", 7.0), ("treble", 5.5), ("output", 6.5)]),
            s(
                "Always-on sweetener",
                &[("gain", 1.0), ("treble", 4.5), ("output", 6.0)],
            ),
        ],
    },
    About {
        id: "germanium",
        real: "germanium-diode overdrive",
        plain: "a germanium-diode overdrive",
        year: 2005,
        facts: "boutique \u{b7} germanium diodes \u{b7} overdrive",
        what: "An overdrive clipping through germanium diodes, which round off softer and a little lopsided. The \
               result is warm, compressed and vintage, closer to a cranked old amp than a modern pedal.",
        heard: &[
            h("All Right Now", "Free", 1970),
            h("Black Magic Woman", "Santana", 1970),
        ],
        try_for: &["Vintage rock", "Blues", "Classic rock"],
        tips: &[
            t(
                "gain",
                "How much it clips. The germanium corners are soft, so even high settings stay smooth.",
                (4.0, 7.0),
                "for vintage rhythm crunch",
            ),
            t(
                "tone",
                "Brightness after the clipping: lower is woolly and warm, higher opens up the top.",
                (4.0, 6.0),
                "around noon",
            ),
            t(
                "level",
                "The output: match the bypassed level, or a little more for leads.",
                (4.5, 6.5),
                "about unity",
            ),
        ],
        starts: &[
            s("70s rhythm", &[("gain", 5.0), ("tone", 5.0), ("level", 5.5)]),
            s("Warm lead", &[("gain", 7.5), ("tone", 4.0), ("level", 6.0)]),
            s("Light warmth", &[("gain", 2.5), ("tone", 5.5), ("level", 5.0)]),
        ],
    },
    About {
        id: "treble_boost",
        real: "Dallas Rangemaster Treble Booster",
        plain: "a mid-60s British treble booster",
        year: 1966,
        facts: "1966 \u{b7} British \u{b7} treble booster",
        what: "A one-transistor box that sat on top of the amp and boosted the upper mids and treble into it. Into \
               a dark British amp it gives the cutting, singing 60s and 70s lead sound; on its own it's thin.",
        heard: &[
            h("Paranoid", "Black Sabbath", 1970),
            h("Keep Yourself Alive", "Queen", 1973),
        ],
        try_for: &["60s British blues", "Classic rock", "Doom", "Leads"],
        tips: &[
            t(
                "boost",
                "How hard it hits the amp. Use it into a crunchy amp: the amp does the distortion.",
                (5.0, 8.0),
                "into a crunch amp; it's thin into a clean one",
            ),
            t(
                "range",
                "Where the boost sits: low is a fuller mid boost, high the classic cutting treble boost.",
                (5.0, 8.0),
                "higher for the classic sound",
            ),
        ],
        starts: &[
            s("British blues", &[("boost", 7.0), ("range", 7.0)]),
            s("Heavy riff", &[("boost", 9.0), ("range", 5.0)]),
            s("Fuller boost", &[("boost", 6.0), ("range", 2.5)]),
        ],
    },
    About {
        id: "orange",
        real: "Boss DS-1 Distortion",
        plain: "a late-70s orange compact distortion",
        year: 1978,
        facts: "1978 \u{b7} Japan \u{b7} distortion",
        what: "The best-selling orange distortion pedal: hard clipping with a scooped, bright tone. It turns a \
               clean amp into a punk or grunge rhythm sound and is a common first pedal.",
        heard: &[
            h("Smells Like Teen Spirit", "Nirvana", 1991),
            h("Surfing with the Alien", "Joe Satriani", 1987),
        ],
        try_for: &["Punk", "Grunge", "Classic rock", "80s rock"],
        tips: &[
            t(
                "dist",
                "How much it clips. Past the middle it's thick and compressed; full up gets buzzy.",
                (5.0, 7.5),
                "for rhythm; less into a crunchy amp",
            ),
            t(
                "tone",
                "This tone knob is strong: past noon it gets harsh and fizzy fast. Keep it lower into a bright \
                 amp.",
                (3.0, 5.0),
                "below noon with high gain",
            ),
            t(
                "level",
                "The output: match the bypassed level.",
                (4.5, 6.5),
                "about unity",
            ),
        ],
        starts: &[
            s("Grunge", &[("dist", 7.0), ("tone", 4.0), ("level", 5.5)]),
            s("Punk rhythm", &[("dist", 6.0), ("tone", 5.5), ("level", 5.5)]),
            s("Shred lead", &[("dist", 8.5), ("tone", 4.5), ("level", 6.0)]),
            s("Crunch", &[("dist", 3.5), ("tone", 4.0), ("level", 6.0)]),
        ],
    },
    About {
        id: "rodent",
        real: "ProCo RAT",
        plain: "a late-70s American black box distortion",
        year: 1978,
        facts: "1978 \u{b7} American \u{b7} distortion",
        what: "A gritty op-amp distortion in a black box, from light crunch to near fuzz. Its Filter knob works \
               backwards: turning it up makes the sound darker, which is how most players tame it.",
        heard: &[],
        try_for: &["Alt rock", "Grunge", "Punk", "Noise rock"],
        tips: &[
            t(
                "distortion",
                "How much it clips: low is a crunchy overdrive, past 7 a thick, fuzzy wall.",
                (4.0, 7.0),
                "for alt-rock rhythm",
            ),
            t(
                "filter",
                "Works backwards: up is darker, down is brighter. Most players run it between 3 and 6.",
                (3.0, 6.0),
                "up if it's fizzy",
            ),
            t(
                "volume",
                "The output: there's a lot on tap, so start low.",
                (4.0, 6.0),
                "about unity",
            ),
        ],
        starts: &[
            s(
                "Alt-rock rhythm",
                &[("distortion", 6.0), ("filter", 4.5), ("volume", 5.0)],
            ),
            s(
                "Light crunch",
                &[("distortion", 2.5), ("filter", 3.0), ("volume", 6.0)],
            ),
            s(
                "Fuzzy wall",
                &[("distortion", 9.0), ("filter", 6.5), ("volume", 5.0)],
            ),
            s(
                "Dark lead",
                &[("distortion", 7.0), ("filter", 7.0), ("volume", 5.5)],
            ),
        ],
    },
    About {
        id: "metal",
        real: "Boss MT-2 Metal Zone",
        plain: "an early-90s compact high-gain distortion",
        year: 1991,
        facts: "1991 \u{b7} Japan \u{b7} high-gain distortion",
        what: "Two stages of high gain and a strong three-band EQ with a sweepable feel, made to get metal from a \
               clean amp. Its EQ is powerful: small moves change a lot.",
        heard: &[],
        try_for: &["Metal", "Hard rock", "Nu metal"],
        tips: &[
            t(
                "gain",
                "Lots of gain. Past about 7 it gets fizzy and noisy; use a gate after it.",
                (5.0, 7.5),
                "lower than you'd think, for tight riffs",
            ),
            t(
                "low",
                "The low end. A little boost is huge here; too much and palm mutes turn to mush.",
                (0.0, 5.0),
                "small boosts",
            ),
            t(
                "mid",
                "The famous mid control: cutting it gives the scooped sound, but scooped vanishes in a band.",
                (-4.0, 4.0),
                "keep some mid to be heard",
            ),
            t(
                "high",
                "Treble and bite. Too much is a hiss of fizz.",
                (0.0, 4.0),
                "a little above flat",
            ),
            t(
                "level",
                "The output: it's easy to make it much louder than bypassed.",
                (4.0, 6.0),
                "match the bypassed level",
            ),
        ],
        starts: &[
            s(
                "Tight modern",
                &[
                    ("gain", 6.5),
                    ("low", 3.0),
                    ("mid", 2.0),
                    ("high", 2.0),
                    ("level", 5.0),
                ],
            ),
            s(
                "Scooped thrash",
                &[
                    ("gain", 7.5),
                    ("low", 6.0),
                    ("mid", -6.0),
                    ("high", 4.0),
                    ("level", 5.0),
                ],
            ),
            s(
                "Lead with mids",
                &[
                    ("gain", 8.0),
                    ("low", 1.0),
                    ("mid", 5.0),
                    ("high", 2.0),
                    ("level", 5.5),
                ],
            ),
            s(
                "Hard rock",
                &[
                    ("gain", 4.0),
                    ("low", 2.0),
                    ("mid", 3.0),
                    ("high", 1.0),
                    ("level", 5.0),
                ],
            ),
        ],
    },
    About {
        id: "bass_drive",
        real: "Tech 21 SansAmp Bass Driver DI",
        plain: "a mid-90s bass preamp and direct box",
        year: 1994,
        facts: "1994 \u{b7} American \u{b7} bass preamp, DI",
        what: "A bass preamp that sounds like a miked tube amp, with a Blend knob that mixes the clean bass back \
               in. Bassists leave it on for grit and growl while the low end stays solid.",
        heard: &[h("Hysteria", "Muse", 2003), h("Schism", "Tool", 2001)],
        try_for: &["Rock bass", "Metal bass", "Indie bass"],
        tips: &[
            t(
                "gain",
                "How much tube-style grit. Low is warm, high a growling overdrive.",
                (3.0, 6.0),
                "for growl that stays clear",
            ),
            t(
                "blend",
                "How much of the driven sound against the clean bass. Below 100% the lows stay clean and tight.",
                (50.0, 80.0),
                "keep some clean for the low end",
            ),
            t(
                "tone",
                "Brightness of the drive: higher brings out the pick and the growl, lower is round.",
                (4.0, 6.0),
                "about noon",
            ),
            t(
                "level",
                "The output: match the bypassed level.",
                (4.5, 6.0),
                "about unity",
            ),
        ],
        starts: &[
            s(
                "Always-on warmth",
                &[("gain", 3.0), ("blend", 60.0), ("tone", 5.0), ("level", 5.0)],
            ),
            s(
                "Rock growl",
                &[("gain", 6.0), ("blend", 75.0), ("tone", 6.0), ("level", 5.0)],
            ),
            s(
                "Fuzzy bass",
                &[("gain", 9.0), ("blend", 50.0), ("tone", 7.0), ("level", 5.0)],
            ),
            s(
                "Round and clean",
                &[("gain", 1.5), ("blend", 40.0), ("tone", 4.0), ("level", 5.5)],
            ),
        ],
    },
    // -- fuzz ----------------------------------------------------------------------------------------------------------
    About {
        id: "round_fuzz",
        real: "Dallas Arbiter Fuzz Face",
        plain: "a mid-60s round British fuzz",
        year: 1966,
        facts: "1966 \u{b7} British \u{b7} two-transistor fuzz",
        what: "Two transistors in a round box: a thick, woolly fuzz that reacts to your guitar's volume knob. \
               Roll the guitar back and it cleans up almost completely; turn it up and it roars.",
        heard: &[
            h("Foxy Lady", "Jimi Hendrix", 1967),
            h("Fire", "Jimi Hendrix", 1967),
        ],
        try_for: &["60s rock", "Blues-rock", "Psychedelic"],
        tips: &[
            t(
                "fuzz",
                "How much fuzz. Most players keep it near full and use the guitar's volume knob instead.",
                (7.0, 10.0),
                "full up, guitar volume to clean up",
            ),
            t(
                "volume",
                "The output. Up high it also pushes the amp into its own breakup.",
                (4.5, 7.0),
                "about unity or a little more",
            ),
        ],
        starts: &[
            s("Full fuzz", &[("fuzz", 10.0), ("volume", 5.5)]),
            s("Woolly rhythm", &[("fuzz", 6.0), ("volume", 6.0)]),
            s("Into a hot amp", &[("fuzz", 8.0), ("volume", 7.5)]),
        ],
    },
    About {
        id: "big_pi",
        real: "Electro-Harmonix Big Muff Pi",
        plain: "a late-60s American big-box fuzz",
        year: 1969,
        facts: "1969 \u{b7} American \u{b7} fuzz, sustainer",
        what: "A huge, sustaining fuzz with a scooped middle: notes hang on for ever under a thick wall of sound. \
               Its tone knob sweeps from dark and smooth to bright and cutting.",
        heard: &[
            h("Seven Nation Army", "The White Stripes", 2003),
            h("Cherub Rock", "Smashing Pumpkins", 1993),
            h("Comfortably Numb", "Pink Floyd", 1979),
        ],
        try_for: &["Alt rock", "Stoner rock", "Shoegaze", "Leads"],
        tips: &[
            t(
                "sustain",
                "How much fuzz and sustain. Even halfway is a lot; full up is a wall.",
                (5.0, 8.0),
                "for the classic wall",
            ),
            t(
                "tone",
                "Left is dark and bassy, right bright and thin; the middle has the famous scooped mids, which can \
                 disappear in a band.",
                (4.0, 6.5),
                "above noon to cut through a mix",
            ),
            t(
                "volume",
                "The output: there's plenty, so start below the middle.",
                (4.0, 6.0),
                "about unity",
            ),
        ],
        starts: &[
            s(
                "Wall of fuzz",
                &[("sustain", 8.0), ("tone", 5.0), ("volume", 5.0)],
            ),
            s(
                "Singing lead",
                &[("sustain", 7.0), ("tone", 6.5), ("volume", 6.0)],
            ),
            s("Dark stoner", &[("sustain", 9.0), ("tone", 3.0), ("volume", 5.0)]),
            s(
                "Less is more",
                &[("sustain", 4.0), ("tone", 5.5), ("volume", 5.5)],
            ),
        ],
    },
    About {
        id: "bender",
        real: "Sola Sound Tone Bender MkII",
        plain: "a mid-60s British three-transistor fuzz",
        year: 1966,
        facts: "1966 \u{b7} British \u{b7} three-transistor fuzz",
        what: "A three-transistor British fuzz: raspy, mid-heavy and aggressive, with more gain and bite than the \
               round fuzzes of the time. It's the sound of late-60s British heavy rock.",
        heard: &[
            h("Communication Breakdown", "Led Zeppelin", 1969),
            h("Dazed and Confused", "Led Zeppelin", 1969),
        ],
        try_for: &["60s British rock", "Heavy blues", "Garage"],
        tips: &[
            t(
                "attack",
                "The amount of fuzz. Up high it's raspy and saturated; lower it's a thick crunch.",
                (6.0, 9.0),
                "for the classic rasp",
            ),
            t(
                "level",
                "The output: match the bypassed level or push the amp a little.",
                (4.5, 6.5),
                "about unity",
            ),
        ],
        starts: &[
            s("Heavy riff", &[("attack", 8.5), ("level", 5.5)]),
            s("Crunchy fuzz", &[("attack", 5.0), ("level", 6.0)]),
            s("Full rasp", &[("attack", 10.0), ("level", 5.0)]),
        ],
    },
    About {
        id: "octave_fuzz",
        real: "Tycobrahe Octavia",
        plain: "a 70s octave-up fuzz",
        year: 1975,
        facts: "1970s \u{b7} American \u{b7} octave fuzz",
        what: "A fuzz that folds the signal over, adding a ghostly octave above the note. It sounds best on single \
               notes with the neck pickup, high up the neck; chords turn into a mess.",
        heard: &[h("Purple Haze", "Jimi Hendrix", 1967)],
        try_for: &["Psychedelic", "Leads", "Funk-rock"],
        tips: &[
            t(
                "fuzz",
                "How much fuzz and octave. The octave shows most above the 12th fret.",
                (6.0, 9.0),
                "single notes, neck pickup",
            ),
            t(
                "volume",
                "The output: match the bypassed level.",
                (4.5, 6.0),
                "about unity",
            ),
        ],
        starts: &[
            s("Octave lead", &[("fuzz", 8.0), ("volume", 5.5)]),
            s("Subtle octave", &[("fuzz", 5.0), ("volume", 5.5)]),
            s("Full ring", &[("fuzz", 10.0), ("volume", 5.0)]),
        ],
    },
    // -- amps ----------------------------------------------------------------------------------------------------------
    About {
        id: "amp_clean",
        real: "Fender Twin Reverb",
        plain: "a 60s American 2x12 clean combo",
        year: 1963,
        facts: "1963 \u{b7} American \u{b7} 85 W 2x12 combo",
        what: "The loud, clean American combo: huge headroom, sparkling highs and scooped mids, so it stays clean \
               at almost any volume. It's the clean sound most records use, and a great base for pedals.",
        heard: &[
            h("The Thrill Is Gone", "B.B. King", 1969),
            h("Sultans of Swing", "Dire Straits", 1978),
        ],
        try_for: &["Clean", "Country", "Funk", "Pedal platform"],
        tips: &[
            t(
                "gain",
                "The preamp's volume. It stays clean for a long way; only near the top does it start to break up.",
                (3.0, 5.0),
                "for sparkling cleans",
            ),
            t(
                "bass",
                "The low end. These amps get boomy fast with humbuckers.",
                (3.0, 5.0),
                "lower with humbuckers",
            ),
            t(
                "mid",
                "Body and warmth. This amp is scooped by design; more mid helps it sit in a band.",
                (4.0, 6.0),
                "up a little in a band",
            ),
            t(
                "treble",
                "Sparkle and snap. With single coils it can get glassy and sharp.",
                (5.0, 7.0),
                "lower with single coils",
            ),
            t(
                "bright",
                "A switch that adds extra top at low volume. Great for dark humbuckers, harsh with bright guitars.",
                (0.0, 1.0),
                "on for humbuckers, off for single coils",
            ),
            t(
                "master",
                "The overall level. Higher also adds a little power-amp warmth and squash.",
                (4.0, 6.0),
                "set the level, match with the board's output",
            ),
        ],
        starts: &[
            s(
                "Sparkling clean",
                &[
                    ("gain", 3.5),
                    ("bass", 4.0),
                    ("mid", 5.0),
                    ("treble", 6.5),
                    ("bright", 1.0),
                ],
            ),
            s(
                "Warm jazz",
                &[
                    ("gain", 3.0),
                    ("bass", 6.0),
                    ("mid", 6.0),
                    ("treble", 3.5),
                    ("bright", 0.0),
                ],
            ),
            s(
                "Funk snap",
                &[
                    ("gain", 4.0),
                    ("bass", 3.0),
                    ("mid", 6.0),
                    ("treble", 7.0),
                    ("bright", 1.0),
                ],
            ),
            s(
                "Pedal platform",
                &[
                    ("gain", 3.5),
                    ("bass", 4.5),
                    ("mid", 6.0),
                    ("treble", 5.5),
                    ("bright", 0.0),
                ],
            ),
        ],
    },
    About {
        id: "amp_tweed",
        real: "Fender 5E3 Tweed Deluxe",
        plain: "a 50s American tweed combo",
        year: 1955,
        facts: "1955 \u{b7} American \u{b7} 12 W 1x12 combo",
        what: "A small 50s combo in tweed cloth with just volume and tone. Turned up it breaks up early into a \
               warm, compressed, hairy overdrive that cleans up when you pick softly.",
        heard: &[
            h("Cinnamon Girl", "Neil Young", 1969),
            h("Like a Hurricane", "Neil Young & Crazy Horse", 1977),
        ],
        try_for: &["Blues", "Roots rock", "Garage", "Rockabilly"],
        tips: &[
            t(
                "volume",
                "How hard the amp is driven. Clean below 3, crunchy from 5, a fat raw overdrive near full.",
                (5.0, 8.0),
                "for the classic tweed breakup",
            ),
            t(
                "tone",
                "The only tone control: dark and woolly to bright and cutting. It also changes how hard it breaks up.",
                (5.0, 7.0),
                "a bit past noon",
            ),
            t(
                "master",
                "The overall level, so you can drive the amp hard without it being loud.",
                (4.0, 6.0),
                "set the level",
            ),
        ],
        starts: &[
            s("Edge of breakup", &[("volume", 4.0), ("tone", 6.0)]),
            s("Roots rock", &[("volume", 6.5), ("tone", 6.5)]),
            s("Raw and loud", &[("volume", 9.0), ("tone", 5.5)]),
            s("Dark blues", &[("volume", 6.0), ("tone", 3.5)]),
        ],
    },
    About {
        id: "amp_chime",
        real: "Vox AC30 Top Boost",
        plain: "an early-60s British 2x12 class A combo",
        year: 1961,
        facts: "1961 \u{b7} British \u{b7} 30 W class A 2x12 combo",
        what: "A British class A combo: bright, chiming cleans that turn into a rich, harmonic crunch when pushed. \
               Its Cut knob tames the top in the power amp, working the opposite way to a presence control.",
        heard: &[
            h("Ticket to Ride", "The Beatles", 1965),
            h("Bohemian Rhapsody", "Queen", 1975),
            h("I Will Follow", "U2", 1980),
        ],
        try_for: &["Jangle pop", "Britpop", "Indie", "Classic rock"],
        tips: &[
            t(
                "gain",
                "Preamp drive: chimey and clean low, a rich, compressed crunch from about 6.",
                (4.0, 7.0),
                "for chime into crunch",
            ),
            t(
                "treble",
                "Brightness. The amp is already bright, so start below noon.",
                (4.0, 6.0),
                "lower with single coils",
            ),
            t(
                "bass",
                "The low end. Top Boost circuits get flabby with too much.",
                (3.0, 5.5),
                "keep it modest",
            ),
            t(
                "cut",
                "Cuts the high end in the power amp: up is darker and smoother, down is brighter.",
                (2.0, 5.0),
                "up if it's spiky",
            ),
            t(
                "master",
                "The overall level, and with it a little power-amp compression.",
                (4.0, 6.0),
                "set the level",
            ),
        ],
        starts: &[
            s(
                "Jangle",
                &[("gain", 3.5), ("treble", 6.0), ("bass", 4.0), ("cut", 2.5)],
            ),
            s(
                "Chime and crunch",
                &[("gain", 6.0), ("treble", 5.5), ("bass", 4.5), ("cut", 3.5)],
            ),
            s(
                "Brit lead",
                &[("gain", 8.5), ("treble", 5.0), ("bass", 5.0), ("cut", 5.0)],
            ),
            s(
                "Dark and warm",
                &[("gain", 5.0), ("treble", 3.5), ("bass", 5.0), ("cut", 6.5)],
            ),
        ],
    },
    About {
        id: "amp_crunch",
        real: "Marshall 1959 Super Lead",
        plain: "a late-60s British 100-watt head",
        year: 1967,
        facts: "1967-69 \u{b7} British \u{b7} 100 W head",
        what: "The loud British head that defined classic rock. The original had no master volume, so players \
               turned it flat out and let the power tubes distort: a fat, chewy crunch with barking upper mids \
               that cleans up when you roll back the guitar's volume.",
        heard: &[
            h("Purple Haze", "Jimi Hendrix", 1967),
            h("Back in Black", "AC/DC", 1980),
            h("Ain't Talkin' 'bout Love", "Van Halen", 1978),
        ],
        try_for: &["Classic rock", "Hard rock", "Blues-rock", "Punk"],
        tips: &[
            t(
                "gain",
                "How hard the preamp is pushed: low is a loud clean, the middle a crunch that follows your \
                 picking, high a thick, sustaining lead.",
                (5.0, 7.0),
                "for rhythm crunch; 8+ for leads",
            ),
            t(
                "bass",
                "The low end of the tone stack. Too much turns palm mutes to mud through a closed cab.",
                (3.5, 5.5),
                "lower it as the gain goes up",
            ),
            t(
                "mid",
                "The body of the sound, where a guitar sits in a band. This amp lives on its mids.",
                (5.5, 8.0),
                "scooping it sounds big alone but vanishes in a mix",
            ),
            t(
                "treble",
                "Brightness and pick attack before the power amp. It also sets how the other two knobs interact.",
                (5.0, 7.0),
                "less with a bright guitar or single coils",
            ),
            t(
                "presence",
                "Adds bite and sparkle above about 2 kHz in the power amp. Turn it down if the tone is harsh or \
                 fizzy; turn it up if the guitar gets lost behind the band.",
                (4.0, 6.0),
                "for crunch; 6-8 if the amp is dark or the room is dull",
            ),
            t(
                "master",
                "The power amp's level: how loud, and past about 7 how much the power tubes add their own squash.",
                (4.0, 6.0),
                "set the level, then match with the board's output",
            ),
        ],
        starts: &[
            s(
                "Clean-ish edge",
                &[
                    ("gain", 3.5),
                    ("bass", 5.0),
                    ("mid", 6.0),
                    ("treble", 6.0),
                    ("presence", 5.0),
                ],
            ),
            s(
                "AC/DC rhythm",
                &[
                    ("gain", 5.5),
                    ("bass", 4.5),
                    ("mid", 7.0),
                    ("treble", 6.5),
                    ("presence", 5.5),
                ],
            ),
            s(
                "Hot-rodded lead",
                &[
                    ("gain", 8.5),
                    ("bass", 4.0),
                    ("mid", 7.5),
                    ("treble", 6.0),
                    ("presence", 6.0),
                ],
            ),
            s(
                "Blues-rock",
                &[
                    ("gain", 6.5),
                    ("bass", 5.5),
                    ("mid", 6.5),
                    ("treble", 5.0),
                    ("presence", 4.5),
                ],
            ),
        ],
    },
    About {
        id: "amp_lead",
        real: "Marshall JCM800 2203",
        plain: "an early-80s British 100-watt master-volume head",
        year: 1981,
        facts: "1981 \u{b7} British \u{b7} 100 W master-volume head",
        what: "The British head with a master volume, so the preamp can be driven hard at any level. Tight, \
               aggressive and upper-mid heavy: the 80s rock and thrash rhythm sound, and a singing lead.",
        heard: &[
            h("Sweet Child O' Mine", "Guns N' Roses", 1987),
            h("Raining Blood", "Slayer", 1986),
        ],
        try_for: &["80s rock", "Hard rock", "Thrash", "Punk"],
        tips: &[
            t(
                "gain",
                "Preamp drive: crunch around 5, full rock rhythm at 7, saturated lead past 8.",
                (5.5, 8.0),
                "for hard rock rhythm",
            ),
            t(
                "bass",
                "The low end. Keep it modest for tight palm mutes.",
                (3.0, 5.5),
                "lower for tight riffs",
            ),
            t(
                "mid",
                "This amp's bark lives in the mids; cutting them loses its character.",
                (5.0, 8.0),
                "keep them up",
            ),
            t(
                "treble",
                "Brightness and attack before the power amp.",
                (5.0, 7.0),
                "lower with bright guitars",
            ),
            t(
                "presence",
                "Top-end bite in the power amp. Too much is harsh and fizzy with high gain.",
                (4.0, 6.0),
                "lower with high gain",
            ),
            t(
                "master",
                "The overall level, with power-amp squash as it goes up.",
                (4.0, 6.0),
                "set the level",
            ),
        ],
        starts: &[
            s(
                "80s rock rhythm",
                &[
                    ("gain", 7.0),
                    ("bass", 4.5),
                    ("mid", 6.5),
                    ("treble", 6.5),
                    ("presence", 5.0),
                ],
            ),
            s(
                "Thrash",
                &[
                    ("gain", 8.5),
                    ("bass", 4.0),
                    ("mid", 5.0),
                    ("treble", 7.0),
                    ("presence", 6.0),
                ],
            ),
            s(
                "Singing lead",
                &[
                    ("gain", 9.0),
                    ("bass", 5.0),
                    ("mid", 7.5),
                    ("treble", 5.5),
                    ("presence", 4.5),
                ],
            ),
            s(
                "Punchy crunch",
                &[
                    ("gain", 5.0),
                    ("bass", 5.0),
                    ("mid", 7.0),
                    ("treble", 6.0),
                    ("presence", 5.0),
                ],
            ),
        ],
    },
    About {
        id: "amp_modern",
        real: "Mesa/Boogie Dual Rectifier",
        plain: "an early-90s American high-gain head",
        year: 1991,
        facts: "1991 \u{b7} American \u{b7} 100 W high-gain head",
        what: "The American high-gain head of 90s and 2000s metal: saturated, tight and with a huge, deep low end. \
               Its EQ is powerful, and the classic mistake is scooping the mids until the guitar disappears.",
        heard: &[
            h("Enter Sandman", "Metallica", 1991),
            h("Blood and Thunder", "Mastodon", 2004),
        ],
        try_for: &["Metal", "Nu metal", "Hard rock", "Djent"],
        tips: &[
            t(
                "gain",
                "Saturation. It has far more than you need; lower gain sounds bigger and tighter.",
                (5.0, 7.5),
                "lower than you think for rhythm",
            ),
            t(
                "bass",
                "The low end. With Depth up as well it gets enormous and loose.",
                (4.0, 6.0),
                "lower if palm mutes go mushy",
            ),
            t(
                "mid",
                "Keep some mids so the guitar is heard; scooping sounds big alone, small in a band.",
                (4.0, 6.0),
                "about noon in a band",
            ),
            t(
                "treble",
                "Brightness and pick attack.",
                (5.0, 7.0),
                "by ear with the cab",
            ),
            t(
                "presence",
                "Upper bite in the power amp. Too much and it fizzes.",
                (4.0, 6.0),
                "lower with high gain",
            ),
            t(
                "depth",
                "Low-end resonance in the power amp: the chest thump of a closed cab.",
                (3.0, 6.0),
                "lower for tighter low end",
            ),
            t("master", "The overall level.", (4.0, 6.0), "set the level"),
        ],
        starts: &[
            s(
                "Modern metal",
                &[
                    ("gain", 7.0),
                    ("bass", 5.5),
                    ("mid", 5.0),
                    ("treble", 6.5),
                    ("presence", 5.5),
                    ("depth", 5.0),
                ],
            ),
            s(
                "Scooped 90s",
                &[
                    ("gain", 8.0),
                    ("bass", 7.0),
                    ("mid", 2.5),
                    ("treble", 7.0),
                    ("presence", 6.0),
                    ("depth", 6.0),
                ],
            ),
            s(
                "Tight djent",
                &[
                    ("gain", 6.0),
                    ("bass", 4.0),
                    ("mid", 6.0),
                    ("treble", 6.5),
                    ("presence", 6.0),
                    ("depth", 3.0),
                ],
            ),
            s(
                "Thick lead",
                &[
                    ("gain", 8.5),
                    ("bass", 5.0),
                    ("mid", 7.0),
                    ("treble", 5.5),
                    ("presence", 4.5),
                    ("depth", 4.5),
                ],
            ),
        ],
    },
    About {
        id: "amp_bass",
        real: "Ampeg SVT",
        plain: "a late-60s American 300-watt tube bass head",
        year: 1969,
        facts: "1969 \u{b7} American \u{b7} 300 W tube bass head",
        what: "The rock bass amp: a 300-watt tube head, warm and huge in the lows, with a mid control that sweeps \
               to where your bass growls. Pushed, it adds a gritty tube edge.",
        heard: &[
            h("Another One Bites the Dust", "Queen", 1980),
            h("Hysteria", "Muse", 2003),
        ],
        try_for: &["Rock bass", "Classic rock bass", "Metal bass"],
        tips: &[
            t(
                "gain",
                "Preamp drive: clean and round low, a growling tube edge up high.",
                (3.0, 6.0),
                "for warm with a little grit",
            ),
            t(
                "bass",
                "The low end. It's a lot; too much muddies the kick drum.",
                (4.5, 7.0),
                "about 5-6",
            ),
            t(
                "mid",
                "Boost or cut at the frequency set by Freq. A boost helps a bass be heard on small speakers.",
                (4.0, 7.0),
                "a little boost for definition",
            ),
            t(
                "freq",
                "Where the Mid knob works: low for punch, 500-800 Hz for growl, higher for finger and pick clank.",
                (400.0, 1200.0),
                "about 800 Hz for rock growl",
            ),
            t(
                "treble",
                "Top end and string clank. Lower for an old-school round sound.",
                (4.0, 6.0),
                "lower with flatwounds",
            ),
            t("master", "The overall level.", (4.0, 6.0), "set the level"),
        ],
        starts: &[
            s(
                "Classic rock bass",
                &[
                    ("gain", 4.0),
                    ("bass", 6.0),
                    ("mid", 6.0),
                    ("freq", 800.0),
                    ("treble", 5.0),
                ],
            ),
            s(
                "Growl",
                &[
                    ("gain", 7.0),
                    ("bass", 5.5),
                    ("mid", 7.0),
                    ("freq", 600.0),
                    ("treble", 6.0),
                ],
            ),
            s(
                "Round and deep",
                &[
                    ("gain", 3.0),
                    ("bass", 7.0),
                    ("mid", 4.0),
                    ("freq", 400.0),
                    ("treble", 3.5),
                ],
            ),
            s(
                "Pick clank",
                &[
                    ("gain", 5.0),
                    ("bass", 5.0),
                    ("mid", 6.5),
                    ("freq", 1800.0),
                    ("treble", 7.0),
                ],
            ),
        ],
    },
    // -- cabs ----------------------------------------------------------------------------------------------------------
    About {
        id: "cab_1x10",
        real: "Fender Princeton 1x10",
        plain: "a small 60s American 1x10 combo's speaker",
        year: 1964,
        facts: "1960s \u{b7} American \u{b7} open-back 1x10",
        what: "One 10-inch speaker in a small open-back box: boxy, focused and light on bass. It's the sound of a \
               small studio amp, great for vintage blues and parts that need to sit small in a mix.",
        heard: &[h("Layla", "Derek and the Dominos", 1970)],
        try_for: &["Blues", "Vintage", "Small cleans", "Recording"],
        tips: CAB_TIPS,
        starts: CAB_STARTS,
    },
    About {
        id: "cab_1x12",
        real: "Fender Deluxe 1x12",
        plain: "a 60s American open-back 1x12",
        year: 1963,
        facts: "1960s \u{b7} American \u{b7} open-back 1x12",
        what: "One 12-inch speaker in an open-back combo: airy and open, with a soft low end that doesn't boom. \
               The classic small-combo sound for cleans and light breakup.",
        heard: &[h("Cinnamon Girl", "Neil Young", 1969)],
        try_for: &["Clean", "Blues", "Roots rock", "Country"],
        tips: CAB_TIPS,
        starts: CAB_STARTS,
    },
    About {
        id: "cab_2x12",
        real: "Fender Twin / Vox AC30 open-back 2x12",
        plain: "an open-back 2x12 combo",
        year: 1963,
        facts: "1960s \u{b7} open-back 2x12",
        what: "Two 12-inch speakers in an open-back combo: wide, bright and room-filling, the speakers of the big \
               clean combos. Less low thump than a closed cab, more air.",
        heard: &[
            h("Ticket to Ride", "The Beatles", 1965),
            h("Sultans of Swing", "Dire Straits", 1978),
        ],
        try_for: &["Clean", "Jangle", "Country", "Indie"],
        tips: CAB_TIPS,
        starts: CAB_STARTS,
    },
    About {
        id: "cab_4x12",
        real: "Marshall 1960A 4x12",
        plain: "a 60s British closed-back slant 4x12",
        year: 1965,
        facts: "1960s \u{b7} British \u{b7} closed-back 4x12",
        what: "Four 12-inch speakers in a closed, slanted box: a tight low-end thump and a forward upper mid. Half \
               of rock and metal was recorded through one of these.",
        heard: &[
            h("Back in Black", "AC/DC", 1980),
            h("Whole Lotta Love", "Led Zeppelin", 1969),
        ],
        try_for: &["Rock", "Metal", "Hard rock", "Punk"],
        tips: CAB_TIPS,
        starts: CAB_STARTS,
    },
    About {
        id: "cab_8x10",
        real: "Ampeg SVT-810E",
        plain: "a 70s American 8x10 bass cabinet",
        year: 1969,
        facts: "1969 on \u{b7} American \u{b7} 8x10 bass cab",
        what: "Eight 10-inch speakers in a cabinet the size of a fridge: deep, punchy bass with a growly upper mid. \
               The partner of the big tube bass head on rock stages.",
        heard: &[h("Hysteria", "Muse", 2003), h("Schism", "Tool", 2001)],
        try_for: &["Rock bass", "Metal bass", "Big stages"],
        tips: CAB_TIPS,
        starts: BASS_CAB_STARTS,
    },
    About {
        id: "cab_1x15",
        real: "Ampeg B-15 Portaflex 1x15",
        plain: "a 60s American 1x15 bass combo",
        year: 1960,
        facts: "1960 \u{b7} American \u{b7} 1x15 bass",
        what: "One 15-inch speaker in a closed box: round, warm, old-school bass with little top end. The sound of \
               60s soul and Motown bass lines.",
        heard: &[
            h("My Girl", "The Temptations", 1964),
            h("What's Going On", "Marvin Gaye", 1971),
        ],
        try_for: &["Motown", "Soul", "Vintage bass", "Reggae"],
        tips: CAB_TIPS,
        starts: BASS_CAB_STARTS,
    },
    // -- EQ ------------------------------------------------------------------------------------------------------------
    About {
        id: "eq5",
        real: "a 5-band guitar graphic EQ",
        plain: "a 5-band guitar graphic EQ",
        year: 1975,
        facts: "70s on \u{b7} 5-band graphic EQ",
        what: "Five sliders for broad strokes: low thump, warmth, body, bite and air. Put it after a drive to shape \
               the distortion, or after the amp to fit the guitar into a mix.",
        heard: &[
            h("Master of Puppets", "Metallica", 1986),
            h("Sweet Child O' Mine", "Guns N' Roses", 1987),
        ],
        try_for: &["Shaping", "Solo boost", "Fixing a dull amp"],
        tips: &[
            band(
                "b63",
                "The deep low end: thump and boom. Cut it to tighten a muddy tone.",
                "small cuts help most",
            ),
            band(
                "b250",
                "Warmth and mud. Cutting here clears up chords; boosting fattens a thin guitar.",
                "cut for clarity",
            ),
            band(
                "b750",
                "The body and the midrange honk: where the guitar sits in a band.",
                "boost for solos",
            ),
            band(
                "b2200",
                "Bite and presence: pick attack and cut.",
                "boost to cut through",
            ),
            band(
                "b5700",
                "Air and fizz. Cut it to tame harsh high gain.",
                "cut with high gain",
            ),
            EQ_LEVEL,
        ],
        starts: &[
            s(
                "Mid boost for solos",
                &[("b750", 6.0), ("b2200", 3.0), ("level", 3.0)],
            ),
            s("Scooped", &[("b63", 3.0), ("b750", -7.0), ("b5700", 3.0)]),
            s("Tighten the low end", &[("b63", -6.0), ("b250", -3.0)]),
            s("Tame the fizz", &[("b2200", -2.0), ("b5700", -6.0)]),
        ],
    },
    About {
        id: "eq8",
        real: "Boss GE-7 Graphic Equalizer (extended to 8 bands)",
        plain: "an 8-band compact graphic EQ",
        year: 1981,
        facts: "1981 on \u{b7} 8-band graphic EQ",
        what: "Eight octave-wide sliders from the deep lows to the top: finer shaping than a tone knob. Often used \
               as a solo boost with the mids pushed, or to fix a boomy or harsh amp.",
        heard: &[
            h("Master of Puppets", "Metallica", 1986),
            h("Sweet Child O' Mine", "Guns N' Roses", 1987),
        ],
        try_for: &["Shaping", "Solo boost", "Metal", "Fixing a mix"],
        tips: &[
            band("b50", "Sub-lows below the guitar: mostly rumble. Cut it.", "cut"),
            band(
                "b100",
                "The low thump of palm mutes; too much is boom.",
                "small moves",
            ),
            band("b200", "Warmth, and mud with high gain.", "cut for clarity"),
            band("b400", "Body; cutting it gives a scooped sound.", "keep some"),
            band("b800", "Midrange honk and presence in a band.", "boost for solos"),
            band("b1600", "Upper mids: bite and cut.", "boost to be heard"),
            band(
                "b3200",
                "Pick attack and edge; harsh if boosted a lot.",
                "small boosts",
            ),
            band("b6400", "Air and fizz.", "cut with high gain"),
            EQ_LEVEL,
        ],
        starts: &[
            s("Solo mid push", &[("b800", 6.0), ("b1600", 4.0), ("level", 3.0)]),
            s(
                "Scooped metal",
                &[("b100", 4.0), ("b400", -6.0), ("b800", -4.0), ("b3200", 3.0)],
            ),
            s("Clean up the mud", &[("b50", -8.0), ("b200", -4.0)]),
            s("Smooth the top", &[("b3200", -3.0), ("b6400", -6.0)]),
        ],
    },
    About {
        id: "bass_eq8",
        real: "Boss GEB-7 Bass Equalizer (extended to 8 bands)",
        plain: "an 8-band compact bass graphic EQ",
        year: 1983,
        facts: "1980s on \u{b7} 8-band bass graphic EQ",
        what: "A graphic EQ with its bands spread for bass, from the deep sub-lows to string zing. It shapes the \
               lows, adds growl, or makes the slap scoop.",
        heard: &[
            h("Higher Ground", "Red Hot Chili Peppers", 1989),
            h("Another One Bites the Dust", "Queen", 1980),
        ],
        try_for: &["Bass", "Slap", "Fixing a mix"],
        tips: &[
            band(
                "b30",
                "Sub-lows you feel more than hear; too much eats headroom.",
                "cut a little",
            ),
            band(
                "b75",
                "The fundamental thump of the low strings.",
                "boost for weight",
            ),
            band("b185", "Warmth and boom.", "cut if it's boomy"),
            band(
                "b460",
                "Body and mud; cutting it is the slap scoop.",
                "cut for slap",
            ),
            band(
                "b1100",
                "Growl and presence on small speakers.",
                "boost to be heard",
            ),
            band("b2700", "Finger and pick attack.", "boost for definition"),
            band("b6800", "String zing and slap click.", "boost for slap"),
            band("b16000", "Air and hiss.", "mostly flat"),
            EQ_LEVEL,
        ],
        starts: &[
            s(
                "Slap scoop",
                &[("b75", 4.0), ("b460", -6.0), ("b2700", 3.0), ("b6800", 5.0)],
            ),
            s("Growl", &[("b75", 2.0), ("b1100", 5.0), ("b2700", 2.0)]),
            s(
                "Vintage round",
                &[("b75", 3.0), ("b185", 2.0), ("b2700", -4.0), ("b6800", -6.0)],
            ),
            s("Tight and clean", &[("b30", -6.0), ("b185", -3.0)]),
        ],
    },
    About {
        id: "studio_eq",
        real: "a 4-band studio console parametric EQ",
        plain: "a 4-band studio console parametric EQ",
        year: 1972,
        facts: "70s on \u{b7} studio \u{b7} 4-band parametric EQ",
        what: "A mixing desk's EQ: four bands, each with its own frequency, so you can boost or cut exactly where \
               the problem or the magic is. Best after the cab, the way engineers shape a recorded guitar.",
        heard: &[
            h("Bohemian Rhapsody", "Queen", 1975),
            h("Money", "Pink Floyd", 1973),
        ],
        try_for: &["Mixing", "Surgical fixes", "Recording"],
        tips: &[
            t(
                "bass",
                "Boost or cut the lows at Bass freq.",
                (-4.0, 3.0),
                "cut boom rather than boost",
            ),
            t(
                "bass_freq",
                "Where the bass band works: 80-120 Hz is thump, 200+ is warmth and mud.",
                (80.0, 150.0),
                "about 100 Hz for guitar",
            ),
            t(
                "lo_mid",
                "Boost or cut the lower mids at Lo freq: body or mud.",
                (-5.0, 2.0),
                "a cut clears chords",
            ),
            t(
                "lo_freq",
                "Where the lower-mid band works: 250-500 Hz is the usual mud.",
                (250.0, 600.0),
                "sweep to find the mud",
            ),
            t(
                "hi_mid",
                "Boost or cut the upper mids at Hi freq: presence and bite.",
                (-3.0, 4.0),
                "boost to cut through",
            ),
            t(
                "hi_freq",
                "Where the upper-mid band works: 1.5-3 kHz is presence, 3-5 kHz harshness.",
                (1500.0, 3500.0),
                "about 2 kHz",
            ),
            t(
                "treble",
                "Boost or cut the top at Treble freq: air or fizz.",
                (-6.0, 2.0),
                "cut fizz with high gain",
            ),
            t(
                "treble_freq",
                "Where the treble band starts: 5 kHz and up is air and fizz.",
                (4000.0, 8000.0),
                "about 6 kHz",
            ),
        ],
        starts: &[
            s(
                "Mix-ready rock",
                &[
                    ("bass", -2.0),
                    ("bass_freq", 100.0),
                    ("lo_mid", -3.0),
                    ("lo_freq", 400.0),
                    ("hi_mid", 2.0),
                    ("hi_freq", 2000.0),
                    ("treble", -3.0),
                    ("treble_freq", 6000.0),
                ],
            ),
            s(
                "Clean sparkle",
                &[
                    ("lo_mid", -2.0),
                    ("lo_freq", 300.0),
                    ("treble", 3.0),
                    ("treble_freq", 8000.0),
                ],
            ),
            s(
                "Kill the fizz",
                &[
                    ("treble", -8.0),
                    ("treble_freq", 5000.0),
                    ("hi_mid", -2.0),
                    ("hi_freq", 4000.0),
                ],
            ),
            s(
                "Solo presence",
                &[
                    ("hi_mid", 5.0),
                    ("hi_freq", 1500.0),
                    ("lo_mid", 2.0),
                    ("lo_freq", 800.0),
                ],
            ),
        ],
    },
    // -- filters -------------------------------------------------------------------------------------------------------
    About {
        id: "wah",
        real: "Dunlop Cry Baby wah",
        plain: "a late-60s wah pedal",
        year: 1966,
        facts: "1966 \u{b7} American \u{b7} wah",
        what: "A rocking pedal that sweeps a sharp filter from bass to treble: the talking, crying guitar of funk \
               and rock solos. Parked halfway it's a fixed, honky mid tone; in Auto it follows your picking.",
        heard: &[
            h("Voodoo Child (Slight Return)", "Jimi Hendrix", 1968),
            h("White Room", "Cream", 1968),
            h("Theme from Shaft", "Isaac Hayes", 1971),
        ],
        try_for: &["Funk", "Rock solos", "Classic rock", "Blues"],
        tips: &[
            t(
                "position",
                "Where the pedal is: heel down (0) is dark, toe down (10) bright. Parked around the middle is the \
                 'cocked wah' honk.",
                (3.0, 7.0),
                "rock it in time with the music",
            ),
            t(
                "mode",
                "Pedal: you set the position. Auto: it sweeps with how hard you pick.",
                (0.0, 1.0),
                "Auto when your foot is busy",
            ),
            t(
                "sens",
                "In Auto, how much your picking opens it. Higher sweeps further on soft notes.",
                (4.0, 7.0),
                "so hard picks reach the top",
            ),
            t(
                "speed",
                "In Auto, how fast it follows: slow is a smooth sweep, fast a quack.",
                (4.0, 6.0),
                "faster for funk",
            ),
        ],
        starts: &[
            s("Cocked wah", &[("position", 6.0), ("mode", 0.0)]),
            s("Funk auto", &[("mode", 1.0), ("sens", 6.5), ("speed", 7.0)]),
            s("Slow auto sweep", &[("mode", 1.0), ("sens", 5.0), ("speed", 3.0)]),
            s("Dark and vocal", &[("position", 3.0), ("mode", 0.0)]),
        ],
    },
    About {
        id: "env_filter",
        real: "Musitronics Mu-Tron III",
        plain: "an early-70s envelope filter",
        year: 1972,
        facts: "1972 \u{b7} American \u{b7} envelope filter",
        what: "An automatic wah that opens with your pick attack: hit hard and it quacks, play softly and it stays \
               dark. The squelchy sound of 70s funk clavinet, guitar and bass.",
        heard: &[
            h("Higher Ground", "Stevie Wonder", 1973),
            h("Shakedown Street", "Grateful Dead", 1978),
        ],
        try_for: &["Funk", "Funk bass", "Jam bands"],
        tips: &[
            t(
                "sens",
                "How much your picking opens the filter. Too high and every note slams to the top.",
                (5.0, 7.0),
                "so only hard picks fully open it",
            ),
            t(
                "res",
                "The peak of the filter: higher is more vocal and quacky.",
                (5.0, 8.0),
                "higher for funk quack",
            ),
            t(
                "attack",
                "How fast it opens on each note. Fast is a quack, slow a swell.",
                (2.0, 4.0),
                "fast for funk",
            ),
            t(
                "release",
                "How fast it closes again as the note fades.",
                (3.0, 6.0),
                "with the groove",
            ),
            t(
                "mode",
                "Low-pass: round and wah-like. Band-pass: thinner and more vocal.",
                (0.0, 1.0),
                "band-pass on guitar, low-pass on bass",
            ),
        ],
        starts: &[
            s(
                "Funk quack",
                &[
                    ("sens", 6.5),
                    ("res", 7.0),
                    ("attack", 2.0),
                    ("release", 4.0),
                    ("mode", 1.0),
                ],
            ),
            s(
                "Bass squelch",
                &[
                    ("sens", 6.0),
                    ("res", 6.0),
                    ("attack", 3.0),
                    ("release", 5.0),
                    ("mode", 0.0),
                ],
            ),
            s(
                "Soft swell",
                &[
                    ("sens", 4.5),
                    ("res", 4.0),
                    ("attack", 7.0),
                    ("release", 6.0),
                    ("mode", 0.0),
                ],
            ),
        ],
    },
    About {
        id: "lofi",
        real: "an old radio, a telephone line, a bit crusher",
        plain: "an old radio, a telephone line, a bit crusher",
        year: 1995,
        facts: "studio trick \u{b7} lo-fi filter",
        what: "Squeezes the sound into a tiny speaker, a telephone line or crunchy 8-bit numbers. A production \
               trick for intros and breakdowns that then open up to the full sound.",
        heard: &[h("Wish You Were Here", "Pink Floyd", 1975)],
        try_for: &["Intros", "Breakdowns", "Indie", "Effects"],
        tips: &[
            t(
                "type",
                "Radio: a small boxy speaker. Phone: a narrow telephone line. Crush: digital 8-bit grit.",
                (0.0, 1.0),
                "Radio for intros",
            ),
            t(
                "mix",
                "How much of the lo-fi sound against your full one. 100% for the full effect.",
                (70.0, 100.0),
                "full for the effect",
            ),
            t(
                "drive",
                "How much the little speaker distorts.",
                (2.0, 5.0),
                "a little crackle",
            ),
        ],
        starts: &[
            s(
                "Old radio intro",
                &[("type", 0.0), ("mix", 100.0), ("drive", 3.0)],
            ),
            s("Telephone", &[("type", 1.0), ("mix", 100.0), ("drive", 4.0)]),
            s("8-bit crunch", &[("type", 2.0), ("mix", 80.0), ("drive", 5.0)]),
            s("Just a hint", &[("type", 0.0), ("mix", 35.0), ("drive", 2.0)]),
        ],
    },
    About {
        id: "acoustic",
        real: "Boss AC-3 Acoustic Simulator",
        plain: "a 2000s acoustic simulator pedal",
        year: 2007,
        facts: "2007 \u{b7} Japan \u{b7} acoustic simulator",
        what: "Reshapes an electric guitar to sound like a miked acoustic: more body resonance and air, less of \
               the pickup's honk. Handy for songs that switch to an acoustic part.",
        heard: &[
            h("Wonderwall", "Oasis", 1995),
            h("Patience", "Guns N' Roses", 1988),
        ],
        try_for: &["Acoustic parts", "Strumming", "Ballads"],
        tips: &[
            t(
                "body",
                "The resonance of a wooden body: more is fuller and boomier.",
                (5.0, 7.0),
                "lower with humbuckers",
            ),
            t(
                "top",
                "The air and string shimmer of an acoustic.",
                (4.0, 6.5),
                "higher with dark pickups",
            ),
            t(
                "level",
                "The output: match the bypassed level.",
                (4.5, 6.0),
                "about unity",
            ),
        ],
        starts: &[
            s(
                "Strummed dreadnought",
                &[("body", 6.5), ("top", 5.5), ("level", 5.0)],
            ),
            s(
                "Small-body fingerpick",
                &[("body", 4.0), ("top", 6.5), ("level", 5.0)],
            ),
            s("Jumbo", &[("body", 8.0), ("top", 4.5), ("level", 5.0)]),
        ],
    },
    // -- modulation ----------------------------------------------------------------------------------------------------
    About {
        id: "chorus",
        real: "Boss CE-2 Chorus",
        plain: "a late-70s blue compact analogue chorus",
        year: 1979,
        facts: "1979 \u{b7} Japan \u{b7} analogue chorus",
        what: "A bucket-brigade chorus: a slightly detuned, wavering copy of your guitar mixed back in, like two \
               players at once. Warm and watery, the sound of 80s cleans.",
        heard: &[
            h("Come As You Are", "Nirvana", 1991),
            h("Message in a Bottle", "The Police", 1979),
        ],
        try_for: &["80s cleans", "New wave", "Grunge cleans", "Ballads"],
        tips: &[
            t(
                "rate",
                "How fast it wavers: slow is a gentle shimmer, fast a seasick wobble.",
                (2.0, 4.0),
                "slow for shimmer",
            ),
            t(
                "depth",
                "How far the copy is detuned. Deep and slow is lush; deep and fast is vibrato-like.",
                (4.0, 7.0),
                "less if it sounds out of tune",
            ),
            mix(
                "How much of the chorused copy. 50% is the classic, fullest chorus.",
                (35.0, 55.0),
                "50% for the full effect",
            ),
        ],
        starts: &[
            s("80s shimmer", &[("rate", 3.0), ("depth", 6.0), ("mix", 50.0)]),
            s("Grunge clean", &[("rate", 2.0), ("depth", 8.0), ("mix", 50.0)]),
            s(
                "Subtle thickening",
                &[("rate", 2.5), ("depth", 3.5), ("mix", 30.0)],
            ),
            s("Seasick", &[("rate", 7.0), ("depth", 8.0), ("mix", 50.0)]),
        ],
    },
    About {
        id: "flanger",
        real: "MXR M117 Flanger",
        plain: "a mid-70s American flanger",
        year: 1976,
        facts: "1976 \u{b7} American \u{b7} flanger",
        what: "A very short delay swept up and down and fed back into itself: the jet-plane whoosh. Slow and deep \
               it's a dramatic sweep on chords; with Regen up it gets metallic.",
        heard: &[h("Unchained", "Van Halen", 1981), h("Barracuda", "Heart", 1977)],
        try_for: &["Hard rock", "80s rock", "Psychedelic"],
        tips: &[
            t(
                "rate",
                "Sweep speed: slow is the jet plane, fast a warble.",
                (1.5, 4.0),
                "slow for the jet",
            ),
            t("depth", "How wide the sweep is.", (5.0, 8.0), "deep for drama"),
            t(
                "regen",
                "Feedback: more is sharper, more metallic and ringing.",
                (4.0, 7.0),
                "up for the jet; down for subtle",
            ),
            mix(
                "How much of the flanged sound. 50% gives the deepest notches.",
                (40.0, 60.0),
                "50% for the classic",
            ),
        ],
        starts: &[
            s(
                "Jet plane",
                &[("rate", 2.0), ("depth", 8.0), ("regen", 7.0), ("mix", 50.0)],
            ),
            s(
                "Subtle swirl",
                &[("rate", 3.0), ("depth", 4.0), ("regen", 3.0), ("mix", 35.0)],
            ),
            s(
                "Metallic",
                &[("rate", 1.0), ("depth", 6.0), ("regen", 9.0), ("mix", 50.0)],
            ),
        ],
    },
    About {
        id: "phaser",
        real: "MXR Phase 90",
        plain: "a mid-70s orange four-stage phaser",
        year: 1974,
        facts: "1974 \u{b7} American \u{b7} phaser",
        what: "Four filter stages swept by a slow wave give a gentle, swirling whoosh. One knob in the original: \
               slow it's a moving texture on rhythm parts, fast a bubbly warble.",
        heard: &[
            h("Eruption", "Van Halen", 1978),
            h("Atomic Punk", "Van Halen", 1978),
        ],
        try_for: &["Classic rock", "Funk rhythm", "70s rock"],
        tips: &[
            t(
                "speed",
                "How fast it swirls: slow is a texture, fast a bubbly warble.",
                (2.0, 4.5),
                "slow for rhythm",
            ),
            t(
                "depth",
                "How deep the swirl is.",
                (5.0, 8.0),
                "deep for the classic",
            ),
            mix(
                "How much of the phased sound; 50% gives the deepest swirl.",
                (40.0, 60.0),
                "50%",
            ),
        ],
        starts: &[
            s("Brown sound", &[("speed", 2.5), ("depth", 7.0), ("mix", 50.0)]),
            s("Funk bubble", &[("speed", 6.0), ("depth", 6.0), ("mix", 50.0)]),
            s("Slow sweep", &[("speed", 1.0), ("depth", 9.0), ("mix", 50.0)]),
            s("Subtle motion", &[("speed", 3.0), ("depth", 4.0), ("mix", 30.0)]),
        ],
    },
    About {
        id: "tremolo",
        real: "Boss TR-2 Tremolo and amp tremolo",
        plain: "a tremolo, as in old amps and a 90s compact pedal",
        year: 1997,
        facts: "amp tremolo since the 50s \u{b7} pedal 1997",
        what: "Pulses the volume up and down. Gentle and slow it's the shimmer of surf and old country; deep and \
               square it chops the sound into rhythmic stabs.",
        heard: &[
            h("How Soon Is Now?", "The Smiths", 1985),
            h("Crimson and Clover", "Tommy James and the Shondells", 1968),
            h("Boulevard of Broken Dreams", "Green Day", 2004),
        ],
        try_for: &["Surf", "Indie", "Country", "Ballads"],
        tips: &[
            t(
                "speed",
                "How fast it pulses. Lock it to the song's tempo so it grooves.",
                (3.0, 6.0),
                "in time with the song",
            ),
            t(
                "depth",
                "How deep the pulse dips: low is a shimmer, full is on-off chop.",
                (4.0, 7.0),
                "full with Square for chop",
            ),
            t(
                "wave",
                "Sine: smooth, like an old amp. Triangle: a bit more defined. Square: hard on-off chop.",
                (0.0, 1.0),
                "Sine for vintage",
            ),
        ],
        starts: &[
            s(
                "Vintage amp shimmer",
                &[("speed", 4.5), ("depth", 5.0), ("wave", 0.0)],
            ),
            s("Hard chop", &[("speed", 5.5), ("depth", 10.0), ("wave", 2.0)]),
            s("Slow throb", &[("speed", 2.0), ("depth", 7.0), ("wave", 1.0)]),
            s("Surf", &[("speed", 6.0), ("depth", 6.0), ("wave", 0.0)]),
        ],
    },
    About {
        id: "vibe",
        real: "Shin-ei Uni-Vibe",
        plain: "a late-60s Japanese photocell phaser",
        year: 1968,
        facts: "1968 \u{b7} Japan \u{b7} photocell phaser",
        what: "Built to imitate a rotating speaker with a lamp and light-sensitive cells: a throbbing, lopsided, \
               liquid phase. Its pulse leans and breathes rather than sweeping evenly.",
        heard: &[
            h("Machine Gun", "Band of Gypsys", 1970),
            h("Bridge of Sighs", "Robin Trower", 1974),
        ],
        try_for: &["Psychedelic", "Blues-rock", "70s rock"],
        tips: &[
            t(
                "speed",
                "How fast it throbs: slow for the classic lazy swirl, faster for a rotary-like warble.",
                (2.5, 5.0),
                "slow for the classic",
            ),
            t(
                "intensity",
                "How deep the throb is.",
                (6.0, 8.0),
                "deep for the classic",
            ),
            t(
                "mode",
                "Chorus mixes it with your guitar (the throb); Vibrato is the wet sound alone, a pitch wobble.",
                (0.0, 0.0),
                "Chorus for the famous sound",
            ),
        ],
        starts: &[
            s(
                "Machine gun throb",
                &[("speed", 3.5), ("intensity", 8.0), ("mode", 0.0)],
            ),
            s("Slow lean", &[("speed", 2.0), ("intensity", 7.0), ("mode", 0.0)]),
            s(
                "Warble vibrato",
                &[("speed", 5.0), ("intensity", 6.0), ("mode", 1.0)],
            ),
        ],
    },
    About {
        id: "rotary",
        real: "Leslie 122 rotating speaker",
        plain: "a 50s rotating-speaker cabinet",
        year: 1950,
        facts: "1950s \u{b7} American \u{b7} rotating speaker",
        what: "A cabinet with a spinning horn and drum, made for organs: slow it's a lush swirl, fast a warbling \
               chorale. Guitarists ran through them for a watery, three-dimensional sound.",
        heard: &[
            h("Badge", "Cream", 1969),
            h("Cold Shot", "Stevie Ray Vaughan", 1984),
        ],
        try_for: &["Psychedelic", "Blues", "Organ-like parts"],
        tips: &[
            t(
                "speed",
                "Slow (chorale) or fast (tremolo). Switching between them, the rotors speed up and slow down.",
                (0.0, 1.0),
                "Slow for chords, Fast for builds",
            ),
            t(
                "balance",
                "Between the drum (low, smooth) and the horn (high, whirly).",
                (4.0, 6.0),
                "about even",
            ),
            t(
                "depth",
                "How much the swirl moves the sound.",
                (5.0, 7.0),
                "by ear",
            ),
            mix(
                "How much of the cabinet against your straight guitar.",
                (60.0, 90.0),
                "mostly wet",
            ),
        ],
        starts: &[
            s(
                "Slow swirl",
                &[("speed", 0.0), ("balance", 5.0), ("depth", 6.0), ("mix", 75.0)],
            ),
            s(
                "Fast warble",
                &[("speed", 1.0), ("balance", 6.0), ("depth", 7.0), ("mix", 80.0)],
            ),
            s(
                "Subtle depth",
                &[("speed", 0.0), ("balance", 4.0), ("depth", 4.0), ("mix", 50.0)],
            ),
        ],
    },
    About {
        id: "octaver",
        real: "Boss OC-2 Octave",
        plain: "an early-80s compact analogue octaver",
        year: 1982,
        facts: "1982 \u{b7} Japan \u{b7} octave divider",
        what: "Adds a note an octave (or two) below what you play, tracking single notes. It makes riffs huge and \
               synth-like and turns a guitar into a bass; chords confuse it.",
        heard: &[h("Seven Nation Army", "The White Stripes", 2003)],
        try_for: &["Fat riffs", "Synth bass", "Garage rock"],
        tips: &[
            t(
                "sub",
                "The level of the octave below.",
                (5.0, 8.0),
                "single notes, neck pickup",
            ),
            t(
                "up",
                "The level of the second voice. Small amounts add a synth-like edge.",
                (0.0, 3.0),
                "mostly off",
            ),
            t(
                "dry",
                "Your own note's level. Turn it down to hear only the octave.",
                (5.0, 8.0),
                "0 for a bass-only sound",
            ),
        ],
        starts: &[
            s("Fat riff", &[("sub", 7.0), ("up", 0.0), ("dry", 7.0)]),
            s("Fake bass", &[("sub", 9.0), ("up", 0.0), ("dry", 0.0)]),
            s("Synth", &[("sub", 6.0), ("up", 4.0), ("dry", 5.0)]),
        ],
    },
    // -- delay ---------------------------------------------------------------------------------------------------------
    About {
        id: "analog_delay",
        real: "MXR Carbon Copy",
        plain: "a modern analogue (bucket-brigade) delay",
        year: 2008,
        facts: "2008 \u{b7} American \u{b7} analogue delay",
        what: "A bucket-brigade delay: each repeat is darker and softer than the one before, with a gentle wobble, \
               so the echoes sit behind your playing instead of on top of it.",
        heard: &[
            h("Comfortably Numb", "Pink Floyd", 1979),
            h("Run Like Hell", "Pink Floyd", 1979),
        ],
        try_for: &["Leads", "Ambient", "Blues", "Slapback"],
        tips: &[
            t(
                "time",
                "The gap between repeats. Under 120 ms is slapback, 300-500 ms a lead echo. Lock it to the song's \
                 tempo for echoes that groove.",
                (300.0, 500.0),
                "a 1/8 or dotted 1/8 at the song's tempo",
            ),
            t(
                "repeats",
                "How many echoes. Past about 8 they build up into a wash.",
                (3.0, 5.0),
                "2-4 echoes",
            ),
            mix(
                "How loud the echoes are against your guitar.",
                (20.0, 35.0),
                "behind the guitar, not on top",
            ),
            t(
                "mod",
                "A gentle pitch wobble on the repeats, like a tired tape.",
                (2.0, 4.0),
                "a little",
            ),
        ],
        starts: &[
            s(
                "Slapback",
                &[("time", 110.0), ("repeats", 1.0), ("mix", 35.0), ("mod", 0.0)],
            ),
            s(
                "Lead echo",
                &[("time", 420.0), ("repeats", 4.0), ("mix", 28.0), ("mod", 3.0)],
            ),
            s(
                "Warm wash",
                &[("time", 550.0), ("repeats", 7.0), ("mix", 35.0), ("mod", 5.0)],
            ),
            s(
                "Short doubler",
                &[("time", 60.0), ("repeats", 0.5), ("mix", 30.0), ("mod", 2.0)],
            ),
        ],
    },
    About {
        id: "tape_echo",
        real: "Roland RE-201 Space Echo",
        plain: "a mid-70s Japanese tape echo",
        year: 1974,
        facts: "1974 \u{b7} Japan \u{b7} tape echo",
        what: "A loop of tape running past several heads: repeats with wow, saturation and a warm, dark top. Turn \
               the repeats up and it runs away into swirling, self-oscillating echoes.",
        heard: &[
            h("Mystery Train", "Elvis Presley", 1955),
            h("Brighton Rock", "Queen", 1974),
        ],
        try_for: &["Rockabilly", "Dub", "Psychedelic", "Leads"],
        tips: &[
            t(
                "time",
                "The gap between repeats: short is rockabilly slapback, long a dub echo.",
                (250.0, 500.0),
                "lock it to the song's tempo",
            ),
            t(
                "repeats",
                "How many echoes. Past 9 it runs away into self-oscillation.",
                (3.0, 6.0),
                "past 9 for dub chaos",
            ),
            mix("How loud the echoes are.", (25.0, 40.0), "behind the guitar"),
            t(
                "wow",
                "The pitch wobble of worn tape. More is seasick.",
                (2.0, 4.0),
                "a little for character",
            ),
            t(
                "heads",
                "Which playback heads play. One head: one echo per Time. Heads together: echoes at 1, 2 and 3 times \
                 the Time at once, the galloping rhythm of the real unit, a wash with Repeats up.",
                (5.0, 6.0),
                "1 for slapback and plain echo; 2+3 or 1+2+3 for the classic gallop",
            ),
        ],
        starts: &[
            s(
                "Rockabilly slap",
                &[("time", 100.0), ("repeats", 1.5), ("mix", 40.0), ("wow", 2.0)],
            ),
            s(
                "Tape lead",
                &[("time", 380.0), ("repeats", 4.5), ("mix", 30.0), ("wow", 3.0)],
            ),
            s(
                "Dub runaway",
                &[("time", 520.0), ("repeats", 9.5), ("mix", 45.0), ("wow", 4.0)],
            ),
            s(
                "Worn tape",
                &[("time", 300.0), ("repeats", 3.0), ("mix", 30.0), ("wow", 7.0)],
            ),
            s(
                "Space gallop",
                &[
                    ("time", 160.0),
                    ("repeats", 4.5),
                    ("mix", 35.0),
                    ("wow", 3.0),
                    ("heads", 5.0),
                ],
            ),
        ],
    },
    About {
        id: "digital_delay",
        real: "Boss DD-3 Digital Delay",
        plain: "a mid-80s compact digital delay",
        year: 1986,
        facts: "1986 \u{b7} Japan \u{b7} digital delay",
        what: "Clean, exact repeats that sound just like what you played. Set to a dotted eighth at the song's \
               tempo, the echoes fill the gaps between notes into the famous rhythmic cascade.",
        heard: &[
            h("Where the Streets Have No Name", "U2", 1987),
            h("Run Like Hell", "Pink Floyd", 1979),
        ],
        try_for: &["Rhythmic delay", "Ambient", "Leads", "Pop"],
        tips: &[
            t(
                "time",
                "The gap between repeats. A dotted 1/8 at the song's tempo is the rhythmic U2 sound.",
                (300.0, 500.0),
                "lock it to the song's tempo",
            ),
            t(
                "repeats",
                "How many echoes; clean repeats build up fast.",
                (3.0, 5.0),
                "3-4 for rhythmic delay",
            ),
            mix(
                "How loud the echoes are. For dotted eighths, nearly as loud as the guitar.",
                (25.0, 45.0),
                "louder for rhythmic delay",
            ),
            t(
                "tone",
                "Brightness of the repeats: lower softens them so they don't clash with your playing.",
                (5.0, 7.5),
                "a touch darker than the guitar",
            ),
        ],
        starts: &[
            s(
                "U2 dotted eighths",
                &[("time", 375.0), ("repeats", 4.0), ("mix", 45.0), ("tone", 7.0)],
            ),
            s(
                "Ambient lead",
                &[("time", 600.0), ("repeats", 6.0), ("mix", 30.0), ("tone", 5.5)],
            ),
            s(
                "Slapback",
                &[("time", 90.0), ("repeats", 1.0), ("mix", 30.0), ("tone", 6.0)],
            ),
            s(
                "Quarter-note echo",
                &[("time", 500.0), ("repeats", 3.5), ("mix", 25.0), ("tone", 6.5)],
            ),
        ],
    },
    About {
        id: "mod_delay",
        real: "a modulated digital delay",
        plain: "a modulated digital delay",
        year: 1985,
        facts: "80s on \u{b7} modulated delay",
        what: "A digital delay whose repeats are gently chorused: wide, lush and dreamy, the echoes shimmering \
               instead of copying you exactly.",
        heard: &[
            h("Just Like Heaven", "The Cure", 1987),
            h("Where the Streets Have No Name", "U2", 1987),
        ],
        try_for: &["Ambient", "Dream pop", "80s cleans", "Ballads"],
        tips: &[
            t(
                "time",
                "The gap between repeats.",
                (300.0, 550.0),
                "lock it to the song's tempo",
            ),
            t("repeats", "How many echoes.", (3.0, 6.0), "more for a wash"),
            mix("How loud the echoes are.", (25.0, 40.0), "behind the guitar"),
            t(
                "rate",
                "How fast the repeats waver.",
                (2.0, 4.0),
                "slow for shimmer",
            ),
            t(
                "depth",
                "How far the repeats waver; too much sounds out of tune.",
                (3.0, 5.0),
                "subtle",
            ),
        ],
        starts: &[
            s(
                "Dreamy",
                &[
                    ("time", 450.0),
                    ("repeats", 5.0),
                    ("mix", 35.0),
                    ("rate", 3.0),
                    ("depth", 4.0),
                ],
            ),
            s(
                "Wide shimmer",
                &[
                    ("time", 320.0),
                    ("repeats", 3.0),
                    ("mix", 30.0),
                    ("rate", 2.0),
                    ("depth", 5.0),
                ],
            ),
            s(
                "Seasick wash",
                &[
                    ("time", 600.0),
                    ("repeats", 7.0),
                    ("mix", 40.0),
                    ("rate", 5.0),
                    ("depth", 7.0),
                ],
            ),
        ],
    },
    // -- reverb --------------------------------------------------------------------------------------------------------
    About {
        id: "spring",
        real: "Fender 6G15 Reverb unit",
        plain: "an early-60s American tube spring reverb",
        year: 1961,
        facts: "1961 \u{b7} American \u{b7} tube spring reverb",
        what: "A box of springs driven by a tube: the drippy, splashy 'boing' of surf guitar. Gentle, it's the \
               ambience on countless clean tones; turned up it's a wet, sloshing surf sound.",
        heard: &[
            h("Misirlou", "Dick Dale", 1962),
            h("Pipeline", "The Chantays", 1962),
        ],
        try_for: &["Surf", "Rockabilly", "Clean", "Blues"],
        tips: &[
            t(
                "dwell",
                "How hard the springs are driven: higher is splashier and drippier.",
                (4.0, 7.0),
                "high for surf",
            ),
            t(
                "tone",
                "Brightness of the reverb. Bright is the classic drip; darker sits back.",
                (4.0, 6.5),
                "about noon",
            ),
            mix(
                "How much reverb. Surf goes past 50%; for ambience 15-30% is plenty.",
                (15.0, 35.0),
                "higher for surf",
            ),
        ],
        starts: &[
            s("Clean ambience", &[("dwell", 4.0), ("tone", 5.0), ("mix", 20.0)]),
            s("Surf drip", &[("dwell", 8.0), ("tone", 6.5), ("mix", 55.0)]),
            s("Rockabilly", &[("dwell", 5.0), ("tone", 5.5), ("mix", 30.0)]),
            s("Dark and deep", &[("dwell", 6.0), ("tone", 3.0), ("mix", 35.0)]),
        ],
    },
    About {
        id: "plate",
        real: "EMT 140 plate reverb",
        plain: "a late-50s studio plate reverb",
        year: 1957,
        facts: "1957 \u{b7} German \u{b7} studio plate",
        what: "A large steel sheet in a box, set ringing by a speaker driver: a dense, bright, smooth reverb with \
               no echoes in it. The reverb of 60s and 70s studio records.",
        heard: &[
            h("Let It Be", "The Beatles", 1970),
            h("Bohemian Rhapsody", "Queen", 1975),
        ],
        try_for: &["Studio sheen", "Leads", "Ballads", "Classic records"],
        tips: &[
            t(
                "time",
                "How long it rings. 1.5-2.5 s is the classic plate.",
                (1.5, 3.0),
                "shorter for busy parts",
            ),
            t(
                "tone",
                "Brightness of the reverb. Plates are bright; darker sits back in the mix.",
                (4.0, 6.0),
                "about noon",
            ),
            t(
                "predelay",
                "A gap before the reverb starts, so the note stays clear in front of it.",
                (10.0, 40.0),
                "20-30 ms for leads",
            ),
            mix("How much reverb.", (15.0, 30.0), "less than you think"),
        ],
        starts: &[
            s(
                "Studio sheen",
                &[("time", 1.8), ("tone", 5.0), ("predelay", 20.0), ("mix", 20.0)],
            ),
            s(
                "Lead plate",
                &[("time", 2.5), ("tone", 5.5), ("predelay", 35.0), ("mix", 28.0)],
            ),
            s(
                "Short and bright",
                &[("time", 1.0), ("tone", 7.0), ("predelay", 10.0), ("mix", 18.0)],
            ),
            s(
                "Ballad",
                &[("time", 3.5), ("tone", 4.5), ("predelay", 25.0), ("mix", 30.0)],
            ),
        ],
    },
    About {
        id: "room",
        real: "a small studio room",
        plain: "a small studio room",
        year: 1970,
        facts: "a real room \u{b7} short reverb",
        what: "The short sound of the room around the amp. You barely hear it as reverb, but a dry modelled tone \
               suddenly sounds like a real amp in a real space.",
        heard: &[h("When the Levee Breaks", "Led Zeppelin", 1971)],
        try_for: &["Always on", "Rock", "Recording", "Rhythm"],
        tips: &[
            t(
                "size",
                "How big the room is: small is a booth, large a live room.",
                (3.0, 6.0),
                "small for rhythm",
            ),
            t(
                "tone",
                "Brightness of the room: hard walls or soft furnishings.",
                (4.0, 6.0),
                "about noon",
            ),
            mix(
                "How much room. Just enough that turning it off feels dry.",
                (10.0, 25.0),
                "subtle",
            ),
        ],
        starts: &[
            s("Just real", &[("size", 3.0), ("tone", 5.0), ("mix", 15.0)]),
            s("Live room", &[("size", 6.5), ("tone", 5.5), ("mix", 25.0)]),
            s("Big drum room", &[("size", 9.0), ("tone", 6.0), ("mix", 35.0)]),
        ],
    },
    About {
        id: "hall",
        real: "a concert hall",
        plain: "a concert hall",
        year: 1980,
        facts: "a big space \u{b7} long reverb",
        what: "A large, smooth hall with a long, even tail. Huge for ambient leads, swells and ballads; too much \
               on a busy part turns it to mush.",
        heard: &[
            h("Purple Rain", "Prince", 1984),
            h("Shine On You Crazy Diamond", "Pink Floyd", 1975),
        ],
        try_for: &["Ambient", "Ballads", "Swells", "Leads"],
        tips: &[
            t(
                "time",
                "How long it rings. Long tails blur fast parts.",
                (2.0, 4.0),
                "longer for slow parts",
            ),
            t(
                "tone",
                "Brightness of the tail: darker keeps it behind the guitar.",
                (4.0, 6.0),
                "a little dark",
            ),
            t(
                "predelay",
                "A gap before the hall answers, keeping the notes clear.",
                (20.0, 50.0),
                "more for leads",
            ),
            mix("How much hall.", (15.0, 30.0), "behind the guitar"),
        ],
        starts: &[
            s(
                "Ballad hall",
                &[("time", 3.0), ("tone", 5.0), ("predelay", 30.0), ("mix", 25.0)],
            ),
            s(
                "Ambient wash",
                &[("time", 6.5), ("tone", 4.0), ("predelay", 40.0), ("mix", 40.0)],
            ),
            s(
                "Big lead",
                &[("time", 2.5), ("tone", 5.5), ("predelay", 50.0), ("mix", 22.0)],
            ),
        ],
    },
];

// -- behind the panel (P21.5, D45): the amps' back panel and the drives' diodes -------------------------------------------

const SAG: &str = "How much the amp gives when you dig in: the power supply droops on hard notes, so they squash a \
                   little and bloom back. Up: spongy and compressed. Down: tight and immediate.";
const BIAS: &str = "How hot the power tubes idle. Cold (down): tighter, a bit gritty on quiet notes (crossover \
                    distortion). Hot (up): warmer and fatter, breaks up and squashes sooner. Noon is as it left the \
                    factory.";
const TUBES: &str = "The power tubes, and with them how the power stage breaks up. EL34: British bark, mid-forward, \
                     early crunch. 6L6: American, clean lows, glassy, more headroom. KT88: huge headroom, firm lows. \
                     EL84: chimey, breaks up early. 6V6: small and sweet, early, loose breakup.";
const RECT: &str = "What turns the mains into the tubes' supply. Tube: the supply droops on big chords, a looser, \
                    spongier feel (it adds to Sag). Solid: firm and punchy.";
const PM: &str = "How hard the power tubes work. Up: the cranked-amp sound (power-tube crunch, compression) at a \
                  sensible level. Down: clean power, the grit only from the preamp.";
const DIODES: &str = "The clipping diodes inside, as a modder would swap them. Silicon: the classic, even clip. \
                      Asymmetric: one side clips sooner, warmer, with more even harmonics. LED: more headroom, \
                      louder and more open, harder-edged when it clips. Germanium: softer and rounder, \
                      compresses early.";

/// An amp's back panel tips: its sag sweet spot, its own power tube and rectifier (`catalog::TUBES`, `RECTIFIERS`).
macro_rules! amp_back {
    ($sag:expr, $tube:expr, $rect:expr, $own:expr) => {
        &[
            t(
                "sag",
                SAG,
                ($sag - 1.0, $sag + 1.5),
                "the real amp's feel; more for blues, less for tight metal",
            ),
            t("bias", BIAS, (4.0, 6.0), "noon; a touch hot for a fatter lead"),
            t("tubes", TUBES, ($tube, $tube), $own),
            t("rectifier", RECT, ($rect, $rect), "as the real amp has it"),
            t(
                "power_master",
                PM,
                (4.0, 7.0),
                "up for power-tube crunch at home; down for clean headroom",
            ),
        ]
    };
}

/// Tips for the controls behind the panel, by model.
pub(super) static BACK: &[(&str, &[KnobTip])] = &[
    ("amp_clean", amp_back!(2.5, 1.0, 1.0, "the real amp's: four 6L6s")),
    ("amp_tweed", amp_back!(6.0, 4.0, 0.0, "the real amp's: two 6V6s")),
    (
        "amp_chime",
        amp_back!(3.5, 3.0, 0.0, "the real amp's: four EL84s"),
    ),
    (
        "amp_crunch",
        amp_back!(5.0, 0.0, 1.0, "the real amp's: four EL34s"),
    ),
    ("amp_lead", amp_back!(4.0, 0.0, 1.0, "the real amp's: four EL34s")),
    (
        "amp_modern",
        amp_back!(6.0, 1.0, 1.0, "the real amp's: four 6L6s"),
    ),
    (
        "amp_bass",
        amp_back!(3.5, 2.0, 1.0, "the real amp's: six 6550s, close to KT88s"),
    ),
    (
        "green",
        &[t("diodes", DIODES, (0.0, 0.0), "the pedal's own: a silicon pair")],
    ),
    (
        "blues",
        &[t(
            "diodes",
            DIODES,
            (1.0, 1.0),
            "the pedal's own: lopsided transistor clipping",
        )],
    ),
    (
        "gold",
        &[t("diodes", DIODES, (3.0, 3.0), "the pedal's own: germanium")],
    ),
    (
        "germanium",
        &[t("diodes", DIODES, (3.0, 3.0), "the pedal's own: germanium")],
    ),
    (
        "orange",
        &[t(
            "diodes",
            DIODES,
            (0.0, 0.0),
            "the pedal's own: silicon; LEDs are the classic mod",
        )],
    ),
    (
        "rodent",
        &[t(
            "diodes",
            DIODES,
            (0.0, 0.0),
            "the pedal's own: silicon; LEDs make it the louder variant",
        )],
    ),
];
