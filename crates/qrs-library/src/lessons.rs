//! Lessons (new in quickRS, P13.3, D25; v2 P16.1, D28; v3 P17.1, D29; no Python source): what the Lessons page lists
//! and the player's coach runs. A lesson is a JSON file (`quickRS_ lessons/lesson-format.md`): the built-in ones are
//! in `lessons/` (listed in `BUILTIN`), more can be put in `<data>/lessons/` (a file with the same id replaces the
//! built-in one). A lesson is in a course (courses listed by `course_order`, lessons numbered within their course).
//! Each item (a "riff" in the files) names its song by artist and title (found in the library by its canonical id,
//! so a psarc and its feedpak are the same song), the part, the stretch of the chart, and its kind: found by ear,
//! learned from the tab, the scale around it, or theory in pages. What's been done is kept in userdata.json under
//! "lessons" (`UserData::riff_progress`).

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::group::normalize;
use crate::scan::PartEntry;

/// The built-in lessons: (file name, its text). A new one is a file in `lessons/` and a line here.
const BUILTIN: &[(&str, &str)] = &[
    ("by-ear-1.json", include_str!("../lessons/by-ear-1.json")),
    ("reptilia.json", include_str!("../lessons/reptilia.json")),
    (
        "johnny-b-goode.json",
        include_str!("../lessons/johnny-b-goode.json"),
    ),
    ("by-ear-2.json", include_str!("../lessons/by-ear-2.json")),
    (
        "power-chords-1.json",
        include_str!("../lessons/power-chords-1.json"),
    ),
    (
        "pentatonic-box-1.json",
        include_str!("../lessons/pentatonic-box-1.json"),
    ),
    (
        "blues-shuffle-1.json",
        include_str!("../lessons/blues-shuffle-1.json"),
    ),
    (
        "bends-vibrato-1.json",
        include_str!("../lessons/bends-vibrato-1.json"),
    ),
    (
        "all-right-now.json",
        include_str!("../lessons/all-right-now.json"),
    ),
];

/// A play-along pass this accurate moves up a speed (or, at full speed, finishes the item).
pub const PASS: f64 = 0.9;

/// A course's place on the page when its lessons don't say (lowest first).
pub const COURSE_ORDER: u32 = 50;

/// The speeds on offer when an item doesn't say.
pub const SPEEDS: [f64; 4] = [1.0, 0.85, 0.7, 0.55];

/// Note names by pitch class.
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// How an item is taught (each kind has its own steps).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Found by ear, note by note, then played with the record (lesson 1's method).
    #[default]
    Ear,
    /// Seen on the tab, learned with the song waiting at each note, then played with the record.
    Learn,
    /// The scale around a stretch: its shape on the neck, run up and down, then played over the song.
    Scale,
    /// Theory in pages (the key, its chords, the scale, the notes leaned on, how a solo was thought), each page a
    /// step, some with something to play (`Page::act`).
    Theory,
}

/// A step of an item: its key in the progress, its name, and what you do (the Lessons page's notes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepInfo {
    pub key: &'static str,
    pub title: &'static str,
    pub what: &'static str,
}

const fn step(key: &'static str, title: &'static str, what: &'static str) -> StepInfo {
    StepInfo { key, title, what }
}

const EAR_STEPS: [StepInfo; 5] = [
    step(
        "listen",
        "Listen",
        "Hands off the guitar: hear it a few times with the tab hidden and hum along. Where does it go up, where \
         down? Which notes are long?",
    ),
    step(
        "first",
        "The first note",
        "The hardest one. Try notes on the string you're given; the coach says higher or lower until you've got it.",
    ),
    step(
        "notes",
        "Note by note",
        "Hear the riff up to the next note, find it, and again. Each note you find goes on the tab.",
    ),
    step(
        "play",
        "Play along",
        "With the record at full speed, every pass scored. Slower if you need it: a good pass brings the speed back \
         up, and one at full speed finishes it.",
    ),
    step(
        "check",
        "Check the tab",
        "Only now look at the tab. Your notes in another place on the neck are just as right.",
    ),
];

const LEARN_STEPS: [StepInfo; 4] = [
    step(
        "listen",
        "Listen",
        "Hear it with the tab in front of you, following the notes as they go by.",
    ),
    step(
        "practise",
        "Practise",
        "The whole part loops with the record, every pass scored. Play it out freely; Slower if you need it, and \
         note by note (W) for a tricky spot.",
    ),
    step(
        "record",
        "Record",
        "When you're ready: a count-in and one take of the whole part. A good enough take at full speed finishes \
         it.",
    ),
    step(
        "done",
        "Done",
        "Saved as a lick, to come back to whenever you like.",
    ),
];

const SCALE_STEPS: [StepInfo; 4] = [
    step(
        "see",
        "See it",
        "The scale's shape on the neck with its roots marked, and the stretch's own notes over it: the riff lives \
         inside the box.",
    ),
    step(
        "run",
        "Up and down",
        "Play the shape from its lowest note to its highest and back down. The coach lights each note as you play it.",
    ),
    step(
        "jam",
        "Over the song",
        "The stretch loops with the tab hidden. Play the riff, then make up your own with the shape: every note you \
         play lands on the neck.",
    ),
    step(
        "done",
        "Done",
        "Now you know where the song's notes live, and a shape to make up your own with.",
    ),
];

impl Kind {
    /// Its steps, in order.
    pub fn steps(self) -> &'static [StepInfo] {
        match self {
            Kind::Ear => &EAR_STEPS,
            Kind::Learn => &LEARN_STEPS,
            Kind::Scale => &SCALE_STEPS,
            // (its pages are its steps: `Riff::step_list`)
            Kind::Theory => &[],
        }
    }

    /// The step stored as `key` (none, or one this kind doesn't have such as v1's "tune": the first). A learn item's
    /// v2 steps, "learn" (note by note) and "play" (play along), are v3's Practise. A theory item's are its pages:
    /// `Riff::step_index`.
    pub fn step_index(self, key: &str) -> usize {
        let key = match (self, key) {
            (Kind::Learn, "learn" | "play") => "practise",
            _ => key,
        };
        self.steps().iter().position(|s| s.key == key).unwrap_or(0)
    }

    /// For the page: "by ear", "learn", "scale", "theory".
    pub fn label(self) -> &'static str {
        match self {
            Kind::Ear => "by ear",
            Kind::Learn => "learn",
            Kind::Scale => "scale",
            Kind::Theory => "theory",
        }
    }

    /// Times to hear an item before playing, when it doesn't say.
    pub fn listens(self) -> u32 {
        match self {
            Kind::Ear => 5,
            Kind::Learn => 1,
            Kind::Scale | Kind::Theory => 0,
        }
    }
}

/// A step as the page and the coach show it: its key in the progress, its name, and what you do (a theory item's
/// pages are its steps: "p0", the page's title, its activity).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepView {
    pub key: String,
    pub title: String,
    pub what: String,
}

/// A theory page's step key: "p0".
pub fn page_key(i: usize) -> String {
    format!("p{i}")
}

/// Interval names from a root, by semitones (0 to 11).
pub const INTERVALS: [&str; 12] = ["1", "b2", "2", "b3", "3", "4", "b5", "5", "b6", "6", "b7", "7"];

/// The interval from the root (a pitch class) up to pitch `midi`: "1", "b3", "5".
pub fn interval_name(root: i32, midi: i32) -> &'static str {
    INTERVALS[(midi - root).rem_euclid(12) as usize]
}

/// A note name's pitch class ("C" 0 ... "B" 11; "C#", "Db", "Bb", "F♯" too).
pub fn pitch_class(name: &str) -> Option<i32> {
    let mut ch = name.trim().chars();
    let mut pc: i32 = match ch.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    for c in ch {
        match c {
            '#' | '♯' => pc += 1,
            'b' | '♭' => pc -= 1,
            _ => return None,
        }
    }
    Some(pc.rem_euclid(12))
}

