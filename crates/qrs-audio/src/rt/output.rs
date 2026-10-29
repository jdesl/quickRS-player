//! The output callback's work: the song transport, preview clips, the monitored guitar, a soft limiter, the song
//! clock. Times given to commands are song seconds at the original tempo; the playing buffer is either the song
//! itself or a slowed-down copy whose frame f plays song time f / rate * speed. The song fades in when it starts and
//! out when it's paused (DECLICK), so neither clicks (note by note pauses it at every note).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::jam::Jam;
use super::{Clip, OutCmd, Ramp, Shared, StretchBuffer, Trash, discard};
use crate::clock::{SongClock, now_ns};
use crate::decode::SongBuffer;

/// Seconds the song fades over when it starts or is paused (at most a block: a shorter block fades over itself).
pub const DECLICK: f64 = 0.005;

/// Where the monitored guitar comes from.
pub enum MonitorSource {
    /// Handed to `process` by the duplex callback.
    Duplex,
    /// From another device's input callback, through a cushion kept at a steady fill.
    Cushion(Cushion),
    None,
}

/// The guitar's cushion between two devices' clocks: kept at a steady fill; when the clocks drift apart, one
/// sample more or less is taken per block (and stretched to the block, inaudibly) instead of leaving gaps. After a
/// stall it skips ahead; when it runs dry it goes quiet for a block and refills.
pub struct Cushion {
    /// Frames of (toned, dry) pairs.
    pub rx: rtrb::Consumer<f32>,
    /// Frames.
    pub target: usize,
    primed: bool,
    fill: f64,
    /// Frames waiting before the last pull (the looper's round trip counts them).
    pub last_fill: usize,
    scratch: Vec<f32>,
}

impl Cushion {
    pub fn new(rx: rtrb::Consumer<f32>, target: usize, max_block: usize) -> Cushion {
        Cushion {
            rx,
            target,
            primed: false,
            fill: 0.0,
            last_fill: 0,
            scratch: vec![0.0; (max_block + 8) * 2],
        }
    }

