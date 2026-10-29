//! The synth guitar in place of the song's (P23.11, owner request; new in the Rust port): the lamp row's abalone
//! key (G) plays the part on screen (Lead, Rhythm, Bass, ...) on the app's synth along with the song.
//!
//! The whole part is rendered once on a worker thread (`qrs_audio::synth::render_part`, on the song's clock) and
//! then mixed into a new song buffer on another, which goes into the engine where the song is (`swap_song`, as a
//! new stem mix does). So play, pause, seek, loops, slow-down and transpose all work on it unchanged: they work on
//! the song buffer, and the synth is in it. Nothing new runs on the audio thread.
//!
//! - A stemmed feedpak with a stem for the part (a guitar stem for a guitar part, the bass stem for bass): that
//!   stem is left out of the mix and the synth takes its place at its loudness; the rest plays as the mixer has it.
//! - Otherwise (a full mix, or no matching stem): the song is turned down (`DUCK`) and the synth plays over it,
//!   a little under the song's own loudness, since the guitar can't be taken out of a mixdown.
//!
//! A part, difficulty or sync offset change renders again; until it's ready the version playing plays on and the
//! key says "preparing". Switching it off puts the song back as it was. config.json `song_synth` remembers it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use qrs_audio::decode::SongBuffer;
use qrs_audio::stems::{StemMix, StemSet, effective_gains};

use super::PlayerView;
use crate::state::AppState;

/// The song's level under the synth when its guitar can't be taken out.
pub const DUCK: f32 = 0.5;
/// The synth's loudness against the song's (its loud parts) when it plays over the full mix.
const OVER: f32 = 0.7;
/// The synth's loudness (RMS of its loud parts) when there's nothing to match it to (a song with no audio).
const ALONE: f32 = 0.12;

/// What a render is of: a new one is needed when any of it changes.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    part: String,
    mastery: i64,
    offset_ms: i64,
    notes: usize,
    frames: usize,
    rate: u32,
}

/// The song buffer in the engine: the render it has in it and the stem mix it was made from.
#[derive(Clone, Debug, PartialEq)]
struct Made {
    key: Key,
    mix_gen: u64,
}

/// How the synth sits in the song.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// In place of the part's stem.
    Stem,
    /// Over the whole song, turned down.
    Over,
}

/// What the key shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Off,
    /// Getting the synth ready (0..1).
    Preparing(f32),
    On,
}

struct Render {
    key: Key,
    cancel: Arc<AtomicBool>,
    progress: Arc<AtomicU32>,
    rx: Receiver<Vec<f32>>,
}

impl Drop for Render {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

#[derive(Default)]
pub struct SongSynth {
    pub on: bool,
    /// The song as it was loaded (a song without stems), put back when the synth goes off.
    orig: Option<Arc<SongBuffer>>,
    /// The part rendered (mono, on the song's clock).
    track: Option<(Key, Arc<Vec<f32>>)>,
    render: Option<Render>,
    mixing: Option<(Made, Receiver<SongBuffer>)>,
    /// What the engine's song has in it, when it has the synth.
    made: Option<Made>,
    /// Bumped when the stem mixer changes (the synth's version is mixed again).
    pub mix_gen: u64,
    /// Screenshots: shown as this far prepared (no engine, no render).
    pub shot: Option<f32>,
}

/// config.json's key for it.
const CONFIG_KEY: &str = "song_synth";

impl SongSynth {
    /// As config.json has it (off at first).
    pub fn from_config(cfg: &qrs_library::config::Config) -> SongSynth {
        SongSynth {
            on: cfg.bool(CONFIG_KEY, false),
            ..Default::default()
        }
    }

