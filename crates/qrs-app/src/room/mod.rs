//! The amp room (PLAN.md P22.2-P22.3, D47): plug in and play through the pedalboard with no song. Reached from the
//! home screen's AMP ROOM button (key A). From the owner-approved mockups (Room-Main, Room-Library, Room-Save). New
//! code (no Python source).
//!
//! The rig is your own board (config.json `tone_board`, the player's MINE), played and edited with the
//! pedalboard's own UI (`player::board::Rack`, shared with the player): the stage is its board, the editor its
//! editor. The room adds the selected unit's knobs, a gear shelf, the strip (input, gate, tuner, scenes, rig
//! stepping, master), scenes on keys 1-4, your saved sounds (`rigs`), and the Sounds tab. While you're in it the
//! engine plays the board and hears the guitar; leaving puts the tone and monitoring back as they were.
//!
//! R (or the Record key) records what you hear to a file, R pauses, Shift R stops and saves (`crate::recording`, P23.9); leaving the
//! room stops and saves it. The Rig tab's key is G.

pub(crate) mod draw;
pub mod rigs;
mod save;
mod sounds;
mod stage;
mod strip;

use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Align2, Color32, Event, Key, Pos2, Rect, Ui, pos2, vec2};
use qrs_audio::board::{self, Board, Kind};
use qrs_audio::tone::ToneSpec;
use qrs_library::Config;
use qrs_ui::paint::{self, Fill};
use qrs_ui::text::{self, Style};
use qrs_ui::theme::{IVORY_INK, t};
use qrs_ui::units::px;
use qrs_ui::widgets;

use crate::listen::Listen;
use crate::pedalboard as pb;
use crate::player::board::{Pick, Rack};
use crate::state::AppState;
use draw::KeyLook;
use rigs::{Rig, RoomState, SCENES};

/// The room's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Rig,
    Sounds,
}

/// Where typed text goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Typing {
    Shelf,
    Sounds,
    SaveName,
}

/// The rig as the home screen's button names it: the saved sound it is, else its chain ("Perspex 100 · Mean
/// Green · 2 more").
pub fn rig_label(cfg: &Config) -> String {
    let st = RoomState::load(cfg);
    if let Some(n) = st.rig {
        return n;
    }
    let b = pb::my_board(cfg);
    let mut names: Vec<&str> = b
        .blocks
        .iter()
        .filter(|x| matches!(x.def().kind, Kind::Amp | Kind::Drive | Kind::Fuzz))
        .map(|x| x.def().name)
        .collect();
    if names.is_empty() {
        names = b.blocks.iter().map(|x| x.def().name).collect();
    }
    let more = b.blocks.len().saturating_sub(names.len().min(2));
    let mut s = names.iter().take(2).copied().collect::<Vec<_>>().join(" \u{b7} ");
    if more > 0 {
        s.push_str(&format!(" \u{b7} {more} more"));
    }
    if s.is_empty() { "Your rig".into() } else { s }
}

pub struct RoomView {
    pub rack: Rack,
    pub listen: Listen,
    pub tab: Tab,
    pub state: RoomState,
    /// Your saved sounds (read again when the user data changes).
    saved: Vec<Rig>,
    saved_ver: u64,
    /// What the engine had when the room opened (put back on leaving).
    tone_before: Option<ToneSpec>,
    monitor_before: Option<bool>,
    toast: Option<(String, String, Instant)>,
    /// A/B: hearing the saved sound instead of the rig as it is now.
    pub ab: bool,
    /// The shelf's family, its search, and where it adds (after the selected unit unless a + said where).
    pub shelf_kind: Kind,
    pub shelf_query: String,
    shelf_at: Option<usize>,
    typing: Option<Typing>,
    pub save: Option<save::Sheet>,
    pub sounds: sounds::Sounds,
    dice: rigs::Dice,
    pub closed: bool,
    /// Screenshots: hold still (no engine, no clock).
    pub shot: bool,
    area: Rect,
    /// The wheel's places this frame (the selected card's knobs; the strip's trim, gate and master).
    spots: Vec<stage::Spot>,
    strip_spots: Vec<(Rect, strip::What)>,
    drag: Option<stage::Drag>,
    /// When the wheel last turned one of the room's own knobs (the edit is kept once it rests).
    wheel_at: Option<Instant>,
    /// The Record key (the recording's badge hangs under it).
    rec_at: Rect,
}

