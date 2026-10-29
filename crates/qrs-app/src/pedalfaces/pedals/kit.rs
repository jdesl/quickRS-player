//! The parts every pedal and rack face is built from (PLAN.md P21.3), after the approved mockups' generators: a
//! canvas in the mockup's units (2.2 px a millimetre) for the cached body (tiny-skia: enclosures, finishes,
//! knob bodies, footswitches, screws, bezels), and the live parts drawn with egui over it (pointers, LED lenses,
//! switches, print).

use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};
use qrs_ui::raster::{self, Image};
use tiny_skia::{
    FillRule, GradientStop, LineCap, LinearGradient, Mask, Paint, Path, PathBuilder, Pixmap, Point,
    RadialGradient, Rect as SkRect, SpreadMode, Transform,
};

use super::super::Lettering;

// -- colours ---------------------------------------------------------------------------------------------------------

/// A colour as 0xRRGGBB.
pub type C = u32;

pub fn rgb(c: C) -> [u8; 3] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8]
}

pub fn c32(c: C) -> Color32 {
    let [r, g, b] = rgb(c);
    Color32::from_rgb(r, g, b)
}

pub fn c32a(c: C, a: f32) -> Color32 {
    let [r, g, b] = rgb(c);
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn from3(c: [u8; 3]) -> C {
    ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32
}

pub fn mix(a: C, b: C, t: f32) -> C {
    let (a, b) = (rgb(a), rgb(b));
    let f = |i: usize| {
        (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    from3([f(0), f(1), f(2)])
}

pub fn lt(c: C, t: f32) -> C {
    mix(c, 0xffffff, t)
}

pub fn dk(c: C, t: f32) -> C {
    mix(c, 0x000000, t)
}

fn skc(c: C, a: f32) -> tiny_skia::Color {
    let [r, g, b] = rgb(c);
    tiny_skia::Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
}

/// A knob's angle (degrees, 0 up, clockwise) at a turn of `u`.
pub fn ang(u: f32) -> f32 {
    -135.0 + 270.0 * u
}

/// A tiny repeatable random number.
pub fn hash01(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

// -- where the mockup's units land on screen -------------------------------------------------------------------------

/// The mapping from a face's mockup units to screen points: `o + v * k`.
#[derive(Clone, Copy, Debug)]
pub struct Fit {
    pub o: Pos2,
    pub k: f32,
}

impl Fit {
    /// The `w` x `h` face (plus the `ext` margins left, top, right, bottom for parts that stick out) fitted
    /// into `r`, centred.
    pub fn new(r: Rect, w: f32, h: f32, ext: [f32; 4]) -> Fit {
        let tw = w + ext[0] + ext[2];
        let th = h + ext[1] + ext[3];
        let k = (r.width() / tw).min(r.height() / th).max(1e-3);
        let left = r.center().x - tw * k * 0.5;
        let top = r.center().y - th * k * 0.5;
        Fit {
            o: pos2(left + ext[0] * k, top + ext[1] * k),
            k,
        }
    }

    pub fn p(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.o.x + x * self.k, self.o.y + y * self.k)
    }

    pub fn s(&self, v: f32) -> f32 {
        v * self.k
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.p(x, y), vec2(w * self.k, h * self.k))
    }
}

// -- the body canvas ---------------------------------------------------------------------------------------------------

/// A pixmap covering the face's rect at device pixels, drawn in mockup units.
pub struct Cv {
    pub pm: Pixmap,
    pub tr: Transform,
}

impl Cv {
    pub fn new(r: Rect, fit: &Fit, ppp: f32) -> Option<Cv> {
        let w = (r.width() * ppp).round().max(2.0) as u32;
        let h = (r.height() * ppp).round().max(2.0) as u32;
        let pm = Pixmap::new(w, h)?;
        let px = fit.k * ppp;
        let tr = Transform::from_row(
            px,
            0.0,
            0.0,
            px,
            (fit.o.x - r.min.x) * ppp,
            (fit.o.y - r.min.y) * ppp,
        );
        Some(Cv { pm, tr })
    }

    pub fn image(&self) -> Image {
        raster::from_pixmap(&self.pm)
    }

    pub fn fill(&mut self, path: &Path, paint: &Paint) {
        self.pm.fill_path(path, paint, FillRule::Winding, self.tr, None);
    }

    pub fn fill_in(&mut self, path: &Path, paint: &Paint, m: &Mask) {
        self.pm
            .fill_path(path, paint, FillRule::Winding, self.tr, Some(m));
    }

    pub fn stroke(&mut self, path: &Path, paint: &Paint, w: f32) {
        let st = tiny_skia::Stroke {
            width: w,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        self.pm.stroke_path(path, paint, &st, self.tr, None);
    }

    pub fn stroke_in(&mut self, path: &Path, paint: &Paint, w: f32, m: &Mask) {
        let st = tiny_skia::Stroke {
            width: w,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        self.pm.stroke_path(path, paint, &st, self.tr, Some(m));
    }

    /// A dashed stroke (knurling).
    pub fn dashed(&mut self, path: &Path, paint: &Paint, w: f32, on: f32, off: f32) {
        let st = tiny_skia::Stroke {
            width: w,
            dash: tiny_skia::StrokeDash::new(vec![on.max(0.01), off.max(0.01)], 0.0),
            ..Default::default()
        };
        self.pm.stroke_path(path, paint, &st, self.tr, None);
    }

    /// A clip of `path`.
    pub fn mask(&self, path: &Path) -> Mask {
        let mut m = Mask::new(self.pm.width(), self.pm.height())
            .unwrap_or_else(|| Mask::new(1, 1).expect("1x1 mask"));
        m.fill_path(path, FillRule::Winding, true, self.tr);
        m
    }

    // shorthands
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, rx: f32, paint: &Paint) {
        if let Some(p) = rrect(x, y, w, h, rx) {
            self.fill(&p, paint);
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, paint: &Paint) {
        if let Some(p) = circle(cx, cy, r) {
            self.fill(&p, paint);
        }
    }

    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, paint: &Paint, w: f32) {
        if let Some(p) = circle(cx, cy, r) {
            self.stroke(&p, paint, w);
        }
    }

    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, paint: &Paint, w: f32) {
        if let Some(p) = raster::polyline(&[(x0, y0), (x1, y1)]) {
            self.stroke(&p, paint, w);
        }
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, paint: &Paint) {
        if let Some(p) =
            SkRect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0).and_then(PathBuilder::from_oval)
        {
            self.fill(&p, paint);
        }
    }
}

// -- paths -------------------------------------------------------------------------------------------------------------

pub fn rrect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    rrect4(x, y, w, h, [r; 4])
}

