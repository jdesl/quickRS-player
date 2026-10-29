//! Recording what you hear (P23.9): the output's final mix (the song, the synth, the jam, your guitar through the
//! rig) streamed to a 16-bit stereo WAV for as long as you like. New for quickRS.
//!
//! The output audio thread copies each block into the engine's record tap (`rt::record`, 20 s, made by
//! `Engine::set_record`); a thread here drains it ten times a second into a buffered file, so nothing on the audio
//! thread allocates, locks or touches the disk, and a 30-minute (or 6-hour) take costs 20 s of memory. The WAV's
//! sizes are written every few seconds, so a crash leaves a file that plays up to then. Paused, the thread reads
//! on and throws the frames away. An output reopened at another rate gives a new tap: the thread follows it and
//! converts to the file's rate (linear, good enough for a device switched mid-take).

use std::fs::File;
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, bounded};

use crate::Engine;
use crate::rt::record::RecordTap;

/// A RIFF file can't hold more than 4 GB: the recording stops a little before (6 h 12 min at 48 kHz).
const MAX_DATA_BYTES: u64 = 4_000_000_000;
/// How often the thread drains the tap, and how often the WAV's sizes are brought up to date.
const TICK: Duration = Duration::from_millis(100);
const HEADER_EVERY: Duration = Duration::from_secs(5);

/// What a finished recording left.
#[derive(Clone, Debug)]
pub struct Saved {
    pub path: PathBuf,
    /// Seconds in the file.
    pub seconds: f64,
    /// Seconds the thread fell behind and filled with silence (0 unless the machine stalled for 20 s).
    pub lost_seconds: f64,
    /// It stopped by itself at the WAV size limit.
    pub full: bool,
}

struct State {
    paused: AtomicBool,
    /// Frames in the file.
    frames: AtomicU64,
    lost: AtomicU64,
    rate: AtomicU32,
    full: AtomicBool,
}

pub struct Recorder {
    path: PathBuf,
    state: Arc<State>,
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<io::Result<()>>>,
    engine: Arc<Engine>,
}

impl Recorder {
    /// Start recording what the engine plays into a new WAV at `path` (its folder made if needed).
    pub fn start(engine: Arc<Engine>, path: PathBuf) -> io::Result<Recorder> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = File::create(&path)?;
        let tap = engine
            .set_record(true)
            .ok_or_else(|| io::Error::other("the engine has no record tap"))?;
        let rate = tap.rate;
        let mut w = BufWriter::with_capacity(1 << 20, file);
        if let Err(e) = write_header(&mut w, rate, 0) {
            engine.set_record(false);
            return Err(e);
        }
        let state = Arc::new(State {
            paused: AtomicBool::new(false),
            frames: AtomicU64::new(0),
            lost: AtomicU64::new(0),
            rate: AtomicU32::new(rate),
            full: AtomicBool::new(false),
        });
        let (tx, rx) = bounded::<()>(1);
        let (st, eng) = (state.clone(), engine.clone());
        let from = tap.written();
        let thread = std::thread::Builder::new()
            .name("recorder".into())
            .spawn(move || drain(eng, tap, from, w, st, rx))
            .inspect_err(|_| {
                engine.set_record(false);
            })?;
        Ok(Recorder {
            path,
            state,
            stop: Some(tx),
            thread: Some(thread),
            engine,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_paused(&self, paused: bool) {
        self.state.paused.store(paused, Ordering::Relaxed);
    }

    pub fn paused(&self) -> bool {
        self.state.paused.load(Ordering::Relaxed)
    }

    /// Seconds recorded so far (paused stretches left out).
    pub fn seconds(&self) -> f64 {
        self.state.frames.load(Ordering::Relaxed) as f64
            / self.state.rate.load(Ordering::Relaxed).max(1) as f64
    }

    /// It stopped by itself (the WAV size limit, or the disk failed): `stop` says which.
    pub fn ended(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| t.is_finished())
    }

    /// Stop: what's been played up to now written, the file closed.
    pub fn stop(mut self) -> io::Result<Saved> {
        self.finish()?;
        let rate = self.state.rate.load(Ordering::Relaxed).max(1) as f64;
        Ok(Saved {
            path: self.path.clone(),
            seconds: self.state.frames.load(Ordering::Relaxed) as f64 / rate,
            lost_seconds: self.state.lost.load(Ordering::Relaxed) as f64 / rate,
            full: self.state.full.load(Ordering::Relaxed),
        })
    }

    fn finish(&mut self) -> io::Result<()> {
        if let Some(tx) = self.stop.take() {
            let _ = tx.try_send(());
        }
        let r = match self.thread.take() {
            Some(t) => t
                .join()
                .unwrap_or_else(|_| Err(io::Error::other("the recorder's thread panicked"))),
            None => Ok(()),
        };
        self.engine.set_record(false);
        r
    }
}

impl Drop for Recorder {
    /// (dropped without `stop`: the file is still finished properly)
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.finish();
        }
    }
}