    /// Switch it, and remember that in config.json.
    fn set(&mut self, cfg: &mut qrs_library::config::Config, on: bool) {
        self.on = on;
        cfg.set(CONFIG_KEY, on);
    }
}

/// The stems that are the part: for bass the bass stem(s); for a guitar part the guitar stems, narrowed to the ones
/// named for it when there are several (a "Lead" part and a "lead guitar" stem). `stems` is (id, name) each.
pub fn part_stems(stems: &[(&str, &str)], part: &str, bass: bool) -> Vec<usize> {
    let names: Vec<String> = stems
        .iter()
        .map(|(id, name)| format!("{id} {name}").to_lowercase())
        .collect();
    let is = |n: &str, words: &[&str]| words.iter().any(|w| n.contains(w));
    let mut picked: Vec<usize> = (0..names.len())
        .filter(|&i| {
            if bass {
                is(&names[i], &["bass"])
            } else {
                is(&names[i], &["guitar", "gtr"])
            }
        })
        .collect();
    if !bass && picked.len() > 1 {
        let part = part.to_lowercase();
        for role in ["lead", "rhythm"] {
            if part.contains(role) {
                let narrowed: Vec<usize> = picked
                    .iter()
                    .copied()
                    .filter(|&i| names[i].contains(role))
                    .collect();
                if !narrowed.is_empty() {
                    picked = narrowed;
                }
            }
        }
    }
    picked
}

/// The RMS of the loud parts of `frames` frames of `at(i)` (each frame's level): the mean of quarter-second blocks
/// that are within 20 dB of the loudest, so silence and fade-outs don't drag it down. 0 for silence.
fn loud_rms(frames: usize, rate: u32, at: impl Fn(usize) -> f32) -> f32 {
    let block = ((rate / 4) as usize).max(1);
    let mut blocks = Vec::with_capacity(frames / block + 1);
    let mut a = 0;
    while a < frames {
        let b = (a + block).min(frames);
        let sum: f64 = (a..b).map(|i| (at(i) as f64).powi(2)).sum();
        blocks.push((sum / (b - a) as f64).sqrt() as f32);
        a = b;
    }
    let top = blocks.iter().copied().fold(0.0f32, f32::max);
    if top <= 0.0 {
        return 0.0;
    }
    let loud: Vec<f32> = blocks.into_iter().filter(|&r| r >= top * 0.1).collect();
    loud.iter().sum::<f32>() / loud.len() as f32
}

/// A stereo i16 buffer's frame level (the louder channel, full scale 1).
fn frame_level(s: &[i16], i: usize) -> f32 {
    let (l, r) = (s[2 * i] as f32, s[2 * i + 1] as f32);
    l.abs().max(r.abs()) / 32768.0
}

/// `base` (stereo i16) times `base_gain` with the synth (mono, full scale 1) times `synth_gain` on both sides, as
/// i16; turned down as a whole if the sum would clip.
pub fn combine(base: &[i16], base_gain: f32, synth: &[f32], synth_gain: f32) -> Vec<i16> {
    let frames = base.len() / 2;
    let at = |i: usize, c: usize| {
        base[2 * i + c] as f32 / 32768.0 * base_gain + synth.get(i).copied().unwrap_or(0.0) * synth_gain
    };
    let peak = (0..frames).fold(0.0f32, |m, i| m.max(at(i, 0).abs()).max(at(i, 1).abs()));
    let k = if peak > 0.97 { 0.97 / peak } else { 1.0 } * 32768.0;
    let mut out = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        for c in 0..2 {
            out.push((at(i, c) * k).round().clamp(-32768.0, 32767.0) as i16);
        }
    }
    out
}

/// The song with the synth in it. `stems`: the set, the mixer's mix, the part's stems (empty: none). `orig`: the
/// song itself when there are no stems.
fn make_song(
    stems: Option<(&StemSet, &[StemMix], &[usize])>,
    orig: Option<&SongBuffer>,
    synth: &[f32],
) -> Option<(SongBuffer, How)> {
    let (base, base_gain, target, how) = match stems {
        Some((set, mix, part)) if !part.is_empty() => {
            let gains = effective_gains(mix);
            let mut without = mix.to_vec();
            for &i in part {
                if let Some(m) = without.get_mut(i) {
                    m.on = false;
                }
            }
            let base = set.mix(&without);
            // the synth as loud as the stems it stands in for (at their mixer level; unity if they were muted)
            let parts: Vec<(&[i16], f32)> = part
                .iter()
                .filter_map(|&i| {
                    let g = match gains.get(i) {
                        Some(&g) if g > 0.0 => g,
                        _ => mix
                            .get(i)
                            .map_or(1.0, |m| if m.gain > 0.0 { m.gain } else { 1.0 }),
                    };
                    set.stems.get(i).map(|s| (s.samples.as_slice(), g))
                })
                .collect();
            let target = loud_rms(set.frames, set.rate, |f| {
                parts.iter().map(|(s, g)| frame_level(s, f) * g).sum::<f32>()
            });
            let target = if target > 0.0 {
                target
            } else {
                OVER * loud_rms(base.frames(), base.rate, |f| frame_level(&base.samples, f))
            };
            (base, 1.0, target, How::Stem)
        }
        Some((set, mix, _)) => {
            let base = set.mix(mix);
            let t = OVER * loud_rms(base.frames(), base.rate, |f| frame_level(&base.samples, f));
            (base, DUCK, t, How::Over)
        }
        None => {
            let base = orig?.clone();
            let t = OVER * loud_rms(base.frames(), base.rate, |f| frame_level(&base.samples, f));
            (base, DUCK, t, How::Over)
        }
    };
    let target = if target > 1e-4 { target } else { ALONE };
    let have = loud_rms(synth.len(), base.rate, |i| synth[i].abs());
    let gain = if have > 0.0 { target / have } else { 0.0 };
    let samples = combine(&base.samples, base_gain, synth, gain);
    Some((
        SongBuffer {
            rate: base.rate,
            samples: Arc::new(samples),
        },
        how,
    ))
}

