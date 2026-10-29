//! Rack units and graphic EQs (PLAN.md P21.3), after the approved mockups: the studio compressor with its
//! gain-reduction meter, the four-band console EQ with coloured caps, the plate reverb's remote with its decay
//! meter (19-inch faces with their ears and screws), the five-band slider EQ in its black box, and the two
//! eight-band compacts with sliders. Built on the pedals' face engine (`pedals::face`).

use eframe::egui::{Align2, Color32, Painter, Rect, Stroke, Vec2, vec2};
use qrs_audio::board::Block;
use qrs_ui::raster::Image;

use super::pedals::compact;
use super::pedals::face::{self, Fd, Lab, LabPos, Led, Look, Pr, Spec, Sw, kn};
use super::pedals::kit::{self, C, Cv, Finish, Ks};
use super::{Detail, Hot, Layout, Lettering};

fn spec(id: &str) -> Option<Spec> {
    Some(match id {
        "studio_comp" => needle_comp(),
        "studio_eq" => console_eq(),
        "plate" => plate(),
        "eq5" => vshape5(),
        "eq8" => compact::sliders(
            "eq8",
            compact::look(0xededea, 0x2a2a2a, "Graphic 8", "GRAPHIC EQUALIZER"),
            &[
                (0, "50"),
                (1, "100"),
                (2, "200"),
                (3, "400"),
                (4, "800"),
                (5, "1.6k"),
                (6, "3.2k"),
                (7, "6.4k"),
                (8, "LVL"),
            ],
        ),
        "bass_eq8" => compact::sliders(
            "bass_eq8",
            compact::look(0xa7c4de, 0x16222e, "Bass Graphic 8", "BASS EQUALIZER"),
            &[
                (0, "30"),
                (1, "75"),
                (2, "185"),
                (3, "460"),
                (4, "1.1k"),
                (5, "2.7k"),
                (6, "6.8k"),
                (7, "16k"),
                (8, "LVL"),
            ],
        ),
        _ => return None,
    })
}

pub(super) fn layout(b: &Block, r: Rect, detail: Detail) -> Layout {
    match spec(b.def().id) {
        Some(s) => face::layout(&s, r),
        None => super::legacy_layout(b, r, detail),
    }
}

/// The body (cached per size): everything that doesn't move.
pub(super) fn body(b: &Block, r: Rect, l: &Layout, ppp: f32) -> Image {
    match spec(b.def().id) {
        Some(s) => face::body(&s, r, ppp),
        None => super::legacy_body(b, r, l, ppp),
    }
}

/// What's drawn live over the body: knobs, switches, the LED, labels.
pub(super) fn controls(p: &Painter, r: Rect, b: &Block, l: &Layout, detail: Detail, hot: Hot) {
    match spec(b.def().id) {
        Some(s) => face::controls(&s, p, r, b, detail, hot),
        None => super::legacy_controls(p, r, b, l, detail, hot),
    }
}

pub(super) fn board_size(b: &Block) -> Vec2 {
    match spec(b.def().id) {
        Some(s) => face::board_size(&s),
        None => super::legacy_board_size(b),
    }
}

// -- the 19-inch face ------------------------------------------------------------------------------------------------------

/// A rack face's width (483 mm) and a rack unit's height (44.45 mm), in the mockups' units.
const RW: f32 = 1063.0;
const U1: f32 = 97.8;

fn rack_spec(id: &'static str, units: f32, look: Look, body: fn(&mut Cv, &Spec), art: fn(&Pr)) -> Spec {
    let mut s = Spec::new(id, RW, units * U1 - 2.0, body, art);
    s.look = look;
    s.radius = 3.0;
    s.wide = true;
    s
}