/// A rect with its own radius at each corner (top-left, top-right, bottom-right, bottom-left).
pub fn rrect4(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Option<Path> {
    let m = (w.min(h) * 0.5).max(0.0);
    let r = r.map(|v| v.clamp(0.0, m));
    let mut b = PathBuilder::new();
    b.move_to(x + r[0], y);
    b.line_to(x + w - r[1], y);
    b.quad_to(x + w, y, x + w, y + r[1]);
    b.line_to(x + w, y + h - r[2]);
    b.quad_to(x + w, y + h, x + w - r[2], y + h);
    b.line_to(x + r[3], y + h);
    b.quad_to(x, y + h, x, y + h - r[3]);
    b.line_to(x, y + r[0]);
    b.quad_to(x, y, x + r[0], y);
    b.close();
    b.finish()
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Option<Path> {
    PathBuilder::from_circle(cx, cy, r.max(0.01))
}

pub fn poly(pts: &[(f32, f32)]) -> Option<Path> {
    raster::polygon(pts)
}

/// A polygon with rounded corners (the generators' `rpoly`).
pub fn rpoly(pts: &[(f32, f32)], radii: &[f32]) -> Option<Path> {
    let m = pts.len();
    let mut b = PathBuilder::new();
    for i in 0..m {
        let p0 = pts[(i + m - 1) % m];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % m];
        let r = radii.get(i).copied().unwrap_or(radii[0]);
        let v1 = (p0.0 - p1.0, p0.1 - p1.1);
        let l1 = (v1.0 * v1.0 + v1.1 * v1.1).sqrt().max(1e-3);
        let v2 = (p2.0 - p1.0, p2.1 - p1.1);
        let l2 = (v2.0 * v2.0 + v2.1 * v2.1).sqrt().max(1e-3);
        let a = (p1.0 + v1.0 / l1 * r, p1.1 + v1.1 / l1 * r);
        let c = (p1.0 + v2.0 / l2 * r, p1.1 + v2.1 / l2 * r);
        if i == 0 {
            b.move_to(a.0, a.1);
        } else {
            b.line_to(a.0, a.1);
        }
        b.quad_to(p1.0, p1.1, c.0, c.1);
    }
    b.close();
    b.finish()
}

/// Points along an arc (degrees, 0 = +x, clockwise on screen).
pub fn arc_pts(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, n: usize) -> Vec<(f32, f32)> {
    (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

// -- paints -------------------------------------------------------------------------------------------------------------

pub fn col(c: C, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(skc(c, a));
    p.anti_alias = true;
    p
}

fn stops(s: &[(f32, C, f32)]) -> Vec<GradientStop> {
    s.iter()
        .map(|&(o, c, a)| GradientStop::new(o, skc(c, a)))
        .collect()
}

/// A linear gradient from (x0, y0) to (x1, y1); stops are (offset, colour, alpha).
pub fn lin(x0: f32, y0: f32, x1: f32, y1: f32, s: &[(f32, C, f32)]) -> Paint<'static> {
    match LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        stops(s),
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        Some(shader) => Paint {
            shader,
            anti_alias: true,
            ..Default::default()
        },
        None => col(s.first().map(|x| x.1).unwrap_or(0), 1.0),
    }
}

/// A radial gradient centred at (cx, cy), radii rx, ry (an ellipse).
pub fn rad(cx: f32, cy: f32, rx: f32, ry: f32, s: &[(f32, C, f32)]) -> Paint<'static> {
    match RadialGradient::new(
        Point::from_xy(0.0, 0.0),
        0.0,
        Point::from_xy(0.0, 0.0),
        1.0,
        stops(s),
        SpreadMode::Pad,
        Transform::from_row(rx.max(0.01), 0.0, 0.0, ry.max(0.01), cx, cy),
    ) {
        Some(shader) => Paint {
            shader,
            anti_alias: true,
            ..Default::default()
        },
        None => col(s.first().map(|x| x.1).unwrap_or(0), 1.0),
    }
}

/// The generators' radial gradients set in a box: centre at (fx, fy) of it, radius `fr` of its width.
pub fn rad_box(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fx: f32,
    fy: f32,
    fr: f32,
    s: &[(f32, C, f32)],
) -> Paint<'static> {
    rad(x + w * fx, y + h * fy, w * fr, h * fr, s)
}

// finishes over a shape's box
pub fn gloss(x: f32, y: f32, w: f32, h: f32) -> Paint<'static> {
    lin(
        x,
        y,
        x + w,
        y + h,
        &[
            (0.0, 0xffffff, 0.34),
            (0.2, 0xffffff, 0.1),
            (0.42, 0xffffff, 0.0),
            (0.75, 0x000000, 0.0),
            (1.0, 0x000000, 0.2),
        ],
    )
}

pub fn satin(x: f32, y: f32, w: f32, h: f32) -> Paint<'static> {
    lin(
        x,
        y,
        x + w * 0.7,
        y + h,
        &[
            (0.0, 0xffffff, 0.16),
            (0.45, 0xffffff, 0.0),
            (1.0, 0x000000, 0.18),
        ],
    )
}

pub fn matte(x: f32, y: f32, _w: f32, h: f32) -> Paint<'static> {
    lin(x, y, x, y + h, &[(0.0, 0xffffff, 0.07), (1.0, 0x000000, 0.16)])
}

/// Chrome: a diagonal of bright and dark bands.
pub fn chrome(x: f32, y: f32, w: f32, h: f32) -> Paint<'static> {
    lin(
        x,
        y,
        x + w,
        y + h,
        &[
            (0.0, 0xf6f8fa, 1.0),
            (0.3, 0x9ba2a9, 1.0),
            (0.48, 0xeef0f2, 1.0),
            (0.7, 0x5a6067, 1.0),
            (1.0, 0xcdd1d6, 1.0),
        ],
    )
}

pub fn chrome2(x: f32, y: f32, w: f32, h: f32) -> Paint<'static> {
    lin(
        x + w,
        y,
        x,
        y + h,
        &[
            (0.0, 0xe9ecef, 1.0),
            (0.35, 0x6c727a, 1.0),
            (0.55, 0xf4f6f8, 1.0),
            (1.0, 0x7d838a, 1.0),
        ],
    )
}

pub fn chrome_v(x: f32, y: f32, _w: f32, h: f32) -> Paint<'static> {
    lin(
        x,
        y,
        x,
        y + h,
        &[
            (0.0, 0xf4f6f8, 1.0),
            (0.45, 0xa7adb4, 1.0),
            (0.55, 0x6a7077, 1.0),
            (1.0, 0xd9dde1, 1.0),
        ],
    )
}

pub fn chrome_h(x: f32, y: f32, w: f32, _h: f32) -> Paint<'static> {
    lin(
        x,
        y,
        x + w,
        y,
        &[
            (0.0, 0x6a7077, 1.0),
            (0.3, 0xeef0f2, 1.0),
            (0.6, 0x9aa0a7, 1.0),
            (1.0, 0x555b62, 1.0),
        ],
    )
}

pub fn dome(cx: f32, cy: f32, r: f32) -> Paint<'static> {
    rad_box(
        cx - r,
        cy - r,
        2.0 * r,
        2.0 * r,
        0.36,
        0.3,
        0.78,
        &[
            (0.0, 0xffffff, 1.0),
            (0.22, 0xe6e9ec, 1.0),
            (0.58, 0x8b9198, 1.0),
            (0.84, 0x3f444a, 1.0),
            (1.0, 0xb5bbc1, 1.0),
        ],
    )
}

