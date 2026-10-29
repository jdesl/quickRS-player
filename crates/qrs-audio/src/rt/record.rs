//! The record tap: a ring of the last seconds of what the output plays (the final stereo mix, plus the guitar when
//! it isn't monitored here), written by the output audio thread and drained to a WAV file by `crate::recorder` on
//! its own thread. New for quickRS (P23.9).
//!
//! One frame is one `AtomicU32` (left in the low 16 bits, right in the high, 16-bit PCM), addressed by its absolute
//! frame index since the tap was made. Like the Rewind ring, the writer publishes the total with Release and a
//! reader copies, then checks nothing it copied was overwritten. The tap is made off the audio thread; writing is
//! a store per frame (no allocation, no locks, no I/O).

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub struct RecordTap {
    pub rate: u32,
    frames: usize,
    buf: Box<[AtomicU32]>,
    written: AtomicU64,
}

#[inline]
fn q(x: f32) -> u16 {
    ((x.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16
}

impl RecordTap {
    /// A tap holding `seconds` at `rate` (built off the audio thread).
    pub fn new(rate: u32, seconds: f64) -> RecordTap {
        let frames = ((seconds * rate as f64) as usize).max(4096);
        RecordTap {
            rate,
            frames,
            buf: (0..frames).map(|_| AtomicU32::new(0)).collect(),
            written: AtomicU64::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.frames
    }

    /// Frames written since the tap was made (the index the next frame gets).
    pub fn written(&self) -> u64 {
        self.written.load(Ordering::Acquire)
    }

    /// Frames a writer may be overwriting before it publishes them (readers treat them as gone).
    fn guard(&self) -> u64 {
        (self.frames as u64 / 8).min(16384)
    }

    /// The oldest frame that can still be read.
    pub fn oldest(&self) -> u64 {
        self.written().saturating_sub(self.frames as u64 - self.guard())
    }

    /// Audio thread only (one writer): a block of interleaved stereo frames (`lr.len() / 2` of them).
    pub fn write(&self, lr: &[f32]) {
        let n = lr.len() / 2;
        let mut w = self.written.load(Ordering::Relaxed);
        let piece = self.guard() as usize;
        let mut i = 0;
        while i < n {
            let end = (i + piece).min(n);
            for k in i..end {
                let f = ((w + (k - i) as u64) % self.frames as u64) as usize;
                let v = u32::from(q(lr[2 * k])) | (u32::from(q(lr[2 * k + 1])) << 16);
                self.buf[f].store(v, Ordering::Relaxed);
            }
            w += (end - i) as u64;
            self.written.store(w, Ordering::Release);
            i = end;
        }
    }

    /// Frames [from, to) as interleaved stereo 16-bit samples, appended to `out`; false (and `out` as it was) if
    /// they aren't all there (not yet written, or overwritten).
    pub fn read_into(&self, from: u64, to: u64, out: &mut Vec<i16>) -> bool {
        if to < from || to > self.written() || from < self.oldest() {
            return false;
        }
        let start = out.len();
        out.reserve(2 * (to - from) as usize);
        for i in from..to {
            let v = self.buf[(i % self.frames as u64) as usize].load(Ordering::Relaxed);
            out.push(v as u16 as i16);
            out.push((v >> 16) as u16 as i16);
        }
        std::sync::atomic::fence(Ordering::Acquire);
        if from >= self.oldest() {
            true
        } else {
            out.truncate(start);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tap_wraps_and_reads_back_what_was_written() {
        let t = RecordTap::new(48000, 0.1); // 4800 frames
        assert_eq!(t.capacity(), 4800);
        let sig = |i: u64| ((i as f32) * 0.01).sin() * 0.5;
        let mut i = 0u64;
        for block in 0..80 {
            let n = 64 + (block * 53) % 300;
            let lr: Vec<f32> = (i..i + n).flat_map(|k| [sig(k), -sig(k)]).collect();
            t.write(&lr);
            i += n;
        }
        assert_eq!(t.written(), i);
        let from = t.oldest();
        let mut got = Vec::new();
        assert!(t.read_into(from, i, &mut got));
        assert_eq!(got.len() as u64, 2 * (i - from));
        for (k, pair) in got.chunks(2).enumerate() {
            let want = (sig(from + k as u64) * 32767.0).round() as i16;
            assert!((pair[0] - want).abs() <= 1 && (pair[1] + want).abs() <= 1);
        }
        let mut none = vec![7i16];
        assert!(!t.read_into(from - 1, i, &mut none), "overwritten");
        assert!(!t.read_into(i - 10, i + 1, &mut none), "not yet written");
        assert_eq!(none, [7], "a failed read leaves the buffer as it was");
    }
}