/// The panel and its ears: slots and screws at each unit.
fn rack_face(cv: &mut Cv, s: &Spec, paint: &tiny_skia::Paint) {
    let (w, h) = (s.w, s.h);
    cv.rect(0.0, 3.0, w, h, 3.0, &kit::col(0, 0.5));
    let Some(p) = kit::rrect(0.0, 0.0, w, h, 3.0) else {
        return;
    };
    cv.fill(&p, paint);
    let m = cv.mask(&p);
    s.look.finish.apply(cv, &m, (0.0, 0.0, w, h), &p);
    cv.stroke(&p, &kit::col(0, 0.35), 1.0);
    cv.rect(0.0, 0.0, w, 1.0, 0.0, &kit::col(0xffffff, 0.35));
    cv.rect(0.0, h - 2.0, w, 2.0, 0.0, &kit::col(0, 0.4));
    let units = ((h + 2.0) / U1).round() as usize;
    for ex in [18.0f32, w - 18.0] {
        for k in 0..units {
            let cy = (k as f32 + 0.5) * (h / units as f32);
            cv.rect(ex - 11.0, cy - 5.0, 22.0, 10.0, 5.0, &kit::col(0x0a0a0a, 1.0));
            cv.ring(ex, cy, 8.0, &kit::chrome(ex - 9.0, cy - 9.0, 18.0, 18.0), 2.5);
            kit::screw(cv, ex, cy, 5.5, 35.0 + 20.0 * k as f32, true);
        }
    }
}

fn ticks_round(cv: &mut Cv, cx: f32, cy: f32, r0: f32, r1: f32, n: usize, c: C, w: f32) {
    for t in 0..n {
        let a = -135.0 + 270.0 * t as f32 / (n - 1).max(1) as f32;
        let (x0, y0) = kit::polar(cx, cy, r0, a);
        let (x1, y1) = kit::polar(cx, cy, r1, a);
        cv.line(x0, y0, x1, y1, &kit::col(c, 1.0), w);
    }
}

fn scale_labels(pr: &Pr, cx: f32, cy: f32, r: f32, labels: &[(f32, &str)], size: f32, c: C) {
    for &(u, t) in labels {
        let (x, y) = kit::polar(cx, cy, r, -135.0 + 270.0 * u);
        pr.t(
            t,
            x,
            y,
            Align2::CENTER_CENTER,
            size,
            c,
            Lettering::Condensed,
            40.0,
        );
    }
}

/// A moving-coil meter in a rack: face, ticks, the red part, a title (the needle is drawn live).
#[allow(clippy::too_many_arguments)]
fn meter(
    cv: &mut Cv,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    marks: &[(f32, bool)],
    red: Option<f32>,
    face: (C, C),
    bezel: C,
) {
    cv.rect(x - 5.0, y - 5.0, w + 10.0, h + 10.0, 8.0, &kit::col(bezel, 1.0));
    cv.rect(
        x,
        y,
        w,
        h,
        6.0,
        &kit::rad(
            x + w / 2.0,
            y + h * 0.85,
            w * 0.9,
            h * 0.9,
            &[(0.0, face.0, 1.0), (1.0, face.1, 1.0)],
        ),
    );
    let (px, py, r) = (x + w / 2.0, y + h * 1.18, h * 0.98);
    let pt = |fr: f32, rr: f32| {
        let a = (-47.0 + 94.0 * fr).to_radians();
        (px + a.sin() * rr, py - a.cos() * rr)
    };
    let arc: Vec<(f32, f32)> = (0..=30).map(|i| pt(i as f32 / 30.0, r * 0.78)).collect();
    if let Some(p) = qrs_ui::raster::polyline(&arc) {
        cv.stroke(&p, &kit::col(0x1d1a14, 1.0), 1.6);
    }
    if let Some(rf) = red {
        let arc: Vec<(f32, f32)> = (0..=12)
            .map(|i| pt(rf + (1.0 - rf) * i as f32 / 12.0, r * 0.8))
            .collect();
        if let Some(p) = qrs_ui::raster::polyline(&arc) {
            cv.stroke(&p, &kit::col(0xc4271c, 1.0), 5.0);
        }
    }
    for &(fr, major) in marks {
        let (xa, ya) = pt(fr, r * 0.78);
        let (xb, yb) = pt(fr, r * if major { 0.88 } else { 0.84 });
        let c = if red.is_some_and(|rf| fr > rf + 1e-3) {
            0xc4271c
        } else {
            0x1d1a14
        };
        cv.line(xa, ya, xb, yb, &kit::col(c, 1.0), if major { 1.6 } else { 1.0 });
    }
    cv.rect(
        x,
        y + h * 0.86,
        w,
        h * 0.14,
        0.0,
        &kit::lin(
            0.0,
            y + h * 0.86,
            0.0,
            y + h,
            &[(0.0, 0x2a2a2a, 1.0), (1.0, 0x0e0e0e, 1.0)],
        ),
    );
}

