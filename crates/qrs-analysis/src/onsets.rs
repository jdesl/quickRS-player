//! Plucks in a whole recording (offline, new in quickRS, P18.3): for the take analysis, which has the take in
//! hand and can afford a better detector than the live one (rt/onset.rs, RMS hops of 256 samples).
//!
//! - Novelty: spectral flux of the log magnitude (21 ms Hann frames every 2.7 ms, 50 Hz-6 kHz), each bin compared
//!   with the loudest of its neighbours a frame before (as SuperFlux: a bend or vibrato moving between bins doesn't
//!   count). A pick attack, a new pitch (a hammer-on) and a repeated note all show up; a level change alone barely
//!   does, as everything is in log magnitude.
//! - Peaks: the highest point within 16 ms either way, above the local mean by a share of the take's strongest
//!   flux, at least 30 ms after the previous pluck, and where the signal is above the take's noise floor.
//! - Timing: the flux peak lags the attack (a frame sees the pluck gradually), so each pluck is moved to where the
//!   fine energy envelope (64-sample blocks every 32) rises; with no rise (legato), a fixed lag is taken off.

use realfft::RealFftPlanner;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TakeOnset {
    /// Sample index in the take.
    pub sample: usize,
    /// Its flux (relative to the take's strongest, 0..1).
    pub strength: f32,
    /// Whether the energy rose there (a pick attack) rather than only the spectrum changing (legato).
    pub attack: bool,
}

/// Frame length for the flux at this rate (about 21 ms, a power of two).
fn frame_len(rate: u32) -> usize {
    ((rate as f64 * 0.0213) as usize).next_power_of_two().max(256)
}

