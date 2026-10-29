//! The amp room's small parts: dark keys, chips, tiny icons, a card. Theme colours throughout (the mockups'
//! graphite numbers are the theme's own there).

use eframe::egui;
use egui::{Align2, Color32, CursorIcon, Painter, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use qrs_ui::keys::{self, CapOn, keycap, keycap_width};
use qrs_ui::paint::{self, Fill};
use qrs_ui::text::{self, Style};
use qrs_ui::theme::{IVORY_INK, t};
use qrs_ui::units::px;
use qrs_ui::widgets::interact;

/// A card: the room's panels (the selected unit, the gear shelf, the strip).
pub fn card(p: &Painter, r: Rect, rad: f32) {
    paint::raised(p, r, Some(rad), 4.0, Fill::Base, 0.0);
}

/// Small caps.
pub fn caps(p: &Painter, s: &str, pos: Pos2, size: f32, color: Color32, anchor: Align2) -> Rect {
    text::caps(p, s, pos, size, color, anchor, None)
}

pub fn label(
    p: &Painter,
    s: &str,
    pos: Pos2,
    size: f32,
    weight: u16,
    color: Color32,
    anchor: Align2,
) -> Rect {
    text::draw(p, s, pos, anchor, Style::new(size, weight, color), None)
}

pub fn label_w(
    p: &Painter,
    s: &str,
    pos: Pos2,
    size: f32,
    weight: u16,
    color: Color32,
    anchor: Align2,
    max_w: f32,
) -> Rect {
    text::draw(p, s, pos, anchor, Style::new(size, weight, color), Some(max_w))
}

/// What a key looks like.
#[derive(Clone, Copy, PartialEq)]
pub enum KeyLook {
    /// Ivory (Home, Save as, Load).
    Ivory,
    /// Amber (Save).
    Amber,
    /// Sunk into the material (undo, redo, A/B, Shuffle).
    Dark,
    /// Dark, lit: on (A/B while you hear the saved sound).
    Lit,
    /// Dark, ringed in red and glowing: recording (P23.9).
    Rec,
}

/// A key with an optional icon drawn by `icon` at its left, a label and a key cap; returns its response.
pub fn key(
    ui: &Ui,
    r: Rect,
    id: impl egui::AsId,
    look: KeyLook,
    label: &str,
    cap: &str,
    icon: Option<&dyn Fn(&Painter, Pos2, f32, Color32)>,
) -> Response {
    let resp = interact(ui, r, id, Sense::click());
    let p = ui.painter();
    let th = t();
    let hot = resp.hovered();
    let down = resp.is_pointer_button_down_on();
    if hot {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let rad = px(12.0);
    let ink = match look {
        KeyLook::Ivory => {
            keys::key_face(p, r, Some(rad), false, down);
            IVORY_INK
        }
        KeyLook::Amber => {
            keys::key_face(p, r, Some(rad), true, down);
            th.on_accent
        }
        KeyLook::Rec => {
            paint::glow(p, r, th.miss, 10.0, Some(0.35), Some(rad));
            paint::inset(p, r, Some(rad), 2.0, Fill::Base);
            p.rect_stroke(
                r.shrink(1.0),
                rad,
                Stroke::new(if hot { 2.0 } else { 1.5 }, th.miss),
                egui::StrokeKind::Inside,
            );
            th.text
        }
        KeyLook::Dark | KeyLook::Lit => {
            paint::inset(p, r, Some(rad), 2.0, Fill::Base);
            if look == KeyLook::Lit {
                p.rect_stroke(
                    r.shrink(1.0),
                    rad,
                    Stroke::new(1.5, th.accent),
                    egui::StrokeKind::Inside,
                );
            } else if hot {
                p.rect_stroke(
                    r.shrink(1.0),
                    rad,
                    Stroke::new(1.0, th.accent.gamma_multiply(0.5)),
                    egui::StrokeKind::Inside,
                );
            }
            if look == KeyLook::Lit {
                th.accent_ink
            } else {
                th.text
            }
        }
    };
    let ctx = ui.ctx();
    let st = Style::new(13.0, 800, ink);
    let tw = if label.is_empty() {
        0.0
    } else {
        text::width(ctx, label, st)
    };
    let iw = if icon.is_some() {
        px(16.0) + if label.is_empty() { 0.0 } else { px(7.0) }
    } else {
        0.0
    };
    let kw = if cap.is_empty() {
        0.0
    } else {
        keycap_width(ctx, cap, 10.0) + px(7.0)
    };
    let mut x = r.center().x - (tw + iw + kw) / 2.0;
    let cy = r.center().y;
    if let Some(f) = icon {
        f(p, pos2(x + px(8.0), cy), px(15.0), ink);
        x += iw;
    }
    if !label.is_empty() {
        text::draw(p, label, pos2(x, cy), Align2::LEFT_CENTER, st, None);
        x += tw + px(7.0);
    }
    if !cap.is_empty() {
        keycap(
            p,
            cap,
            pos2(x, cy),
            Align2::LEFT_CENTER,
            10.0,
            match look {
                KeyLook::Ivory => CapOn::Ivory,
                KeyLook::Amber => CapOn::Accent,
                _ => CapOn::Material,
            },
        );
    }
    resp
}

/// A key's width for its label, cap and icon.
pub fn key_w(ctx: &egui::Context, label: &str, cap: &str, icon: bool) -> f32 {
    let mut w = px(28.0);
    if !label.is_empty() {
        w += text::width(ctx, label, Style::new(13.0, 800, Color32::WHITE));
    }
    if icon {
        w += px(16.0) + if label.is_empty() { 0.0 } else { px(7.0) };
    }
    if !cap.is_empty() {
        w += keycap_width(ctx, cap, 10.0) + px(7.0);
    }
    w
}

/// A chip: a small rounded key with a coloured dot (a family, a tag); `on` shows it chosen.
pub fn chip(
    ui: &Ui,
    r: Rect,
    id: impl egui::AsId,
    label: &str,
    dot: Option<Color32>,
    on: bool,
    size: f32,
) -> Response {
    let resp = interact(ui, r, id, Sense::click());
    let p = ui.painter();
    let th = t();
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let rad = r.height() * 0.36;
    if on {
        keys::key_face(p, r, Some(rad), false, false);
    } else {
        paint::inset(p, r, Some(rad), 1.5, Fill::Base);
        if resp.hovered() {
            p.rect_stroke(
                r.shrink(0.5),
                rad,
                Stroke::new(1.0, th.accent.gamma_multiply(0.5)),
                egui::StrokeKind::Inside,
            );
        }
    }
    let ink = if on { IVORY_INK } else { th.text_2 };
    let mut x = r.min.x + px(9.0);
    if let Some(c) = dot {
        p.circle_filled(pos2(x + px(3.5), r.center().y), px(3.5), c);
        x += px(12.0);
    }
    text::draw(
        p,
        label,
        pos2(x, r.center().y),
        Align2::LEFT_CENTER,
        Style::new(size, 800, ink),
        Some(r.max.x - x - px(4.0)),
    );
    resp
}

pub fn chip_w(ctx: &egui::Context, label: &str, dot: bool, size: f32) -> f32 {
    text::width(ctx, label, Style::new(size, 800, Color32::WHITE))
        + px(18.0)
        + if dot { px(12.0) } else { 0.0 }
}

/// A rig as tiny gear icons in a row, `h` high (the mockups' chain summaries: a pedal in its colour, a head, a cab,
/// a rack unit; off ones faint). Returns where it ends.
pub fn chain_icons(p: &Painter, b: &qrs_audio::board::Board, x: f32, cy: f32, h: f32, max: usize) -> f32 {
    use qrs_audio::board::Kind;
    let k = h / 26.0;
    let mut x = x;
    let dark = Color32::from_rgb(0x14, 0x14, 0x14);
    for (i, blk) in b.blocks.iter().take(max).enumerate() {
        let d = blk.def();
        let a = if blk.on { 1.0 } else { 0.38 };
        let col = Color32::from_rgb(d.colour[0], d.colour[1], d.colour[2]).gamma_multiply(a);
        let w = match d.kind {
            Kind::Amp => {
                let r = Rect::from_min_size(pos2(x, cy - 7.0 * k), vec2(28.0 * k, 14.0 * k));
                p.rect_filled(r, 2.0 * k, dark.gamma_multiply(a));
                p.rect_filled(
                    Rect::from_min_size(r.min + vec2(2.0, 1.5) * k, vec2(24.0 * k, 4.5 * k)),
                    k,
                    col,
                );
                p.rect_filled(
                    Rect::from_min_size(r.min + vec2(2.0, 7.5) * k, vec2(24.0 * k, 4.5 * k)),
                    k,
                    Color32::from_rgb(0x3a, 0x35, 0x2b).gamma_multiply(a),
                );
                28.0
            }
            Kind::Cab => {
                let r = Rect::from_min_size(pos2(x, cy - 11.0 * k), vec2(22.0 * k, 22.0 * k));
                p.rect_filled(r, 2.5 * k, dark.gamma_multiply(a));
                p.rect_filled(r.shrink(2.5 * k), k, col);
                for (dx, dy) in [(7.0, -4.0), (15.0, -4.0), (7.0, 4.0), (15.0, 4.0)] {
                    p.circle_filled(
                        pos2(x + dx * k, cy + dy * k),
                        3.0 * k,
                        Color32::from_black_alpha((76.0 * a) as u8),
                    );
                }
                22.0
            }
            Kind::Eq => {
                let r = Rect::from_min_size(pos2(x, cy - 5.0 * k), vec2(28.0 * k, 10.0 * k));
                p.rect_filled(r, 1.5 * k, col);
                for dx in [7.0, 13.0, 19.0] {
                    p.circle_filled(pos2(x + dx * k, cy), 2.0 * k, Color32::from_rgb(0x11, 0x11, 0x11));
                }
                28.0
            }
            _ => {
                let r = Rect::from_min_size(pos2(x, cy - 10.0 * k), vec2(13.0 * k, 20.0 * k));
                p.rect_filled(r, 2.5 * k, col);
                p.rect_filled(
                    Rect::from_min_size(r.min, vec2(r.width(), r.height() * 0.45)),
                    2.5 * k,
                    Color32::from_white_alpha(18),
                );
                p.circle_filled(
                    pos2(x + 4.0 * k, cy - 5.0 * k),
                    1.5 * k,
                    Color32::from_rgb(0x11, 0x11, 0x11),
                );
                p.circle_filled(
                    pos2(x + 9.0 * k, cy - 5.0 * k),
                    1.5 * k,
                    Color32::from_rgb(0x11, 0x11, 0x11),
                );
                p.circle_filled(
                    pos2(x + 6.5 * k, cy + 5.0 * k),
                    2.2 * k,
                    Color32::from_rgb(0xc9, 0xcd, 0xd3).gamma_multiply(a),
                );
                13.0
            }
        };
        x += w * k;
        if i + 1 < b.blocks.len().min(max) {
            p.line_segment(
                [pos2(x + 1.5 * k, cy), pos2(x + 7.5 * k, cy)],
                Stroke::new(1.2 * k.max(0.8), t().caption.gamma_multiply(0.7)),
            );
            x += 9.0 * k;
        } else {
            x += 5.0 * k;
        }
    }
    if b.blocks.len() > max {
        text::draw(
            p,
            &format!("+{}", b.blocks.len() - max),
            pos2(x + 2.0, cy),
            Align2::LEFT_CENTER,
            Style::new(10.0, 800, t().caption),
            None,
        );
    }
    x
}

/// A soft round glow (a radial gradient to nothing): a lamp's light on the wall.
pub fn radial_glow(p: &Painter, c: Pos2, radius: egui::Vec2, color: Color32, alpha: f32) {
    let mut m = egui::Mesh::default();
    let inner = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0) as u8);
    let mid = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 0.35 * 255.0) as u8);
    m.colored_vertex(c, inner);
    let n = 48;
    for ring in [0.55f32, 1.0] {
        for i in 0..n {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            m.colored_vertex(
                pos2(c.x + ring * radius.x * a.cos(), c.y + ring * radius.y * a.sin()),
                if ring < 1.0 { mid } else { Color32::TRANSPARENT },
            );
        }
    }
    let n = n as u32;
    for i in 0..n {
        let j = (i + 1) % n;
        m.add_triangle(0, 1 + i, 1 + j);
        m.add_triangle(1 + i, 1 + n + i, 1 + j);
        m.add_triangle(1 + j, 1 + n + i, 1 + n + j);
    }
    p.add(m);
}

