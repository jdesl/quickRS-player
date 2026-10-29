//! The jam on the output audio thread (P22.4): the metronome, the drum loop, the backing loop, the looper and
//! the Rewind writer, all timed by frames of the output: a click or a drum hit lands on the exact sample its beat
//! falls on, and a loop closes on a bar line.
//!
//! - The transport: beats are a straight line through the output's frames (`anchor_t`, `anchor_beat`, the bpm); a
//!   tempo change re-anchors where it's applied. Beat `b` sounds at frame `ceil(anchor_t + (b - anchor_beat) *
//!   frames per beat)`. Drums and the click run on a grid of 12 ticks a beat (sixteenths and triplets).
//! - Drum hits and clicks are one-shot voices (a fixed pool) reading the kit's clips; a closed hat chokes an open
//!   one. The backing clip plays locked to the beat (re-synced if it drifts or the tempo jumps); a new clip fades
//!   the old one out first.
//! - The looper keeps each layer (the guitar dry and through the rig, 16-bit) in memory made off the audio thread.
//!   Recording is a window of heard time; what the guitar played at heard time x arrives here `delay` frames later
//!   (its round trip), so it's written where x falls in the loop: played in time with the click, it plays back in
//!   time. A layer remembers which part of the loop it has covered, so nothing is cleared on the audio thread.
//!   With re-amp on, the dry layers play through a second copy of the rig.
//!
//! No allocation, locks or I/O: everything new comes in through `JamCmd`, everything let go goes to the trash.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use super::rewind::{PeakAcc, RewindRing};
use super::{ToneProcessor, Trash, discard};
use crate::clock::{AtomicF32, ClockCell, SongClock};
use crate::jam::drums::{Inst, TICKS_PER_BEAT};
use crate::jam::{BackingClip, ClickParams, DrumKit, DrumParams, LoopAction, LoopState, LooperSettings};

/// Layers a looper can hold at most.
pub const MAX_LAYERS: usize = 16;
const MAX_VOICES: usize = 32;
/// The kit's level under its volume knob: a full groove (kick, snare, hats and a crash at once) peaks near -6 dBFS
/// at the default volume, under the guitar.
pub const KIT_LEVEL: f32 = 0.5;
/// Loop samples are stored as i16 of x * LOOP_SCALE (6 dB of headroom over full scale).
pub const LOOP_SCALE: f32 = 16384.0;

/// What the jam publishes for the page.
pub struct JamShared {
    /// Beats as a clock: `song_time` is the beat heard at `wall_ns`, `rate` beats a second while running.
    pub clock: ClockCell,
    pub beats_per_bar: AtomicU32,
    pub bpm: AtomicU64,
    pub loop_state: AtomicU32,
    pub loop_pos: AtomicU64,
    pub loop_len: AtomicU64,
    pub layers: AtomicU32,
    pub redo: AtomicU32,
    pub intensity: AtomicF32,
    pub loop_delay: AtomicU64,
    pub rate: AtomicU32,
}

impl Default for JamShared {
    fn default() -> Self {
        JamShared {
            clock: ClockCell::default(),
            beats_per_bar: AtomicU32::new(4),
            bpm: AtomicU64::new(120f64.to_bits()),
            loop_state: AtomicU32::new(0),
            loop_pos: AtomicU64::new(0),
            loop_len: AtomicU64::new(0),
            layers: AtomicU32::new(0),
            redo: AtomicU32::new(0),
            intensity: AtomicF32::default(),
            loop_delay: AtomicU64::new(0),
            rate: AtomicU32::new(48000),
        }
    }
}

/// The looper's memory: `layers` layers of `frames` frames, each frame (wet, dry) as i16.
pub struct LoopStore {
    pub frames: usize,
    pub layers: usize,
    data: Vec<i16>,
}

impl LoopStore {
    /// Built off the audio thread; the zeros are written (not just mapped) so the audio thread never takes a
    /// page fault on first use.
    #[allow(clippy::slow_vector_initialization)] // written zeros on purpose (vec![0; n] may only map them)
    pub fn new(rate: u32, s: LooperSettings) -> LoopStore {
        let frames = ((s.max_seconds.clamp(1.0, 600.0)) * rate as f64) as usize;
        let layers = s.max_layers.clamp(1, MAX_LAYERS);
        let n = frames * layers * 2;
        let mut data = Vec::with_capacity(n);
        data.resize(n, 0i16);
        LoopStore { frames, layers, data }
    }
}

