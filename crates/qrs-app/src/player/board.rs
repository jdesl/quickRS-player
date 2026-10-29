//! The Pedalboard tile and its big view (PLAN.md P14.3-P14.4, D26).
//!
//! The tile: the song's tones (the one playing lit; SONG follows the chart's tone changes), the chain as small
//! pedals (click one to show it, double-click or its light to switch it, drag to move it, + to add one), the
//! shown block's face with every control (drag a knob up or down, Shift for fine, the wheel for steps,
//! double-click for its default; ‹ › try the other models of its kind; the footswitch), the input and output
//! meters and DRY, which bypasses everything while held on to compare. The expand key opens the big view: the
//! whole board with its cables, a large editor with a picture of what the block does, a browser of all 54 models,
//! the starting boards and yours, Save as, an input level check.
//!
//! Edits go to the running board in place (tails ring on) and are kept when the mouse lets go: a song's tone in
//! userdata.json (`rigs`), your own board in config.json (`tone_board`).

use std::time::{Duration, Instant};

use eframe::egui;
use egui::epaint::CubicBezierShape;
use egui::{
    Align2, Color32, CursorIcon, Event, Key, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2,
};
use qrs_audio::board::picture::{self, Picture};
use qrs_audio::board::{self, Block, Board, Face, Kind, MAX_BLOCKS, MODELS, Unit, catalog};
use qrs_audio::tone::ToneSpec;
use qrs_ui::icons::Icon;
use qrs_ui::keys;
use qrs_ui::paint::{self, Fill};
use qrs_ui::text::{self, Style};
use qrs_ui::theme::{IVORY_INK, t};
use qrs_ui::units::px;
use qrs_ui::widgets::{self, RoundKind, interact};

use super::PlayerView;
use super::tiles::CLOSE_SLOT;
use crate::pedalboard::{self as pb, SongRig};
use crate::pedalfaces::{self as faces, Detail, Hot};
use crate::state::AppState;

mod back;
mod edit;
mod sounds;

/// A tone picked by hand on the tile (it holds until the song or part changes, or SONG is turned back on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    Tone(usize),
    Mine,
}

/// The block browser: where it adds (or which block it changes) and what it shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Picker {
    Kinds { at: usize },
    Models { at: usize, kind: Kind },
}

#[derive(Clone, Copy, Debug)]
struct KnobDrag {
    uid: u32,
    knob: usize,
    start: Pos2,
    u0: f32,
    across: bool,
    len: f32,
}

#[derive(Clone, Copy, Debug)]
struct MoveDrag {
    uid: u32,
    grab: f32,
    x: f32,
    moved: bool,
}

/// A control the wheel can turn: (block, knob, its rect).
#[derive(Clone, Copy, Debug)]
struct WheelSpot {
    uid: u32,
    knob: usize,
    rect: Rect,
}

pub struct BoardUi {
    /// The engine's clip count last seen, until when the input meter shows CLIP, and when it was last said.
    clips_seen: Option<u32>,
    clip_until: Option<Instant>,
    clip_said: Option<Instant>,
    pub rig: Option<SongRig>,
    rig_for: String,
    /// The song's tone in force now (followed from the chart).
    pub song_tone: usize,
    pub pick: Option<Pick>,
    mine: Option<Board>,
    /// The block shown on the tile and in the editor.
    pub sel: Option<u32>,
    pub picker: Option<Picker>,
    /// The big view is up.
    pub open: bool,
    drag: Option<KnobDrag>,
    moving: Option<MoveDrag>,
    dirty: bool,
    wheeled: Option<Instant>,
    /// Everything bypassed, to compare with the dry guitar.
    pub dry: bool,
    sent: Option<Board>,
    browse: Option<Kind>,
    /// Save as: the name being typed.
    pub naming: Option<String>,
    menu: bool,
    undo: Option<(Board, String)>,
    /// The input check: until when, and the highest peak heard.
    level: Option<(Instant, f32)>,
    level_note: Option<String>,
    wheel: Vec<WheelSpot>,
    tile_rect: Rect,
    /// The big view shows the selected block's editor (else the whole board).
    pub editing: bool,
    /// The editor's model list is open.
    model_menu: bool,
    /// A block's A/B: the slot not playing, and whether B is playing.
    ab: Option<Ab>,
    /// Edits that can be undone (the board before, what changed), newest last; and redone.
    hist: Vec<(Board, String)>,
    redo: Vec<(Board, String)>,
    /// The board as last kept, and which board that is (the history is per board).
    kept: Option<Board>,
    kept_for: String,
    /// The starting point last put on a block: (block, name).
    last_start: Option<(u32, &'static str)>,
    /// Tap tempo: the taps, and the tempo they make.
    taps: Vec<Instant>,
    tap_bpm: Option<f32>,
    /// The cab editor: the mic's angle on the speaker (it only sets how far from the centre), a drag on its pad.
    mic_angle: f32,
    pad_drag: Option<usize>,
    /// Screenshots: the editor's knob shown as hovered (QUICKRS_BOARD_HOVER, a knob key).
    shot_hover: Option<String>,
    /// The editor shows what's behind the panel (an amp's back, a drive's insides), for models that have it.
    pub back: bool,
    /// The back-panel control under the mouse last frame (the tip column beside the art explains it).
    back_hot: Option<usize>,
    /// How brightly the back panel's power tubes glow, following the board's output (smoothed).
    glow: f32,
    /// A board to hear instead of the one shown (the amp room's A/B against the saved sound, a sound previewed).
    pub audition: Option<Board>,
    /// The board is drawn on the amp room's stage (no aluminium board under it).
    pub stage: bool,
    /// Your saved sounds listed on the tile (P23.3), and what putting one on changed (for Back).
    sounds: Option<sounds::SoundPick>,
    sound_before: Option<sounds::Before>,
}

/// What kind of control a face's control is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ctl {
    Knob,
    Fader {
        across: bool,
    },
    Choice,
    /// The cab editor's mic pad (the Mic knob) and distance slider.
    Pad,
}

/// The control under the mouse (or being turned): its knob, where it is, what it is.
#[derive(Clone, Copy, Debug)]
struct HotCtl {
    knob: usize,
    rect: Rect,
    ctl: Ctl,
}

/// A block's two settings to compare.
#[derive(Clone, Copy, Debug)]
struct Ab {
    uid: u32,
    other: [f32; board::MAX_KNOBS],
    on_b: bool,
}

