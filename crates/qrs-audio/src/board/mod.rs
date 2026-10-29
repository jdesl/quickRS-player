//! The pedalboard (PLAN.md Phase 14, D26): a tone made of blocks (pedals, an amp, a cab, racks) in an order you
//! choose, each switched on or off and turned with its own knobs. It runs as one `ToneProcessor` on the input
//! side; knob turns, switches, new blocks, removed blocks and moves are made in place on the audio thread
//! (`BoardCmd`), so delay and reverb tails keep ringing while you change things. A new board altogether (a
//! preset, a song's tone change) is a whole-tone swap, crossfaded by the input side.
//!
//! Bypass fades each block in and out over 10 ms. Delays and reverbs are "wet only" blocks: the dry signal passes
//! by them and they add their echoes, so switching one off stops what goes in and lets its tail ring out.

pub mod catalog;
pub mod dsp;
pub mod picture;
pub mod rocksmith;

pub mod amp;
mod cab;
mod delay;
mod drive;
mod dynamics;
mod eq;
mod filter;
mod modfx;
mod reverb;

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub use catalog::{Face, Kind, KnobDef, MODELS, ModelDef, Unit};

/// Blocks on a board at most.
pub const MAX_BLOCKS: usize = 12;
/// Knobs on a model at most.
pub const MAX_KNOBS: usize = 12;

/// One model running on the audio thread. Everything it needs is allocated by its constructor.
pub trait Pedal: Send {
    /// Process mono samples in place.
    fn process(&mut self, x: &mut [f32]);
    /// The knob values, in the model's knob order and units (the catalogue's `KnobDef`s). Called whenever any
    /// of them changes; the pedal glides to them.
    fn set(&mut self, v: &[f32; MAX_KNOBS]);
    fn reset(&mut self) {}
    /// Delays and reverbs: `process` leaves only the wet signal (the board adds the dry), so switching it off can
    /// let the tail ring out.
    fn wet_only(&self) -> bool {
        false
    }
}

static NEXT_UID: AtomicU32 = AtomicU32::new(1);

/// A fresh id for a block (it follows the block through edits, so the running board can find it).
pub fn new_uid() -> u32 {
    NEXT_UID.fetch_add(1, Ordering::Relaxed)
}

/// One block: a model, on or off, and its knob values (in the model's knob order and units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    pub model: usize,
    pub on: bool,
    pub v: [f32; MAX_KNOBS],
    pub uid: u32,
}

impl Block {
    /// A model at its default settings.
    pub fn new(model: usize) -> Block {
        Block {
            model,
            on: true,
            v: MODELS[model].defaults(),
            uid: new_uid(),
        }
    }

    pub fn of(id: &str) -> Option<Block> {
        catalog::find(id).map(Block::new)
    }

    pub fn def(&self) -> &'static ModelDef {
        &MODELS[self.model]
    }

    pub fn get(&self, key: &str) -> Option<f32> {
        self.def().knob_index(key).map(|i| self.v[i])
    }

    /// Set a knob by key (clamped to its range); unknown keys are ignored.
    pub fn set(&mut self, key: &str, v: f32) -> &mut Block {
        if let Some(i) = self.def().knob_index(key) {
            self.v[i] = self.def().knobs[i].clamp(v);
        }
        self
    }

    pub fn with(mut self, key: &str, v: f32) -> Block {
        self.set(key, v);
        self
    }

    /// The same model with another one's knobs carried over where the keys match (changing model in a kind).
    pub fn changed_to(&self, model: usize) -> Block {
        let mut b = Block {
            model,
            on: self.on,
            v: MODELS[model].defaults(),
            uid: self.uid,
        };
        for (i, k) in self.def().knobs.iter().enumerate() {
            if let Some(j) = MODELS[model].knob_index(k.key)
                && MODELS[model].knobs[j].unit == k.unit
            {
                b.v[j] = MODELS[model].knobs[j].clamp(self.v[i]);
            }
        }
        b
    }

    pub fn to_json(&self) -> Value {
        let mut knobs = Map::new();
        for (i, k) in self.def().knobs.iter().enumerate() {
            knobs.insert(
                k.key.into(),
                Value::from((self.v[i] as f64 * 1000.0).round() / 1000.0),
            );
        }
        let mut m = Map::new();
        m.insert("model".into(), Value::from(self.def().id));
        m.insert("on".into(), Value::from(self.on));
        m.insert("knobs".into(), Value::Object(knobs));
        Value::Object(m)
    }

    /// A block from JSON (None for a model this version doesn't know).
    pub fn from_json(v: &Value) -> Option<Block> {
        let mut b = Block::of(v.get("model")?.as_str()?)?;
        b.on = v.get("on").and_then(Value::as_bool).unwrap_or(true);
        if let Some(knobs) = v.get("knobs").and_then(Value::as_object) {
            for (k, x) in knobs {
                if let Some(f) = x.as_f64() {
                    b.set(k, f as f32);
                }
            }
        }
        Some(b)
    }
}