#[allow(clippy::too_many_arguments)]
fn meter_live(
    pr: &Pr,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fr: f32,
    labels: &[(f32, &str)],
    title: &str,
    sub: &str,
    style: Lettering,
) {
    let (px, py, r) = (x + w / 2.0, y + h * 1.18, h * 0.98);
    for &(f, t) in labels {
        let a = (-47.0 + 94.0 * f).to_radians();
        pr.t(
            t,
            px + a.sin() * r * 0.97,
            py - a.cos() * r * 0.97,
            Align2::CENTER_CENTER,
            h * 0.09,
            0x1d1a14,
            Lettering::Condensed,
            30.0,
        );
    }
    pr.t(
        title,
        px,
        y + h * 0.62,
        Align2::CENTER_CENTER,
        h * 0.15,
        0x1d1a14,
        style,
        w * 0.8,
    );
    if !sub.is_empty() {
        pr.t(
            sub,
            px,
            y + h * 0.76,
            Align2::CENTER_CENTER,
            h * 0.075,
            0x3a3220,
            Lettering::Condensed,
            w * 0.8,
        );
    }
    let a = (-47.0 + 94.0 * fr.clamp(0.0, 1.0)).to_radians();
    let t0 = (py - (y + h * 0.86)) / a.cos().max(0.2);
    let p0 = (px + a.sin() * t0, py - a.cos() * t0);
    let p1 = (px + a.sin() * r * 0.95, py - a.cos() * r * 0.95);
    let mid = (p0.0 + (p1.0 - p0.0) * 0.78, p0.1 + (p1.1 - p0.1) * 0.78);
    let wpx = pr.f.s(1.8).max(1.0);
    pr.p.line_segment(
        [pr.f.p(p0.0, p0.1), pr.f.p(mid.0, mid.1)],
        Stroke::new(wpx, Color32::from_rgb(21, 21, 21)),
    );
    pr.p.line_segment(
        [pr.f.p(mid.0, mid.1), pr.f.p(p1.0, p1.1)],
        Stroke::new(wpx, Color32::from_rgb(209, 32, 26)),
    );
    kit::vgrad(
        pr.p,
        pr.f.rect(x, y, w, h * 0.45),
        Color32::from_white_alpha(60),
        Color32::from_white_alpha(0),
    );
}

// -- the compressor ------------------------------------------------------------------------------------------------------------------

const NINK: C = 0x1d1f22;

fn needle_comp() -> Spec {
    let look = Look {
        c: 0x9aa1a9,
        ink: NINK,
        finish: Finish::Brushed,
        ..Default::default()
    };
    let mut s = rack_spec("studio_comp", 2.0, look, comp_body, comp_art);
    let big = |x: f32| Lab::new(LabPos::At(x, 20.0), 10.0, NINK);
    let small = Lab::new(LabPos::Right, 9.5, NINK).gap(10.0);
    s.knobs = vec![
        kn(0, 318.0, 104.0, 42.0, Ks::Alu, big(318.0)),
        kn(2, 446.0, 62.0, 20.0, Ks::Alu, small),
        kn(3, 446.0, 142.0, 20.0, Ks::Alu, small),
        kn(
            1,
            866.0,
            104.0,
            20.0,
            Ks::Rack,
            Lab::new(LabPos::At(866.0, 64.0), 9.5, NINK),
        ),
        kn(4, 966.0, 104.0, 42.0, Ks::Alu, big(966.0)),
    ];
    s.led = Some(Led {
        x: 72.0,
        y: 146.0,
        r: 8.0,
        c: 0xe2231a,
        jewel: true,
        bezel: true,
    });
    s.switch = Sw::Toggle(140.0, 150.0, 7.0);
    s
}