    /// Fill `out` (mono, `frames` samples); false when there's nothing to play this block.
    fn pull(&mut self, out: &mut [f32], dry: &mut [f32], gaps: &std::sync::atomic::AtomicU32) -> bool {
        let frames = out.len().min(dry.len());
        let target = self.target;
        let mut avail = self.rx.slots() / 2;
        if avail > target + 8 * frames {
            // long stall on the output side: skip ahead to the target fill
            if let Ok(c) = self.rx.read_chunk((avail - target) * 2) {
                c.commit_all();
            }
            avail = target;
        }
        self.last_fill = avail;
        if !self.primed {
            if avail < target {
                return false; // still filling
            }
            self.primed = true;
            self.fill = avail as f64;
        }
        if avail < frames {
            self.primed = false;
            gaps.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        self.fill = self.fill * 0.995 + avail as f64 * 0.005;
        let mut take = frames;
        if self.fill > (target + 48) as f64 && avail > frames {
            take = frames + 1;
        } else if self.fill < target as f64 - 48.0 && frames > 1 {
            take = frames - 1;
        }
        let take = take.min(self.scratch.len() / 2);
        let Ok(chunk) = self.rx.read_chunk(take * 2) else {
            return false;
        };
        let (a, b) = chunk.as_slices();
        self.scratch[..a.len()].copy_from_slice(a);
        self.scratch[a.len()..a.len() + b.len()].copy_from_slice(b);
        chunk.commit_all();
        let s = &self.scratch;
        if take == frames {
            for i in 0..frames {
                out[i] = s[2 * i];
                dry[i] = s[2 * i + 1];
            }
        } else {
            let scale = (take - 1) as f32 / (frames - 1).max(1) as f32;
            for i in 0..frames {
                let p = i as f32 * scale;
                let k = (p as usize).min(take - 1);
                let frac = p - k as f32;
                let k1 = (k + 1).min(take - 1);
                out[i] = s[2 * k] + (s[2 * k1] - s[2 * k]) * frac;
                dry[i] = s[2 * k + 1] + (s[2 * k1 + 1] - s[2 * k + 1]) * frac;
            }
        }
        true
    }
}

struct Preview {
    clip: Arc<Clip>,
    pos: usize,
    gain: f32,
    target: f32,
    looped: bool,
    /// Seconds for a full fade in or out.
    fade: f32,
    id: u64,
}

/// Mix a clip into `mix` (stereo); true when it has finished (played out, or faded to nothing).
fn mix_clip(p: &mut Preview, mix: &mut [f32], frames: usize, rate: u32, sg: f32) -> bool {
    let clip_frames = p.clip.frames();
    let fade_step = frames as f32 / (rate as f32 * p.fade);
    let g0 = p.gain;
    let g1 = if p.target > g0 {
        (g0 + fade_step).min(p.target)
    } else {
        (g0 - fade_step).max(p.target)
    };
    let mut i = 0;
    while i < frames {
        if p.pos >= clip_frames {
            if p.looped && clip_frames > 0 {
                p.pos = 0;
            } else {
                break;
            }
        }
        let n = (frames - i).min(clip_frames - p.pos);
        for k in 0..n {
            let g = (g0 + (g1 - g0) * (i + k) as f32 / frames as f32) * sg;
            mix[2 * (i + k)] += p.clip.samples[2 * (p.pos + k)] * g;
            mix[2 * (i + k) + 1] += p.clip.samples[2 * (p.pos + k) + 1] * g;
        }
        p.pos += n;
        i += n;
    }
    p.gain = g1;
    (g1 <= 0.0 && p.target <= 0.0) || (!p.looped && p.pos >= clip_frames)
}

/// What the output side starts with (carried over when devices are reopened).
#[derive(Clone)]
pub struct OutputState {
    pub song: Option<Arc<SongBuffer>>,
    pub stretched: Option<Arc<StretchBuffer>>,
    pub speed: f64,
    /// Song seconds.
    pub position: f64,
    pub playing: bool,
    pub song_gain: f32,
    pub guitar_gain: f32,
    pub monitor: bool,
}

impl Default for OutputState {
    fn default() -> Self {
        OutputState {
            song: None,
            stretched: None,
            speed: 1.0,
            position: 0.0,
            playing: false,
            song_gain: 0.8,
            guitar_gain: 0.8,
            monitor: true,
        }
    }
}

pub struct OutputSide {
    pub rate: u32,
    shared: Arc<Shared>,
    commands: rtrb::Consumer<OutCmd>,
    trash: rtrb::Producer<Trash>,
    monitor: MonitorSource,
    song: Option<Arc<SongBuffer>>,
    stretched: Option<Arc<StretchBuffer>>,
    speed: f64,
    /// Frames into the playing buffer (the song, or the stretched copy).
    frame: usize,
    playing: bool,
    /// The song starts from silence in the next block it plays.
    fade_in: bool,
    /// Paused: the next block fades the song out, then it stops.
    fade_out: bool,
    song_gain: Ramp,
    guitar_gain: Ramp,
    monitor_on: bool,
    preview: Option<Preview>,
    /// A clip that was replaced, fading out under the new one.
    preview_old: Option<Preview>,
    /// The newest preview id taken from the queue (published after the clip's state).
    preview_taken: u64,
    /// A seek mark taken from the queue, published with the next clock.
    mark: Option<u64>,
    /// Which output channels get the stereo mix (RS_ASIO's BaseChannel).
    pub base_channel: usize,
    /// Output latency beyond what the backend's timestamps include (seconds).
    pub extra_latency: f64,
    /// The guitar's round trip in frames (played to heard), for the looper: set by the duplex callback, or
    /// measured here through the cushion.
    pub guitar_delay: u64,
    /// The metronome, drums, backing, looper and Rewind.
    jam: Jam,
    /// The record tap (P23.9): what's heard, written after the limiter.
    record: Option<Arc<super::record::RecordTap>>,
    rec: Vec<f32>,
    mix: Vec<f32>,
    guitar: Vec<f32>,
    guitar_dry: Vec<f32>,
}

impl OutputSide {
    pub fn new(
        rate: u32,
        state: OutputState,
        shared: Arc<Shared>,
        commands: rtrb::Consumer<OutCmd>,
        trash: rtrb::Producer<Trash>,
        monitor: MonitorSource,
        max_block: usize,
    ) -> OutputSide {
        let mut o = OutputSide {
            rate,
            shared,
            commands,
            trash,
            monitor,
            song: state.song,
            stretched: state.stretched,
            speed: state.speed,
            frame: 0,
            playing: state.playing,
            fade_in: state.playing,
            fade_out: false,
            song_gain: Ramp::new(state.song_gain),
            guitar_gain: Ramp::new(state.guitar_gain),
            monitor_on: state.monitor,
            preview: None,
            preview_old: None,
            preview_taken: 0,
            mark: None,
            base_channel: 0,
            extra_latency: 0.0,
            guitar_delay: 0,
            jam: Jam::new(rate, max_block.max(64)),
            record: None,
            rec: vec![0.0; max_block.max(64) * 2],
            mix: vec![0.0; max_block.max(64) * 2],
            guitar: vec![0.0; max_block.max(64)],
            guitar_dry: vec![0.0; max_block.max(64)],
        };
        o.frame = o.frame_for(state.position);
        o.publish(now_ns(), 0);
        o
    }