/// Every pluck in `x` (mono), in order.
pub fn detect(x: &[f32], rate: u32) -> Vec<TakeOnset> {
    let n = frame_len(rate);
    let hop = n / 8;
    if x.len() < n + hop {
        return vec![];
    }
    let frames = (x.len() - n) / hop + 1;
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let win: Vec<f32> = (0..n)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
        .collect();
    let binhz = rate as f32 / n as f32;
    let lo = ((50.0 / binhz) as usize).max(1);
    let hi = ((6000.0 / binhz) as usize).min(n / 2);
    let mut input = fft.make_input_vec();
    let mut spec = fft.make_output_vec();
    let mut scratch = fft.make_scratch_vec();
    let mut prev = vec![0.0f32; hi + 1];
    let mut cur = vec![0.0f32; hi + 1];
    let mut flux = vec![0.0f32; frames];
    let mut rms = vec![0.0f32; frames];
    for f in 0..frames {
        let s = f * hop;
        let frame = &x[s..s + n];
        let mut e = 0.0f32;
        for i in 0..n {
            input[i] = frame[i] * win[i];
            e += frame[i] * frame[i];
        }
        rms[f] = (e / n as f32).sqrt();
        let _ = fft.process_with_scratch(&mut input, &mut spec, &mut scratch);
        for k in lo.saturating_sub(1)..=hi {
            cur[k] = (1.0 + spec[k].norm()).ln();
        }
        if f > 0 {
            let mut d = 0.0f32;
            for k in lo..hi {
                let before = prev[k - 1].max(prev[k]).max(prev[k + 1]);
                d += (cur[k] - before).max(0.0);
            }
            flux[f] = d;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let mut sorted_flux = flux.clone();
    sorted_flux.sort_by(|a, b| a.total_cmp(b));
    let top = sorted_flux[((frames as f64 * 0.995) as usize).min(frames - 1)].max(1e-6);
    let mut sorted_rms = rms.clone();
    sorted_rms.sort_by(|a, b| a.total_cmp(b));
    // the noise floor: the quietest tenth of the take, unless the take is loud all through (a rhythm part that
    // never stops, or a quiet stretch), where 3% of its loud level stands in
    let floor = sorted_rms[frames / 10];
    let loud = sorted_rms[frames * 95 / 100];
    let gate = (floor * 3.0).min(0.03 * loud).max(2e-4);
    let w_peak = ((0.016 * rate as f64) as usize / hop).max(2);
    let w_mean = ((0.06 * rate as f64) as usize / hop).max(4);
    let w_recent = ((0.08 * rate as f64) as usize / hop).max(w_peak + 1);
    let delta = 0.08 * top;
    let min_gap = (0.03 * rate as f64) as usize;
    let env = Envelope::new(x);
    // the log spectrum of the n samples from `start` (bins lo-1..=hi)
    let mut logspec = |start: usize| -> Vec<f32> {
        let mut v = vec![0.0f32; hi + 1];
        if start + n > x.len() {
            return v;
        }
        for i in 0..n {
            input[i] = x[start + i] * win[i];
        }
        let _ = fft.process_with_scratch(&mut input, &mut spec, &mut scratch);
        for (k, o) in v.iter_mut().enumerate().skip(lo.saturating_sub(1)) {
            *o = (1.0 + spec[k].norm()).ln();
        }
        v
    };
    let mut out: Vec<TakeOnset> = Vec::new();
    for f in 1..frames {
        let v = flux[f];
        if v < delta {
            continue;
        }
        let a = f.saturating_sub(w_peak);
        let b = (f + w_peak + 1).min(frames);
        if flux[a..b].iter().any(|&u| u > v) || flux[a..f].contains(&v) {
            continue; // not the (first) highest point around
        }
        let ma = f.saturating_sub(w_mean);
        let local = flux[ma..f].iter().sum::<f32>() / (f - ma).max(1) as f32;
        if v < local + delta {
            continue;
        }
        // where the flux first got to half this peak (the pluck is near that frame's end)
        let mut g = f;
        while g > a && flux[g - 1] >= 0.5 * v {
            g -= 1;
        }
        let (sample, rise) = env.refine(g * hop, n);
        let attack = rise >= 3.0;
        // it must stand out from the flux of the last 80 ms (a chord's partials beating make a ripple of small
        // peaks), unless the energy clearly jumped
        let ra = f.saturating_sub(w_recent);
        let rb = f.saturating_sub(w_peak).max(ra);
        let recent = flux[ra..rb].iter().copied().fold(0.0f32, f32::max);
        if v < 1.25 * recent && rise < 6.0 {
            continue;
        }
        // the signal must be above the noise floor just after, and not dying away (a note stopped short makes a
        // click of flux too)
        let fa = (sample / hop).min(frames - 1);
        let after = rms[fa..(fa + 6).min(frames)]
            .iter()
            .copied()
            .fold(0.0f32, f32::max);
        let before = rms[(sample.saturating_sub(n) / hop).saturating_sub(2).min(frames - 1)];
        if after < gate || after < 0.6 * before {
            continue;
        }
        // no attack: a new pitch must stay (a note stopped short leaves only a click)
        if !attack && sample >= n + hop {
            let (pre, post) = (logspec(sample - n - hop), logspec(sample + 2 * hop));
            let mut d = 0.0f32;
            for k in lo..hi {
                d += (post[k] - pre[k - 1].max(pre[k]).max(pre[k + 1])).max(0.0);
            }
            if d < 0.3 * v {
                continue;
            }
        }
        let strength = v / top;
        if let Some(last) = out.last_mut()
            && sample < last.sample + min_gap
        {
            if strength > last.strength {
                last.strength = strength;
            }
            continue;
        }
        out.push(TakeOnset {
            sample,
            strength,
            attack,
        });
    }
    out
}

/// Fine energy envelope in dB: blocks of 64 samples every 32.
struct Envelope {
    db: Vec<f32>,
}

const BLOCK: usize = 64;
const STEP: usize = 32;
/// Blocks looked back over for the loudest point before a rise, to time a pluck (~27 ms at 48 kHz: a period of a
/// bass's low E, so a ringing note's waveform doesn't look like a rise).
const BACK: usize = 40;
/// ... and to call it an attack at all (50 ms: a chord's waveform and its beating don't look like one).
const BACK_ATTACK: usize = 75;

impl Envelope {
    fn new(x: &[f32]) -> Envelope {
        let blocks = if x.len() < BLOCK {
            0
        } else {
            (x.len() - BLOCK) / STEP + 1
        };
        let db = (0..blocks)
            .map(|k| {
                let s = &x[k * STEP..k * STEP + BLOCK];
                let e = s.iter().map(|v| v * v).sum::<f32>() / BLOCK as f32;
                10.0 * (e + 1e-12).log10()
            })
            .collect();
        Envelope { db }
    }

    /// The rise of block k over the loudest block in the `back` blocks before it (dB).
    fn rise_over(&self, k: usize, back: usize) -> f32 {
        let a = k.saturating_sub(back);
        if a >= k {
            return 0.0;
        }
        let m = self.db[a..k - 1.min(k - a - 1)]
            .iter()
            .copied()
            .fold(f32::MIN, f32::max);
        self.db[k] - m
    }

    fn rise(&self, k: usize) -> f32 {
        self.rise_over(k, BACK)
    }

    /// The pluck around the end of the frame starting at `start` (n samples long): (sample, how many dB the
    /// energy rose there). Without a 3 dB rise (legato), the frame's last quarter.
    fn refine(&self, start: usize, n: usize) -> (usize, f32) {
        let fallback = (start + n * 3 / 4, 0.0);
        if self.db.is_empty() {
            return fallback;
        }
        let k0 = (start + n / 4) / STEP;
        let k1 = ((start + n + n / 2) / STEP).min(self.db.len());
        let mut best = (0.0f32, k0);
        for k in k0.max(1)..k1 {
            let r = self.rise(k);
            if r > best.0 + 0.5 {
                best = (r, k);
            }
        }
        let attack = self.rise_over(best.1, BACK_ATTACK).min(best.0);
        if best.0 < 3.0 || attack < 3.0 {
            return (fallback.0, attack);
        }
        // the first block that already holds half the rise
        let mut k = best.1;
        while k > k0.max(1) && self.rise(k - 1) >= 0.5 * best.0 {
            k -= 1;
        }
        (k * STEP + STEP, attack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plucked string: harmonics decaying, starting at `at` seconds.
    pub fn pluck(x: &mut [f32], rate: u32, at: f64, hz: f64, amp: f32, dur: f64) {
        let s = (at * rate as f64) as usize;
        let n = (dur * rate as f64) as usize;
        for i in 0..n {
            if s + i >= x.len() {
                break;
            }
            let t = i as f64 / rate as f64;
            let w = std::f64::consts::TAU * hz * t;
            let v = (w.sin() + 0.5 * (2.0 * w).sin() + 0.3 * (3.0 * w).sin() + 0.2 * (4.0 * w).sin())
                * (-t * 4.0).exp()
                * (1.0 - (-t * 2000.0).exp());
            x[s + i] += amp * v as f32;
        }
    }

    #[test]
    fn finds_plucks_to_a_few_ms() {
        let rate = 48000;
        let mut x = vec![0.0f32; rate as usize * 4];
        let times = [0.3, 0.55, 0.8, 1.2, 1.35, 2.0, 2.9];
        let hz = [110.0, 146.8, 196.0, 110.0, 110.0, 329.6, 82.4];
        // (the note at 1.2 is on the string plucked again at 1.35, which stops it)
        let dur = [0.6, 0.6, 0.6, 0.15, 0.6, 0.6, 0.6];
        for i in 0..times.len() {
            pluck(&mut x, rate, times[i], hz[i], 0.3, dur[i]);
        }
        let on = detect(&x, rate);
        assert_eq!(on.len(), times.len(), "{on:?}");
        for (o, t) in on.iter().zip(times) {
            let err = o.sample as f64 / rate as f64 - t;
            assert!(err.abs() < 0.004, "{t}: {err}");
        }
        assert!(on[0].attack && on[5].attack);
    }

    #[test]
    fn quiet_notes_and_noise() {
        let rate = 48000;
        let mut x = vec![0.0f32; rate as usize * 3];
        let mut seed: u32 = 1;
        for v in &mut x {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *v = ((seed >> 8) as f32 / 16777216.0 - 0.5) * 0.002; // hiss
        }
        pluck(&mut x, rate, 0.5, 196.0, 0.4, 0.5);
        pluck(&mut x, rate, 1.2, 220.0, 0.03, 0.5); // 22 dB quieter
        pluck(&mut x, rate, 2.0, 246.9, 0.4, 0.5);
        let on = detect(&x, rate);
        let ts: Vec<f64> = on.iter().map(|o| o.sample as f64 / rate as f64).collect();
        assert_eq!(on.len(), 3, "{ts:?}");
        assert!((ts[1] - 1.2).abs() < 0.006, "{ts:?}");
    }

    #[test]
    fn a_new_pitch_without_an_attack_is_found() {
        // a hammer-on: the note changes pitch with no new attack
        let rate = 48000;
        let mut x = vec![0.0f32; rate as usize * 2];
        let s = (0.5 * rate as f64) as usize;
        let mut phase = 0.0f64;
        for i in 0..rate as usize {
            let t = i as f64 / rate as f64;
            let hz = if t < 0.4 { 196.0 } else { 220.0 };
            phase += std::f64::consts::TAU * hz / rate as f64;
            x[s + i] = (0.3 * (phase.sin() + 0.4 * (2.0 * phase).sin()) * (-t * 1.5).exp()) as f32;
        }
        let on = detect(&x, rate);
        let ts: Vec<f64> = on.iter().map(|o| o.sample as f64 / rate as f64).collect();
        assert_eq!(on.len(), 2, "{ts:?}");
        assert!((ts[1] - 0.9).abs() < 0.02, "{ts:?}");
    }
}
