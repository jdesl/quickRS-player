//! The Rig tab: the stage (the pedalboard's own board, drawn on a stage), the selected unit's knobs and the gear
//! shelf (Room-Main).

use eframe::egui;
use egui::{Align2, Color32, CursorIcon, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use qrs_audio::board::{Block, Kind, MODELS, Unit};
use qrs_ui::paint;
use qrs_ui::text::{self, Style};
use qrs_ui::theme::t;
use qrs_ui::units::px;
use qrs_ui::widgets::interact;

use super::draw::{self, KeyLook};
use super::{RoomView, Typing};
use crate::ampbutton::{rgb, rgba, rounded_vgrad};
use crate::pedalfaces as faces;
use crate::player::board::Picker;
use crate::state::AppState;

/// Words on the stage (dark in every theme).
const STAGE_DIM: Color32 = Color32::from_rgb(0x8a, 0x90, 0x99);

/// A knob of the selected card being dragged: (block, knob, where the drag started, its value there 0..1).
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    uid: u32,
    knob: usize,
    y0: f32,
    u0: f32,
}

/// Where the wheel turns something of the room's: (where, block, knob).
#[derive(Clone, Copy, Debug)]
pub struct Spot {
    pub rect: Rect,
    pub uid: u32,
    pub knob: usize,
}

pub fn rig_page(room: &mut RoomView, ui: &Ui, st: &mut AppState, body: Rect) {
    let cards_h = px(208.0);
    let stage_r = Rect::from_min_max(body.min, pos2(body.max.x, body.max.y - cards_h - px(12.0)));
    let cards = Rect::from_min_max(pos2(body.min.x, stage_r.max.y + px(12.0)), body.max);
    room.spots.clear();
    stage(room, ui, st, stage_r);
    let split = cards.min.x + (cards.width() * 0.455).round();
    let sel_r = Rect::from_min_max(cards.min, pos2(split - px(6.0), cards.max.y));
    let shelf_r = Rect::from_min_max(pos2(split + px(6.0), cards.min.y), cards.max);
    selected_card(room, ui, st, sel_r);
    shelf_card(room, ui, st, shelf_r);
}