impl RoomView {
    /// Open the room: the engine plays your board and hears the guitar.
    pub fn open(st: &mut AppState) -> RoomView {
        let engine = st.audio.as_ref().map(|a| a.engine.clone());
        let mut rack = Rack::new(engine.clone());
        rack.board.pick = Some(Pick::Mine);
        rack.board.open = true;
        rack.board.stage = true;
        let state = RoomState::load(&st.config);
        let mut v = RoomView {
            rack,
            listen: Listen::default(),
            tab: Tab::Rig,
            state,
            saved: Vec::new(),
            saved_ver: u64::MAX,
            tone_before: None,
            monitor_before: None,
            toast: None,
            ab: false,
            shelf_kind: Kind::Drive,
            shelf_query: String::new(),
            shelf_at: None,
            typing: None,
            save: None,
            sounds: sounds::Sounds::default(),
            dice: rigs::Dice::from_clock(),
            closed: false,
            shot: false,
            area: Rect::NOTHING,
            spots: Vec::new(),
            strip_spots: Vec::new(),
            drag: None,
            wheel_at: None,
            rec_at: Rect::NOTHING,
        };
        if let Some(e) = &engine {
            v.tone_before = Some(e.tone());
            v.monitor_before = Some(e.monitor());
            e.set_monitor(true);
            e.set_tone(ToneSpec::Board {
                board: pb::my_board(&st.config),
            });
        }
        v.refresh_saved(st);
        v.rack.title = Some(v.title());
        // the songs' tones for the Sounds tab (read in the background, once a session)
        if st.audio_mode != crate::state::AudioMode::Shots {
            crate::gearindex::start(
                st.songs.clone(),
                st.keys.clone(),
                st.data.cache().join("gear.json"),
            );
        }
        v
    }

    /// Leave: the edits kept, the room's state saved, the engine's tone and monitoring as they were.
    pub fn leave(&mut self, st: &mut AppState) {
        self.rack.board.audition = None;
        self.rack.board_commit(st);
        self.keep_scene(st);
        self.state.save(&mut st.config);
        if let Some(e) = self.rack.engine().cloned() {
            match self.tone_before.take() {
                // (the board the engine had is your board as it was: it's your board as it is now)
                Some(ToneSpec::Board { .. }) | None => e.set_tone(ToneSpec::Board {
                    board: pb::my_board(&st.config),
                }),
                Some(t) => e.set_tone(t),
            }
            if let Some(m) = self.monitor_before.take() {
                e.set_monitor(m);
            }
        }
        st.save_config();
    }

    fn refresh_saved(&mut self, st: &AppState) {
        if self.saved_ver != st.userdata.version {
            self.saved_ver = st.userdata.version;
            self.saved = rigs::saved(&st.userdata);
        }
    }

    /// The saved sound the rig is.
    fn saved_rig(&self) -> Option<&Rig> {
        let n = self.state.rig.as_ref()?;
        self.saved.iter().find(|r| r.name.eq_ignore_ascii_case(n))
    }

    /// The rig's name: the saved sound, else what it's based on, else "My rig".
    pub fn title(&self) -> String {
        if let Some(n) = &self.state.rig {
            return n.clone();
        }
        if !self.state.based_on.is_empty() {
            return self.state.based_on.clone();
        }
        "My rig".into()
    }

    /// The rig differs from the saved sound (or isn't one).
    fn edited(&mut self, st: &AppState) -> bool {
        let now = self.rack.board_now(st);
        match self.saved_rig() {
            Some(r) => !rigs::same_sound(&r.board, &now),
            None => true,
        }
    }

    pub fn say(&mut self, text: impl Into<String>, tail: impl Into<String>) {
        self.toast = Some((
            text.into(),
            tail.into(),
            Instant::now() + Duration::from_millis(1800),
        ));
    }

    // -- the rig ------------------------------------------------------------------------------------------------------

    /// Put a whole rig on (a saved sound, a starting rig, a song's tone): undo has the one before.
    pub fn load(
        &mut self,
        st: &mut AppState,
        b: Board,
        scenes: [Option<Board>; SCENES],
        rig: Option<String>,
        based_on: String,
    ) {
        self.ab = false;
        self.rack.board.audition = None;
        let name = rig.clone().unwrap_or_else(|| based_on.clone());
        self.rack.load_board(st, &name, b);
        self.rack.said.clear();
        self.state.rig = rig;
        self.state.based_on = based_on;
        self.state.scenes = scenes;
        self.state.scene = 0;
        // the first scene the sound has, if it has one, is what plays
        if let Some(s) = self.state.scenes[0].clone() {
            let cur = self.rack.board_now(st);
            let out = rigs::scene_board(&cur, &s);
            self.rack.board_edit(st, |x| *x = out);
            self.rack.board_commit(st);
        }
        self.rack.board.sel = None;
        self.rack.title = Some(self.title());
        self.state.save(&mut st.config);
        st.save_config();
        self.say(format!("{name} on"), "  \u{b7}  Ctrl Z takes it back");
    }

    pub fn load_saved(&mut self, st: &mut AppState, r: &Rig) {
        self.load(
            st,
            r.board.clone(),
            r.scenes.clone(),
            Some(r.name.clone()),
            r.based_on.clone(),
        );
    }

    /// The playing scene's settings into it (before switching away, saving, leaving).
    fn keep_scene(&mut self, st: &AppState) {
        let cur = self.rack.board_now(st);
        self.state.scenes[self.state.scene] = Some(cur);
    }