fn comp_body(cv: &mut Cv, s: &Spec) {
    rack_face(
        cv,
        s,
        &kit::lin(
            0.0,
            0.0,
            0.0,
            s.h,
            &[(0.0, 0xaab0b7, 1.0), (0.45, 0x9aa1a9, 1.0), (1.0, 0x8b929a, 1.0)],
        ),
    );
    cv.rect(56.0, 92.0, 150.0, 2.0, 0.0, &kit::col(NINK, 0.6));
    ticks_round(cv, 318.0, 104.0, 48.0, 55.0, 11, NINK, 1.3);
    ticks_round(cv, 966.0, 104.0, 48.0, 55.0, 11, NINK, 1.3);
    ticks_round(cv, 446.0, 62.0, 24.0, 28.0, 7, NINK, 1.1);
    ticks_round(cv, 446.0, 142.0, 24.0, 28.0, 7, NINK, 1.1);
    ticks_round(cv, 866.0, 104.0, 24.0, 28.0, 7, NINK, 1.1);
    let marks: Vec<(f32, bool)> = [0.0, 0.2, 0.36, 0.5, 0.64, 0.74, 0.84, 0.93]
        .iter()
        .zip([true, true, false, true, false, false, false, true])
        .map(|(&a, b)| (a, b))
        .collect();
    meter(
        cv,
        586.0,
        30.0,
        236.0,
        134.0,
        &marks,
        None,
        (0xfbf3dc, 0xe3d3a8),
        0x141518,
    );
}

fn comp_art(pr: &Pr) {
    pr.t(
        "NEEDLE COMP",
        56.0,
        40.0,
        Align2::LEFT_CENTER,
        22.0,
        NINK,
        Lettering::Condensed,
        200.0,
    );
    pr.t(
        "STUDIO COMPRESSOR \u{b7} NC-1",
        58.0,
        68.0,
        Align2::LEFT_CENTER,
        8.5,
        0x3a3f45,
        Lettering::Condensed,
        200.0,
    );
    pr.ta(
        "quickRS",
        160.0,
        118.0,
        Align2::CENTER_CENTER,
        9.0,
        0x2d3034,
        Lettering::Heavy,
        80.0,
    );
    pr.t(
        "IN",
        140.0,
        128.0,
        Align2::CENTER_CENTER,
        7.0,
        NINK,
        Lettering::Condensed,
        20.0,
    );
    pr.t(
        "OUT",
        140.0,
        176.0,
        Align2::CENTER_CENTER,
        7.0,
        NINK,
        Lettering::Condensed,
        20.0,
    );
    pr.t(
        "POWER",
        72.0,
        172.0,
        Align2::CENTER_CENTER,
        7.0,
        NINK,
        Lettering::Condensed,
        40.0,
    );
    scale_labels(
        pr,
        318.0,
        104.0,
        64.0,
        &[(0.0, "-40"), (0.5, "-20"), (1.0, "0")],
        8.0,
        NINK,
    );
    scale_labels(
        pr,
        966.0,
        104.0,
        64.0,
        &[(0.0, "0"), (0.5, "12"), (1.0, "24")],
        8.0,
        NINK,
    );
    // the gain-reduction needle: more with a lower threshold and a higher ratio
    let def = pr.b.def();
    let thr = pr.b.v[0];
    let ratio = pr.b.v[1].max(1.0);
    let gr = ((-thr) / 40.0 * (1.0 - 1.0 / ratio)).clamp(0.0, 1.0);
    let fr = if pr.on() { 0.93 - 0.8 * gr } else { 0.93 };
    let labels = [
        (0.0, "-20"),
        (0.2, "-10"),
        (0.36, "-7"),
        (0.5, "-5"),
        (0.64, "-3"),
        (0.74, "-2"),
        (0.84, "-1"),
        (0.93, "0"),
    ];
    meter_live(
        pr,
        586.0,
        30.0,
        236.0,
        134.0,
        fr,
        &labels,
        "GAIN REDUCTION",
        "dB",
        Lettering::Condensed,
    );
    let _ = def;
}