    fn song_len(&self) -> usize {
        match (&self.stretched, &self.song) {
            (Some(s), _) => s.frames,
            (None, Some(s)) => s.frames(),
            _ => 0,
        }
    }

    fn duration(&self) -> f64 {
        self.song.as_ref().map(|s| s.duration()).unwrap_or(0.0)
    }

    fn frame_for(&self, t: f64) -> usize {
        let t = t.clamp(0.0, self.duration());
        (t / self.speed * self.rate as f64) as usize
    }

    fn time_of(&self, frame: usize) -> f64 {
        (frame as f64 / self.rate as f64 * self.speed).min(self.duration())
    }

    fn finished(&self) -> bool {
        self.song.is_none()
            || self.frame + (self.rate as usize / 20) >= self.song_len()
            || self.time_of(self.frame) >= self.duration() - 0.05
    }

    fn apply_commands(&mut self) {
        while let Ok(cmd) = self.commands.pop() {
            match cmd {
                OutCmd::Song(s) => {
                    if let Some(old) = std::mem::replace(&mut self.song, s) {
                        discard(&mut self.trash, Trash::Song(old));
                    }
                    if let Some(old) = self.stretched.take() {
                        discard(&mut self.trash, Trash::Stretch(old));
                    }
                    self.speed = 1.0;
                    self.frame = 0;
                    self.playing = false;
                    self.fade_in = false;
                    self.fade_out = false;
                }
                OutCmd::Replace(s) => {
                    if let Some(old) = self.song.replace(s) {
                        discard(&mut self.trash, Trash::Song(old));
                    }
                    self.frame = self.frame.min(self.song_len());
                }
                OutCmd::Play(from) => {
                    if self.song.is_some() {
                        if let Some(t) = from {
                            self.frame = self.frame_for(t);
                        } else if self.finished() {
                            self.frame = 0;
                        }
                        // from silence; or, if a pause hasn't faded it out yet, it just carries on
                        self.fade_in = !self.playing;
                        self.fade_out = false;
                        self.playing = true;
                    }
                }
                OutCmd::Pause => {
                    self.fade_out = self.playing;
                    self.fade_in = false;
                }
                OutCmd::Seek(t) => self.frame = self.frame_for(t),
                OutCmd::Mark(n) => self.mark = Some(n),
                OutCmd::Speed(speed, buf) => {
                    let t = self.time_of(self.frame);
                    if let Some(old) = std::mem::replace(&mut self.stretched, buf) {
                        discard(&mut self.trash, Trash::Stretch(old));
                    }
                    self.speed = if self.stretched.is_some() { speed } else { 1.0 };
                    self.frame = self.frame_for(t);
                }
                OutCmd::SongGain(g) => self.song_gain.target = g,
                OutCmd::GuitarGain(g) => self.guitar_gain.target = g,
                OutCmd::Monitor(m) => self.monitor_on = m,
                OutCmd::Preview {
                    clip,
                    fade,
                    looped,
                    id,
                } => {
                    let p = Preview {
                        clip,
                        pos: 0,
                        gain: if fade { 0.0 } else { 1.0 },
                        target: 1.0,
                        looped,
                        fade: 0.25,
                        id,
                    };
                    self.preview_taken = id;
                    if let Some(mut old) = self.preview.replace(p) {
                        // the clip being replaced fades out quickly under the new one (no pop on restart)
                        old.target = 0.0;
                        old.fade = 0.02;
                        if let Some(older) = self.preview_old.replace(old) {
                            discard(&mut self.trash, Trash::Clip(older.clip));
                        }
                    }
                }
                OutCmd::Cushion(c) => {
                    let new = match c {
                        Some(c) => MonitorSource::Cushion(*c),
                        None => MonitorSource::None,
                    };
                    if let MonitorSource::Cushion(old) = std::mem::replace(&mut self.monitor, new) {
                        discard(&mut self.trash, Trash::Cushion(Box::new(old)));
                    }
                }
                OutCmd::Jam(c) => self.jam.command(c, &mut self.trash),
                OutCmd::Record(t) => {
                    if let Some(old) = std::mem::replace(&mut self.record, t) {
                        discard(&mut self.trash, Trash::Record(old));
                    }
                }
                OutCmd::StopPreview { now } => {
                    if now {
                        for old in [self.preview.take(), self.preview_old.take()]
                            .into_iter()
                            .flatten()
                        {
                            discard(&mut self.trash, Trash::Clip(old.clip));
                        }
                    } else if let Some(p) = self.preview.as_mut() {
                        p.target = 0.0;
                    }
                }
            }
        }
    }

