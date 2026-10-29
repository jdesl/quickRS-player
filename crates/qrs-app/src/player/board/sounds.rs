//! Your sounds on the Pedalboard tile (P23.3, owner request): the sounds saved in the amp room (userdata.json
//! `room_rigs`, D47), picked while a song plays. The star key on the tile (or Shift T) lists them in place of the
//! block's face, favourite slots first, then the rest as saved, searched by typing (as the room's Sounds tab). A
//! click (or Enter) puts one on as your board (MINE) the way the room loads it, so the room opens on it too; the
//! first row then goes Back to what you had before (the song's tone, your board as it was, the tone if it wasn't
//! the pedalboard). The board reaches the engine as any whole new board does (its 30 ms crossfade). New code.

use super::*;
use crate::ampbutton::rgb;
use crate::room::draw as rd;
use crate::room::rigs::{self, COLOURS, Rig, RoomState};
use qrs_library::Config;
use serde_json::Value;

/// The list while it's open.
pub struct SoundPick {
    pub query: String,
    /// The row picked with the arrows (the Back row counts as row 0 when there is one).
    pub sel: usize,
    scroll: f32,
    /// Your sounds, read again when the user data changes.
    rigs: Vec<Rig>,
    ver: Option<u64>,
    list: Rect,
    rows: Vec<Rect>,
}

impl Default for SoundPick {
    fn default() -> Self {
        SoundPick {
            query: String::new(),
            sel: 0,
            scroll: 0.0,
            rigs: Vec::new(),
            ver: None,
            list: Rect::NOTHING,
            rows: Vec::new(),
        }
    }
}

/// What putting a sound on changed, as it was before the first one (Back puts it all back).
#[derive(Clone, Debug)]
pub struct Before {
    pick: Option<Pick>,
    title: Option<String>,
    /// Your board (config.json `tone_board`).
    mine: Board,
    /// The room's state (config.json `room`), as it was (None: there was none).
    room: Option<Value>,
    /// The tone, when it wasn't the pedalboard.
    tone: Option<ToneSpec>,
    /// What Back goes back to, as words.
    label: String,
}

/// A row of the list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Back,
    /// A sound (its index in the saved list).
    Sound(usize),
}

/// The rows: Back first while there's something to go back to, then the sounds (see `rigs::listed`).
pub fn rows(back: bool, list: &[Rig], query: &str) -> Vec<Row> {
    let mut out = Vec::new();
    if back {
        out.push(Row::Back);
    }
    out.extend(rigs::listed(list, query).into_iter().map(Row::Sound));
    out
}

/// The room's state as it is (to put back).
fn room_now(cfg: &Config) -> Option<Value> {
    cfg.get("room").cloned()
}

/// The room's state back as it was.
fn room_back(cfg: &mut Config, was: &Option<Value>) {
    match was {
        Some(v) => cfg.set("room", v.clone()),
        None => {
            cfg.remove("room");
        }
    }
}

/// A sound put on in config.json: your board is the sound (at its first scene), the room's state names it.
/// Returns the board.
fn put_on(cfg: &mut Config, r: &Rig) -> Board {
    let b = rigs::sound_board(r).renewed();
    pb::set_my_board(cfg, &b);
    rigs::state_for(r).save(cfg);
    b
}

impl Rack {
    pub fn sounds_open(&self) -> bool {
        self.board.sounds.is_some()
    }

    /// Show or hide the list (the star key, Shift T).
    pub fn sounds_toggle(&mut self) {
        if self.board.sounds.take().is_none() {
            self.board.sounds = Some(SoundPick::default());
            self.board.picker = None;
        }
    }

    fn sounds_refresh(&mut self, st: &AppState) {
        let Some(sp) = self.board.sounds.as_mut() else {
            return;
        };
        if sp.ver != Some(st.userdata.version) {
            sp.ver = Some(st.userdata.version);
            sp.rigs = rigs::saved(&st.userdata);
        }
    }