// -- the console EQ -------------------------------------------------------------------------------------------------------------------

const EINK: C = 0x1e2124;
const BANDS: [(&str, C, &str, &str); 4] = [
    ("BASS", 0x3b2f28, "50", "500"),
    ("LO-MID", 0x2f66b3, "100", "2k"),
    ("HI-MID", 0x3c9a4e, "500", "8k"),
    ("TREBLE", 0xc8323a, "1.5k", "16k"),
];

fn console_eq() -> Spec {
    let look = Look {
        c: 0x9aa1a9,
        ink: EINK,
        finish: Finish::Brushed,
        ..Default::default()
    };
    let mut s = rack_spec("studio_eq", 2.0, look, eq_body, eq_art);
    for (i, (_, cap, _, _)) in BANDS.iter().enumerate() {
        let bx = 252.0 + i as f32 * 192.0;
        s.knobs.push(kn(
            i * 2,
            bx + 54.0,
            112.0,
            29.0,
            Ks::Ssl(*cap),
            Lab::new(LabPos::At(bx + 54.0, 164.0), 8.5, 0x3a3f45).text("GAIN"),
        ));
        s.knobs.push(kn(
            i * 2 + 1,
            bx + 138.0,
            112.0,
            20.0,
            Ks::Ssl(*cap),
            Lab::new(LabPos::At(bx + 138.0, 164.0), 8.5, 0x3a3f45).text("FREQ"),
        ));
    }
    s.led = Some(Led {
        x: 124.0,
        y: 119.0,
        r: 4.5,
        c: 0x5df08a,
        jewel: false,
        bezel: true,
    });
    s.switch = Sw::Key(58.0, 104.0, 46.0, 30.0, "IN");
    s
}

fn eq_body(cv: &mut Cv, s: &Spec) {
    rack_face(
        cv,
        s,
        &kit::lin(
            0.0,
            0.0,
            0.0,
            s.h,
            &[(0.0, 0xaab0b7, 1.0), (0.45, 0x9aa1a9, 1.0), (1.0, 0x8b929a, 1.0)],
        ),
    );
    for (i, (_, cap, _, _)) in BANDS.iter().enumerate() {
        let bx = 252.0 + i as f32 * 192.0;
        cv.rect(bx - 8.0, 16.0, 1.0, s.h - 32.0, 0.0, &kit::col(0, 0.35));
        cv.rect(bx - 7.0, 16.0, 1.0, s.h - 32.0, 0.0, &kit::col(0xffffff, 0.35));
        cv.rect(bx + 4.0, 18.0, 168.0, 5.0, 2.0, &kit::col(*cap, 1.0));
        ticks_round(cv, bx + 54.0, 112.0, 34.0, 40.0, 11, EINK, 1.2);
        ticks_round(cv, bx + 138.0, 112.0, 25.0, 30.0, 9, EINK, 1.1);
    }
}

