//! The strip along the room's foot, always there (Room-Main): the input meter with its clip light and trim, the
//! noise gate, the tuner, scenes 1-4, stepping through rigs, the master output.

use eframe::egui;
use egui::{Align2, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use qrs_audio::board::{self, Block};
use qrs_ui::paint::{self, Fill};
use qrs_ui::text::{self, Style};
use qrs_ui::theme::t;
use qrs_ui::units::px;
use qrs_ui::widgets::{self, interact};

use super::RoomView;
use super::draw::{self, KeyLook};
use super::rigs::{SCENE_COLOURS, SCENE_NAMES};
use crate::ampbutton::{rgb, rgba};
use crate::state::AppState;

const GREEN: Color32 = Color32::from_rgb(0x3e, 0xc4, 0x6d);
const AMBER: Color32 = Color32::from_rgb(0xf2, 0xb5, 0x3c);
const RED: Color32 = Color32::from_rgb(0xf0, 0x64, 0x5a);

/// What the wheel turns on the strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum What {
    Trim,
    Gate,
    Master,
}

/// The board's noise gate: the first "gate" block (its index).
fn gate_of(b: &board::Board) -> Option<usize> {
    b.blocks.iter().position(|x| x.def().id == "gate")
}

fn gate_model() -> usize {
    board::catalog::find("gate").unwrap_or(0)
}

/// The trim of the input now (dB), and a change to it kept with its calibration (config.json `input_cal`).
fn trim(st: &AppState) -> f32 {
    crate::calibrate::current_input(st)
        .map(|(id, _)| crate::calibrate::load(&st.config, &id).trim_db)
        .unwrap_or(0.0)
}

fn set_trim(st: &mut AppState, db: f32) {
    let Some((id, _)) = crate::calibrate::current_input(st) else {
        return;
    };
    let mut cal = crate::calibrate::load(&st.config, &id);
    cal.trim_db = (db * 2.0).round() / 2.0;
    cal.trim_db = cal
        .trim_db
        .clamp(crate::calibrate::TRIM.0, crate::calibrate::TRIM.1);
    crate::calibrate::save(&mut st.config, &id, &cal);
    crate::calibrate::apply(st);
}