/// The 44-byte header of a 16-bit stereo PCM WAV with `data_bytes` of samples.
fn write_header(w: &mut impl Write, rate: u32, data_bytes: u64) -> io::Result<()> {
    let data = data_bytes.min(u32::MAX as u64 - 36) as u32;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes()); // PCM
    h.extend_from_slice(&2u16.to_le_bytes()); // stereo
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&(rate * 4).to_le_bytes());
    h.extend_from_slice(&4u16.to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    w.write_all(&h)
}

/// Make a recording left behind (the app crashed or was killed while it ran) whole: the header's sizes set from
/// what's in the file (the last patch may be up to 5 s old, or never made), a half frame at the end cut off.
/// Returns the seconds it holds. Only for this recorder's own files (a 44-byte header, 16-bit stereo).
pub fn repair_wav(path: &Path) -> io::Result<f64> {
    use std::io::Read;
    let mut f = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut h = [0u8; 44];
    if len < 44 || f.read_exact(&mut h).is_err() {
        return Err(io::Error::other("too short to be a recording"));
    }
    if &h[0..4] != b"RIFF" || &h[8..16] != b"WAVEfmt " || &h[36..40] != b"data" {
        return Err(io::Error::other("not a quickRS recording"));
    }
    let rate = u32::from_le_bytes([h[24], h[25], h[26], h[27]]);
    if rate == 0 {
        return Err(io::Error::other("no sample rate in the header"));
    }
    let data = ((len - 44) / 4 * 4).min(MAX_DATA_BYTES);
    f.seek(SeekFrom::Start(0))?;
    write_header(&mut f, rate, data)?;
    f.set_len(44 + data)?;
    f.sync_all()?;
    Ok(data as f64 / 4.0 / rate as f64)
}

/// Bring the header's sizes up to date (then carry on appending).
fn patch_header(w: &mut BufWriter<File>, rate: u32, data_bytes: u64) -> io::Result<()> {
    w.flush()?;
    let f = w.get_mut();
    f.seek(SeekFrom::Start(0))?;
    write_header(f, rate, data_bytes)?;
    f.seek(SeekFrom::End(0))?;
    Ok(())
}

/// Linear conversion from a tap at another rate to the file's (only after an output reopened at a new rate).
struct Convert {
    step: f64,
    pos: f64,
    last: [f32; 2],
}

impl Convert {
    fn run(&mut self, input: &[i16], out: &mut Vec<i16>) {
        let n = input.len() / 2;
        let at = |i: isize, c: usize, last: &[f32; 2]| -> f32 {
            if i < 0 {
                last[c]
            } else {
                input[2 * i as usize + c] as f32
            }
        };
        // positions in this block's frames (-1 = the last frame of the block before, `last`)
        while self.pos < n as f64 - 1.0 {
            let i = self.pos.floor() as isize;
            let t = (self.pos - i as f64) as f32;
            for c in 0..2 {
                let (a, b) = (at(i, c, &self.last), at(i + 1, c, &self.last));
                out.push((a + (b - a) * t).round() as i16);
            }
            self.pos += self.step;
        }
        if n > 0 {
            self.last = [input[2 * (n - 1)] as f32, input[2 * (n - 1) + 1] as f32];
            self.pos -= n as f64;
        }
    }
}