/// Commands for the jam (inside `OutCmd::Jam`).
pub enum JamCmd {
    Tempo(f64),
    Meter(u32),
    /// Start the click / drums / backing from bar 0, after `count_in` bars of click.
    Start {
        count_in: u32,
    },
    Stop,
    Click(ClickParams),
    Drums(DrumParams),
    Kit(Arc<DrumKit>),
    /// A new backing loop (None: none), faded over.
    Backing(Option<Arc<BackingClip>>),
    BackingGain(f32),
    Loop(LoopAction),
    /// The looper's memory (None: no looper).
    LoopStore(Option<Box<LoopStore>>),
    LoopGain(f32),
    /// Play the loop's dry layers through the re-amp rig instead of the layers as recorded.
    Reamp(bool),
    ReampTone(Option<Box<dyn ToneProcessor>>),
    ReampBoard(crate::board::BoardCmd),
    ReampParams(crate::tone::ToneParams),
    Rewind(Option<Arc<RewindRing>>),
}

#[derive(Clone, Copy)]
struct Transport {
    running: bool,
    bpm: f64,
    bpb: u32,
    anchor_t: u64,
    anchor_beat: f64,
    rate: f64,
}

impl Transport {
    /// Frames per beat.
    fn spb(&self) -> f64 {
        60.0 * self.rate / self.bpm
    }
    /// The beat at frame `t` (stopped: where it stopped).
    fn beat_at(&self, t: u64) -> f64 {
        if !self.running {
            return self.anchor_beat;
        }
        self.anchor_beat + (t as f64 - self.anchor_t as f64) / self.spb()
    }
    /// The first frame at or after beat `b`.
    fn frame_of(&self, b: f64) -> i64 {
        self.anchor_t as i64 + ((b - self.anchor_beat) * self.spb() - 1e-6).ceil() as i64
    }
    fn reanchor(&mut self, t: u64) {
        self.anchor_beat = self.beat_at(t);
        self.anchor_t = t;
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    active: bool,
    inst: u8,
    /// Samples into the sound.
    pos: usize,
    /// Frame in the current block it starts at (0 once it's playing).
    start: usize,
    gl: f32,
    gr: f32,
    /// Choked: fades from `fade_from` to silence at `end` (sound sample indexes).
    fade_from: usize,
    end: usize,
}

/// A recording: what was heard in [from, to) (timeline frames), arriving `delay` frames later.
#[derive(Clone, Copy)]
struct Window {
    layer: usize,
    from: u64,
    to: u64,
    delay: u64,
}

/// The part of the loop a layer has recorded: `n` positions from `start`, going round.
#[derive(Clone, Copy, Default)]
struct Cover {
    start: usize,
    n: usize,
}

impl Cover {
    #[inline]
    fn has(&self, p: usize, len: usize) -> bool {
        self.n >= len || (p + len - self.start) % len < self.n
    }
}

struct Looper {
    store: Option<Box<LoopStore>>,
    /// Empty, Recording (also armed and closing), Playing, Overdubbing, Stopped; Off without a store.
    state: LoopState,
    /// Timeline frame of the loop's start, and its beat (for quantising).
    origin: u64,
    origin_beat: f64,
    synced: bool,
    len: usize,
    /// The first take is still open (its length isn't known).
    open: bool,
    /// Nothing plays before this frame (the first take closes, a stopped loop starts again).
    play_from: u64,
    layers: usize,
    redo: usize,
    cover: [Cover; MAX_LAYERS],
    rec: Option<Window>,
    /// A recording punched out, still taking what's on its way (its round trip).
    tail: Option<Window>,
    gain: f32,
}

impl Looper {
    fn new() -> Looper {
        Looper {
            store: None,
            state: LoopState::Off,
            origin: 0,
            origin_beat: 0.0,
            synced: false,
            len: 0,
            open: false,
            play_from: 0,
            layers: 0,
            redo: 0,
            cover: [Cover::default(); MAX_LAYERS],
            rec: None,
            tail: None,
            gain: 1.0,
        }
    }

