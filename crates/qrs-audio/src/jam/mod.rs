//! The amp room's jam tools (P22.4): a metronome, drum loops, a backing drone or chord loop, a looper and Rewind.
//! New for quickRS; the Python app has none of it.
//!
//! What runs on the audio thread is in `rt/jam.rs` (and `rt/rewind.rs`); this module holds what the app sets
//! (plain values sent over the output's command queue), what it reads back (`JamState`), and what is built off the
//! audio thread: the drum kit (`drums`), the backing clip (`backing`), the theory behind it (`theory`) and the tap
//! tempo helper. `Engine` (engine.rs) has the methods: `set_jam_tempo`, `jam_play`, `set_click`, `set_drums`,
//! `set_backing`, `looper`, `set_rewind`, `jam`...

pub mod backing;
pub mod drums;
pub mod theory;

pub use crate::rt::rewind::{RewindKeep, RewindRing, RewindSource};
pub use backing::{BackingClip, BackingKind, BackingSpec};
pub use drums::{DrumKit, DrumStyle};

/// The metronome's click.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClickParams {
    pub on: bool,
    /// Beat one louder and higher.
    pub accent: bool,
    /// 0..1 (and a little over).
    pub volume: f32,
}

impl Default for ClickParams {
    fn default() -> Self {
        ClickParams {
            on: true,
            accent: true,
            volume: 0.6,
        }
    }
}

/// The drum loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrumParams {
    pub on: bool,
    pub style: DrumStyle,
    /// The kit's level, and each part's (the page's VOL, KICK, SNARE, HAT).
    pub volume: f32,
    pub kick: f32,
    pub snare: f32,
    pub hats: f32,
    /// A fill in the last two beats of every n-th bar (0: no fills), a crash on the bar after.
    pub fill_every: u32,
    /// The band follows how hard you play: the kit gets louder and adds its ghost notes as the guitar does.
    pub follow: bool,
}

impl Default for DrumParams {
    fn default() -> Self {
        DrumParams {
            on: false,
            style: DrumStyle::Rock,
            volume: 0.7,
            kick: 1.0,
            snare: 1.0,
            hats: 1.0,
            fill_every: 0,
            follow: false,
        }
    }
}

/// What the looper's buttons do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopAction {
    /// The one-button flow: record, close the loop, overdub, stop overdubbing (a looper pedal's footswitch).
    Press,
    /// Start the first take (when empty) or overdub (when playing).
    Record,
    /// Close the first take / stop overdubbing and play; play a stopped loop from its top.
    Play,
    Overdub,
    /// Stop playing (the loop is kept).
    Stop,
    /// Drop the newest layer (an overdub; never the first take: that's Clear).
    Undo,
    /// Put the layer back that Undo dropped.
    Redo,
    Clear,
}

/// Where the looper is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoopState {
    /// No memory for it yet (`Engine::set_looper` makes it).
    #[default]
    Off,
    Empty,
    /// Recording starts at the next bar line (the click or drums are on).
    Armed,
    /// The first take.
    Recording,
    Playing,
    Overdubbing,
    Stopped,
}

impl LoopState {
    pub fn from_u32(v: u32) -> LoopState {
        match v {
            1 => LoopState::Empty,
            2 => LoopState::Armed,
            3 => LoopState::Recording,
            4 => LoopState::Playing,
            5 => LoopState::Overdubbing,
            6 => LoopState::Stopped,
            _ => LoopState::Off,
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            LoopState::Off => 0,
            LoopState::Empty => 1,
            LoopState::Armed => 2,
            LoopState::Recording => 3,
            LoopState::Playing => 4,
            LoopState::Overdubbing => 5,
            LoopState::Stopped => 6,
        }
    }
}

/// The looper's memory, made off the audio thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LooperSettings {
    /// The longest loop (the first take's length is capped here).
    pub max_seconds: f64,
    /// Layers kept for undo (the first take and its overdubs).
    pub max_layers: usize,
}