    fn sound_rows(&self) -> Vec<Row> {
        match &self.board.sounds {
            Some(sp) => rows(self.board.sound_before.is_some(), &sp.rigs, &sp.query),
            None => Vec::new(),
        }
    }

    /// The saved sound your board is now (the room's state names it), when it's MINE that's playing.
    fn sound_on(&self, st: &AppState) -> Option<String> {
        if self.source(st) != Source::Mine {
            return None;
        }
        RoomState::load(&st.config).rig
    }

    /// Put a saved sound on as your board, live: what you had is kept for Back (the first time).
    pub fn put_sound(&mut self, st: &mut AppState, r: &Rig) {
        self.board_commit(st);
        let tone = self.engine().map(|e| e.tone());
        if self.board.sound_before.is_none() {
            let label = if self.board_mode() {
                match self.source(st) {
                    Source::Song(_) => self.board_title(st),
                    Source::Mine => format!("{} as it was", self.board_title(st)),
                }
            } else {
                tone.as_ref()
                    .map(|t| format!("Tone: {}", crate::player::tone_label(t)))
                    .unwrap_or_else(|| "what you had".into())
            };
            self.board.sound_before = Some(Before {
                pick: self.board.pick,
                title: self.title.clone(),
                mine: pb::my_board(&st.config),
                room: room_now(&st.config),
                tone: tone.clone().filter(|t| !matches!(t, ToneSpec::Board { .. })),
                label,
            });
        }
        self.board.pick = Some(Pick::Mine);
        self.board.picker = None;
        self.board.sel = None;
        self.board.audition = None;
        let b = put_on(&mut st.config, r);
        self.board.mine = Some(b.clone());
        st.save_config();
        self.title = Some(r.name.clone());
        // (MINE's undo history starts again at the sound)
        self.board.kept_for.clear();
        if self.board_mode() {
            self.board_send(st);
        } else {
            // a sound is for hearing: the pedalboard comes on
            let spec = ToneSpec::Board { board: b.clone() };
            crate::tones::store(&mut st.config, &spec);
            st.save_config();
            if let Some(e) = self.engine() {
                e.set_tone(spec);
            }
            self.board.sent = Some(b);
        }
        let tail = match &self.board.sound_before {
            Some(bf) => format!("  \u{b7}  Back: {}", bf.label),
            None => String::new(),
        };
        self.say(format!("{} on", r.name), tail);
    }

    /// Back to what you had before the first sound was put on.
    pub fn sound_back(&mut self, st: &mut AppState) {
        let Some(bf) = self.board.sound_before.take() else {
            return;
        };
        self.board_commit(st);
        pb::set_my_board(&mut st.config, &bf.mine);
        room_back(&mut st.config, &bf.room);
        self.board.mine = Some(bf.mine.clone());
        self.board.pick = bf.pick;
        self.board.sel = None;
        self.board.kept_for.clear();
        self.title = bf.title.clone();
        if let Some(t) = bf.tone.clone() {
            crate::tones::store(&mut st.config, &t);
            if let Some(e) = self.engine() {
                e.set_tone(t);
            }
            self.board.sent = None;
        } else {
            self.board_send(st);
        }
        st.save_config();
        self.say(format!("Back to {}", bf.label), "");
    }

    /// A song or part changed: what Back would go back to is gone (the song's tone comes back by itself).
    pub(super) fn sounds_forget(&mut self) {
        self.board.sound_before = None;
        self.board.sounds = None;
    }

    /// Screenshot mode: the list open with the sound in favourite slot 2 put on (Back showing), or nothing open.
    pub(crate) fn sounds_shot(&mut self, st: &mut AppState, on: bool) {
        self.sounds_forget();
        if !on {
            return;
        }
        self.board.sounds = Some(SoundPick::default());
        self.sounds_refresh(st);
        let rigs = self
            .board
            .sounds
            .as_ref()
            .map(|s| s.rigs.clone())
            .unwrap_or_default();
        if let Some(r) = rigs.iter().find(|r| r.slot == 2).or(rigs.first()) {
            self.put_sound(st, r);
            self.said.clear();
        }
        let rows = self.sound_rows();
        let at = rows
            .iter()
            .position(|r| matches!(r, Row::Sound(i) if rigs[*i].slot == 2))
            .unwrap_or(0);
        if let Some(sp) = self.board.sounds.as_mut() {
            sp.sel = at;
        }
    }