// -- tiny icons, drawn (centre, size, colour) --------------------------------------------------------------------

fn arc(p: &Painter, c: Pos2, r: f32, a0: f32, a1: f32, w: f32, col: Color32) -> Vec<Pos2> {
    let pts: Vec<Pos2> = (0..=16)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / 16.0;
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    p.add(egui::Shape::line(pts.clone(), Stroke::new(w, col)));
    pts
}

fn head(p: &Painter, tip: Pos2, dir: egui::Vec2, s: f32, w: f32, col: Color32) {
    let d = dir.normalized();
    let n = vec2(-d.y, d.x);
    p.line_segment([tip, tip - d * s + n * s * 0.8], Stroke::new(w, col));
    p.line_segment([tip, tip - d * s - n * s * 0.8], Stroke::new(w, col));
}

/// ↶ undo.
pub fn icon_undo(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let w = s * 0.13;
    let pts = arc(p, c + vec2(0.0, s * 0.08), s * 0.36, -2.6, 0.9, w, col);
    let tip = pts[0];
    head(p, tip, tip - pts[2], s * 0.22, w, col);
}

/// ↷ redo.
pub fn icon_redo(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let w = s * 0.13;
    let pts = arc(p, c + vec2(0.0, s * 0.08), s * 0.36, -0.54, 2.24, w, col);
    let tip = pts[0];
    head(p, tip, tip - pts[2], s * 0.22, w, col);
}