fn knob_top(cx: f32, cy: f32, r: f32) -> Paint<'static> {
    rad_box(
        cx - r,
        cy - r,
        2.0 * r,
        2.0 * r,
        0.4,
        0.32,
        0.72,
        &[(0.0, 0x4c4e53, 1.0), (0.55, 0x1b1c1e, 1.0), (1.0, 0x0a0a0b, 1.0)],
    )
}

fn silver_cap(cx: f32, cy: f32, r: f32) -> Paint<'static> {
    rad_box(
        cx - r,
        cy - r,
        2.0 * r,
        2.0 * r,
        0.36,
        0.3,
        0.78,
        &[
            (0.0, 0xffffff, 1.0),
            (0.3, 0xd8dce0, 1.0),
            (0.78, 0x8b9198, 1.0),
            (1.0, 0xb9bec4, 1.0),
        ],
    )
}

/// A body colour's gradient (light top-left, the colour, dark bottom-right).
pub fn body_grad(c: C, x: f32, y: f32, w: f32, h: f32) -> Paint<'static> {
    lin(
        x,
        y,
        x + w * 0.55,
        y + h,
        &[(0.0, lt(c, 0.17), 1.0), (0.5, c, 1.0), (1.0, dk(c, 0.24), 1.0)],
    )
}

// -- textures ----------------------------------------------------------------------------------------------------------

/// Specks over a box: `per` specks per 100 square units, colours (colour, alpha) picked at random, sizes from
/// `smin` to `smax`, round or square.
#[allow(clippy::too_many_arguments)]
pub fn specks(
    cv: &mut Cv,
    m: &Mask,
    (x, y, w, h): (f32, f32, f32, f32),
    per: f32,
    colors: &[(C, f32)],
    smin: f32,
    smax: f32,
    seed: u32,
    round: bool,
) {
    let n = ((w * h / 100.0 * per) as u32).min(60_000);
    let paints: Vec<Paint> = colors.iter().map(|&(c, a)| col(c, a)).collect();
    // in device space, so a speck is at least half a pixel
    for i in 0..n {
        let k = seed.wrapping_mul(7919).wrapping_add(i * 5);
        let sx = x + hash01(k) * w;
        let sy = y + hash01(k + 1) * h;
        let s = smin + (smax - smin) * hash01(k + 2);
        let p = &paints[(hash01(k + 3) * paints.len() as f32) as usize % paints.len()];
        if round {
            if let Some(c) = circle(sx, sy, s * 0.5) {
                cv.fill_in(&c, p, m);
            }
        } else if let Some(r) = SkRect::from_xywh(sx, sy, s, s) {
            let path = PathBuilder::from_rect(r);
            cv.fill_in(&path, p, m);
        }
    }
}

/// Hammertone: overlapping dimples, a shadow below and a light above each.
pub fn hammer(cv: &mut Cv, m: &Mask, (x, y, w, h): (f32, f32, f32, f32), seed: u32) {
    let step = 9.5;
    let mut j = 0u32;
    let mut yy = y - step;
    while yy < y + h + step {
        let mut xx = x - step + if j.is_multiple_of(2) { 0.0 } else { step * 0.5 };
        let mut i = 0u32;
        while xx < x + w + step {
            let k = seed.wrapping_add(j * 977 + i * 31);
            let cx = xx + (hash01(k) - 0.5) * step * 0.6;
            let cy = yy + (hash01(k + 1) - 0.5) * step * 0.6;
            let r = 3.0 + 2.2 * hash01(k + 2);
            if let Some(c) = circle(cx + 0.8, cy + 0.9, r) {
                cv.fill_in(
                    &c,
                    &rad(
                        cx + 0.8,
                        cy + 0.9,
                        r,
                        r,
                        &[(0.0, 0, 0.34), (0.7, 0, 0.1), (1.0, 0, 0.0)],
                    ),
                    m,
                );
            }
            if let Some(c) = circle(cx - 0.9, cy - 1.0, r * 0.62) {
                cv.fill_in(
                    &c,
                    &rad(
                        cx - 0.9,
                        cy - 1.0,
                        r * 0.62,
                        r * 0.62,
                        &[(0.0, 0xffffff, 0.22), (1.0, 0xffffff, 0.0)],
                    ),
                    m,
                );
            }
            xx += step;
            i += 1;
        }
        yy += step * 0.86;
        j += 1;
    }
}

/// Brushed metal: hairlines across, light and dark.
pub fn brushed(cv: &mut Cv, m: &Mask, (x, y, w, h): (f32, f32, f32, f32), seed: u32, strength: f32) {
    let mut yy = y;
    let mut i = 0u32;
    while yy < y + h {
        let mut xx = x - 40.0 * hash01(seed + i);
        let mut j = 0u32;
        while xx < x + w {
            let k = seed.wrapping_add(i * 131 + j * 7);
            let len = 20.0 + 70.0 * hash01(k);
            let light = hash01(k + 1) < 0.5;
            let a = (0.03 + 0.1 * hash01(k + 2)) * strength;
            if let Some(r) = SkRect::from_xywh(xx, yy, len, 0.6) {
                let p = PathBuilder::from_rect(r);
                cv.fill_in(&p, &col(if light { 0xffffff } else { 0 }, a), m);
            }
            xx += len + 12.0 * hash01(k + 3);
            j += 1;
        }
        yy += 1.0;
        i += 1;
    }
}

/// Wood grain: wavy lines along.
pub fn wood(cv: &mut Cv, m: &Mask, (x, y, w, h): (f32, f32, f32, f32), dark: C) {
    let mut yy = y + 2.0;
    let mut i = 0u32;
    while yy < y + h {
        let a = 0.07 + 0.1 * hash01(i + 40);
        let mut pts = Vec::new();
        let mut xx = x;
        let ph = hash01(i + 80) * 6.0;
        while xx <= x + w + 4.0 {
            pts.push((
                xx,
                yy + 2.2 * ((xx - x) / 38.0 + ph).sin() + 0.8 * ((xx - x) / 11.0 + ph).sin(),
            ));
            xx += 4.0;
        }
        if let Some(p) = raster::polyline(&pts) {
            cv.stroke_in(&p, &col(dark, a), if i.is_multiple_of(2) { 1.1 } else { 0.7 }, m);
        }
        yy += 3.0 + 4.0 * hash01(i + 7);
        i += 1;
    }
}

/// A rubber pad's ribs.
pub fn rubber(cv: &mut Cv, x: f32, y: f32, w: f32, h: f32, rx: f32, base: C) {
    let Some(pad) = rrect(x, y, w, h, rx) else { return };
    // its shadow
    if let Some(sh) = rrect(x + 0.5, y + 1.8, w, h, rx) {
        cv.fill(&sh, &col(0, 0.45));
    }
    cv.fill(&pad, &col(dk(base, 0.25), 1.0));
    let m = cv.mask(&pad);
    let mut yy = y + 0.6;
    while yy < y + h {
        cv.fill_in(
            &PathBuilder::from_rect(
                SkRect::from_xywh(x, yy, w, 2.6).unwrap_or(SkRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()),
            ),
            &col(lt(base, 0.08), 1.0),
            &m,
        );
        cv.fill_in(
            &PathBuilder::from_rect(
                SkRect::from_xywh(x, yy, w, 0.55).unwrap_or(SkRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()),
            ),
            &col(lt(base, 0.18), 1.0),
            &m,
        );
        yy += 5.0;
    }
    cv.fill(&pad, &matte(x, y, w, h));
    cv.stroke(&pad, &col(0, 0.9), 0.8);
}

