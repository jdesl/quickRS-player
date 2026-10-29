//! Ear training (new in quickRS, P13.2; no Python source): which note each pluck was, read from the recording just
//! after it, and how it compares with the note you're looking for. One verdict a pluck, so a repeated note needs a
//! new pluck, and a note left ringing isn't judged again.

use std::collections::VecDeque;

use crate::detect::SampleSource;
use crate::pitch::{hz_to_midi, pitch};
use crate::spectrum::Spectra;

/// Seconds after the onset before the pitch is read (past the pick's attack).
pub const SETTLE: f64 = 0.03;
/// Seconds of sound the pitch is read from (two periods of 35 Hz and some).
pub const WINDOW: f64 = 0.085;
/// A reading this clear counts; a less clear one is tried again a little later.
pub const CLARITY: f64 = 0.8;
/// Tries per pluck (each RETRY seconds later than the one before).
pub const TRIES: u32 = 3;
pub const RETRY: f64 = 0.05;

/// How a played note compares with the one you're after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The same note.
    Match,
    /// The same note in another octave: this many octaves to go (+ up, - down).
    Octave(i32),
    /// Another note: this many semitones to go (+ up, - down), which is frets on the same string.
    Off(i32),
}

/// What you played (MIDI, fractional) against the note you're after.
pub fn judge(played: f64, target: i32) -> Verdict {
    let to_go = target - played.round() as i32;
    if to_go == 0 {
        Verdict::Match
    } else if to_go % 12 == 0 {
        Verdict::Octave(to_go / 12)
    } else {
        Verdict::Off(to_go)
    }
}

/// Reads the note of each pluck from the recording (`SampleSource`) once enough of it has come in.
pub struct PluckReader {
    /// Onsets waiting to be read: (capture index, tries so far).
    queue: VecDeque<(u64, u32)>,
    sp: Spectra,
    buf: Vec<f32>,
}

impl Default for PluckReader {
    fn default() -> Self {
        PluckReader {
            queue: VecDeque::new(),
            sp: Spectra::new(),
            buf: Vec::new(),
        }
    }
}

impl PluckReader {
    /// A pluck at capture index `index`. An earlier pluck still waiting whose window this one would cut into is
    /// dropped (a fast run, or the detector firing twice on one pluck): only the newer one is read.
    pub fn push(&mut self, index: u64, rate: u32) {
        let (settle, window) = Self::spans(rate);
        self.queue.retain(|&(i, tries)| {
            let start = i + settle + tries as u64 * Self::retry(rate);
            start + window <= index
        });
        self.queue.push_back((index, 0));
    }

    /// Forget every pluck waiting (the lesson moved on).
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn waiting(&self) -> bool {
        !self.queue.is_empty()
    }

    fn spans(rate: u32) -> (u64, u64) {
        ((SETTLE * rate as f64) as u64, (WINDOW * rate as f64) as u64)
    }

    fn retry(rate: u32) -> u64 {
        (RETRY * rate as f64) as u64
    }