    /// Scene `n` (0-3): its on/off and knobs on the rig, in place (no gap: the engine changes knobs and bypasses).
    pub fn scene(&mut self, st: &mut AppState, n: usize) {
        if n >= SCENES || n == self.state.scene {
            return;
        }
        self.rack.board_commit(st);
        self.keep_scene(st);
        if let Some(s) = self.state.scenes[n].clone() {
            let cur = self.rack.board_now(st);
            let out = rigs::scene_board(&cur, &s);
            self.rack.board_edit(st, |x| *x = out);
            self.rack.board_commit(st);
        }
        self.state.scene = n;
        self.state.save(&mut st.config);
        st.save_config();
        self.rack.said.clear();
    }

    /// Save the rig as the sound it is (Ctrl S); a rig that isn't one yet opens the save sheet.
    pub fn save_now(&mut self, st: &mut AppState) {
        let Some(mut r) = self.saved_rig().cloned() else {
            self.open_save(st);
            return;
        };
        self.rack.board_commit(st);
        self.keep_scene(st);
        r.board = self.rack.board_now(st);
        r.scenes = self.state.scenes.clone();
        r.saved = now_secs();
        rigs::store(&mut st.userdata, &r);
        self.refresh_saved(st);
        self.say(format!("Saved \u{201c}{}\u{201d}", r.name), "");
    }

    pub fn open_save(&mut self, st: &mut AppState) {
        self.rack.board_commit(st);
        self.keep_scene(st);
        let base = self.saved_rig().cloned();
        let now = self.rack.board_now(st);
        self.save = Some(save::Sheet::new(
            &self.title(),
            base,
            &self.state.based_on,
            &self.saved,
            &now,
        ));
        self.typing = Some(Typing::SaveName);
    }

    fn shuffle(&mut self, st: &mut AppState) {
        let b = rigs::shuffle(&mut self.dice);
        let name = format!("Shuffle: {}", rigs::shuffle_name(&b));
        self.load(st, b, Default::default(), None, name);
    }

    /// Every rig you can step through: your sounds, then the starting rigs.
    fn rig_list(&self) -> Vec<(String, Board, Option<Rig>)> {
        let mut v: Vec<(String, Board, Option<Rig>)> = self
            .saved
            .iter()
            .map(|r| (r.name.clone(), r.board.clone(), Some(r.clone())))
            .collect();
        for (n, b) in board::presets() {
            v.push((n.to_string(), b, None));
        }
        v
    }

    /// Where the rig is in that list.
    fn rig_index(&self, list: &[(String, Board, Option<Rig>)]) -> Option<usize> {
        let name = self
            .state
            .rig
            .clone()
            .unwrap_or_else(|| self.state.based_on.clone());
        list.iter().position(|(n, _, saved)| {
            n.eq_ignore_ascii_case(&name) && (saved.is_some() == self.state.rig.is_some())
        })
    }

    fn step_rig(&mut self, st: &mut AppState, dir: i32) {
        let list = self.rig_list();
        if list.is_empty() {
            return;
        }
        let n = list.len() as i32;
        let i = match self.rig_index(&list) {
            Some(i) => (i as i32 + dir).rem_euclid(n),
            None => {
                if dir > 0 {
                    0
                } else {
                    n - 1
                }
            }
        } as usize;
        let (name, b, saved) = list[i].clone();
        match saved {
            Some(r) => self.load_saved(st, &r),
            None => self.load(st, b, Default::default(), None, name),
        }
    }

    fn toggle_ab(&mut self, st: &mut AppState) {
        let Some(r) = self.saved_rig().cloned() else {
            self.say("Nothing saved to compare with", "  \u{b7}  save the rig first");
            return;
        };
        self.ab = !self.ab;
        self.rack.board.audition = if self.ab {
            let mut b = r.board.clone();
            if let Some(s) = &r.scenes[self.state.scene] {
                b = rigs::scene_board(&b, s);
            }
            Some(b.renewed())
        } else {
            None
        };
        self.rack.board_send(st);
        self.say(
            if self.ab {
                "Hearing the saved sound"
            } else {
                "Hearing your rig"
            },
            "  \u{b7}  ` flips back",
        );
    }

    // -- keys ---------------------------------------------------------------------------------------------------------