/// Diamond tread (a wah's rubber): little raised squares turned 45 degrees.
pub fn diamond(cv: &mut Cv, m: &Mask, (x, y, w, h): (f32, f32, f32, f32)) {
    cv.fill_in(
        &PathBuilder::from_rect(
            SkRect::from_xywh(x, y, w, h).unwrap_or(SkRect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()),
        ),
        &col(0x121315, 1.0),
        m,
    );
    let step = 9.0;
    let mut j = 0u32;
    let mut yy = y;
    while yy < y + h + step {
        let mut xx = x + if j.is_multiple_of(2) { 0.0 } else { step * 0.5 };
        while xx < x + w + step {
            let d = 3.3;
            if let Some(p) = poly(&[(xx, yy - d), (xx + d, yy), (xx, yy + d), (xx - d, yy)]) {
                cv.fill_in(&p, &col(0x26272a, 1.0), m);
            }
            if let Some(p) = raster::polyline(&[(xx - d, yy), (xx, yy - d), (xx + d, yy)]) {
                cv.stroke_in(&p, &col(0x3a3b3f, 1.0), 0.7, m);
            }
            xx += step;
        }
        yy += step * 0.5;
        j += 1;
    }
}

/// A woven cloth: two sets of fine diagonals.
pub fn cloth(cv: &mut Cv, m: &Mask, (x, y, w, h): (f32, f32, f32, f32), ink: C, a: f32, step: f32) {
    let mut k = -h;
    while k < w + h {
        for (dir, aa) in [(1.0f32, a), (-1.0, a * 0.8)] {
            let (x0, x1) = if dir > 0.0 {
                (x + k, x + k + h)
            } else {
                (x + k + h, x + k)
            };
            if let Some(line) = raster::polyline(&[(x0, y), (x1, y + h)]) {
                cv.stroke_in(&line, &col(ink, aa), 0.9, m);
            }
        }
        k += step;
    }
}

// -- parts drawn into the body -----------------------------------------------------------------------------------------

/// The soft shadow a round part casts.
pub fn kshadow(cv: &mut Cv, cx: f32, cy: f32, r: f32, a: f32) {
    cv.ellipse(
        cx + r * 0.1,
        cy + r * 0.28,
        r * 1.16,
        r * 1.1,
        &rad(
            cx + r * 0.1,
            cy + r * 0.28,
            r * 1.16,
            r * 1.1,
            &[(0.0, 0, a), (0.75, 0, a * 0.8), (1.0, 0, 0.0)],
        ),
    );
}

/// Knurling: a dashed ring.
pub fn knurl(cv: &mut Cv, cx: f32, cy: f32, r: f32, width: f32, c: C, teeth: u32, a: f32) {
    if let Some(p) = circle(cx, cy, r) {
        let d = std::f32::consts::PI * r / teeth as f32;
        cv.dashed(&p, &col(c, a), width, d, d);
    }
}

/// How a knob looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ks {
    /// Boss: black knurled skirt, a silver ring, a black top.
    Boss,
    /// MXR: a black knurled knob.
    Mxr,
    /// A chrome knurled knob (Tone Bender, Copper Top).
    Chrome,
    /// Black skirt and a silver cap (Fuzz Face, Big Muff).
    SilverCap,
    /// A chicken-head pointer, black or cream.
    Chicken(bool),
    /// Fender-style skirted knob numbered 1-10 on the skirt (colour of the skirt, the numbers, the cap).
    Numbered(C, C, C),
    /// A rack knob: chrome skirt, black top.
    Rack,
    /// A modern matte knob with a coloured pointer.
    Modern(C),
    /// A cream thumb wheel (the radio).
    Wheel,
    /// A small brass-capped knob (the rotary cabinet).
    Brass,
    /// A console knob with a coloured cap (SSL style).
    Ssl(C),
    /// A big aluminium knob (the rack compressor).
    Alu,
    /// A silver-skirted black knob (the tape echo).
    SSkirt,
    /// Nothing drawn (a treadle).
    Hidden,
}