    /// The plucks that can be read now, oldest first: each one's note (MIDI, fractional), or None when no clear
    /// pitch could be read from it (a muted string, a scratch, a chord).
    pub fn poll(&mut self, src: &dyn SampleSource) -> Vec<(u64, Option<f64>)> {
        let rate = src.rate();
        let mut out = Vec::new();
        if rate == 0 {
            return out;
        }
        let (settle, window) = Self::spans(rate);
        self.buf.resize(window as usize, 0.0);
        while let Some(&(index, tries)) = self.queue.front() {
            let start = index + settle + tries as u64 * Self::retry(rate);
            if src.written() < start + window {
                break; // not recorded yet (the queue is in order)
            }
            self.queue.pop_front();
            if !src.read(start, &mut self.buf) {
                out.push((index, None)); // already gone from the recording
                continue;
            }
            let (hz, clarity) = pitch(&mut self.sp, &self.buf, rate);
            if hz > 0.0 && clarity >= CLARITY {
                out.push((index, Some(hz_to_midi(hz))));
            } else if tries + 1 < TRIES {
                self.queue.push_front((index, tries + 1)); // try a little later in the note
                if src.written() < start + Self::retry(rate) + window {
                    break;
                }
            } else {
                out.push((index, None));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::midi_to_hz;

    /// A recording that grows as it's "played".
    struct Rec {
        rate: u32,
        data: Vec<f32>,
        written: u64,
    }

    impl SampleSource for Rec {
        fn rate(&self) -> u32 {
            self.rate
        }
        fn written(&self) -> u64 {
            self.written
        }
        fn read(&self, start: u64, out: &mut [f32]) -> bool {
            let end = start as usize + out.len();
            if end as u64 > self.written || end > self.data.len() {
                return false;
            }
            out.copy_from_slice(&self.data[start as usize..end]);
            true
        }
        fn latest(&self, out: &mut [f32]) -> bool {
            let w = self.written as usize;
            w >= out.len() && self.read((w - out.len()) as u64, out)
        }
    }

    /// A plucked-string-like tone (fundamental and a few decaying harmonics) from `at` seconds for `dur`.
    fn pluck(data: &mut [f32], rate: u32, at: f64, dur: f64, midi: f64) {
        let f = midi_to_hz(midi);
        let i0 = (at * rate as f64) as usize;
        let n = (dur * rate as f64) as usize;
        for k in 0..n.min(data.len().saturating_sub(i0)) {
            let t = k as f64 / rate as f64;
            let env = (-t * 3.0).exp();
            let v: f64 = (1..=4)
                .map(|h| (2.0 * std::f64::consts::PI * f * h as f64 * t).sin() / h as f64)
                .sum();
            data[i0 + k] = (0.3 * env * v) as f32;
        }
    }

    #[test]
    fn judging_a_note() {
        assert_eq!(judge(52.0, 52), Verdict::Match);
        assert_eq!(judge(52.3, 52), Verdict::Match); // a little sharp is still the note
        assert_eq!(judge(51.7, 52), Verdict::Match);
        assert_eq!(judge(50.0, 52), Verdict::Off(2)); // two frets higher
        assert_eq!(judge(57.0, 52), Verdict::Off(-5));
        assert_eq!(judge(40.0, 52), Verdict::Octave(1)); // the right note an octave down
        assert_eq!(judge(76.0, 52), Verdict::Octave(-2));
    }

    #[test]
    fn reads_each_pluck_once_when_its_sound_is_in() {
        let rate = 48000;
        let mut rec = Rec {
            rate,
            data: vec![0.0; rate as usize * 3],
            written: 0,
        };
        // E3 at 0.5 s, G3 at 1.2 s
        pluck(&mut rec.data, rate, 0.5, 0.7, 52.0);
        pluck(&mut rec.data, rate, 1.2, 1.0, 55.0);
        let mut r = PluckReader::default();
        r.push((0.5 * rate as f64) as u64, rate);
        rec.written = (0.55 * rate as f64) as u64; // not enough of it yet
        assert!(r.poll(&rec).is_empty());
        assert!(r.waiting());
        rec.written = (1.25 * rate as f64) as u64;
        let got = r.poll(&rec);
        assert_eq!(got.len(), 1);
        assert!((got[0].1.unwrap() - 52.0).abs() < 0.2, "{got:?}");
        r.push((1.2 * rate as f64) as u64, rate);
        rec.written = (2.0 * rate as f64) as u64;
        let got = r.poll(&rec);
        assert_eq!(got.len(), 1);
        assert!((got[0].1.unwrap() - 55.0).abs() < 0.2, "{got:?}");
        assert!(r.poll(&rec).is_empty()); // once each
    }

    #[test]
    fn a_double_trigger_is_read_once() {
        let rate = 44100;
        let mut rec = Rec {
            rate,
            data: vec![0.0; rate as usize * 2],
            written: 0,
        };
        pluck(&mut rec.data, rate, 0.2, 1.0, 45.0);
        let mut r = PluckReader::default();
        r.push((0.2 * rate as f64) as u64, rate);
        r.push((0.23 * rate as f64) as u64, rate); // the detector fired again 30 ms later
        rec.written = rec.data.len() as u64;
        let got = r.poll(&rec);
        assert_eq!(got.len(), 1, "{got:?}");
        assert!((got[0].1.unwrap() - 45.0).abs() < 0.2, "{got:?}");
    }

    #[test]
    fn silence_reads_as_nothing() {
        let rate = 48000;
        let rec = Rec {
            rate,
            data: vec![0.0; rate as usize],
            written: rate as u64,
        };
        let mut r = PluckReader::default();
        r.push(1000, rate);
        let got = r.poll(&rec);
        assert_eq!(got, vec![(1000, None)]);
        assert!(!r.waiting());
    }
}