/// A pitch class's name ("C#").
pub fn pc_name(pc: i32) -> &'static str {
    NOTE_NAMES[pc.rem_euclid(12) as usize]
}

/// A scale item's scale: its name, root and the shape played (string, fret), lowest note first.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Scale {
    /// "B minor pentatonic".
    pub name: String,
    /// The root's note name ("B", "F#", "Bb").
    pub root: String,
    /// (string, fret) in the order it's played going up; string 0 is the lowest.
    pub shape: Vec<(i32, i32)>,
}

impl Scale {
    pub fn root_pc(&self) -> Option<i32> {
        pitch_class(&self.root)
    }

    /// The shape's notes (MIDI) on strings tuned to `opens` (None on a string it doesn't have).
    pub fn midis(&self, opens: &[i32]) -> Vec<Option<i32>> {
        self.shape
            .iter()
            .map(|&(s, f)| usize::try_from(s).ok().and_then(|s| opens.get(s)).map(|o| o + f))
            .collect()
    }

    /// The first note of the shape that isn't higher than the one before on this tuning (its index), if any.
    pub fn out_of_order(&self, opens: &[i32]) -> Option<usize> {
        let m = self.midis(opens);
        (0..m.len()).find(|&i| m[i].is_none() || (i > 0 && m[i] <= m[i - 1]))
    }

    /// The shape's pitch classes (a bit each): the scale's notes.
    pub fn pitch_classes(&self, opens: &[i32]) -> u16 {
        self.midis(opens)
            .into_iter()
            .flatten()
            .fold(0, |acc, m| acc | 1 << m.rem_euclid(12))
    }

    /// Up and down: the shape's notes (MIDI) going up, then back down without the top one twice.
    pub fn run(&self, opens: &[i32]) -> Vec<i32> {
        let up: Vec<i32> = self.midis(opens).into_iter().flatten().collect();
        let mut out = up.clone();
        out.extend(up.iter().rev().skip(1));
        out
    }
}

/// Whether pitch `midi` is one of `pcs` (`Scale::pitch_classes`).
pub fn in_scale(pcs: u16, midi: i32) -> bool {
    pcs & (1 << midi.rem_euclid(12)) != 0
}

/// Whether (string, fret) is on a neck of `strings` strings and 24 frets.
fn on_neck(s: i32, f: i32, strings: i32) -> bool {
    (0..strings).contains(&s) && (0..=24).contains(&f)
}

/// What's wrong with a scale on a neck of `strings` strings, if anything.
fn scale_problem(sc: &Scale, strings: i32) -> Option<String> {
    if sc.root_pc().is_none() {
        return Some(format!("the scale's root \"{}\" isn't a note name", sc.root));
    }
    if sc.shape.is_empty() {
        return Some("the scale's shape has no notes".into());
    }
    sc.shape
        .iter()
        .find(|&&(s, f)| !on_neck(s, f, strings))
        .map(|&(s, f)| {
            format!(
                "the shape's [{s}, {f}] isn't on the neck (strings 0 to {}, frets 0 to 24)",
                strings - 1
            )
        })
}

/// A moment of an item pointed out as you play (chart seconds): shown a moment before the play head gets there, as a
/// line on the coach's card and a flag on the tab.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Callout {
    pub at: f64,
    pub text: String,
}

/// How long before a callout's moment (seconds) it shows, and how long after.
pub const CALLOUT_BEFORE: f64 = 2.0;
pub const CALLOUT_AFTER: f64 = 3.0;

/// The callout to show at chart time `now`: of those whose window (`CALLOUT_BEFORE` before their moment to
/// `CALLOUT_AFTER` after) has `now` in it, the one whose moment is nearest.
pub fn callout_at(callouts: &[Callout], now: f64) -> Option<&Callout> {
    callouts
        .iter()
        .filter(|c| now >= c.at - CALLOUT_BEFORE && now <= c.at + CALLOUT_AFTER)
        .min_by(|a, b| (a.at - now).abs().total_cmp(&(b.at - now).abs()))
}

/// What a theory page has you do.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Activity {
    /// Just read it (Next).
    #[default]
    Read,
    /// Play the page's scale up and down (as a scale item's run).
    Run,
    /// Play each of its targets (the exact pitch, anywhere on the neck).
    Find,
    /// Play over its stretch looping at full speed, the tab hidden: your notes land on the diagram.
    Jam,
}

/// A chord box: its name and its frets, low string first (-1 not played, 0 open).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ChordBox {
    pub name: String,
    pub frets: Vec<i32>,
}

/// What a theory page's diagram shows: one neck (a scale's dots, their intervals, how much the stretch uses each
/// note, labelled marks) and a row of chord boxes.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Show {
    pub scale: Option<Scale>,
    /// Each dot labelled with its interval from the root.
    pub intervals: bool,
    /// The neck shaded by how much the item's stretch uses each note, the notes it leans on ranked.
    pub emphasis: bool,
    /// Extra dots: (string, fret, label).
    pub marks: Vec<(i32, i32, String)>,
    pub chords: Vec<ChordBox>,
}

impl Show {
    /// Whether there's a neck to draw.
    pub fn neck(&self) -> bool {
        self.scale.is_some() || !self.marks.is_empty()
    }
}

/// A page of a theory item.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Page {
    pub title: String,
    /// Paragraphs.
    pub text: Vec<String>,
    pub show: Show,
    #[serde(rename = "do")]
    pub act: Activity,
    /// Find: (string, fret) to play.
    pub targets: Vec<(i32, i32)>,
    /// Jam: its own stretch (chart seconds) and part (else the item's).
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub part: Option<String>,
}

impl Page {
    /// What you do on it, for the Lessons page.
    pub fn what(&self) -> String {
        match self.act {
            Activity::Read => "Read".into(),
            Activity::Run => "Play the scale up and down".into(),
            Activity::Find => format!(
                "Find {} note{} on the neck",
                self.targets.len(),
                if self.targets.len() == 1 { "" } else { "s" }
            ),
            Activity::Jam => "Jam over the song".into(),
        }
    }

    /// What's wrong with it, if anything (on a neck of `strings` strings).
    fn problem(&self, strings: i32) -> Option<String> {
        if self.title.trim().is_empty() {
            return Some("a page without a title".into());
        }
        let t = &self.title;
        let sh = &self.show;
        if let Some(sc) = &sh.scale
            && let Some(e) = scale_problem(sc, strings)
        {
            return Some(format!("page \"{t}\": {e}"));
        }
        if sh.scale.is_none() && (sh.intervals || sh.emphasis) {
            return Some(format!(
                "page \"{t}\": intervals and emphasis need a scale (its root)"
            ));
        }
        if let Some((s, f, _)) = sh.marks.iter().find(|m| !on_neck(m.0, m.1, strings)) {
            return Some(format!("page \"{t}\": the mark [{s}, {f}] isn't on the neck"));
        }
        if let Some(&(s, f)) = self.targets.iter().find(|m| !on_neck(m.0, m.1, strings)) {
            return Some(format!("page \"{t}\": the target [{s}, {f}] isn't on the neck"));
        }
        for c in &sh.chords {
            if c.frets.len() != strings as usize {
                return Some(format!(
                    "page \"{t}\": the chord {} has {} frets, not {strings} (low string first)",
                    c.name,
                    c.frets.len()
                ));
            }
            if c.frets.iter().any(|&f| !(-1..=24).contains(&f)) || c.frets.iter().all(|&f| f < 0) {
                return Some(format!(
                    "page \"{t}\": the chord {}'s frets must be -1 (not played), 0 (open) or up to 24",
                    c.name
                ));
            }
        }
        match self.act {
            Activity::Run | Activity::Jam if sh.scale.is_none() => {
                return Some(format!("page \"{t}\": a run or a jam needs a scale"));
            }
            Activity::Find if self.targets.is_empty() => {
                return Some(format!("page \"{t}\": find needs targets"));
            }
            _ => {}
        }
        match (self.start, self.end) {
            (Some(a), Some(b)) if b <= a => Some(format!("page \"{t}\": its end is before its start")),
            (Some(_), None) | (None, Some(_)) => Some(format!("page \"{t}\": a start needs an end")),
            _ => None,
        }
    }
}