/// A board: its blocks in signal order and an output level.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(from = "Value", into = "Value")]
pub struct Board {
    pub blocks: Vec<Block>,
    pub level_db: f32,
}

impl From<Value> for Board {
    fn from(v: Value) -> Board {
        Board::from_json(&v)
    }
}

impl From<Board> for Value {
    fn from(b: Board) -> Value {
        b.to_json()
    }
}

impl Board {
    pub fn new(blocks: Vec<Block>) -> Board {
        Board {
            blocks,
            level_db: 0.0,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert(
            "blocks".into(),
            Value::Array(self.blocks.iter().map(Block::to_json).collect()),
        );
        m.insert(
            "level".into(),
            Value::from((self.level_db as f64 * 100.0).round() / 100.0),
        );
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Board {
        let blocks = v
            .get("blocks")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Block::from_json).take(MAX_BLOCKS).collect())
            .unwrap_or_default();
        Board {
            blocks,
            level_db: v
                .get("level")
                .and_then(Value::as_f64)
                .map(|l| (l as f32).clamp(-24.0, 12.0))
                .unwrap_or(0.0),
        }
    }

    /// The same board with fresh block ids (a copy that edits apart from its original).
    pub fn renewed(&self) -> Board {
        let mut b = self.clone();
        for blk in &mut b.blocks {
            blk.uid = new_uid();
        }
        b
    }

    pub fn index_of(&self, uid: u32) -> Option<usize> {
        self.blocks.iter().position(|b| b.uid == uid)
    }

    /// The first block of a kind.
    pub fn first(&self, kind: Kind) -> Option<usize> {
        self.blocks.iter().position(|b| b.def().kind == kind)
    }
}

// -- the running board ----------------------------------------------------------------------------------------------

/// Changes to a running board, made on the audio thread without allocating. Blocks are found by uid.
pub enum BoardCmd {
    Knob {
        uid: u32,
        knob: usize,
        value: f32,
    },
    Bypass {
        uid: u32,
        on: bool,
    },
    /// A new block (its pedal built off the audio thread), added at the end; an `Order` puts it in place.
    Insert {
        block: Block,
        pedal: Box<dyn Pedal>,
    },
    /// The block's model changed: the new pedal takes its place.
    Replace {
        block: Block,
        pedal: Box<dyn Pedal>,
    },
    /// Fade the block out (a delay or reverb lets its tail ring first), then drop it.
    Remove {
        uid: u32,
    },
    /// The blocks in this order (the first `n` uids).
    Order {
        uids: [u32; MAX_BLOCKS],
        n: usize,
    },
    Level(f32),
}

impl BoardCmd {
    /// The pedal a command carries (to be freed when the tone isn't a board).
    pub fn into_pedal(self) -> Option<Box<dyn Pedal>> {
        match self {
            BoardCmd::Insert { pedal, .. } | BoardCmd::Replace { pedal, .. } => Some(pedal),
            _ => None,
        }
    }
}

/// The pedal for a block, set to its knobs (allocates: off the audio thread).
pub fn make_pedal(b: &Block, rate: u32) -> Box<dyn Pedal> {
    let mut p = (b.def().make)(rate as f32);
    p.set(&b.v);
    p.reset();
    p
}