    /// Mix the song into the stereo scratch; returns false while the slowed-down audio here isn't ready. Fades it
    /// in on the block it starts and out on the block it's paused in (stopping where the fade ends).
    fn mix_song(&mut self, frames: usize) -> bool {
        if !self.playing {
            return true;
        }
        let start = self.frame;
        let len = self.song_len();
        let mut n = frames.min(len.saturating_sub(start));
        let fade = ((self.rate as f64 * DECLICK) as usize).clamp(1, frames.max(1));
        let (fade_in, fade_out) = (self.fade_in, self.fade_out);
        if fade_out {
            n = n.min(fade);
        }
        // the fade's gain at frame i of the block
        let env = |i: usize| -> f32 {
            if fade_out {
                (1.0 - (i + 1) as f32 / fade as f32).max(0.0)
            } else if fade_in {
                ((i + 1) as f32 / fade as f32).min(1.0)
            } else {
                1.0
            }
        };
        let (g0, step) = self.song_gain.block(frames);
        if let Some(st) = &self.stretched {
            if !st.is_ready(start, start + frames) {
                if fade_out {
                    // nothing is being heard: stop now
                    self.playing = false;
                    self.fade_out = false;
                    return true;
                }
                return false;
            }
            for i in 0..n {
                let g = (g0 + step * i as f32) * env(i);
                self.mix[2 * i] += st.sample(2 * (start + i)) * g;
                self.mix[2 * i + 1] += st.sample(2 * (start + i) + 1) * g;
            }
        } else if let Some(song) = &self.song {
            let s = &song.samples[2 * start..2 * (start + n)];
            for i in 0..n {
                let g = (g0 + step * i as f32) * env(i) / 32768.0;
                self.mix[2 * i] += s[2 * i] as f32 * g;
                self.mix[2 * i + 1] += s[2 * i + 1] as f32 * g;
            }
        }
        self.fade_in = false;
        if fade_out {
            self.frame = start + n;
            self.playing = false;
            self.fade_out = false;
            return true;
        }
        self.frame = start + frames;
        if self.frame >= len {
            self.frame = len;
            self.playing = false;
        }
        true
    }

    fn mix_preview(&mut self, frames: usize) {
        let sg = self.song_gain.value;
        if let Some(p) = self.preview_old.as_mut()
            && mix_clip(p, &mut self.mix, frames, self.rate, sg)
            && let Some(old) = self.preview_old.take()
        {
            discard(&mut self.trash, Trash::Clip(old.clip));
        }
        let Some(p) = self.preview.as_mut() else { return };
        let done = mix_clip(p, &mut self.mix, frames, self.rate, sg);
        self.shared.preview_pos.store(p.pos as u64, Ordering::Relaxed);
        self.shared
            .preview_len
            .store(p.clip.frames() as u64, Ordering::Relaxed);
        if done && let Some(old) = self.preview.take() {
            discard(&mut self.trash, Trash::Clip(old.clip));
        }
    }

