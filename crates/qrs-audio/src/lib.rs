//! quickRS audio engine: devices, the real-time graph, tones, decoding, stretching, synth.
//!
//! Designed fresh for this app (docs/PLAN.md §4); the Python app's engine is only a list of features here.

pub mod backend;
pub mod board;
pub mod clock;
pub mod decode;
pub mod devices;
pub mod engine;
pub mod jam;
pub mod recorder;
pub mod rt;
pub mod stems;
pub mod stretch;
pub mod synth;
pub mod tone;

#[cfg(test)]
mod alloc_count;

pub use devices::{Audio, AudioEvent, AudioPrefs, Choice, DevicePref};
pub use engine::{Engine, Event, InputRoute, Meters, PreviewState, Route, Stats, Status};