/// A knob's static body (everything but its pointer).
pub fn knob_body(cv: &mut Cv, s: Ks, cx: f32, cy: f32, r: f32) {
    match s {
        Ks::Hidden => {}
        Ks::Boss => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(cx, cy, r, &col(0x101010, 1.0));
            knurl(cv, cx, cy, r * 0.9, r * 0.2, 0x3b3b3d, 26, 1.0);
            let rr = r * 0.74;
            cv.circle(
                cx,
                cy,
                rr,
                &lin(
                    cx - rr,
                    cy - rr,
                    cx + rr,
                    cy + rr,
                    &[(0.0, 0xe3e6e9, 1.0), (0.5, 0x7d838a, 1.0), (1.0, 0xc3c8cd, 1.0)],
                ),
            );
            cv.circle(cx, cy, r * 0.66, &knob_top(cx, cy, r * 0.66));
        }
        Ks::Mxr => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(cx, cy, r, &col(0x0c0c0c, 1.0));
            knurl(cv, cx, cy, r * 0.93, r * 0.14, 0x303033, 40, 1.0);
            cv.circle(cx, cy, r * 0.82, &knob_top(cx, cy, r * 0.82));
        }
        Ks::Chrome => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(cx, cy, r, &chrome(cx - r, cy - r, 2.0 * r, 2.0 * r));
            knurl(cv, cx, cy, r * 0.9, r * 0.2, 0x4f555c, 26, 0.85);
            cv.circle(cx, cy, r * 0.68, &dome(cx, cy, r * 0.68));
            cv.ring(cx, cy, r * 0.68, &col(0x6b7178, 1.0), 0.6);
        }
        Ks::SilverCap => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(cx, cy, r, &col(0x0e0e0e, 1.0));
            knurl(cv, cx, cy, r * 0.92, r * 0.16, 0x353538, 32, 1.0);
            cv.circle(cx, cy, r * 0.64, &silver_cap(cx, cy, r * 0.64));
            cv.ring(cx, cy, r * 0.64, &col(0x1a1a1a, 1.0), 0.8);
        }
        Ks::Chicken(_) => {
            kshadow(cv, cx, cy, r * 0.8, 0.45);
        }
        Ks::Numbered(..) => {
            kshadow(cv, cx, cy, r, 0.5);
        }
        Ks::Rack => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(cx, cy, r, &chrome(cx - r, cy - r, 2.0 * r, 2.0 * r));
            cv.ring(cx, cy, r, &col(0x2a2d31, 1.0), 0.7);
            cv.circle(cx, cy, r * 0.74, &knob_top(cx, cy, r * 0.74));
            cv.ring(cx, cy, r * 0.74, &col(0, 1.0), 0.6);
        }
        Ks::Modern(_) => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(
                cx,
                cy,
                r,
                &rad_box(
                    cx - r,
                    cy - r,
                    2.0 * r,
                    2.0 * r,
                    0.38,
                    0.3,
                    0.75,
                    &[(0.0, 0x3a3d42, 1.0), (0.7, 0x17181a, 1.0), (1.0, 0x0b0b0c, 1.0)],
                ),
            );
            cv.ring(cx, cy, r * 0.9, &col(0, 0.6), 0.8);
            cv.ring(cx, cy, r * 0.72, &col(0xffffff, 0.06), 1.0);
        }
        Ks::Wheel => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(cx, cy, r, &col(0xc8b994, 1.0));
            cv.ring(cx, cy, r, &col(0x7f704f, 1.0), 0.7);
            knurl(cv, cx, cy, r * 0.94, r * 0.12, 0x94855f, 44, 1.0);
            cv.circle(
                cx,
                cy,
                r * 0.84,
                &rad_box(
                    cx - r,
                    cy - r,
                    2.0 * r,
                    2.0 * r,
                    0.38,
                    0.3,
                    0.8,
                    &[(0.0, 0xfffbf2, 1.0), (0.7, 0xece2c8, 1.0), (1.0, 0xc9b994, 1.0)],
                ),
            );
            cv.ring(cx, cy, r * 0.6, &col(0xb9a983, 1.0), 0.8);
            cv.ring(cx, cy, r * 0.4, &col(0xb9a983, 1.0), 0.6);
        }
        Ks::Brass => {
            kshadow(cv, cx, cy, r, 0.55);
            cv.circle(cx, cy, r, &col(0x33241a, 1.0));
            knurl(cv, cx, cy, r * 0.92, r * 0.16, 0x2a1a0f, 24, 1.0);
            let rr = r * 0.64;
            cv.circle(
                cx,
                cy,
                rr,
                &rad_box(
                    cx - rr,
                    cy - rr,
                    2.0 * rr,
                    2.0 * rr,
                    0.36,
                    0.3,
                    0.8,
                    &[
                        (0.0, 0xfff3c4, 1.0),
                        (0.35, 0xd7ae55, 1.0),
                        (0.75, 0x8d6420, 1.0),
                        (1.0, 0xc49a45, 1.0),
                    ],
                ),
            );
        }
        Ks::Ssl(cap) => {
            kshadow(cv, cx, cy, r, 0.5);
            cv.circle(cx, cy, r, &col(0x1d1f22, 1.0));
            knurl(cv, cx, cy, r * 0.93, r * 0.14, 0x383b3f, 36, 1.0);
            cv.circle(cx, cy, r * 0.74, &knob_top(cx, cy, r * 0.74));
            let rr = r * 0.42;
            cv.circle(
                cx,
                cy,
                rr,
                &rad_box(
                    cx - rr,
                    cy - rr,
                    2.0 * rr,
                    2.0 * rr,
                    0.38,
                    0.3,
                    0.8,
                    &[
                        (0.0, lt(cap, 0.45), 1.0),
                        (0.55, cap, 1.0),
                        (1.0, dk(cap, 0.35), 1.0),
                    ],
                ),
            );
        }
        Ks::Alu => {
            kshadow(cv, cx, cy, r, 0.6);
            cv.circle(cx, cy, r * 1.04, &col(0x050505, 1.0));
            // a conic-ish brushed top: bands of light round it
            for i in 0..16 {
                let a0 = i as f32 * 22.5;
                let pts: Vec<(f32, f32)> = std::iter::once((cx, cy))
                    .chain(arc_pts(cx, cy, r, a0, a0 + 22.5, 4))
                    .collect();
                let t = (((a0 + 11.25).to_radians() * 2.0).cos() * 0.5 + 0.5) * 0.7 + 0.15;
                if let Some(p) = poly(&pts) {
                    cv.fill(&p, &col(mix(0x8b9095, 0xeef0f2, t), 1.0));
                }
            }
            cv.circle(
                cx,
                cy,
                r,
                &rad_box(
                    cx - r,
                    cy - r,
                    2.0 * r,
                    2.0 * r,
                    0.4,
                    0.32,
                    0.6,
                    &[(0.0, 0xffffff, 0.5), (1.0, 0xffffff, 0.0)],
                ),
            );
        }
        Ks::SSkirt => {
            kshadow(cv, cx, cy, r, 0.6);
            cv.circle(
                cx,
                cy,
                r,
                &rad_box(
                    cx - r,
                    cy - r,
                    2.0 * r,
                    2.0 * r,
                    0.4,
                    0.3,
                    0.8,
                    &[(0.0, 0xf6f7f8, 1.0), (0.55, 0xaab0b5, 1.0), (1.0, 0x5b6066, 1.0)],
                ),
            );
            cv.ring(cx, cy, r, &col(0, 0.6), 0.8);
            cv.circle(cx, cy, r * 0.66, &knob_top(cx, cy, r * 0.66));
            cv.ring(cx, cy, r * 0.66, &col(0, 1.0), 0.8);
        }
    }
}

/// The chrome footswitch: a dark ring, knurled chrome, a hex nut, a domed cap.
pub fn stomp(cv: &mut Cv, cx: f32, cy: f32, r: f32) {
    cv.ellipse(
        cx + r * 0.12,
        cy + r * 0.32,
        r * 1.2,
        r * 1.08,
        &rad(
            cx + r * 0.12,
            cy + r * 0.32,
            r * 1.2,
            r * 1.08,
            &[(0.0, 0, 0.6), (0.7, 0, 0.45), (1.0, 0, 0.0)],
        ),
    );
    cv.circle(cx, cy, r * 1.06, &col(0x25272a, 1.0));
    cv.circle(cx, cy, r, &chrome(cx - r, cy - r, 2.0 * r, 2.0 * r));
    knurl(cv, cx, cy, r * 0.92, r * 0.16, 0x5b6168, 34, 0.9);
    let hex: Vec<(f32, f32)> = (0..6)
        .map(|i| {
            let a = (30.0 + 60.0 * i as f32).to_radians();
            (cx + r * 0.8 * a.cos(), cy + r * 0.8 * a.sin())
        })
        .collect();
    if let Some(p) = poly(&hex) {
        cv.fill(&p, &chrome2(cx - r, cy - r, 2.0 * r, 2.0 * r));
        cv.stroke(&p, &col(0x43484e, 1.0), 0.7);
    }
    cv.circle(cx, cy, r * 0.6, &col(0x7d838a, 1.0));
    cv.circle(cx, cy, r * 0.52, &dome(cx, cy, r * 0.52));
    cv.ellipse(
        cx - r * 0.16,
        cy - r * 0.2,
        r * 0.17,
        r * 0.1,
        &col(0xffffff, 0.85),
    );
}

/// An LED's chrome bezel and dark socket (the lens is drawn live).
pub fn led_bezel(cv: &mut Cv, cx: f32, cy: f32, r: f32) {
    cv.circle(
        cx,
        cy,
        r * 1.55,
        &chrome(cx - r * 1.55, cy - r * 1.55, r * 3.1, r * 3.1),
    );
    cv.circle(cx, cy, r * 1.25, &col(0x16171a, 1.0));
}