    fn publish(&mut self, play_ns: u64, frames: usize) {
        let sh = &self.shared;
        let wall = (play_ns as f64 + self.extra_latency * 1e9).max(0.0) as u64;
        let buffering = sh.buffering.load(Ordering::Relaxed);
        let start_time =
            self.time_of(
                self.frame
                    .saturating_sub(if self.playing && !buffering { frames } else { 0 }),
            );
        let clock = SongClock {
            wall_ns: wall,
            song_time: if self.playing && !buffering {
                start_time
            } else {
                self.time_of(self.frame)
            },
            rate: if self.playing && !buffering {
                self.speed
            } else {
                0.0
            },
            limit: self.time_of(self.frame),
            playing: self.playing,
        };
        sh.clock.store(&clock);
        if let Some(n) = self.mark.take() {
            sh.seek_mark.store(n, Ordering::Release);
        }
        sh.frame.store(self.frame as u64, Ordering::Relaxed);
        sh.playing.store(self.playing, Ordering::Relaxed);
        sh.finished.store(self.finished(), Ordering::Relaxed);
        let live = self.preview.as_ref().filter(|p| p.target > 0.0);
        sh.previewing.store(live.is_some(), Ordering::Relaxed);
        sh.preview_id
            .store(live.map(|p| p.id).unwrap_or(0), Ordering::Release);
        sh.preview_started
            .fetch_max(self.preview_taken, Ordering::Release);
    }

    /// The device ran dry (the output wasn't refilled in time).
    pub fn note_dropout(&self) {
        self.shared.dropouts.fetch_add(1, Ordering::Relaxed);
    }

    /// Longest block handled in one go (longer ones are split).
    pub fn max_block(&self) -> usize {
        self.guitar.len()
    }

    /// A block of output of any length (split into pieces of at most `max_block` frames): `out` is interleaved
    /// with `channels` per frame; its first frame will be heard at `play_ns`.
    pub fn process(&mut self, out: &mut [f32], channels: usize, play_ns: u64) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let max = self.max_block();
        let mut f = 0;
        while f < frames {
            let n = (frames - f).min(max);
            let t = play_ns + (f as f64 / self.rate as f64 * 1e9) as u64;
            self.process_block(&mut out[f * channels..(f + n) * channels], channels, t, None);
            f += n;
        }
    }