/// Two crossing arrows (Shuffle).
pub fn icon_shuffle(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let w = s * 0.12;
    let h = s * 0.32;
    let (l, r) = (c.x - s * 0.45, c.x + s * 0.45);
    p.line_segment(
        [pos2(l, c.y - h), pos2(r - s * 0.05, c.y + h)],
        Stroke::new(w, col),
    );
    p.line_segment(
        [pos2(l, c.y + h), pos2(r - s * 0.05, c.y - h)],
        Stroke::new(w, col),
    );
    head(p, pos2(r, c.y + h), vec2(1.0, 0.6), s * 0.22, w, col);
    head(p, pos2(r, c.y - h), vec2(1.0, -0.6), s * 0.22, w, col);
}

/// A floppy disk (Save).
pub fn icon_save(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let w = s * 0.11;
    let r = Rect::from_center_size(c, vec2(s * 0.8, s * 0.8));
    p.rect_stroke(r, s * 0.1, Stroke::new(w, col), egui::StrokeKind::Middle);
    p.rect_stroke(
        Rect::from_min_max(
            pos2(r.min.x + s * 0.18, r.min.y),
            pos2(r.max.x - s * 0.22, r.min.y + s * 0.26),
        ),
        0.0,
        Stroke::new(w, col),
        egui::StrokeKind::Middle,
    );
    p.rect_filled(
        Rect::from_min_max(
            pos2(r.min.x + s * 0.2, c.y + s * 0.08),
            pos2(r.max.x - s * 0.2, r.max.y - s * 0.12),
        ),
        s * 0.04,
        col,
    );
}