fn default_course_order() -> u32 {
    COURSE_ORDER
}

fn default_pass() -> f64 {
    PASS
}

fn default_speeds() -> Vec<f64> {
    SPEEDS.to_vec()
}

/// An item of a lesson (a "riff" in the files): a stretch of one song's part, and how it's taught.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Riff {
    pub id: String,
    pub kind: Kind,
    /// "The opening riff".
    pub name: String,
    pub artist: String,
    pub title: String,
    /// The part's name ("Lead", "Rhythm").
    pub part: String,
    /// Chart seconds: the stretch looped while you listen and play along.
    pub start: f64,
    pub end: f64,
    /// By ear: how many of its notes are found one by one (0: all of them).
    pub phrase: usize,
    /// By ear: say up front which string (or strings) it's on.
    pub string_hint: bool,
    /// Times to hear it before playing, as the file has it (None: the kind's; `listens()`).
    #[serde(rename = "listens")]
    pub listens_set: Option<u32>,
    /// The play-along speeds on offer, in any order (`ladder()`: fastest first).
    #[serde(default = "default_speeds")]
    pub speeds: Vec<f64>,
    /// A play-along pass this accurate moves up a speed, or at full speed finishes it.
    #[serde(default = "default_pass")]
    pub pass: f64,
    /// A word about it, shown with it.
    pub about: String,
    /// Short teaching points (fingering, picking, muting), shown on the coach's cards.
    pub how: Vec<String>,
    /// A scale item's scale.
    pub scale: Option<Scale>,
    /// Moments pointed out as you play.
    pub callouts: Vec<Callout>,
    /// A theory item's pages.
    pub pages: Vec<Page>,
}

impl Default for Riff {
    fn default() -> Self {
        Riff {
            id: String::new(),
            kind: Kind::Ear,
            name: String::new(),
            artist: String::new(),
            title: String::new(),
            part: String::new(),
            start: 0.0,
            end: 0.0,
            phrase: 0,
            string_hint: false,
            listens_set: None,
            speeds: default_speeds(),
            pass: PASS,
            about: String::new(),
            how: Vec::new(),
            scale: None,
            callouts: Vec::new(),
            pages: Vec::new(),
        }
    }
}

impl Riff {
    /// The song's canonical id ("artist|title", as the library groups songs).
    pub fn canonical(&self) -> String {
        format!("{}|{}", normalize(&self.artist), normalize(&self.title))
    }

    /// Whether a song with this artist and title is the riff's: the same canonical id, or the same artist and one
    /// title inside the other ("Satisfaction" and "(I Can't Get No) Satisfaction").
    pub fn is_song(&self, artist: &str, title: &str) -> bool {
        let (a, t) = (normalize(artist), normalize(title));
        let (ra, rt) = (normalize(&self.artist), normalize(&self.title));
        if a != ra || t.is_empty() || rt.is_empty() {
            return false;
        }
        t == rt || t.contains(&rt) || rt.contains(&t)
    }

    /// Which of a song's parts (its arrangements, in order) is the riff's: the one with its name; where two share a
    /// name, "Combo 2" is the second (as the player labels them); else the arrangement's key ("combo2").
    pub fn part_of(&self, parts: &[PartEntry]) -> Option<usize> {
        let want = self.part.trim();
        if let Some(i) = parts.iter().position(|p| p.name.eq_ignore_ascii_case(want)) {
            return Some(i);
        }
        if let Some((name, n)) = want.rsplit_once(' ')
            && let Ok(n) = n.parse::<usize>()
            && n > 0
            && let Some((i, _)) = parts
                .iter()
                .enumerate()
                .filter(|(_, p)| p.name.eq_ignore_ascii_case(name.trim()))
                .nth(n - 1)
        {
            return Some(i);
        }
        parts.iter().position(|p| p.key.eq_ignore_ascii_case(want))
    }

    /// Times to hear it before playing.
    pub fn listens(&self) -> u32 {
        self.listens_set.unwrap_or(self.kind.listens())
    }

    /// The play-along speeds, fastest first, each once.
    pub fn ladder(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self.speeds.iter().copied().filter(|s| *s > 0.0).collect();
        if v.is_empty() {
            v = default_speeds();
        }
        v.sort_by(|a, b| b.total_cmp(a));
        v.dedup_by(|a, b| (*a - *b).abs() < 0.005);
        v
    }

    /// Its steps (a theory item's are its pages).
    pub fn step_list(&self) -> Vec<StepView> {
        if self.kind == Kind::Theory {
            return self
                .pages
                .iter()
                .enumerate()
                .map(|(i, p)| StepView {
                    key: page_key(i),
                    title: p.title.clone(),
                    what: p.what(),
                })
                .collect();
        }
        self.kind
            .steps()
            .iter()
            .map(|s| StepView {
                key: s.key.into(),
                title: s.title.into(),
                what: s.what.into(),
            })
            .collect()
    }

    /// The step stored as `key` (`Kind::step_index`; a theory item's page, "p3", within its pages).
    pub fn step_index(&self, key: &str) -> usize {
        if self.kind == Kind::Theory {
            return key
                .strip_prefix('p')
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0)
                .min(self.pages.len().saturating_sub(1));
        }
        self.kind.step_index(key)
    }

    /// Whether its part is a bass part (four strings).
    pub fn is_bass(&self) -> bool {
        self.part.to_lowercase().contains("bass")
    }

    /// What's wrong with it, if anything (`Lesson::parse` refuses the lesson).
    fn problem(&self) -> Option<String> {
        if self.id.trim().is_empty() || self.title.trim().is_empty() {
            return Some("an item without an id or a title".into());
        }
        let id = &self.id;
        if self.end <= self.start {
            return Some(format!("{id}: its end is before its start"));
        }
        if !(self.pass > 0.0 && self.pass <= 1.0) {
            return Some(format!("{id}: its pass must be between 0 and 1"));
        }
        if self.speeds.iter().any(|s| !(*s > 0.0 && *s <= 2.0)) {
            return Some(format!("{id}: its speeds must be between 0 and 2"));
        }
        if self.callouts.iter().any(|c| !c.at.is_finite()) {
            return Some(format!("{id}: a callout without a time"));
        }
        let strings = self.strings();
        match self.kind {
            Kind::Scale => {
                let Some(sc) = &self.scale else {
                    return Some(format!("{id}: a scale item needs a scale"));
                };
                scale_problem(sc, strings).map(|e| format!("{id}: {e}"))
            }
            Kind::Theory if self.pages.is_empty() => Some(format!("{id}: a theory item needs pages")),
            Kind::Theory => self
                .pages
                .iter()
                .find_map(|p| p.problem(strings))
                .map(|e| format!("{id}: {e}")),
            Kind::Ear | Kind::Learn => None,
        }
    }

    /// Its part's strings: 4 on a bass, else 6.
    pub fn strings(&self) -> i32 {
        if self.is_bass() { 4 } else { 6 }
    }
}

