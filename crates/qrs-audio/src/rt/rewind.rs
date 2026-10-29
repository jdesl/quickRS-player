//! Rewind: an always-on ring of the last minutes of the guitar (dry, through the rig, or both), written by the
//! output audio thread and read by anyone (the waveform, saving a stretch to WAV). New for quickRS (P22.4).
//!
//! Mono 16-bit per kept signal: 10 minutes at 48 kHz is 57.6 MB for one, 115.2 MB for both. Samples live in
//! `AtomicU16`s and are addressed by their absolute frame index since the ring was made; like the capture ring,
//! the writer publishes the total with Release and a reader copies, then checks nothing it copied was overwritten.
//! A peak per 10 ms block is kept beside the samples, so a ten-minute waveform is read from 60 000 numbers.

use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};

/// Which signals the ring keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RewindKeep {
    /// The guitar as it came in (it can be re-amped later).
    Dry,
    /// Through the rig, as heard.
    Wet,
    #[default]
    Both,
}

/// Which signal to read back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewindSource {
    Dry,
    Wet,
}

/// Frames per stored peak (10 ms at 48 kHz).
pub const PEAK_BLOCK: usize = 480;

pub struct RewindRing {
    pub rate: u32,
    pub keep: RewindKeep,
    /// Frames held (a whole number of peak blocks).
    frames: usize,
    channels: usize,
    buf: Box<[AtomicU16]>,
    /// Per block: the dry peak in the high half, the wet in the low (0..65535 = 0..1).
    peaks: Box<[AtomicU32]>,
    written: AtomicU64,
}

/// The writer's running peaks of the block being filled (audio thread state).
#[derive(Default, Clone, Copy)]
pub struct PeakAcc {
    dry: f32,
    wet: f32,
}