impl PlayerView {
    /// The part's stems in this song's stem set (empty when it has none, or none for the part).
    fn synth_part_stems(&self) -> Vec<usize> {
        let Some(set) = &self.mixer.stems else {
            return Vec::new();
        };
        let ids: Vec<(&str, &str)> = set
            .stems
            .iter()
            .map(|s| (s.info.id.as_str(), s.info.name.as_str()))
            .collect();
        part_stems(&ids, &self.part_name, self.chart.arr.is_bass)
    }

    /// How the synth would sit in this song, and the stems it would stand in for.
    pub(super) fn synth_how(&self) -> (How, Vec<String>) {
        let part = self.synth_part_stems();
        match &self.mixer.stems {
            Some(set) if !part.is_empty() => (
                How::Stem,
                part.iter().map(|&i| set.stems[i].info.name.clone()).collect(),
            ),
            _ => (How::Over, Vec::new()),
        }
    }

    fn synth_key(&self) -> Option<Key> {
        let (frames, rate) = match (
            &self.mixer.stems,
            self.song_synth.orig.as_ref().or(self.transport.song()),
        ) {
            (Some(set), _) => (set.frames, set.rate),
            (None, Some(song)) => (song.frames(), song.rate),
            (None, None) => return None,
        };
        Some(Key {
            part: self.part_key.clone(),
            mastery: (self.mastery * 1000.0).round() as i64,
            offset_ms: (self.offset * 1000.0).round() as i64,
            notes: self.chart.glyphs.len(),
            frames,
            rate,
        })
    }

    /// What the key shows now.
    pub(super) fn song_synth_status(&self) -> Status {
        let s = &self.song_synth;
        if let Some(p) = s.shot {
            return if p >= 1.0 {
                Status::On
            } else {
                Status::Preparing(p)
            };
        }
        if !s.on {
            return Status::Off;
        }
        if self.engine().is_none() {
            return Status::On;
        }
        let want = self.synth_key();
        let ready = s.made.as_ref().is_some_and(|m| Some(&m.key) == want.as_ref());
        if ready {
            return Status::On;
        }
        let p = match (&s.render, &s.mixing) {
            (Some(r), _) => 0.9 * r.progress.load(Ordering::Relaxed) as f32 / 1000.0,
            (None, Some(_)) => 0.95,
            (None, None) => 0.0,
        };
        Status::Preparing(p)
    }

    /// The key (G): the synth guitar on or off, remembered.
    pub(super) fn toggle_song_synth(&mut self, st: &mut AppState) {
        let on = !self.song_synth.on;
        self.song_synth.set(&mut st.config, on);
        if !on {
            self.say("Real guitar: ", "the song as recorded");
            return;
        }
        if self.engine().is_none() {
            self.say("The synth guitar needs the audio engine", "");
            return;
        }
        let tail = match self.synth_how() {
            (How::Stem, names) => format!("{} in place of the {} stem", self.part_name, names.join(" and ")),
            (How::Over, _) => format!("{} over the song (its guitar can't be taken out)", self.part_name),
        };
        self.say("Synth guitar: ", tail);
    }

    /// The stem mixer changed: with the synth on, its version is mixed again (instead of the plain mix).
    pub(super) fn song_synth_remix(&mut self) -> bool {
        if self.song_synth.on {
            self.song_synth.mix_gen += 1;
            true
        } else {
            false
        }
    }