/// A die (Randomise).
pub fn icon_dice(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let r = Rect::from_center_size(c, vec2(s * 0.8, s * 0.8));
    p.rect_stroke(r, s * 0.16, Stroke::new(s * 0.1, col), egui::StrokeKind::Middle);
    for (dx, dy) in [(-0.2, -0.2), (0.2, 0.2), (0.0, 0.0), (0.2, -0.2), (-0.2, 0.2)] {
        p.circle_filled(c + vec2(dx * s, dy * s), s * 0.07, col);
    }
}

/// A chevron, left (`dir` -1) or right (1).
pub fn icon_chev(p: &Painter, c: Pos2, s: f32, col: Color32, dir: f32) {
    let w = s * 0.14;
    let a = pos2(c.x - dir * s * 0.15, c.y - s * 0.3);
    let b = pos2(c.x + dir * s * 0.15, c.y);
    let d = pos2(c.x - dir * s * 0.15, c.y + s * 0.3);
    p.add(egui::Shape::line(vec![a, b, d], Stroke::new(w, col)));
}

/// A magnifier (search).
pub fn icon_search(p: &Painter, c: Pos2, s: f32, col: Color32) {
    let w = s * 0.12;
    p.circle_stroke(c + vec2(-s * 0.08, -s * 0.08), s * 0.28, Stroke::new(w, col));
    p.line_segment(
        [c + vec2(s * 0.13, s * 0.13), c + vec2(s * 0.36, s * 0.36)],
        Stroke::new(w, col),
    );
}

/// The text box's cursor, blinking.
pub fn caret(p: &Painter, x: f32, cy: f32, h: f32, col: Color32) {
    let t = p.ctx().input(|i| i.time);
    if (t * 2.0).fract() < 0.6 {
        p.line_segment(
            [pos2(x + 1.0, cy - h / 2.0), pos2(x + 1.0, cy + h / 2.0)],
            Stroke::new(1.5, col),
        );
    }
    p.ctx()
        .request_repaint_after(std::time::Duration::from_millis(250));
}