/// A lesson: what it teaches (the page shows it beside its items) and its items, easiest first.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Lesson {
    pub id: String,
    /// "Lesson 1" within its course.
    pub number: u32,
    /// The course it's part of ("Songs"); the page groups lessons by it.
    pub course: String,
    /// Courses are listed by this (lowest first), then by name.
    #[serde(default = "default_course_order")]
    pub course_order: u32,
    pub title: String,
    pub summary: String,
    /// Paragraphs.
    pub intro: Vec<String>,
    pub tips: Vec<String>,
    /// What you'll learn (the page's "You'll learn").
    pub teaches: Vec<String>,
    /// The lessons it assumes, by id ("Builds on").
    pub builds_on: Vec<String>,
    /// Where the method comes from (shown under the tips).
    pub credit: String,
    pub riffs: Vec<Riff>,
    /// The file it came from (None: built in).
    #[serde(skip)]
    pub file: Option<PathBuf>,
}

impl Lesson {
    /// Read a lesson file (v1's `steps` are left out: each kind has its own).
    pub fn parse(text: &str) -> Result<Lesson, String> {
        let l: Lesson = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if l.id.trim().is_empty() {
            return Err("it has no id".into());
        }
        if l.riffs.is_empty() {
            return Err("it has no riffs".into());
        }
        for (i, r) in l.riffs.iter().enumerate() {
            if let Some(e) = r.problem() {
                return Err(e);
            }
            if l.riffs[..i].iter().any(|o| o.id == r.id) {
                return Err(format!("{}: two items with this id", r.id));
            }
        }
        Ok(l)
    }

    /// "Lesson 1".
    pub fn label(&self) -> String {
        if self.number > 0 {
            format!("Lesson {}", self.number)
        } else {
            "Lesson".into()
        }
    }

    pub fn riff(&self, id: &str) -> Option<&Riff> {
        self.riffs.iter().find(|r| r.id == id)
    }

    /// How another lesson names it: "Songs, Lesson 1: Johnny B. Goode".
    pub fn reference(&self) -> String {
        if self.course.is_empty() {
            format!("{}: {}", self.label(), self.title)
        } else {
            format!("{}, {}: {}", self.course, self.label(), self.title)
        }
    }
}

/// Lessons in the page's order: by course (the lowest `course_order` of its lessons, then its name), then by number
/// within it. Returns their indexes.
pub fn lesson_order<L: std::borrow::Borrow<Lesson>>(lessons: &[L]) -> Vec<usize> {
    let rank = |course: &str| {
        lessons
            .iter()
            .map(|l| l.borrow())
            .filter(|l| l.course == course)
            .map(|l| l.course_order)
            .min()
            .unwrap_or(COURSE_ORDER)
    };
    let mut order: Vec<usize> = (0..lessons.len()).collect();
    order.sort_by(|&a, &b| {
        let (la, lb) = (lessons[a].borrow(), lessons[b].borrow());
        (rank(&la.course), &la.course, la.number, &la.id).cmp(&(
            rank(&lb.course),
            &lb.course,
            lb.number,
            &lb.id,
        ))
    });
    order
}

/// What's wrong with a set of lessons together: two lessons with one number in a course, a lesson building on one
/// that isn't there. Each is (lesson id, why).
pub fn set_problems<L: std::borrow::Borrow<Lesson>>(lessons: &[L]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (i, l) in lessons.iter().map(|l| l.borrow()).enumerate() {
        if l.number > 0
            && let Some(o) = lessons[..i]
                .iter()
                .map(|o| o.borrow())
                .find(|o| o.course == l.course && o.number == l.number)
        {
            out.push((
                l.id.clone(),
                format!(
                    "{} of \"{}\" is {} too: numbers are one per lesson within a course",
                    l.label(),
                    l.course,
                    o.id
                ),
            ));
        }
        for b in &l.builds_on {
            if *b == l.id {
                out.push((l.id.clone(), "it builds on itself".into()));
            } else if !lessons.iter().any(|o| o.borrow().id == *b) {
                out.push((l.id.clone(), format!("builds_on \"{b}\": no lesson has that id")));
            }
        }
    }
    out
}

/// The built-in lessons, each read: (file name, the lesson or why not).
pub fn builtin_files() -> Vec<(&'static str, Result<Lesson, String>)> {
    BUILTIN
        .iter()
        .map(|(name, text)| (*name, Lesson::parse(text)))
        .collect()
}

/// The built-in lessons (one that won't read is left out; a test keeps them all readable).
pub fn builtin() -> Vec<Lesson> {
    builtin_files().into_iter().filter_map(|(_, l)| l.ok()).collect()
}

/// The built-in lessons and those in `dir` (`*.json`; one with a built-in's id replaces it), in the page's order
/// (`lesson_order`). Files that couldn't be read come back as (file name, why), and so do problems between lessons
/// (`set_problems`: those lessons are kept).
pub fn load(dir: &Path) -> (Vec<Lesson>, Vec<(String, String)>) {
    let mut lessons = Vec::new();
    let mut errors = Vec::new();
    for (name, l) in builtin_files() {
        match l {
            Ok(l) => lessons.push(l),
            Err(e) => errors.push((format!("{name} (built in)"), e)),
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for path in files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| Lesson::parse(&t))
        {
            Ok(mut l) => {
                l.file = Some(path.clone());
                lessons.retain(|o| o.id != l.id);
                lessons.push(l);
            }
            Err(e) => errors.push((name, e)),
        }
    }
    let order = lesson_order(&lessons);
    let mut slots: Vec<Option<Lesson>> = lessons.into_iter().map(Some).collect();
    let lessons: Vec<Lesson> = order.into_iter().filter_map(|i| slots[i].take()).collect();
    // (kept: a lesson numbered as another, or building on one that isn't there, still teaches)
    for (id, why) in set_problems(&lessons) {
        errors.push((format!("{id} (kept)"), why));
    }
    (lessons, errors)
}

// -- progress -------------------------------------------------------------------------------------------------------

/// How far you've got with a riff, as kept in userdata.json (`lessons.<lesson>.riffs.<riff>`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RiffProgress {
    /// The step to carry on from (a key of the item's kind's steps; empty: not started).
    pub step: String,
    pub done: bool,
    pub listens: u32,
    /// Notes of the phrase found so far.
    pub found: u32,
    /// Wrong notes played while looking, the right note in another octave, hints taken.
    pub wrong: u32,
    pub octave: u32,
    pub hints: u32,
    /// Best loop accuracy at each speed: (percent, accuracy 0..1).
    pub best: Vec<(u32, f64)>,
    /// A scale's runs up and down: finished, and finished without a wrong note.
    pub runs: u32,
    pub clean: u32,
    /// A learn item's takes recorded.
    pub takes: u32,
    /// Seconds spent on it.
    pub seconds: f64,
    /// Epoch seconds: first opened, and finished (0: not yet).
    pub started: f64,
    pub finished: f64,
    /// Fields this version doesn't use, kept as they were.
    pub other: Map<String, Value>,
}

impl RiffProgress {
    /// The step to carry on from, in `kind`'s steps (v1's "tune" and anything unknown: the first; a learn item's v2
    /// "learn" and "play": Practise). A theory item's page: `Riff::step_index`.
    pub fn step_index(&self, kind: Kind) -> usize {
        kind.step_index(&self.step)
    }

    pub fn started(&self) -> bool {
        !self.step.is_empty() || self.done
    }

    /// The best accuracy at `speed` (0.7 -> 70%).
    pub fn best_at(&self, speed: f64) -> Option<f64> {
        let pct = (speed * 100.0).round() as u32;
        self.best.iter().find(|b| b.0 == pct).map(|b| b.1)
    }

    /// Note a loop pass at `speed`; true if it's the best there.
    pub fn record(&mut self, speed: f64, acc: f64) -> bool {
        let pct = (speed * 100.0).round() as u32;
        match self.best.iter_mut().find(|b| b.0 == pct) {
            Some(b) if b.1 >= acc => false,
            Some(b) => {
                b.1 = acc;
                true
            }
            None => {
                self.best.push((pct, acc));
                self.best.sort_by_key(|b| b.0);
                true
            }
        }
    }