    fn clear(&mut self) {
        self.state = if self.store.is_some() {
            LoopState::Empty
        } else {
            LoopState::Off
        };
        self.len = 0;
        self.open = false;
        self.layers = 0;
        self.redo = 0;
        self.rec = None;
        self.tail = None;
    }

    fn punch_out(&mut self, at: u64) {
        if let Some(mut w) = self.rec.take() {
            w.to = at.max(w.from);
            self.tail = Some(w);
        }
    }
}

pub struct Jam {
    rate: u32,
    /// Output frames since the jam started (its timeline).
    t: u64,
    tr: Transport,
    click: ClickParams,
    drums: DrumParams,
    kit: Option<Arc<DrumKit>>,
    voices: [Voice; MAX_VOICES],
    /// The guitar's level, and the band's intensity following it.
    env: f32,
    intensity: f32,
    backing: Option<Arc<BackingClip>>,
    /// A clip waiting for the playing one to fade out (`Some(None)`: fading to none).
    backing_next: Option<Option<Arc<BackingClip>>>,
    backing_pos: usize,
    backing_env: f32,
    backing_gain: f32,
    looper: Looper,
    /// The guitar's round trip (heard to arriving here), frames: set by the output side each block.
    pub delay: u64,
    reamp: bool,
    reamp_tone: Option<Box<dyn ToneProcessor>>,
    rewind: Option<Arc<RewindRing>>,
    rw_acc: PeakAcc,
    loop_wet: Vec<f32>,
    loop_dry: Vec<f32>,
}

#[inline]
fn q16(x: f32) -> i16 {
    (x * LOOP_SCALE).clamp(-32767.0, 32767.0) as i16
}

impl Jam {
    pub fn new(rate: u32, max_block: usize) -> Jam {
        Jam {
            rate,
            t: 0,
            tr: Transport {
                running: false,
                bpm: 120.0,
                bpb: 4,
                anchor_t: 0,
                anchor_beat: 0.0,
                rate: rate as f64,
            },
            click: ClickParams::default(),
            drums: DrumParams::default(),
            kit: None,
            voices: [Voice::default(); MAX_VOICES],
            env: 0.0,
            intensity: 0.0,
            backing: None,
            backing_next: None,
            backing_pos: 0,
            backing_env: 0.0,
            backing_gain: 0.6,
            looper: Looper::new(),
            delay: 0,
            reamp: false,
            reamp_tone: None,
            rewind: None,
            rw_acc: PeakAcc::default(),
            loop_wet: vec![0.0; max_block],
            loop_dry: vec![0.0; max_block],
        }
    }

    /// Frames since the jam started.
    pub fn timeline(&self) -> u64 {
        self.t
    }

    /// The click or the drums are keeping time (the looper locks to their bars).
    fn synced(&self) -> bool {
        self.tr.running && (self.click.on || self.drums.on)
    }

    pub fn command(&mut self, cmd: JamCmd, trash: &mut rtrb::Producer<Trash>) {
        let t0 = self.t;
        match cmd {
            JamCmd::Tempo(bpm) => {
                self.tr.reanchor(t0);
                self.tr.bpm = bpm.clamp(20.0, 400.0);
            }
            JamCmd::Meter(b) => {
                self.tr.reanchor(t0);
                self.tr.bpb = b.clamp(1, 16);
            }
            JamCmd::Start { count_in } => {
                self.tr.running = true;
                self.tr.anchor_t = t0;
                self.tr.anchor_beat = -((count_in.min(8) * self.tr.bpb) as f64);
            }
            JamCmd::Stop => {
                self.tr.reanchor(t0);
                self.tr.running = false;
            }
            JamCmd::Click(p) => self.click = p,
            JamCmd::Drums(p) => self.drums = p,
            JamCmd::Kit(k) => {
                if let Some(old) = self.kit.replace(k) {
                    discard(trash, Trash::Kit(old));
                }
            }
            JamCmd::Backing(b) => {
                if self.backing.is_none() {
                    self.backing = b;
                    self.backing_env = 0.0;
                } else if let Some(Some(old)) = self.backing_next.replace(b) {
                    discard(trash, Trash::Backing(old));
                }
            }
            JamCmd::BackingGain(g) => self.backing_gain = g,
            JamCmd::Loop(a) => self.loop_action(a, t0),
            JamCmd::LoopStore(s) => {
                if let Some(old) = std::mem::replace(&mut self.looper.store, s) {
                    discard(trash, Trash::LoopStore(old));
                }
                self.looper.clear();
            }
            JamCmd::LoopGain(g) => self.looper.gain = g,
            JamCmd::Reamp(on) => self.reamp = on,
            JamCmd::ReampTone(t) => {
                if let Some(old) = std::mem::replace(&mut self.reamp_tone, t) {
                    discard(trash, Trash::Tone(old));
                }
            }
            JamCmd::ReampBoard(c) => {
                let spare = match self.reamp_tone.as_mut() {
                    Some(t) => t.board(c),
                    None => c.into_pedal(),
                };
                if let Some(p) = spare {
                    discard(trash, Trash::Pedal(p));
                }
            }
            JamCmd::ReampParams(p) => {
                if let Some(t) = self.reamp_tone.as_mut() {
                    t.set_params(&p);
                }
            }
            JamCmd::Rewind(r) => {
                if let Some(old) = std::mem::replace(&mut self.rewind, r) {
                    discard(trash, Trash::Rewind(old));
                }
                self.rw_acc = PeakAcc::default();
            }
        }
    }