fn eq_art(pr: &Pr) {
    pr.t(
        "CONSOLE EQ",
        56.0,
        42.0,
        Align2::LEFT_CENTER,
        22.0,
        EINK,
        Lettering::Condensed,
        180.0,
    );
    pr.t(
        "FOUR-BAND PARAMETRIC",
        58.0,
        70.0,
        Align2::LEFT_CENTER,
        9.0,
        0x3a3f45,
        Lettering::Condensed,
        180.0,
    );
    pr.plain(
        "after the cab, before the room",
        56.0,
        156.0,
        Align2::LEFT_CENTER,
        9.0,
        0x3a3f45,
        700,
    );
    for (i, (name, _, lo, hi)) in BANDS.iter().enumerate() {
        let bx = 252.0 + i as f32 * 192.0;
        pr.t(
            name,
            bx + 6.0,
            36.0,
            Align2::LEFT_CENTER,
            11.0,
            EINK,
            Lettering::Condensed,
            120.0,
        );
        scale_labels(
            pr,
            bx + 54.0,
            112.0,
            49.0,
            &[(0.0, "-12"), (0.5, "0"), (1.0, "+12")],
            8.0,
            EINK,
        );
        scale_labels(pr, bx + 138.0, 112.0, 38.0, &[(0.0, lo), (1.0, hi)], 7.5, EINK);
    }
}

// -- the plate reverb ----------------------------------------------------------------------------------------------------------------

const PINK: C = 0x1d211d;

fn plate() -> Spec {
    let look = Look {
        c: 0x7a857a,
        ink: PINK,
        finish: Finish::Hammer,
        ..Default::default()
    };
    let mut s = rack_spec("plate", 3.0, look, plate_body, plate_art);
    s.knobs = vec![
        kn(
            0,
            726.0,
            136.0,
            52.0,
            Ks::Chicken(false),
            Lab::new(LabPos::At(726.0, 30.0), 11.0, PINK),
        ),
        kn(
            1,
            846.0,
            136.0,
            23.0,
            Ks::Chicken(false),
            Lab::new(LabPos::At(846.0, 97.0), 9.0, PINK),
        ),
        kn(
            2,
            916.0,
            136.0,
            23.0,
            Ks::Chicken(false),
            Lab::new(LabPos::At(916.0, 97.0), 9.0, PINK),
        ),
        kn(
            3,
            986.0,
            136.0,
            23.0,
            Ks::Chicken(false),
            Lab::new(LabPos::At(986.0, 97.0), 9.0, PINK),
        ),
    ];
    s.led = Some(Led {
        x: 916.0,
        y: 220.0,
        r: 4.5,
        c: 0xffc94a,
        jewel: false,
        bezel: true,
    });
    s.switch = Sw::Area(896.0, 206.0, 40.0, 42.0);
    s
}

fn plate_body(cv: &mut Cv, s: &Spec) {
    rack_face(
        cv,
        s,
        &kit::lin(
            0.0,
            0.0,
            0.0,
            s.h,
            &[(0.0, 0x87917f, 1.0), (0.45, 0x7a857a, 1.0), (1.0, 0x6b766b, 1.0)],
        ),
    );
    // the plate itself: a steel sheet, its driver and two pickups
    cv.rect(
        88.0,
        108.0,
        200.0,
        120.0,
        4.0,
        &kit::lin(
            0.0,
            108.0,
            0.0,
            228.0,
            &[(0.0, 0x5e685e, 1.0), (1.0, 0x525b52, 1.0)],
        ),
    );
    cv.rect(104.0, 122.0, 168.0, 96.0, 0.0, &kit::col(0, 0.4));
    cv.rect(
        104.0,
        120.0,
        168.0,
        96.0,
        0.0,
        &kit::lin(
            104.0,
            120.0,
            272.0,
            216.0,
            &[
                (0.0, 0xeef1f3, 1.0),
                (0.45, 0x9aa1a7, 1.0),
                (0.7, 0xd5dade, 1.0),
                (1.0, 0x8b9298, 1.0),
            ],
        ),
    );
    for (cx, cy, d, c) in [
        (150.0, 168.0, 14.0, 0xc8323a),
        (222.0, 140.0, 10.0, PINK),
        (240.0, 196.0, 10.0, PINK),
    ] {
        cv.circle(cx, cy, d / 2.0 + 2.0, &kit::col(0xffffff, 0.5));
        cv.circle(cx, cy, d / 2.0, &kit::col(c, 1.0));
    }
    for (x, y) in [(106.0, 122.0), (270.0, 122.0), (106.0, 214.0), (270.0, 214.0)] {
        cv.circle(x, y, 2.0, &kit::col(0x2a2a2a, 1.0));
    }
    // the decay meter, in seconds
    let marks: Vec<(f32, bool)> = (0..9).map(|i| (i as f32 / 8.0, i % 2 == 0)).collect();
    meter(
        cv,
        334.0,
        40.0,
        300.0,
        176.0,
        &marks,
        None,
        (0xf4ecd2, 0xd9cba4),
        PINK,
    );
    // the dials' scales
    ticks_round(cv, 726.0, 136.0, 56.0, 64.0, 9, PINK, 1.6);
    for x in [846.0, 916.0, 986.0] {
        ticks_round(cv, x, 136.0, 26.0, 31.0, 7, PINK, 1.2);
    }
}