    /// One block of at most `max_block` frames. `duplex_guitar` is the monitored guitar from the same callback's
    /// input (duplex mode).
    pub fn process_block(
        &mut self,
        out: &mut [f32],
        channels: usize,
        play_ns: u64,
        duplex_guitar: Option<(&[f32], &[f32])>,
    ) {
        let t0 = now_ns();
        self.apply_commands();
        let channels = channels.max(1);
        let frames = (out.len() / channels).min(self.guitar.len());
        let sh = self.shared.clone();
        sh.out_callbacks.fetch_add(1, Ordering::Relaxed);
        sh.last_out_ns.store(t0, Ordering::Relaxed);
        self.mix[..2 * frames].fill(0.0);

        let was_playing = self.playing;
        let ready = self.mix_song(frames);
        sh.buffering.store(was_playing && !ready, Ordering::Relaxed);
        self.mix_preview(frames);

        // the guitar (toned, and dry for the looper and Rewind)
        let mut have_guitar = false;
        match (&mut self.monitor, duplex_guitar) {
            (MonitorSource::Duplex, Some((g, d))) if !g.is_empty() => {
                let n = g.len().min(d.len()).min(frames);
                self.guitar[..n].copy_from_slice(&g[..n]);
                self.guitar_dry[..n].copy_from_slice(&d[..n]);
                self.guitar[n..frames].fill(0.0);
                self.guitar_dry[n..frames].fill(0.0);
                have_guitar = true;
            }
            (MonitorSource::Cushion(c), _) => {
                have_guitar = c.pull(
                    &mut self.guitar[..frames],
                    &mut self.guitar_dry[..frames],
                    &sh.mon_gaps,
                );
                if have_guitar {
                    // played (the newest sample, less what's waiting) to heard
                    let heard = play_ns as f64 + self.extra_latency * 1e9;
                    let played = sh.in_newest_ns.load(Ordering::Relaxed) as f64
                        - c.last_fill as f64 / self.rate as f64 * 1e9;
                    let d = ((heard - played).max(0.0) * 1e-9 * self.rate as f64) as u64;
                    // smoothed: the input's blocks arrive in bursts
                    self.guitar_delay = if self.guitar_delay == 0 {
                        d
                    } else {
                        (self.guitar_delay * 15 + d) / 16
                    };
                }
            }
            _ => {}
        }
        if !have_guitar {
            self.guitar[..frames].fill(0.0);
            self.guitar_dry[..frames].fill(0.0);
        }
        // the jam plays with the song, and takes the guitar as heard (whether it's monitored or not)
        self.jam.delay = self.guitar_delay;
        let wall = (play_ns as f64 + self.extra_latency * 1e9).max(0.0) as u64;
        self.jam.process(
            &mut self.mix[..2 * frames],
            &self.guitar[..frames],
            &self.guitar_dry[..frames],
            frames,
            wall,
            &sh.jam,
            &mut self.trash,
        );
        let (gg0, gstep) = self.guitar_gain.block(frames);
        if have_guitar && self.monitor_on {
            for i in 0..frames {
                let g = self.guitar[i] * (gg0 + gstep * i as f32);
                self.mix[2 * i] += g;
                self.mix[2 * i + 1] += g;
            }
        }

        // limit and write out; a recording takes the same, plus the guitar when it isn't heard from here (it's
        // monitored by the interface or an amp modeller: what you hear is still your guitar)
        let mut peak = 0.0f32;
        out.fill(0.0);
        let unheard = have_guitar && !self.monitor_on;
        for i in 0..frames {
            let l = soft_clip(self.mix[2 * i]);
            let r = soft_clip(self.mix[2 * i + 1]);
            if self.record.is_some() {
                let (a, b) = if unheard {
                    let g = self.guitar[i] * (gg0 + gstep * i as f32);
                    (soft_clip(self.mix[2 * i] + g), soft_clip(self.mix[2 * i + 1] + g))
                } else {
                    (l, r)
                };
                self.rec[2 * i] = a;
                self.rec[2 * i + 1] = b;
            }
            peak = peak.max(l.abs()).max(r.abs());
            let f = &mut out[i * channels..i * channels + channels];
            if channels == 1 {
                f[0] = 0.5 * (l + r);
            } else {
                let b = self.base_channel.min(channels - 2);
                f[b] = l;
                f[b + 1] = r;
            }
        }
        if let Some(tap) = &self.record {
            tap.write(&self.rec[..2 * frames]);
        }
        sh.out_peak.set(peak.max(sh.out_peak.get() * 0.95));
        self.publish(play_ns, frames);
        sh.note_work(now_ns().saturating_sub(t0));
    }
}

