//! Slow-down (and speed-up) without changing pitch, and pitch shift without changing speed (P20.3), with
//! Signalsmith Stretch (MIT, in-process; DECISIONS D4). Replaces rsplay/stretch.py's Rubber Band pieces with the
//! same scheme:
//!
//! The song is cut into pieces of `CHUNK` seconds of *output* that worker threads stretch, starting with the piece
//! under the playhead and working outwards, so playback resumes about a second after a speed change. Each piece is
//! stretched with `PAD` seconds of context on both sides; neighbouring pieces overlap by 2 × `XFADE` and are
//! cross-faded with equal-power curves (the two stretches aren't phase-aligned). The output buffer is marked
//! ready in small slots as each part becomes final, and the audio thread plays whatever is ready (it waits,
//! silently, where it isn't yet).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

use parking_lot::Mutex;

use crate::decode::SongBuffer;
use crate::rt::StretchBuffer;

/// Seconds of output per piece.
pub const CHUNK: f64 = 20.0;
/// Seconds of input context on each side of a piece.
pub const PAD: f64 = 1.0;
/// Half the cross-fade between pieces (seconds of output).
pub const XFADE: f64 = 0.1;
/// Readiness granularity (seconds of output).
pub const SLOT: f64 = 0.1;

pub const MIN_SPEED: f64 = 0.25;
pub const MAX_SPEED: f64 = 1.5;
/// Pitch shift range (semitones).
pub const MAX_SEMITONES: i32 = 12;

/// A stretcher for stereo at `rate`, transposed by `semitones`.
/// The pitch shifter's analysis block and hop (seconds). The library's default (0.12 / 0.03) leaves a held tone up to
/// 8 cents off after a shift; 0.2 / 0.05 measured within 6 cents at -12 and 0-3 cents for -5..+12 (pure tones).
const SHIFT_BLOCK: f32 = 0.2;
const SHIFT_HOP: f32 = 0.05;

fn stretcher(rate: u32, semitones: f32) -> signalsmith_stretch::Stretch {
    let mut s = if semitones != 0.0 {
        let r = rate as f32;
        signalsmith_stretch::Stretch::new(2, (r * SHIFT_BLOCK) as usize, (r * SHIFT_HOP) as usize)
    } else {
        signalsmith_stretch::Stretch::preset_default(2, rate)
    };
    transpose(&mut s, rate, semitones);
    s
}

fn transpose(s: &mut signalsmith_stretch::Stretch, rate: u32, semitones: f32) {
    if semitones != 0.0 {
        // above ~8 kHz the shifter stops keeping partials tonal (the library's advice for music)
        s.set_transpose_factor_semitones(semitones, Some(8000.0 / rate as f32));
    }
}

/// Stretch a short clip in one go (previews, the selection synth). `speed` < 1 slows down; `semitones` shifts the
/// pitch (0: unchanged).
pub fn stretch_clip(stereo: &[f32], rate: u32, speed: f64, semitones: f32) -> Vec<f32> {
    if ((speed - 1.0).abs() < 1e-6 && semitones == 0.0) || stereo.is_empty() {
        return stereo.to_vec();
    }
    let frames = stereo.len() / 2;
    let out_frames = (frames as f64 / speed).round() as usize;
    let mut out = vec![0.0f32; out_frames * 2];
    stretcher(rate, semitones).exact(stereo, &mut out);
    out
}

struct Zone {
    /// From the piece on the left (fading out) and on the right (fading in).
    left: Option<Vec<f32>>,
    right: Option<Vec<f32>>,
}

struct Plan {
    rate: u32,
    speed: f64,
    in_frames: usize,
    out_frames: usize,
    chunk: usize,
    xfade: usize,
    pad_in: usize,
    slot: usize,
    pieces: usize,
}