/// The commands that turn the running `old` board into `new`, or None when so much changed that a fresh board is
/// better (fewer than half of `new`'s blocks were on `old`). Builds new blocks' pedals (allocates).
pub fn diff(old: &Board, new: &Board, rate: u32) -> Option<Vec<BoardCmd>> {
    let kept = new
        .blocks
        .iter()
        .filter(|b| old.index_of(b.uid).is_some())
        .count();
    if !new.blocks.is_empty() && kept * 2 < new.blocks.len() {
        return None;
    }
    let mut cmds = Vec::new();
    for b in &old.blocks {
        if new.index_of(b.uid).is_none() {
            cmds.push(BoardCmd::Remove { uid: b.uid });
        }
    }
    for b in &new.blocks {
        match old.index_of(b.uid).map(|i| old.blocks[i]) {
            None => cmds.push(BoardCmd::Insert {
                block: *b,
                pedal: make_pedal(b, rate),
            }),
            Some(o) if o.model != b.model => cmds.push(BoardCmd::Replace {
                block: *b,
                pedal: make_pedal(b, rate),
            }),
            Some(o) => {
                for k in 0..b.def().knobs.len() {
                    if o.v[k] != b.v[k] {
                        cmds.push(BoardCmd::Knob {
                            uid: b.uid,
                            knob: k,
                            value: b.v[k],
                        });
                    }
                }
                if o.on != b.on {
                    cmds.push(BoardCmd::Bypass { uid: b.uid, on: b.on });
                }
            }
        }
    }
    let order_same = old
        .blocks
        .iter()
        .map(|b| b.uid)
        .filter(|u| new.index_of(*u).is_some())
        .eq(new
            .blocks
            .iter()
            .map(|b| b.uid)
            .filter(|u| old.index_of(*u).is_some()));
    let added = new.blocks.iter().any(|b| old.index_of(b.uid).is_none());
    if !order_same || added {
        let mut uids = [0u32; MAX_BLOCKS];
        for (o, b) in uids.iter_mut().zip(&new.blocks) {
            *o = b.uid;
        }
        cmds.push(BoardCmd::Order {
            uids,
            n: new.blocks.len().min(MAX_BLOCKS),
        });
    }
    if old.level_db != new.level_db {
        cmds.push(BoardCmd::Level(new.level_db));
    }
    Some(cmds)
}

struct Slot {
    uid: u32,
    pedal: Box<dyn Pedal>,
    v: [f32; MAX_KNOBS],
    on: bool,
    /// Being removed: fading out, then dropped.
    dying: bool,
    /// The fade between dry (0) and the block (1).
    g: f32,
    wet_only: bool,
    /// Samples the switched-off tail has been silent.
    quiet: usize,
}

/// Samples processed at a time (longer blocks are split).
const CHUNK: usize = 512;
const FADE_MS: f32 = 10.0;

/// A board running on the audio thread.
pub struct BoardProcessor {
    rate: f32,
    slots: Vec<Slot>,
    dry: Vec<f32>,
    wet: Vec<f32>,
    /// Pedals dropped while processing, for the trash.
    dropped: Vec<Box<dyn Pedal>>,
    level: f32,
    level_target: f32,
    /// A switched-off tail stops being processed after this many silent samples.
    tail_hold: usize,
}

impl BoardProcessor {
    pub fn new(board: &Board, rate: u32) -> BoardProcessor {
        let mut slots = Vec::with_capacity(MAX_BLOCKS * 2);
        for b in board.blocks.iter().take(MAX_BLOCKS) {
            let pedal = make_pedal(b, rate);
            slots.push(Slot {
                uid: b.uid,
                wet_only: pedal.wet_only(),
                pedal,
                v: b.v,
                on: b.on,
                dying: false,
                g: if b.on { 1.0 } else { 0.0 },
                quiet: usize::MAX / 2,
            });
        }
        let level = db_gain(board.level_db);
        BoardProcessor {
            rate: rate as f32,
            slots,
            dry: vec![0.0; CHUNK],
            wet: vec![0.0; CHUNK],
            dropped: Vec::with_capacity(MAX_BLOCKS * 4),
            level,
            level_target: level,
            tail_hold: (rate as usize) / 2,
        }
    }

    fn slot(&mut self, uid: u32) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|s| s.uid == uid && !s.dying)
    }

    fn process_chunk(&mut self, x: &mut [f32]) {
        let n = x.len();
        let step = 1.0 / (FADE_MS * 1e-3 * self.rate);
        let (dry, wet) = (&mut self.dry[..n], &mut self.wet[..n]);
        for s in self.slots.iter_mut() {
            let target = if s.on && !s.dying { 1.0 } else { 0.0 };
            if s.wet_only {
                if s.g == 0.0 && target == 0.0 && s.quiet >= self.tail_hold {
                    continue;
                }
                let mut g = s.g;
                for (w, &v) in wet.iter_mut().zip(x.iter()) {
                    g = approach(g, target, step);
                    *w = v * g;
                }
                s.g = g;
                s.pedal.process(wet);
                let mut peak = 0.0f32;
                for (v, &w) in x.iter_mut().zip(wet.iter()) {
                    *v += w;
                    peak = peak.max(w.abs());
                }
                if target == 0.0 && s.g == 0.0 && peak < 1e-5 {
                    s.quiet = s.quiet.saturating_add(n);
                } else {
                    s.quiet = 0;
                }
            } else {
                if s.g == 0.0 && target == 0.0 {
                    continue;
                }
                if s.g == 1.0 && target == 1.0 {
                    s.pedal.process(x);
                    continue;
                }
                dry.copy_from_slice(x);
                s.pedal.process(x);
                let mut g = s.g;
                for (v, &d) in x.iter_mut().zip(dry.iter()) {
                    g = approach(g, target, step);
                    *v = d + (*v - d) * g;
                }
                s.g = g;
            }
        }
        // drop the blocks that have faded out (and whose tails have died)
        let mut i = 0;
        while i < self.slots.len() {
            let s = &self.slots[i];
            if s.dying && s.g == 0.0 && (!s.wet_only || s.quiet >= self.tail_hold) {
                let s = self.slots.remove(i);
                if self.dropped.len() < self.dropped.capacity() {
                    self.dropped.push(s.pedal);
                } else {
                    std::mem::forget(s.pedal);
                }
            } else {
                i += 1;
            }
        }
        let (l0, l1) = (self.level, self.level_target);
        let dl = (l1 - l0) / n.max(1) as f32;
        for (i, v) in x.iter_mut().enumerate() {
            *v = dsp::soft_limit(*v * (l0 + dl * i as f32));
        }
        self.level = l1;
    }
}