impl Default for BoardUi {
    fn default() -> Self {
        BoardUi {
            clips_seen: None,
            clip_until: None,
            clip_said: None,
            rig: None,
            rig_for: String::new(),
            song_tone: 0,
            pick: None,
            mine: None,
            sel: None,
            picker: None,
            open: false,
            drag: None,
            moving: None,
            dirty: false,
            wheeled: None,
            dry: false,
            sent: None,
            browse: None,
            naming: None,
            menu: false,
            undo: None,
            level: None,
            level_note: None,
            wheel: Vec::new(),
            tile_rect: Rect::NOTHING,
            editing: false,
            model_menu: false,
            ab: None,
            hist: Vec::new(),
            redo: Vec::new(),
            kept: None,
            kept_for: String::new(),
            last_start: None,
            taps: Vec::new(),
            tap_bpm: None,
            mic_angle: -0.6,
            pad_drag: None,
            shot_hover: None,
            back: false,
            back_hot: None,
            glow: 0.0,
            audition: None,
            stage: false,
            sounds: None,
            sound_before: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Source {
    Song(usize),
    Mine,
}

/// What the board's UI needs of the song it's shown with (nothing in the amp room: no song, the board is yours).
#[derive(Clone, Debug, Default)]
pub struct SongCtx {
    pub title: String,
    pub display_title: String,
    pub artist: String,
    pub sid: String,
    pub part_name: String,
    /// The part's tones only (`rig`, `base_tone`, `tones`): what `SongRig` reads.
    pub arr: qrs_formats::Arrangement,
    /// The chart's beats (the editor's tempo sync).
    pub btimes: Vec<f64>,
    /// The song's position now.
    pub pos: f64,
}

/// The pedalboard's state and UI (the tile, the big view, the block editor), owned by the player and by the amp
/// room (D47): everything it needs of its host is here, and what it says or asks goes out through `said`, `tip`
/// and `reqs` for the host to show.
pub struct Rack {
    pub board: BoardUi,
    pub song: SongCtx,
    pub engine: Option<std::sync::Arc<qrs_audio::Engine>>,
    /// The tooltip to draw this frame (text, key, where, anchor).
    pub tip: Option<(String, String, Pos2, Align2)>,
    /// Toasts to show: (text, tail).
    pub said: Vec<(String, String)>,
    pub reqs: Vec<super::PlayerReq>,
    /// What "My board" is called here (the amp room: the rig's name).
    pub title: Option<String>,
}

impl Rack {
    pub fn new(engine: Option<std::sync::Arc<qrs_audio::Engine>>) -> Rack {
        Rack {
            board: BoardUi::default(),
            song: SongCtx::default(),
            engine,
            tip: None,
            said: Vec::new(),
            reqs: Vec::new(),
            title: None,
        }
    }

    pub fn engine(&self) -> Option<&std::sync::Arc<qrs_audio::Engine>> {
        self.engine.as_ref()
    }

    pub fn say(&mut self, text: impl Into<String>, tail: impl Into<String>) {
        self.said.push((text.into(), tail.into()));
    }

    /// There's an edit to undo / to redo.
    pub fn can_undo(&self) -> bool {
        !self.board.hist.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.board.redo.is_empty()
    }
}

impl PlayerView {
    /// The rack in step with the player: the engine, the song's position, and the part when it changed.
    fn rack_sync(&mut self) {
        let e = self.transport.engine().cloned();
        let r = &mut self.rack;
        r.engine = e;
        r.song.pos = self.transport.position();
        if r.song.sid != self.sid || r.song.part_name != self.part_name || r.song.title != self.entry.title {
            r.song.title = self.entry.title.clone();
            r.song.display_title = self.entry.display_title();
            r.song.artist = self.entry.artist.clone();
            r.song.sid = self.sid.clone();
            r.song.part_name = self.part_name.clone();
            r.song.arr = qrs_formats::Arrangement {
                rig: self.chart.arr.rig.clone(),
                base_tone: self.chart.arr.base_tone.clone(),
                tones: self.chart.arr.tones.clone(),
                ..Default::default()
            };
        }
        if r.song.btimes.len() != self.chart.btimes.len() {
            r.song.btimes = self.chart.btimes.clone();
        }
    }

    /// What the rack said, its tooltip and its requests, to the player's.
    fn rack_drain(&mut self) {
        for (a, b) in std::mem::take(&mut self.rack.said) {
            self.say(a, b);
        }
        if let Some(t) = self.rack.tip.take() {
            self.tip = Some(t);
        }
        self.reqs.append(&mut self.rack.reqs);
    }

    pub fn board_update(&mut self, st: &mut AppState) {
        self.rack_sync();
        self.rack.board_update(st);
        self.rack_drain();
    }

    pub fn board_leave(&mut self, st: &mut AppState) {
        self.rack_sync();
        self.rack.board_leave(st);
        self.rack_drain();
    }

    pub(crate) fn board_use(&mut self, st: &mut AppState) {
        self.rack_sync();
        self.rack.board_use(st);
        self.rack_drain();
    }

    pub(crate) fn board_shot(&mut self, st: &mut AppState, preset: Option<&str>, sel: Option<&str>) {
        self.rack_sync();
        self.rack.board_shot(st, preset, sel);
        self.rack_drain();
    }

    pub(crate) fn board_sounds_shot(&mut self, st: &mut AppState, on: bool) {
        self.rack_sync();
        self.rack.sounds_shot(st, on);
        self.rack_drain();
    }

    pub fn board_wheel(&mut self, st: &mut AppState, pos: Pos2, notches: f32, shift: bool) -> bool {
        self.rack_sync();
        let used = self.rack.board_wheel(st, pos, notches, shift);
        self.rack_drain();
        used
    }

    pub fn board_key(&mut self, st: &mut AppState, ev: &Event) -> bool {
        let used = self.rack.board_key(st, ev);
        self.rack_drain();
        used
    }

    pub(super) fn draw_board_tile(
        &mut self,
        ui: &Ui,
        st: &mut AppState,
        x: f32,
        y: f32,
        w: f32,
        lifting: bool,
    ) -> f32 {
        self.rack_sync();
        let h = self.rack.draw_board_tile(ui, st, x, y, w, lifting);
        self.rack_drain();
        h
    }

    pub(crate) fn draw_board_view(&mut self, ui: &Ui, st: &mut AppState, area: Rect) {
        self.rack_sync();
        self.rack.draw_board_view(ui, st, area);
        self.rack_drain();
    }
}

// -- small drawing helpers --------------------------------------------------------------------------------------------

fn caps(p: &Painter, s: &str, pos: Pos2, size: f32, color: Color32, anchor: Align2) -> Rect {
    text::caps(p, s, pos, size, color, anchor, None)
}

/// A row of pills in a sunken track, the chosen one an ivory key. Returns each pill's rect.
fn pills(p: &Painter, track: Rect, labels: &[String], sel: Option<usize>, size: f32) -> Vec<Rect> {
    let th = t();
    let ctx = p.ctx();
    paint::inset(p, track, Some(10.0), 2.0, Fill::Base);
    let inner = track.shrink(px(3.0));
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| text::width(ctx, l, Style::caps(size, th.caption)) + px(14.0))
        .collect();
    let total: f32 = widths.iter().sum::<f32>().max(1.0);
    let k = inner.width() / total;
    let mut x = inner.min.x;
    let mut out = Vec::new();
    for (i, (l, w)) in labels.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(pos2(x, inner.min.y), vec2(w * k, inner.height()));
        if Some(i) == sel {
            keys::key_face(p, r, Some(7.0), false, false);
        }
        text::caps(
            p,
            l,
            r.center(),
            size,
            if Some(i) == sel { IVORY_INK } else { th.caption },
            Align2::CENTER_CENTER,
            Some(r.width() - px(4.0)),
        );
        out.push(r);
        x += w * k;
    }
    out
}

/// A small toggle key with a light: (rect, on).
fn lamp_key(p: &Painter, r: Rect, label: &str, on: bool, hot: bool) {
    let th = t();
    if on {
        keys::key_face(p, r, Some(9.0), false, false);
    } else {
        paint::inset(p, r, Some(9.0), 2.0, Fill::Base);
    }
    let lc = pos2(r.min.x + px(11.0), r.center().y);
    if on {
        paint::led(p, lc, 7.0, th.accent);
    } else {
        p.circle_filled(lc, 3.0, th.unlit);
    }
    caps(
        p,
        label,
        pos2(lc.x + px(8.0), r.center().y),
        10.0,
        if on { IVORY_INK } else { th.caption },
        Align2::LEFT_CENTER,
    );
    if hot {
        p.rect_stroke(
            r.expand(1.0),
            10.0,
            Stroke::new(1.0, th.accent.gamma_multiply(0.6)),
            egui::StrokeKind::Outside,
        );
    }
}

/// A level meter: a groove, the target band, the level, and a dB readout (CLIP while the input clips). The label
/// keeps clear of the readout at its widest: a device name after " · " that doesn't fit is cut to its first word
/// ("IN · FOCUSRITE"), or left off if even that doesn't fit (the header names the device too).
fn meter(p: &Painter, r: Rect, label: &str, peak: f32, target: bool, clip: bool) {
    let th = t();
    let db = 20.0 * peak.max(1e-6).log10();
    let ud = |d: f32| ((d + 60.0) / 60.0).clamp(0.0, 1.0);
    let ctx = p.ctx();
    let read_w = text::width(ctx, "-100 dB", Style::new(9.0, 800, th.caption));
    let room = (r.width() - read_w - px(8.0)).max(px(12.0));
    let fits = |s: &str| text::width(ctx, s, Style::caps(9.0, th.caption)) <= room;
    let mut label = label.to_string();
    if !fits(&label)
        && let Some((head, name)) = label.split_once(" \u{b7} ")
    {
        let word = name.split_whitespace().next().unwrap_or(name);
        let short = format!("{head} \u{b7} {word}");
        label = if fits(&short) { short } else { head.to_string() };
    }
    text::caps(
        p,
        &label,
        pos2(r.min.x, r.min.y + px(5.0)),
        9.0,
        th.caption,
        Align2::LEFT_CENTER,
        Some(room),
    );
    let color = if clip || db > -1.0 {
        th.miss
    } else if target && db > -18.0 {
        th.hit
    } else if target {
        th.late
    } else {
        th.hit
    };
    let txt = if clip {
        "CLIP".to_string()
    } else if peak < 1e-5 {
        "\u{2212}\u{221e}".to_string()
    } else {
        format!("{:.0} dB", db)
    };
    text::draw(
        p,
        &txt,
        pos2(r.max.x, r.min.y + px(5.0)),
        Align2::RIGHT_CENTER,
        Style::new(9.0, 800, if peak < 1e-5 { th.caption } else { color }),
        None,
    );
    let g = Rect::from_min_max(pos2(r.min.x, r.max.y - px(8.0)), r.max);
    paint::inset(p, g, Some(4.0), 1.5, Fill::Color(th.track));
    let inner = g.shrink(1.0);
    if target {
        let band = Rect::from_min_max(
            pos2(inner.min.x + inner.width() * ud(-18.0), inner.min.y),
            pos2(inner.min.x + inner.width() * ud(-6.0), inner.max.y),
        );
        p.rect_filled(band, 2.0, th.hit.gamma_multiply(0.18));
    }
    let fill = Rect::from_min_max(inner.min, pos2(inner.min.x + inner.width() * ud(db), inner.max.y));
    p.rect_filled(fill, 3.0, color);
}

/// A tone's label for a pill: its first word, at most six letters ("clean lead" → "CLEAN", "breakdown" → "BREAK").
fn short_tone(label: &str) -> String {
    let w = label.split(' ').next().unwrap_or(label).to_uppercase();
    if w.chars().count() > 6 {
        w.chars().take(5).collect()
    } else {
        w
    }
}

/// Strip a device name to its first words ("Focusrite USB ASIO", "Analogue 1 + 2").
fn short_device(n: &str) -> String {
    let n = n.split('(').next().unwrap_or(n).trim();
    let mut s: String = n.chars().take(22).collect();
    if n.chars().count() > 22 {
        s.push('\u{2026}');
    }
    s.to_uppercase()
}

/// What changed between two boards, in a few words ("Treble 6.0 \u{2192} 6.5", "Added Mean Green").
fn change_label(before: &Board, after: &Board) -> String {
    if before.blocks.len() < after.blocks.len()
        && let Some(n) = after.blocks.iter().find(|b| before.index_of(b.uid).is_none())
    {
        return format!("Added {}", n.def().name);
    }
    if before.blocks.len() > after.blocks.len()
        && let Some(o) = before.blocks.iter().find(|b| after.index_of(b.uid).is_none())
    {
        return format!("Took off {}", o.def().name);
    }
    for (i, a) in after.blocks.iter().enumerate() {
        let Some(j) = before.index_of(a.uid) else {
            continue;
        };
        let o = &before.blocks[j];
        if o.model != a.model {
            return format!("{} \u{2192} {}", o.def().name, a.def().name);
        }
        if o.on != a.on {
            return format!("{} {}", a.def().name, if a.on { "on" } else { "off" });
        }
        let def = a.def();
        let changed: Vec<usize> = (0..def.knobs.len())
            .filter(|&k| (o.v[k] - a.v[k]).abs() > 1e-6)
            .collect();
        match changed.as_slice() {
            [] => {}
            [k] => {
                let kd = def.knobs[*k];
                return format!("{} {} \u{2192} {}", kd.label, kd.text(o.v[*k]), kd.text(a.v[*k]));
            }
            ks => return format!("{} knobs of {}", ks.len(), def.name),
        }
        if i != j {
            return format!("Moved {}", def.name);
        }
    }
    if (before.level_db - after.level_db).abs() > 1e-6 {
        return "Board level".into();
    }
    "The board".into()
}

/// A block as a miniature of its face (the chain strips): selected, under the mouse, off.
fn mini(p: &Painter, r: Rect, b: &Block, sel: bool, hot: bool, dim: bool) {
    let th = t();
    let r = if sel { r.translate(vec2(0.0, -2.0)) } else { r };
    let l = faces::layout(b, r, Detail::Board);
    faces::miniature(p, r, b, px(90.0));
    if dim {
        paint::wash(p, r, Color32::BLACK, 0.5);
    }
    if sel {
        p.rect_stroke(
            r.expand(3.0),
            l.radius + 3.0,
            Stroke::new(2.0, th.accent),
            egui::StrokeKind::Outside,
        );
    } else if hot {
        p.rect_stroke(
            r.expand(2.5),
            l.radius + 2.5,
            Stroke::new(1.2, th.accent.gamma_multiply(0.6)),
            egui::StrokeKind::Outside,
        );
    }
}

impl Rack {
    // -- which board ------------------------------------------------------------------------------------------------

    fn source(&self, st: &AppState) -> Source {
        let n = self.board.rig.as_ref().map(|r| r.tones.len()).unwrap_or(0);
        match self.board.pick {
            Some(Pick::Mine) => Source::Mine,
            Some(Pick::Tone(i)) if i < n => Source::Song(i),
            _ if n > 0 && pb::follow(&st.config) => Source::Song(self.board.song_tone.min(n - 1)),
            _ => Source::Mine,
        }
    }

    fn mine(&mut self, st: &AppState) -> &mut Board {
        if self.board.mine.is_none() {
            self.board.mine = Some(pb::my_board(&st.config));
        }
        self.board.mine.get_or_insert_with(Board::default)
    }

    /// The board the tile shows (and the engine plays, when the tone is the pedalboard).
    pub fn board_now(&mut self, st: &AppState) -> Board {
        match self.source(st) {
            Source::Song(i) => self
                .board
                .rig
                .as_ref()
                .and_then(|r| r.tones.get(i))
                .map(|t| t.board.clone())
                .unwrap_or_default(),
            Source::Mine => self.mine(st).clone(),
        }
    }

    /// What's playing, as words: "Be Yourself · wah solo" or "My board".
    fn board_title(&self, st: &AppState) -> String {
        match self.source(st) {
            Source::Song(i) => {
                let label = self
                    .board
                    .rig
                    .as_ref()
                    .and_then(|r| r.tones.get(i))
                    .map(|t| format!("{}{}", t.label, if t.edited { " (yours)" } else { "" }))
                    .unwrap_or_default();
                format!("{} \u{b7} {label}", self.song.title)
            }
            Source::Mine => self.title.clone().unwrap_or_else(|| "My board".into()),
        }
    }

    pub(crate) fn board_edit(&mut self, st: &mut AppState, f: impl FnOnce(&mut Board)) {
        match self.source(st) {
            Source::Song(i) => {
                if let Some(t) = self.board.rig.as_mut().and_then(|r| r.tones.get_mut(i)) {
                    f(&mut t.board);
                }
            }
            Source::Mine => f(self.mine(st)),
        }
        self.board.dirty = true;
        self.board_send(st);
    }

    /// Keep the edits (a song's tone in userdata.json, your board in config.json).
    pub(crate) fn board_commit(&mut self, st: &mut AppState) {
        if !self.board.dirty {
            return;
        }
        self.board.dirty = false;
        self.board.wheeled = None;
        match self.source(st) {
            Source::Song(i) => {
                if let Some(r) = self.board.rig.as_mut()
                    && let Some(b) = r.tones.get(i).map(|t| t.board.clone())
                {
                    r.keep(i, &b, &mut st.userdata);
                }
            }
            Source::Mine => {
                let b = self.mine(st).clone();
                pb::set_my_board(&mut st.config, &b);
                st.save_config();
            }
        }
        // the change goes on the board's history (to undo)
        let key = self.source_key(st);
        if self.board.kept_for == key {
            let now = self.board_now(st);
            if let Some(before) = self.board.kept.take()
                && before != now
            {
                let label = change_label(&before, &now);
                self.board.hist.push((before, label));
                if self.board.hist.len() > 100 {
                    self.board.hist.remove(0);
                }
                self.board.redo.clear();
            }
            self.board.kept = Some(now);
        }
    }

    /// Which board is shown, as a key (the edit history is per board).
    fn source_key(&self, st: &AppState) -> String {
        format!("{}|{:?}", self.board.rig_for, self.source(st))
    }

    pub(crate) fn board_mode(&self) -> bool {
        self.engine()
            .is_some_and(|e| matches!(e.tone(), ToneSpec::Board { .. }))
    }

    /// Give the engine the board the tile shows (only when it changed).
    pub(crate) fn board_send(&mut self, st: &AppState) {
        let Some(e) = self.engine().cloned() else {
            return;
        };
        if !matches!(e.tone(), ToneSpec::Board { .. }) {
            self.board.sent = None;
            return;
        }
        let mut want = match &self.board.audition {
            Some(b) => b.clone(),
            None => self.board_now(st),
        };
        if self.board.dry {
            for b in &mut want.blocks {
                b.on = false;
            }
        }
        if self.board.sent.as_ref() != Some(&want) {
            e.set_tone(ToneSpec::Board { board: want.clone() });
            self.board.sent = Some(want);
        }
    }

    /// Switch the guitar's tone to the pedalboard.
    pub(crate) fn board_use(&mut self, st: &mut AppState) {
        let b = self.board_now(st);
        let spec = ToneSpec::Board { board: b.clone() };
        crate::tones::store(&mut st.config, &spec);
        st.save_config();
        if let Some(e) = self.engine() {
            e.set_tone(spec);
        }
        self.board.sent = Some(b);
        self.say("Tone: Pedalboard", "");
    }

    /// Screenshot mode: a starting board on "mine" and a block shown.
    pub(crate) fn board_shot(&mut self, st: &mut AppState, preset: Option<&str>, sel: Option<&str>) {
        // (the song's rig first, so the next frame doesn't start the tile over)
        self.board_update(st);
        if let Some(name) = preset
            && let Some((_, b)) = board::presets()
                .into_iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
        {
            self.board.pick = Some(Pick::Mine);
            self.board.mine = Some(b.renewed());
        }
        // QUICKRS_BOARD_CHAIN="amp_crunch,cab_4x12,...": a board of exactly these models (to see faces side by side)
        if let Ok(chain) = std::env::var("QUICKRS_BOARD_CHAIN") {
            let mut b = Board::default();
            for id in chain.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if let Some(blk) = Block::of(id) {
                    b.blocks.push(blk);
                }
            }
            self.board.pick = Some(Pick::Mine);
            self.board.mine = Some(b.renewed());
        }
        if let Some(id) = sel {
            let b = self.board_now(st);
            self.board.sel = b.blocks.iter().find(|x| x.def().id == id).map(|x| x.uid);
            self.board.editing = self.board.sel.is_some();
        }
        self.board.shot_hover = std::env::var("QUICKRS_BOARD_HOVER").ok();
        // QUICKRS_BOARD_TAB=back: the editor on the amp's back panel (a drive's insides)
        self.board.back = std::env::var("QUICKRS_BOARD_TAB").is_ok_and(|t| t.eq_ignore_ascii_case("back"));
        self.board.sent = None;
        self.board_send(st);
    }