/// Transparent below 0.85, then a smooth knee that never exceeds 1.0 (instead of hard clipping).
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    const K: f32 = 0.85;
    let a = x.abs();
    if a <= K {
        x
    } else {
        let y = K + (1.0 - K) * ((a - K) / (1.0 - K)).tanh();
        y.copysign(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_clip_is_bounded_and_transparent() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert!(soft_clip(10.0) <= 1.0 && soft_clip(10.0) > 0.99);
        assert!((soft_clip(-0.9) + soft_clip(0.9)).abs() < 1e-7);
    }

    /// A loud steady song paused and started again: no step in the output bigger than the fade's, and the pause
    /// lands within the fade of where it was asked for.
    #[test]
    fn pause_and_play_fade_the_song() {
        let shared = Arc::new(Shared::new(12));
        let (mut tx, rx) = rtrb::RingBuffer::<OutCmd>::new(16);
        let (trash, _trash_rx) = rtrb::RingBuffer::<Trash>::new(16);
        let song = SongBuffer {
            rate: 48000,
            samples: Arc::new(vec![16384; 48000 * 2]),
        };
        let mut o = OutputSide::new(
            48000,
            OutputState {
                song: Some(Arc::new(song)),
                song_gain: 1.0,
                ..OutputState::default()
            },
            shared.clone(),
            rx,
            trash,
            MonitorSource::None,
            256,
        );
        let mut out = vec![0.0f32; 512];
        let (mut prev, mut worst) = (0.0f32, 0.0f32);
        let run = |o: &mut OutputSide, out: &mut Vec<f32>, prev: &mut f32, worst: &mut f32| {
            o.process(out, 2, 0);
            for f in out.chunks(2) {
                *worst = worst.max((f[0] - *prev).abs());
                *prev = f[0];
            }
        };
        assert!(tx.push(OutCmd::Play(None)).is_ok());
        for _ in 0..4 {
            run(&mut o, &mut out, &mut prev, &mut worst);
        }
        assert!((prev - 0.5).abs() < 1e-3, "playing at {prev}");
        let at = o.frame;
        assert!(tx.push(OutCmd::Pause).is_ok());
        run(&mut o, &mut out, &mut prev, &mut worst);
        assert!(!o.playing && prev == 0.0, "paused");
        let fade = (48000.0 * DECLICK) as usize;
        assert!(o.frame - at <= fade, "stopped {} frames on", o.frame - at);
        assert!(tx.push(OutCmd::Play(None)).is_ok());
        for _ in 0..2 {
            run(&mut o, &mut out, &mut prev, &mut worst);
        }
        assert!((prev - 0.5).abs() < 1e-3, "playing again at {prev}");
        // a cut would be a step of 0.5; the fades are 240 frames long
        assert!(worst < 0.5 / 200.0, "largest step {worst}");
        // paused and played again before the fade-out was heard: it just carries on
        assert!(tx.push(OutCmd::Pause).is_ok());
        assert!(tx.push(OutCmd::Play(None)).is_ok());
        run(&mut o, &mut out, &mut prev, &mut worst);
        assert!(o.playing && (prev - 0.5).abs() < 1e-3);
        assert!(worst < 0.5 / 200.0, "largest step {worst}");
    }

    #[test]
    fn a_replaced_clip_fades_out_and_ids_are_reported() {
        use std::sync::atomic::Ordering::Relaxed;
        let shared = Arc::new(Shared::new(12));
        let (mut tx, rx) = rtrb::RingBuffer::<OutCmd>::new(16);
        let (trash, _trash_rx) = rtrb::RingBuffer::<Trash>::new(16);
        let mut o = OutputSide::new(
            48000,
            OutputState::default(),
            shared.clone(),
            rx,
            trash,
            MonitorSource::None,
            256,
        );
        let dc = |v: f32| {
            Arc::new(Clip {
                rate: 48000,
                samples: vec![v; 48000 * 2],
            })
        };
        let preview = |clip, id| OutCmd::Preview {
            clip,
            fade: false,
            looped: true,
            id,
        };
        assert!(tx.push(preview(dc(0.5), 1)).is_ok());
        let mut out = vec![0.0f32; 512];
        o.process(&mut out, 2, 0);
        assert_eq!(shared.preview_id.load(Relaxed), 1);
        assert_eq!(shared.preview_started.load(Relaxed), 1);
        // the new clip starts from silence; the old one must not stop dead under it
        assert!(tx.push(preview(dc(0.0), 2)).is_ok());
        let mut prev = out[out.len() - 2];
        let mut worst = 0.0f32;
        for _ in 0..8 {
            o.process(&mut out, 2, 0);
            for f in out.chunks(2) {
                worst = worst.max((f[0] - prev).abs());
                prev = f[0];
            }
        }
        assert!(worst < 0.002, "largest step {worst}");
        assert_eq!(prev, 0.0, "the old clip has faded out");
        assert_eq!(shared.preview_id.load(Relaxed), 2);
        assert!(tx.push(OutCmd::StopPreview { now: true }).is_ok());
        o.process(&mut out, 2, 0);
        assert_eq!(shared.preview_id.load(Relaxed), 0);
        assert_eq!(shared.preview_started.load(Relaxed), 2);
    }

    /// A new stem mix replaces the song where it is: the playhead and play state stay, the old buffer goes to the
    /// trash (not freed here), and what's heard is the new mix.
    #[test]
    fn a_replaced_song_keeps_its_place() {
        use std::sync::atomic::Ordering::Relaxed;
        let shared = Arc::new(Shared::new(12));
        let (mut tx, rx) = rtrb::RingBuffer::<OutCmd>::new(16);
        let (trash, mut trash_rx) = rtrb::RingBuffer::<Trash>::new(16);
        let song = |v: i16| {
            Arc::new(SongBuffer {
                rate: 48000,
                samples: Arc::new(vec![v; 48000 * 2]),
            })
        };
        let mut o = OutputSide::new(
            48000,
            OutputState {
                song: Some(song(1000)),
                song_gain: 1.0,
                ..Default::default()
            },
            shared.clone(),
            rx,
            trash,
            MonitorSource::None,
            256,
        );
        assert!(tx.push(OutCmd::Play(Some(0.25))).is_ok());
        let mut out = vec![0.0f32; 512];
        o.process(&mut out, 2, 0);
        let before = shared.frame.load(Relaxed);
        assert!(tx.push(OutCmd::Replace(song(8000))).is_ok());
        o.process(&mut out, 2, 0);
        assert!(shared.playing.load(Relaxed));
        assert_eq!(shared.frame.load(Relaxed), before + 256);
        assert!((out[510] - 8000.0 / 32768.0).abs() < 0.01, "{}", out[510]);
        assert!(matches!(trash_rx.pop(), Ok(Trash::Song(_))));
    }

    /// The record tap (P23.9) gets exactly what's played, with no allocator call on the audio thread; a tap
    /// replaced or stopped leaves through the trash.
    #[test]
    fn the_record_tap_gets_what_is_heard_without_allocating() {
        use super::super::record::RecordTap;
        let shared = Arc::new(Shared::new(12));
        let (mut tx, rx) = rtrb::RingBuffer::<OutCmd>::new(16);
        let (trash, mut trash_rx) = rtrb::RingBuffer::<Trash>::new(16);
        let song = SongBuffer {
            rate: 48000,
            samples: Arc::new(
                (0..48000 * 2)
                    .map(|i| if (i / 2) % 3 == 0 { 16384 } else { -8000 })
                    .collect(),
            ),
        };
        let mut o = OutputSide::new(
            48000,
            OutputState {
                song: Some(Arc::new(song)),
                song_gain: 1.0,
                ..OutputState::default()
            },
            shared,
            rx,
            trash,
            MonitorSource::None,
            256,
        );
        let tap = Arc::new(RecordTap::new(48000, 1.0));
        assert!(tx.push(OutCmd::Play(None)).is_ok());
        assert!(tx.push(OutCmd::Record(Some(tap.clone()))).is_ok());
        let mut out = vec![0.0f32; 2 * 300];
        let mut heard: Vec<f32> = Vec::new();
        crate::alloc_count::watch();
        for _ in 0..20 {
            o.process(&mut out, 2, 0);
            // (the first blocks only: `heard` grows, which the counter would see)
            if heard.len() < 2 * 300 {
                crate::alloc_count::calls();
                heard.extend_from_slice(&out);
                crate::alloc_count::watch();
            }
        }
        let calls = crate::alloc_count::calls();
        assert_eq!(calls, 0, "allocator calls on the audio thread");
        assert_eq!(tap.written(), 20 * 300);
        let mut got = Vec::new();
        assert!(tap.read_into(0, 300, &mut got));
        for (k, (g, h)) in got.iter().zip(&heard).enumerate() {
            let want = (h * 32767.0).round() as i16;
            assert!((g - want).abs() <= 1, "{k}: {g} vs {want}");
        }
        assert!(heard.iter().any(|v| v.abs() > 0.1), "the song played");
        assert!(tx.push(OutCmd::Record(None)).is_ok());
        o.process(&mut out, 2, 0);
        assert_eq!(tap.written(), 20 * 300, "nothing more once stopped");
        assert!(matches!(trash_rx.pop(), Ok(Trash::Record(_))));
    }
}