/// A jewel lamp's knurled chrome bezel.
pub fn jewel_bezel(cv: &mut Cv, cx: f32, cy: f32, r: f32) {
    kshadow(cv, cx, cy, r * 1.3, 0.45);
    cv.circle(
        cx,
        cy,
        r * 1.38,
        &chrome(cx - r * 1.4, cy - r * 1.4, r * 2.8, r * 2.8),
    );
    cv.ring(cx, cy, r * 1.38, &col(0x3d4248, 1.0), 0.6);
    knurl(cv, cx, cy, r * 1.3, r * 0.14, 0x5b6168, 24, 0.8);
}

/// A screw head (Phillips or slotted).
pub fn screw(cv: &mut Cv, cx: f32, cy: f32, r: f32, a: f32, phillips: bool) {
    cv.circle(cx + 0.5, cy + 0.7, r, &col(0, 0.45));
    cv.circle(cx, cy, r, &silver_cap(cx, cy, r));
    cv.ring(cx, cy, r, &col(0x3d4045, 1.0), 0.5);
    for extra in [0.0f32, 90.0] {
        if extra > 0.0 && !phillips {
            break;
        }
        let t = (a + extra).to_radians();
        let (s, c) = t.sin_cos();
        cv.line(
            cx - r * 0.62 * c,
            cy - r * 0.62 * s,
            cx + r * 0.62 * c,
            cy + r * 0.62 * s,
            &col(0x3a3d42, 1.0),
            r * 0.28,
        );
    }
}

/// Chrome jack nuts sticking out of both side walls of a box `w` wide.
pub fn jacks(cv: &mut Cv, w: f32, y: f32, h: f32) {
    for (x0, s) in [(0.0f32, -1.0f32), (w, 1.0)] {
        let xa = x0.min(x0 + s * 5.0);
        cv.rect(xa, y - h / 2.0, 5.0, h, 1.0, &chrome_v(xa, y - h / 2.0, 5.0, h));
        let xb = (x0 + s * 5.0).min(x0 + s * 8.0);
        cv.rect(
            xb,
            y - h * 0.36,
            3.0,
            h * 0.72,
            0.8,
            &chrome_v(xb, y - h * 0.36, 3.0, h * 0.72),
        );
    }
}

/// A box's enclosure: its colour's gradient, an optional finish over it, a dark edge and a light inner edge.
pub fn enclosure(cv: &mut Cv, path: &Path, (x, y, w, h): (f32, f32, f32, f32), fill: &Paint, finish: Finish) {
    cv.fill(path, fill);
    let m = cv.mask(path);
    finish.apply(cv, &m, (x, y, w, h), path);
    cv.stroke(path, &col(0, 0.6), 1.1);
}

/// A finish laid over a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Finish {
    Gloss,
    Satin,
    /// Metal-flake sparkle over gloss (orange flecks: Molten).
    Sparkle,
    /// Fine grain under satin.
    Grain,
    /// Black crinkle under matte.
    Crinkle,
    /// Hammertone dimples under satin.
    Hammer,
    /// Brushed metal under satin.
    Brushed,
    /// Wood grain under gloss.
    Wood,
    None,
}

impl Finish {
    pub fn apply(self, cv: &mut Cv, m: &Mask, b: (f32, f32, f32, f32), path: &Path) {
        let (x, y, w, h) = b;
        match self {
            Finish::Sparkle => specks(
                cv,
                m,
                b,
                3.6,
                &[
                    (0x8f9298, 0.9),
                    (0xc9ccd2, 0.8),
                    (0x6b6e74, 0.9),
                    (0xf2601e, 0.35),
                ],
                0.5,
                1.3,
                11,
                false,
            ),
            Finish::Grain => specks(
                cv,
                m,
                b,
                15.0,
                &[(0, 0.28), (0, 0.16), (0xffffff, 0.08)],
                0.4,
                1.1,
                5,
                false,
            ),
            Finish::Crinkle => specks(
                cv,
                m,
                b,
                18.0,
                &[(0, 0.45), (0xffffff, 0.06), (0x3a3c40, 0.5)],
                0.5,
                1.4,
                9,
                true,
            ),
            Finish::Hammer => hammer(cv, m, b, 7),
            Finish::Brushed => brushed(cv, m, b, 3, 1.0),
            Finish::Wood => wood(cv, m, b, 0x2a1606),
            _ => {}
        }
        let over = match self {
            Finish::Gloss | Finish::Sparkle | Finish::Wood => Some(gloss(x, y, w, h)),
            Finish::Satin | Finish::Grain | Finish::Hammer | Finish::Brushed => Some(satin(x, y, w, h)),
            Finish::Crinkle => Some(matte(x, y, w, h)),
            Finish::None => None,
        };
        if let Some(p) = over {
            cv.fill(path, &p);
        }
    }
}

// -- live parts (egui) ---------------------------------------------------------------------------------------------------

/// A point at `a` degrees (0 up, clockwise) and `r` from (cx, cy).
pub fn polar(cx: f32, cy: f32, r: f32, a: f32) -> (f32, f32) {
    let t = a.to_radians();
    (cx + r * t.sin(), cy - r * t.cos())
}

/// A knob's pointer (and its turning parts) at a turn of `u`.
pub fn pointer(p: &Painter, f: &Fit, s: Ks, cx: f32, cy: f32, r: f32, u: f32) {
    let a = ang(u);
    let line = |r0: f32, r1: f32, c: Color32, w: f32| {
        let (x0, y0) = polar(cx, cy, r0, a);
        let (x1, y1) = polar(cx, cy, r1, a);
        p.line_segment([f.p(x0, y0), f.p(x1, y1)], Stroke::new(f.s(w).max(0.8), c));
    };
    match s {
        Ks::Hidden => {}
        Ks::Boss => line(r * 0.16, r * 0.58, Color32::WHITE, r * 0.13),
        Ks::Mxr => line(r * 0.3, r * 0.8, Color32::WHITE, r * 0.12),
        Ks::Modern(c) => line(r * 0.34, r * 0.84, c32(c), r * 0.15),
        Ks::Chrome => line(r * 0.1, r * 0.62, Color32::from_rgb(26, 26, 26), r * 0.1),
        Ks::SilverCap => line(r * 0.08, r * 0.6, Color32::from_rgb(17, 17, 17), r * 0.11),
        Ks::Rack => {
            line(r * 0.2, r * 0.68, Color32::WHITE, r * 0.09);
            line(r * 0.78, r * 0.98, Color32::from_rgb(17, 17, 17), r * 0.09);
        }
        Ks::Ssl(_) => line(r * 0.45, r * 0.93, Color32::WHITE, r * 0.08),
        Ks::Alu => line(r * 0.45, r * 0.95, Color32::from_rgb(17, 17, 17), r * 0.1),
        Ks::SSkirt => line(r * 0.7, r * 0.98, Color32::from_rgb(250, 250, 250), r * 0.09),
        Ks::Brass => line(r * 0.25, r * 0.62, Color32::from_rgb(42, 26, 15), r * 0.12),
        Ks::Wheel => {
            let (x, y) = polar(cx, cy, r * 0.72, a);
            p.circle_filled(f.p(x, y), f.s(r * 0.1).max(1.0), Color32::from_rgb(192, 48, 42));
        }
        Ks::Chicken(cream) => chicken(p, f, cx, cy, r, a, cream),
        Ks::Numbered(skirt, num, cap) => numbered(p, f, cx, cy, r, u, skirt, num, cap),
    }
}