    // -- the looper's buttons --------------------------------------------------------------------------------

    /// Where a loop starts when pressed at `t`: now, or (locked to the click) the bar line just gone if it was
    /// less than a quarter beat ago, else the next one. Returns (frame, beat).
    fn sync_point(&self, t: u64) -> (u64, f64) {
        if !self.synced() {
            return (t, 0.0);
        }
        let tr = &self.tr;
        let bpb = tr.bpb as f64;
        let bar = (tr.beat_at(t) / bpb).floor();
        let last = tr.frame_of(bar * bpb);
        let grace = (tr.spb() * 0.25) as i64;
        if t as i64 - last <= grace && last >= 0 {
            (last as u64, bar * bpb)
        } else {
            (
                tr.frame_of((bar + 1.0) * bpb).max(t as i64) as u64,
                (bar + 1.0) * bpb,
            )
        }
    }

    fn start_take(&mut self, t: u64) {
        let (s, beat) = self.sync_point(t);
        let synced = self.synced();
        let delay = self.delay;
        let l = &mut self.looper;
        l.clear();
        l.origin = s;
        l.origin_beat = beat;
        l.synced = synced;
        l.open = true;
        l.layers = 1;
        l.cover[0] = Cover::default();
        l.rec = Some(Window {
            layer: 0,
            from: s,
            to: u64::MAX,
            delay,
        });
        l.state = LoopState::Recording;
    }

    /// Close the first take pressed at `q`: its length to the press, or to the nearest whole bar when it was
    /// started on the click's bar line.
    fn close_take(&mut self, q: u64) {
        let tr = self.tr;
        let l = &mut self.looper;
        let Some(max) = l.store.as_ref().map(|s| s.frames as u64) else {
            return;
        };
        let mut e = q;
        if l.synced && tr.running {
            let bar = tr.spb() * tr.bpb as f64;
            let mut bars = ((q.saturating_sub(l.origin)) as f64 / bar).round().max(1.0);
            let end = |bars: f64| tr.frame_of(l.origin_beat + bars * tr.bpb as f64).max(0) as u64;
            while bars > 1.0 && end(bars).saturating_sub(l.origin) > max {
                bars -= 1.0;
            }
            e = end(bars);
        }
        let e = e.clamp(l.origin + 1, l.origin + max);
        l.len = (e - l.origin) as usize;
        l.open = false;
        l.play_from = e;
        let c = &mut l.cover[0];
        c.n = c.n.min(l.len.saturating_sub(c.start));
        if c.start >= l.len {
            *c = Cover::default();
        }
        l.punch_out(e);
        l.state = LoopState::Playing;
    }

    fn overdub(&mut self, t: u64) {
        let delay = self.delay;
        let l = &mut self.looper;
        let Some(cap) = l.store.as_ref().map(|s| s.layers) else {
            return;
        };
        if l.layers >= cap {
            return; // full: undo a layer first
        }
        let layer = l.layers;
        l.layers += 1;
        l.redo = 0;
        l.cover[layer] = Cover::default();
        l.rec = Some(Window {
            layer,
            from: t.max(l.play_from),
            to: u64::MAX,
            delay,
        });
        l.state = LoopState::Overdubbing;
    }

