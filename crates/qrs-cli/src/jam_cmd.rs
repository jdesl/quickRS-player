//! `quickrs-cli jam`: the amp room's jam engine offline (P22.4). `render` writes WAVs to listen to: 8 bars of every
//! drum style (a bar of count-in, a fill every 4 bars), the metronome, a drone, and a chord loop over a groove,
//! all made by the audio thread's own code (`rt::jam::Jam`) block by block. `scale` prints the fretboard of a key's
//! scale and a progression's chords.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use qrs_audio::jam::theory::{self, Scale};
use qrs_audio::jam::{
    BackingKind, BackingSpec, ClickParams, DrumKit, DrumParams, DrumStyle, LoopAction, LooperSettings,
    backing,
};
use qrs_audio::rt::Trash;
use qrs_audio::rt::jam::{Jam, JamCmd, JamShared, LoopStore};

#[derive(clap::Args)]
pub struct Args {
    /// render (the listening set) or scale (the fretboard map)
    #[arg(default_value = "render")]
    pub action: String,
    /// Folder for the WAVs
    #[arg(short, long)]
    pub out: Option<PathBuf>,
    /// Key (C, F#, Bb...)
    #[arg(long, default_value = "A")]
    pub key: String,
    /// Scale: minor-pentatonic, major-pentatonic, blues, major, natural-minor, dorian, mixolydian, phrygian,
    /// lydian, harmonic-minor
    #[arg(long, default_value = "natural-minor")]
    pub scale: String,
    /// The chord loop, as names or numerals ("Am F C G", "I-V-vi-IV")
    #[arg(long, default_value = "i VI III VII")]
    pub chords: String,
    /// Tempo for the chord loop and the click (the drum styles use their own unless this is given)
    #[arg(long)]
    pub bpm: Option<f64>,
    /// Bars of each drum style
    #[arg(long, default_value_t = 8)]
    pub bars: u32,
}

const RATE: u32 = 48000;
const BLOCK: usize = 256;

fn parse_scale(s: &str) -> anyhow::Result<Scale> {
    let k = s.to_ascii_lowercase().replace(['-', '_', ' '], "");
    Scale::ALL
        .into_iter()
        .find(|sc| sc.name().to_ascii_lowercase().replace(' ', "") == k)
        .ok_or_else(|| anyhow::anyhow!("unknown scale {s}"))
}

/// A tempo that suits each groove.
fn style_bpm(s: DrumStyle) -> f64 {
    match s {
        DrumStyle::Rock => 120.0,
        DrumStyle::Indie => 128.0,
        DrumStyle::Funk => 98.0,
        DrumStyle::Shuffle => 96.0,
        DrumStyle::HalfTime => 140.0,
        DrumStyle::Ballad => 72.0,
        DrumStyle::Metal => 170.0,
    }
}

/// Drive a jam offline: `cmds` first, then `seconds` of output (the guitar silent, or from `guitar`).
struct Offline {
    jam: Jam,
    sh: JamShared,
    trash: rtrb::Producer<Trash>,
    trash_rx: rtrb::Consumer<Trash>,
}

impl Offline {
    fn new() -> Offline {
        let (trash, trash_rx) = rtrb::RingBuffer::new(256);
        Offline {
            jam: Jam::new(RATE, BLOCK),
            sh: JamShared::default(),
            trash,
            trash_rx,
        }
    }

    fn cmd(&mut self, c: JamCmd) {
        self.jam.command(c, &mut self.trash);
    }

    fn run(&mut self, seconds: f64, guitar: &dyn Fn(u64) -> f32, out: &mut Vec<f32>) {
        let frames = (seconds * RATE as f64) as usize;
        let mut mix = vec![0.0f32; BLOCK * 2];
        let mut g = vec![0.0f32; BLOCK];
        let mut done = 0;
        while done < frames {
            let n = BLOCK.min(frames - done);
            let t = self.jam.timeline();
            for (i, v) in g[..n].iter_mut().enumerate() {
                *v = guitar(t + i as u64);
            }
            mix[..2 * n].fill(0.0);
            self.jam.process(
                &mut mix[..2 * n],
                &g[..n],
                &g[..n],
                n,
                0,
                &self.sh,
                &mut self.trash,
            );
            // the guitar is heard too (monitored)
            for (f, v) in mix[..2 * n].chunks(2).zip(&g[..n]) {
                out.push(f[0] + v * 0.7);
                out.push(f[1] + v * 0.7);
            }
            while self.trash_rx.pop().is_ok() {}
            done += n;
        }
    }
}

