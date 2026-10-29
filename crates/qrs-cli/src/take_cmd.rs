//! Takes from the command line (P18.3-P18.4, qrs-analysis take.rs):
//!
//!   quickrs-cli take take.wav song.feedpak --arr lead --from 10.7 --to 27.6 [--speed 1] [--offset-ms N] [--json out]
//!       analyse a recording of the guitar against the chart: the report, the problems, both tabs.
//!       Sample 0 of the WAV is chart time --start (default --from); --offset-ms is the input latency to take off.
//!       A take saved by the app has its timing in the .json beside it, used when --from/--to aren't given.
//!   quickrs-cli take-check song.feedpak --arr lead --from 10.7 --to 27.6 [--speed 1] [--tone clean|dry]
//!       the analysis checked on the synth: the passage played clean, then with wrong frets, dropped notes, extra
//!       notes, notes 40-80 ms off, the whole take 30 ms late, hiss and a level drop, at half speed; how many of
//!       the changes it reports, and how many right notes it calls wrong.
//!   quickrs-cli transcribe recording.wav [--from s] [--to s]
//!       every pluck in a recording with its pitch(es) and a string/fret: the tab generator with no chart.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use qrs_analysis::take::{self, Neck, Status, TakeInput, TakeTiming};
use qrs_formats::song_audio::{Codec, EncodedAudio};
use qrs_formats::{Arrangement, Techs};