    fn play(&mut self, t: u64) {
        let l = &self.looper;
        match l.state {
            LoopState::Recording => self.close_take(t),
            LoopState::Overdubbing => {
                self.looper.punch_out(t);
                self.looper.state = LoopState::Playing;
            }
            LoopState::Stopped => {
                // from the top: now, or on the click's bar line
                let (s, _) = self.sync_point(t);
                let l = &mut self.looper;
                l.origin = s;
                l.play_from = s;
                l.state = LoopState::Playing;
            }
            _ => {}
        }
    }

    fn loop_action(&mut self, a: LoopAction, t: u64) {
        let state = self.looper.state;
        if state == LoopState::Off {
            return;
        }
        let armed = self.looper.open && t < self.looper.origin;
        match a {
            LoopAction::Press => match state {
                LoopState::Empty => self.start_take(t),
                LoopState::Recording if armed => self.looper.clear(),
                LoopState::Recording => self.close_take(t),
                LoopState::Playing => self.overdub(t),
                LoopState::Overdubbing | LoopState::Stopped => self.play(t),
                _ => {}
            },
            LoopAction::Record => match state {
                LoopState::Empty => self.start_take(t),
                LoopState::Playing => self.overdub(t),
                _ => {}
            },
            LoopAction::Overdub => match state {
                LoopState::Recording if !armed => {
                    self.close_take(t);
                    let from = self.looper.play_from;
                    self.overdub(from);
                }
                LoopState::Playing => self.overdub(t),
                LoopState::Overdubbing => self.play(t),
                _ => {}
            },
            LoopAction::Play => self.play(t),
            LoopAction::Stop => {
                match state {
                    LoopState::Recording if armed => {
                        self.looper.clear();
                        return;
                    }
                    LoopState::Recording => self.close_take(t),
                    LoopState::Overdubbing => self.looper.punch_out(t),
                    _ => {}
                }
                if self.looper.state != LoopState::Empty {
                    self.looper.state = LoopState::Stopped;
                }
            }
            LoopAction::Undo => match state {
                LoopState::Recording => self.looper.clear(),
                LoopState::Overdubbing => {
                    let l = &mut self.looper;
                    l.rec = None;
                    l.layers -= 1;
                    l.redo += 1;
                    l.state = LoopState::Playing;
                }
                LoopState::Playing | LoopState::Stopped if self.looper.layers > 1 => {
                    let l = &mut self.looper;
                    if l.tail.is_some_and(|w| w.layer == l.layers - 1) {
                        l.tail = None;
                    }
                    l.layers -= 1;
                    l.redo += 1;
                }
                _ => {}
            },
            LoopAction::Redo => {
                let l = &mut self.looper;
                if matches!(state, LoopState::Playing | LoopState::Stopped) && l.redo > 0 {
                    l.layers += 1;
                    l.redo -= 1;
                }
            }
            LoopAction::Clear => self.looper.clear(),
        }
    }

    // -- the block -------------------------------------------------------------------------------------------

    fn trigger(&mut self, inst: Inst, gain: f32, at: usize) {
        if gain <= 0.0 {
            return;
        }
        let Some(kit) = self.kit.as_ref() else { return };
        let len = kit.sound(inst).len();
        if inst == Inst::HatClosed {
            // the closed hat chokes an open one
            for v in self.voices.iter_mut() {
                if v.active && v.inst == Inst::HatOpen as u8 && at >= v.start {
                    let idx = v.pos + (at - v.start);
                    v.fade_from = v.fade_from.min(idx);
                    v.end = v.end.min(idx + 96);
                }
            }
        }
        let slot = match self.voices.iter().position(|v| !v.active) {
            Some(i) => i,
            None => {
                // steal the voice furthest into its sound
                let mut best = 0;
                for (i, v) in self.voices.iter().enumerate() {
                    if v.pos > self.voices[best].pos {
                        best = i;
                    }
                }
                best
            }
        };
        let pan = inst.pan();
        self.voices[slot] = Voice {
            active: true,
            inst: inst as u8,
            pos: 0,
            start: at,
            gl: gain * (1.0 - pan).min(1.0),
            gr: gain * (1.0 + pan).min(1.0),
            fade_from: len,
            end: len,
        };
    }