fn write(dir: &Path, name: &str, stereo: &[f32]) -> anyhow::Result<()> {
    let peak = stereo.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let s: Vec<i16> = stereo
        .iter()
        .map(|&v| qrs_audio::decode::f32_to_i16(qrs_audio::rt::output::soft_clip(v)))
        .collect();
    let path = dir.join(name);
    qrs_audio::decode::write_wav_i16(&path, RATE, 2, &s)?;
    println!(
        "  {name:<28} {:5.1} s  peak {:5.1} dBFS",
        stereo.len() as f64 / 2.0 / RATE as f64,
        20.0 * peak.max(1e-6).log10()
    );
    Ok(())
}

fn render(args: &Args) -> anyhow::Result<()> {
    let dir = args
        .out
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join("quickrs-jam"));
    std::fs::create_dir_all(&dir)?;
    let key = theory::parse_note(&args.key).ok_or_else(|| anyhow::anyhow!("unknown key {}", args.key))?;
    let scale = parse_scale(&args.scale)?;
    let chords = theory::parse_progression(&args.chords, key, scale).map_err(anyhow::Error::msg)?;
    let kit = Arc::new(DrumKit::new(RATE));
    let silent = |_: u64| 0.0f32;
    println!("Writing to {}", dir.display());

    // every drum style: a bar of count-in, then the groove with a fill every 4 bars
    for style in DrumStyle::ALL {
        let bpm = args.bpm.unwrap_or(style_bpm(style));
        let mut o = Offline::new();
        o.cmd(JamCmd::Kit(kit.clone()));
        o.cmd(JamCmd::Tempo(bpm));
        o.cmd(JamCmd::Click(ClickParams {
            on: false,
            ..Default::default()
        }));
        o.cmd(JamCmd::Drums(DrumParams {
            on: true,
            style,
            volume: 0.8,
            fill_every: 4,
            ..Default::default()
        }));
        o.cmd(JamCmd::Start { count_in: 1 });
        let mut out = Vec::new();
        o.run((args.bars + 1) as f64 * 4.0 * 60.0 / bpm + 1.5, &silent, &mut out);
        write(&dir, &format!("drums_{}.wav", style.id()), &out)?;
    }

    let bpm = args.bpm.unwrap_or(100.0);
    // the metronome: 4 bars, accented
    {
        let mut o = Offline::new();
        o.cmd(JamCmd::Kit(kit.clone()));
        o.cmd(JamCmd::Tempo(bpm));
        o.cmd(JamCmd::Click(ClickParams::default()));
        o.cmd(JamCmd::Start { count_in: 0 });
        let mut out = Vec::new();
        o.run(16.0 * 60.0 / bpm, &silent, &mut out);
        write(&dir, "click.wav", &out)?;
    }

    // a drone in the key, 4 bars
    {
        let spec = BackingSpec {
            kind: BackingKind::Drone,
            key,
            scale,
            ..Default::default()
        };
        let clip = Arc::new(backing::render(&spec, bpm, 4, RATE));
        let mut o = Offline::new();
        o.cmd(JamCmd::Kit(kit.clone()));
        o.cmd(JamCmd::Tempo(bpm));
        o.cmd(JamCmd::Click(ClickParams {
            on: false,
            ..Default::default()
        }));
        o.cmd(JamCmd::Backing(Some(clip)));
        o.cmd(JamCmd::BackingGain(0.8));
        o.cmd(JamCmd::Start { count_in: 0 });
        let mut out = Vec::new();
        o.run(16.0 * 60.0 / bpm + 0.5, &silent, &mut out);
        write(&dir, "backing_drone.wav", &out)?;
    }

    // the chord loop over a rock groove, two times round, after a bar of click
    {
        let spec = BackingSpec {
            kind: BackingKind::Chords,
            key,
            scale,
            chords: chords.clone(),
            beats_per_chord: 0,
            bass: true,
        };
        let clip = Arc::new(backing::render(&spec, bpm, 4, RATE));
        let mut o = Offline::new();
        o.cmd(JamCmd::Kit(kit.clone()));
        o.cmd(JamCmd::Tempo(bpm));
        o.cmd(JamCmd::Click(ClickParams {
            on: false,
            ..Default::default()
        }));
        o.cmd(JamCmd::Drums(DrumParams {
            on: true,
            style: DrumStyle::Rock,
            volume: 0.6,
            fill_every: 4,
            ..Default::default()
        }));
        o.cmd(JamCmd::Backing(Some(clip)));
        o.cmd(JamCmd::BackingGain(0.7));
        o.cmd(JamCmd::Start { count_in: 1 });
        let bars = chords.len() as f64 * 2.0 + 1.0;
        let mut out = Vec::new();
        o.run(bars * 4.0 * 60.0 / bpm + 1.0, &silent, &mut out);
        write(&dir, "backing_chords.wav", &out)?;
    }

    // the looper: a synth "guitar" riff recorded for two bars on the click, then overdubbed a fifth up, then played
    {
        let mut o = Offline::new();
        o.cmd(JamCmd::Kit(kit.clone()));
        o.cmd(JamCmd::Tempo(bpm));
        o.cmd(JamCmd::Click(ClickParams {
            on: true,
            accent: true,
            volume: 0.35,
        }));
        o.cmd(JamCmd::LoopStore(Some(Box::new(LoopStore::new(
            RATE,
            LooperSettings {
                max_seconds: 20.0,
                max_layers: 4,
            },
        )))));
        o.cmd(JamCmd::Start { count_in: 0 });
        let spb = 60.0 / bpm;
        let riff = |root: f64| {
            move |t: u64| {
                let s = t as f64 / RATE as f64;
                let beat = s / spb;
                let k = (beat * 2.0).floor();
                let into = (beat * 2.0 - k) * spb / 2.0;
                let steps = [0.0, 3.0, 5.0, 7.0, 10.0, 7.0, 5.0, 3.0];
                let m = root + steps[(k as usize) % 8];
                let f = 440.0 * 2f64.powf((m - 69.0) / 12.0);
                ((std::f64::consts::TAU * f * s).sin() * (-into * 6.0).exp() * 0.18) as f32
            }
        };
        let low = riff(45.0);
        let high = riff(52.0);
        let mut out = Vec::new();
        o.cmd(JamCmd::Loop(LoopAction::Record));
        o.run(8.0 * spb, &low, &mut out);
        o.cmd(JamCmd::Loop(LoopAction::Play));
        o.run(8.0 * spb, &silent, &mut out);
        o.cmd(JamCmd::Loop(LoopAction::Overdub));
        o.run(8.0 * spb, &high, &mut out);
        o.cmd(JamCmd::Loop(LoopAction::Play));
        o.run(16.0 * spb, &silent, &mut out);
        write(&dir, "looper.wav", &out)?;
    }

    println!(
        "Chords: {}",
        chords
            .iter()
            .map(|c| format!("{} ({})", c.name(), theory::roman_of(c, key, scale)))
            .collect::<Vec<_>>()
            .join("  ")
    );
    Ok(())
}