    pub fn handle(&mut self, st: &mut AppState, ev: &Event) {
        // typing first
        if let Some(t) = self.typing {
            match ev {
                Event::Text(s) => {
                    let s = s.clone();
                    match t {
                        Typing::Shelf => push_text(&mut self.shelf_query, &s, 24),
                        Typing::Sounds => {
                            push_text(&mut self.sounds.query, &s, 40);
                            self.sounds.moved();
                        }
                        Typing::SaveName => {
                            if let Some(sh) = &mut self.save {
                                push_text(&mut sh.name, &s, 40);
                            }
                        }
                    }
                    return;
                }
                Event::Key {
                    key: Key::Backspace,
                    pressed: true,
                    ..
                } => {
                    match t {
                        Typing::Shelf => {
                            self.shelf_query.pop();
                        }
                        Typing::Sounds => {
                            self.sounds.query.pop();
                            self.sounds.moved();
                        }
                        Typing::SaveName => {
                            if let Some(sh) = &mut self.save {
                                sh.name.pop();
                            }
                        }
                    }
                    return;
                }
                // (letters and digits typed are the text's, not keys)
                Event::Key { key, modifiers, .. }
                    if !matches!(key, Key::Escape | Key::Enter | Key::ArrowUp | Key::ArrowDown)
                        && !modifiers.command =>
                {
                    return;
                }
                _ => {}
            }
        }
        if let Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = ev
        {
            let notches = crate::library::wheel_notches(*unit, delta.y);
            let pos = st.ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default();
            if self.tab == Tab::Rig && self.rack.board_wheel(st, pos, notches, modifiers.shift) {
                return;
            }
            if self.wheel(st, pos, notches, modifiers.shift) {
                return;
            }
            if self.tab == Tab::Sounds {
                self.sounds.wheel(pos, notches);
            }
            return;
        }
        let Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = ev
        else {
            return;
        };
        let ctrl = modifiers.command;
        // the save sheet is modal
        if self.save.is_some() {
            match key {
                Key::Escape => {
                    self.save = None;
                    self.typing = None;
                }
                Key::Enter => self.finish_save(st),
                _ => {}
            }
            return;
        }
        if self.rack.board.editing {
            if *key == Key::Escape {
                self.rack.board_commit(st);
                self.rack.board.editing = false;
            } else if ctrl && *key == Key::Z {
                self.rack.undo_edit(st);
            } else if ctrl && *key == Key::Y {
                self.rack.redo_edit(st);
            }
            return;
        }
        match key {
            Key::Escape => {
                if self.typing.is_some() {
                    self.typing = None;
                    if self.tab == Tab::Sounds && !self.sounds.query.is_empty() {
                        self.sounds.query.clear();
                        self.sounds.moved();
                    }
                } else if self.tab == Tab::Sounds && self.sounds.previewing() {
                    self.sounds.stop(&mut self.rack, st);
                    self.say("Back to your rig", "");
                } else if self.tab != Tab::Rig {
                    self.go(st, Tab::Rig);
                } else if self.shelf_at.is_some() {
                    self.shelf_at = None;
                } else {
                    self.closed = true;
                }
            }
            Key::S if ctrl && modifiers.shift => self.open_save(st),
            Key::S if ctrl => self.save_now(st),
            Key::Z if ctrl => self.rack.undo_edit(st),
            Key::Y if ctrl => self.rack.redo_edit(st),
            // R records what you hear, pauses, resumes; Shift R stops and saves (P23.9); the Rig tab is G
            Key::R if !ctrl => {
                let said = if modifiers.shift {
                    crate::recording::finish(st)
                } else {
                    crate::recording::press(st, crate::recording::Page::Room, &self.title())
                };
                if let Some((a, b)) = said {
                    self.say(a, b);
                }
            }
            Key::G if !ctrl => self.go(st, Tab::Rig),
            Key::L if !ctrl => self.go(st, Tab::Sounds),
            Key::F if ctrl => {
                self.go(st, Tab::Sounds);
                self.typing = Some(Typing::Sounds);
            }
            Key::Num1 | Key::Num2 | Key::Num3 | Key::Num4 => {
                let n = match key {
                    Key::Num1 => 0,
                    Key::Num2 => 1,
                    Key::Num3 => 2,
                    _ => 3,
                };
                if self.tab == Tab::Sounds && self.sounds.previewing() && modifiers.shift {
                    sounds::load_into_scene(self, st, n);
                } else {
                    self.scene(st, n);
                }
            }
            Key::Backtick => self.toggle_ab(st),
            Key::E if self.tab == Tab::Rig && !ctrl => {
                if self.rack.board.sel.is_some() {
                    self.rack.board.editing = true;
                }
            }
            Key::Tab if self.tab == Tab::Rig => {
                let b = self.rack.board_now(st);
                if !b.blocks.is_empty() {
                    let i = self.rack.board.sel.and_then(|u| b.index_of(u)).unwrap_or(0);
                    let n = b.blocks.len();
                    let j = if modifiers.shift {
                        (i + n - 1) % n
                    } else {
                        (i + 1) % n
                    };
                    self.rack.board.sel = Some(b.blocks[j].uid);
                }
            }
            Key::Space if self.tab == Tab::Rig => {
                if let Some(u) = self.rack.board.sel {
                    self.rack.toggle_block(st, u);
                }
            }
            Key::ArrowLeft if self.tab == Tab::Rig => self.step_rig(st, -1),
            Key::ArrowRight if self.tab == Tab::Rig => self.step_rig(st, 1),
            _ if self.tab == Tab::Sounds => sounds::key(self, st, *key, *modifiers),
            _ => {}
        }
        self.rack.said.clear();
    }

    pub fn go(&mut self, st: &mut AppState, tab: Tab) {
        if self.tab == Tab::Sounds && tab != Tab::Sounds && self.sounds.previewing() {
            self.sounds.stop(&mut self.rack, st);
        }
        self.tab = tab;
        self.typing = None;
        if tab == Tab::Sounds {
            self.sounds.moved();
        }
    }