/// The stage: a dark room with a lit wall and a floor, the rig standing on it (the board's own drawing: click a
/// unit to show it, double-click for its editor, drag to move it, + to add one there).
fn stage(room: &mut RoomView, ui: &Ui, st: &mut AppState, r: Rect) {
    let p = ui.painter().clone();
    let th = t();
    let rad = px(20.0);
    rounded_vgrad(&p, r, rad, rgb([0x18, 0x19, 0x1c]), rgb([0x0e, 0x0f, 0x11]));
    // acoustic slats on the wall, the floor
    let floor_y = (r.max.y - px(112.0)).max(r.min.y + r.height() * 0.4);
    let mut x = r.min.x + px(16.0);
    let mut i = 0;
    while x < r.max.x - px(14.0) {
        let a = if i % 2 == 0 { 0.012 } else { 0.018 };
        p.rect_filled(
            Rect::from_min_max(pos2(x, r.min.y + px(2.0)), pos2(x + px(12.0), floor_y)),
            0.0,
            rgba(255, 255, 255, a),
        );
        x += px(26.0);
        i += 1;
    }
    let floor = Rect::from_min_max(pos2(r.min.x, floor_y), r.max);
    rounded_vgrad(&p, floor, rad, rgb([0x1a, 0x1b, 0x1e]), rgb([0x0c, 0x0d, 0x0f]));
    p.rect_filled(
        Rect::from_min_max(pos2(r.min.x, floor_y), pos2(r.max.x, floor_y + rad)),
        0.0,
        rgb([0x1a, 0x1b, 0x1e]),
    );
    p.line_segment(
        [pos2(r.min.x, floor_y), pos2(r.max.x, floor_y)],
        Stroke::new(1.0, rgba(255, 255, 255, 0.05)),
    );
    for dy in [14.0, 32.0, 56.0, 88.0, 128.0] {
        let y = floor_y + px(dy);
        if y < r.max.y - px(6.0) {
            p.line_segment(
                [pos2(r.min.x, y), pos2(r.max.x, y)],
                Stroke::new(0.8, rgba(255, 255, 255, 0.028)),
            );
        }
    }
    // a warm light over the rig (brighter while it hears you), a lamp's pool on the wall
    let live = room.listen.level_db > -50.0;
    draw::radial_glow(
        &p,
        pos2(r.center().x + r.width() * 0.1, floor_y - r.height() * 0.12),
        vec2(r.width() * 0.36, r.height() * 0.5),
        Color32::from_rgb(0xff, 0xb3, 0x47),
        if live { 0.16 } else { 0.08 },
    );
    draw::radial_glow(
        &p,
        pos2(r.center().x, r.min.y + px(20.0)),
        vec2(r.width() * 0.42, r.height() * 0.6),
        Color32::from_rgb(0xff, 0xf1, 0xd8),
        0.06,
    );
    p.rect_stroke(
        r,
        rad,
        Stroke::new(1.0, rgba(255, 255, 255, 0.04)),
        egui::StrokeKind::Inside,
    );
    // the rig
    let inner = Rect::from_min_max(
        pos2(r.min.x + px(8.0), r.min.y + px(40.0)),
        pos2(r.max.x - px(8.0), r.max.y - px(6.0)),
    );
    room.rack.board_area(ui, st, inner);
    // a + on the stage: the shelf adds there
    if let Some(Picker::Kinds { at } | Picker::Models { at, .. }) = room.rack.board.picker.take() {
        room.shelf_at = Some(at);
        room.say("Pick a unit on the shelf", "  \u{b7}  it goes where the + was");
    }
    // overlays: LIVE, the hint, what's on it
    let ty = r.min.y + px(22.0);
    let has_input = room.rack.engine().is_some_and(|e| e.status().input.is_some());
    let (dot, word, what) = if room.ab {
        (
            th.accent,
            "A/B",
            "you're hearing the saved sound \u{b7} ` flips back".to_string(),
        )
    } else if has_input || room.shot {
        let sc = room.state.scene;
        (
            Color32::from_rgb(0x3e, 0xc4, 0x6d),
            "LIVE",
            format!(
                "your guitar is playing through this rig \u{b7} scene {} {}",
                sc + 1,
                super::rigs::SCENE_NAMES[sc]
            ),
        )
    } else {
        (
            STAGE_DIM,
            "NO INPUT",
            "set up your guitar in Settings > Audio input / output (F2 on Home)".into(),
        )
    };
    let dc = pos2(r.min.x + px(24.0), ty);
    paint::halo(&p, dc, px(8.0), dot, 70);
    p.circle_filled(dc, px(3.5), dot);
    let wr = draw::caps(
        &p,
        word,
        pos2(dc.x + px(10.0), ty),
        11.0,
        dot,
        Align2::LEFT_CENTER,
    );
    draw::label(
        &p,
        &what,
        pos2(wr.max.x + px(10.0), ty),
        11.5,
        600,
        STAGE_DIM,
        Align2::LEFT_CENTER,
    );
    let b = room.rack.board_now(st);
    let on = b.blocks.iter().filter(|x| x.on).count();
    let count = format!(
        "{} block{} \u{b7} {on} on \u{b7} level {:+.1} dB",
        b.blocks.len(),
        if b.blocks.len() == 1 { "" } else { "s" },
        b.level_db
    );
    draw::label(
        &p,
        &count,
        pos2(r.max.x - px(22.0), ty),
        11.0,
        700,
        STAGE_DIM,
        Align2::RIGHT_CENTER,
    );
    let hint = match room.shelf_at {
        Some(at) => format!(
            "adding at {}: pick a unit on the shelf \u{b7} Esc cancels",
            at + 1
        ),
        None => "click a unit to tweak it \u{b7} drag to reorder \u{b7} double-click: its editor".into(),
    };
    draw::label(
        &p,
        &hint,
        pos2(r.center().x, ty),
        11.0,
        600,
        if room.shelf_at.is_some() {
            th.accent
        } else {
            STAGE_DIM
        },
        Align2::CENTER_CENTER,
    );
}

