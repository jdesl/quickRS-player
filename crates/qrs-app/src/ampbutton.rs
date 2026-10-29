//! The AMP ROOM button at the foot of the sidebar, on Home and every library page (PLAN.md P22.1), the owner's pick of the mockups ("VU"): a
//! cream VU meter whose needle follows the guitar's input with a VU's ballistics, a red peak lamp, the room's rig
//! and the note being played; click it or press A for the amp room. New code (no Python source).

use std::f32::consts::PI;

use eframe::egui;
use egui::{Align2, Color32, CursorIcon, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use qrs_ui::keys::{CapOn, keycap};
use qrs_ui::paint::{self, Fill};
use qrs_ui::text::{self, Style};
use qrs_ui::theme::t;
use qrs_ui::units::px;
use qrs_ui::widgets::interact;

use crate::listen::Listen;
use crate::state::AppState;

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// A rounded rect's outline (corner arcs of `seg` steps each), clockwise from the top left.
pub fn rounded_points(r: Rect, rad: f32, seg: usize) -> Vec<Pos2> {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let corners = [
        (pos2(r.min.x + rad, r.min.y + rad), PI),
        (pos2(r.max.x - rad, r.min.y + rad), 1.5 * PI),
        (pos2(r.max.x - rad, r.max.y - rad), 0.0),
        (pos2(r.min.x + rad, r.max.y - rad), 0.5 * PI),
    ];
    let mut out = Vec::with_capacity(4 * (seg + 1));
    for (c, a0) in corners {
        for i in 0..=seg {
            let a = a0 + 0.5 * PI * i as f32 / seg as f32;
            out.push(pos2(c.x + rad * a.cos(), c.y + rad * a.sin()));
        }
    }
    out
}

/// A rounded rect shaded from `top` to `bottom`.
pub fn rounded_vgrad(p: &Painter, r: Rect, rad: f32, top: Color32, bottom: Color32) {
    let pts = rounded_points(r, rad, 6);
    let mut m = egui::Mesh::default();
    let lerp = |y: f32| {
        let k = ((y - r.min.y) / r.height().max(1.0)).clamp(0.0, 1.0);
        top.lerp_to_gamma(bottom, k)
    };
    m.colored_vertex(r.center(), lerp(r.center().y));
    for q in &pts {
        m.colored_vertex(*q, lerp(q.y));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        m.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(m);
}

/// The VU meter's face in `well`: cream, its scale and ticks (the last two red), "VU", the needle at `needle`
/// (0..1) and the peak lamp. Shared by the button and the room's strip.
pub fn vu_face(p: &Painter, well: Rect, needle: f32, lamp: bool) {
    let k = well.height() / 48.0;
    let rad = 13.0 * k;
    rounded_vgrad(
        p,
        well,
        rad,
        Color32::from_rgb(0xf5, 0xec, 0xd4),
        Color32::from_rgb(0xd8, 0xc9, 0xa2),
    );
    // the glass's inner shadow
    p.rect_stroke(
        well.shrink(0.5),
        rad,
        Stroke::new(1.0, rgba(0, 0, 0, 0.25)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        Rect::from_min_max(well.min, pos2(well.max.x, well.min.y + 3.0 * k)).shrink2(vec2(rad * 0.6, 0.0)),
        0.0,
        Stroke::new(1.5 * k, rgba(60, 40, 0, 0.18)),
        egui::StrokeKind::Inside,
    );
    let o = well.min;
    let at = |x: f32, y: f32| pos2(o.x + x * well.width() / 70.0, o.y + y * k);
    let ink = Color32::from_rgb(0x5b, 0x4e, 0x36);
    // the scale's arc: a quadratic from (8,22) through (35,-2) to (62,22)
    let arc: Vec<Pos2> = (0..=20)
        .map(|i| {
            let s = i as f32 / 20.0;
            let x = (1.0 - s).powi(2) * 8.0 + 2.0 * (1.0 - s) * s * 35.0 + s * s * 62.0;
            let y = (1.0 - s).powi(2) * 22.0 + 2.0 * (1.0 - s) * s * -2.0 + s * s * 22.0;
            at(x, y)
        })
        .collect();
    p.add(egui::Shape::line(arc, Stroke::new(0.8 * k, ink)));
    for i in 0..9 {
        let a = (-50.0 + i as f32 * 12.5).to_radians();
        let (x1, y1) = (35.0 + 32.0 * a.sin(), 44.0 - 32.0 * a.cos());
        let (x2, y2) = (35.0 + 37.0 * a.sin(), 44.0 - 37.0 * a.cos());
        let col = if i >= 7 {
            Color32::from_rgb(0xc0, 0x39, 0x2b)
        } else {
            Color32::from_rgb(0x3a, 0x32, 0x25)
        };
        p.line_segment([at(x1, y1), at(x2, y2)], Stroke::new(1.2 * k, col));
    }
    text::draw(
        p,
        "VU",
        at(35.0, 37.5),
        Align2::CENTER_CENTER,
        Style::new(7.0 * k, 800, ink),
        None,
    );
    // the needle, pivoting under the face
    let a = (-50.0 + 100.0 * needle.clamp(0.0, 1.0)).to_radians();
    let pivot = at(34.0, 47.0);
    let len = 40.0 * k;
    let tip = pos2(pivot.x + len * a.sin(), pivot.y - len * a.cos());
    p.line_segment(
        [pivot, tip],
        Stroke::new(1.8 * k, Color32::from_rgb(0x1e, 0x1b, 0x16)),
    );
    p.circle_filled(pivot, 3.0 * k, Color32::from_rgb(0x2a, 0x25, 0x1c));
    // the peak lamp
    let lc = pos2(well.max.x - 8.0 * k, well.min.y + 8.0 * k);
    if lamp {
        paint::halo(p, lc, 9.0 * k, Color32::from_rgb(0xff, 0x5a, 0x4c), 110);
        p.circle_filled(lc, 3.2 * k, Color32::from_rgb(0xff, 0x5a, 0x4c));
        p.circle_filled(lc + vec2(-0.8, -0.8) * k, 1.1 * k, rgba(255, 255, 255, 0.7));
    } else {
        p.circle_filled(lc, 3.0 * k, Color32::from_rgb(0x7a, 0x3a, 0x33));
    }
}

/// What the button shows under its label.
pub struct Look<'a> {
    /// The rig ("Reptilia lead, warmer", or its chain).
    pub rig: &'a str,
    pub hover: bool,
    pub pressed: bool,
    /// Show its key (A works on Home only: elsewhere letters go to the search).
    pub key: bool,
}

/// The note badge: "E2 +3¢" in the dark well (returns its width).
fn note_badge(p: &Painter, right: Pos2, h: f32, note: &(String, i32, i32)) -> f32 {
    let ctx = p.ctx();
    let amber = Color32::from_rgb(0xf2, 0xb5, 0x3c);
    let n = Style::new(15.0, 800, amber);
    let o = Style::new(10.0, 800, Color32::from_rgb(0xd7, 0x9a, 0x2a));
    let c = Style::new(10.0, 800, Color32::from_rgb(0x3e, 0xc4, 0x6d));
    let oct = note.1.to_string();
    let cents = format!("{:+}\u{a2}", note.2);
    let w = text::width(ctx, &note.0, n) + text::width(ctx, &oct, o) + text::width(ctx, &cents, c) + px(22.0);
    let r = Rect::from_min_size(pos2(right.x - w, right.y - h / 2.0), vec2(w, h));
    p.rect_filled(r, px(9.0), rgba(0, 0, 0, 0.5));
    p.rect_stroke(
        r,
        px(9.0),
        Stroke::new(1.0, rgba(255, 255, 255, 0.09)),
        egui::StrokeKind::Inside,
    );
    text::draw_runs(
        p,
        &[
            (note.0.as_str(), n),
            (oct.as_str(), o),
            (" ", c),
            (cents.as_str(), c),
        ],
        pos2(r.min.x + px(9.0), r.center().y),
        Align2::LEFT_CENTER,
    );
    w
}

/// The button in `r` (its body; it rises a little when hovered). Returns its response.
pub fn button(ui: &Ui, r: Rect, listen: &Listen, look: &Look) -> Response {
    let resp = interact(ui, r, "amp-room-button", Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let hover = look.hover || resp.hovered();
    let pressed = look.pressed || resp.is_pointer_button_down_on();
    draw(ui.painter(), r, listen, look.rig, hover, pressed, look.key);
    resp
}

pub fn draw(p: &Painter, r: Rect, listen: &Listen, rig: &str, hover: bool, pressed: bool, key: bool) {
    let th = t();
    let hot = listen.hot();
    let playing = !hot && listen.level_db > -50.0;
    let lift = if pressed {
        0.0
    } else if hover {
        px(2.0)
    } else {
        0.0
    };
    let body = r.translate(vec2(0.0, -lift));
    let rad = body.height() * 0.3;
    // the glow under it: green while it hears you, red while it's too hot
    if hot {
        paint::glow(
            p,
            body,
            Color32::from_rgb(0xf0, 0x64, 0x5a),
            px(16.0),
            Some(0.45),
            Some(rad),
        );
    } else if playing || hover {
        paint::glow(
            p,
            body,
            Color32::from_rgb(0x3e, 0xc4, 0x6d),
            px(14.0),
            Some(0.22),
            Some(rad),
        );
    }
    paint::raised(
        p,
        body,
        Some(rad),
        if pressed { 2.0 } else { 6.0 },
        Fill::Base,
        0.0,
    );
    if hover {
        p.rect_stroke(
            body.expand(0.5),
            rad,
            Stroke::new(1.5, Color32::from_rgb(0xfb, 0xf5, 0xe8)),
            egui::StrokeKind::Outside,
        );
    } else if hot {
        p.rect_stroke(
            body.expand(0.5),
            rad,
            Stroke::new(1.0, Color32::from_rgb(0x5a, 0x24, 0x20)),
            egui::StrokeKind::Outside,
        );
    }
    // the VU's window at the left
    let pad = px(6.0);
    let wh = body.height() - 2.0 * pad;
    let well = Rect::from_min_size(
        pos2(body.min.x + pad, body.min.y + pad),
        vec2(wh * 70.0 / 48.0, wh),
    );
    vu_face(p, well, listen.needle, listen.lamp());
    // the key at the right, the note before it
    let cy = body.center().y;
    let mut right = if key {
        let kr = keycap(
            p,
            "A",
            pos2(body.max.x - px(12.0), cy),
            Align2::RIGHT_CENTER,
            11.0,
            CapOn::Material,
        );
        kr.min.x - px(8.0)
    } else {
        body.max.x - px(12.0)
    };
    let ctx = p.ctx();
    if hot {
        let clip = Style::caps(11.0, Color32::from_rgb(0xff, 0x8a, 0x7f));
        let w = text::width(ctx, "CLIP", clip) + px(16.0);
        let cr = Rect::from_min_size(pos2(right - w, cy - px(14.0)), vec2(w, px(28.0)));
        p.rect_filled(cr, px(9.0), rgba(90, 20, 18, 0.6));
        p.rect_stroke(
            cr,
            px(9.0),
            Stroke::new(1.0, rgba(240, 100, 90, 0.5)),
            egui::StrokeKind::Inside,
        );
        text::draw(p, "CLIP", cr.center(), Align2::CENTER_CENTER, clip, None);
        right = cr.min.x - px(8.0);
    } else if let Some(n) = listen.note()
        && body.width() > px(250.0)
    {
        let w = note_badge(p, pos2(right, cy), px(30.0), &n);
        right -= w + px(8.0);
    }
    // the label and the rig (or what's wrong)
    let tx = well.max.x + px(12.0);
    let room = (right - tx).max(px(20.0));
    let daylight = !th.dark;
    let label_c = if daylight {
        th.text
    } else {
        Color32::from_rgb(0xee, 0xf0, 0xf3)
    };
    text::draw(
        p,
        "AMP ROOM",
        pos2(tx, cy - px(1.0)),
        Align2::LEFT_BOTTOM,
        Style::caps(14.0, label_c).tracked(0.16),
        Some(room),
    );
    let (sub, sub_c) = if hot {
        (
            format!("Too hot \u{b7} input {:.0} dB", listen.peak_db.min(0.0)),
            Color32::from_rgb(0xff, 0x8a, 0x7f),
        )
    } else {
        (
            rig.to_string(),
            if daylight {
                Color32::from_rgb(0x6d, 0x69, 0x63)
            } else {
                Color32::from_rgb(0xa7, 0xad, 0xb6)
            },
        )
    };
    text::draw(
        p,
        &sub,
        pos2(tx, cy + px(2.0)),
        Align2::LEFT_TOP,
        Style::new(11.0, 700, sub_c),
        Some(room),
    );
}

/// The button's state: the guitar's input, and the room's rig as it names it (read again every 2 s).
#[derive(Default)]
pub struct Amp {
    pub listen: Listen,
    rig_text: Option<(String, std::time::Instant)>,
    /// Screenshots: the button shown hovered.
    pub shot_hover: bool,
}

impl Amp {
    /// The button in `body`: the input on its VU, the rig and the note; it repaints only while there's sound or
    /// the needle is still moving (and looks for sound four times a second otherwise). True when clicked.
    pub fn ui(&mut self, ui: &Ui, st: &AppState, body: Rect, key: bool) -> bool {
        let engine = st.audio.as_ref().map(|a| a.engine.clone());
        let live = self.listen.update(engine.as_deref());
        if live {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        } else if engine.as_ref().is_some_and(|e| e.status().input.is_some()) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(250));
        }
        if self
            .rig_text
            .as_ref()
            .is_none_or(|(_, at)| at.elapsed().as_secs_f32() > 2.0)
        {
            self.rig_text = Some((crate::room::rig_label(&st.config), std::time::Instant::now()));
        }
        let rig = self.rig_text.as_ref().map(|r| r.0.clone()).unwrap_or_default();
        let look = Look {
            rig: &rig,
            hover: self.shot_hover,
            pressed: false,
            key,
        };
        let resp = button(ui, body, &self.listen, &look);
        if resp.hovered() || self.shot_hover {
            let l = &self.listen;
            let how = if engine.is_none() {
                "audio is off".to_string()
            } else if l.hot() {
                format!("input {:.0} dB, too hot", l.peak_db)
            } else if l.level_db < -60.0 {
                "nothing heard yet".into()
            } else {
                let word = if l.level_db < -40.0 {
                    "quiet"
                } else {
                    "a good level"
                };
                match l.note() {
                    Some((n, o, c)) => {
                        format!("input {:.0} dB, {word} \u{b7} {n}{o} {c:+}\u{a2}", l.level_db)
                    }
                    None => format!("input {:.0} dB, {word}", l.level_db),
                }
            };
            // (over the page drawn after the strip)
            let top = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("amp-room-tip"),
            ));
            crate::player::draw_tip(
                &top,
                &format!("Open the amp room \u{b7} {how}"),
                if key { "A" } else { "" },
                pos2(body.center().x, body.min.y - px(10.0)),
                Align2::CENTER_BOTTOM,
                st.ctx.content_rect(),
            );
        }
        resp.clicked()
    }
}