/// A recording as mono f32 and its rate.
pub fn load_mono(path: &Path) -> anyhow::Result<(Vec<f32>, u32)> {
    let data = std::fs::read(path)?;
    if let Ok((rate, ch, s)) = qrs_audio::decode::read_wav_i16(&data) {
        let ch = ch.max(1) as usize;
        let mono = s
            .chunks_exact(ch)
            .map(|f| f.iter().map(|&v| v as f32 / 32768.0).sum::<f32>() / ch as f32)
            .collect();
        return Ok((mono, rate));
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("wav");
    let pcm = qrs_audio::decode::decode(&EncodedAudio {
        data,
        codec: Codec::from_name(&format!("x.{ext}")),
        origin: path.display().to_string(),
    })?;
    let ch = pcm.channels.max(1) as usize;
    let mono = pcm
        .samples
        .chunks_exact(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    Ok((mono, pcm.rate))
}

#[derive(clap::Args)]
pub struct TakeArgs {
    /// the take (WAV)
    pub take: PathBuf,
    /// .psarc, .feedpak, .sng or .xml
    pub song: PathBuf,
    #[arg(long)]
    pub arr: Option<String>,
    /// chart seconds: the stretch played
    #[arg(long)]
    pub from: Option<f64>,
    #[arg(long)]
    pub to: Option<f64>,
    /// chart time of the WAV's first sample (default: --from)
    #[arg(long)]
    pub start: Option<f64>,
    /// the speed it was played at (0.25..1.5)
    #[arg(long, default_value_t = 1.0)]
    pub speed: f64,
    /// the input latency in the recording (ms): plucks this much late in the file count as on time
    #[arg(long, default_value_t = 0.0)]
    pub offset_ms: f64,
    /// also write the analysis as JSON here
    #[arg(long)]
    pub json: Option<PathBuf>,
    /// tab width
    #[arg(long, default_value_t = 110)]
    pub width: usize,
}

pub fn take(arr: Arrangement, a: TakeArgs) -> anyhow::Result<()> {
    let (samples, rate) = load_mono(&a.take)?;
    let side = a.take.with_extension("json");
    let saved: Option<serde_json::Value> = std::fs::read_to_string(&side)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let saved_timing = saved
        .as_ref()
        .and_then(|v| v.get("timing"))
        .and_then(TakeTiming::from_json);
    let saved_span = saved.as_ref().and_then(|v| {
        let s = v.get("stretch")?;
        Some((s.get("from")?.as_f64()?, s.get("to")?.as_f64()?))
    });
    let (from, to) = match (a.from, a.to, saved_span) {
        (Some(f), Some(t), _) => (f, t),
        (None, None, Some(s)) => s,
        _ => anyhow::bail!("give --from and --to (chart seconds), or a take with its .json beside it"),
    };
    let timing = match (&saved_timing, a.from) {
        (Some(t), None) => t.clone(),
        _ => {
            let speed = a.speed.clamp(0.25, 1.5);
            TakeTiming::linear(
                rate,
                a.start.unwrap_or(from) - a.offset_ms / 1000.0 * speed,
                speed,
            )
        }
    };
    let input = TakeInput {
        arr,
        from,
        to,
        samples,
        timing,
    };
    let r = take::analyse(&input, &|_| {}, &AtomicBool::new(false))
        .ok_or_else(|| anyhow::anyhow!("analysis cancelled"))?;
    print!("{}", r.text(a.width));
    if let Some(p) = &a.json {
        let mut v = r.to_json();
        v["timing"] = input.timing.to_json();
        std::fs::write(p, serde_json::to_string_pretty(&v)?)?;
        eprintln!("Wrote {}", p.display());
    }
    Ok(())
}

// -- take-check: the analysis against the synth ------------------------------------------------------------------

#[derive(clap::Args)]
pub struct CheckArgs {
    pub song: PathBuf,
    #[arg(long)]
    pub arr: Option<String>,
    #[arg(long, default_value_t = 1.0)]
    pub mastery: f64,
    #[arg(long)]
    pub from: f64,
    #[arg(long)]
    pub to: f64,
    /// clean, crunch, lead, acoustic, or dry (the take is the dry guitar; a tone shows robustness)
    #[arg(long, default_value = "dry")]
    pub tone: String,
    /// write each case's take WAV and report JSON here
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// print each case's full report
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Expect {
    Wrong,
    Missed,
    /// Right, this many wall ms off.
    Off(f64),
}

/// A plain event: no technique that changes how it's judged.
fn plain(t: Techs) -> bool {
    (t & (Techs::BEND
        | Techs::SLIDE
        | Techs::UNPITCHED_SLIDE
        | Techs::HAMMER_ON
        | Techs::PULL_OFF
        | Techs::TAP
        | Techs::HARMONIC
        | Techs::PINCH_HARMONIC
        | Techs::FRET_HAND_MUTE
        | Techs::LINK_NEXT
        | Techs::IGNORE))
        .is_empty()
}

#[derive(Clone, Copy)]
enum Ref {
    Note(usize),
    Chord(usize),
}

fn key(t: f64) -> i64 {
    (t * 1000.0).round() as i64
}

/// The chart's events in [from, to) in time order (notes within 15 ms of each other are one event).
fn events(a: &Arrangement, from: f64, to: f64) -> Vec<(f64, Vec<Ref>, bool)> {
    let mut v: Vec<(f64, Ref, bool)> = Vec::new();
    for (i, n) in a.notes.iter().enumerate() {
        if n.time >= from && n.time < to {
            v.push((n.time, Ref::Note(i), plain(n.techniques)));
        }
    }
    for (i, c) in a.chords.iter().enumerate() {
        if c.time >= from && c.time < to && !c.notes.is_empty() {
            let p = plain(c.techniques) && c.notes.iter().all(|n| plain(n.techniques));
            v.push((c.time, Ref::Chord(i), p));
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    // a note slid or linked into (not picked) isn't plain either
    let mut link: HashMap<i32, bool> = HashMap::new();
    for (_, r, p) in &mut v {
        let notes: Vec<&qrs_formats::Note> = match r {
            Ref::Note(i) => vec![&a.notes[*i]],
            Ref::Chord(i) => a.chords[*i].notes.iter().collect(),
        };
        for n in notes {
            if link.get(&n.string).copied().unwrap_or(false) {
                *p = false;
            }
            link.insert(n.string, n.techniques.has(Techs::LINK_NEXT));
        }
    }
    let mut out: Vec<(f64, Vec<Ref>, bool)> = Vec::new();
    for (t, r, p) in v {
        match out.last_mut() {
            Some(last) if (t - last.0).abs() < 0.015 => {
                last.1.push(r);
                last.2 &= p;
            }
            _ => out.push((t, vec![r], p)),
        }
    }
    out
}

fn shift_ref(a: &mut Arrangement, r: Ref, dt: f64) {
    match r {
        Ref::Note(i) => a.notes[i].time += dt,
        Ref::Chord(i) => {
            a.chords[i].time += dt;
            for n in &mut a.chords[i].notes {
                n.time += dt;
            }
        }
    }
}

struct Case {
    name: &'static str,
    played: Arrangement,
    expect: HashMap<i64, Expect>,
    extras: Vec<f64>,
    speed: f64,
    late: f64,
    noise: bool,
}

fn render(
    arr: &Arrangement,
    from: f64,
    to: f64,
    speed: f64,
    tone: Option<qrs_audio::tone::ToneParams>,
) -> Vec<f32> {
    qrs_audio::synth::render_mono(arr, from, to, speed, tone, 48000)
}

pub fn check(arr: Arrangement, a: CheckArgs) -> anyhow::Result<()> {
    use qrs_audio::tone::Preset;
    let tone = match a.tone.to_lowercase().as_str() {
        "dry" | "none" => None,
        t => Some(
            Preset::ALL
                .into_iter()
                .find(|p| p.label().eq_ignore_ascii_case(t))
                .ok_or_else(|| anyhow::anyhow!("unknown tone {t:?}"))?
                .params(),
        ),
    };
    let (from, to) = (a.from, a.to);
    let evs = events(&arr, from, to);
    if evs.is_empty() {
        anyhow::bail!("no notes between {from} and {to}");
    }
    println!(
        "{} - {} ({}), {from:.2}-{to:.2} s: {} events, tone {}",
        arr.artist,
        arr.title,
        arr.name,
        evs.len(),
        a.tone
    );
    let mut cases: Vec<Case> = Vec::new();
    let base = |name| Case {
        name,
        played: arr.clone(),
        expect: HashMap::new(),
        extras: vec![],
        speed: 1.0,
        late: 0.0,
        noise: false,
    };
    cases.push(base("clean"));
    // a fret up on every 7th plain event (one note of a chord)
    let mut c = base("wrong frets");
    for (k, (t, refs, p)) in evs.iter().enumerate() {
        if k % 7 == 3 && *p {
            match refs[0] {
                Ref::Note(i) if c.played.notes[i].fret < 22 => c.played.notes[i].fret += 1,
                // a chord's top string (a fret off on a low string inside a full chord is a few Hz from the
                // right note, under what the 85 ms window can split)
                Ref::Chord(i) if c.played.chords[i].notes.last().is_some_and(|n| n.fret < 22) => {
                    if let Some(n) = c.played.chords[i].notes.last_mut() {
                        n.fret += 1;
                    }
                }
                _ => continue,
            }
            c.expect.insert(key(*t), Expect::Wrong);
        }
    }
    cases.push(c);
    // every 9th plain event left out
    let mut c = base("dropped notes");
    let mut drop_notes = Vec::new();
    let mut drop_chords = Vec::new();
    for (k, (t, refs, p)) in evs.iter().enumerate() {
        if k % 9 == 4 && *p {
            for r in refs {
                match r {
                    Ref::Note(i) => drop_notes.push(*i),
                    Ref::Chord(i) => drop_chords.push(*i),
                }
            }
            c.expect.insert(key(*t), Expect::Missed);
        }
    }
    let mut i = 0;
    c.played.notes.retain(|_| {
        i += 1;
        !drop_notes.contains(&(i - 1))
    });
    let mut i = 0;
    c.played.chords.retain(|_| {
        i += 1;
        !drop_chords.contains(&(i - 1))
    });
    cases.push(c);
    // an extra note in every 6th gap of 250 ms or more, two frets above the note before, on its string
    let mut c = base("extra notes");
    for k in 0..evs.len().saturating_sub(1) {
        let (t, refs, _) = &evs[k];
        let gap = evs[k + 1].0 - t;
        if k % 6 == 2 && gap >= 0.25 {
            let (string, fret) = match refs[0] {
                Ref::Note(i) => (arr.notes[i].string, arr.notes[i].fret),
                Ref::Chord(i) => (arr.chords[i].notes[0].string, arr.chords[i].notes[0].fret),
            };
            let at = t + gap / 2.0;
            let f = if fret + 2 <= 22 { fret + 2 } else { fret - 2 };
            c.played.notes.push(qrs_formats::Note::new(at, string, f));
            c.extras.push(at);
        }
    }
    c.played.notes.sort_by(|a, b| a.time.total_cmp(&b.time));
    cases.push(c);
    // every 5th plain event 40 ms late, 40 ms early, 80 ms late in turn
    let mut c = base("timing +-40/80 ms");
    for (k, (t, refs, p)) in evs.iter().enumerate() {
        if k % 5 == 2 && *p {
            let ms = [40.0, -40.0, 80.0][(k / 5) % 3];
            for r in refs {
                shift_ref(&mut c.played, *r, ms / 1000.0);
            }
            c.expect.insert(key(*t), Expect::Off(ms));
        }
    }
    cases.push(c);
    let mut c = base("whole take 30 ms late");
    c.late = 0.03;
    for (t, _, _) in &evs {
        c.expect.insert(key(*t), Expect::Off(30.0));
    }
    cases.push(c);
    let mut c = base("hiss, hum, level -20 dB");
    c.noise = true;
    cases.push(c);
    let mut c = base("half speed");
    c.speed = 0.5;
    cases.push(c);

    println!(
        "\n{:<24} {:>8} {:>9} {:>14} {:>13} {:>8} {:>9}",
        "case", "accuracy", "right ok", "changes found", "extras found", "timing", "analysis"
    );
    for case in &cases {
        let mut x = render(&case.played, from, to + 0.5, case.speed, tone);
        let lead = (case.late * 48000.0) as usize;
        if lead > 0 {
            let mut y = vec![0.0f32; lead];
            y.extend_from_slice(&x);
            x = y;
        }
        if case.noise {
            let n = x.len();
            let mut seed: u32 = 3;
            for (i, v) in x.iter_mut().enumerate() {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let hiss = ((seed >> 8) as f32 / 16777216.0 - 0.5) * 0.004;
                let hum = 0.002 * (std::f32::consts::TAU * 50.0 * i as f32 / 48000.0).sin();
                *v = *v * if i > n / 2 { 0.1 } else { 1.0 } + hiss + hum;
            }
        }
        let input = TakeInput {
            arr: arr.clone(),
            from,
            to,
            samples: x,
            timing: TakeTiming::linear(48000, from, case.speed),
        };
        let t0 = Instant::now();
        let r = take::analyse(&input, &|_| {}, &AtomicBool::new(false)).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let (mut ok_right, mut n_right, mut found, mut n_changes) = (0, 0, 0, 0);
        for e in &r.events {
            match case.expect.get(&key(e.t)) {
                Some(Expect::Off(want)) if case.name.starts_with("whole") => {
                    // every note: right, and timed within 15 ms of the shift
                    n_right += 1;
                    if e.status == Status::Hit {
                        ok_right += 1;
                    }
                    let _ = want;
                }
                Some(x) => {
                    n_changes += 1;
                    let hit = match x {
                        Expect::Wrong => e.status == Status::Wrong,
                        Expect::Missed => e.status == Status::Missed,
                        Expect::Off(want) => {
                            e.status == Status::Hit && e.offset_ms.is_some_and(|o| (o - want).abs() <= 15.0)
                        }
                    };
                    if hit {
                        found += 1;
                    } else if a.verbose {
                        println!(
                            "    not found: {x:?} at {:.2}: {:?} {:?}",
                            e.t, e.status, e.offset_ms
                        );
                    }
                }
                None => {
                    if e.status == Status::NotChecked {
                        continue;
                    }
                    n_right += 1;
                    if e.status == Status::Hit {
                        ok_right += 1;
                    } else if a.verbose {
                        println!("    called {:?} at {:.2} ({:?})", e.status, e.t, e.played);
                    }
                }
            }
        }
        let extras_found = case
            .extras
            .iter()
            .filter(|&&t| r.extras.iter().any(|x| (x.t - t).abs() < 0.03))
            .count();
        let timing = if case.name.starts_with("whole") {
            format!("{:+.0} ms", r.timing.median_ms)
        } else {
            format!("{:+.0}", r.timing.median_ms)
        };
        println!(
            "{:<24} {:>7.0}% {:>4}/{:<4} {:>14} {:>13} {:>8} {:>6.0} ms",
            case.name,
            r.accuracy() * 100.0,
            ok_right,
            n_right,
            if n_changes > 0 {
                format!("{found}/{n_changes}")
            } else {
                "-".into()
            },
            if case.extras.is_empty() {
                if r.extras.is_empty() {
                    "-".to_string()
                } else {
                    format!("({} false)", r.extras.len())
                }
            } else {
                format!("{extras_found}/{} ({} all)", case.extras.len(), r.extras.len())
            },
            timing,
            ms
        );
        if a.verbose {
            for p in &r.problems {
                println!("    * {p}");
            }
        }
        if let Some(dir) = &a.out_dir {
            std::fs::create_dir_all(dir)?;
            let stem = case.name.replace([' ', '/', ','], "_");
            let pcm: Vec<i16> = input
                .samples
                .iter()
                .map(|&v| qrs_audio::decode::f32_to_i16(v))
                .collect();
            qrs_audio::decode::write_wav_i16(&dir.join(format!("{stem}.wav")), 48000, 1, &pcm)?;
            let mut v = r.to_json();
            v["timing"] = input.timing.to_json();
            std::fs::write(
                dir.join(format!("{stem}.json")),
                serde_json::to_string_pretty(&v)?,
            )?;
        }
    }
    Ok(())
}

// -- transcribe: plucks and pitches, no chart --------------------------------------------------------------------

#[derive(clap::Args)]
pub struct TranscribeArgs {
    pub file: PathBuf,
    /// seconds into the file
    #[arg(long, default_value_t = 0.0)]
    pub from: f64,
    #[arg(long)]
    pub to: Option<f64>,
    /// at most this many plucks listed
    #[arg(long, default_value_t = 400)]
    pub max: usize,
}

pub fn transcribe(a: TranscribeArgs) -> anyhow::Result<()> {
    let (x, rate) = load_mono(&a.file)?;
    let s0 = ((a.from * rate as f64) as usize).min(x.len());
    let s1 =
        a.to.map_or(x.len(), |t| ((t * rate as f64) as usize).min(x.len()));
    let x = &x[s0..s1.max(s0)];
    let t = Instant::now();
    let r = free_transcription(x, rate);
    let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let rms = (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt();
    println!(
        "{}: {:.1} s at {rate} Hz, peak {:.3} ({:.1} dBFS), rms {:.4}; {} plucks ({:.0} ms)",
        a.file.display(),
        x.len() as f64 / rate as f64,
        peak,
        20.0 * peak.max(1e-9).log10(),
        rms,
        r.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
    println!("\n    time  attack  clarity  heard          string/fret");
    for p in r.iter().take(a.max) {
        println!(
            "{:>8.3}  {:>5}  {:>6.2}  {:<14} {}",
            a.from + p.t,
            if p.attack { "pick" } else { "legato" },
            p.clarity,
            p.names,
            p.place
        );
    }
    Ok(())
}

pub struct FreeNote {
    pub t: f64,
    pub attack: bool,
    pub clarity: f64,
    pub names: String,
    pub place: String,
}

/// Every pluck with its pitch (MPM when clear, else the strongest harmonic series) and a place on a standard-tuned
/// neck, the hand following the notes.
pub fn free_transcription(x: &[f32], rate: u32) -> Vec<FreeNote> {
    use qrs_analysis::pitch::{hz_to_midi, mpm_pitch, note_label};
    let neck = Neck::new(&Arrangement::new("Lead"));
    let ons = qrs_analysis::onsets::detect(x, rate);
    let mut sp = qrs_analysis::Spectra::new();
    let mut hand = 5.0f64;
    let mut out = Vec::new();
    for (j, o) in ons.iter().enumerate() {
        let at = o.sample + (0.015 * rate as f64) as usize;
        let next = ons.get(j + 1).map_or(x.len(), |n| n.sample);
        let n = (next.saturating_sub(at)).clamp((0.04 * rate as f64) as usize, (0.12 * rate as f64) as usize);
        if at + n > x.len() {
            continue;
        }
        let w = &x[at..at + n];
        let (hz, clarity) = mpm_pitch(&mut sp, w, rate, 60.0, 1400.0, 0.9);
        let midis: Vec<i32> = if hz > 0.0 && clarity >= 0.85 {
            vec![hz_to_midi(hz).round() as i32]
        } else {
            take::pitches(&mut sp, w, rate, 40, 88, 3)
                .iter()
                .map(|p| p.0)
                .collect()
        };
        let places: Vec<String> = midis
            .iter()
            .filter_map(|&m| neck.place(m, None, hand))
            .map(|p| format!("{}:{}", neck.label(p.string), p.fret))
            .collect();
        if let Some(&m) = midis.first()
            && let Some(p) = neck.place(m, None, hand)
            && p.fret > 0
        {
            hand = 0.6 * hand + 0.4 * p.fret as f64;
        }
        out.push(FreeNote {
            t: o.sample as f64 / rate as f64,
            attack: o.attack,
            clarity,
            names: midis
                .iter()
                .map(|&m| note_label(m as f64))
                .collect::<Vec<_>>()
                .join("+"),
            place: places.join(" "),
        });
    }
    out
}