#[inline]
fn to_i16(x: f32) -> u16 {
    ((x.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16
}

#[inline]
fn from_i16(v: u16) -> f32 {
    v as i16 as f32 / 32767.0
}

impl RewindRing {
    /// A ring for `seconds` at `rate` (built off the audio thread: this is the big allocation).
    pub fn new(rate: u32, seconds: f64, keep: RewindKeep) -> RewindRing {
        let blocks = ((seconds * rate as f64) as usize).div_ceil(PEAK_BLOCK).max(4);
        let frames = blocks * PEAK_BLOCK;
        let channels = if keep == RewindKeep::Both { 2 } else { 1 };
        RewindRing {
            rate,
            keep,
            frames,
            channels,
            buf: (0..frames * channels).map(|_| AtomicU16::new(0)).collect(),
            peaks: (0..blocks).map(|_| AtomicU32::new(0)).collect(),
            written: AtomicU64::new(0),
        }
    }

    /// Bytes of audio held (for the page's "115 MB").
    pub fn bytes(&self) -> usize {
        self.buf.len() * 2 + self.peaks.len() * 4
    }

    pub fn capacity(&self) -> usize {
        self.frames
    }

    pub fn seconds(&self) -> f64 {
        self.frames as f64 / self.rate as f64
    }

    /// Frames written since the ring was made (the index the next frame gets).
    pub fn written(&self) -> u64 {
        self.written.load(Ordering::Acquire)
    }

    /// Frames a writer may be overwriting before it publishes them (readers treat them as gone).
    fn guard(&self) -> u64 {
        (self.frames as u64 / 8).min(16384)
    }

    /// The oldest frame that can still be read back.
    pub fn oldest(&self) -> u64 {
        self.written().saturating_sub(self.frames as u64 - self.guard())
    }

    fn slot(&self, src: RewindSource) -> Option<usize> {
        match (self.keep, src) {
            (RewindKeep::Both, RewindSource::Dry) => Some(0),
            (RewindKeep::Both, RewindSource::Wet) => Some(1),
            (RewindKeep::Dry, RewindSource::Dry) | (RewindKeep::Wet, RewindSource::Wet) => Some(0),
            _ => None,
        }
    }

    pub fn keeps(&self, src: RewindSource) -> bool {
        self.slot(src).is_some()
    }

    /// Audio thread only (one writer): a block of the guitar, dry and through the rig (same length).
    pub fn write(&self, dry: &[f32], wet: &[f32], acc: &mut PeakAcc) {
        let n = dry.len().min(wet.len());
        let mut w = self.written.load(Ordering::Relaxed);
        let piece = self.guard() as usize;
        let mut i = 0;
        while i < n {
            let end = (i + piece).min(n);
            for k in i..end {
                let f = ((w + (k - i) as u64) % self.frames as u64) as usize;
                let (d, x) = (dry[k], wet[k]);
                match self.keep {
                    RewindKeep::Both => {
                        self.buf[2 * f].store(to_i16(d), Ordering::Relaxed);
                        self.buf[2 * f + 1].store(to_i16(x), Ordering::Relaxed);
                    }
                    RewindKeep::Dry => self.buf[f].store(to_i16(d), Ordering::Relaxed),
                    RewindKeep::Wet => self.buf[f].store(to_i16(x), Ordering::Relaxed),
                }
                acc.dry = acc.dry.max(d.abs());
                acc.wet = acc.wet.max(x.abs());
                if (f + 1).is_multiple_of(PEAK_BLOCK) {
                    let q = |v: f32| (v.min(1.0) * 65535.0) as u32;
                    self.peaks[f / PEAK_BLOCK].store((q(acc.dry) << 16) | q(acc.wet), Ordering::Relaxed);
                    *acc = PeakAcc::default();
                }
            }
            w += (end - i) as u64;
            self.written.store(w, Ordering::Release);
            i = end;
        }
    }

    /// Frames [from, to) of one signal, or None if they aren't all there (not yet written, or overwritten).
    pub fn read(&self, from: u64, to: u64, src: RewindSource) -> Option<Vec<f32>> {
        let slot = self.slot(src)?;
        if to < from || to > self.written() || from < self.oldest() {
            return None;
        }
        let ch = self.channels;
        let out: Vec<f32> = (from..to)
            .map(|i| {
                let f = (i % self.frames as u64) as usize;
                from_i16(self.buf[f * ch + slot].load(Ordering::Relaxed))
            })
            .collect();
        std::sync::atomic::fence(Ordering::Acquire);
        (from >= self.oldest()).then_some(out)
    }

    /// The waveform of frames [from, to) in `columns` peaks (0..1), from the 10 ms block peaks (a block not yet
    /// complete, or gone, reads 0).
    pub fn waveform(&self, from: u64, to: u64, columns: usize, src: RewindSource) -> Vec<f32> {
        let mut out = vec![0.0f32; columns];
        if columns == 0 || to <= from || !self.keeps(src) {
            return out;
        }
        let (lo, hi) = (self.oldest(), self.written());
        let blocks = self.peaks.len() as u64;
        let pb = PEAK_BLOCK as u64;
        let span = (to - from) as f64 / columns as f64;
        for (c, o) in out.iter_mut().enumerate() {
            let a = from + (c as f64 * span) as u64;
            let b = (from + ((c + 1) as f64 * span) as u64).max(a + 1);
            // the blocks overlapping [a, b), complete and still held
            let (ba, bb) = (a / pb, b.div_ceil(pb));
            let mut m = 0u32;
            for blk in ba..bb {
                if blk * pb < lo || (blk + 1) * pb > hi {
                    continue;
                }
                let v = self.peaks[(blk % blocks) as usize].load(Ordering::Relaxed);
                m = m.max(match src {
                    RewindSource::Dry => v >> 16,
                    RewindSource::Wet => v & 0xffff,
                });
            }
            *o = m as f32 / 65535.0;
        }
        out
    }

    /// Save frames [from, to) of one signal as a mono 16-bit WAV.
    pub fn save_wav(
        &self,
        path: &std::path::Path,
        from: u64,
        to: u64,
        src: RewindSource,
    ) -> std::io::Result<()> {
        let x = self.read(from, to, src).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "that stretch isn't in the Rewind ring (not recorded, or already overwritten)",
            )
        })?;
        let s: Vec<i16> = x.iter().map(|&v| to_i16(v) as i16).collect();
        crate::decode::write_wav_i16(path, self.rate, 1, &s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_wraps_and_reads_back_what_was_written() {
        // 4 blocks of 480 frames: 1920 frames held, the oldest 240 (guard) treated as gone
        let r = RewindRing::new(48000, 0.04, RewindKeep::Both);
        assert_eq!(r.capacity(), 1920);
        let mut acc = PeakAcc::default();
        let sig = |i: u64| ((i as f32) * 0.013).sin() * 0.5;
        let mut i = 0u64;
        for block in 0..60 {
            let n = 64 + (block * 37) % 200;
            let dry: Vec<f32> = (i..i + n).map(sig).collect();
            let wet: Vec<f32> = dry.iter().map(|v| v * -0.5).collect();
            r.write(&dry, &wet, &mut acc);
            i += n;
        }
        assert_eq!(r.written(), i);
        assert!(i > 3 * 1920, "wrapped a few times: {i}");
        assert_eq!(r.oldest(), i - (1920 - 240));
        let from = r.oldest();
        let got = r.read(from, i, RewindSource::Dry).unwrap();
        for (k, v) in got.iter().enumerate() {
            assert!((v - sig(from + k as u64)).abs() < 1e-4);
        }
        let wet = r.read(i - 100, i, RewindSource::Wet).unwrap();
        assert!((wet[99] + 0.5 * sig(i - 1)).abs() < 1e-4);
        assert!(r.read(from - 1, i, RewindSource::Dry).is_none(), "overwritten");
        assert!(
            r.read(i - 10, i + 1, RewindSource::Dry).is_none(),
            "not yet written"
        );
        let wave = r.waveform(from, i, 8, RewindSource::Dry);
        // (the first and last columns fall on blocks gone or not yet complete)
        assert!(wave[1..7].iter().all(|&p| p > 0.4 && p <= 0.51), "{wave:?}");
        let dry_only = RewindRing::new(48000, 0.04, RewindKeep::Dry);
        assert!(dry_only.read(0, 0, RewindSource::Wet).is_none());
        assert_eq!(r.bytes(), 1920 * 2 * 2 + 4 * 4);
        assert_eq!(dry_only.bytes(), 1920 * 2 + 4 * 4);
    }

    #[test]
    fn a_saved_stretch_is_the_input() {
        let r = RewindRing::new(48000, 1.0, RewindKeep::Dry);
        let x: Vec<f32> = (0..30000).map(|i| ((i as f32) * 0.05).sin() * 0.7).collect();
        let mut acc = PeakAcc::default();
        for c in x.chunks(256) {
            r.write(c, c, &mut acc);
        }
        let dir = std::env::temp_dir().join(format!("qrs-rewind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bit.wav");
        r.save_wav(&path, 1000, 25000, RewindSource::Dry).unwrap();
        let (rate, ch, s) = crate::decode::read_wav_i16(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!((rate, ch, s.len()), (48000, 1, 24000));
        for (k, v) in s.iter().enumerate() {
            let want = (x[1000 + k] * 32767.0).round() as i16;
            assert!((v - want).abs() <= 1, "{k}: {v} vs {want}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ten_minutes_of_both_is_115_mb() {
        // the arithmetic only (not allocated here): 10 min x 48 kHz x 2 signals x 2 bytes
        let frames = 600 * 48000usize;
        assert_eq!(frames * 2 * 2, 115_200_000);
    }
}