    /// Clicks and drum hits on the ticks that fall in this block.
    fn schedule(&mut self, t0: u64, frames: usize) {
        let tr = self.tr;
        let end = (t0 + frames as u64) as i64;
        let tpb = TICKS_PER_BEAT as i64;
        let mut g = (tr.beat_at(t0) * tpb as f64).floor() as i64 - 1;
        loop {
            let f = tr.frame_of(g as f64 / tpb as f64);
            if f >= end {
                break;
            }
            if f >= t0 as i64 {
                self.tick(g, (f - t0 as i64) as usize);
            }
            g += 1;
        }
    }

    fn tick(&mut self, g: i64, at: usize) {
        let tpb = TICKS_PER_BEAT as i64;
        let bpb = self.tr.bpb as i64;
        let per_bar = tpb * bpb;
        let bar = g.div_euclid(per_bar);
        let in_bar = g.rem_euclid(per_bar);
        let (beat, sub) = (in_bar / tpb, in_bar % tpb);
        if sub == 0 && (self.click.on || bar < 0) {
            let accent = beat == 0 && (self.click.accent || bar < 0);
            let (inst, g) = if accent {
                (Inst::ClickAccent, 1.0)
            } else {
                (Inst::Click, 0.7)
            };
            self.trigger(inst, g * self.click.volume, at);
        }
        if !self.drums.on || bar < 0 {
            return;
        }
        let Some(kit) = self.kit.clone() else { return };
        let d = self.drums;
        let (groove, fill) = &kit.patterns[d.style.index().min(kit.patterns.len() - 1)];
        let every = d.fill_every as i64;
        let fill_bar = every > 0 && (bar + 1) % every == 0;
        let (pat, pb) = if fill_bar && beat + 2 >= bpb {
            (fill, 4 - (bpb - beat))
        } else {
            (groove, beat % 4)
        };
        let follow = if d.follow {
            0.35 + 0.65 * self.intensity
        } else {
            1.0
        };
        let soft = d.follow && self.intensity < 0.5;
        let groups = [d.kick, d.snare, d.hats];
        let ptick = (pb.clamp(0, 3) * tpb + sub) as u32;
        for h in pat.at(ptick) {
            if h.ghost && soft {
                continue;
            }
            let gain = h.vel * d.volume * KIT_LEVEL * groups[h.inst.group().min(2)] * follow;
            self.trigger(h.inst, gain, at);
        }
        if every > 0 && bar > 0 && bar % every == 0 && in_bar == 0 {
            self.trigger(Inst::Crash, 0.8 * d.volume * KIT_LEVEL * d.hats * follow, at);
        }
    }

    fn render_voices(&mut self, mix: &mut [f32], frames: usize) {
        let Some(kit) = self.kit.as_ref() else {
            for v in self.voices.iter_mut() {
                v.active = false;
            }
            return;
        };
        for v in self.voices.iter_mut().filter(|v| v.active) {
            let s = &kit.sounds[(v.inst as usize).min(kit.sounds.len() - 1)];
            let end = v.end.min(s.len());
            let mut i = v.start;
            let mut idx = v.pos;
            while i < frames && idx < end {
                let mut x = s[idx];
                if idx >= v.fade_from {
                    x *= (end - idx) as f32 / (end - v.fade_from).max(1) as f32;
                }
                mix[2 * i] += x * v.gl;
                mix[2 * i + 1] += x * v.gr;
                i += 1;
                idx += 1;
            }
            v.pos = idx;
            v.start = 0;
            if idx >= end {
                v.active = false;
            }
        }
    }