fn plate_art(pr: &Pr) {
    pr.t(
        "PLATE 140",
        84.0,
        46.0,
        Align2::LEFT_CENTER,
        36.0,
        PINK,
        Lettering::Condensed,
        240.0,
    );
    pr.t(
        "ECHO PLATE \u{b7} REMOTE CONTROL",
        88.0,
        80.0,
        Align2::LEFT_CENTER,
        9.5,
        0x2e342e,
        Lettering::Condensed,
        240.0,
    );
    pr.t(
        "DRIVER",
        92.0,
        242.0,
        Align2::LEFT_CENTER,
        8.0,
        0x2e342e,
        Lettering::Condensed,
        60.0,
    );
    pr.t(
        "PICKUPS",
        216.0,
        242.0,
        Align2::LEFT_CENTER,
        8.0,
        0x2e342e,
        Lettering::Condensed,
        60.0,
    );
    // the decay meter reads the Time knob in seconds (0.5 to 6 s across the arc)
    let def = pr.b.def();
    let secs = pr.b.v[0];
    let fr = ((secs - 0.5) / 5.5).clamp(0.0, 1.0);
    let labels: Vec<(f32, String)> = [0.5f32, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
        .iter()
        .map(|s| ((s - 0.5) / 5.5, format!("{}", s)))
        .collect();
    let lr: Vec<(f32, &str)> = labels.iter().map(|(f, s)| (*f, s.as_str())).collect();
    meter_live(
        pr,
        334.0,
        40.0,
        300.0,
        176.0,
        fr,
        &lr,
        "DECAY",
        "SECONDS",
        Lettering::Condensed,
    );
    pr.t(
        "DECAY TIME FOLLOWS THE TIME DIAL",
        484.0,
        244.0,
        Align2::CENTER_CENTER,
        8.5,
        0x2e342e,
        Lettering::Condensed,
        300.0,
    );
    let _ = def;
    pr.t(
        "ON",
        916.0,
        238.0,
        Align2::CENTER_CENTER,
        8.0,
        PINK,
        Lettering::Condensed,
        30.0,
    );
}

// -- the five-band slider EQ ------------------------------------------------------------------------------------------------------

const VW: f32 = 374.0;
const VH: f32 = 264.0;
const VINK: C = 0xecebe6;

fn vshape5() -> Spec {
    let look = Look {
        c: 0x141414,
        ink: VINK,
        finish: Finish::Satin,
        ..Default::default()
    };
    let mut s = Spec::new("eq5", VW, VH, vshape_body, vshape_art);
    s.look = look;
    s.radius = 7.0;
    s.ext = [8.0, 0.0, 8.0, 0.0];
    let xs = [62.0f32, 112.0, 162.0, 212.0, 262.0, 326.0];
    s.faders = xs
        .iter()
        .enumerate()
        .map(|(i, &x)| Fd {
            i,
            x,
            y0: 34.0,
            y1: 168.0,
            cap: vec2(26.0, 16.0),
            chrome: true,
            lab: Lab::new(LabPos::At(x, 184.0), 9.0, VINK),
        })
        .collect();
    s.led = Some(Led {
        x: 232.0,
        y: 228.0,
        r: 3.5,
        c: 0xff3b30,
        jewel: false,
        bezel: true,
    });
    s.switch = Sw::Stomp(300.0, 226.0, 18.0);
    s
}

fn vshape_body(cv: &mut Cv, s: &Spec) {
    let (w, h) = (VW, VH);
    kit::jacks(cv, w, 0.3 * h, 18.0);
    if let Some(p) = kit::rrect(0.0, 4.0, w, h, 7.0) {
        cv.fill(&p, &kit::col(0x0a0a0a, 1.0));
    }
    if let Some(p) = kit::rrect(0.0, 0.0, w, h, 7.0) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                0.0,
                0.0,
                h,
                &[(0.0, 0x2e2e2e, 1.0), (0.26, 0x141414, 1.0), (1.0, 0x0e0e0e, 1.0)],
            ),
        );
        cv.stroke(&p, &kit::col(0xffffff, 0.15), 1.0);
    }
    for (i, (x, y)) in [
        (12.0, 12.0),
        (w - 12.0, 12.0),
        (12.0, h - 12.0),
        (w - 12.0, h - 12.0),
    ]
    .into_iter()
    .enumerate()
    {
        kit::screw(cv, x, y, 4.5, 70.0 + 10.0 * i as f32, true);
    }
    let (top, bot) = (34.0, 168.0);
    for k in 0..9 {
        let y = top + (bot - top) * k as f32 / 8.0;
        cv.rect(
            44.0,
            y,
            240.0,
            1.0,
            0.0,
            &kit::col(VINK, if k == 4 { 0.38 } else { 0.12 }),
        );
    }
    cv.rect(
        297.0,
        top - 8.0,
        1.0,
        bot - top + 16.0,
        0.0,
        &kit::col(VINK, 0.18),
    );
    for fd in &s.faders {
        cv.rect(
            fd.x - 3.0,
            top - 6.0,
            6.0,
            bot - top + 12.0,
            3.0,
            &kit::col(0, 1.0),
        );
        cv.rect(fd.x - 3.0, bot + 6.0, 6.0, 1.0, 0.0, &kit::col(0xffffff, 0.12));
    }
}