    /// Leaving the player: your own board goes back on (when the tone is the pedalboard).
    pub fn board_leave(&mut self, st: &mut AppState) {
        self.board_commit(st);
        if self.board_mode() {
            let mine = pb::my_board(&st.config);
            if let Some(e) = self.engine() {
                e.set_tone(ToneSpec::Board { board: mine });
            }
        }
    }

    /// The interface clipped in the last moments (the input meter shows CLIP).
    fn clipping(&self) -> bool {
        self.board.clip_until.is_some_and(|t| Instant::now() < t)
    }

    /// Every frame: the song's rig, following its tone changes, the engine in step, the level check.
    pub fn board_update(&mut self, st: &mut AppState) {
        let key = format!("{}|{}", self.song.sid, self.song.part_name);
        if self.board.rig_for != key {
            self.board_commit(st);
            self.board.rig_for = key;
            self.board.rig = SongRig::new(&self.song.arr, &self.song.sid, &self.song.part_name, &st.userdata);
            self.sounds_forget();
            self.board.pick = None;
            self.board.sel = None;
            self.board.picker = None;
            let t = self.song.pos;
            self.board.song_tone = self.board.rig.as_ref().map(|r| r.at(t)).unwrap_or(0);
        }
        if self.board.pick.is_none() && pb::follow(&st.config) {
            let t = self.song.pos;
            if let Some(i) = self.board.rig.as_ref().map(|r| r.at(t))
                && i != self.board.song_tone
            {
                self.board_commit(st);
                self.board.song_tone = i;
                self.board.sel = None;
            }
        }
        if self.board.drag.is_none()
            && self
                .board
                .wheeled
                .is_some_and(|w| w.elapsed() > Duration::from_millis(700))
        {
            self.board_commit(st);
        }
        // a selection that went away: the amp, else the first block
        let b = self.board_now(st);
        if self.board.sel.is_none_or(|u| b.index_of(u).is_none()) {
            self.board.sel = b
                .first(Kind::Amp)
                .or((!b.blocks.is_empty()).then_some(0))
                .map(|i| b.blocks[i].uid);
        }
        // a board shown for the first time: what it is now is where undo stops
        let key = self.source_key(st);
        if self.board.kept_for != key {
            self.board.kept_for = key;
            self.board.kept = Some(b);
            self.board.hist.clear();
            self.board.redo.clear();
            self.board.ab = None;
            self.board.last_start = None;
        }
        self.board_send(st);
        // the interface clipping while the board plays: CLIP held on the input meter, and said every 30 s at most
        if let Some(clips) = self.engine().map(|e| e.meters().clips) {
            if self.board.clips_seen.is_some_and(|c| clips > c) && self.board_mode() {
                self.board.clip_until = Some(Instant::now() + Duration::from_millis(2500));
                if self
                    .board
                    .clip_said
                    .is_none_or(|t| t.elapsed() > Duration::from_secs(30))
                {
                    self.board.clip_said = Some(Instant::now());
                    self.say(
                        "Your guitar is clipping",
                        "  \u{b7}  turn the gain on your interface down a little",
                    );
                }
            }
            self.board.clips_seen = Some(clips);
        }
        if let Some((until, peak)) = self.board.level {
            let now = self.engine().map(|e| e.meters().peak).unwrap_or(0.0);
            let peak = peak.max(now);
            if Instant::now() >= until {
                self.board.level = None;
                let db = 20.0 * peak.max(1e-6).log10();
                let note = if peak < 1e-4 {
                    "Nothing heard: is the guitar plugged in, the INST button on?".to_string()
                } else if db > -1.0 {
                    format!(
                        "Peaks at {db:.0} dB: clipping, turn the interface's gain down until its ring stays green"
                    )
                } else if db > -4.0 {
                    format!(
                        "Peaks at {db:.0} dB: a little hot, a touch less gain (amber only on your hardest notes)"
                    )
                } else if db < -22.0 {
                    format!("Peaks at {db:.0} dB: quiet, turn the interface's gain up")
                } else if db < -14.0 {
                    format!("Peaks at {db:.0} dB: a little quiet, a touch more gain")
                } else {
                    format!("Peaks at {db:.0} dB: just right")
                };
                self.board.level_note = Some(note);
            } else {
                self.board.level = Some((until, peak));
            }
        }
    }

    // -- edits --------------------------------------------------------------------------------------------------------

    pub(crate) fn set_knob(&mut self, st: &mut AppState, uid: u32, knob: usize, v: f32) {
        self.board_edit(st, |b| {
            if let Some(i) = b.index_of(uid)
                && let Some(kd) = b.blocks[i].def().knobs.get(knob)
            {
                b.blocks[i].v[knob] = kd.clamp(v);
            }
        });
    }

    pub(crate) fn toggle_block(&mut self, st: &mut AppState, uid: u32) {
        self.board_edit(st, |b| {
            if let Some(i) = b.index_of(uid) {
                b.blocks[i].on = !b.blocks[i].on;
            }
        });
        self.board_commit(st);
    }

    fn change_model(&mut self, st: &mut AppState, uid: u32, model: usize) {
        self.board_edit(st, |b| {
            if let Some(i) = b.index_of(uid) {
                b.blocks[i] = b.blocks[i].changed_to(model);
            }
        });
        self.board_commit(st);
    }

    /// Step to the next (or previous) model of the block's kind.
    fn cycle_model(&mut self, st: &mut AppState, uid: u32, dir: i32) {
        let b = self.board_now(st);
        let Some(i) = b.index_of(uid) else { return };
        let kind = b.blocks[i].def().kind;
        let same: Vec<usize> = catalog::of_kind(kind).map(|(j, _)| j).collect();
        let at = same.iter().position(|&j| j == b.blocks[i].model).unwrap_or(0) as i32;
        let n = same.len() as i32;
        let next = same[((at + dir).rem_euclid(n)) as usize];
        self.change_model(st, uid, next);
    }

    pub(crate) fn add_block(&mut self, st: &mut AppState, at: usize, model: usize) {
        if self.board_now(st).blocks.len() >= MAX_BLOCKS {
            self.say("The board is full", "  \u{b7}  12 blocks at most");
            return;
        }
        let mut blk = Block::new(model);
        if let Some(v) = pb::model_default(&st.config, MODELS[model].id) {
            blk.v = v;
        }
        let uid = blk.uid;
        self.board_edit(st, |b| {
            let at = at.min(b.blocks.len());
            b.blocks.insert(at, blk);
        });
        self.board_commit(st);
        self.board.sel = Some(uid);
        self.board.picker = None;
        self.say(format!("Added {}", MODELS[model].shown_name()), "");
    }

    pub(crate) fn remove_block(&mut self, st: &mut AppState, uid: u32) {
        let b = self.board_now(st);
        let Some(i) = b.index_of(uid) else { return };
        let name = b.blocks[i].def().name;
        self.board_edit(st, |b| {
            b.blocks.remove(i);
        });
        self.board_commit(st);
        let b = self.board_now(st);
        self.board.sel = b
            .blocks
            .get(i.min(b.blocks.len().saturating_sub(1)))
            .map(|x| x.uid);
        self.say(format!("Took off {name}"), "");
    }

    pub(crate) fn move_block(&mut self, st: &mut AppState, uid: u32, to: usize) {
        self.board_edit(st, |b| {
            if let Some(i) = b.index_of(uid) {
                let blk = b.blocks.remove(i);
                let to = to.min(b.blocks.len());
                b.blocks.insert(to, blk);
            }
        });
    }

    /// Put a whole board in (a starting board, a saved one); the one it replaces can be had back.
    pub(crate) fn load_board(&mut self, st: &mut AppState, name: &str, b: Board) {
        let before = self.board_now(st);
        self.board.undo = Some((before, self.board_title(st)));
        let fresh = b.renewed();
        self.board_edit(st, |cur| *cur = fresh);
        self.board_commit(st);
        self.board.sel = None;
        self.say(format!("{name} board on"), "  \u{b7}  Undo in the big view");
    }

    fn pick_tone(&mut self, st: &mut AppState, pick: Pick) {
        self.board_commit(st);
        self.board.pick = Some(pick);
        self.board.sel = None;
        self.board.picker = None;
    }

    fn set_follow(&mut self, st: &mut AppState, on: bool) {
        self.board_commit(st);
        pb::set_follow(&mut st.config, on);
        st.save_config();
        self.board.pick = None;
        self.board.sel = None;
        if on {
            let t = self.song.pos;
            self.board.song_tone = self.board.rig.as_ref().map(|r| r.at(t)).unwrap_or(0);
        }
    }

    /// The wheel over a control turns it (true: the wheel was used here).
    pub fn board_wheel(&mut self, st: &mut AppState, pos: Pos2, notches: f32, shift: bool) -> bool {
        if !self.board.open && self.sounds_wheel(pos, notches) {
            return true;
        }
        let Some(spot) = self.board.wheel.iter().find(|w| w.rect.contains(pos)).copied() else {
            return false;
        };
        let b = self.board_now(st);
        let Some(i) = b.index_of(spot.uid) else {
            return false;
        };
        let blk = b.blocks[i];
        let Some(kd) = blk.def().knobs.get(spot.knob) else {
            return false;
        };
        let v = match kd.unit {
            Unit::Choice(c) => {
                let n = c.len() as f32;
                (blk.v[spot.knob] + notches.signum()).rem_euclid(n)
            }
            _ => kd.from_u(kd.to_u(blk.v[spot.knob]) + notches * if shift { 0.005 } else { 0.025 }),
        };
        self.set_knob(st, spot.uid, spot.knob, v);
        self.board.wheeled = Some(Instant::now());
        true
    }