    fn follow_guitar(&mut self, dry: &[f32]) {
        let n = dry.len();
        if n == 0 {
            return;
        }
        let rms = (dry.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
        let tau = if rms > self.env { 0.25 } else { 2.0 };
        let a = (-(n as f32) / (self.rate as f32 * tau)).exp();
        self.env = self.env * a + rms * (1.0 - a);
        let db = 20.0 * self.env.max(1e-6).log10();
        self.intensity = ((db + 48.0) / 30.0).clamp(0.0, 1.0);
    }

    fn render_backing(&mut self, mix: &mut [f32], t0: u64, frames: usize, trash: &mut rtrb::Producer<Trash>) {
        let fade_step = 1.0 / (self.rate as f32 * 0.05);
        let tr = self.tr;
        // a waiting clip: fade the playing one out first
        if self.backing_next.is_some() && self.backing_env <= 0.0 {
            let next = self.backing_next.take().flatten();
            if let Some(old) = std::mem::replace(&mut self.backing, next) {
                discard(trash, Trash::Backing(old));
            }
        }
        let Some(clip) = self.backing.as_ref() else { return };
        let fits = (clip.bpm - tr.bpm).abs() < 1e-6 && clip.beats_per_bar == tr.bpb;
        let target = if tr.running && fits && self.backing_next.is_none() {
            1.0
        } else {
            0.0
        };
        if target == 0.0 && self.backing_env <= 0.0 {
            return;
        }
        let n = clip.frames();
        if n == 0 {
            return;
        }
        // where the beat says the clip should be (it starts on bar 0's downbeat)
        let b0 = tr.beat_at(t0);
        let mut k0 = 0usize;
        if b0 < 0.0 {
            let f = tr.frame_of(0.0) - t0 as i64;
            if f >= frames as i64 {
                return;
            }
            k0 = f.max(0) as usize;
        }
        if fits && tr.running {
            let beats = clip.beats.max(1) as f64;
            let b = tr.beat_at(t0 + k0 as u64).max(0.0);
            let want = ((b % beats) / beats * n as f64) as usize % n;
            let off = (want as i64 - self.backing_pos as i64).rem_euclid(n as i64);
            if off > 4 && off < n as i64 - 4 {
                self.backing_pos = want;
            }
        }
        let g = self.backing_gain;
        let mut pos = self.backing_pos % n;
        let mut env = self.backing_env;
        for i in k0..frames {
            if env < target {
                env = (env + fade_step).min(target);
            } else if env > target {
                env = (env - fade_step).max(target);
            }
            mix[2 * i] += clip.samples[2 * pos] * g * env;
            mix[2 * i + 1] += clip.samples[2 * pos + 1] * g * env;
            pos += 1;
            if pos >= n {
                pos = 0;
            }
        }
        self.backing_pos = pos;
        self.backing_env = env;
    }

    /// Write the guitar into the recording windows, and play the loop into `loop_wet` / `loop_dry`.
    fn run_looper(&mut self, wet: &[f32], dry: &[f32], t0: u64, frames: usize) -> bool {
        // the first take reaching the longest loop closes itself
        let l = &self.looper;
        let full = match l.store.as_ref() {
            Some(s) if l.state == LoopState::Recording && l.open => {
                let q = l.origin + s.frames as u64;
                (t0 >= q).then_some(q)
            }
            _ => None,
        };
        if let Some(q) = full {
            self.close_take(q);
        }
        let l = &mut self.looper;
        let Some(store) = l.store.as_mut() else {
            return false;
        };
        let max = store.frames;
        for w in [&mut l.rec, &mut l.tail] {
            let Some(win) = w.as_mut() else { continue };
            let lay = win.layer;
            let c = &mut l.cover[lay];
            for i in 0..frames {
                let t = t0 + i as u64;
                if t < win.delay {
                    continue;
                }
                let x = t - win.delay;
                if x < win.from || x >= win.to || x < l.origin {
                    continue;
                }
                let (p, len) = if l.open {
                    let p = (x - l.origin) as usize;
                    if p >= max {
                        continue;
                    }
                    (p, max)
                } else {
                    (((x - l.origin) % l.len as u64) as usize, l.len)
                };
                let k = (lay * max + p) * 2;
                let (qw, qd) = (q16(wet[i]), q16(dry[i]));
                if c.n == 0 {
                    c.start = p;
                    c.n = 1;
                    store.data[k] = qw;
                    store.data[k + 1] = qd;
                } else if c.n < len && p == (c.start + c.n) % len {
                    c.n += 1;
                    store.data[k] = qw;
                    store.data[k + 1] = qd;
                } else if c.has(p, len) {
                    // round again (sound on sound)
                    store.data[k] = store.data[k].saturating_add(qw);
                    store.data[k + 1] = store.data[k + 1].saturating_add(qd);
                } else {
                    store.data[k] = qw;
                    store.data[k + 1] = qd;
                }
            }
            if (t0 + frames as u64).saturating_sub(win.delay) >= win.to {
                *w = None;
            }
        }
        if !matches!(l.state, LoopState::Playing | LoopState::Overdubbing) || l.len == 0 {
            return false;
        }
        let len = l.len;
        let mut any = false;
        for i in 0..frames {
            let t = t0 + i as u64;
            let (mut sw, mut sd) = (0i32, 0i32);
            if t >= l.play_from && t >= l.origin {
                let p = ((t - l.origin) % len as u64) as usize;
                for lay in 0..l.layers {
                    if l.cover[lay].has(p, len) {
                        let k = (lay * max + p) * 2;
                        sw += store.data[k] as i32;
                        sd += store.data[k + 1] as i32;
                    }
                }
                any = true;
            }
            self.loop_wet[i] = sw as f32 / LOOP_SCALE;
            self.loop_dry[i] = sd as f32 / LOOP_SCALE;
        }
        any
    }

    /// One block: `mix` is stereo (2 x frames), `wet`/`dry` the guitar this block (through the rig, as it came
    /// in), `wall_ns` when the block's first frame is heard.
    pub fn process(
        &mut self,
        mix: &mut [f32],
        wet: &[f32],
        dry: &[f32],
        frames: usize,
        wall_ns: u64,
        sh: &JamShared,
        trash: &mut rtrb::Producer<Trash>,
    ) {
        let frames = frames.min(self.loop_wet.len()).min(wet.len()).min(dry.len());
        let t0 = self.t;
        let (wet, dry) = (&wet[..frames], &dry[..frames]);
        if let Some(r) = self.rewind.as_ref() {
            r.write(dry, wet, &mut self.rw_acc);
        }
        if self.drums.follow {
            self.follow_guitar(dry);
        }
        if self.tr.running {
            self.schedule(t0, frames);
        }
        self.render_voices(mix, frames);
        self.render_backing(mix, t0, frames, trash);
        if self.looper.state != LoopState::Off && self.run_looper(wet, dry, t0, frames) {
            let src = if self.reamp
                && let Some(tone) = self.reamp_tone.as_mut()
            {
                tone.process(&mut self.loop_dry[..frames]);
                while let Some(p) = tone.trash() {
                    discard(trash, Trash::Pedal(p));
                }
                &self.loop_dry
            } else {
                &self.loop_wet
            };
            let g = self.looper.gain;
            for i in 0..frames {
                let v = src[i] * g;
                mix[2 * i] += v;
                mix[2 * i + 1] += v;
            }
        }
        self.t += frames as u64;
        self.publish(t0, frames, wall_ns, sh);
    }

    fn publish(&self, t0: u64, frames: usize, wall_ns: u64, sh: &JamShared) {
        let tr = &self.tr;
        let running = tr.running;
        sh.clock.store(&SongClock {
            wall_ns,
            song_time: tr.beat_at(t0),
            rate: if running { tr.bpm / 60.0 } else { 0.0 },
            limit: tr.beat_at(t0 + frames as u64),
            playing: running,
        });
        sh.beats_per_bar.store(tr.bpb, Ordering::Relaxed);
        sh.bpm.store(tr.bpm.to_bits(), Ordering::Relaxed);
        sh.rate.store(self.rate, Ordering::Relaxed);
        let l = &self.looper;
        let state = match l.state {
            LoopState::Recording if l.open && t0 < l.origin => LoopState::Armed,
            // the first take still running to the bar line it closes on
            LoopState::Playing | LoopState::Overdubbing
                if t0 < l.play_from && l.tail.is_some_and(|w| w.layer == 0 && w.to == l.play_from) =>
            {
                LoopState::Recording
            }
            s => s,
        };
        let pos = match state {
            LoopState::Recording => t0.saturating_sub(l.origin),
            LoopState::Playing | LoopState::Overdubbing if l.len > 0 => {
                t0.saturating_sub(l.origin) % l.len as u64
            }
            _ => 0,
        };
        sh.loop_state.store(state.as_u32(), Ordering::Relaxed);
        sh.loop_pos.store(pos, Ordering::Relaxed);
        sh.loop_len.store(l.len as u64, Ordering::Relaxed);
        sh.layers.store(l.layers as u32, Ordering::Relaxed);
        sh.redo.store(l.redo as u32, Ordering::Relaxed);
        sh.intensity.set(self.intensity);
        sh.loop_delay.store(self.delay, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests;
