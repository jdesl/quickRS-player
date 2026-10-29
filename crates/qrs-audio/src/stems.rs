//! A stemmed feedpak's stems, decoded side by side, and mixed into the one song buffer the engine plays. New in
//! the Rust port (rsplay plays a single stem; feedpak spec §5.3).
//!
//! Every stem is decoded and brought to the engine's format exactly as a single-stem song is (`to_song_buffer`:
//! stereo, the same resampler), then padded with silence to the longest, so the stems stay sample-aligned. The mix
//! is made off the audio thread whenever the mixer changes (a few hundred ms for a long song) and handed to the
//! engine as a whole new song buffer, so slow-down and pitch shift keep working on it unchanged. While the mixer is
//! as the pack set it, the pack's `full` mixdown (if it kept one) is played instead: the spec asks for it, since
//! the separated stems don't add back up to it exactly.

use std::sync::Arc;

use qrs_formats::feedpak::StemInfo;
use qrs_formats::song_audio::{EncodedAudio, StemAudio};

use crate::decode::{DecodeError, Pcm, SongBuffer, decode, to_song_buffer};

/// Gain steps the mixer goes up and down in, and its ceiling.
pub const GAIN_STEP: f32 = 0.1;
pub const MAX_GAIN: f32 = 1.5;

/// One stem: stereo i16 at the set's rate, as long as the set.
#[derive(Clone, Debug)]
pub struct Stem {
    pub info: StemInfo,
    pub samples: Arc<Vec<i16>>,
}

/// How one stem sits in the mix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StemMix {
    pub gain: f32,
    pub on: bool,
    pub solo: bool,
}

#[derive(Clone, Debug)]
pub struct StemSet {
    pub rate: u32,
    pub frames: usize,
    pub stems: Vec<Stem>,
    /// The pack's complete mixdown, same length.
    pub full: Option<Arc<Vec<i16>>>,
}

/// Each stem's gain in the mix: off when muted, or when something is soloed and it isn't (mute wins over solo).
pub fn effective_gains(mix: &[StemMix]) -> Vec<f32> {
    let any_solo = mix.iter().any(|m| m.solo);
    mix.iter()
        .map(|m| {
            if m.on && (!any_solo || m.solo) {
                m.gain.max(0.0)
            } else {
                0.0
            }
        })
        .collect()
}

fn padded(buf: SongBuffer, frames: usize) -> Arc<Vec<i16>> {
    let mut v = Arc::try_unwrap(buf.samples).unwrap_or_else(|a| (*a).clone());
    v.resize(frames * 2, 0);
    Arc::new(v)
}

impl StemSet {
    /// Decoded stems (and the mixdown) brought to `rate` and padded to the longest.
    pub fn from_pcm(
        rate: u32,
        stems: Vec<(StemInfo, Pcm)>,
        full: Option<Pcm>,
    ) -> Result<StemSet, DecodeError> {
        let mut bufs = Vec::with_capacity(stems.len());
        for (info, pcm) in stems {
            bufs.push((info, to_song_buffer(pcm, rate)?));
        }
        let full = full.map(|p| to_song_buffer(p, rate)).transpose()?;
        Ok(Self::assemble(rate, bufs, full))
    }

    fn assemble(rate: u32, stems: Vec<(StemInfo, SongBuffer)>, full: Option<SongBuffer>) -> StemSet {
        let frames = stems
            .iter()
            .map(|(_, b)| b.frames())
            .chain(full.as_ref().map(|f| f.frames()))
            .max()
            .unwrap_or(0);
        StemSet {
            rate,
            frames,
            stems: stems
                .into_iter()
                .map(|(info, b)| Stem {
                    info,
                    samples: padded(b, frames),
                })
                .collect(),
            full: full.map(|f| padded(f, frames)),
        }
    }

