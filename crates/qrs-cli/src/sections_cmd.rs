//! `quickrs-cli sections`: which bars and sections of a part repeat (P18.1, qrs-analysis similar.rs).
//!
//!   quickrs-cli sections song.feedpak --arr rhythm [--spans "28.2-45.1,62.0-79.3"] [--novel 0-200] [--bars]
//!
//! Prints each section's bars as group letters (the same letter = the same bar; "A+5" = bar A moved up five
//! semitones, a 12-bar's IV; "-" = no notes), the similarity matrix of the sections (or of the given spans), the
//! sections that repeat with the bars where they differ, and with --novel the bars of that span unlike anything in
//! --spans ("new here" in a whole-song stretch after the spans were learned).

use std::path::PathBuf;

use qrs_analysis::similar::{self, BarLabel, Bars, SectionBars};
use qrs_formats::Arrangement;

#[derive(clap::Args)]
pub struct Args {
    /// .psarc, .feedpak, .sng or .xml
    pub file: PathBuf,
    /// part: lead, rhythm, bass, combo, a number, or a prefix
    #[arg(long)]
    pub arr: Option<String>,
    /// difficulty between 0 and 1 (1 = the full chart)
    #[arg(long, default_value_t = 1.0)]
    pub mastery: f64,
    /// compare these chart spans instead of the sections: "a-b,c-d,..." (seconds)
    #[arg(long)]
    pub spans: Option<String>,
    /// the bars of this span ("a-b") that nothing in --spans resembles ("new here")
    #[arg(long)]
    pub novel: Option<String>,
    /// also list every bar with its time and notes
    #[arg(long)]
    pub bars: bool,
}

fn parse_span(s: &str) -> anyhow::Result<(f64, f64)> {
    let (a, b) = s
        .trim()
        .split_once('-')
        .ok_or_else(|| anyhow::anyhow!("a span is \"start-end\" in seconds, not {s:?}"))?;
    let (a, b): (f64, f64) = (a.trim().parse()?, b.trim().parse()?);
    if b <= a {
        anyhow::bail!("span {s:?} ends before it starts");
    }
    Ok((a, b))
}

pub fn parse_spans(s: &str) -> anyhow::Result<Vec<(f64, f64)>> {
    s.split(',')
        .filter(|p| !p.trim().is_empty())
        .map(parse_span)
        .collect()
}

fn short(label: &str) -> String {
    label.chars().take(10).collect()
}