/// A big knob (the selected card's): its scale as an arc, the value's arc in the accent, a black cap and its
/// pointer; the name above, the value under.
fn big_knob(p: &Painter, c: Pos2, r: f32, u: f32, name: &str, value: &str, hot: bool) {
    let th = t();
    if hot {
        p.circle_filled(c, r + px(16.0), th.accent.gamma_multiply(0.07));
    }
    let arc = |a0: f32, a1: f32, col: Color32| {
        let n = 32;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                pos2(c.x + (r + px(9.0)) * a.sin(), c.y - (r + px(9.0)) * a.cos())
            })
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(px(4.0), col)));
    };
    arc(-135.0, 135.0, th.track);
    let a = -135.0 + 270.0 * u.clamp(0.0, 1.0);
    if u > 0.004 {
        arc(-135.0, a, th.accent);
    }
    // the cap: a shadow, a dark body, a ribbed rim, a lit top
    p.circle_filled(c + vec2(0.0, px(3.0)), r, rgba(0, 0, 0, 0.45));
    p.circle_filled(c, r, rgb([0x16, 0x17, 0x1a]));
    for i in 0..36 {
        let aa = (i as f32 * 10.0).to_radians();
        let o = pos2(c.x + r * aa.sin(), c.y - r * aa.cos());
        let n = pos2(c.x + (r - px(2.6)) * aa.sin(), c.y - (r - px(2.6)) * aa.cos());
        p.line_segment([o, n], Stroke::new(1.0, rgba(255, 255, 255, 0.07)));
    }
    p.circle_filled(c, r * 0.72, rgb([0x24, 0x26, 0x2a]));
    p.circle_filled(c + vec2(-r * 0.15, -r * 0.2), r * 0.42, rgba(255, 255, 255, 0.04));
    let ar = a.to_radians();
    p.line_segment(
        [
            pos2(c.x + r * 0.25 * ar.sin(), c.y - r * 0.25 * ar.cos()),
            pos2(c.x + r * 0.86 * ar.sin(), c.y - r * 0.86 * ar.cos()),
        ],
        Stroke::new(px(3.0), rgb([0xee, 0xee, 0xea])),
    );
    text::caps(
        p,
        name,
        pos2(c.x, c.y - r - px(22.0)),
        10.5,
        th.caption,
        Align2::CENTER_CENTER,
        Some(r * 3.2),
    );
    text::draw(
        p,
        value,
        pos2(c.x, c.y + r + px(25.0)),
        Align2::CENTER_CENTER,
        Style::new(15.0, 800, th.text),
        Some(r * 3.4),
    );
}