/// A chicken-head knob pointing at `a` degrees.
pub fn chicken(p: &Painter, f: &Fit, cx: f32, cy: f32, r: f32, a: f32, cream: bool) {
    let (body, edge, line, hi) = if cream {
        (0xeadfc4, 0x8a7a5c, 0x2a2218, 0xfffaf0)
    } else {
        (0x1a1b1d, 0x000000, 0xf4f4f4, 0x55575c)
    };
    let pt = |x: f32, y: f32| {
        // rotate (x, y) about the centre by a
        let t = a.to_radians();
        let (s, c) = t.sin_cos();
        f.p(cx + x * c - y * s, cy + x * s + y * c)
    };
    let shadow = vec2(f.s(r * 0.08), f.s(r * 0.16));
    let head = [
        pt(-r * 0.32, r * 0.3),
        pt(-r * 0.17, -r * 0.9),
        pt(0.0, -r * 1.04),
        pt(r * 0.17, -r * 0.9),
        pt(r * 0.32, r * 0.3),
        pt(0.0, r * 0.46),
    ];
    p.circle_filled(f.p(cx, cy) + shadow, f.s(r * 0.74), Color32::from_black_alpha(90));
    p.add(egui::Shape::convex_polygon(
        head.iter().map(|q| *q + shadow).collect(),
        Color32::from_black_alpha(90),
        Stroke::NONE,
    ));
    p.circle_filled(f.p(cx, cy), f.s(r * 0.74), c32(body));
    p.add(egui::Shape::convex_polygon(
        head.to_vec(),
        c32(body),
        Stroke::new(f.s(0.6).max(0.5), c32(edge)),
    ));
    p.circle_stroke(
        f.p(cx, cy),
        f.s(r * 0.74),
        Stroke::new(f.s(0.6).max(0.5), c32(edge)),
    );
    // a highlight towards the top-left
    p.circle_filled(
        f.p(cx - r * 0.22, cy - r * 0.24),
        f.s(r * 0.3),
        c32a(hi, if cream { 0.6 } else { 0.35 }),
    );
    p.line_segment(
        [pt(0.0, -r * 0.25), pt(0.0, -r * 0.86)],
        Stroke::new(f.s(r * 0.08).max(0.8), c32(line)),
    );
    p.circle_filled(f.p(cx, cy), f.s(r * 0.28), c32(body));
    p.circle_stroke(
        f.p(cx, cy),
        f.s(r * 0.28),
        Stroke::new(f.s(0.4).max(0.4), c32(edge)),
    );
}

/// A Fender-style knob numbered 1-10 round its skirt; read at the index mark above it.
#[allow(clippy::too_many_arguments)]
pub fn numbered(p: &Painter, f: &Fit, cx: f32, cy: f32, r: f32, u: f32, skirt: C, num: C, cap: C) {
    let c = f.p(cx, cy);
    p.circle_filled(c, f.s(r), c32(skirt));
    p.circle_stroke(
        c,
        f.s(r),
        Stroke::new(f.s(0.8).max(0.5), Color32::from_white_alpha(38)),
    );
    // 1 at the index when turned fully down; the numbers turn with the knob
    let rot = 135.0 - 270.0 * u;
    let size = f.s(r * 0.24);
    if size >= 3.0 {
        for n in 1..=10 {
            let a = -135.0 + (n - 1) as f32 * 30.0 + rot;
            let (x, y) = polar(cx, cy, r * 0.76, a);
            text_rot(
                p,
                &n.to_string(),
                f.p(x, y),
                size,
                c32(num),
                Lettering::Condensed,
                a.to_radians(),
            );
        }
    }
    let rc = r * 0.56;
    p.circle_filled(c, f.s(rc), c32(dk(cap, 0.25)));
    p.circle_filled(f.p(cx - rc * 0.15, cy - rc * 0.18), f.s(rc * 0.78), c32(cap));
    p.circle_filled(
        f.p(cx - rc * 0.3, cy - rc * 0.35),
        f.s(rc * 0.35),
        c32a(lt(cap, 0.4), 0.7),
    );
    p.circle_stroke(
        c,
        f.s(rc),
        Stroke::new(f.s(0.8).max(0.5), Color32::from_black_alpha(90)),
    );
    // the index mark
    let (x0, y0) = (cx, cy - r - 2.0);
    p.add(egui::Shape::convex_polygon(
        vec![f.p(x0, y0), f.p(x0 - 3.0, y0 - 5.0), f.p(x0 + 3.0, y0 - 5.0)],
        c32(num),
        Stroke::NONE,
    ));
}

/// An LED's lens, lit or dark, with its glow.
pub fn led_lens(p: &Painter, f: &Fit, cx: f32, cy: f32, r: f32, c: C, on: bool) {
    let ctr = f.p(cx, cy);
    let rr = f.s(r).max(1.2);
    if on {
        for (k, a) in [(3.4, 0.10), (2.4, 0.18), (1.6, 0.32)] {
            p.circle_filled(ctr, rr * k, c32a(c, a));
        }
        p.circle_filled(ctr, rr, c32(c));
        p.circle_filled(ctr, rr * 0.5, c32a(lt(c, 0.65), 0.95));
    } else {
        p.circle_filled(ctr, rr, c32(mix(c, 0, 0.64)));
    }
    p.circle_filled(
        f.p(cx - r * 0.3, cy - r * 0.38),
        rr * 0.3,
        Color32::from_white_alpha(if on { 210 } else { 70 }),
    );
}

/// A jewel lamp: faceted lens in its bezel, lit or dark.
pub fn jewel_lens(p: &Painter, f: &Fit, cx: f32, cy: f32, r: f32, c: C, on: bool) {
    let ctr = f.p(cx, cy);
    let rr = f.s(r).max(1.5);
    if on {
        for (k, a) in [(2.8, 0.1), (2.0, 0.2), (1.4, 0.3)] {
            p.circle_filled(ctr, rr * k, c32a(c, a));
        }
    }
    let base = if on { c } else { mix(c, 0, 0.66) };
    p.circle_filled(ctr, rr, c32(base));
    p.circle_stroke(ctr, rr, Stroke::new(0.6, Color32::from_black_alpha(130)));
    let fac = lt(base, 0.45);
    let pts: Vec<Pos2> = (0..8)
        .map(|i| {
            let a = (22.5 + 45.0 * i as f32).to_radians();
            pos2(ctr.x + rr * 0.52 * a.cos(), ctr.y + rr * 0.52 * a.sin())
        })
        .collect();
    p.add(egui::Shape::convex_polygon(
        pts.clone(),
        c32(if on { lt(base, 0.25) } else { base }),
        Stroke::new(0.7, c32a(fac, 0.9)),
    ));
    for (i, q) in pts.iter().enumerate() {
        let a = (22.5 + 45.0 * i as f32).to_radians();
        p.line_segment(
            [*q, pos2(ctr.x + rr * a.cos(), ctr.y + rr * a.sin())],
            Stroke::new(0.6, c32a(fac, 0.7)),
        );
    }
    if on {
        p.circle_filled(ctr, rr * 0.28, Color32::from_white_alpha(200));
    }
}