    fn finish_save(&mut self, st: &mut AppState) {
        let Some(sh) = self.save.take() else { return };
        self.typing = None;
        let name = sh.name.trim().to_string();
        if name.is_empty() {
            self.say("A sound needs a name", "");
            self.save = Some(sh);
            self.typing = Some(Typing::SaveName);
            return;
        }
        self.keep_scene(st);
        let r = Rig {
            name: if sh.overwrite {
                sh.base.as_ref().map(|b| b.name.clone()).unwrap_or(name.clone())
            } else {
                name.clone()
            },
            tags: sh.tags.clone(),
            color: sh.color,
            slot: sh.slot,
            based_on: sh.based_on.clone(),
            board: self.rack.board_now(st),
            scenes: self.state.scenes.clone(),
            saved: now_secs(),
        };
        rigs::store(&mut st.userdata, &r);
        self.refresh_saved(st);
        self.state.rig = Some(r.name.clone());
        self.state.based_on = r.based_on.clone();
        self.state.save(&mut st.config);
        st.save_config();
        self.rack.title = Some(self.title());
        self.say(
            format!("Saved \u{201c}{}\u{201d}", r.name),
            "  \u{b7}  in your sounds",
        );
    }

    /// The wheel over the room's own controls (the selected unit's knobs, the gate, the trim, the master).
    fn wheel(&mut self, st: &mut AppState, pos: Pos2, notches: f32, shift: bool) -> bool {
        if self.tab == Tab::Rig && stage::wheel(self, st, pos, notches, shift) {
            return true;
        }
        strip::wheel(self, st, pos, notches, shift)
    }

    // -- drawing ------------------------------------------------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut Ui, st: &mut AppState, area: Rect) {
        self.area = area;
        self.refresh_saved(st);
        let p = ui.painter().clone();
        let th = t();
        p.rect_filled(area, 0.0, th.base);
        // the engine in step: the board sent when it changes, edits kept when the mouse lets go
        self.rack.engine = st.audio.as_ref().map(|a| a.engine.clone());
        self.rack.title = Some(self.title());
        self.rack.board_update(st);
        let live = self.listen.update(self.rack.engine.as_deref());
        if live || self.rack.board.editing {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        } else {
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
        self.sounds.poll(st);
        // a wheel turned on the room's own knobs: kept once it rests
        if self
            .wheel_at
            .is_some_and(|w| w.elapsed() > Duration::from_millis(700))
        {
            self.wheel_at = None;
            self.rack.board_commit(st);
        }
        let m = px(20.0);
        let top = Rect::from_min_size(
            pos2(area.min.x + m, area.min.y + px(18.0)),
            vec2(area.width() - 2.0 * m, px(44.0)),
        );
        let strip_r = Rect::from_min_max(
            pos2(area.min.x + m, area.max.y - px(12.0) - px(72.0)),
            pos2(area.max.x - m, area.max.y - px(12.0)),
        );
        let body = Rect::from_min_max(
            pos2(area.min.x + m, top.max.y + px(22.0)),
            pos2(area.max.x - m, strip_r.min.y - px(10.0)),
        );
        self.top_bar(ui, st, top);
        match self.tab {
            Tab::Rig => stage::rig_page(self, ui, st, body),
            Tab::Sounds => sounds::page(self, ui, st, body),
        }
        strip::strip(self, ui, st, strip_r);
        // the detailed editor, over the stage and cards
        if self.rack.board.editing && self.tab == Tab::Rig {
            paint::dim(&p, area, Some(150));
            let panel = body;
            paint::floating(&p, panel, Some(24.0));
            self.rack.editor_view(ui, st, panel.shrink(px(20.0)), 0.0);
            if !self.rack.board.open {
                // (its close key: back to the room)
                self.rack.board.open = true;
                self.rack.board.editing = false;
            }
        }
        if let Some(sh) = &mut self.save {
            let act = sh.ui(ui, st, area);
            match act {
                save::Act::None => {}
                save::Act::Cancel => {
                    self.save = None;
                    self.typing = None;
                }
                save::Act::Save => self.finish_save(st),
                save::Act::Type => self.typing = Some(Typing::SaveName),
            }
        }
        // recording: the lamp, time and keys under the Record key (a click stops it)
        if let Some(s) = st.rec.status() {
            let w = crate::recording::badge_width(ui, s);
            let x = (self.rec_at.center().x - w / 2.0).min(area.max.x - m - w);
            let at = pos2(x, self.rec_at.max.y + px(4.0) + px(13.0));
            if crate::recording::badge(ui, at, s).clicked()
                && let Some((a, b)) = crate::recording::toggle(st, crate::recording::Page::Room, "")
            {
                self.say(a, b);
            }
        }
        // what the rack said, as the room's toast; its tooltip (its requests are the player's: none here)
        self.rack.reqs.clear();
        for (a, b) in std::mem::take(&mut self.rack.said) {
            self.say(a, b);
        }
        if let Some((msg, tail, until)) = self.toast.clone() {
            if Instant::now() < until {
                widgets::toast(&p, &msg, pos2(body.center().x, body.max.y - px(40.0)), &tail);
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            } else {
                self.toast = None;
            }
        }
        if let Some((text_, key, pos, anchor)) = self.rack.tip.take() {
            crate::player::draw_tip(&p, &text_, &key, pos, anchor, area);
        }
    }