/// The selected unit: its name, its face, its front knobs large (drag, Shift fine, the wheel, double-click for the
/// default), on/off, the detailed editor, Randomise, Defaults.
fn selected_card(room: &mut RoomView, ui: &Ui, st: &mut AppState, r: Rect) {
    let p = ui.painter().clone();
    let th = t();
    let ctx = ui.ctx().clone();
    draw::card(&p, r, px(20.0));
    let b = room.rack.board_now(st);
    let Some(i) = room.rack.board.sel.and_then(|u| b.index_of(u)) else {
        draw::label_w(
            &p,
            "Click a unit on the stage to tweak it, or add one from the shelf.",
            r.center(),
            13.0,
            600,
            th.caption,
            Align2::CENTER_CENTER,
            r.width() - px(40.0),
        );
        return;
    };
    let blk = b.blocks[i];
    let def = blk.def();
    let x0 = r.min.x + px(22.0);
    draw::caps(
        &p,
        &format!(
            "SELECTED  \u{b7}  {} OF {}  \u{b7}  {}",
            i + 1,
            b.blocks.len(),
            def.kind.label().to_uppercase()
        ),
        pos2(x0, r.min.y + px(22.0)),
        10.5,
        th.caption,
        Align2::LEFT_CENTER,
    );
    let col_x = r.max.x - px(22.0) - px(130.0);
    let nr = draw::label_w(
        &p,
        &def.shown_name(),
        pos2(x0, r.min.y + px(46.0)),
        20.0,
        800,
        th.text,
        Align2::LEFT_CENTER,
        col_x - x0 - px(150.0),
    );
    draw::label_w(
        &p,
        &format!("after {}", def.after),
        pos2(nr.max.x + px(10.0), r.min.y + px(47.0)),
        12.5,
        500,
        th.caption,
        Align2::LEFT_CENTER,
        (col_x - nr.max.x - px(20.0)).max(0.0),
    );
    // its face, small
    let fs = faces::board_size(&blk);
    let box_ = vec2(px(92.0), px(118.0));
    let k = (box_.x / fs.x).min(box_.y / fs.y);
    let fr = Rect::from_min_size(pos2(x0, r.min.y + px(66.0) + (box_.y - fs.y * k) / 2.0), fs * k);
    faces::miniature(&p, fr, &blk, 0.0);
    if !blk.on {
        paint::wash(&p, fr, Color32::BLACK, 0.4);
    }
    // its front knobs, large
    let front = def.front_knobs();
    let kx0 = x0 + box_.x + px(18.0);
    let kw = col_x - px(14.0) - kx0;
    let n = front.len().max(1);
    let step = kw / n as f32;
    let kr = (step * 0.26).clamp(px(16.0), px(28.0));
    let ky = r.min.y + px(122.0);
    let shift = ctx.input(|i| i.modifiers.shift);
    for (k, kd) in front.iter().enumerate() {
        let c = pos2(kx0 + step * (k as f32 + 0.5), ky);
        let hit = Rect::from_center_size(c, Vec2::splat(kr * 2.0 + px(18.0)));
        room.spots.push(Spot {
            rect: hit,
            uid: blk.uid,
            knob: k,
        });
        let resp = interact(ui, hit, ("room-knob", blk.uid, k), Sense::click_and_drag());
        let u = kd.to_u(blk.v[k]);
        if resp.hovered() || resp.dragged() {
            ctx.set_cursor_icon(CursorIcon::ResizeVertical);
        }
        if resp.double_clicked() {
            room.drag = None;
            room.rack.set_knob(st, blk.uid, k, kd.def);
            room.rack.board_commit(st);
        } else if resp.drag_started()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            room.drag = Some(Drag {
                uid: blk.uid,
                knob: k,
                y0: pos.y,
                u0: u,
            });
        }
        if let Some(d) = room.drag
            && d.uid == blk.uid
            && d.knob == k
        {
            if resp.dragged() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let span = if shift { px(900.0) } else { px(180.0) };
                    let nu = (d.u0 + (d.y0 - pos.y) / span).clamp(0.0, 1.0);
                    let v = match kd.unit {
                        Unit::Choice(c) => (nu * (c.len() as f32 - 1.0)).round(),
                        _ => kd.from_u(nu),
                    };
                    room.rack.set_knob(st, blk.uid, k, v);
                }
            } else {
                room.drag = None;
                room.rack.board_commit(st);
            }
        }
        let hot = resp.hovered() || resp.dragged();
        big_knob(&p, c, kr, u, &kd.label.to_uppercase(), &kd.text(blk.v[k]), hot);
    }
    // the keys at the right
    let kh = px(34.0);
    let col = |j: usize| {
        Rect::from_min_size(
            pos2(col_x, r.min.y + px(18.0) + j as f32 * px(42.0)),
            vec2(px(130.0), kh),
        )
    };
    let on_r = col(0);
    let resp = interact(ui, on_r, "room-sel-on", Sense::click());
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
    if blk.on {
        qrs_ui::keys::key_face(&p, on_r, Some(px(12.0)), false, resp.is_pointer_button_down_on());
    } else {
        paint::inset(&p, on_r, Some(px(12.0)), 2.0, paint::Fill::Base);
    }
    let lc = pos2(on_r.center().x - px(18.0), on_r.center().y);
    if blk.on {
        paint::halo(&p, lc, px(7.0), Color32::from_rgb(0x3e, 0xc4, 0x6d), 90);
        p.circle_filled(lc, px(4.0), Color32::from_rgb(0x3e, 0xc4, 0x6d));
    } else {
        p.circle_filled(lc, px(4.0), th.unlit);
    }
    draw::label(
        &p,
        if blk.on { "ON" } else { "OFF" },
        pos2(lc.x + px(10.0), on_r.center().y),
        13.0,
        800,
        if blk.on {
            qrs_ui::theme::IVORY_INK
        } else {
            th.text_2
        },
        Align2::LEFT_CENTER,
    );
    if resp.clicked() {
        room.rack.toggle_block(st, blk.uid);
    }
    if draw::key(
        ui,
        col(1),
        "room-sel-editor",
        KeyLook::Dark,
        "Detailed editor",
        "E",
        None,
    )
    .clicked()
    {
        room.rack.board.editing = true;
    }
    if draw::key(
        ui,
        col(2),
        "room-sel-random",
        KeyLook::Dark,
        "Randomise",
        "",
        Some(&draw::icon_dice),
    )
    .clicked()
    {
        let mut nb = blk;
        super::rigs::randomise(&mut nb, &mut room.dice);
        room.rack.apply_values(st, blk.uid, nb.v);
        room.say(
            format!("{} randomised", def.name),
            "  \u{b7}  Ctrl Z takes it back",
        );
    }
    if draw::key(
        ui,
        col(3),
        "room-sel-defaults",
        KeyLook::Dark,
        "Defaults",
        "",
        None,
    )
    .clicked()
    {
        room.rack.block_defaults(st, blk);
    }
    draw::label_w(
        &p,
        "drag \u{b7} Shift fine \u{b7} wheel steps \u{b7} double-click resets \u{b7} Tab next unit",
        pos2(kx0, r.max.y - px(18.0)),
        10.5,
        600,
        th.caption,
        Align2::LEFT_CENTER,
        col_x - kx0,
    );
}