/// A filled rect shaded from `top` to `bottom` (egui has no gradients; a mesh does it).
pub fn vgrad(p: &Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(m));
}

// -- print -----------------------------------------------------------------------------------------------------------------

/// Text in a lettering, rotated `angle` radians (clockwise) about its centre `c`.
pub fn text_rot(p: &Painter, s: &str, c: Pos2, size: f32, color: Color32, style: Lettering, angle: f32) {
    let ctx = p.ctx();
    let family = match super::super::lettering_family(ctx, style) {
        Some(f) => f,
        None => egui::FontFamily::Proportional,
    };
    let g =
        ctx.fonts_mut(|fo| fo.layout_no_wrap(s.to_string(), egui::FontId::new(size.max(1.0), family), color));
    let half = g.size() * 0.5;
    let (sn, cs) = angle.sin_cos();
    let off = vec2(half.x * cs - half.y * sn, half.x * sn + half.y * cs);
    p.add(egui::epaint::TextShape::new(c - off, g, color).with_angle(angle));
}

/// Text along an arc (centre (cx, cy), radius `r`, centred on angle `mid` degrees, 0 up, clockwise), letters
/// upright to the arc; `inside` reads along the bottom of a circle (letters' feet towards the centre... the
/// smile's text).
#[allow(clippy::too_many_arguments)]
pub fn text_arc(
    p: &Painter,
    f: &Fit,
    s: &str,
    cx: f32,
    cy: f32,
    r: f32,
    mid: f32,
    size: f32,
    spacing: f32,
    color: Color32,
    style: Lettering,
    bottom: bool,
) {
    let ctx = p.ctx();
    let family = match super::super::lettering_family(ctx, style) {
        Some(f) => f,
        None => egui::FontFamily::Proportional,
    };
    let sz = f.s(size).max(2.0);
    let widths: Vec<f32> = s
        .chars()
        .map(|ch| {
            ctx.fonts_mut(|fo| {
                fo.layout_no_wrap(ch.to_string(), egui::FontId::new(sz, family.clone()), color)
                    .size()
                    .x
            }) / f.k
                + spacing
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() - spacing;
    let deg_per = 180.0 / (std::f32::consts::PI * r);
    let mut at = -total * 0.5;
    for (ch, w) in s.chars().zip(&widths) {
        let centre = at + (w - spacing) * 0.5;
        // along the bottom of the circle, left to right means decreasing angle (clockwise from 0 up)
        let a = if bottom {
            mid - centre * deg_per
        } else {
            mid + centre * deg_per
        };
        let (x, y) = polar(cx, cy, r, a);
        let rot = if bottom { a + 180.0 } else { a };
        text_rot(p, &ch.to_string(), f.p(x, y), sz, color, style, rot.to_radians());
        at += w;
    }
}

/// A piece of print at mockup coordinates, size in mockup units (never smaller than `min` points), shrunk to fit
/// `max_w` (mockup units).
#[allow(clippy::too_many_arguments)]
pub fn print(
    p: &Painter,
    f: &Fit,
    s: &str,
    x: f32,
    y: f32,
    anchor: Align2,
    size: f32,
    color: C,
    style: Lettering,
    max_w: f32,
    min: f32,
) -> Rect {
    let caps = !matches!(style, Lettering::Script | Lettering::Brush);
    super::super::lettering(
        p,
        s,
        f.p(x, y),
        anchor,
        f.s(size).max(min),
        c32(color),
        f.s(max_w).max(4.0),
        style,
        caps,
    )
}

/// As `print`, keeping the text's case.
#[allow(clippy::too_many_arguments)]
pub fn print_as(
    p: &Painter,
    f: &Fit,
    s: &str,
    x: f32,
    y: f32,
    anchor: Align2,
    size: f32,
    color: C,
    style: Lettering,
    max_w: f32,
    min: f32,
) -> Rect {
    super::super::lettering(
        p,
        s,
        f.p(x, y),
        anchor,
        f.s(size).max(min),
        c32(color),
        f.s(max_w).max(4.0),
        style,
        false,
    )
}

/// A path from SVG path data (absolute M, L, H, V, C, Q, Z; numbers separated by spaces or commas), scaled by
/// `scale` then moved by (`dx`, `dy`).
pub fn svg_path(d: &str, dx: f32, dy: f32, scale: f32) -> Option<Path> {
    let mut b = PathBuilder::new();
    let mut nums: Vec<f32> = Vec::new();
    let mut cmd = ' ';
    let mut cur = (0.0f32, 0.0f32);
    let flush = |b: &mut PathBuilder, cmd: char, nums: &mut Vec<f32>, cur: &mut (f32, f32)| {
        let t = |x: f32, y: f32| (dx + x * scale, dy + y * scale);
        let n = nums.len();
        let mut i = 0;
        loop {
            match cmd {
                'M' | 'L' if i + 1 < n => {
                    let (x, y) = t(nums[i], nums[i + 1]);
                    if cmd == 'M' && i == 0 {
                        b.move_to(x, y);
                    } else {
                        b.line_to(x, y);
                    }
                    *cur = (nums[i], nums[i + 1]);
                    i += 2;
                }
                'H' if i < n => {
                    let (x, y) = t(nums[i], cur.1);
                    b.line_to(x, y);
                    cur.0 = nums[i];
                    i += 1;
                }
                'V' if i < n => {
                    let (x, y) = t(cur.0, nums[i]);
                    b.line_to(x, y);
                    cur.1 = nums[i];
                    i += 1;
                }
                'C' if i + 5 < n => {
                    let (x1, y1) = t(nums[i], nums[i + 1]);
                    let (x2, y2) = t(nums[i + 2], nums[i + 3]);
                    let (x, y) = t(nums[i + 4], nums[i + 5]);
                    b.cubic_to(x1, y1, x2, y2, x, y);
                    *cur = (nums[i + 4], nums[i + 5]);
                    i += 6;
                }
                'Q' if i + 3 < n => {
                    let (x1, y1) = t(nums[i], nums[i + 1]);
                    let (x, y) = t(nums[i + 2], nums[i + 3]);
                    b.quad_to(x1, y1, x, y);
                    *cur = (nums[i + 2], nums[i + 3]);
                    i += 4;
                }
                _ => break,
            }
        }
        nums.clear();
    };
    let mut tok = String::new();
    for ch in d.chars().chain(std::iter::once(' ')) {
        let sep =
            ch.is_ascii_alphabetic() || ch.is_whitespace() || ch == ',' || (ch == '-' && !tok.is_empty());
        if sep {
            if let Ok(v) = tok.parse::<f32>() {
                nums.push(v);
            }
            tok.clear();
            if ch == '-' {
                tok.push(ch);
            } else if ch.is_ascii_alphabetic() {
                flush(&mut b, cmd, &mut nums, &mut cur);
                if ch == 'Z' || ch == 'z' {
                    b.close();
                }
                cmd = ch;
            }
        } else {
            tok.push(ch);
        }
    }
    flush(&mut b, cmd, &mut nums, &mut cur);
    b.finish()
}