    fn top_bar(&mut self, ui: &Ui, st: &mut AppState, r: Rect) {
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        let h = r.height();
        let home = Rect::from_min_size(r.min, vec2(px(96.0), h));
        if draw::key(
            ui,
            home,
            "room-home",
            KeyLook::Ivory,
            "Home",
            "",
            Some(&|p: &egui::Painter, c: Pos2, s: f32, col: Color32| draw::icon_chev(p, c, s, col, -1.0)),
        )
        .clicked()
        {
            self.closed = true;
        }
        // the right group: undo, redo, A/B, Shuffle, Save as, Save
        let mut x = r.max.x;
        let place = |x: &mut f32, w: f32| {
            let rr = Rect::from_min_size(pos2(*x - w, r.min.y), vec2(w, h));
            *x = rr.min.x - px(8.0);
            rr
        };
        let save_r = place(&mut x, draw::key_w(&ctx, "Save", "Ctrl S", true));
        let saveas_r = place(&mut x, draw::key_w(&ctx, "Save as\u{2026}", "", false));
        x -= px(8.0);
        let shuffle_r = place(&mut x, draw::key_w(&ctx, "Shuffle", "", true));
        let ab_r = place(&mut x, draw::key_w(&ctx, "A/B saved", "`", false));
        let redo_r = place(&mut x, h);
        let undo_r = place(&mut x, h);
        // Record (R): what you hear, to a file (P23.9); its badge is drawn under it once the page is (on top)
        let recording = st.rec.status();
        x -= px(8.0);
        let rec_label = if recording.is_some() { "Stop" } else { "Record" };
        let rec_r = place(&mut x, draw::key_w(&ctx, rec_label, "R", true));
        self.rec_at = rec_r;
        let dot =
            |p: &egui::Painter, c: Pos2, s: f32, _: Color32| crate::recording::icon_dot(p, c, s, t().miss);
        if draw::key(
            ui,
            rec_r,
            "room-rec",
            if recording.is_some() {
                KeyLook::Rec
            } else {
                KeyLook::Dark
            },
            rec_label,
            "R",
            Some(&dot),
        )
        .clicked()
            && let Some((a, b)) = crate::recording::toggle(st, crate::recording::Page::Room, &self.title())
        {
            self.say(a, b);
        }
        if draw::key(
            ui,
            save_r,
            "room-save",
            KeyLook::Amber,
            "Save",
            "Ctrl S",
            Some(&draw::icon_save),
        )
        .clicked()
        {
            self.save_now(st);
        }
        if draw::key(
            ui,
            saveas_r,
            "room-saveas",
            KeyLook::Ivory,
            "Save as\u{2026}",
            "",
            None,
        )
        .clicked()
        {
            self.open_save(st);
        }
        if draw::key(
            ui,
            shuffle_r,
            "room-shuffle",
            KeyLook::Dark,
            "Shuffle",
            "",
            Some(&draw::icon_shuffle),
        )
        .clicked()
        {
            self.shuffle(st);
        }
        if draw::key(
            ui,
            ab_r,
            "room-ab",
            if self.ab { KeyLook::Lit } else { KeyLook::Dark },
            if self.ab { "B saved" } else { "A/B saved" },
            "`",
            None,
        )
        .clicked()
        {
            self.toggle_ab(st);
        }
        let can_redo = self.rack.can_redo();
        let can_undo = self.rack.can_undo();
        if draw::key(
            ui,
            redo_r,
            "room-redo",
            KeyLook::Dark,
            "",
            "",
            Some(&draw::icon_redo),
        )
        .clicked()
            && can_redo
        {
            self.rack.redo_edit(st);
        }
        if draw::key(
            ui,
            undo_r,
            "room-undo",
            KeyLook::Dark,
            "",
            "",
            Some(&draw::icon_undo),
        )
        .clicked()
            && can_undo
        {
            self.rack.undo_edit(st);
        }
        if !can_undo {
            paint::wash(&p, undo_r, th.base, 0.45);
        }
        if !can_redo {
            paint::wash(&p, redo_r, th.base, 0.45);
        }
        // the tabs, centred between the title and the keys
        // (no Jam tab until the jam engine has its page: P23.2)
        let tabs = [(Tab::Rig, "Rig", "G"), (Tab::Sounds, "Sounds", "L")];
        let widths: Vec<f32> = tabs
            .iter()
            .map(|(_, l, k)| draw::key_w(&ctx, l, k, false))
            .collect();
        let tw: f32 = widths.iter().sum::<f32>() + px(8.0) + px(3.0) * 2.0;
        let tx = (r.min.x + px(530.0)).min(rec_r.min.x - px(24.0) - tw);
        let track = Rect::from_min_size(pos2(tx, r.min.y), vec2(tw, h));
        paint::inset(&p, track, Some(px(13.0)), 2.0, Fill::Base);
        let mut kx = track.min.x + px(4.0);
        for (i, (tab, l, k)) in tabs.iter().enumerate() {
            let kr = Rect::from_min_size(pos2(kx, track.min.y + px(4.0)), vec2(widths[i], h - px(8.0)));
            kx = kr.max.x + px(3.0);
            let on = self.tab == *tab;
            let resp = widgets::interact(ui, kr, ("room-tab", i), egui::Sense::click());
            if resp.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if on {
                qrs_ui::keys::key_face(&p, kr, Some(px(10.0)), false, false);
            }
            let ink = if on { IVORY_INK } else { th.text_2 };
            let lw = text::width(&ctx, l, Style::new(13.0, 800, ink));
            let cw = qrs_ui::keys::keycap_width(&ctx, k, 10.0);
            let x0 = kr.center().x - (lw + px(7.0) + cw) / 2.0;
            text::draw(
                &p,
                l,
                pos2(x0, kr.center().y),
                Align2::LEFT_CENTER,
                Style::new(13.0, 800, ink),
                None,
            );
            qrs_ui::keys::keycap(
                &p,
                k,
                pos2(x0 + lw + px(7.0), kr.center().y),
                Align2::LEFT_CENTER,
                10.0,
                if on {
                    qrs_ui::keys::CapOn::Ivory
                } else {
                    qrs_ui::keys::CapOn::Material
                },
            );
            if resp.clicked() {
                self.go(st, *tab);
            }
        }
        // the title: AMP ROOM · what it's based on; the rig's name and EDITED
        let tx0 = home.max.x + px(16.0);
        let room_w = track.min.x - tx0 - px(16.0);
        let sub = match (self.tab, self.sounds.previewing()) {
            (Tab::Sounds, true) => format!(
                "AMP ROOM  \u{b7}  SOUNDS  \u{b7}  PREVIEWING {}",
                self.sounds.preview_name().to_uppercase()
            ),
            _ => match (&self.state.rig, self.state.based_on.is_empty()) {
                (Some(_), true) => "AMP ROOM  \u{b7}  YOUR SOUND".into(),
                (_, false) => format!(
                    "AMP ROOM  \u{b7}  BASED ON {}",
                    self.state.based_on.to_uppercase()
                ),
                (None, true) => "AMP ROOM".into(),
            },
        };
        text::caps(
            &p,
            &sub,
            pos2(tx0, r.min.y + px(4.0)),
            10.5,
            th.caption,
            Align2::LEFT_CENTER,
            Some(room_w),
        );
        let title = self.title();
        let edited = self.edited(st);
        let badge = if self.state.rig.is_some() {
            edited.then_some("EDITED")
        } else {
            Some("NOT SAVED")
        };
        let bw = badge
            .map(|b| text::width(&ctx, b, Style::caps(10.0, th.accent)) + px(30.0))
            .unwrap_or(0.0);
        let tr = text::draw(
            &p,
            &title,
            pos2(tx0, r.min.y + px(28.0)),
            Align2::LEFT_CENTER,
            Style::new(24.0, 800, th.text),
            Some((room_w - bw - px(12.0)).max(px(60.0))),
        );
        if let Some(b) = badge {
            let br = Rect::from_min_size(
                pos2(tr.max.x + px(12.0), tr.center().y - px(11.0)),
                vec2(bw, px(22.0)),
            );
            p.rect_filled(br, px(11.0), th.accent.gamma_multiply(0.12));
            p.rect_stroke(
                br,
                px(11.0),
                egui::Stroke::new(1.0, th.accent.gamma_multiply(0.45)),
                egui::StrokeKind::Inside,
            );
            p.circle_filled(pos2(br.min.x + px(12.0), br.center().y), px(3.5), th.accent);
            text::caps(
                &p,
                b,
                pos2(br.min.x + px(21.0), br.center().y),
                10.0,
                th.accent_ink,
                Align2::LEFT_CENTER,
                None,
            );
        }
    }
}