    /// Decode a pack's stems at `rate`, each on its own thread. A stem that won't decode is left out (and
    /// logged); it's an error only when none does.
    pub fn decode(
        rate: u32,
        stems: Vec<StemAudio>,
        full: Option<EncodedAudio>,
    ) -> Result<StemSet, DecodeError> {
        let one = |a: &EncodedAudio| decode(a).and_then(|pcm| to_song_buffer(pcm, rate));
        let (decoded, full) = std::thread::scope(|s| {
            let hs: Vec<_> = stems.iter().map(|st| s.spawn(|| one(&st.audio))).collect();
            let fh = full.as_ref().map(|f| s.spawn(|| one(f)));
            let decoded: Vec<_> = hs
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err(DecodeError("decoder panicked".into())))
                })
                .collect();
            let full = fh.map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(DecodeError("decoder panicked".into())))
            });
            (decoded, full)
        });
        let mut ok = Vec::new();
        let mut first_err = None;
        for (st, r) in stems.into_iter().zip(decoded) {
            match r {
                Ok(b) => ok.push((st.info, b)),
                Err(e) => {
                    log::warn!("stem {}: {e}", st.audio.origin);
                    first_err.get_or_insert(e);
                }
            }
        }
        if ok.is_empty() {
            return Err(first_err.unwrap_or_else(|| DecodeError("no stems".into())));
        }
        let full = match full {
            Some(Ok(b)) => Some(b),
            Some(Err(e)) => {
                log::warn!("full mixdown: {e}");
                None
            }
            None => None,
        };
        Ok(Self::assemble(rate, ok, full))
    }

    /// The mix as the pack sets it: each stem on if it's `default`, at unity, nothing soloed.
    pub fn default_mix(&self) -> Vec<StemMix> {
        self.stems
            .iter()
            .map(|s| StemMix {
                gain: 1.0,
                on: s.info.default,
                solo: false,
            })
            .collect()
    }

    /// Whether `mix` is the pack's own (so the `full` mixdown plays, when there is one).
    pub fn is_default(&self, mix: &[StemMix]) -> bool {
        let d = self.default_mix();
        mix.len() == d.len()
            && mix
                .iter()
                .zip(&d)
                .all(|(m, d)| m.on == d.on && !m.solo && (m.gain - 1.0).abs() < 1e-4)
    }

    /// The song buffer for `mix`: the stems summed with their gains (saturating), or the pack's mixdown while the
    /// mix is the pack's own. Off the audio thread (it allocates the whole song).
    pub fn mix(&self, mix: &[StemMix]) -> SongBuffer {
        if let Some(full) = &self.full
            && self.is_default(mix)
        {
            return SongBuffer {
                rate: self.rate,
                samples: full.clone(),
            };
        }
        let gains = effective_gains(mix);
        let parts: Vec<(&[i16], f32)> = self
            .stems
            .iter()
            .zip(gains.iter().copied().chain(std::iter::repeat(0.0)))
            .filter(|(_, g)| *g > 0.0)
            .map(|(s, g)| (s.samples.as_slice(), g))
            .collect();
        let n = self.frames * 2;
        let mut out = vec![0i16; n];
        const BLOCK: usize = 1 << 14;
        let mut acc = vec![0f32; BLOCK];
        let mut a = 0;
        while a < n {
            let b = (a + BLOCK).min(n);
            let acc = &mut acc[..b - a];
            acc.fill(0.0);
            for (s, g) in &parts {
                for (x, &v) in acc.iter_mut().zip(&s[a..b]) {
                    *x += v as f32 * g;
                }
            }
            for (o, x) in out[a..b].iter_mut().zip(acc.iter()) {
                *o = x.round().clamp(-32768.0, 32767.0) as i16;
            }
            a = b;
        }
        SongBuffer {
            rate: self.rate,
            samples: Arc::new(out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: &str, default: bool) -> StemInfo {
        StemInfo {
            id: id.into(),
            name: id.into(),
            file: format!("stems/{id}.ogg"),
            default,
        }
    }

    fn sine(rate: u32, hz: f64, amp: f64, seconds: f64) -> Pcm {
        let n = (rate as f64 * seconds) as usize;
        Pcm {
            rate,
            channels: 1,
            samples: (0..n)
                .map(|i| ((i as f64 / rate as f64 * hz * std::f64::consts::TAU).sin() * amp) as f32)
                .collect(),
        }
    }

    fn set() -> StemSet {
        // two stems of different lengths at 44.1 kHz, brought to 48 kHz like a song
        StemSet::from_pcm(
            48000,
            vec![
                (info("guitar", true), sine(44100, 220.0, 0.3, 2.0)),
                (info("drums", true), sine(44100, 330.0, 0.2, 1.5)),
                (info("click", false), sine(44100, 1000.0, 0.1, 2.0)),
            ],
            Some(sine(44100, 440.0, 0.5, 2.0)),
        )
        .unwrap()
    }

    #[test]
    fn stems_are_aligned_and_padded() {
        let s = set();
        assert!((s.frames as i64 - 96000).abs() <= 2, "{}", s.frames);
        assert!(s.stems.iter().all(|t| t.samples.len() == s.frames * 2));
        assert_eq!(s.full.as_ref().unwrap().len(), s.frames * 2);
        // the shorter stem is silent after its end
        assert!(s.stems[1].samples[2 * 73000..].iter().all(|&v| v == 0));
    }

    #[test]
    fn default_mix_plays_the_mixdown() {
        let s = set();
        let m = s.default_mix();
        assert!(s.is_default(&m));
        let b = s.mix(&m);
        assert!(Arc::ptr_eq(&b.samples, s.full.as_ref().unwrap()));
    }

    #[test]
    fn stems_are_summed_with_their_gains() {
        let s = set();
        let mut m = s.default_mix();
        m[0].gain = 0.5;
        m[2].on = true;
        let b = s.mix(&m);
        assert_eq!(b.frames(), s.frames);
        for i in [1000usize, 20001, 50000, 90001] {
            let want = s.stems[0].samples[i] as f32 * 0.5
                + s.stems[1].samples[i] as f32
                + s.stems[2].samples[i] as f32;
            assert!(
                (b.samples[i] as f32 - want).abs() <= 0.5,
                "{i}: {} {want}",
                b.samples[i]
            );
        }
        // mute and solo
        let mut m = s.default_mix();
        m[1].solo = true;
        let b = s.mix(&m);
        assert_eq!(&b.samples[..], &s.stems[1].samples[..]);
        m[1].on = false;
        assert!(s.mix(&m).samples.iter().all(|&v| v == 0), "mute wins over solo");
        assert_eq!(effective_gains(&m), vec![0.0, 0.0, 0.0]);
    }

    /// The spec's example pack (synthetic stems, vendored with the spec): every stem decodes and lines up.
    #[test]
    fn the_spec_example_pack_decodes() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("qrs-formats/tests/fixtures/feedpak/extended.feedpak");
        let mut pkg = qrs_formats::Package::open(&dir, &qrs_formats::Keys::default(), None).unwrap();
        let (stems, full) = qrs_formats::song_audio::song_stems(&mut pkg).unwrap().unwrap();
        let set = StemSet::decode(48000, stems, full).unwrap();
        assert_eq!(set.stems.len(), 4);
        assert!(set.frames > 0 && set.full.is_some());
        let mut m = set.default_mix();
        m[3].on = true; // the click too
        assert_eq!(set.mix(&m).frames(), set.frames);
    }

    #[test]
    fn a_loud_sum_saturates() {
        let loud = StemSet::from_pcm(
            8000,
            vec![
                (info("a", true), sine(8000, 100.0, 0.9, 0.5)),
                (info("b", true), sine(8000, 100.0, 0.9, 0.5)),
            ],
            None,
        )
        .unwrap();
        let b = loud.mix(&loud.default_mix());
        let peak = b.samples.iter().map(|v| v.unsigned_abs()).max().unwrap();
        assert!(peak >= 32767, "{peak}");
    }
}