fn vshape_art(pr: &Pr) {
    let s = pr.spec;
    pr.t(
        "+15",
        22.0,
        34.0,
        Align2::LEFT_CENTER,
        8.0,
        0xa7a6a0,
        Lettering::Condensed,
        20.0,
    );
    pr.t(
        "0",
        28.0,
        101.0,
        Align2::LEFT_CENTER,
        8.0,
        0xa7a6a0,
        Lettering::Condensed,
        20.0,
    );
    pr.t(
        "-15",
        22.0,
        168.0,
        Align2::LEFT_CENTER,
        8.0,
        0xa7a6a0,
        Lettering::Condensed,
        20.0,
    );
    // the curve the caps draw
    let def = pr.b.def();
    let pts: Vec<_> = s
        .faders
        .iter()
        .take(5)
        .map(|fd| {
            let u = def.knobs[fd.i].to_u(pr.b.v[fd.i]);
            pr.f.p(fd.x, fd.y1 - (fd.y1 - fd.y0) * u)
        })
        .collect();
    pr.p.add(eframe::egui::Shape::line(
        pts,
        Stroke::new(
            pr.f.s(2.0).max(1.0),
            Color32::from_rgba_unmultiplied(242, 181, 60, 140),
        ),
    ));
    pr.t(
        "V-SHAPE 5",
        22.0,
        224.0,
        Align2::LEFT_CENTER,
        20.0,
        VINK,
        Lettering::Condensed,
        160.0,
    );
    pr.t(
        "FIVE-BAND GRAPHIC",
        24.0,
        244.0,
        Align2::LEFT_CENTER,
        8.5,
        0xa7a6a0,
        Lettering::Condensed,
        160.0,
    );
}