/// Screenshot mode: four saved sounds (in the favourite slots) and the room on one of them with an edit, as the
/// mockups; songs' tones from the library's first songs when none have been read.
pub fn stage_data(st: &mut AppState) {
    let b = |id: &str| board::Block::of(id).expect("catalogue model");
    let lead = Board::new(vec![
        b("squeeze").with("sustain", 4.0),
        b("green").with("drive", 3.8).with("tone", 5.5).with("level", 7.2),
        b("amp_crunch").with("gain", 7.0).with("treble", 6.0),
        b("cab_4x12"),
        {
            let mut x = b("phaser");
            x.on = false;
            x
        },
        b("analog_delay").with("mix", 22.0),
        b("spring").with("dwell", 4.0).with("mix", 20.0),
    ]);
    if rigs::saved(&st.userdata).is_empty() {
        let presets = board::presets();
        let p = |n: &str| {
            presets
                .iter()
                .find(|(x, _)| x.eq_ignore_ascii_case(n))
                .map(|x| x.1.clone())
                .unwrap_or_default()
        };
        let mk = |name: &str, tags: &[&str], color: usize, slot: u8, based: &str, board: Board| Rig {
            name: name.into(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            color,
            slot,
            based_on: based.into(),
            board,
            scenes: Default::default(),
            saved: now_secs() - 86_400.0 * slot as f64,
        };
        let mut scenes: [Option<Board>; SCENES] = Default::default();
        let mut clean = lead.clone();
        for x in clean.blocks.iter_mut() {
            if x.def().id == "green" || x.def().id == "analog_delay" {
                x.on = false;
            }
        }
        scenes[0] = Some(clean);
        scenes[2] = Some(lead.clone());
        let mut warm = mk(
            "Reptilia lead, warmer",
            &["Indie", "Lead"],
            2,
            2,
            "Reptilia \u{b7} Lead",
            lead.clone(),
        );
        warm.scenes = scenes;
        for r in [
            mk("Glass clean", &["Pop", "Clean"], 0, 1, "Clean", p("Clean")),
            warm,
            mk("Bletchley lead", &["Rock", "Lead"], 3, 3, "Lead", p("Lead")),
            mk("Shimmer pad", &["Ambient"], 4, 4, "Ambient", p("Ambient")),
            mk(
                "Strokes rhythm, tight",
                &["Indie", "Crunch"],
                1,
                0,
                "Reptilia \u{b7} Rhythm",
                p("Crunch"),
            ),
        ] {
            rigs::store(&mut st.userdata, &r);
        }
    }
    // the room on the warmer lead, scene 3, with its Drive turned up a little since it was saved
    let mut now = lead.renewed();
    if let Some(g) = now.blocks.iter_mut().find(|x| x.def().id == "green") {
        g.set("drive", 4.6);
    }
    pb::set_my_board(&mut st.config, &now);
    let saved = rigs::saved(&st.userdata);
    let warm = saved.iter().find(|r| r.name == "Reptilia lead, warmer");
    RoomState {
        rig: warm.map(|r| r.name.clone()),
        based_on: warm.map(|r| r.based_on.clone()).unwrap_or_default(),
        scene: 2,
        scenes: warm.map(|r| r.scenes.clone()).unwrap_or_default(),
    }
    .save(&mut st.config);
    if crate::gearindex::version() == 0 {
        let presets = board::presets();
        let mut tones = Vec::new();
        let rows: Vec<usize> = (0..st.groups.len())
            .filter(|&g| st.song(g).playable())
            .take(8)
            .collect();
        for (i, g) in rows.into_iter().enumerate() {
            let e = st.song(g);
            let (pn, pbrd) = &presets[i % presets.len()];
            let part = e
                .arrangements
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "Lead".into());
            tones.push((
                e.display_title(),
                e.artist.clone(),
                part,
                pn.to_lowercase(),
                pbrd.clone(),
            ));
            if i % 3 == 0 {
                tones.push((
                    e.display_title(),
                    e.artist.clone(),
                    "Rhythm".into(),
                    "crunch".into(),
                    presets[1].1.clone(),
                ));
            }
        }
        crate::gearindex::stage(tones);
    }
}