/// A song being stretched to one speed (and pitch) in the background.
pub struct StretchJob {
    pub speed: f64,
    pub semitones: f32,
    pub buffer: Arc<StretchBuffer>,
    cancel: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl StretchJob {
    /// Start stretching `song` to `speed`, beginning with the piece that plays song time `from`.
    pub fn start(song: Arc<SongBuffer>, speed: f64, from: f64) -> StretchJob {
        Self::start_shifted(song, speed, 0.0, from)
    }

    /// As `start`, also shifting the pitch by `semitones` (speed 1 with a shift is a pitch shift alone).
    pub fn start_shifted(song: Arc<SongBuffer>, speed: f64, semitones: f32, from: f64) -> StretchJob {
        let semitones = semitones.clamp(-(MAX_SEMITONES as f32), MAX_SEMITONES as f32);
        let speed = speed.clamp(MIN_SPEED, MAX_SPEED);
        let rate = song.rate;
        let in_frames = song.frames();
        let out_frames = (in_frames as f64 / speed).ceil() as usize;
        let chunk = ((CHUNK * rate as f64) as usize).max(rate as usize);
        let slot = ((SLOT * rate as f64) as usize).max(64);
        let plan = Arc::new(Plan {
            rate,
            speed,
            in_frames,
            out_frames,
            chunk,
            xfade: (XFADE * rate as f64) as usize,
            pad_in: (PAD * rate as f64) as usize,
            slot,
            pieces: out_frames.div_ceil(chunk).max(1),
        });
        let buffer = Arc::new(StretchBuffer::new(rate, speed, out_frames, slot));
        // order: the piece under the playhead, then outwards (ahead first)
        let first = ((from.max(0.0) / speed * rate as f64) as usize / chunk).min(plan.pieces - 1);
        let mut order = vec![first];
        for d in 1..plan.pieces {
            if first + d < plan.pieces {
                order.push(first + d);
            }
            if d <= first {
                order.push(first - d);
            }
        }
        let order = Arc::new(order);
        let next = Arc::new(AtomicUsize::new(0));
        let zones: Arc<Mutex<HashMap<usize, Zone>>> = Arc::new(Mutex::new(HashMap::new()));
        let cancel = Arc::new(AtomicBool::new(false));
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .saturating_sub(1)
            .clamp(1, 4);
        let workers = (0..threads)
            .map(|_| {
                let (song, plan, buffer, order, next, zones, cancel) = (
                    song.clone(),
                    plan.clone(),
                    buffer.clone(),
                    order.clone(),
                    next.clone(),
                    zones.clone(),
                    cancel.clone(),
                );
                std::thread::Builder::new()
                    .name("qrs-stretch".into())
                    .spawn(move || {
                        let mut stretch = stretcher(plan.rate, semitones);
                        loop {
                            if cancel.load(Ordering::Acquire) {
                                return;
                            }
                            let i = next.fetch_add(1, Ordering::AcqRel);
                            let Some(&piece) = order.get(i) else { return };
                            stretch.reset();
                            transpose(&mut stretch, plan.rate, semitones);
                            do_piece(&song, &plan, &buffer, &zones, &mut stretch, piece);
                        }
                    })
                    .expect("spawn stretch worker")
            })
            .collect();
        StretchJob {
            speed,
            semitones,
            buffer,
            cancel,
            workers,
        }
    }

    pub fn progress(&self) -> f64 {
        self.buffer.progress()
    }

    pub fn done(&self) -> bool {
        self.progress() >= 1.0
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    /// Whether the audio from song time `t` on (a couple of seconds of it) is ready to play.
    pub fn ready_at(&self, t: f64) -> bool {
        let f = (t.max(0.0) / self.speed * self.buffer.rate as f64) as usize;
        self.buffer.is_ready(f, f + 2 * self.buffer.rate as usize)
    }
}

/// Cancelled, the workers stop after the piece they're on (up to ~0.5 s): they're let go rather than waited for,
/// since a job is dropped on the UI thread (a speed change) and they hold their own references to all they use.
impl Drop for StretchJob {
    fn drop(&mut self) {
        self.cancel();
        self.workers.clear(); // (detached)
    }
}

/// Mark every slot fully inside [a, b) ready.
fn mark(buffer: &StretchBuffer, plan: &Plan, a: usize, b: usize) {
    let first = a.div_ceil(plan.slot);
    let last = if b >= plan.out_frames {
        buffer.chunks()
    } else {
        b / plan.slot
    };
    for s in first..last {
        buffer.mark_ready(s);
    }
}

fn do_piece(
    song: &SongBuffer,
    plan: &Plan,
    buffer: &StretchBuffer,
    zones: &Mutex<HashMap<usize, Zone>>,
    stretch: &mut signalsmith_stretch::Stretch,
    piece: usize,
) {
    let (chunk, x) = (plan.chunk, plan.xfade);
    // output range this piece is responsible for, including its cross-fade halves
    let out_a = (piece * chunk).saturating_sub(x);
    let out_b = ((piece + 1) * chunk + x).min(plan.out_frames);
    // input range with context
    let in_a = ((out_a as f64 * plan.speed) as usize).saturating_sub(plan.pad_in);
    let in_b = (((out_b as f64) * plan.speed).ceil() as usize + plan.pad_in).min(plan.in_frames);
    if in_b <= in_a {
        mark(buffer, plan, out_a, out_b);
        return;
    }
    let input: Vec<f32> = song.samples[2 * in_a..2 * in_b]
        .iter()
        .map(|&s| s as f32 / 32768.0)
        .collect();
    let n_out = ((in_b - in_a) as f64 / plan.speed).round() as usize;
    let mut out = vec![0.0f32; n_out * 2];
    stretch.exact(&input, &mut out);
    // output frame f (absolute) is at out[(f - base)]
    let base = (in_a as f64 / plan.speed).round() as usize;
    let at = |f: usize| -> (f32, f32) {
        let i = f.saturating_sub(base);
        if i < n_out {
            (out[2 * i], out[2 * i + 1])
        } else {
            (0.0, 0.0)
        }
    };

    let left_boundary = piece * chunk; // zone around it unless piece == 0
    let right_boundary = (piece + 1) * chunk; // zone unless it's past the end
    let own_a = if piece == 0 { 0 } else { left_boundary + x };
    let own_b = if right_boundary >= plan.out_frames {
        plan.out_frames
    } else {
        right_boundary - x
    };
    // the middle: ours alone
    let mut tmp = Vec::with_capacity((own_b.saturating_sub(own_a)) * 2);
    for f in own_a..own_b {
        let (l, r) = at(f);
        tmp.push(l);
        tmp.push(r);
    }
    buffer.write(own_a, &tmp);
    mark(buffer, plan, own_a, own_b);

    // the zones: both neighbours' audio kept raw; when both are in, they're cross-faded with weights that keep
    // the level constant whatever their correlation (equal power for unrelated audio, equal gain when the two
    // stretches came out in phase, e.g. on a held note)
    let contribute = |boundary: usize, from_left: bool| {
        let (za, zb) = (boundary - x, (boundary + x).min(plan.out_frames));
        let mut part = Vec::with_capacity((zb - za) * 2);
        for f in za..zb {
            let (l, r) = at(f);
            part.push(l);
            part.push(r);
        }
        let mut z = zones.lock();
        let entry = z.entry(boundary).or_insert(Zone {
            left: None,
            right: None,
        });
        if from_left {
            entry.left = Some(part);
        } else {
            entry.right = Some(part);
        }
        if let (Some(a), Some(b)) = (&entry.left, &entry.right) {
            buffer.write(za, &crossfade(a, b));
            mark(buffer, plan, za, zb);
            z.remove(&boundary);
        }
    };
    if piece > 0 {
        contribute(left_boundary, false);
    }
    if right_boundary < plan.out_frames {
        contribute(right_boundary, true);
    }
}

/// Cross-fade `a` (fading out) into `b` (fading in), interleaved stereo of equal length, keeping the level
/// constant: weights g_out, g_in on a quarter-sine, normalised by sqrt(g_out² + g_in² + 2ρ g_out g_in) where ρ is
/// the two signals' correlation.
pub fn crossfade(a: &[f32], b: &[f32]) -> Vec<f32> {
    let n = a.len().min(b.len());
    let (mut ab, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        ab += a[i] as f64 * b[i] as f64;
        aa += a[i] as f64 * a[i] as f64;
        bb += b[i] as f64 * b[i] as f64;
    }
    let rho = if aa > 0.0 && bb > 0.0 {
        (ab / (aa * bb).sqrt()).clamp(-0.99, 1.0)
    } else {
        0.0
    } as f32;
    let frames = n / 2;
    let mut out = Vec::with_capacity(n);
    for f in 0..frames {
        let t = (f as f32 + 0.5) / frames.max(1) as f32;
        let (go, gi) = (
            (t * std::f32::consts::FRAC_PI_2).cos(),
            (t * std::f32::consts::FRAC_PI_2).sin(),
        );
        let k = 1.0 / (go * go + gi * gi + 2.0 * rho * go * gi).max(1e-6).sqrt();
        for c in 0..2 {
            out.push((a[2 * f + c] * go + b[2 * f + c] * gi) * k);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f64) -> SongBuffer {
        let n = (48000.0 * seconds) as usize;
        let mut s = Vec::with_capacity(n * 2);
        for i in 0..n {
            let v = ((i as f64 * 330.0 / 48000.0 * std::f64::consts::TAU).sin() * 9000.0) as i16;
            s.push(v);
            s.push(v);
        }
        SongBuffer {
            rate: 48000,
            samples: Arc::new(s),
        }
    }

    #[test]
    fn half_speed_is_twice_as_long_and_complete() {
        let song = Arc::new(tone(50.0));
        let job = StretchJob::start(song, 0.5, 30.0);
        let t0 = std::time::Instant::now();
        while !job.done() && t0.elapsed().as_secs() < 120 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(job.done(), "progress {}", job.progress());
        let b = &job.buffer;
        assert_eq!(b.frames, 100 * 48000);
        // level is steady across a piece boundary (no dip or click from the cross-fade)
        let boundary = 40 * 48000;
        let rms = |a: usize, n: usize| -> f32 {
            ((a..a + n).map(|f| b.sample(2 * f).powi(2)).sum::<f32>() / n as f32).sqrt()
        };
        let before = rms(boundary - 9600, 4800);
        let across = rms(boundary - 2400, 4800);
        assert!(before > 0.1, "{before}");
        assert!((across / before - 1.0).abs() < 0.2, "{before} {across}");
    }

    #[test]
    fn crossfade_keeps_the_level_for_correlated_and_unrelated_audio() {
        let sine: Vec<f32> = (0..9600).map(|i| ((i / 2) as f32 * 0.05).sin()).collect();
        let same = crossfade(&sine, &sine);
        let peak = same.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!((peak - 1.0).abs() < 0.02, "{peak}");
        let noise = |seed: u32| -> Vec<f32> {
            let mut x = seed;
            (0..48000)
                .map(|_| {
                    x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                    (x >> 9) as f32 / 4194304.0 - 1.0
                })
                .collect()
        };
        let mixed = crossfade(&noise(1), &noise(2));
        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        let (r_in, r_mid) = (rms(&noise(1)), rms(&mixed[20000..28000]));
        assert!((r_mid / r_in - 1.0).abs() < 0.08, "{r_in} {r_mid}");
    }

    /// A speed change drops the running job on the UI thread: that mustn't wait for its workers' pieces.
    #[test]
    fn dropping_a_running_job_does_not_wait_for_it() {
        let job = StretchJob::start(Arc::new(tone(50.0)), 0.5, 0.0);
        std::thread::sleep(std::time::Duration::from_millis(30)); // (the workers are mid-piece)
        let t0 = std::time::Instant::now();
        drop(job);
        assert!(t0.elapsed().as_millis() < 50, "{:?}", t0.elapsed());
    }

    /// Frequency of a steady tone from its upward zero crossings (interpolated), over interleaved stereo frames.
    fn freq(at: impl Fn(usize) -> f32, a: usize, b: usize, rate: f64) -> f64 {
        let mut cross = Vec::new();
        for f in a..b {
            let (x0, x1) = (at(f), at(f + 1));
            if x0 < 0.0 && x1 >= 0.0 {
                cross.push(f as f64 + (x0 / (x0 - x1)) as f64);
            }
        }
        let n = cross.len() - 1;
        n as f64 / ((cross[n] - cross[0]) / rate)
    }

    #[test]
    fn pitch_shift_moves_a_tone_and_keeps_the_length() {
        // +7 semitones on a clip: 330 Hz -> 494.4 Hz, same length
        let x: Vec<f32> = (0..96000)
            .flat_map(|i| {
                let v = (i as f64 * 330.0 / 48000.0 * std::f64::consts::TAU).sin() as f32 * 0.3;
                [v, v]
            })
            .collect();
        let y = stretch_clip(&x, 48000, 1.0, 7.0);
        assert_eq!(y.len(), x.len());
        let f = freq(|i| y[2 * i], 20000, 76000, 48000.0);
        let want = 330.0 * 2f64.powf(7.0 / 12.0);
        assert!((1200.0 * (f / want).log2()).abs() < 4.0, "{f} vs {want}");

        // the whole-song job at speed 1, down 5 semitones, across a piece boundary
        let job = StretchJob::start_shifted(Arc::new(tone(25.0)), 1.0, -5.0, 0.0);
        let t0 = std::time::Instant::now();
        while !job.done() && t0.elapsed().as_secs() < 120 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(job.done());
        let b = &job.buffer;
        assert_eq!(b.frames, 25 * 48000, "length kept");
        assert!(job.ready_at(10.0));
        let want = 330.0 * 2f64.powf(-5.0 / 12.0);
        for (a, z) in [(48000, 5 * 48000), (19 * 48000, 21 * 48000)] {
            let f = freq(|i| b.sample(2 * i), a, z, 48000.0);
            assert!((1200.0 * (f / want).log2()).abs() < 4.0, "{f} vs {want}");
        }
        // every step of the range lands within a few cents (a pure tone is the hardest case)
        for st in [-12.0f32, -1.0, 1.0, 5.0, 12.0] {
            let y = stretch_clip(&x, 48000, 1.0, st);
            let f = freq(|i| y[2 * i], 20000, 76000, 48000.0);
            let cents = 1200.0 * (f / (330.0 * 2f64.powf(st as f64 / 12.0))).log2();
            assert!(cents.abs() < 8.0, "{st}: {f} ({cents} cents)");
        }
    }

    #[test]
    fn clip_stretch_length() {
        let x = vec![0.1f32; 48000 * 2];
        assert_eq!(
            stretch_clip(&x, 48000, 0.7, 0.0).len() / 2,
            (48000.0 / 0.7f64).round() as usize
        );
    }
}