pub fn strip(room: &mut RoomView, ui: &Ui, st: &mut AppState, r: Rect) {
    let p = ui.painter().clone();
    let th = t();
    let ctx = ui.ctx().clone();
    room.strip_spots.clear();
    draw::card(&p, r, px(18.0));
    let k = r.width() / px(1400.0);
    let sx = |v: f32| r.min.x + px(v) * k;
    let y = r.min.y;
    let sep = |x: f32| {
        p.line_segment(
            [pos2(sx(x), y + px(14.0)), pos2(sx(x), r.max.y - px(14.0))],
            Stroke::new(1.0, th.divider),
        );
    };
    let cap = |s: &str, x: f32, yy: f32| {
        draw::caps(&p, s, pos2(x, y + yy), 10.0, th.caption, Align2::LEFT_CENTER);
    };
    // A. the input
    let ax = sx(20.0);
    let dev = room
        .rack
        .engine()
        .and_then(|e| e.status().input.map(|i| (i.device_name, e.channel())));
    let head = match &dev {
        Some((n, ch)) => {
            let short: String = n.split('(').next().unwrap_or(n).trim().chars().take(16).collect();
            if *ch >= 0 {
                format!("INPUT  \u{b7}  {}  \u{b7}  CH {}", short.to_uppercase(), ch + 1)
            } else {
                format!("INPUT  \u{b7}  {}", short.to_uppercase())
            }
        }
        None => "INPUT  \u{b7}  NONE".into(),
    };
    text::caps(
        &p,
        &head,
        pos2(ax, y + px(16.0)),
        10.0,
        th.caption,
        Align2::LEFT_CENTER,
        Some(sx(268.0) - ax),
    );
    let seg_w = (px(6.0) * k).max(px(4.0));
    let seg_g = px(2.0) * k;
    let lit_to = ((room.listen.peak_db + 60.0) / 60.0 * 24.0).clamp(0.0, 24.0);
    for i in 0..24 {
        let col = if i < 16 {
            GREEN
        } else if i < 21 {
            AMBER
        } else {
            RED
        };
        let sr = Rect::from_min_size(
            pos2(ax + i as f32 * (seg_w + seg_g), y + px(28.0)),
            vec2(seg_w, px(12.0)),
        );
        let on = (i as f32) < lit_to;
        p.rect_filled(sr, px(2.0), if on { col } else { col.gamma_multiply(0.16) });
    }
    let hold = ((room.listen.hold_db + 60.0) / 60.0 * 24.0).clamp(0.0, 24.0);
    if hold > 0.5 {
        let hx = ax + hold * (seg_w + seg_g) - seg_g;
        p.rect_filled(
            Rect::from_min_size(pos2(hx, y + px(25.0)), vec2(px(2.0), px(18.0))),
            1.0,
            AMBER,
        );
    }
    let lc = pos2(ax + 24.0 * (seg_w + seg_g) + px(12.0), y + px(34.0));
    if room.listen.lamp() {
        paint::halo(&p, lc, px(10.0), RED, 120);
        p.circle_filled(lc, px(6.0), RED);
    } else {
        p.circle_filled(lc, px(6.0), rgb([0x3a, 0x16, 0x14]));
        p.circle_stroke(lc, px(6.0), Stroke::new(1.0, Color32::BLACK));
    }
    draw::caps(
        &p,
        "CLIP",
        pos2(lc.x + px(10.0), lc.y),
        9.5,
        th.caption,
        Align2::LEFT_CENTER,
    );
    let lvl = room.listen.level_db;
    let lcol = if room.listen.lamp() {
        RED
    } else if lvl > -50.0 {
        GREEN
    } else {
        th.caption
    };
    let tr = text::draw_runs(
        &p,
        &[
            (
                &if lvl > -100.0 {
                    format!("{lvl:.0} dB")
                } else {
                    "\u{2212}\u{221e}".into()
                },
                Style::new(10.5, 800, lcol),
            ),
            (
                &format!(
                    "  peak {:.0}  \u{b7}  trim {:+.1} dB",
                    room.listen.hold_db.max(-99.0),
                    trim(st)
                ),
                Style::new(10.5, 700, th.caption),
            ),
        ],
        pos2(ax, y + px(56.0)),
        Align2::LEFT_CENTER,
    );
    room.strip_spots.push((tr.expand(px(4.0)), What::Trim));
    let tb = |i: usize| {
        Rect::from_center_size(
            pos2(sx(242.0) + i as f32 * px(22.0), y + px(56.0)),
            vec2(px(18.0), px(18.0)),
        )
    };
    for (i, (lab, d)) in [("\u{2212}", -0.5f32), ("+", 0.5)].iter().enumerate() {
        let rr = tb(i);
        let resp = interact(ui, rr, ("room-trim", i), Sense::click());
        paint::inset(&p, rr, Some(px(6.0)), 1.5, Fill::Base);
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            room.rack.tip = Some((
                "The input's trim (this input's calibration)".into(),
                String::new(),
                pos2(rr.center().x, rr.min.y - px(6.0)),
                Align2::CENTER_BOTTOM,
            ));
        }
        text::draw(
            &p,
            lab,
            rr.center(),
            Align2::CENTER_CENTER,
            Style::new(12.0, 800, th.text_2),
            None,
        );
        if resp.clicked() {
            set_trim(st, trim(st) + d);
        }
    }
    sep(272.0);
    // B. the noise gate (the board's own gate, first in the chain)
    let bx = sx(290.0);
    let b = room.rack.board_now(st);
    let gate = gate_of(&b).map(|i| b.blocks[i]);
    let on = gate.is_some_and(|g| g.on);
    cap("NOISE GATE", bx, 16.0);
    let swr = Rect::from_min_size(pos2(bx + px(96.0), y + px(8.0)), vec2(px(30.0), px(17.0)));
    if widgets::switch(ui, swr, "room-gate", on).clicked() {
        match gate {
            Some(g) => room.rack.toggle_block(st, g.uid),
            None => {
                room.rack.add_block(st, 0, gate_model());
                room.rack.said.clear();
                room.say(
                    "Noise gate on",
                    "  \u{b7}  first in the chain; the wheel sets its threshold",
                );
            }
        }
    }
    let thr = gate
        .and_then(|g| g.get("threshold"))
        .unwrap_or_else(|| Block::new(gate_model()).get("threshold").unwrap_or(-56.0));
    let dr = draw::label(
        &p,
        &format!("{thr:.0} dB"),
        pos2(bx, y + px(42.0)),
        17.0,
        800,
        if on { th.text } else { th.caption },
        Align2::LEFT_CENTER,
    );
    room.strip_spots.push((dr.expand(px(6.0)), What::Gate));
    let open = !on || lvl > thr;
    let oc = pos2(dr.max.x + px(12.0), y + px(43.0));
    p.circle_filled(
        oc,
        px(3.5),
        if !on {
            th.unlit
        } else if open {
            GREEN
        } else {
            th.caption
        },
    );
    draw::label(
        &p,
        if !on {
            "off"
        } else if open {
            "open"
        } else {
            "shut"
        },
        pos2(oc.x + px(8.0), oc.y),
        11.0,
        700,
        th.caption,
        Align2::LEFT_CENTER,
    );
    sep(436.0);
    // C. the tuner
    let cx = sx(454.0);
    cap("TUNER  \u{b7}  RAW INPUT", cx, 16.0);
    let note = room.listen.note();
    let (name, oct, cents) = note.clone().unwrap_or(("\u{2013}".into(), 0, 0));
    let nr = draw::label(
        &p,
        &name,
        pos2(cx, y + px(46.0)),
        32.0,
        800,
        if note.is_some() { th.text } else { th.caption },
        Align2::LEFT_CENTER,
    );
    if note.is_some() {
        draw::label(
            &p,
            &oct.to_string(),
            pos2(nr.max.x + px(2.0), y + px(52.0)),
            14.0,
            800,
            th.caption,
            Align2::LEFT_CENTER,
        );
    }
    let bw = px(140.0) * k;
    let bx0 = cx + px(62.0);
    let bar = Rect::from_min_size(pos2(bx0, y + px(30.0)), vec2(bw, px(18.0)));
    p.rect_filled(
        Rect::from_center_size(bar.center(), vec2(px(18.0), px(18.0))),
        px(4.0),
        GREEN.gamma_multiply(0.14),
    );
    for i in 0..=10 {
        let x = bar.min.x + bw * i as f32 / 10.0;
        let (t0, h) = if i % 5 == 0 {
            (px(2.0), px(14.0))
        } else {
            (px(6.0), px(6.0))
        };
        p.line_segment(
            [pos2(x, bar.min.y + t0), pos2(x, bar.min.y + t0 + h)],
            Stroke::new(1.0, if i == 5 { th.caption } else { th.divider }),
        );
    }
    let in_tune = cents.abs() <= 5;
    if note.is_some() {
        let nx = bar.center().x + (cents as f32 / 50.0).clamp(-1.0, 1.0) * bw / 2.0;
        let nc = if in_tune { GREEN } else { AMBER };
        paint::halo(&p, pos2(nx, bar.center().y), px(8.0), nc, 60);
        p.rect_filled(
            Rect::from_center_size(pos2(nx, bar.center().y), vec2(px(3.0), px(22.0))),
            px(2.0),
            nc,
        );
    }
    let hint = if note.is_none() {
        "play a string".to_string()
    } else if in_tune {
        "in tune".into()
    } else if cents < 0 {
        "flat".into()
    } else {
        "sharp".into()
    };
    text::draw_runs(
        &p,
        &[
            (
                &if note.is_some() {
                    format!("{cents:+} c  ")
                } else {
                    String::new()
                },
                Style::new(10.5, 800, if in_tune { GREEN } else { AMBER }),
            ),
            (&hint, Style::new(10.5, 700, th.caption)),
        ],
        pos2(bx0, y + px(58.0)),
        Align2::LEFT_CENTER,
    );
    sep(666.0);
    // D. the scenes
    let dx = sx(684.0);
    cap("SCENES  \u{b7}  KEYS 1-4", dx, 12.0);
    let sw = px(76.0) * k;
    for i in 0..4 {
        let col = rgb(SCENE_COLOURS[i]);
        let sr = Rect::from_min_size(
            pos2(dx + i as f32 * (sw + px(6.0) * k), y + px(26.0)),
            vec2(sw, px(36.0)),
        );
        let resp = interact(ui, sr, ("room-scene", i), Sense::click());
        let on = room.state.scene == i;
        if on {
            p.rect_filled(sr, px(10.0), col.gamma_multiply(0.18));
            paint::glow(&p, sr, col, px(8.0), Some(0.25), Some(px(10.0)));
            p.rect_stroke(sr, px(10.0), Stroke::new(1.5, col), egui::StrokeKind::Inside);
        } else {
            paint::inset(&p, sr, Some(px(10.0)), 1.5, Fill::Base);
            if resp.hovered() {
                p.rect_stroke(
                    sr,
                    px(10.0),
                    Stroke::new(1.0, col.gamma_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        p.rect_filled(
            Rect::from_min_size(
                pos2(sr.min.x + px(9.0), sr.min.y + px(7.0)),
                vec2(px(22.0), px(4.0)),
            ),
            px(2.0),
            if on { col } else { col.gamma_multiply(0.5) },
        );
        let has = room.state.scenes[i].is_some() || on;
        draw::label(
            &p,
            SCENE_NAMES[i],
            pos2(sr.min.x + px(9.0), sr.min.y + px(23.0)),
            12.5,
            800,
            if on {
                th.text
            } else if has {
                th.text_2
            } else {
                th.caption
            },
            Align2::LEFT_CENTER,
        );
        draw::label(
            &p,
            &(i + 1).to_string(),
            pos2(sr.max.x - px(8.0), sr.min.y + px(9.0)),
            10.0,
            800,
            if on { col } else { th.caption },
            Align2::RIGHT_CENTER,
        );
        if resp.clicked() {
            room.scene(st, i);
        }
    }
    sep(1022.0);
    // E. stepping through rigs
    let ex = sx(1040.0);
    let list = room.rig_list();
    let at = room.rig_index(&list);
    cap(
        &match at {
            Some(i) => format!("RIG {} OF {}", i + 1, list.len()),
            None => format!("{} RIGS", list.len()),
        },
        ex,
        16.0,
    );
    let pr = Rect::from_min_size(pos2(ex, y + px(30.0)), vec2(px(30.0), px(30.0)));
    let nr_ = Rect::from_min_size(pos2(sx(1040.0 + 140.0), y + px(30.0)), vec2(px(30.0), px(30.0)));
    if draw::key(
        ui,
        pr,
        "room-rig-prev",
        KeyLook::Dark,
        "",
        "",
        Some(&|p: &egui::Painter, c: Pos2, s: f32, col: Color32| draw::icon_chev(p, c, s, col, -1.0)),
    )
    .clicked()
    {
        room.step_rig(st, -1);
    }
    if draw::key(
        ui,
        nr_,
        "room-rig-next",
        KeyLook::Dark,
        "",
        "",
        Some(&|p: &egui::Painter, c: Pos2, s: f32, col: Color32| draw::icon_chev(p, c, s, col, 1.0)),
    )
    .clicked()
    {
        room.step_rig(st, 1);
    }
    let mid_w = nr_.min.x - pr.max.x - px(12.0);
    draw::label_w(
        &p,
        &room.title(),
        pos2(pr.max.x + px(8.0), y + px(38.0)),
        10.5,
        700,
        th.text_2,
        Align2::LEFT_CENTER,
        mid_w,
    );
    if !list.is_empty() {
        let next = at.map(|i| (i + 1) % list.len()).unwrap_or(0);
        text::draw_runs(
            &p,
            &[
                ("next: ", Style::new(10.5, 700, th.text_2)),
                (&list[next].0, Style::new(10.5, 700, th.caption)),
            ],
            pos2(pr.max.x + px(8.0), y + px(53.0)),
            Align2::LEFT_CENTER,
        );
    }
    sep(1222.0);
    // F. the master output: the board's level, and what comes out
    let fx = sx(1240.0);
    cap("MASTER OUT", fx, 16.0);
    let lvl_db = b.level_db;
    let fw = (r.max.x - px(20.0) - fx).max(px(60.0));
    draw::label(
        &p,
        &format!("{lvl_db:+.1} dB"),
        pos2(fx + fw, y + px(16.0)),
        10.5,
        800,
        GREEN,
        Align2::RIGHT_CENTER,
    );
    let groove = Rect::from_min_size(pos2(fx, y + px(34.0)), vec2(fw, px(6.0)));
    let u = (lvl_db + 24.0) / 36.0;
    let hit = groove.expand2(vec2(px(8.0), px(10.0)));
    room.strip_spots.push((hit, What::Master));
    let resp = interact(ui, hit, "room-master", Sense::click_and_drag());
    paint::inset(&p, groove, Some(px(3.0)), 1.0, Fill::Base);
    p.rect_filled(
        Rect::from_min_size(groove.min, vec2(groove.width() * u, groove.height())),
        px(3.0),
        AMBER,
    );
    qrs_ui::keys::thumb(
        &p,
        pos2(groove.min.x + groove.width() * u, groove.center().y),
        px(16.0),
    );
    if resp.hovered() || resp.dragged() {
        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    if (resp.dragged() || resp.clicked())
        && let Some(pos) = resp.interact_pointer_pos()
    {
        let nu = ((pos.x - groove.min.x) / groove.width()).clamp(0.0, 1.0);
        let db = ((-24.0 + 36.0 * nu) * 2.0).round() / 2.0;
        room.rack.board_edit(st, |x| x.level_db = db);
    }
    if resp.drag_stopped() || resp.clicked() {
        room.rack.board_commit(st);
    }
    if resp.double_clicked() {
        room.rack.board_edit(st, |x| x.level_db = 0.0);
        room.rack.board_commit(st);
    }
    let out = room.rack.engine().map(|e| e.meters().tone_peak).unwrap_or(0.0);
    let odb = 20.0 * out.max(1e-6).log10();
    let ou = ((odb + 60.0) / 60.0).clamp(0.0, 1.0);
    let og = Rect::from_min_size(pos2(fx, y + px(50.0)), vec2(fw, px(4.0)));
    p.rect_filled(og, px(2.0), th.track);
    p.rect_filled(
        Rect::from_min_size(og.min, vec2(og.width() * ou, og.height())),
        px(2.0),
        if odb > -1.0 { RED } else { GREEN },
    );
    let _ = rgba;
}

/// The wheel over the trim, the gate's threshold or the master.
pub fn wheel(room: &mut RoomView, st: &mut AppState, pos: Pos2, notches: f32, shift: bool) -> bool {
    let Some(&(_, what)) = room.strip_spots.iter().find(|(r, _)| r.contains(pos)) else {
        return false;
    };
    let step = if shift { 0.5 } else { 1.0 } * notches.signum();
    match what {
        What::Trim => set_trim(st, trim(st) + step * 0.5),
        What::Gate => {
            let b = room.rack.board_now(st);
            let Some(i) = gate_of(&b) else { return true };
            let g = b.blocks[i];
            let k = g.def().knob_index("threshold").unwrap_or(0);
            room.rack.set_knob(st, g.uid, k, g.v[k] + step);
            room.wheel_at = Some(std::time::Instant::now());
        }
        What::Master => {
            room.rack
                .board_edit(st, |x| x.level_db = (x.level_db + step * 0.5).clamp(-24.0, 12.0));
            room.wheel_at = Some(std::time::Instant::now());
        }
    }
    true
}