fn scale_map(args: &Args) -> anyhow::Result<()> {
    let key = theory::parse_note(&args.key).ok_or_else(|| anyhow::anyhow!("unknown key {}", args.key))?;
    let scale = parse_scale(&args.scale)?;
    let notes = theory::scale_notes(key, scale);
    println!(
        "{} {}: {}",
        theory::note_name(key),
        scale.name(),
        notes
            .iter()
            .map(|&n| theory::note_name(n))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let map = theory::fretboard(&theory::STANDARD, 15, key, scale);
    let names = ["E", "A", "D", "G", "B", "e"];
    for s in (0..6).rev() {
        let mut line = format!("{} |", names[s]);
        for f in 0..=15u8 {
            let cell = match map.iter().find(|n| n.string == s && n.fret == f) {
                Some(n) if n.is_root() => format!("({:<2})", theory::note_name(n.pc)),
                Some(n) => format!(" {:<2} ", theory::note_name(n.pc)),
                None => " -- ".into(),
            };
            line.push_str(&cell);
        }
        println!("{line}");
    }
    let frets: String = (0..=15).map(|f| format!(" {f:<3}")).collect();
    println!("   {frets}");
    if let Ok(chords) = theory::parse_progression(&args.chords, key, scale) {
        println!(
            "Chords: {}",
            chords
                .iter()
                .map(|c| format!("{} ({})", c.name(), theory::roman_of(c, key, scale)))
                .collect::<Vec<_>>()
                .join("  ")
        );
    }
    Ok(())
}

pub fn run(args: Args) -> anyhow::Result<()> {
    match args.action.as_str() {
        "render" => render(&args),
        "scale" => scale_map(&args),
        a => anyhow::bail!("unknown jam action {a} (render, scale)"),
    }
}