    /// Typing a name for Save as (true: the event was used).
    pub fn board_key(&mut self, st: &mut AppState, ev: &Event) -> bool {
        let Some(name) = self.board.naming.as_mut() else {
            // (the sounds list, while it's open and the big view isn't)
            return !self.board.open && self.sounds_key(st, ev);
        };
        match ev {
            Event::Text(s) => {
                if name.chars().count() < 40 {
                    name.push_str(s);
                }
                true
            }
            Event::Key {
                key: Key::Backspace,
                pressed: true,
                ..
            } => {
                name.pop();
                true
            }
            Event::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } => {
                self.board.naming = None;
                true
            }
            Event::Key {
                key: Key::Enter,
                pressed: true,
                ..
            } => {
                let n = name.trim().to_string();
                self.board.naming = None;
                if !n.is_empty() {
                    let b = self.board_now(st);
                    pb::save_as(&mut st.config, &n, &b);
                    st.save_config();
                    self.say(format!("Saved \u{201c}{n}\u{201d}"), "  \u{b7}  in Presets");
                }
                true
            }
            _ => false,
        }
    }

    // -- a block's face, played with the mouse ----------------------------------------------------------------------

    /// A block's face, played with the mouse. Returns the control under the mouse or being turned.
    fn face_ui(
        &mut self,
        ui: &Ui,
        st: &mut AppState,
        blk: Block,
        r: Rect,
        detail: Detail,
        tag: &'static str,
    ) -> Option<HotCtl> {
        let ctx = ui.ctx().clone();
        let def = blk.def();
        let l = faces::layout(&blk, r, detail);
        let mut hot = Hot::default();
        let mut shapes: Vec<(usize, Rect, Ctl)> = Vec::new();
        for &(k, c, d) in &l.knobs {
            shapes.push((k, Rect::from_center_size(c, Vec2::splat(d)), Ctl::Knob));
        }
        for &(k, tr) in &l.faders {
            let across = def.face == Face::Cab;
            shapes.push((k, tr, Ctl::Fader { across }));
        }
        let shift = ctx.input(|i| i.modifiers.shift);
        let spots: Vec<(usize, Rect, bool, f32)> = l
            .knobs
            .iter()
            .map(|&(k, c, d)| (k, Rect::from_center_size(c, Vec2::splat(d + 10.0)), false, 160.0))
            .chain(l.faders.iter().map(|&(k, tr)| {
                let across = def.face == Face::Cab;
                let hit = if across {
                    tr.expand2(vec2(8.0, 9.0))
                } else {
                    tr.expand2(vec2(10.0, 7.0))
                };
                (k, hit, across, if across { tr.width() } else { tr.height() })
            }))
            .collect();
        for &(k, cr) in &l.choices {
            shapes.push((k, cr, Ctl::Choice));
        }
        for (k, hit, across, len) in spots {
            self.board.wheel.push(WheelSpot {
                uid: blk.uid,
                knob: k,
                rect: hit,
            });
            let resp = interact(ui, hit, (tag, "knob", blk.uid, k), Sense::click_and_drag());
            let kd = def.knobs[k];
            if resp.hovered() || resp.dragged() {
                hot.hover = Some(k);
                ctx.set_cursor_icon(if across {
                    CursorIcon::ResizeHorizontal
                } else {
                    CursorIcon::ResizeVertical
                });
            }
            if resp.double_clicked() {
                self.board.drag = None;
                self.set_knob(st, blk.uid, k, kd.def);
                self.board_commit(st);
                continue;
            }
            let mine = self.board.drag.is_some_and(|d| d.uid == blk.uid && d.knob == k);
            if resp.is_pointer_button_down_on()
                && !mine
                && let Some(pos) = resp.interact_pointer_pos()
            {
                // a click on a fader's track jumps there
                let u0 = if def.face == Face::Graphic || across {
                    let tr = l.faders.iter().find(|f| f.0 == k).map(|f| f.1).unwrap_or(hit);
                    if across {
                        ((pos.x - tr.min.x) / tr.width()).clamp(0.0, 1.0)
                    } else {
                        ((tr.max.y - pos.y) / tr.height()).clamp(0.0, 1.0)
                    }
                } else {
                    kd.to_u(blk.v[k])
                };
                if def.face == Face::Graphic || across {
                    self.set_knob(st, blk.uid, k, kd.from_u(u0));
                }
                self.board.drag = Some(KnobDrag {
                    uid: blk.uid,
                    knob: k,
                    start: pos,
                    u0,
                    across,
                    len,
                });
            }
            if let Some(d) = self.board.drag
                && d.uid == blk.uid
                && d.knob == k
            {
                hot.active = Some(k);
                if resp.is_pointer_button_down_on() || resp.dragged() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        let moved = if d.across {
                            pos.x - d.start.x
                        } else {
                            d.start.y - pos.y
                        };
                        let fine = if shift { 0.25 } else { 1.0 };
                        let u = (d.u0 + moved / d.len.max(20.0) * fine).clamp(0.0, 1.0);
                        let v = kd.from_u(u);
                        if (v - blk.v[k]).abs() > 1e-6 {
                            self.set_knob(st, blk.uid, k, v);
                        }
                    }
                } else {
                    self.board.drag = None;
                    self.board_commit(st);
                }
            }
        }
        for &(k, cr) in &l.choices {
            self.board.wheel.push(WheelSpot {
                uid: blk.uid,
                knob: k,
                rect: cr,
            });
            let resp = interact(ui, cr.expand(3.0), (tag, "choice", blk.uid, k), Sense::click());
            if resp.hovered() {
                hot.hover = Some(k);
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked()
                && let Unit::Choice(opts) = def.knobs[k].unit
            {
                let n = opts.len();
                let v = if def.face == Face::Amp {
                    if blk.v[k] >= 0.5 { 0.0 } else { 1.0 }
                } else {
                    let x = resp.interact_pointer_pos().map(|p| p.x).unwrap_or(cr.center().x);
                    (((x - cr.min.x) / cr.width() * n as f32).floor()).clamp(0.0, n as f32 - 1.0)
                };
                self.set_knob(st, blk.uid, k, v);
                self.board_commit(st);
            }
        }
        if l.switch.is_positive() {
            let resp = interact(ui, l.switch.expand(4.0), (tag, "switch", blk.uid), Sense::click());
            if resp.hovered() {
                hot.switch = true;
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.toggle_block(st, blk.uid);
            }
        }
        // (screenshots: a knob shown as hovered in the editor)
        if tag == "editor"
            && hot.hover.is_none()
            && hot.active.is_none()
            && let Some(key) = self.board.shot_hover.as_deref()
        {
            hot.hover = def.knob_index(key).filter(|k| shapes.iter().any(|s| s.0 == *k));
        }
        // the face drawn with the values as they now are
        let now = self.board_now(st);
        let shown = now.index_of(blk.uid).map(|i| now.blocks[i]).unwrap_or(blk);
        faces::draw(ui.painter(), r, &shown, &l, detail, hot);
        let k = hot.active.or(hot.hover)?;
        let &(_, rect, ctl) = shapes.iter().find(|s| s.0 == k)?;
        Some(HotCtl { knob: k, rect, ctl })
    }

    // -- the chain strip ------------------------------------------------------------------------------------------

    /// The chain in `well`, each block a miniature of its face; with `names`, each named under it (the editor's
    /// strip).
    fn chain_ui(&mut self, ui: &Ui, st: &mut AppState, well: Rect, tag: &'static str, names: bool) {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        let b = self.board_now(st);
        let name_h = if names { px(15.0) } else { 0.0 };
        let h0 = (well.height() - px(14.0) - name_h).max(px(20.0));
        let cy = well.min.y + px(7.0) + h0 / 2.0;
        let jack = |x: f32, right: bool| {
            let c = pos2(x, cy);
            p.rect_filled(
                Rect::from_center_size(c, vec2(px(8.0), px(8.0))),
                2.0,
                Color32::from_rgb(109, 122, 135),
            );
            let tip = if right {
                c + vec2(px(6.0), 0.0)
            } else {
                c - vec2(px(6.0), 0.0)
            };
            p.rect_filled(
                Rect::from_center_size(tip, vec2(px(4.0), px(3.0))),
                1.0,
                Color32::from_rgb(154, 166, 178),
            );
        };
        jack(well.min.x + px(12.0), true);
        jack(well.max.x - px(12.0), false);
        let gap = px(if names { 16.0 } else { 9.0 });
        let add_w = px(26.0);
        let widths: Vec<f32> = b
            .blocks
            .iter()
            .map(|x| {
                let s = faces::board_size(x);
                h0 * s.x / s.y.max(1.0)
            })
            .collect();
        let total: f32 = widths.iter().sum::<f32>() + gap * (b.blocks.len() as f32 + 1.0) + add_w;
        let avail = well.width() - px(44.0);
        let k = (avail / total).min(1.0);
        let h = h0 * k;
        // (room to spare: the blocks spread out, up to four times the gap)
        let gk = if k >= 1.0 {
            (gap + (avail - total) / (b.blocks.len() as f32 + 1.0)).min(gap * 4.0)
        } else {
            gap * k
        };
        let mut x = well.min.x + px(22.0);
        let cable = |x0: f32, x1: f32| {
            p.line_segment(
                [pos2(x0, cy), pos2(x1, cy)],
                Stroke::new(2.0, Color32::from_rgb(71, 89, 107)),
            );
        };
        let mut chips: Vec<(u32, Rect)> = Vec::new();
        cable(well.min.x + px(16.0), x);
        for (blk, w) in b.blocks.iter().zip(&widths) {
            x += gk;
            let r = Rect::from_min_size(pos2(x, cy - h / 2.0), vec2(w * k, h));
            cable(x - gk, x);
            chips.push((blk.uid, r));
            x += w * k;
        }
        // + adds after the shown block (or at the end)
        x += gk;
        cable(x - gk, x);
        let add = Rect::from_center_size(pos2(x + add_w * 0.5, cy), Vec2::splat(add_w));
        cable(add.max.x, well.max.x - px(16.0));
        // the chip being dragged follows the mouse; where it would land shows as a gap in the order
        let moving = self.board.moving;
        for (i, (uid, r)) in chips.iter().enumerate() {
            let blk = b.blocks[i];
            let resp = interact(ui, *r, (tag, "chip", *uid), Sense::click_and_drag());
            let sel = self.board.sel == Some(*uid);
            let hovered = resp.hovered();
            if hovered {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
                self.tip = Some((
                    format!("{} \u{b7} after {}", blk.def().shown_name(), blk.def().after),
                    if blk.on {
                        "click to show \u{b7} double-click: off".into()
                    } else {
                        "off \u{b7} double-click: on".into()
                    },
                    pos2(r.center().x, well.max.y + px(4.0)),
                    Align2::CENTER_TOP,
                ));
            }
            if resp.double_clicked() {
                self.toggle_block(st, *uid);
            } else if resp.clicked() {
                self.board.sel = Some(*uid);
                self.board.picker = None;
            }
            if resp.drag_started()
                && let Some(pos) = resp.interact_pointer_pos()
            {
                self.board.moving = Some(MoveDrag {
                    uid: *uid,
                    grab: pos.x - r.min.x,
                    x: r.min.x,
                    moved: false,
                });
            }
            if let Some(m) = self.board.moving.as_mut()
                && m.uid == *uid
            {
                if resp.dragged() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        m.x = pos.x - m.grab;
                        m.moved = true;
                    }
                } else if !resp.is_pointer_button_down_on() {
                    // let go: the chips whose centres are left of the dropped one's come before it
                    let centre = m.x + r.width() / 2.0;
                    let to = chips
                        .iter()
                        .filter(|(u, cr)| *u != *uid && cr.center().x < centre)
                        .count();
                    let moved = m.moved;
                    self.board.moving = None;
                    if moved && b.index_of(*uid) != Some(to) {
                        self.move_block(st, *uid, to);
                        self.board_commit(st);
                    }
                }
            }
            let draw_r = match moving {
                Some(m) if m.uid == *uid && m.moved => {
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                    Rect::from_min_size(pos2(m.x, r.min.y - px(4.0)), r.size())
                }
                _ => *r,
            };
            mini(&p, draw_r, &blk, sel, hovered, !blk.on || self.board.dry);
            if names {
                text::draw(
                    &p,
                    blk.def().name,
                    pos2(r.center().x, well.max.y - px(10.0)),
                    Align2::CENTER_CENTER,
                    Style::new(9.5, 800, if sel { th.accent_ink } else { th.caption }),
                    Some(r.width() + gk - px(2.0)),
                );
            }
            // a selected chip's remove key
            if sel && hovered && moving.is_none() {
                let xr =
                    Rect::from_center_size(pos2(r.max.x - px(2.0), r.min.y + px(2.0)), Vec2::splat(px(16.0)));
                if widgets::round_button(
                    ui,
                    xr,
                    (tag, "remove", *uid),
                    Icon::XBold,
                    8.0,
                    RoundKind::Gem(keys::Gem::Ruby),
                    false,
                )
                .clicked()
                {
                    self.remove_block(st, *uid);
                    return;
                }
            }
        }
        let resp = interact(ui, add, (tag, "add"), Sense::click());
        let hot = resp.hovered();
        p.rect_stroke(
            add,
            px(6.0),
            Stroke::new(
                1.5,
                if hot {
                    th.accent
                } else {
                    Color32::from_rgb(71, 89, 107)
                },
            ),
            egui::StrokeKind::Inside,
        );
        widgets::draw_round_button(
            &p,
            Rect::from_center_size(add.center(), Vec2::splat(px(18.0))),
            Icon::Plus,
            9.0,
            RoundKind::Ivory,
            false,
            hot,
            false,
        );
        if hot {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            let at = self
                .board
                .sel
                .and_then(|u| b.index_of(u))
                .map(|i| i + 1)
                .unwrap_or(b.blocks.len());
            self.board.picker = Some(Picker::Kinds { at });
        }
    }

    // -- the tile ---------------------------------------------------------------------------------------------------

    /// The Pedalboard tile at (x, y), `w` wide. Returns its bottom.
    pub(super) fn draw_board_tile(
        &mut self,
        ui: &Ui,
        st: &mut AppState,
        x: f32,
        y: f32,
        w: f32,
        lifting: bool,
    ) -> f32 {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        self.board.wheel.clear();
        let (pad_x, pad_y) = (px(10.0), px(12.0));
        let (head, tones_h, strip_h, title_h, face_h, foot_h) =
            (px(24.0), px(30.0), px(74.0), px(22.0), px(178.0), px(32.0));
        let gap = px(10.0);
        let height = pad_y
            + head
            + px(8.0)
            + tones_h
            + gap
            + strip_h
            + gap
            + title_h
            + px(6.0)
            + face_h
            + gap
            + foot_h
            + pad_y;
        let card = Rect::from_min_size(pos2(x, y), vec2(w, height));
        self.board.tile_rect = card;
        super::tiles::tile_card(&p, card, lifting);
        let inner_x0 = card.min.x + pad_x;
        let inner_x1 = card.max.x - pad_x;
        // header: the title, what's playing, the expand key
        let hy = card.min.y + pad_y + head / 2.0;
        let tt = caps(
            &p,
            "PEDALBOARD",
            pos2(card.min.x + px(16.0), hy),
            12.0,
            th.caption,
            Align2::LEFT_CENTER,
        );
        let kd = px(26.0);
        let key = Rect::from_min_size(
            pos2(card.max.x - px(CLOSE_SLOT) - kd, hy - kd / 2.0),
            vec2(kd, kd),
        );
        // your saved sounds (the amp room's), beside the expand key
        let star = key.translate(vec2(-(kd + px(8.0)), 0.0));
        let resp = widgets::round_button(
            ui,
            star,
            "board-sounds",
            Icon::Star,
            12.0,
            RoundKind::Ivory,
            self.sounds_open(),
        );
        if resp.hovered() {
            self.tip = Some((
                "Your sounds".into(),
                "Shift T".into(),
                pos2(star.center().x, star.max.y + px(4.0)),
                Align2::CENTER_TOP,
            ));
        }
        if resp.clicked() {
            self.sounds_toggle();
        }
        let mode = self.board_mode();
        let status = if mode {
            self.board_title(st)
        } else {
            "not on: your tone is ".to_string()
                + &crate::player::tone_label(&self.engine().map(|e| e.tone()).unwrap_or_default())
        };
        text::draw(
            &p,
            &status,
            pos2(tt.max.x + px(10.0), hy),
            Align2::LEFT_CENTER,
            Style::new(13.0, 800, if mode { th.accent_ink } else { th.caption }),
            Some(star.min.x - tt.max.x - px(18.0)),
        );
        if widgets::round_button(
            ui,
            key,
            "board-expand",
            Icon::Expand,
            12.0,
            RoundKind::Ivory,
            false,
        )
        .clicked()
        {
            self.board.open = true;
            self.board.menu = false;
        }
        // the tones row
        let ty = card.min.y + pad_y + head + px(8.0);
        let song_w = px(62.0);
        let has_rig = self.board.rig.is_some();
        let follow = pb::follow(&st.config);
        let src = self.source(st);
        let track_r = Rect::from_min_max(
            pos2(inner_x0, ty),
            pos2(
                if has_rig {
                    inner_x1 - song_w - px(8.0)
                } else {
                    inner_x1
                },
                ty + tones_h,
            ),
        );
        let (labels, sel): (Vec<String>, Option<usize>) = match &self.board.rig {
            Some(rig) => {
                let mut l: Vec<String> = rig.tones.iter().map(|t| short_tone(&t.label)).collect();
                l.push("MINE".into());
                let sel = match src {
                    Source::Song(i) => Some(i),
                    Source::Mine => Some(rig.tones.len()),
                };
                (l, sel)
            }
            None => (
                vec![
                    "MINE".into(),
                    "CLEAN".into(),
                    "CRUNCH".into(),
                    "LEAD".into(),
                    "METAL".into(),
                    "MORE".into(),
                ],
                Some(0),
            ),
        };
        let rects = pills(&p, track_r, &labels, sel, 9.0);
        for (i, r) in rects.iter().enumerate() {
            let resp = interact(ui, *r, ("board-tone", i), Sense::click());
            if resp.hovered()
                && let Some(t) = self.board.rig.as_ref().and_then(|rg| rg.tones.get(i))
            {
                self.tip = Some((
                    format!("{}{}", t.label, if t.edited { " (yours)" } else { "" }),
                    t.board
                        .blocks
                        .iter()
                        .map(|b| b.def().short)
                        .collect::<Vec<_>>()
                        .join(" \u{203a} "),
                    pos2(r.center().x, r.max.y + px(4.0)),
                    Align2::CENTER_TOP,
                ));
            }
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if !resp.clicked() {
                continue;
            }
            match self.board.rig.as_ref().map(|r| r.tones.len()) {
                Some(n) if i < n => self.pick_tone(st, Pick::Tone(i)),
                Some(_) => self.pick_tone(st, Pick::Mine),
                None => match i {
                    0 => {}
                    5 => {
                        self.board.open = true;
                        self.board.menu = true;
                    }
                    _ => {
                        let name = ["Clean", "Crunch", "Lead", "Metal"][i - 1];
                        if let Some((_, b)) = board::presets().into_iter().find(|(n, _)| *n == name) {
                            self.load_board(st, name, b);
                        }
                    }
                },
            }
        }
        if has_rig {
            let sr = Rect::from_min_max(pos2(inner_x1 - song_w, ty), pos2(inner_x1, ty + tones_h));
            let resp = interact(ui, sr, "board-follow", Sense::click());
            let on = follow && self.board.pick.is_none();
            lamp_key(&p, sr, "SONG", on, resp.hovered());
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
                self.tip = Some((
                    "Follow the song's tone changes".into(),
                    String::new(),
                    pos2(sr.center().x, sr.max.y + px(4.0)),
                    Align2::CENTER_TOP,
                ));
            }
            if resp.clicked() {
                self.set_follow(st, !on);
            }
        }
        // the chain
        let sy = ty + tones_h + gap;
        let well = Rect::from_min_max(pos2(inner_x0, sy), pos2(inner_x1, sy + strip_h));
        paint::inset(&p, well, Some(12.0), 2.0, Fill::Base);
        let sel_before = self.board.sel;
        self.chain_ui(ui, st, well, "tile", false);
        // (a block clicked on the chain, or + for the browser: the list makes way)
        if self.board.picker.is_some() || (self.board.sel != sel_before && sel_before.is_some()) {
            self.board.sounds = None;
        }
        // the shown block (or the browser, or your sounds)
        let fy = sy + strip_h + gap;
        let area = Rect::from_min_max(
            pos2(inner_x0 + px(4.0), fy),
            pos2(inner_x1 - px(4.0), fy + title_h + px(6.0) + face_h),
        );
        if self.board.sounds.is_some() {
            self.sounds_ui(ui, st, area);
        } else if let Some(picker) = self.board.picker {
            self.picker_ui(ui, st, area, picker, "tile");
        } else {
            let b = self.board_now(st);
            match self.board.sel.and_then(|u| b.index_of(u)).map(|i| b.blocks[i]) {
                Some(blk) => {
                    let def = blk.def();
                    let tr = Rect::from_min_size(area.min, vec2(area.width(), title_h));
                    let ks = px(22.0);
                    let lk = Rect::from_min_size(pos2(tr.min.x, tr.center().y - ks / 2.0), Vec2::splat(ks));
                    let rk =
                        Rect::from_min_size(pos2(tr.max.x - ks, tr.center().y - ks / 2.0), Vec2::splat(ks));
                    if widgets::round_button(
                        ui,
                        lk,
                        ("board-prev", blk.uid),
                        Icon::Left,
                        9.0,
                        RoundKind::Ivory,
                        false,
                    )
                    .clicked()
                    {
                        self.cycle_model(st, blk.uid, -1);
                    }
                    if widgets::round_button(
                        ui,
                        rk,
                        ("board-next", blk.uid),
                        Icon::Right,
                        9.0,
                        RoundKind::Ivory,
                        false,
                    )
                    .clicked()
                    {
                        self.cycle_model(st, blk.uid, 1);
                    }
                    let kind = def.kind.label().to_uppercase();
                    let kr = caps(
                        &p,
                        &kind,
                        pos2(lk.max.x + px(8.0), tr.center().y),
                        10.0,
                        faces::kind_colour(def.kind),
                        Align2::LEFT_CENTER,
                    );
                    let nr = text::draw(
                        &p,
                        &def.shown_name(),
                        pos2(kr.max.x + px(8.0), tr.center().y),
                        Align2::LEFT_CENTER,
                        Style::new(13.0, 800, th.text),
                        None,
                    );
                    text::draw(
                        &p,
                        &format!("after {}", def.after),
                        pos2(nr.max.x + px(8.0), tr.center().y),
                        Align2::LEFT_CENTER,
                        Style::new(11.0, 600, th.caption),
                        Some(rk.min.x - nr.max.x - px(14.0)),
                    );
                    let fr =
                        Rect::from_min_size(pos2(area.min.x, tr.max.y + px(6.0)), vec2(area.width(), face_h));
                    self.face_ui(ui, st, blk, fr, Detail::Full, "tile");
                }
                None => {
                    text::draw(
                        &p,
                        "An empty board: + adds a pedal, an amp, a cab\u{2026}",
                        area.center(),
                        Align2::CENTER_CENTER,
                        Style::new(13.0, 600, th.caption),
                        Some(area.width()),
                    );
                }
            }
            if !mode {
                // not playing: say so over the face, with the way to turn it on
                paint::wash(&p, area, th.screen, 0.72);
                // only an amp or modeller makes the sound itself; a microphone or a dry input is just heard as is
                let modeller = crate::calibrate::current_input(st)
                    .is_some_and(|(id, name)| crate::calibrate::is_modeller_input(&st.config, &id, &name));
                text::draw(
                    &p,
                    if modeller {
                        "Your tone is As is: the amp modeller makes the sound."
                    } else {
                        "Your tone is As is: your input is heard as it comes in."
                    },
                    area.center() - vec2(0.0, px(20.0)),
                    Align2::CENTER_CENTER,
                    Style::new(13.0, 700, th.text_2),
                    Some(area.width() - px(20.0)),
                );
                let bw = px(190.0);
                let br = Rect::from_center_size(area.center() + vec2(0.0, px(18.0)), vec2(bw, px(36.0)));
                if widgets::button(
                    ui,
                    br,
                    "board-use",
                    "Use the pedalboard",
                    "",
                    widgets::ButtonStyle::primary(),
                )
                .clicked()
                {
                    self.board_use(st);
                }
            }
        }
        // meters and DRY
        let my = area.max.y + gap;
        let (in_peak, out_peak, device) = match self.engine() {
            Some(e) => {
                let m = e.meters();
                let name = e
                    .status()
                    .input
                    .map(|i| short_device(&i.device_name))
                    .unwrap_or_else(|| "NO INPUT".into());
                (m.peak, m.tone_peak, name)
            }
            None => (0.0, 0.0, "NO INPUT".into()),
        };
        let dry_w = px(62.0);
        let mw = (inner_x1 - inner_x0 - dry_w - px(20.0)) / 2.0;
        meter(
            &p,
            Rect::from_min_size(pos2(inner_x0 + px(4.0), my), vec2(mw - px(4.0), foot_h - px(4.0))),
            &format!("IN \u{b7} {device}"),
            in_peak,
            true,
            self.clipping(),
        );
        meter(
            &p,
            Rect::from_min_size(
                pos2(inner_x0 + mw + px(10.0), my),
                vec2(mw - px(4.0), foot_h - px(4.0)),
            ),
            "OUT",
            out_peak,
            false,
            false,
        );
        let dr = Rect::from_min_size(
            pos2(inner_x1 - dry_w, my + px(2.0)),
            vec2(dry_w, foot_h - px(6.0)),
        );
        let resp = interact(ui, dr, "board-dry", Sense::click());
        lamp_key(&p, dr, "DRY", self.board.dry, resp.hovered());
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            self.tip = Some((
                "Hear your dry guitar (every block off) to compare".into(),
                String::new(),
                pos2(dr.center().x, dr.max.y + px(4.0)),
                Align2::RIGHT_TOP,
            ));
        }
        if resp.clicked() {
            self.board.dry = !self.board.dry;
            self.board_send(st);
        }
        card.max.y
    }

    // -- the browser ------------------------------------------------------------------------------------------------

    fn picker_ui(&mut self, ui: &Ui, st: &mut AppState, area: Rect, picker: Picker, tag: &'static str) {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        paint::display(&p, area, Some(12.0), 4.0);
        let inner = area.shrink(px(10.0));
        let title = match picker {
            Picker::Kinds { .. } => "ADD A BLOCK".to_string(),
            Picker::Models { kind, .. } => format!("ADD \u{b7} {}", kind.label().to_uppercase()),
        };
        caps(
            &p,
            &title,
            pos2(inner.min.x, inner.min.y + px(7.0)),
            10.0,
            th.caption,
            Align2::LEFT_CENTER,
        );
        let cancel = Rect::from_min_size(
            pos2(inner.max.x - px(60.0), inner.min.y),
            vec2(px(60.0), px(16.0)),
        );
        let cr = interact(ui, cancel, (tag, "pick-cancel"), Sense::click());
        text::draw(
            &p,
            if matches!(picker, Picker::Models { .. }) {
                "\u{2039} Back"
            } else {
                "Cancel"
            },
            cancel.right_center(),
            Align2::RIGHT_CENTER,
            Style::new(11.0, 700, if cr.hovered() { th.accent_ink } else { th.caption }),
            None,
        );
        if cr.clicked() {
            self.board.picker = match picker {
                Picker::Models { at, .. } => Some(Picker::Kinds { at }),
                _ => None,
            };
            return;
        }
        let body = Rect::from_min_max(pos2(inner.min.x, inner.min.y + px(22.0)), inner.max);
        match picker {
            Picker::Kinds { at } => {
                let (cols, rows) = (5, 2);
                let cw = body.width() / cols as f32;
                let rh = body.height() / rows as f32;
                for (i, kind) in Kind::ALL.iter().enumerate() {
                    let r = Rect::from_min_size(
                        pos2(
                            body.min.x + cw * (i % cols) as f32,
                            body.min.y + rh * (i / cols) as f32,
                        ),
                        vec2(cw, rh),
                    )
                    .shrink(px(3.0));
                    let resp = interact(ui, r, (tag, "kind", i), Sense::click());
                    let col = kind.colour();
                    p.rect_filled(r.translate(vec2(0.0, 2.0)), 9.0, Color32::from_black_alpha(90));
                    p.rect_filled(r, 9.0, faces::rgb(col));
                    p.rect_filled(
                        Rect::from_min_size(r.min + vec2(3.0, 2.0), vec2(r.width() - 6.0, 2.0)),
                        1.0,
                        Color32::from_white_alpha(70),
                    );
                    let ink = faces::ink(col);
                    faces::print(
                        &p,
                        kind.label(),
                        r.center() - vec2(0.0, px(7.0)),
                        Align2::CENTER_CENTER,
                        12.0,
                        ink,
                        r.width() - 6.0,
                    );
                    let n = catalog::of_kind(*kind).count();
                    text::draw(
                        &p,
                        &format!("{n} models"),
                        r.center() + vec2(0.0, px(10.0)),
                        Align2::CENTER_CENTER,
                        Style::new(9.0, 700, ink.gamma_multiply(0.8)),
                        None,
                    );
                    if resp.hovered() {
                        ctx.set_cursor_icon(CursorIcon::PointingHand);
                        p.rect_stroke(
                            r.expand(2.0),
                            10.0,
                            Stroke::new(1.5, th.accent),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if resp.clicked() {
                        self.board.picker = Some(Picker::Models { at, kind: *kind });
                    }
                }
            }
            Picker::Models { at, kind } => {
                let models: Vec<(usize, &board::ModelDef)> = catalog::of_kind(kind).collect();
                let cols = if models.len() > 5 { 2 } else { 1 };
                let rows = models.len().div_ceil(cols);
                let cw = body.width() / cols as f32;
                let rh = (body.height() / rows.max(1) as f32).min(px(34.0));
                for (n, (i, m)) in models.iter().enumerate() {
                    let r = Rect::from_min_size(
                        pos2(
                            body.min.x + cw * (n / rows) as f32,
                            body.min.y + rh * (n % rows) as f32,
                        ),
                        vec2(cw, rh),
                    )
                    .shrink2(vec2(px(2.0), px(1.0)));
                    let resp = interact(ui, r, (tag, "model", *i), Sense::click());
                    if resp.hovered() {
                        p.rect_filled(r, 8.0, th.screen_hi);
                        ctx.set_cursor_icon(CursorIcon::PointingHand);
                        self.tip = Some((
                            m.blurb.to_string(),
                            String::new(),
                            pos2(r.center().x, r.max.y + 2.0),
                            Align2::CENTER_TOP,
                        ));
                    }
                    let sw = Rect::from_min_size(
                        pos2(r.min.x + px(4.0), r.center().y - px(10.0)),
                        vec2(px(14.0), px(20.0)),
                    );
                    p.rect_filled(sw, 3.0, faces::rgb(m.colour));
                    p.rect_stroke(
                        sw,
                        3.0,
                        Stroke::new(1.0, Color32::from_black_alpha(120)),
                        egui::StrokeKind::Inside,
                    );
                    text::draw(
                        &p,
                        m.name,
                        pos2(sw.max.x + px(8.0), r.center().y - px(6.0)),
                        Align2::LEFT_CENTER,
                        Style::new(12.0, 800, th.text),
                        Some(r.max.x - sw.max.x - px(10.0)),
                    );
                    text::draw(
                        &p,
                        &format!("after {}", m.after),
                        pos2(sw.max.x + px(8.0), r.center().y + px(7.0)),
                        Align2::LEFT_CENTER,
                        Style::new(9.5, 600, th.caption),
                        Some(r.max.x - sw.max.x - px(10.0)),
                    );
                    if resp.clicked() {
                        self.add_block(st, at, *i);
                    }
                }
            }
        }
    }

    // -- the big view -----------------------------------------------------------------------------------------------

    pub(crate) fn draw_board_view(&mut self, ui: &Ui, st: &mut AppState, area: Rect) {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        self.board.wheel.clear();
        paint::dim(&p, area, Some(215));
        // the editor needs the editing block (an add goes back to the board, where the browser is)
        if self.board.picker.is_some() {
            self.board.editing = false;
        }
        let editing = self.board.editing && {
            let b = self.board_now(st);
            self.board.sel.is_some_and(|u| b.index_of(u).is_some())
        };
        let (wmax, hmax) = if editing { (1320.0, 860.0) } else { (1260.0, 800.0) };
        let w = (area.width() - px(60.0)).min(px(wmax));
        let h = (area.height() - px(40.0)).min(px(hmax));
        let panel = Rect::from_center_size(area.center(), vec2(w, h));
        // a click outside closes it
        let outside = interact(ui, area, "board-view-back", Sense::click());
        if outside.clicked()
            && let Some(pos) = outside.interact_pointer_pos()
            && !panel.contains(pos)
        {
            self.board_commit(st);
            self.board.open = false;
            return;
        }
        paint::floating(&p, panel, Some(26.0));
        let inner = panel.shrink(px(20.0));
        let foot_h = px(40.0);
        if editing {
            self.editor_view(ui, st, inner, foot_h);
            self.view_footer(ui, inner, foot_h);
            return;
        }
        // header
        let hdr = Rect::from_min_size(inner.min, vec2(inner.width(), px(52.0)));
        let sub = match &self.board.rig {
            Some(_) => format!(
                "PEDALBOARD \u{b7} {} \u{b7} {}",
                self.song.title.to_uppercase(),
                self.song.part_name.to_uppercase()
            ),
            None => "PEDALBOARD".into(),
        };
        caps(
            &p,
            &sub,
            pos2(hdr.min.x, hdr.min.y + px(9.0)),
            11.0,
            th.caption,
            Align2::LEFT_CENTER,
        );
        let title = match &self.board.naming {
            Some(n) => format!("Save as: {n}\u{2581}"),
            None => self.board_title(st),
        };
        text::draw(
            &p,
            &title,
            pos2(hdr.min.x, hdr.min.y + px(34.0)),
            Align2::LEFT_CENTER,
            Style::new(
                22.0,
                800,
                if self.board.naming.is_some() {
                    th.accent_ink
                } else {
                    th.text
                },
            ),
            Some(px(420.0)),
        );
        // the tones, follow
        let mut x = hdr.min.x + px(440.0);
        if let Some(rig) = &self.board.rig {
            let mut labels: Vec<String> = rig.tones.iter().map(|t| t.label.to_uppercase()).collect();
            labels.push("MINE".into());
            let n = rig.tones.len();
            let sel = match self.source(st) {
                Source::Song(i) => i,
                Source::Mine => n,
            };
            let tw = (labels
                .iter()
                .map(|l| text::width(&ctx, l, Style::caps(10.5, th.caption)) + px(18.0))
                .sum::<f32>()
                + px(6.0))
            .min(px(470.0));
            let tr = Rect::from_min_size(pos2(x, hdr.min.y + px(10.0)), vec2(tw, px(32.0)));
            let rects = pills(&p, tr, &labels, Some(sel), 10.5);
            for (i, r) in rects.iter().enumerate() {
                let resp = interact(ui, *r, ("board-view-tone", i), Sense::click());
                if resp.hovered() {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    self.pick_tone(st, if i < n { Pick::Tone(i) } else { Pick::Mine });
                }
            }
            x = tr.max.x + px(10.0);
            let fr = Rect::from_min_size(pos2(x, hdr.min.y + px(10.0)), vec2(px(122.0), px(32.0)));
            let on = pb::follow(&st.config) && self.board.pick.is_none();
            let resp = interact(ui, fr, "board-view-follow", Sense::click());
            lamp_key(&p, fr, "FOLLOW SONG", on, resp.hovered());
            if resp.clicked() {
                self.set_follow(st, !on);
            }
            x = fr.max.x + px(10.0);
        }
        let _ = x;
        // the keys at the right: presets, save as, reset / undo, close
        let mut kx = hdr.max.x;
        let close = Rect::from_min_size(pos2(kx - px(34.0), hdr.min.y + px(9.0)), Vec2::splat(px(34.0)));
        if widgets::round_button(
            ui,
            close,
            "board-view-close",
            Icon::XBold,
            13.0,
            RoundKind::Ivory,
            false,
        )
        .clicked()
        {
            self.board_commit(st);
            self.board.open = false;
            self.board.naming = None;
            return;
        }
        kx = close.min.x - px(10.0);
        let key_btn = |ui: &Ui, kx: &mut f32, id: &str, label: &str, primary: bool| -> bool {
            let bw = widgets::button_width(ui.ctx(), label, "", 13.0, false) + px(8.0);
            let r = Rect::from_min_size(pos2(*kx - bw, hdr.min.y + px(9.0)), vec2(bw, px(34.0)));
            *kx = r.min.x - px(8.0);
            let st_ = if primary {
                widgets::ButtonStyle::primary()
            } else {
                widgets::ButtonStyle::default()
            };
            widgets::button(ui, r, id, label, "", st_).clicked()
        };
        let edited_song = matches!(self.source(st), Source::Song(i) if self.board.rig.as_ref().is_some_and(|r| r.tones.get(i).is_some_and(|t| t.edited)));
        if edited_song && key_btn(ui, &mut kx, "board-reset", "Reset to song", false) {
            if let (Source::Song(i), Some(rig)) = (self.source(st), self.board.rig.as_mut()) {
                rig.reset(i, &self.song.arr, &mut st.userdata);
            }
            self.board.sel = None;
            self.say("Back to the song's own tone", "");
        }
        if self.board.undo.is_some()
            && key_btn(ui, &mut kx, "board-undo", "Undo", false)
            && let Some((b, label)) = self.board.undo.take()
        {
            self.board_edit(st, |cur| *cur = b);
            self.board_commit(st);
            self.say(format!("Back to {label}"), "");
        }
        if key_btn(ui, &mut kx, "board-saveas", "Save as\u{2026}", false) {
            self.board.naming = Some(pb::fresh_name(&st.config, "My board"));
        }
        if key_btn(ui, &mut kx, "board-presets", "Presets \u{25be}", self.board.menu) {
            self.board.menu = !self.board.menu;
        }
        let presets_anchor = pos2(kx + px(8.0), hdr.max.y + px(4.0));
        // the board itself, and under it the shown block (or the browser, adding one)
        let lower_h = px(150.0);
        let board_r = Rect::from_min_max(
            pos2(inner.min.x, hdr.max.y + px(10.0)),
            pos2(inner.max.x, inner.max.y - foot_h - px(12.0) - lower_h - px(12.0)),
        );
        self.board_area(ui, st, board_r);
        let lower = Rect::from_min_max(
            pos2(inner.min.x, board_r.max.y + px(12.0)),
            pos2(inner.max.x, inner.max.y - foot_h - px(12.0)),
        );
        let b = self.board_now(st);
        let blk = self.board.sel.and_then(|u| b.index_of(u)).map(|i| b.blocks[i]);
        match (self.board.picker, blk) {
            (Some(picker), _) => self.picker_ui(ui, st, lower, picker, "view"),
            (None, Some(blk)) => self.block_bar(ui, st, lower, blk),
            (None, None) => {
                paint::display(&p, lower, Some(18.0), 6.0);
                text::draw(
                    &p,
                    "Click a block on the board to edit it, or + to add one.",
                    lower.center(),
                    Align2::CENTER_CENTER,
                    Style::new(13.0, 600, th.caption),
                    Some(lower.width() - px(20.0)),
                );
            }
        }
        self.view_footer(ui, inner, foot_h);
        if self.board.menu {
            self.presets_menu(ui, st, presets_anchor);
        }
    }

    /// The big view's footer: the input level and its check, the output, the route.
    fn view_footer(&mut self, ui: &Ui, inner: Rect, foot_h: f32) {
        let p = ui.painter().clone();
        let th = t();
        let fy = inner.max.y - foot_h;
        let (in_peak, out_peak, route) = match self.engine() {
            Some(e) => {
                let m = e.meters();
                let s = e.status();
                let name = s
                    .input
                    .as_ref()
                    .map(|i| short_device(&i.device_name))
                    .unwrap_or_else(|| "NO INPUT".into());
                let route = match &s.input {
                    Some(i) => format!(
                        "{} \u{b7} {} \u{b7} {:.0} kHz \u{b7} {} samples{}",
                        i.device_name,
                        i.mode,
                        i.rate as f64 / 1000.0,
                        i.block,
                        if s.duplex {
                            " \u{b7} one device both ways"
                        } else {
                            ""
                        }
                    ),
                    None => "no guitar input".into(),
                };
                (m.peak, m.tone_peak, (name, route))
            }
            None => (0.0, 0.0, ("NO INPUT".into(), "audio off".into())),
        };
        meter(
            &p,
            Rect::from_min_size(pos2(inner.min.x, fy + px(4.0)), vec2(px(280.0), px(30.0))),
            &format!("INPUT \u{b7} {}", route.0),
            in_peak,
            true,
            self.clipping(),
        );
        let cr = Rect::from_min_size(
            pos2(inner.min.x + px(292.0), fy + px(6.0)),
            vec2(px(150.0), px(30.0)),
        );
        let checking = self.board.level.is_some();
        if widgets::button(
            ui,
            cr,
            "board-level",
            if checking {
                "Strum hard\u{2026}"
            } else {
                "Check my level"
            },
            "",
            widgets::ButtonStyle::active(checking),
        )
        .clicked()
            && !checking
        {
            self.board.level = Some((Instant::now() + Duration::from_secs(5), 0.0));
            self.board.level_note = None;
        }
        meter(
            &p,
            Rect::from_min_size(pos2(cr.max.x + px(16.0), fy + px(4.0)), vec2(px(200.0), px(30.0))),
            "OUTPUT",
            out_peak,
            false,
            false,
        );
        let note = self.board.level_note.clone().unwrap_or(route.1);
        text::draw(
            &p,
            &note,
            pos2(inner.max.x, fy + px(20.0)),
            Align2::RIGHT_CENTER,
            Style::new(
                11.0,
                600,
                if self.board.level_note.is_some() {
                    th.accent_ink
                } else {
                    th.caption
                },
            ),
            Some(inner.max.x - cr.max.x - px(240.0)),
        );
    }

    /// The board itself: every block standing on a pedalboard, cables between, + between them.
    pub(crate) fn board_area(&mut self, ui: &Ui, st: &mut AppState, r: Rect) {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        // the board: dark aluminium with slats (the amp room draws its own stage under it)
        if !self.board.stage {
            p.rect_filled(r, 18.0, Color32::from_rgb(30, 34, 39));
            let slat = r.height() / 7.0;
            for i in 1..7 {
                let y = r.min.y + slat * i as f32;
                p.line_segment(
                    [pos2(r.min.x + 10.0, y), pos2(r.max.x - 10.0, y)],
                    Stroke::new(1.0, Color32::from_white_alpha(8)),
                );
            }
            paint::inset(&p, r, Some(18.0), 3.0, Fill::Color(Color32::TRANSPARENT));
        }
        let b = self.board_now(st);
        // (on the amp room's stage the units stand on the floor, their names under them, and an amp straight before
        // a cab stands on it, the two the same width: `stack[i]`, block i is that amp)
        let stage = self.board.stage;
        let nb = b.blocks.len();
        let stack: Vec<bool> = (0..nb)
            .map(|i| {
                stage
                    && b.blocks[i].def().kind == Kind::Amp
                    && b.blocks.get(i + 1).is_some_and(|c| c.def().kind == Kind::Cab)
            })
            .collect();
        let under = |i: usize| i > 0 && stack[i - 1];
        let mut sizes: Vec<Vec2> = b.blocks.iter().map(faces::board_size).collect();
        for i in 0..nb {
            if stack[i] {
                let (a, c) = (sizes[i], sizes[i + 1]);
                let w = a.x.max(c.x);
                sizes[i] = a * (w / a.x.max(1.0));
                sizes[i + 1] = c * (w / c.x.max(1.0));
            }
        }
        let gap = if stage { 28.0 } else { 40.0 };
        let cols = (0..nb).filter(|&i| !under(i)).count();
        let total: f32 =
            (0..nb).filter(|&i| !under(i)).map(|i| sizes[i].x).sum::<f32>() + gap * (cols as f32 + 1.0);
        let label_h = if stage { px(44.0) } else { 0.0 };
        let tall = if stage {
            (0..nb)
                .map(|i| {
                    if stack[i] {
                        sizes[i].y + sizes[i + 1].y
                    } else {
                        sizes[i].y
                    }
                })
                .fold(200.0f32, f32::max)
        } else {
            200.0
        };
        let k = ((r.width() - 60.0) / total)
            .min((r.height() - 50.0 - label_h) / tall)
            .min(1.7);
        let mut x = r.min.x + 30.0 + gap * k;
        if stage {
            x += ((r.width() - 60.0) - total * k).max(0.0) / 2.0; // (centred on the stage)
        }
        let base = r.max.y - label_h;
        let lows = (0..nb)
            .filter(|&i| !stack[i] && !under(i))
            .map(|i| sizes[i].y)
            .fold(f32::MAX, f32::min);
        let low = if lows == f32::MAX {
            sizes.iter().map(|s| s.y).fold(f32::MAX, f32::min)
        } else {
            lows
        };
        let cy = if stage && !sizes.is_empty() {
            base - low * k * 0.55
        } else {
            r.center().y
        };
        let mut rects: Vec<(u32, Rect)> = Vec::new();
        for (i, (blk, s)) in b.blocks.iter().zip(&sizes).enumerate() {
            let sz = *s * k;
            if under(i) {
                // the cab under the amp before it: the amp goes up onto it
                let ar = rects[i - 1].1;
                rects.push((blk.uid, Rect::from_min_size(pos2(ar.min.x, base - sz.y), sz)));
                rects[i - 1].1 = Rect::from_min_size(pos2(ar.min.x, base - sz.y - ar.height()), ar.size());
                continue;
            }
            let y = if stage { base - sz.y } else { cy - sz.y / 2.0 };
            rects.push((blk.uid, Rect::from_min_size(pos2(x, y), sz)));
            x += sz.x + gap * k;
        }
        if stage {
            for (i, (_, rr)) in rects.iter().enumerate() {
                if under(i) {
                    continue;
                }
                let d = b.blocks[i].def();
                let on = b.blocks[i].on;
                let (num, name, after) = if stack[i] {
                    let c = b.blocks[i + 1].def();
                    (
                        format!("{} + {}  ", i + 1, i + 2),
                        format!("{} on {}", d.name, c.name),
                        format!("after {} \u{b7} {}", d.after, c.after),
                    )
                } else {
                    (
                        format!("{}  ", i + 1),
                        d.name.to_string(),
                        if on {
                            format!("after {}", d.after)
                        } else {
                            "off".into()
                        },
                    )
                };
                // (the stage is dark in every theme: its words are light)
                let (lit, dim) = (
                    Color32::from_rgb(0xe8, 0xea, 0xee),
                    Color32::from_rgb(0x8a, 0x90, 0x99),
                );
                let room_w = rr.width() + gap * k - px(6.0);
                let ns = Style::new(12.5, 800, if on { lit } else { dim });
                let ctx = p.ctx();
                if text::width(ctx, &num, ns) + text::width(ctx, &name, ns) <= room_w {
                    text::draw_runs(
                        &p,
                        &[(&num, Style::new(12.5, 800, dim)), (&name, ns)],
                        pos2(rr.center().x, base + px(16.0)),
                        Align2::CENTER_CENTER,
                    );
                } else {
                    text::draw(
                        &p,
                        &name,
                        pos2(rr.center().x, base + px(16.0)),
                        Align2::CENTER_CENTER,
                        ns,
                        Some(room_w),
                    );
                }
                text::draw(
                    &p,
                    &after,
                    pos2(rr.center().x, base + px(32.0)),
                    Align2::CENTER_CENTER,
                    Style::new(10.0, 500, dim),
                    Some(room_w),
                );
            }
        }
        // cables first, under everything: coloured patch leads drooping between the jacks
        let colours = [
            Color32::from_rgb(210, 59, 59),
            Color32::from_rgb(232, 185, 49),
            Color32::from_rgb(47, 127, 209),
            Color32::from_rgb(63, 179, 107),
            Color32::from_rgb(200, 200, 200),
            Color32::from_rgb(176, 79, 198),
        ];
        let mut prev = pos2(r.min.x + 8.0, cy);
        let points: Vec<(Pos2, Pos2)> = rects
            .iter()
            .map(|(_, rr)| (pos2(rr.min.x, cy + 6.0), pos2(rr.max.x, cy + 6.0)))
            .collect();
        let n = points.len();
        for i in 0..=n {
            if i < n && under(i) {
                // (an amp on its cab: no lead between them showing)
                prev = points[i].1;
                continue;
            }
            let to = if i < n {
                points[i].0
            } else {
                pos2(r.max.x - 8.0, cy)
            };
            let sag = 28.0 + (to.x - prev.x).abs() * 0.08;
            let c = CubicBezierShape::from_points_stroke(
                [prev, prev + vec2(18.0, sag), to + vec2(-18.0, sag), to],
                false,
                Color32::TRANSPARENT,
                Stroke::new(5.0, Color32::from_rgb(10, 12, 14)),
            );
            p.add(c);
            let c2 = CubicBezierShape::from_points_stroke(
                [prev, prev + vec2(18.0, sag), to + vec2(-18.0, sag), to],
                false,
                Color32::TRANSPARENT,
                Stroke::new(3.2, colours[i % colours.len()]),
            );
            p.add(c2);
            if i < n {
                prev = points[i].1;
            }
        }
        caps(
            &p,
            "IN",
            pos2(r.min.x + 14.0, cy - 14.0),
            9.0,
            if self.board.stage {
                Color32::from_rgb(0x8a, 0x90, 0x99)
            } else {
                th.caption
            },
            Align2::LEFT_CENTER,
        );
        caps(
            &p,
            "OUT",
            pos2(r.max.x - 14.0, cy - 14.0),
            9.0,
            if self.board.stage {
                Color32::from_rgb(0x8a, 0x90, 0x99)
            } else {
                th.caption
            },
            Align2::RIGHT_CENTER,
        );
        let moving = self.board.moving;
        for (i, (uid, br)) in rects.iter().enumerate() {
            let blk = b.blocks[i];
            let sel = self.board.sel == Some(*uid);
            // the block's body: click to select, drag to move
            let body = interact(ui, *br, ("board-body", *uid), Sense::click_and_drag());
            if body.hovered() {
                self.tip = Some((
                    format!("{} \u{b7} after {}", blk.def().shown_name(), blk.def().after),
                    "click: show \u{b7} double-click: edit \u{b7} drag: move".into(),
                    pos2(br.center().x, r.max.y + px(4.0)),
                    Align2::CENTER_TOP,
                ));
            }
            if body.double_clicked() {
                self.board.sel = Some(*uid);
                self.board.picker = None;
                self.board.editing = true;
            } else if body.clicked() {
                self.board.sel = Some(*uid);
                self.board.picker = None;
            }
            if body.drag_started()
                && let Some(pos) = body.interact_pointer_pos()
            {
                self.board.moving = Some(MoveDrag {
                    uid: *uid,
                    grab: pos.x - br.min.x,
                    x: br.min.x,
                    moved: false,
                });
            }
            let mut draw_r = *br;
            if let Some(m) = self.board.moving.as_mut()
                && m.uid == *uid
            {
                if body.dragged() {
                    if let Some(pos) = body.interact_pointer_pos() {
                        m.x = pos.x - m.grab;
                        m.moved = true;
                    }
                    draw_r = Rect::from_min_size(pos2(m.x, br.min.y - 8.0), br.size());
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                } else if !body.is_pointer_button_down_on() {
                    let centre = m.x + br.width() / 2.0;
                    let to = rects
                        .iter()
                        .filter(|(u, rr)| *u != *uid && rr.center().x < centre)
                        .count();
                    let moved = m.moved;
                    self.board.moving = None;
                    if moved && b.index_of(*uid) != Some(to) {
                        self.move_block(st, *uid, to);
                        self.board_commit(st);
                    }
                }
            }
            let _ = moving;
            if sel {
                p.rect_stroke(
                    draw_r.expand(6.0),
                    14.0,
                    Stroke::new(2.0, th.accent),
                    egui::StrokeKind::Outside,
                );
            }
            self.face_ui(ui, st, blk, draw_r, Detail::Board, "board");
            if !blk.on {
                paint::wash(&p, draw_r, Color32::BLACK, 0.35);
            }
        }
        // + between the blocks and at the end
        let mut plus_at: Vec<(usize, f32)> = (0..rects.len())
            .filter(|&i| !under(i))
            .map(|i| (i, rects[i].1.min.x - gap * k / 2.0))
            .collect();
        plus_at.push((
            rects.len(),
            rects
                .last()
                .map(|r| r.1.max.x + gap * k / 2.0)
                .unwrap_or(r.center().x),
        ));
        for (at, px_) in plus_at {
            let py = if stage && !sizes.is_empty() {
                base - low * k - px(20.0)
            } else {
                r.max.y - 22.0
            };
            let pr = Rect::from_center_size(pos2(px_, py), Vec2::splat(22.0));
            if widgets::round_button(
                ui,
                pr,
                ("board-plus", at),
                Icon::Plus,
                9.0,
                RoundKind::Ivory,
                false,
            )
            .clicked()
            {
                self.board.picker = Some(Picker::Kinds { at });
                self.board.browse = None;
            }
        }
        if b.blocks.is_empty() {
            text::draw(
                &p,
                "Nothing on the board yet: + adds a block",
                r.center(),
                Align2::CENTER_CENTER,
                Style::new(14.0, 600, th.caption),
                None,
            );
        }
    }

    /// What the block does, drawn.
    fn picture_ui(&self, p: &Painter, r: Rect, blk: &Block) {
        let th = t();
        let (title, note) = picture::caption(blk);
        caps(
            p,
            title,
            pos2(r.min.x + px(14.0), r.min.y + px(16.0)),
            11.0,
            th.caption,
            Align2::LEFT_CENTER,
        );
        text::draw(
            p,
            note,
            pos2(r.max.x - px(14.0), r.min.y + px(16.0)),
            Align2::RIGHT_CENTER,
            Style::new(10.5, 600, th.caption),
            Some(r.width() * 0.6),
        );
        let g = Rect::from_min_max(
            pos2(r.min.x + px(40.0), r.min.y + px(34.0)),
            pos2(r.max.x - px(16.0), r.max.y - px(26.0)),
        );
        let grid = Stroke::new(1.0, th.divider);
        let line = Stroke::new(2.2, th.accent);
        let label = |s: &str, pos: Pos2, a: Align2| {
            text::draw(p, s, pos, a, Style::new(9.0, 700, th.caption), None);
        };
        let poly = |pts: Vec<Pos2>, fill: bool| {
            if fill && pts.len() > 1 {
                let mut area = pts.clone();
                area.push(pos2(pts[pts.len() - 1].x, g.max.y));
                area.push(pos2(pts[0].x, g.max.y));
                p.add(egui::Shape::convex_polygon(
                    area,
                    Color32::TRANSPARENT,
                    Stroke::NONE,
                ));
            }
            p.add(egui::Shape::line(pts, line));
        };
        match picture::picture(blk) {
            Picture::Response(pts) => {
                let fx = |f: f32| g.min.x + (f / 20.0).log10() / 3.0 * g.width();
                let (top, bot) = (12.0, -24.0);
                let fy = |d: f32| g.min.y + (top - d.clamp(bot, top)) / (top - bot) * g.height();
                for f in [100.0, 1000.0, 10000.0] {
                    p.line_segment([pos2(fx(f), g.min.y), pos2(fx(f), g.max.y)], grid);
                    label(
                        if f >= 1000.0 {
                            if f >= 10000.0 { "10k" } else { "1k" }
                        } else {
                            "100"
                        },
                        pos2(fx(f), g.max.y + px(10.0)),
                        Align2::CENTER_CENTER,
                    );
                }
                for d in [12.0, 0.0, -12.0, -24.0] {
                    p.line_segment([pos2(g.min.x, fy(d)), pos2(g.max.x, fy(d))], grid);
                    label(
                        &format!("{d:+.0}"),
                        pos2(g.min.x - px(6.0), fy(d)),
                        Align2::RIGHT_CENTER,
                    );
                }
                let pts: Vec<Pos2> = pts.iter().map(|&(f, d)| pos2(fx(f), fy(d))).collect();
                poly(pts, false);
            }
            Picture::Transfer(pts) => {
                for v in [-1.0, 0.0, 1.0] {
                    let x = g.center().x + v * g.width() / 2.0;
                    let y = g.center().y - v * g.height() / 2.0;
                    p.line_segment([pos2(x, g.min.y), pos2(x, g.max.y)], grid);
                    p.line_segment([pos2(g.min.x, y), pos2(g.max.x, y)], grid);
                }
                label(
                    "in \u{2192}",
                    pos2(g.max.x, g.max.y + px(10.0)),
                    Align2::RIGHT_CENTER,
                );
                label("out", pos2(g.min.x - px(6.0), g.min.y), Align2::RIGHT_CENTER);
                let pts: Vec<Pos2> = pts
                    .iter()
                    .map(|&(a, b)| {
                        pos2(
                            g.center().x + a * g.width() / 2.0,
                            g.center().y - b * g.height() / 2.0,
                        )
                    })
                    .collect();
                poly(pts, false);
            }
            Picture::Dynamics(pts) => {
                let fx = |d: f32| g.min.x + (d + 60.0) / 60.0 * g.width();
                let fy = |d: f32| g.max.y - (d + 60.0) / 60.0 * g.height();
                for d in [-60.0, -40.0, -20.0, 0.0] {
                    p.line_segment([pos2(fx(d), g.min.y), pos2(fx(d), g.max.y)], grid);
                    p.line_segment([pos2(g.min.x, fy(d)), pos2(g.max.x, fy(d))], grid);
                    label(
                        &format!("{d:.0}"),
                        pos2(fx(d), g.max.y + px(10.0)),
                        Align2::CENTER_CENTER,
                    );
                }
                p.line_segment(
                    [pos2(fx(-60.0), fy(-60.0)), pos2(fx(0.0), fy(0.0))],
                    Stroke::new(1.0, th.tick),
                );
                let pts: Vec<Pos2> = pts.iter().map(|&(a, b)| pos2(fx(a), fy(b))).collect();
                poly(pts, false);
            }
            Picture::Echoes(taps, span) => {
                for s in 0..=(span.floor() as i32) {
                    let x = g.min.x + s as f32 / span * g.width();
                    p.line_segment([pos2(x, g.min.y), pos2(x, g.max.y)], grid);
                    label(
                        &format!("{s} s"),
                        pos2(x, g.max.y + px(10.0)),
                        Align2::CENTER_CENTER,
                    );
                }
                for (i, (t_, lvl)) in taps.iter().enumerate() {
                    let x = g.min.x + t_ / span * g.width();
                    let y = g.max.y - lvl * g.height();
                    let c = if i == 0 { th.text_2 } else { th.accent };
                    p.line_segment(
                        [pos2(x, g.max.y), pos2(x, y)],
                        Stroke::new(if i == 0 { 3.0 } else { 2.4 }, c),
                    );
                    p.circle_filled(pos2(x, y), 3.0, c);
                }
            }
            Picture::Decay(pts, span) => {
                for s in 0..=(span.floor() as i32) {
                    let x = g.min.x + s as f32 / span * g.width();
                    p.line_segment([pos2(x, g.min.y), pos2(x, g.max.y)], grid);
                    label(
                        &format!("{s} s"),
                        pos2(x, g.max.y + px(10.0)),
                        Align2::CENTER_CENTER,
                    );
                }
                let pts: Vec<Pos2> = pts
                    .iter()
                    .map(|&(t_, lvl)| pos2(g.min.x + t_ / span * g.width(), g.max.y - lvl * g.height()))
                    .collect();
                poly(pts, false);
            }
            Picture::Movement(pts) => {
                for s in [0.5, 1.0, 1.5] {
                    let x = g.min.x + s / 2.0 * g.width();
                    p.line_segment([pos2(x, g.min.y), pos2(x, g.max.y)], grid);
                    label(
                        &format!("{s} s"),
                        pos2(x, g.max.y + px(10.0)),
                        Align2::CENTER_CENTER,
                    );
                }
                let pts: Vec<Pos2> = pts
                    .iter()
                    .map(|&(t_, v)| pos2(g.min.x + t_ / 2.0 * g.width(), g.max.y - v * g.height()))
                    .collect();
                poly(pts, false);
            }
        }
    }

    /// The model browser: kind tabs, then the kind's models. With a block shown, a click changes it to that model;
    /// with + pressed, it adds one there.
    fn browser_ui(&mut self, ui: &Ui, st: &mut AppState, r: Rect, blk: Option<Block>) {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        let adding = match self.board.picker {
            Some(Picker::Kinds { at }) | Some(Picker::Models { at, .. }) => Some(at),
            None => None,
        };
        let kind = self
            .board
            .browse
            .or(blk.map(|b| b.def().kind))
            .unwrap_or(Kind::Drive);
        caps(
            &p,
            if adding.is_some() {
                "ADD A BLOCK HERE"
            } else {
                "CHANGE MODEL"
            },
            pos2(r.min.x + px(14.0), r.min.y + px(16.0)),
            11.0,
            if adding.is_some() {
                th.accent_ink
            } else {
                th.caption
            },
            Align2::LEFT_CENTER,
        );
        if adding.is_some() {
            let cr = Rect::from_min_size(
                pos2(r.max.x - px(70.0), r.min.y + px(8.0)),
                vec2(px(56.0), px(16.0)),
            );
            let resp = interact(ui, cr, "board-browse-cancel", Sense::click());
            text::draw(
                &p,
                "Cancel",
                cr.right_center(),
                Align2::RIGHT_CENTER,
                Style::new(11.0, 700, if resp.hovered() { th.accent_ink } else { th.caption }),
                None,
            );
            if resp.clicked() {
                self.board.picker = None;
            }
        }
        // kind tabs, wrapping
        let mut x = r.min.x + px(12.0);
        let mut y = r.min.y + px(32.0);
        for k in Kind::ALL {
            let label = k.label();
            let w = text::width(&ctx, label, Style::new(10.0, 800, th.text)) + px(26.0);
            if x + w > r.max.x - px(12.0) {
                x = r.min.x + px(12.0);
                y += px(30.0);
            }
            let tr = Rect::from_min_size(pos2(x, y), vec2(w, px(24.0)));
            let resp = interact(ui, tr, ("board-kind", label), Sense::click());
            let on = k == kind;
            if on {
                keys::key_face(&p, tr, Some(8.0), false, false);
            } else if resp.hovered() {
                p.rect_filled(tr, 8.0, th.screen_hi);
            }
            p.rect_filled(
                Rect::from_center_size(pos2(tr.min.x + px(11.0), tr.center().y), vec2(px(8.0), px(8.0))),
                2.0,
                faces::kind_colour(k),
            );
            text::draw(
                &p,
                label,
                pos2(tr.min.x + px(19.0), tr.center().y),
                Align2::LEFT_CENTER,
                Style::new(10.0, 800, if on { IVORY_INK } else { th.text_2 }),
                None,
            );
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.board.browse = Some(k);
            }
            x = tr.max.x + px(6.0);
        }
        // the kind's models
        let list = Rect::from_min_max(
            pos2(r.min.x + px(8.0), y + px(34.0)),
            pos2(r.max.x - px(8.0), r.max.y - px(8.0)),
        );
        let models: Vec<(usize, &board::ModelDef)> = catalog::of_kind(kind).collect();
        let rh = (list.height() / models.len().max(1) as f32).min(px(40.0));
        // (the name over its "after", where there's the height; else side by side)
        let two_lines = rh >= px(30.0);
        for (n, (i, m)) in models.iter().enumerate() {
            let rr = Rect::from_min_size(
                pos2(list.min.x, list.min.y + rh * n as f32),
                vec2(list.width(), rh),
            )
            .shrink2(vec2(0.0, px(1.0)));
            let resp = interact(ui, rr, ("board-model", *i), Sense::click());
            let current = blk.is_some_and(|b| b.model == *i) && adding.is_none();
            if current {
                p.rect_filled(rr, 9.0, th.accent.gamma_multiply(0.14));
            } else if resp.hovered() {
                p.rect_filled(rr, 9.0, th.screen_hi);
            }
            let sh = (rr.height() - px(4.0)).clamp(px(8.0), px(22.0));
            let sw = Rect::from_min_size(
                pos2(rr.min.x + px(6.0), rr.center().y - sh / 2.0),
                vec2(px(16.0), sh),
            );
            p.rect_filled(sw, 3.0, faces::rgb(m.colour));
            p.rect_stroke(
                sw,
                3.0,
                Stroke::new(1.0, Color32::from_black_alpha(120)),
                egui::StrokeKind::Inside,
            );
            let name = Style::new(12.0, 800, if current { th.accent_ink } else { th.text });
            let after = Style::new(10.0, 600, th.caption);
            let tx = sw.max.x + px(10.0);
            if two_lines {
                text::draw(
                    &p,
                    m.name,
                    pos2(tx, rr.center().y - px(7.0)),
                    Align2::LEFT_CENTER,
                    name,
                    Some(rr.max.x - tx - px(4.0)),
                );
                text::draw(
                    &p,
                    &format!("after {}", m.after),
                    pos2(tx, rr.center().y + px(8.0)),
                    Align2::LEFT_CENTER,
                    after,
                    Some(rr.max.x - tx - px(4.0)),
                );
            } else {
                let nr = text::draw(
                    &p,
                    m.name,
                    pos2(tx, rr.center().y),
                    Align2::LEFT_CENTER,
                    name,
                    Some(rr.max.x - tx - px(4.0)),
                );
                let ax = nr.max.x + px(8.0);
                if rr.max.x - ax > px(40.0) {
                    text::draw(
                        &p,
                        &format!("after {}", m.after),
                        pos2(ax, rr.center().y + px(1.0)),
                        Align2::LEFT_CENTER,
                        after,
                        Some(rr.max.x - ax - px(6.0)),
                    );
                }
            }
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
                self.tip = Some((
                    m.blurb.to_string(),
                    String::new(),
                    pos2(rr.min.x, rr.center().y),
                    Align2::RIGHT_CENTER,
                ));
            }
            if resp.clicked() {
                match (adding, blk) {
                    (Some(at), _) => self.add_block(st, at, *i),
                    (None, Some(b)) => self.change_model(st, b.uid, *i),
                    (None, None) => {
                        let at = self.board_now(st).blocks.len();
                        self.add_block(st, at, *i);
                    }
                }
            }
        }
    }

    /// The starting boards and yours, under the Presets key.
    fn presets_menu(&mut self, ui: &Ui, st: &mut AppState, anchor: Pos2) {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        let starters = board::presets();
        let saved = pb::saved(&st.config);
        let row_h = px(28.0);
        let n = starters.len() + saved.len() + 2;
        let w = px(260.0);
        let r = Rect::from_min_size(
            pos2(anchor.x - w + px(120.0), anchor.y),
            vec2(w, row_h * n as f32 + px(16.0)),
        );
        paint::floating(&p, r, Some(14.0));
        let mut y = r.min.y + px(8.0);
        caps(
            &p,
            "STARTING BOARDS",
            pos2(r.min.x + px(14.0), y + row_h / 2.0),
            10.0,
            th.caption,
            Align2::LEFT_CENTER,
        );
        y += row_h;
        let mut chosen: Option<(String, Board)> = None;
        for (name, b) in &starters {
            let rr = Rect::from_min_size(pos2(r.min.x + px(6.0), y), vec2(w - px(12.0), row_h));
            let resp = interact(ui, rr, ("board-preset", *name), Sense::click());
            if resp.hovered() {
                p.rect_filled(rr, 8.0, th.screen_hi);
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            let ids: Vec<&str> = b.blocks.iter().map(|x| x.def().short).collect();
            text::draw(
                &p,
                name,
                pos2(rr.min.x + px(10.0), rr.center().y),
                Align2::LEFT_CENTER,
                Style::new(13.0, 700, th.text),
                None,
            );
            text::draw(
                &p,
                &ids.join(" \u{203a} "),
                pos2(rr.max.x - px(8.0), rr.center().y),
                Align2::RIGHT_CENTER,
                Style::new(9.5, 700, th.caption),
                Some(w * 0.5),
            );
            if resp.clicked() {
                chosen = Some((name.to_string(), b.clone()));
            }
            y += row_h;
        }
        caps(
            &p,
            "YOUR BOARDS",
            pos2(r.min.x + px(14.0), y + row_h / 2.0),
            10.0,
            th.caption,
            Align2::LEFT_CENTER,
        );
        if saved.is_empty() {
            text::draw(
                &p,
                "none yet: Save as\u{2026}",
                pos2(r.max.x - px(14.0), y + row_h / 2.0),
                Align2::RIGHT_CENTER,
                Style::new(10.0, 600, th.caption),
                None,
            );
        }
        y += row_h;
        let mut delete: Option<String> = None;
        for (name, b) in &saved {
            let rr = Rect::from_min_size(pos2(r.min.x + px(6.0), y), vec2(w - px(12.0), row_h));
            let xr = Rect::from_center_size(pos2(rr.max.x - px(14.0), rr.center().y), Vec2::splat(px(18.0)));
            let resp = interact(ui, rr, ("board-saved", name.as_str()), Sense::click());
            if resp.hovered() {
                p.rect_filled(rr, 8.0, th.screen_hi);
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            text::draw(
                &p,
                name,
                pos2(rr.min.x + px(10.0), rr.center().y),
                Align2::LEFT_CENTER,
                Style::new(13.0, 700, th.text),
                Some(w - px(60.0)),
            );
            if widgets::round_button(
                ui,
                xr,
                ("board-saved-x", name.as_str()),
                Icon::X,
                8.0,
                RoundKind::Ivory,
                false,
            )
            .clicked()
            {
                delete = Some(name.clone());
            } else if resp.clicked() {
                chosen = Some((name.clone(), b.clone()));
            }
            y += row_h;
        }
        if let Some(n) = delete {
            pb::delete_saved(&mut st.config, &n);
            st.save_config();
            self.say(format!("Deleted \u{201c}{n}\u{201d}"), "");
        }
        if let Some((name, b)) = chosen {
            self.board.menu = false;
            self.load_board(st, &name, b);
        }
    }
}