pub fn run(arr: &Arrangement, a: Args) -> anyhow::Result<()> {
    let bars = Bars::new(arr);
    let labels = bars.labels();
    let groups = labels.iter().filter_map(|l| l.group).max().map_or(0, |g| g + 1);
    println!(
        "{} - {} ({}): {} bars, {} different bar patterns",
        arr.artist,
        arr.title,
        arr.name,
        bars.bars.len(),
        groups
    );
    println!(
        "(a letter is a bar pattern, the same letter the same notes; A+5 = pattern A five semitones up; - = no notes)"
    );
    if a.bars {
        println!();
        for (b, l) in bars.bars.iter().zip(&labels) {
            let like = match l.like {
                Some(k) if l.similarity < 0.999 => {
                    format!(" ~bar {} {:.2}", bars.bars[k].number, l.similarity)
                }
                _ => String::new(),
            };
            println!(
                "  bar {:>3}  {:>7.2}-{:<7.2} {:<5}{like:<16} {}",
                b.number,
                b.t0,
                b.t1,
                l.text(),
                bars.prints[b.index].describe()
            );
        }
    }
    let secs = similar::sections(arr, &bars);
    println!();
    for s in &secs {
        println!(
            "  {:<14} {:>7.2}-{:<7.2} bars {:>3}-{:<3}  {}",
            s.label,
            s.start,
            s.end,
            bars.bars[s.bars.start].number,
            bars.bars[s.bars.end - 1].number,
            row(&labels, s)
        );
    }
    let spans: Vec<(String, (f64, f64))> = match &a.spans {
        Some(sp) => parse_spans(sp)?
            .into_iter()
            .map(|(x, y)| (format!("{x:.1}-{y:.1}"), (x, y)))
            .collect(),
        None => secs.iter().map(|s| (s.label.clone(), (s.start, s.end))).collect(),
    };
    if spans.len() > 1 {
        println!("\nSimilarity (bar by bar, best alignment; 1 = the same notes)");
        print!("  {:<11}", "");
        for (i, _) in spans.iter().enumerate() {
            print!("{:>6}", i + 1);
        }
        println!();
        for (i, (la, sa)) in spans.iter().enumerate() {
            print!("  {:>2} {:<8}", i + 1, short(la));
            for (_, sb) in &spans {
                let c = bars.compare_spans(*sa, *sb, false);
                print!("{:>6.2}", c.score);
            }
            println!();
        }
    }
    if a.spans.is_none() {
        println!(
            "\nRepeats (each group's first section is the one to learn; bars counted within the section):"
        );
        for g in similar::section_groups(&bars, &secs) {
            let first = &secs[g.first];
            if g.repeats.is_empty() {
                println!("  {}: unlike the others", first.label);
                continue;
            }
            println!("  {}:", first.label);
            for (k, score, diff) in &g.repeats {
                let s = &secs[*k];
                let t = bars.compare_spans((first.start, first.end), (s.start, s.end), true);
                let what = if diff.is_empty() {
                    "the same".to_string()
                } else {
                    let at: Vec<String> = diff
                        .iter()
                        .filter_map(|&p| bars.bars.get(s.bars.start + p - 1))
                        .map(|b| format!("{:.1}", b.t0))
                        .collect();
                    format!(
                        "differs in bar{} {} (at {} s)",
                        if diff.len() > 1 { "s" } else { "" },
                        join(diff),
                        at.join(", ")
                    )
                };
                println!(
                    "    ~ {}: {score:.2}{}, {what}",
                    s.label,
                    if t.score > score + 0.005 {
                        format!(" ({:.2} transposed)", t.score)
                    } else {
                        String::new()
                    }
                );
            }
        }
    } else {
        let (l0, s0) = &spans[0];
        println!("\nAgainst {l0}:");
        for (l, s) in &spans[1..] {
            let c = bars.compare_spans(*s0, *s, false);
            let r = bars.in_span(s.0, s.1);
            let diff = similar::differing_positions(&c, &r);
            let t = bars.compare_spans(*s0, *s, true);
            println!(
                "  {l}: {:.2}{}{}",
                c.score,
                if t.score > c.score + 0.005 {
                    format!(" ({:.2} transposed)", t.score)
                } else {
                    String::new()
                },
                if diff.is_empty() {
                    ", the same".into()
                } else {
                    format!(
                        ", differs in its bar{} {}",
                        if diff.len() > 1 { "s" } else { "" },
                        join(&diff)
                    )
                }
            );
        }
    }
    if let Some(n) = &a.novel {
        let span = parse_span(n)?;
        let learned: Vec<(f64, f64)> = spans.iter().map(|s| s.1).filter(|_| a.spans.is_some()).collect();
        let novel = similar::novel_bars_in(&bars, &learned, span);
        println!(
            "\nNew in {:.1}-{:.1} s compared with {}: {} bar{}",
            span.0,
            span.1,
            if learned.is_empty() {
                "nothing"
            } else {
                "the spans"
            },
            novel.len(),
            if novel.len() == 1 { "" } else { "s" }
        );
        for b in &novel {
            println!(
                "  bar {:>3}  {:>7.2}-{:<7.2} {}% covered{}  {} new note{}",
                b.number,
                b.t0,
                b.t1,
                (b.best_similarity * 100.0).round().max(0.0) as i64,
                b.best_at.map(|t| format!(" (like {t:.1} s)")).unwrap_or_default(),
                b.new_events.len(),
                if b.new_events.len() == 1 { "" } else { "s" }
            );
        }
    }
    Ok(())
}

fn row(labels: &[BarLabel], s: &SectionBars) -> String {
    labels[s.bars.clone()]
        .iter()
        .map(BarLabel::text)
        .collect::<Vec<_>>()
        .join(" ")
}

fn join(v: &[usize]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
}
