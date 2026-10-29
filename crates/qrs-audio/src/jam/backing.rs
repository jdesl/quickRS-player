//! The jam's backing: a drone or a chord loop, rendered off the audio thread into a clip that loops seamlessly at
//! the metronome's tempo. New for quickRS (P22.4).
//!
//! The pad is additive (a few soft harmonics, three voices detuned a few cents and spread left to right, a slow
//! attack); the bass is the tab synth's Karplus-Strong string (`synth::string_tone`) on the chord's root. A chord
//! loop is rendered twice through and the second pass kept, so the last chord's release rings into the first
//! (no cut at the loop point); a drone is one held note whose end is crossfaded into its start.

use super::theory::{Chord, Quality, Scale};
use crate::synth::{Rng, midi_hz, string_tone};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BackingKind {
    /// The key's root and fifth, held.
    #[default]
    Drone,
    /// The progression, one chord every `beats_per_chord` beats.
    Chords,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackingSpec {
    pub kind: BackingKind,
    /// The key's root (pitch class, 0 = C).
    pub key: u8,
    pub scale: Scale,
    /// The chord loop (Chords): see `theory::parse_progression`.
    pub chords: Vec<Chord>,
    /// Beats each chord lasts (0: a bar).
    pub beats_per_chord: u32,
    /// A bass under the pad (plucked roots).
    pub bass: bool,
}

impl Default for BackingSpec {
    fn default() -> Self {
        BackingSpec {
            kind: BackingKind::Drone,
            key: 9,
            scale: Scale::MinorPentatonic,
            chords: Vec::new(),
            beats_per_chord: 0,
            bass: true,
        }
    }
}

/// A rendered backing loop. Frame 0 is the downbeat of bar 0 (and of every `beats`-beat cycle after it).
pub struct BackingClip {
    pub rate: u32,
    pub bpm: f64,
    pub beats_per_bar: u32,
    /// The loop's length in beats.
    pub beats: u32,
    /// Interleaved stereo.
    pub samples: Vec<f32>,
}

impl BackingClip {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
}

/// One pad note into the stereo buffer `out`: `midi` from `start` for `dur` seconds (then a release).
fn pad_note(out: &mut [f32], rate: u32, midi: f64, start: f64, dur: f64, gain: f32) {
    let r = rate as f64;
    let attack = 0.12f64.min(dur * 0.5);
    let release = 0.45;
    let s0 = (start * r) as usize;
    let n = ((dur + release) * r) as usize;
    let frames = out.len() / 2;
    let detune = [(-7.0f64, 0.8f32, 0.2f32), (0.0, 1.0, 0.5), (6.0, 0.8, 0.8)];
    for &(cents, amp, pan) in &detune {
        let f = midi_hz(midi + cents / 100.0);
        let w = std::f64::consts::TAU * f / r;
        let (gl, gr) = ((1.0 - pan) * amp * gain, pan * amp * gain);
        for i in 0..n {
            let k = s0 + i;
            if k >= frames {
                break;
            }
            let t = i as f64 / r;
            let env = if t < attack {
                let x = t / attack;
                x * x * (3.0 - 2.0 * x)
            } else if t < dur {
                1.0
            } else {
                let x = ((t - dur) / release).min(1.0);
                (1.0 - x) * (1.0 - x)
            };
            let (s1, c1) = (w * i as f64).sin_cos();
            // soft harmonics, falling fast: a warm, flute-ish pad (2nd to 4th from the fundamental's sine and
            // cosine: sin 2x = 2 sin x cos x, sin 3x = sin x (3 - 4 sin² x), sin 4x = 2 sin 2x cos 2x)
            let s2 = 2.0 * s1 * c1;
            let c2 = 1.0 - 2.0 * s1 * s1;
            let s3 = s1 * (3.0 - 4.0 * s1 * s1);
            let s4 = 2.0 * s2 * c2;
            let v = s1 + 0.35 * s2 + 0.15 * s3 + 0.06 * s4;
            let v = (v * env) as f32;
            out[2 * k] += v * gl;
            out[2 * k + 1] += v * gr;
        }
    }
}

/// One plucked bass note (the tab synth's string), faded out at the next note.
fn bass_note(out: &mut [f32], rate: u32, midi: f64, start: f64, dur: f64, gain: f32, rng: &mut Rng) {
    let x = string_tone(midi_hz(midi), dur, rate, None, 1.6, 0.3, rng);
    let s0 = (start * rate as f64) as usize;
    let fade = (rate as usize / 50).min(x.len());
    let frames = out.len() / 2;
    for (i, v) in x.iter().enumerate() {
        let k = s0 + i;
        if k >= frames {
            break;
        }
        let left = x.len() - i;
        let g = if left < fade {
            left as f32 / fade as f32
        } else {
            1.0
        } * (i as f32 / (rate as f32 * 0.002)).min(1.0);
        let v = *v as f32 * gain * g;
        out[2 * k] += v;
        out[2 * k + 1] += v;
    }
}

/// The root's MIDI note in the octave from E2 (the bass) or from the middle of the guitar's range (the pad).
fn bass_midi(pc: u8) -> f64 {
    let m = 36 + pc as i32;
    (if m < 40 { m + 12 } else { m }) as f64
}

/// Render the backing at `bpm` in a bar of `beats_per_bar`.
pub fn render(spec: &BackingSpec, bpm: f64, beats_per_bar: u32, rate: u32) -> BackingClip {
    let bpm = bpm.clamp(20.0, 400.0);
    let bpb = beats_per_bar.max(1);
    let spb = 60.0 / bpm;
    let mut rng = Rng::new(0xbac1);
    let (beats, samples) = match spec.kind {
        BackingKind::Drone => {
            let beats = bpb * 2;
            let len = ((beats as f64 * spb) * rate as f64).round() as usize;
            let x = ((rate as f64 * 0.5) as usize).min(len / 2).max(1);
            let total = (len + x) as f64 / rate as f64;
            let mut r = vec![0.0f32; (len + x) * 2];
            let root = 45.0 + ((spec.key as i32 + 12 - 9) % 12) as f64; // A2..G#3
            let fifth = if spec.scale.intervals().contains(&7) {
                7.0
            } else {
                // a scale without a perfect fifth (Locrian-like): the root's octave alone
                12.0
            };
            for (m, g) in [
                (root, 0.22),
                (root + fifth, 0.16),
                (root + 12.0, 0.14),
                (root - 12.0, 0.18),
            ] {
                pad_note(&mut r, rate, m, 0.0, total + 1.0, g);
            }
            // fold: the held note's end is crossfaded into the start (equal power)
            let mut clip = r[..len * 2].to_vec();
            for i in 0..x {
                let w = (i as f32 / x as f32 * std::f32::consts::FRAC_PI_2).sin();
                let v = (1.0 - w * w).sqrt();
                for c in 0..2 {
                    clip[2 * i + c] = r[2 * i + c] * w + r[2 * (len + i) + c] * v;
                }
            }
            (beats, clip)
        }
        BackingKind::Chords => {
            let chords: Vec<Chord> = if spec.chords.is_empty() {
                vec![Chord::new(
                    spec.key,
                    if spec.scale.is_minor() {
                        Quality::Minor
                    } else {
                        Quality::Major
                    },
                )]
            } else {
                spec.chords.clone()
            };
            let bpc = if spec.beats_per_chord == 0 {
                bpb
            } else {
                spec.beats_per_chord
            };
            let beats = bpc * chords.len() as u32;
            let len = ((beats as f64 * spb) * rate as f64).round() as usize;
            let cycle = len as f64 / rate as f64;
            let mut r = vec![0.0f32; len * 2 * 2];
            for pass in 0..2 {
                for (k, ch) in chords.iter().enumerate() {
                    let start = pass as f64 * cycle + (k as u32 * bpc) as f64 * spb;
                    let dur = bpc as f64 * spb;
                    for m in ch.voicing(52) {
                        pad_note(&mut r, rate, m as f64, start, dur - 0.05, 0.16);
                    }
                    if spec.bass {
                        // the root on beat one and halfway through (two-beat chords: once)
                        let hits: &[u32] = if bpc >= 4 { &[0, bpc / 2] } else { &[0] };
                        for (j, &h) in hits.iter().enumerate() {
                            let next = hits.get(j + 1).copied().unwrap_or(bpc);
                            let d = (next - h) as f64 * spb;
                            bass_note(
                                &mut r,
                                rate,
                                bass_midi(ch.root),
                                start + h as f64 * spb,
                                d,
                                0.55,
                                &mut rng,
                            );
                        }
                    }
                }
            }
            (beats, r[len * 2..].to_vec())
        }
    };
    // a steady level whatever the chords: peak at 0.5
    let peak = samples.iter().fold(1e-9f32, |m, v| m.max(v.abs()));
    let g = 0.5 / peak;
    BackingClip {
        rate,
        bpm,
        beats_per_bar: bpb,
        beats,
        samples: samples.into_iter().map(|v| v * g).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::theory::parse_progression;
    use super::*;

    /// The strongest frequency in a stretch (a coarse scan, enough to tell notes apart).
    fn pitch_near(x: &[f32], rate: u32, lo: f64, hi: f64) -> f64 {
        let mut best = (0.0, lo);
        let mut f = lo;
        while f < hi {
            let w = std::f64::consts::TAU * f / rate as f64;
            let (mut c, mut s) = (0.0f64, 0.0f64);
            for (i, v) in x.iter().enumerate() {
                c += *v as f64 * (w * i as f64).cos();
                s += *v as f64 * (w * i as f64).sin();
            }
            let p = c * c + s * s;
            if p > best.0 {
                best = (p, f);
            }
            f *= 1.0 + 1.0 / 400.0;
        }
        best.1
    }

    #[test]
    fn a_chord_loop_plays_its_chords_at_the_tempo() {
        let a = 9;
        let spec = BackingSpec {
            kind: BackingKind::Chords,
            key: a,
            scale: Scale::NaturalMinor,
            chords: parse_progression("Am F C G", a, Scale::NaturalMinor).unwrap(),
            beats_per_chord: 0,
            bass: true,
        };
        let rate = 48000;
        let clip = render(&spec, 120.0, 4, rate);
        assert_eq!(clip.beats, 16);
        assert_eq!(clip.frames(), 16 * 24000, "4 bars of 4 at 120 bpm");
        // the bass on each bar's beat 3 (away from the chord change): A2, F2, C3, G2
        let mono: Vec<f32> = clip.samples.chunks(2).map(|f| f[0] + f[1]).collect();
        for (bar, want) in [(0usize, 45.0), (1, 41.0), (2, 48.0), (3, 43.0)] {
            let at = bar * 96000 + 48000 + 2400;
            let f = pitch_near(&mono[at..at + 9600], rate, 70.0, 140.0);
            let m = 69.0 + 12.0 * (f / 440.0).log2();
            assert!(
                (m - want).abs() < 0.3,
                "bar {bar}: {f:.1} Hz = midi {m:.2}, want {want}"
            );
        }
        // the loop point is no step like a cut: no bigger than a bass note's pluck inside the loop
        let n = clip.frames();
        let step = (clip.samples[0] - clip.samples[2 * (n - 1)]).abs();
        let inside = (1..n)
            .map(|i| (clip.samples[2 * i] - clip.samples[2 * (i - 1)]).abs())
            .fold(0.0f32, f32::max);
        assert!(
            step <= inside * 1.01,
            "step at the loop point {step}, inside {inside}"
        );
    }

    #[test]
    fn a_drone_holds_its_root_and_loops_smoothly() {
        let spec = BackingSpec {
            key: 4,
            scale: Scale::Major,
            ..Default::default()
        };
        let clip = render(&spec, 90.0, 4, 48000);
        assert_eq!(clip.beats, 8);
        let mono: Vec<f32> = clip.samples.chunks(2).map(|f| f[0] + f[1]).collect();
        // E2 an octave under the E3 root
        let f = pitch_near(&mono[24000..72000], 48000, 75.0, 90.0);
        assert!((f - 82.4).abs() < 1.0, "{f}");
        let n = clip.frames();
        let step = (clip.samples[0] - clip.samples[2 * (n - 1)]).abs();
        assert!(step < 0.05, "step at the loop point {step}");
        let peak = clip.samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!((peak - 0.5).abs() < 1e-3);
    }
}