impl Default for LooperSettings {
    fn default() -> Self {
        LooperSettings {
            max_seconds: 60.0,
            max_layers: 8,
        }
    }
}

impl LooperSettings {
    /// Bytes this asks for at `rate`: every layer keeps the guitar dry and through the rig, 16-bit.
    pub fn bytes(&self, rate: u32) -> usize {
        (self.max_seconds * rate as f64) as usize * self.max_layers * 2 * 2
    }
}

/// Rewind's memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RewindSettings {
    pub minutes: f64,
    pub keep: RewindKeep,
}

impl Default for RewindSettings {
    fn default() -> Self {
        RewindSettings {
            minutes: 10.0,
            keep: RewindKeep::Both,
        }
    }
}

/// The jam as the page shows it (read whenever it repaints).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JamState {
    /// The click / drums / backing are playing.
    pub running: bool,
    pub bpm: f64,
    pub beats_per_bar: u32,
    /// Beats since bar 0's downbeat, as heard now (negative in the count-in).
    pub beat: f64,
    /// The bar (negative in the count-in), the beat in it (0-based) and how far through that beat (0..1): the
    /// beat light is on for the first part of each beat.
    pub bar: i64,
    pub beat_in_bar: u32,
    pub phase: f64,
    /// How hard the band is playing when it follows the guitar (0..1).
    pub intensity: f32,
    pub looper: LoopState,
    /// Seconds into the loop and its length (0 until the first take is closed).
    pub loop_pos: f64,
    pub loop_len: f64,
    /// Layers playing (the first take counts), and layers Redo can bring back.
    pub layers: u32,
    pub redo: u32,
    /// The latency the looper takes off what it records (the guitar's round trip), seconds.
    pub loop_latency: f64,
    /// Seconds of Rewind recorded (up to its length) and its length.
    pub rewind_seconds: f64,
    pub rewind_capacity: f64,
}

/// Tap tempo: the beats a user taps, turned into a tempo (off the audio thread; the page calls `tap` on a click
/// or a key).
#[derive(Clone, Debug, Default)]
pub struct TapTempo {
    taps: Vec<f64>,
}

impl TapTempo {
    /// Taps further apart than this start again.
    pub const RESET: f64 = 2.0;

    pub fn new() -> TapTempo {
        TapTempo::default()
    }

    /// A tap at `t` seconds (any clock); the tempo once there are two taps (the average of the last few
    /// intervals, 30..300 bpm).
    pub fn tap(&mut self, t: f64) -> Option<f64> {
        if self.taps.last().is_some_and(|&l| t - l > Self::RESET || t <= l) {
            self.taps.clear();
        }
        self.taps.push(t);
        if self.taps.len() > 8 {
            self.taps.remove(0);
        }
        if self.taps.len() < 2 {
            return None;
        }
        let span = self.taps[self.taps.len() - 1] - self.taps[0];
        let bpm = 60.0 * (self.taps.len() - 1) as f64 / span;
        Some(bpm.clamp(30.0, 300.0))
    }

    pub fn reset(&mut self) {
        self.taps.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_tempo_averages_and_resets() {
        let mut t = TapTempo::new();
        assert_eq!(t.tap(10.0), None);
        let bpm = t.tap(10.5).unwrap();
        assert!((bpm - 120.0).abs() < 1e-9);
        t.tap(11.02);
        let bpm = t.tap(11.5).unwrap();
        assert!((bpm - 120.0).abs() < 0.1, "{bpm}");
        // a pause starts again
        assert_eq!(t.tap(20.0), None);
        assert!((t.tap(21.0).unwrap() - 60.0).abs() < 1e-9);
    }

    #[test]
    fn the_looper_memory_budget() {
        // 60 s x 8 layers x (dry + wet) x 16-bit at 48 kHz
        assert_eq!(LooperSettings::default().bytes(48000), 92_160_000);
    }
}