#[inline]
fn approach(g: f32, target: f32, step: f32) -> f32 {
    if g < target {
        (g + step).min(target)
    } else if g > target {
        (g - step).max(target)
    } else {
        g
    }
}

fn db_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

impl crate::rt::ToneProcessor for BoardProcessor {
    fn process(&mut self, x: &mut [f32]) {
        for c in x.chunks_mut(CHUNK) {
            self.process_chunk(c);
        }
    }

    fn reset(&mut self) {
        for s in &mut self.slots {
            s.pedal.reset();
        }
    }

    fn board(&mut self, cmd: BoardCmd) -> Option<Box<dyn Pedal>> {
        match cmd {
            BoardCmd::Knob { uid, knob, value } => {
                if let Some(s) = self.slot(uid)
                    && knob < MAX_KNOBS
                {
                    s.v[knob] = value;
                    let v = s.v;
                    s.pedal.set(&v);
                }
                None
            }
            BoardCmd::Bypass { uid, on } => {
                if let Some(s) = self.slot(uid) {
                    s.on = on;
                    if on {
                        s.quiet = 0;
                    }
                }
                None
            }
            BoardCmd::Insert { block, pedal } => {
                if self.slots.len() >= self.slots.capacity() {
                    return Some(pedal);
                }
                self.slots.push(Slot {
                    uid: block.uid,
                    wet_only: pedal.wet_only(),
                    pedal,
                    v: block.v,
                    on: block.on,
                    dying: false,
                    g: 0.0,
                    quiet: 0,
                });
                None
            }
            BoardCmd::Replace { block, pedal } => match self.slot(block.uid) {
                Some(s) => {
                    s.v = block.v;
                    s.on = block.on;
                    s.wet_only = pedal.wet_only();
                    s.g = 0.0;
                    s.quiet = 0;
                    Some(std::mem::replace(&mut s.pedal, pedal))
                }
                None => Some(pedal),
            },
            BoardCmd::Remove { uid } => {
                if let Some(s) = self.slot(uid) {
                    s.dying = true;
                    s.uid = 0;
                }
                None
            }
            BoardCmd::Order { uids, n } => {
                // live slots in the given order, dying ones after them
                let mut at = 0;
                for &u in uids.iter().take(n.min(MAX_BLOCKS)) {
                    if let Some(j) =
                        (at..self.slots.len()).find(|&j| self.slots[j].uid == u && !self.slots[j].dying)
                    {
                        self.slots[at..=j].rotate_right(1);
                        at += 1;
                    }
                }
                None
            }
            BoardCmd::Level(db) => {
                self.level_target = db_gain(db.clamp(-24.0, 12.0));
                None
            }
        }
    }

    fn trash(&mut self) -> Option<Box<dyn Pedal>> {
        self.dropped.pop()
    }
}

/// The processor for a board (allocates: off the audio thread).
pub fn make_board(board: &Board, rate: u32) -> Box<dyn crate::rt::ToneProcessor> {
    Box::new(BoardProcessor::new(board, rate))
}

/// Render `x` (mono) through a board offline, in blocks of 128 like the audio thread.
pub fn render(board: &Board, rate: u32, x: &[f32]) -> Vec<f32> {
    use crate::rt::ToneProcessor;
    let mut p = BoardProcessor::new(board, rate);
    let mut y = x.to_vec();
    for c in y.chunks_mut(128) {
        p.process(c);
    }
    y
}

