//! The clock of a screenshot sequence (QUICKRS_SHOT_SEQ="t0:t1:fps"): while one is being rendered, the song time of
//! the frame being drawn, advanced exactly 1/fps per frame by `shots.rs` (never the wall clock). Anything animated
//! by time (the highway's hit and miss effects) reads it so a sequence shows motion as it happens at speed.

use std::sync::atomic::{AtomicU64, Ordering};

/// The time's f64 bits; NaN = no sequence running.
static NOW: AtomicU64 = AtomicU64::new(0x7ff8_0000_0000_0000);

/// The sequence's time (seconds of song audio, which a sequence frame holds the song at), or None outside a
/// sequence. It only ever moves forward, by exactly 1/fps a frame, so it also serves as a clock for ageing effects.
pub fn shot_now() -> Option<f64> {
    let t = f64::from_bits(NOW.load(Ordering::Relaxed));
    (!t.is_nan()).then_some(t)
}

/// Set by the sequence (None: it's over).
pub(crate) fn set(t: Option<f64>) {
    NOW.store(t.unwrap_or(f64::NAN).to_bits(), Ordering::Relaxed);
}

/// QUICKRS_SHOT_SEQ parsed: (t0, t1, fps), t1 > t0, 1 <= fps <= 240.
pub fn seq_env() -> Option<(f64, f64, f64)> {
    parse(&std::env::var("QUICKRS_SHOT_SEQ").ok()?)
}

fn parse(s: &str) -> Option<(f64, f64, f64)> {
    let v: Vec<f64> = s
        .split(':')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    match v[..] {
        [t0, t1, fps] if t1 > t0 && t0 >= 0.0 && (1.0..=240.0).contains(&fps) => Some((t0, t1, fps)),
        _ => None,
    }
}

/// How many frames a sequence has: t0, t0 + 1/fps, ... up to and including t1.
pub fn frame_count(t0: f64, t1: f64, fps: f64) -> u64 {
    ((t1 - t0) * fps + 1e-6).floor() as u64 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequences_parse_and_count() {
        assert_eq!(parse("58:66:30"), Some((58.0, 66.0, 30.0)));
        assert_eq!(parse("1.5 : 2 : 60"), Some((1.5, 2.0, 60.0)));
        for bad in ["66:58:30", "1:2", "a:b:c", "1:2:0", "1:2:1000", "-1:2:30"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        assert_eq!(frame_count(58.0, 66.0, 30.0), 241);
        assert_eq!(frame_count(0.0, 0.1, 30.0), 4);
        assert_eq!(shot_now(), None);
        set(Some(60.5));
        assert_eq!(shot_now(), Some(60.5));
        set(None);
        assert_eq!(shot_now(), None);
    }
}