fn drain(
    engine: Arc<Engine>,
    mut tap: Arc<RecordTap>,
    mut cur: u64,
    mut w: BufWriter<File>,
    st: Arc<State>,
    stop: crossbeam_channel::Receiver<()>,
) -> io::Result<()> {
    let file_rate = st.rate.load(Ordering::Relaxed);
    let mut buf: Vec<i16> = Vec::new();
    let mut conv: Option<Convert> = None;
    let mut conv_out: Vec<i16> = Vec::new();
    let mut bytes: u64 = 0;
    let mut header_at = Instant::now();
    let result = (|| -> io::Result<()> {
        loop {
            let stopping = !matches!(
                stop.recv_timeout(TICK),
                Err(crossbeam_channel::RecvTimeoutError::Timeout)
            );
            // the output reopened: follow its new tap from its start
            if let Some(t) = engine.record_tap()
                && !Arc::ptr_eq(&t, &tap)
            {
                tap = t;
                cur = 0;
                conv = (tap.rate != file_rate).then(|| Convert {
                    step: tap.rate as f64 / file_rate as f64,
                    pos: 0.0,
                    last: [0.0; 2],
                });
            }
            let paused = st.paused.load(Ordering::Relaxed);
            // fell behind (a stall longer than the tap): silence where the frames were, so the time still adds up
            let oldest = tap.oldest();
            if cur < oldest {
                let gap = oldest - cur;
                st.lost.fetch_add(gap, Ordering::Relaxed);
                cur = oldest;
                if !paused {
                    let zeros = vec![0u8; (gap * 4).min(MAX_DATA_BYTES.saturating_sub(bytes)) as usize];
                    w.write_all(&zeros)?;
                    bytes += zeros.len() as u64;
                    st.frames.fetch_add(zeros.len() as u64 / 4, Ordering::Relaxed);
                }
            }
            let now = tap.written();
            buf.clear();
            if now > cur && tap.read_into(cur, now, &mut buf) {
                cur = now;
                if !paused {
                    let samples: &[i16] = match conv.as_mut() {
                        Some(c) => {
                            conv_out.clear();
                            c.run(&buf, &mut conv_out);
                            &conv_out
                        }
                        None => &buf,
                    };
                    let room = (MAX_DATA_BYTES.saturating_sub(bytes) / 4) as usize;
                    let frames = (samples.len() / 2).min(room);
                    let mut raw = Vec::with_capacity(frames * 4);
                    for s in &samples[..frames * 2] {
                        raw.extend_from_slice(&s.to_le_bytes());
                    }
                    w.write_all(&raw)?;
                    bytes += raw.len() as u64;
                    st.frames.fetch_add(frames as u64, Ordering::Relaxed);
                    if frames * 2 < samples.len() {
                        st.full.store(true, Ordering::Relaxed);
                        return Ok(());
                    }
                }
            }
            if header_at.elapsed() >= HEADER_EVERY {
                header_at = Instant::now();
                patch_header(&mut w, file_rate, bytes)?;
            }
            if stopping {
                return Ok(());
            }
        }
    })();
    // the header written last, whatever happened
    let closed = patch_header(&mut w, file_rate, bytes).and_then(|_| w.flush());
    result.and(closed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_reads_back() {
        let mut h = Vec::new();
        write_header(&mut h, 48000, 4 * 48000).unwrap();
        assert_eq!(h.len(), 44);
        let mut whole = h.clone();
        whole.extend(std::iter::repeat_n(0u8, 4 * 48000));
        let (rate, ch, s) = crate::decode::read_wav_i16(&whole).unwrap();
        assert_eq!((rate, ch, s.len()), (48000, 2, 2 * 48000));
    }

    #[test]
    fn a_left_behind_file_is_made_whole() {
        let dir = std::env::temp_dir().join(format!("qrs-repair-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("crashed.wav");
        // the header as written at the start (sizes 0), then 1.5 s and a half frame
        let mut bytes = Vec::new();
        write_header(&mut bytes, 48000, 0).unwrap();
        bytes.extend(std::iter::repeat_n(7u8, 4 * 72000 + 2));
        std::fs::write(&path, &bytes).unwrap();
        let secs = repair_wav(&path).unwrap();
        assert!((secs - 1.5).abs() < 1e-9, "{secs}");
        let (rate, ch, s) = crate::decode::read_wav_i16(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!((rate, ch, s.len()), (48000, 2, 2 * 72000));
        // what isn't one of ours is left alone
        std::fs::write(
            dir.join("x.wav"),
            b"not a wav at all, not at all, no, not one bit",
        )
        .unwrap();
        assert!(repair_wav(&dir.join("x.wav")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn conversion_keeps_the_length_and_the_shape() {
        // 44.1 kHz in, 48 kHz out, in odd blocks
        let mut c = Convert {
            step: 44100.0 / 48000.0,
            pos: 0.0,
            last: [0.0; 2],
        };
        let sig = |i: usize| ((i as f64 * 0.01).sin() * 10000.0) as i16;
        let input: Vec<i16> = (0..44100).flat_map(|i| [sig(i), sig(i)]).collect();
        let mut out = Vec::new();
        for block in input.chunks(2 * 333) {
            c.run(block, &mut out);
        }
        let frames = out.len() / 2;
        assert!((frames as i64 - 48000).abs() < 4, "{frames}");
        // the frame at 48 kHz index k is the input at k * 44100 / 48000
        for k in [1000usize, 20000, 40000] {
            let t = k as f64 * 44100.0 / 48000.0;
            let want = (t * 0.01).sin() * 10000.0;
            assert!(
                (out[2 * k] as f64 - want).abs() < 30.0,
                "{k}: {} vs {want}",
                out[2 * k]
            );
        }
    }
}