    pub(crate) fn from_json(v: &Value) -> RiffProgress {
        let Some(m) = v.as_object() else {
            return RiffProgress::default();
        };
        let mut m = m.clone();
        let mut num = |k: &str| m.remove(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let listens = num("listens") as u32;
        let found = num("found") as u32;
        let wrong = num("wrong") as u32;
        let octave = num("octave") as u32;
        let hints = num("hints") as u32;
        let runs = num("runs") as u32;
        let clean = num("clean") as u32;
        let takes = num("takes") as u32;
        let seconds = num("seconds");
        let started = num("started");
        let finished = num("finished");
        let step = match m.remove("step") {
            Some(Value::String(s)) => s,
            _ => String::new(),
        };
        let done = m.remove("done").and_then(|v| v.as_bool()).unwrap_or(false);
        let mut best: Vec<(u32, f64)> = match m.remove("best") {
            Some(Value::Object(b)) => b
                .iter()
                .filter_map(|(k, v)| Some((k.parse::<u32>().ok()?, v.as_f64()?)))
                .collect(),
            _ => Vec::new(),
        };
        best.sort_by_key(|b| b.0);
        RiffProgress {
            step,
            done,
            listens,
            found,
            wrong,
            octave,
            hints,
            best,
            runs,
            clean,
            takes,
            seconds,
            started,
            finished,
            other: m,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        let mut m = self.other.clone();
        m.insert("step".into(), json!(self.step));
        m.insert("done".into(), json!(self.done));
        m.insert("listens".into(), json!(self.listens));
        m.insert("found".into(), json!(self.found));
        m.insert("wrong".into(), json!(self.wrong));
        m.insert("octave".into(), json!(self.octave));
        m.insert("hints".into(), json!(self.hints));
        let mut best = Map::new();
        for (pct, acc) in &self.best {
            best.insert(pct.to_string(), json!((acc * 1000.0).round() / 1000.0));
        }
        m.insert("best".into(), Value::Object(best));
        if self.runs > 0 {
            m.insert("runs".into(), json!(self.runs));
            m.insert("clean".into(), json!(self.clean));
        }
        if self.takes > 0 {
            m.insert("takes".into(), json!(self.takes));
        }
        m.insert("seconds".into(), json!(self.seconds.round()));
        m.insert("started".into(), json!(self.started));
        if self.finished > 0.0 {
            m.insert("finished".into(), json!(self.finished));
        }
        Value::Object(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDARD: [i32; 6] = [40, 45, 50, 55, 59, 64];

    #[test]
    fn every_built_in_lesson_reads() {
        for (name, l) in builtin_files() {
            if let Err(e) = l {
                panic!("{name}: {e}");
            }
        }
        let ids: Vec<String> = builtin().into_iter().map(|l| l.id).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            ids.len(),
            unique.len(),
            "two built-in lessons with one id: {ids:?}"
        );
    }

    #[test]
    fn lesson_one_is_built_in() {
        let l = builtin().into_iter().find(|l| l.id == "by-ear-1").unwrap();
        assert_eq!(l.number, 1);
        assert_eq!(l.riffs.len(), 4);
        let smoke = l.riff("smoke").unwrap();
        assert_eq!(smoke.canonical(), "deep purple|smoke on the water");
        assert_eq!(smoke.kind, Kind::Ear);
        assert_eq!(smoke.listens(), 5);
        assert_eq!(smoke.ladder(), vec![1.0, 0.85, 0.7, 0.55]);
        assert_eq!(smoke.pass, PASS);
        assert!(smoke.string_hint);
        assert!(!l.riff("bites-the-dust").unwrap().string_hint);
    }

    #[test]
    fn finding_a_riffs_song() {
        let l = &builtin()[0];
        let sat = l.riff("satisfaction").unwrap();
        assert!(sat.is_song("The Rolling Stones", "(I Can't Get No) Satisfaction"));
        assert!(sat.is_song("the rolling stones", "Satisfaction"));
        assert!(!sat.is_song("Devo", "Satisfaction"));
        assert!(!sat.is_song("The Rolling Stones", "Paint It, Black"));
    }

    #[test]
    fn a_riffs_part_by_name_label_or_key() {
        let part = |key: &str, name: &str| PartEntry {
            key: key.into(),
            name: name.into(),
            ..Default::default()
        };
        let parts = [
            part("lead", "Lead"),
            part("combo", "Combo"),
            part("combo2", "Combo"),
            part("bass", "Bass"),
        ];
        let riff = |p: &str| Riff {
            part: p.into(),
            ..Default::default()
        };
        assert_eq!(riff("lead").part_of(&parts), Some(0));
        assert_eq!(riff("Combo").part_of(&parts), Some(1));
        assert_eq!(riff("Combo 2").part_of(&parts), Some(2));
        assert_eq!(riff("combo2").part_of(&parts), Some(2));
        assert_eq!(riff("Combo 3").part_of(&parts), None);
        assert_eq!(riff("Rhythm").part_of(&parts), None);
    }

    #[test]
    fn a_lessons_folder_adds_and_replaces() {
        let dir = std::env::temp_dir().join(format!("qrs-lessons-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let two = r#"{"id": "by-ear-2", "number": 2, "title": "Root notes",
            "riffs": [{"id": "a", "artist": "X", "title": "Y", "start": 1, "end": 3}]}"#;
        std::fs::write(dir.join("two.json"), two).unwrap();
        let one = r#"{"id": "by-ear-1", "number": 1, "title": "Mine",
            "riffs": [{"id": "a", "artist": "X", "title": "Y", "start": 1, "end": 3}]}"#;
        std::fs::write(dir.join("one.json"), one).unwrap();
        std::fs::write(dir.join("bad.json"), "{").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a lesson").unwrap();
        let (ls, errs) = load(&dir);
        let one = ls.iter().find(|l| l.id == "by-ear-1").unwrap();
        assert_eq!(one.title, "Mine"); // replaced the built-in lesson 1
        assert!(one.file.is_some());
        assert!(ls.iter().any(|l| l.id == "by-ear-2"));
        assert_eq!(ls.iter().filter(|l| l.id == "by-ear-1").count(), 1);
        let two = ls.iter().find(|l| l.id == "by-ear-2").unwrap();
        assert_eq!(two.riffs[0].speeds, SPEEDS.to_vec()); // defaults
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].0, "bad.json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lesson_without_riffs_is_refused() {
        assert!(Lesson::parse(r#"{"id": "x", "riffs": []}"#).is_err());
        assert!(Lesson::parse(r#"{"riffs": [{"id": "a", "title": "t", "start": 1, "end": 2}]}"#).is_err());
        assert!(
            Lesson::parse(r#"{"id": "x", "riffs": [{"id": "a", "title": "t", "start": 3, "end": 2}]}"#)
                .is_err()
        );
    }

    /// A lesson with one item made of `item` (JSON fields after the id, title and stretch).
    fn one(item: &str) -> Result<Lesson, String> {
        Lesson::parse(&format!(
            r#"{{"id": "x", "riffs": [{{"id": "a", "title": "t", "start": 1, "end": 2{item}}}]}}"#
        ))
    }

    #[test]
    fn kinds_and_their_defaults() {
        let ear = &one("").unwrap().riffs[0];
        assert_eq!((ear.kind, ear.listens(), ear.pass), (Kind::Ear, 5, 0.9));
        assert_eq!(ear.ladder(), vec![1.0, 0.85, 0.7, 0.55]);
        let learn = &one(r#", "kind": "learn", "how": ["Fret it with your first finger."], "pass": 0.8"#)
            .unwrap()
            .riffs[0];
        assert_eq!((learn.kind, learn.listens(), learn.pass), (Kind::Learn, 1, 0.8));
        assert_eq!(learn.how, vec!["Fret it with your first finger.".to_string()]);
        let scale = &one(
            r#", "kind": "scale", "scale": {"name": "E minor pentatonic", "root": "E",
            "shape": [[0, 0], [0, 3]]}"#,
        )
        .unwrap()
        .riffs[0];
        assert_eq!((scale.kind, scale.listens()), (Kind::Scale, 0));
        assert_eq!(scale.scale.as_ref().unwrap().shape, vec![(0, 0), (0, 3)]);
        // listens as given, speeds in any order (fastest first, each once)
        let r = &one(r#", "listens": 3, "speeds": [0.7, 1.0, 0.85, 1.0]"#)
            .unwrap()
            .riffs[0];
        assert_eq!((r.listens(), r.ladder()), (3, vec![1.0, 0.85, 0.7]));
        // the kinds' steps
        let keys = |k: Kind| k.steps().iter().map(|s| s.key).collect::<Vec<_>>();
        assert_eq!(keys(Kind::Ear), ["listen", "first", "notes", "play", "check"]);
        assert_eq!(keys(Kind::Learn), ["listen", "practise", "record", "done"]);
        assert_eq!(keys(Kind::Scale), ["see", "run", "jam", "done"]);
        assert_eq!(Kind::Learn.label(), "learn");
    }

    #[test]
    fn items_that_cant_be_taught_are_refused() {
        assert!(one(r#", "kind": "sing""#).is_err());
        assert!(
            one(r#", "kind": "scale""#).is_err(),
            "a scale item without a scale"
        );
        let scale = |s: &str| one(&format!(r#", "kind": "scale", "scale": {s}"#));
        assert!(scale(r#"{"root": "H", "shape": [[0, 0]]}"#).is_err());
        assert!(scale(r#"{"root": "E", "shape": []}"#).is_err());
        assert!(
            scale(r#"{"root": "E", "shape": [[6, 0]]}"#).is_err(),
            "no seventh string"
        );
        assert!(scale(r#"{"root": "E", "shape": [[0, 25]]}"#).is_err());
        assert!(scale(r#"{"root": "Bb", "shape": [[0, 6], [5, 24]]}"#).is_ok());
        let bass = |s: &str| one(&format!(r#", "kind": "scale", "part": "Bass", "scale": {s}"#));
        assert!(
            bass(r#"{"root": "E", "shape": [[4, 0]]}"#).is_err(),
            "a bass has four strings"
        );
        assert!(bass(r#"{"root": "E", "shape": [[3, 2]]}"#).is_ok());
        assert!(one(r#", "pass": 1.5"#).is_err());
        assert!(one(r#", "speeds": [1.0, 0]"#).is_err());
        // two items with one id would share their progress
        let twice = r#"{"id": "x", "riffs": [{"id": "a", "title": "t", "start": 1, "end": 2},
            {"id": "a", "title": "u", "start": 3, "end": 4}]}"#;
        assert!(Lesson::parse(twice).is_err());
        // v1's steps are fine, and left out
        assert!(one("").is_ok());
        assert!(
            Lesson::parse(
                r#"{"id": "x", "steps": [["Tune up", "..."]], "riffs": [{"id": "a", "title": "t", "start": 1,
                "end": 2}]}"#
            )
            .is_ok()
        );
    }

    #[test]
    fn note_names() {
        assert_eq!(pitch_class("C"), Some(0));
        assert_eq!(pitch_class("c#"), Some(1));
        assert_eq!(pitch_class("Db"), Some(1));
        assert_eq!(pitch_class("Bb"), Some(10));
        assert_eq!(pitch_class("B"), Some(11));
        assert_eq!(pitch_class("Cb"), Some(11));
        assert_eq!(pitch_class("F♯"), Some(6));
        assert_eq!(pitch_class("H"), None);
        assert_eq!(pitch_class("C minor"), None);
        assert_eq!(pitch_class(""), None);
        assert_eq!(pc_name(13), "C#");
    }

    #[test]
    fn a_scale_shape_up_and_down() {
        // B minor pentatonic, box 1 (the format's example)
        let b = Scale {
            name: "B minor pentatonic".into(),
            root: "B".into(),
            shape: vec![
                (0, 7),
                (0, 10),
                (1, 7),
                (1, 9),
                (2, 7),
                (2, 9),
                (3, 7),
                (3, 9),
                (4, 7),
                (4, 10),
                (5, 7),
                (5, 10),
            ],
        };
        assert_eq!(b.out_of_order(&STANDARD), None);
        let run = b.run(&STANDARD);
        assert_eq!(run.len(), 23, "up 12, down 11: the top note once");
        assert_eq!(&run[..3], &[47, 50, 52]);
        assert_eq!(run[11], 74);
        assert_eq!(run[12], 71);
        assert_eq!(run[22], 47);
        // its pitch classes are the scale's: B D E F# A
        let pcs = b.pitch_classes(&STANDARD);
        assert_eq!(pcs.count_ones(), 5);
        for pc in [11, 2, 4, 6, 9] {
            assert!(in_scale(pcs, pc + 48), "{pc}");
        }
        assert!(!in_scale(pcs, 48)); // C
        assert!(in_scale(pcs, 47 - 24)); // a B anywhere
        // on another tuning the same frets can go out of order: Drop D's low string is two lower, so fine; a
        // shape written high string first isn't
        let down = Scale {
            shape: vec![(1, 7), (0, 10)],
            ..b.clone()
        };
        assert_eq!(down.out_of_order(&STANDARD), Some(1));
        let missing = Scale {
            shape: vec![(0, 7), (6, 0)],
            ..b
        };
        assert_eq!(missing.out_of_order(&STANDARD), Some(1));
        assert_eq!(missing.run(&STANDARD), vec![47]);
    }

    #[test]
    fn v1_progress_at_tune_up_carries_on_at_the_first_step() {
        // the owner's own data: riffs left at "tune"
        let p = RiffProgress::from_json(&json!({"step": "tune", "listens": 0, "best": {}}));
        assert!(p.started());
        assert_eq!(p.step_index(Kind::Ear), 0);
        assert_eq!(Kind::Ear.steps()[p.step_index(Kind::Ear)].key, "listen");
        assert_eq!(Kind::Scale.steps()[p.step_index(Kind::Scale)].key, "see");
        let notes = RiffProgress::from_json(&json!({"step": "notes"}));
        assert_eq!(notes.step_index(Kind::Ear), 2);
        assert_eq!(notes.step_index(Kind::Learn), 0, "not a learn step");
        assert_eq!(RiffProgress::default().step_index(Kind::Learn), 0);
    }

    #[test]
    fn progress_round_trips_and_keeps_unknown_fields() {
        let mut p = RiffProgress {
            step: "notes".into(),
            listens: 5,
            found: 4,
            wrong: 9,
            octave: 1,
            hints: 2,
            seconds: 312.4,
            started: 1_760_000_000.0,
            ..Default::default()
        };
        assert!(p.record(0.7, 0.8));
        assert!(!p.record(0.7, 0.75));
        assert!(p.record(0.7, 0.93));
        assert!(p.record(1.0, 0.5));
        assert_eq!(p.best_at(0.7), Some(0.93));
        assert_eq!(p.step_index(Kind::Ear), 2);
        let mut v = p.to_json();
        assert!(v.get("runs").is_none(), "only a scale's progress has runs");
        v.as_object_mut().unwrap().insert("later".into(), json!(1));
        let back = RiffProgress::from_json(&v);
        assert_eq!(back.found, 4);
        assert_eq!(back.best, vec![(70, 0.93), (100, 0.5)]);
        assert_eq!(back.seconds, 312.0);
        assert_eq!(back.other.get("later"), Some(&json!(1)));
        assert_eq!(RiffProgress::from_json(&json!(null)), RiffProgress::default());
        let runs = RiffProgress {
            step: "run".into(),
            runs: 3,
            clean: 1,
            ..Default::default()
        };
        let back = RiffProgress::from_json(&runs.to_json());
        assert_eq!((back.runs, back.clean), (3, 1));
        // a learn item's takes, written once there are some
        let takes = RiffProgress {
            step: "record".into(),
            takes: 2,
            ..Default::default()
        };
        assert_eq!(RiffProgress::from_json(&takes.to_json()).takes, 2);
        assert!(RiffProgress::default().to_json().get("takes").is_none());
    }

    #[test]
    fn a_learn_items_v2_steps_carry_on_at_practise() {
        // the owner's v2 progress: learning it note by note, or playing along
        for (stored, want) in [
            ("learn", "practise"),
            ("play", "practise"),
            ("listen", "listen"),
            ("record", "record"),
            ("done", "done"),
            ("tune", "listen"),
        ] {
            let p = RiffProgress::from_json(&json!({"step": stored}));
            assert_eq!(
                Kind::Learn.steps()[p.step_index(Kind::Learn)].key,
                want,
                "{stored}"
            );
        }
        // an ear item's "play" is still its play along
        assert_eq!(Kind::Ear.step_index("play"), 3);
        // the stored step is kept as it was until the coach writes it again
        let p = RiffProgress::from_json(&json!({"step": "play", "best": {"100": 0.5}}));
        assert_eq!(p.to_json()["step"], json!("play"));
    }

    /// A theory item made of `pages` (a JSON array).
    fn theory(pages: &str) -> Result<Lesson, String> {
        one(&format!(r#", "kind": "theory", "pages": {pages}"#))
    }

    const BB_BOX: &str = r#"{"name": "Bb minor pentatonic", "root": "Bb", "shape": [[0, 6], [0, 9], [1, 6],
        [1, 8], [2, 6], [2, 8], [3, 6], [3, 8], [4, 6], [4, 9], [5, 6], [5, 9]]}"#;

    #[test]
    fn a_lesson_v3_reads() {
        let text = format!(
            r#"{{"id": "johnny-b-goode", "course": "Songs", "course_order": 1, "number": 1,
            "title": "Johnny B. Goode", "teaches": ["Bb minor pentatonic, box 1", "the 12-bar blues in Bb"],
            "builds_on": ["by-ear-1"], "some_new_field": {{"later": true}},
            "riffs": [
              {{"id": "intro", "kind": "learn", "title": "Johnny B. Goode", "start": 10.83, "end": 27.35,
                "pass": 0.8, "callouts": [{{"at": 20.1, "text": "The double stops start here"}}]}},
              {{"id": "intro-theory", "kind": "theory", "name": "Where the intro comes from",
                "title": "Johnny B. Goode", "part": "Lead", "start": 10.83, "end": 27.35,
                "pages": [
                  {{"title": "The key: Bb", "text": ["One.", "Two."],
                    "show": {{"chords": [{{"name": "Bb", "frets": [6, 8, 8, 7, 6, 6]}},
                                       {{"name": "Eb", "frets": [-1, 6, 8, 8, 8, 6]}}]}}}},
                  {{"title": "The scale", "text": ["..."], "show": {{"scale": {BB_BOX}, "intervals": true}},
                    "do": "run"}},
                  {{"title": "What he leans on", "show": {{"scale": {BB_BOX}, "emphasis": true}}}},
                  {{"title": "Find the roots", "show": {{"scale": {BB_BOX}, "marks": [[1, 1, "Bb"], [3, 3, "Bb"]]}},
                    "do": "find", "targets": [[0, 6], [2, 8], [5, 6]]}},
                  {{"title": "Jam over a verse", "show": {{"scale": {BB_BOX}}}, "do": "jam",
                    "start": 28.19, "end": 45.13, "part": "Rhythm"}}
                ]}}
            ]}}"#
        );
        let l = Lesson::parse(&text).unwrap();
        assert_eq!(
            (l.course_order, l.number, l.label()),
            (1, 1, "Lesson 1".to_string())
        );
        assert_eq!(l.teaches.len(), 2);
        assert_eq!(l.builds_on, vec!["by-ear-1".to_string()]);
        assert_eq!(l.reference(), "Songs, Lesson 1: Johnny B. Goode");
        let intro = l.riff("intro").unwrap();
        assert_eq!(
            intro.callouts,
            vec![Callout {
                at: 20.1,
                text: "The double stops start here".into()
            }]
        );
        let th = l.riff("intro-theory").unwrap();
        assert_eq!(
            (th.kind, th.listens(), th.kind.label()),
            (Kind::Theory, 0, "theory")
        );
        assert_eq!(th.pages.len(), 5);
        let acts: Vec<Activity> = th.pages.iter().map(|p| p.act).collect();
        assert_eq!(
            acts,
            [
                Activity::Read,
                Activity::Run,
                Activity::Read,
                Activity::Find,
                Activity::Jam
            ]
        );
        assert_eq!(th.pages[0].show.chords[1].frets, vec![-1, 6, 8, 8, 8, 6]);
        assert!(!th.pages[0].show.neck());
        assert!(th.pages[1].show.intervals && th.pages[1].show.neck());
        assert_eq!(th.pages[3].show.marks[1], (3, 3, "Bb".to_string()));
        assert_eq!(th.pages[3].targets, vec![(0, 6), (2, 8), (5, 6)]);
        assert_eq!(
            (th.pages[4].start, th.pages[4].end, th.pages[4].part.as_deref()),
            (Some(28.19), Some(45.13), Some("Rhythm"))
        );
        // the pages are its steps
        let steps = th.step_list();
        assert_eq!(steps.len(), 5);
        assert_eq!(
            (steps[2].key.as_str(), steps[2].title.as_str()),
            ("p2", "What he leans on")
        );
        assert_eq!(steps[3].what, "Find 3 notes on the neck");
        assert_eq!(th.step_index("p3"), 3);
        assert_eq!(th.step_index("p99"), 4, "within its pages");
        assert_eq!(th.step_index("listen"), 0);
        assert_eq!(th.step_index(""), 0);
        // the others' steps as before
        assert_eq!(intro.step_list()[1].key, "practise");
        assert_eq!(intro.step_index("play"), 1);
        // the defaults: course order 50, nothing taught or built on
        let d = one("").unwrap();
        assert_eq!(
            (d.course_order, d.teaches.len(), d.builds_on.len()),
            (COURSE_ORDER, 0, 0)
        );
    }

    #[test]
    fn theory_pages_that_cant_be_shown_are_refused() {
        let ok = |pages: &str| {
            let r = theory(pages);
            assert!(r.is_ok(), "{pages}: {r:?}");
        };
        let bad = |pages: &str| {
            let r = theory(pages);
            assert!(r.is_err(), "{pages} should be refused");
            r.unwrap_err()
        };
        ok(r#"[{"title": "Just words", "text": ["Hello."]}]"#);
        bad("[]");
        assert!(one(r#", "kind": "theory""#).is_err(), "no pages");
        bad(r#"[{"text": ["no title"]}]"#);
        let e = bad(r#"[{"title": "x", "do": "dance"}]"#);
        assert!(e.contains("dance"), "{e}");
        // chords: six frets, low string first, -1 to 24, not all unplayed
        ok(r#"[{"title": "x", "show": {"chords": [{"name": "E5", "frets": [0, 2, 2, -1, -1, -1]}]}}]"#);
        let e = bad(r#"[{"title": "x", "show": {"chords": [{"name": "E5", "frets": [0, 2, 2]}]}}]"#);
        assert!(e.contains("3 frets, not 6"), "{e}");
        bad(r#"[{"title": "x", "show": {"chords": [{"name": "E", "frets": [0, 2, 2, 1, 0, 25]}]}}]"#);
        bad(r#"[{"title": "x", "show": {"chords": [{"name": "E", "frets": [0, 2, 2, 1, 0, -2]}]}}]"#);
        bad(r#"[{"title": "x", "show": {"chords": [{"name": "N", "frets": [-1, -1, -1, -1, -1, -1]}]}}]"#);
        // a bass part's chord boxes have four strings
        let bass = one(
            r#", "kind": "theory", "part": "Bass", "pages": [{"title": "x", "show": {"chords":
            [{"name": "E5", "frets": [0, 2, 2, -1]}]}}]"#,
        );
        assert!(bass.is_ok(), "{bass:?}");
        // marks and targets on the neck
        bad(r#"[{"title": "x", "show": {"marks": [[6, 1, "?"]]}}]"#);
        bad(r#"[{"title": "x", "show": {"marks": [[0, 25, "?"]]}}]"#);
        ok(r#"[{"title": "x", "show": {"marks": [[5, 24, "top"]]}}]"#);
        bad(r#"[{"title": "x", "do": "find", "targets": [[-1, 3]]}]"#);
        // what an activity needs: a run or a jam a scale, find its targets
        bad(r#"[{"title": "x", "do": "run"}]"#);
        bad(r#"[{"title": "x", "do": "jam"}]"#);
        bad(r#"[{"title": "x", "do": "find"}]"#);
        ok(&format!(
            r#"[{{"title": "x", "show": {{"scale": {BB_BOX}}}, "do": "run"}}]"#
        ));
        ok(r#"[{"title": "x", "do": "find", "targets": [[0, 6]]}]"#);
        // intervals and emphasis need a scale's root; the scale is checked as a scale item's
        bad(r#"[{"title": "x", "show": {"intervals": true}}]"#);
        bad(r#"[{"title": "x", "show": {"emphasis": true}}]"#);
        bad(r#"[{"title": "x", "show": {"scale": {"root": "H", "shape": [[0, 0]]}}}]"#);
        bad(r#"[{"title": "x", "show": {"scale": {"root": "E", "shape": [[0, 0], [6, 2]]}}}]"#);
        // a page's own stretch: both ends, the right way round
        bad(&format!(
            r#"[{{"title": "x", "show": {{"scale": {BB_BOX}}}, "do": "jam", "start": 5}}]"#
        ));
        bad(&format!(
            r#"[{{"title": "x", "show": {{"scale": {BB_BOX}}}, "do": "jam", "start": 5, "end": 4}}]"#
        ));
        // a callout needs a time
        assert!(
            one(r#", "callouts": [{"text": "here"}]"#).is_ok(),
            "(0 is a time)"
        );
    }

    #[test]
    fn interval_names_from_the_root() {
        let bb = pitch_class("Bb").unwrap();
        assert_eq!(interval_name(bb, 46), "1"); // Bb2
        assert_eq!(interval_name(bb, 49), "b3"); // Db
        assert_eq!(interval_name(bb, 50), "3"); // D
        assert_eq!(interval_name(bb, 51), "4"); // Eb
        assert_eq!(interval_name(bb, 52), "b5"); // E
        assert_eq!(interval_name(bb, 53), "5"); // F
        assert_eq!(interval_name(bb, 56), "b7"); // Ab
        assert_eq!(interval_name(bb, 45), "7"); // A, below the root
        assert_eq!(interval_name(0, 62), "2");
        assert_eq!(interval_name(0, 68), "b6");
        assert_eq!(interval_name(0, 69), "6");
        assert_eq!(interval_name(0, 61), "b2");
    }

    #[test]
    fn a_callout_shows_a_moment_before_and_after() {
        let cs = [
            Callout {
                at: 30.0,
                text: "a".into(),
            },
            Callout {
                at: 33.0,
                text: "b".into(),
            },
        ];
        let at = |now: f64| callout_at(&cs, now).map(|c| c.text.as_str());
        assert_eq!(at(27.9), None, "not yet");
        assert_eq!(at(28.0), Some("a"), "two seconds before");
        assert_eq!(at(30.0), Some("a"));
        assert_eq!(at(31.4), Some("a"));
        assert_eq!(at(31.6), Some("b"), "the next one's moment is nearer");
        assert_eq!(at(35.9), Some("b"));
        assert_eq!(at(36.0), Some("b"), "three seconds after");
        assert_eq!(at(36.1), None);
        assert_eq!(callout_at(&[], 1.0), None);
    }

    #[test]
    fn courses_in_their_order_and_numbers_within_them() {
        let l = |id: &str, course: &str, order: u32, number: u32| Lesson {
            id: id.into(),
            course: course.into(),
            course_order: order,
            number,
            ..Default::default()
        };
        let ls = vec![
            l("all-right-now", "Rock and blues foundations", 3, 5),
            l("by-ear-2", "Ear training", 2, 2),
            l("reptilia", "Songs", 1, 2),
            l("power-chords-1", "Rock and blues foundations", 3, 1),
            l("by-ear-1", "Ear training", 2, 1),
            l("johnny-b-goode", "Songs", 1, 1),
            l("mine", "Extras", COURSE_ORDER, 1),
            l("also-mine", "Another", COURSE_ORDER, 1),
        ];
        let order: Vec<&str> = lesson_order(&ls).into_iter().map(|i| ls[i].id.as_str()).collect();
        assert_eq!(
            order,
            [
                "johnny-b-goode",
                "reptilia",
                "by-ear-1",
                "by-ear-2",
                "power-chords-1",
                "all-right-now",
                "also-mine",
                "mine"
            ]
        );
        // a course is placed by its lowest course_order (one lesson missing it doesn't split the course)
        let ls2 = vec![
            l("a", "Songs", COURSE_ORDER, 3),
            l("b", "Ear training", 2, 1),
            l("c", "Songs", 1, 1),
        ];
        let order: Vec<&str> = lesson_order(&ls2)
            .into_iter()
            .map(|i| ls2[i].id.as_str())
            .collect();
        assert_eq!(order, ["c", "a", "b"]);
        // numbers are per course: two "Lesson 1"s in different courses are fine, two in one aren't
        assert!(set_problems(&ls).is_empty(), "{:?}", set_problems(&ls));
        let mut twice = ls.clone();
        twice.push(l("reptilia-2", "Songs", 1, 2));
        let p = set_problems(&twice);
        assert_eq!(p.len(), 1, "{p:?}");
        assert_eq!(p[0].0, "reptilia-2");
        assert!(p[0].1.contains("reptilia"), "{p:?}");
        // builds_on: ids that are there
        let mut on = ls.clone();
        on[1].builds_on = vec!["by-ear-1".into(), "nope".into(), "by-ear-2".into()];
        let p = set_problems(&on);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p.iter().any(|x| x.1.contains("\"nope\"")));
        assert!(p.iter().any(|x| x.1.contains("itself")));
    }

    #[test]
    fn the_built_in_lessons_are_in_three_courses() {
        let ls = builtin();
        assert!(set_problems(&ls).is_empty(), "{:?}", set_problems(&ls));
        let order: Vec<(String, u32, String)> = lesson_order(&ls)
            .into_iter()
            .map(|i| (ls[i].course.clone(), ls[i].number, ls[i].id.clone()))
            .collect();
        let want = [
            ("Songs", 1, "johnny-b-goode"),
            ("Songs", 2, "reptilia"),
            ("Ear training", 1, "by-ear-1"),
            ("Ear training", 2, "by-ear-2"),
            ("Rock and blues foundations", 1, "power-chords-1"),
            ("Rock and blues foundations", 2, "pentatonic-box-1"),
            ("Rock and blues foundations", 3, "blues-shuffle-1"),
            ("Rock and blues foundations", 4, "bends-vibrato-1"),
            ("Rock and blues foundations", 5, "all-right-now"),
        ];
        assert_eq!(
            order,
            want.iter()
                .map(|(c, n, id)| (c.to_string(), *n, id.to_string()))
                .collect::<Vec<_>>()
        );
    }
}