/// The gear shelf: every model by family (or found by name); click one to add it after the selected unit (or where
/// a + on the stage said).
fn shelf_card(room: &mut RoomView, ui: &Ui, st: &mut AppState, r: Rect) {
    let p = ui.painter().clone();
    let th = t();
    let ctx = ui.ctx().clone();
    draw::card(&p, r, px(20.0));
    let x0 = r.min.x + px(22.0);
    let hr = draw::caps(
        &p,
        &format!("GEAR SHELF  \u{b7}  {} MODELS", MODELS.len()),
        pos2(x0, r.min.y + px(22.0)),
        10.5,
        th.caption,
        Align2::LEFT_CENTER,
    );
    // the search box
    let sr = Rect::from_min_size(
        pos2(r.max.x - px(20.0) - px(150.0), r.min.y + px(10.0)),
        vec2(px(150.0), px(26.0)),
    );
    let typing = room.typing == Some(Typing::Shelf);
    let resp = interact(ui, sr, "room-shelf-find", Sense::click());
    paint::inset(&p, sr, Some(px(9.0)), 1.5, paint::Fill::Base);
    if typing {
        p.rect_stroke(sr, px(9.0), Stroke::new(1.5, th.accent), egui::StrokeKind::Inside);
    }
    draw::icon_search(&p, pos2(sr.min.x + px(14.0), sr.center().y), px(13.0), th.caption);
    let (qtext, qcol) = if room.shelf_query.is_empty() && !typing {
        ("Find a model".to_string(), th.caption)
    } else {
        (room.shelf_query.clone(), th.text)
    };
    let qr = draw::label_w(
        &p,
        &qtext,
        pos2(sr.min.x + px(27.0), sr.center().y),
        11.5,
        700,
        qcol,
        Align2::LEFT_CENTER,
        sr.width() - px(34.0),
    );
    if typing {
        let x = if room.shelf_query.is_empty() {
            sr.min.x + px(27.0)
        } else {
            qr.max.x
        };
        draw::caret(&p, x, sr.center().y, px(14.0), th.accent);
    }
    if resp.hovered() {
        ctx.set_cursor_icon(CursorIcon::Text);
    }
    if resp.clicked() {
        room.typing = Some(Typing::Shelf);
    }
    let hint = match room.shelf_at {
        Some(at) => format!("adds at {} \u{b7} Esc cancels", at + 1),
        None => "click to add it after the selected unit".into(),
    };
    draw::label_w(
        &p,
        &hint,
        pos2(hr.max.x + px(14.0), r.min.y + px(22.0)),
        11.0,
        600,
        if room.shelf_at.is_some() {
            th.accent_ink
        } else {
            th.caption
        },
        Align2::LEFT_CENTER,
        (sr.min.x - hr.max.x - px(24.0)).max(0.0),
    );
    // the families
    let q = room.shelf_query.trim().to_lowercase();
    let mut x = x0 - px(2.0);
    let fy = r.min.y + px(44.0);
    let size = 11.0;
    let total: f32 = Kind::ALL
        .iter()
        .map(|k| draw::chip_w(&ctx, k.label(), true, size) + px(5.0))
        .sum();
    let squeeze = ((r.max.x - px(20.0) - x) / total).min(1.0);
    for kind in Kind::ALL {
        let w = draw::chip_w(&ctx, kind.label(), true, size) * squeeze;
        let cr = Rect::from_min_size(pos2(x, fy), vec2(w, px(26.0)));
        x = cr.max.x + px(5.0) * squeeze;
        let on = q.is_empty() && room.shelf_kind == kind;
        if draw::chip(
            ui,
            cr,
            ("room-shelf-kind", kind.label()),
            kind.label(),
            Some(faces::kind_colour(kind)),
            on,
            size,
        )
        .clicked()
        {
            room.shelf_kind = kind;
            room.shelf_query.clear();
            room.typing = None;
        }
    }
    // the models
    let models: Vec<usize> = if q.is_empty() {
        super::rigs::shelf(room.shelf_kind)
    } else {
        (0..MODELS.len())
            .filter(|&i| {
                let m = &MODELS[i];
                m.name.to_lowercase().contains(&q)
                    || m.after.to_lowercase().contains(&q)
                    || m.kind.label().to_lowercase().contains(&q)
            })
            .collect()
    };
    let b = room.rack.board_now(st);
    let cell_w = px(76.0);
    let gap = px(6.0);
    let iy = r.min.y + px(80.0);
    let fits = ((r.max.x - x0 + gap) / (cell_w + gap)).floor().max(1.0) as usize;
    if models.is_empty() {
        draw::label(
            &p,
            "No model by that name",
            pos2(x0, iy + px(40.0)),
            12.0,
            600,
            th.caption,
            Align2::LEFT_CENTER,
        );
    }
    let mut add: Option<usize> = None;
    for (j, &m) in models.iter().take(fits).enumerate() {
        let cell = Rect::from_min_size(
            pos2(x0 - px(4.0) + j as f32 * (cell_w + gap), iy),
            vec2(cell_w, px(116.0)),
        );
        let resp = interact(ui, cell, ("room-shelf-item", m), Sense::click());
        let in_rig = b.blocks.iter().any(|x| x.model == m);
        if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
            p.rect_filled(cell, px(12.0), th.base_hi.gamma_multiply(0.6));
            p.rect_stroke(
                cell,
                px(12.0),
                Stroke::new(1.5, th.accent.gamma_multiply(0.35)),
                egui::StrokeKind::Inside,
            );
            room.rack.tip = Some((
                format!("{} \u{b7} after {}", MODELS[m].shown_name(), MODELS[m].after),
                "click: add it".into(),
                pos2(cell.center().x, cell.min.y - px(6.0)),
                Align2::CENTER_BOTTOM,
            ));
        }
        let blk = Block::new(m);
        let fs = faces::board_size(&blk);
        let k = (px(68.0) / fs.x).min(px(84.0) / fs.y);
        let sz = fs * k;
        let fr = Rect::from_min_size(
            pos2(
                cell.center().x - sz.x / 2.0,
                cell.min.y + px(4.0) + px(84.0) - sz.y,
            ),
            sz,
        );
        p.rect_filled(fr.translate(vec2(0.0, px(4.0))), px(6.0), rgba(0, 0, 0, 0.35));
        faces::miniature(&p, fr, &blk, 0.0);
        draw::label_w(
            &p,
            MODELS[m].name,
            pos2(cell.center().x, cell.min.y + px(102.0)),
            10.5,
            700,
            if in_rig || resp.hovered() {
                th.text
            } else {
                th.text_2
            },
            Align2::CENTER_CENTER,
            cell_w,
        );
        if in_rig {
            let tr = Rect::from_center_size(
                pos2(cell.center().x, cell.min.y + px(2.0)),
                vec2(px(42.0), px(14.0)),
            );
            p.rect_filled(tr, px(5.0), th.accent);
            text::caps(
                &p,
                "IN RIG",
                tr.center(),
                8.5,
                th.on_accent,
                Align2::CENTER_CENTER,
                None,
            );
        }
        if resp.clicked() {
            add = Some(m);
        }
    }
    if models.len() > fits {
        draw::label(
            &p,
            &format!("+{} more: type to find", models.len() - fits),
            pos2(r.max.x - px(22.0), r.max.y - px(14.0)),
            10.5,
            600,
            th.caption,
            Align2::RIGHT_CENTER,
        );
    }
    if let Some(m) = add {
        let at = room.shelf_at.take().unwrap_or_else(|| {
            room.rack
                .board
                .sel
                .and_then(|u| b.index_of(u))
                .map(|i| i + 1)
                .unwrap_or(b.blocks.len())
        });
        room.rack.add_block(st, at, m);
        room.typing = None;
    }
}

/// The wheel over a knob of the selected card.
pub fn wheel(room: &mut RoomView, st: &mut AppState, pos: Pos2, notches: f32, shift: bool) -> bool {
    let Some(s) = room.spots.iter().find(|s| s.rect.contains(pos)).copied() else {
        return false;
    };
    let b = room.rack.board_now(st);
    let Some(i) = b.index_of(s.uid) else { return false };
    let blk = b.blocks[i];
    let Some(kd) = blk.def().knobs.get(s.knob) else {
        return false;
    };
    let v = match kd.unit {
        Unit::Choice(c) => (blk.v[s.knob] + notches.signum()).rem_euclid(c.len() as f32),
        _ => kd.from_u(kd.to_u(blk.v[s.knob]) + notches * if shift { 0.005 } else { 0.025 }),
    };
    room.rack.set_knob(st, s.uid, s.knob, v);
    room.wheel_at = Some(std::time::Instant::now());
    true
}