impl RoomView {
    /// Screenshot mode: the room in a state ("rig", "sounds", "save", "editor"), the guitar heard at a good
    /// level playing A2.
    pub fn stage_shot(&mut self, st: &mut AppState, what: &str) {
        self.shot = true;
        self.listen.stage(-16.0, 45.04, false);
        self.save = None;
        self.typing = None;
        self.rack.board.editing = false;
        let b = self.rack.board_now(st);
        self.rack.board.sel = b
            .blocks
            .iter()
            .find(|x| x.def().id == if what == "editor" { "amp_crunch" } else { "green" })
            .map(|x| x.uid);
        match what {
            "sounds" => {
                self.go(st, Tab::Sounds);
                let saved = self.saved.clone();
                self.sounds.build(&saved, st);
                sounds::select(self, st, 0);
            }
            "save" => {
                self.go(st, Tab::Rig);
                self.open_save(st);
            }
            "editor" => {
                self.go(st, Tab::Rig);
                self.rack.board.editing = true;
            }
            _ => self.go(st, Tab::Rig),
        }
        // recording (P23.9): "rec", "rec_paused", "rec_saved"
        if let Some(w) = what.strip_prefix("rec") {
            let dir = st.config.recordings_dir(&st.data.root);
            let w = match w.trim_start_matches('_') {
                "" => "recording",
                x => x,
            };
            st.rec
                .stage_for(crate::recording::Page::Room, &dir, w, &self.title());
        }
    }
}

fn push_text(s: &mut String, add: &str, max: usize) {
    for ch in add.chars() {
        if s.chars().count() < max && !ch.is_control() {
            s.push(ch);
        }
    }
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