    fn sounds_pick_row(&mut self, st: &mut AppState, row: Row) {
        match row {
            Row::Back => self.sound_back(st),
            Row::Sound(i) => {
                let r = self.board.sounds.as_ref().and_then(|sp| sp.rigs.get(i).cloned());
                if let Some(r) = r {
                    self.put_sound(st, &r);
                }
            }
        }
        let n = self.sound_rows().len();
        if let Some(sp) = self.board.sounds.as_mut() {
            sp.sel = sp.sel.min(n.saturating_sub(1));
        }
    }

    /// The keys while the list is open: typing searches, the arrows pick, Enter puts it on, Esc (or Shift T)
    /// closes. Space and the seeking keys still go to the song. True: the event was the list's.
    pub(super) fn sounds_key(&mut self, st: &mut AppState, ev: &Event) -> bool {
        if self.board.sounds.is_none() {
            return false;
        }
        match ev {
            Event::Text(s) => {
                if s.trim().is_empty() {
                    return false;
                }
                if let Some(sp) = self.board.sounds.as_mut() {
                    for ch in s.chars().filter(|c| !c.is_control()) {
                        if sp.query.chars().count() < 40 {
                            sp.query.push(ch);
                        }
                    }
                    sp.sel = 0;
                    sp.scroll = 0.0;
                }
                true
            }
            Event::Key {
                key,
                pressed,
                modifiers,
                ..
            } => {
                if matches!(
                    key,
                    Key::Space | Key::ArrowLeft | Key::ArrowRight | Key::PageUp | Key::PageDown | Key::F1
                ) {
                    return false;
                }
                if !pressed {
                    return true;
                }
                let n = self.sound_rows().len();
                match key {
                    Key::Escape => self.board.sounds = None,
                    Key::T if modifiers.shift => self.board.sounds = None,
                    Key::Backspace => {
                        if let Some(sp) = self.board.sounds.as_mut() {
                            sp.query.pop();
                            sp.sel = 0;
                        }
                    }
                    Key::ArrowDown | Key::ArrowUp => {
                        if let Some(sp) = self.board.sounds.as_mut()
                            && n > 0
                        {
                            sp.sel = if *key == Key::ArrowDown {
                                (sp.sel + 1).min(n - 1)
                            } else {
                                sp.sel.saturating_sub(1)
                            };
                            // keep it in view
                            if let Some(r) = sp.rows.get(sp.sel).copied() {
                                if r.max.y > sp.list.max.y {
                                    sp.scroll += r.max.y - sp.list.max.y;
                                } else if r.min.y < sp.list.min.y {
                                    sp.scroll = (sp.scroll - (sp.list.min.y - r.min.y)).max(0.0);
                                }
                            }
                        }
                    }
                    Key::Enter => {
                        let sel = self.board.sounds.as_ref().map(|sp| sp.sel).unwrap_or(0);
                        if let Some(row) = self.sound_rows().get(sel).copied() {
                            self.sounds_pick_row(st, row);
                        }
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }

    /// The wheel over the list scrolls it (true: it was over the list).
    pub(super) fn sounds_wheel(&mut self, pos: Pos2, notches: f32) -> bool {
        let Some(sp) = self.board.sounds.as_mut() else {
            return false;
        };
        if !sp.list.contains(pos) {
            return false;
        }
        sp.scroll = (sp.scroll - notches * px(36.0)).max(0.0);
        true
    }

    /// The list, in `area` (where the block's face is).
    pub(super) fn sounds_ui(&mut self, ui: &Ui, st: &mut AppState, area: Rect) {
        self.sounds_refresh(st);
        let p = ui.painter().clone();
        let th = t();
        let ctx = ui.ctx().clone();
        let rows = self.sound_rows();
        let on_now = self.sound_on(st);
        let before = self.board.sound_before.as_ref().map(|b| b.label.clone());
        let Some(sp) = self.board.sounds.as_mut() else {
            return;
        };
        let count = sp.rigs.len();
        // the search, and the close key
        let head_h = px(30.0);
        let ks = px(22.0);
        let close = Rect::from_min_size(
            pos2(area.max.x - ks, area.min.y + (head_h - ks) / 2.0),
            Vec2::splat(ks),
        );
        let sr = Rect::from_min_max(area.min, pos2(close.min.x - px(8.0), area.min.y + head_h));
        paint::inset(&p, sr, Some(px(10.0)), 2.0, Fill::Base);
        p.rect_stroke(
            sr,
            px(10.0),
            Stroke::new(1.5, th.accent.gamma_multiply(0.8)),
            egui::StrokeKind::Inside,
        );
        rd::icon_search(&p, pos2(sr.min.x + px(16.0), sr.center().y), px(14.0), th.accent);
        let tx = sr.min.x + px(30.0);
        let qr = text::draw(
            &p,
            &sp.query,
            pos2(tx, sr.center().y),
            Align2::LEFT_CENTER,
            Style::new(13.0, 800, th.text),
            Some(sr.width() - px(110.0)),
        );
        rd::caret(
            &p,
            if sp.query.is_empty() { tx } else { qr.max.x },
            sr.center().y,
            px(16.0),
            th.accent,
        );
        if sp.query.is_empty() {
            text::draw(
                &p,
                "Your sounds \u{b7} type to search",
                pos2(tx + px(6.0), sr.center().y),
                Align2::LEFT_CENTER,
                Style::new(12.0, 600, th.caption),
                Some(sr.width() - px(110.0)),
            );
        }
        let shown = rows.iter().filter(|r| matches!(r, Row::Sound(_))).count();
        text::draw(
            &p,
            &if sp.query.is_empty() {
                format!("{count} saved")
            } else {
                format!("{shown} of {count}")
            },
            pos2(sr.max.x - px(10.0), sr.center().y),
            Align2::RIGHT_CENTER,
            Style::new(11.0, 700, th.text_2),
            None,
        );
        let close = widgets::round_button(
            ui,
            close,
            "board-sounds-close",
            Icon::X,
            9.0,
            RoundKind::Ivory,
            false,
        )
        .clicked();
        // the rows
        let hint_h = px(16.0);
        let list = Rect::from_min_max(
            pos2(area.min.x, sr.max.y + px(6.0)),
            pos2(area.max.x, area.max.y - hint_h),
        );
        sp.list = list;
        let lp = p.with_clip_rect(list);
        let row_h = px(36.0);
        let content = rows.len() as f32 * (row_h + px(3.0));
        sp.scroll = sp.scroll.min((content - list.height()).max(0.0));
        sp.sel = sp.sel.min(rows.len().saturating_sub(1));
        sp.rows.clear();
        let mut picked: Option<Row> = None;
        if rows.is_empty() {
            text::draw(
                &p,
                if count == 0 {
                    "No saved sounds yet: build one in the amp room (Home, AMP ROOM) and save it"
                } else {
                    "No sound matches"
                },
                list.center(),
                Align2::CENTER_CENTER,
                Style::new(12.5, 600, th.caption),
                Some(list.width() - px(20.0)),
            );
        }
        let mut y = list.min.y - sp.scroll;
        for (i, row) in rows.iter().enumerate() {
            let rr = Rect::from_min_size(pos2(list.min.x, y), vec2(list.width(), row_h));
            y += row_h + px(3.0);
            sp.rows.push(rr);
            if rr.max.y < list.min.y || rr.min.y > list.max.y {
                continue;
            }
            let resp = interact(ui, rr.intersect(list), ("board-sound", i), Sense::click());
            let sel = sp.sel == i;
            if sel {
                lp.rect_filled(rr, px(10.0), th.accent.gamma_multiply(0.08));
                lp.rect_stroke(
                    rr,
                    px(10.0),
                    Stroke::new(1.5, th.accent),
                    egui::StrokeKind::Inside,
                );
            } else if resp.hovered() {
                lp.rect_filled(rr, px(10.0), th.base_hi.gamma_multiply(0.5));
            }
            if resp.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if resp.clicked() {
                sp.sel = i;
                picked = Some(*row);
            }
            let well =
                Rect::from_center_size(pos2(rr.min.x + px(20.0), rr.center().y), vec2(px(26.0), px(26.0)));
            paint::inset(&lp, well, Some(px(7.0)), 1.5, Fill::Base);
            let tx = well.max.x + px(10.0);
            match row {
                Row::Back => {
                    text::draw(
                        &lp,
                        "\u{21a9}",
                        well.center(),
                        Align2::CENTER_CENTER,
                        Style::new(13.0, 800, th.accent),
                        None,
                    );
                    text::draw(
                        &lp,
                        &format!("Back to {}", before.as_deref().unwrap_or("what you had")),
                        pos2(tx, rr.center().y - px(7.0)),
                        Align2::LEFT_CENTER,
                        Style::new(12.5, 800, th.accent_ink),
                        Some(rr.max.x - tx - px(10.0)),
                    );
                    text::draw(
                        &lp,
                        "as it was before you picked a sound",
                        pos2(tx, rr.center().y + px(8.0)),
                        Align2::LEFT_CENTER,
                        Style::new(10.5, 600, th.caption),
                        Some(rr.max.x - tx - px(10.0)),
                    );
                }
                Row::Sound(k) => {
                    let rg = &sp.rigs[*k];
                    let col = rgb(COLOURS[rg.color.min(COLOURS.len() - 1)]);
                    lp.rect_stroke(well, px(7.0), Stroke::new(1.5, col), egui::StrokeKind::Inside);
                    text::draw(
                        &lp,
                        "\u{2605}",
                        well.center(),
                        Align2::CENTER_CENTER,
                        Style::new(12.0, 700, col),
                        None,
                    );
                    let current = on_now.as_ref().is_some_and(|n| n.eq_ignore_ascii_case(&rg.name));
                    // the right: the slot's number in its colour, ON for the one playing, the chain as icons
                    let mut rx = rr.max.x - px(10.0);
                    if rg.slot > 0 {
                        let r = text::draw(
                            &lp,
                            &rg.slot.to_string(),
                            pos2(rx, rr.center().y),
                            Align2::RIGHT_CENTER,
                            Style::new(12.0, 800, col),
                            None,
                        );
                        rx = r.min.x - px(8.0);
                    }
                    if current {
                        let r = text::caps(
                            &lp,
                            "ON",
                            pos2(rx, rr.center().y),
                            9.0,
                            th.accent_ink,
                            Align2::RIGHT_CENTER,
                            None,
                        );
                        rx = r.min.x - px(8.0);
                    }
                    // (the name first: the chain's icons take what room is left, three at least, else none)
                    let full = text::width(&ctx, &rg.name, Style::new(12.5, 800, th.text));
                    let spare = rx - (tx + full + px(14.0)) - px(18.0);
                    let fit = ((spare / px(15.5)).floor().max(0.0) as usize)
                        .min(6)
                        .min(rg.board.blocks.len());
                    let mut name_w = rx - tx - px(6.0);
                    if fit >= 3 {
                        let more = if fit < rg.board.blocks.len() {
                            px(18.0)
                        } else {
                            0.0
                        };
                        let ix = rx - fit as f32 * px(15.5) - more;
                        rd::chain_icons(&lp, &rg.board, ix, rr.center().y, px(13.0), fit);
                        name_w = ix - tx - px(8.0);
                    }
                    text::draw(
                        &lp,
                        &rg.name,
                        pos2(tx, rr.center().y - px(7.0)),
                        Align2::LEFT_CENTER,
                        Style::new(12.5, 800, if current { th.accent_ink } else { th.text }),
                        Some(name_w),
                    );
                    let mut sub = if rg.slot > 0 {
                        format!("favourite {}", rg.slot)
                    } else {
                        String::new()
                    };
                    if !rg.tags.is_empty() {
                        if !sub.is_empty() {
                            sub.push_str(" \u{b7} ");
                        }
                        sub.push_str(&rg.tags.join(", ").to_lowercase());
                    }
                    if sub.is_empty() {
                        sub = if rg.based_on.is_empty() {
                            "your sound".into()
                        } else {
                            format!("based on {}", rg.based_on)
                        };
                    }
                    text::draw(
                        &lp,
                        &sub,
                        pos2(tx, rr.center().y + px(8.0)),
                        Align2::LEFT_CENTER,
                        Style::new(10.5, 600, th.caption),
                        Some(name_w),
                    );
                    if resp.hovered() {
                        self.tip = Some((
                            rg.chain(),
                            String::new(),
                            pos2(rr.center().x, rr.max.y + px(4.0)),
                            Align2::CENTER_TOP,
                        ));
                    }
                }
            }
        }
        text::draw(
            &p,
            "\u{2191}\u{2193} pick \u{b7} Enter puts it on \u{b7} Esc closes",
            pos2(area.min.x + px(2.0), area.max.y - hint_h / 2.0 + px(1.0)),
            Align2::LEFT_CENTER,
            Style::new(10.5, 600, th.caption),
            Some(area.width()),
        );
        if let Some(row) = picked {
            self.sounds_pick_row(st, row);
        }
        if close {
            self.board.sounds = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rig(name: &str, slot: u8) -> Rig {
        let (_, b) = board::presets().swap_remove(1);
        Rig {
            name: name.into(),
            tags: vec!["Clean".into()],
            color: 1,
            slot,
            based_on: String::new(),
            board: b,
            scenes: Default::default(),
            saved: 0.0,
        }
    }

    #[test]
    fn back_comes_first_then_slots_then_the_rest() {
        let list = vec![rig("Bravo", 0), rig("Alpha", 3), rig("Charlie", 1)];
        assert_eq!(
            rows(false, &list, ""),
            vec![Row::Sound(2), Row::Sound(1), Row::Sound(0)]
        );
        assert_eq!(
            rows(true, &list, ""),
            vec![Row::Back, Row::Sound(2), Row::Sound(1), Row::Sound(0)]
        );
        // Back stays while searching
        assert_eq!(rows(true, &list, "alpha"), vec![Row::Back, Row::Sound(1)]);
        assert_eq!(rows(true, &list, "nothing like it"), vec![Row::Back]);
    }

    #[test]
    fn a_sound_put_on_and_taken_back_leaves_config_as_it_was() {
        let mut cfg = Config::default();
        let (_, mine) = board::presets().swap_remove(0);
        pb::set_my_board(&mut cfg, &mine);
        let room_before = room_now(&cfg);
        assert!(room_before.is_none());
        let mut r = rig("Glass clean", 1);
        let mut s = r.board.clone();
        s.blocks[0].on = !s.blocks[0].on;
        r.scenes[0] = Some(s.clone());
        let b = put_on(&mut cfg, &r);
        // your board is the sound at its first scene, and the room's state names it
        assert!(rigs::same_sound(&pb::my_board(&cfg), &b));
        assert_eq!(b.blocks[0].on, s.blocks[0].on);
        assert_eq!(RoomState::load(&cfg).rig.as_deref(), Some("Glass clean"));
        // back: the board and the room as they were (no `room` key, as there was none)
        pb::set_my_board(&mut cfg, &mine);
        room_back(&mut cfg, &room_before);
        assert!(rigs::same_sound(&pb::my_board(&cfg), &mine));
        assert!(cfg.get("room").is_none());
        // a room state that was there comes back as it was
        let mut cfg = Config::default();
        rigs::state_for(&rig("Old", 0)).save(&mut cfg);
        let was = room_now(&cfg);
        put_on(&mut cfg, &r);
        room_back(&mut cfg, &was);
        assert_eq!(RoomState::load(&cfg).rig.as_deref(), Some("Old"));
    }
}