    /// Every frame: renders the part when it changed, mixes it into the song when that's ready, and puts the song
    /// back when the synth goes off.
    pub(super) fn update_song_synth(&mut self) {
        if self.song_synth.shot.is_some() || self.engine().is_none() {
            return;
        }
        // a render done
        if let Some(r) = &self.song_synth.render {
            match r.rx.try_recv() {
                Ok(v) => {
                    self.song_synth.track = Some((r.key.clone(), Arc::new(v)));
                    self.song_synth.render = None;
                }
                Err(TryRecvError::Disconnected) => self.song_synth.render = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        // a version mixed
        if let Some((made, rx)) = &self.song_synth.mixing {
            match rx.try_recv() {
                Ok(buf) => {
                    self.song_synth.made = Some(made.clone());
                    self.song_synth.mixing = None;
                    self.swap_song(buf);
                }
                Err(TryRecvError::Disconnected) => self.song_synth.mixing = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        if !self.song_synth.on {
            self.song_synth.render = None;
            if self.song_synth.made.is_some() && self.song_synth.mixing.is_none() {
                self.song_synth.made = None;
                if self.mixer.stems.is_some() {
                    self.remix_now();
                } else if let Some(o) = self.song_synth.orig.clone() {
                    self.swap_song((*o).clone());
                }
            }
            return;
        }
        let Some(key) = self.synth_key() else { return };
        let have = self.song_synth.track.as_ref().is_some_and(|(k, _)| *k == key);
        if !have && self.song_synth.render.as_ref().map(|r| &r.key) != Some(&key) {
            self.start_part_render(key.clone());
        }
        if !have || self.song_synth.mixing.is_some() {
            return;
        }
        let want = Made {
            key,
            mix_gen: self.song_synth.mix_gen,
        };
        if self.song_synth.made.as_ref() == Some(&want) {
            return;
        }
        if self.song_synth.made.is_none() && self.mixer.stems.is_none() {
            // the song as it is now, to put back
            self.song_synth.orig = self.transport.song().cloned();
        }
        let synth = self
            .song_synth
            .track
            .as_ref()
            .map(|t| t.1.clone())
            .unwrap_or_default();
        let stems = self.mixer.stems.clone();
        let mix = self.mixer.mix.clone();
        let part = self.synth_part_stems();
        let orig = self.song_synth.orig.clone();
        let (tx, rx) = channel();
        let spawned = std::thread::Builder::new()
            .name("qrs-song-synth-mix".into())
            .spawn(move || {
                let stems = stems.as_deref().map(|s| (s, mix.as_slice(), part.as_slice()));
                if let Some((buf, _)) = make_song(stems, orig.as_deref(), &synth) {
                    let _ = tx.send(buf);
                }
            });
        if spawned.is_ok() {
            self.song_synth.mixing = Some((want, rx));
        }
    }

    fn start_part_render(&mut self, key: Key) {
        let Some(e) = self.engine() else { return };
        let notes = self.chart.synth_notes();
        // the open strings as written: a transpose shifts the whole song, the synth with it
        let opens = self.chart.opens.clone();
        let bass = self.chart.arr.is_bass;
        let tone = qrs_audio::synth::synth_tone(&e.tone(), bass);
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(AtomicU32::new(0));
        let (tx, rx) = channel();
        let (c, p, k) = (cancel.clone(), progress.clone(), key.clone());
        let spawned = std::thread::Builder::new()
            .name("qrs-song-synth".into())
            .spawn(move || {
                let offset = k.offset_ms as f64 / 1000.0;
                if let Some(v) = qrs_audio::synth::render_part(
                    &notes,
                    &opens,
                    bass,
                    Some(tone),
                    k.rate,
                    k.frames,
                    offset,
                    &c,
                    &p,
                ) {
                    let _ = tx.send(v);
                }
            });
        if spawned.is_ok() {
            // (a render of what was wanted before is dropped, which cancels it)
            self.song_synth.render = Some(Render {
                key,
                cancel,
                progress,
                rx,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qrs_formats::feedpak::StemInfo;

    #[test]
    fn the_part_s_stem_is_found() {
        let set = [
            ("vocals", "Vocals"),
            ("guitar", "Guitar"),
            ("bass", "Bass"),
            ("drums", "Drums"),
        ];
        assert_eq!(part_stems(&set, "Lead", false), [1]);
        assert_eq!(part_stems(&set, "Rhythm", false), [1]);
        assert_eq!(part_stems(&set, "Bass", true), [2]);
        let two = [
            ("lead_guitar", "Lead guitar"),
            ("rhythm_gtr", "Rhythm"),
            ("other", "Other"),
        ];
        assert_eq!(part_stems(&two, "Lead", false), [0]);
        assert_eq!(part_stems(&two, "Rhythm 2", false), [1]);
        assert_eq!(part_stems(&two, "Combo", false), [0, 1]);
        assert!(part_stems(&[("other", "Other")], "Lead", false).is_empty());
        assert!(part_stems(&two, "Bass", true).is_empty());
    }

    fn info(id: &str) -> StemInfo {
        StemInfo {
            id: id.into(),
            name: id.into(),
            file: format!("stems/{id}.ogg"),
            default: true,
        }
    }

    fn tone(rate: u32, hz: f64, amp: f64, frames: usize) -> Vec<i16> {
        (0..frames)
            .flat_map(|i| {
                let v = ((i as f64 / rate as f64 * hz * std::f64::consts::TAU).sin() * amp * 32767.0) as i16;
                [v, v]
            })
            .collect()
    }

    fn set(rate: u32, frames: usize) -> StemSet {
        let stem = |id: &str, hz, amp| qrs_audio::stems::Stem {
            info: info(id),
            samples: Arc::new(tone(rate, hz, amp, frames)),
        };
        StemSet {
            rate,
            frames,
            stems: vec![stem("drums", 100.0, 0.2), stem("guitar", 440.0, 0.3)],
            full: None,
        }
    }

    #[test]
    fn the_synth_takes_the_guitar_stem_s_place() {
        let (rate, frames) = (8000, 8000);
        let set = set(rate, frames);
        let mix = set.default_mix();
        // a synth "part" at another level: it's brought to the guitar stem's
        let synth: Vec<f32> = tone(rate, 220.0, 0.05, frames)
            .iter()
            .step_by(2)
            .map(|&v| v as f32 / 32768.0)
            .collect();
        let (song, how) = make_song(Some((&set, &mix, &[1])), None, &synth).unwrap();
        assert_eq!(how, How::Stem);
        assert_eq!(song.frames(), frames);
        // the song is the drums plus the synth at the guitar's level: no 440 Hz left
        let lv = |s: &[f32]| loud_rms(s.len(), rate, |i| s[i].abs());
        let left: Vec<f32> = song
            .samples
            .iter()
            .step_by(2)
            .map(|&v| v as f32 / 32768.0)
            .collect();
        let drums: Vec<f32> = set.stems[0]
            .samples
            .iter()
            .step_by(2)
            .map(|&v| v as f32 / 32768.0)
            .collect();
        let rest: Vec<f32> = left.iter().zip(&drums).map(|(a, b)| a - b).collect();
        assert!((lv(&rest) - 0.3 / 2f32.sqrt()).abs() < 0.01, "{}", lv(&rest));
        let corr = |a: &[f32], hz: f64| {
            a.iter()
                .enumerate()
                .map(|(i, v)| v * (i as f64 / rate as f64 * hz * std::f64::consts::TAU).sin() as f32)
                .sum::<f32>()
                / a.len() as f32
        };
        assert!(corr(&rest, 440.0).abs() < 0.005, "the guitar stem is out");
        assert!(corr(&rest, 220.0) > 0.1, "the synth is in");
    }

    #[test]
    fn over_a_full_mix_the_song_is_turned_down() {
        let (rate, frames) = (8000, 8000);
        let orig = SongBuffer {
            rate,
            samples: Arc::new(tone(rate, 440.0, 0.4, frames)),
        };
        let synth = vec![0.0f32; frames];
        let (song, how) = make_song(None, Some(&orig), &synth).unwrap();
        assert_eq!(how, How::Over);
        for (a, b) in song.samples.iter().zip(orig.samples.iter()).take(4000) {
            assert!((*a as f32 - *b as f32 * DUCK).abs() <= 1.0);
        }
        // a stem set with no stem for the part: the same, over its mix
        let set = set(rate, frames);
        let mix = set.default_mix();
        let (_, how) = make_song(Some((&set, &mix, &[])), None, &synth).unwrap();
        assert_eq!(how, How::Over);
    }

    #[test]
    fn it_is_remembered() {
        let path = std::env::temp_dir().join(format!("qrs-song-synth-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut cfg = qrs_library::config::Config::load(&path);
        let mut s = SongSynth::from_config(&cfg);
        assert!(!s.on, "off at first");
        s.set(&mut cfg, true);
        cfg.save().unwrap();
        let back = qrs_library::config::Config::load(&path);
        assert!(SongSynth::from_config(&back).on);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_loud_sum_is_turned_down_not_clipped() {
        let base = vec![30000i16; 200];
        let synth = vec![0.9f32; 100];
        let out = combine(&base, 1.0, &synth, 1.0);
        assert_eq!(out.len(), 200);
        assert!(out.iter().all(|&v| v < 32767 && v > 31000), "{}", out[0]);
    }
}