/// How much louder a board makes an idle input's hiss (dB, from -90 dBFS of white noise). Crunch is 19 to 24 (a JCM800
/// at gain 3.5, our Crunch and Blues), as a real amp hisses; over about 28 the hiss between notes is loud and the
/// board wants a gate (a JCM800 from gain 6, the Mesa-style amp 57, a Tube Screamer into the lead amp 50, fuzz 47).
/// Renders 0.4 s, off the audio thread.
pub fn hiss_gain_db(board: &Board) -> f32 {
    const RATE: u32 = 48000;
    const RMS: f32 = 3.162e-5; // -90 dBFS
    let n = RATE as usize * 2 / 5;
    let mut seed = 0x2545_f491u32;
    let x: Vec<f32> = (0..n)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            // uniform on [-a, a] has an RMS of a / sqrt(3)
            (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * RMS * 3f32.sqrt()
        })
        .collect();
    let y = render(board, RATE, &x);
    let tail = &y[n / 4..];
    let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
    20.0 * (rms.max(1e-9) / RMS).log10()
}

// -- starting boards ------------------------------------------------------------------------------------------------

/// The boards the tile offers before you've saved any of your own: (name, board).
pub fn presets() -> Vec<(&'static str, Board)> {
    let b = |id: &str| Block::of(id).expect("catalogue model");
    let mut boards = vec![
        (
            "Clean",
            Board::new(vec![
                b("squeeze").with("sustain", 3.5).with("level", 5.5),
                b("amp_clean").with("gain", 3.5).with("treble", 6.5),
                b("cab_2x12").with("mic", 5.0),
                b("spring").with("dwell", 5.0).with("mix", 22.0),
            ]),
        ),
        (
            "Crunch",
            Board::new(vec![
                b("amp_crunch").with("gain", 6.0).with("mid", 6.5),
                b("cab_4x12").with("mic", 4.0),
                b("room").with("mix", 14.0),
            ]),
        ),
        (
            "Lead",
            Board::new(vec![
                b("gate").with("threshold", -54.0).with("release", 120.0),
                b("green").with("drive", 3.0).with("level", 7.0),
                b("amp_lead").with("gain", 7.0),
                b("cab_4x12"),
                b("analog_delay").with("time", 380.0).with("mix", 22.0),
                b("plate").with("mix", 16.0),
            ]),
        ),
        (
            "Metal",
            Board::new(vec![
                b("gate").with("threshold", -50.0),
                b("green").with("drive", 0.5).with("tone", 6.0).with("level", 8.0),
                b("amp_modern").with("gain", 7.5),
                b("cab_4x12").with("mic", 3.5),
            ]),
        ),
        (
            "Blues",
            Board::new(vec![
                b("blues").with("gain", 4.0),
                b("amp_tweed").with("volume", 6.0),
                b("cab_1x12"),
                b("spring").with("mix", 20.0),
            ]),
        ),
        (
            "Ambient",
            Board::new(vec![
                b("squeeze").with("sustain", 4.0),
                b("amp_clean").with("gain", 3.0),
                b("cab_2x12"),
                b("mod_delay")
                    .with("time", 480.0)
                    .with("repeats", 5.5)
                    .with("mix", 35.0),
                b("hall").with("time", 5.0).with("mix", 35.0),
            ]),
        ),
        (
            "Fuzz",
            Board::new(vec![
                b("gate").with("threshold", -54.0).with("release", 120.0),
                b("big_pi"),
                b("amp_clean").with("gain", 5.0),
                b("cab_4x12"),
            ]),
        ),
        (
            "Acoustic",
            Board::new(vec![
                b("acoustic"),
                b("studio_comp").with("threshold", -22.0).with("ratio", 3.0),
                b("room").with("mix", 18.0),
            ]),
        ),
        (
            "Bass",
            Board::new(vec![b("bass_comp"), b("amp_bass"), b("cab_8x10")]),
        ),
    ];
    // output levels matched to Clean by loudness (BS.1770, K-weighted): the owner's guitar through the Scarlett,
    // two takes rendered through every board (2026-10-15); Crunch had come out 3.6 LU under Clean, Blues 1.5 over
    for (name, board) in &mut boards {
        board.level_db = match *name {
            "Crunch" | "Acoustic" => 3.5,
            "Ambient" => 3.0,
            "Lead" => 2.0,
            "Bass" => 1.5,
            "Blues" => -1.5,
            "Metal" | "Fuzz" => -1.0,
            _ => 0.0,
        };
    }
    boards
}
