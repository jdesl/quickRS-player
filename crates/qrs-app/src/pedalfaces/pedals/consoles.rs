//! The tabletop units: the tape echo with its VU meter and tape window, the photo-cell vibe console, the rotary
//! speaker cabinet and the spring reverb box in brown tolex. After the effects mockups' `space_tape`,
//! `machine_vibe`, `rotary122` and `spring63`. None has a footswitch: the pilot lamp (or the badge) turns them on.

use eframe::egui::{Align2, Color32, Pos2, Stroke};

use qrs_audio::board::catalog::{HEAD_BITS, HEADS};

use super::super::Lettering;
use super::face::{ChK, Chs, Lab, LabPos, Led, Look, Pr, Spec, Sw, kn};
use super::kit::{self, C, Cv, Finish, Ks};

pub fn of(id: &str) -> Option<Spec> {
    Some(match id {
        "tape_echo" => space_tape(),
        "vibe" => machine_vibe(),
        "rotary" => rotary(),
        "spring" => spring(),
        _ => return None,
    })
}

fn look(c: C, ink: C) -> Look {
    Look {
        c,
        ink,
        finish: Finish::None,
        ..Default::default()
    }
}

/// A moving-coil meter's face: an arc of ticks from -47 to 47 degrees, the red part from `red`, a title.
#[allow(clippy::too_many_arguments)]
fn meter_face(
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
    cv.rect(x - 6.0, y - 6.0, w + 12.0, 1.0, 0.0, &kit::col(0xffffff, 0.12));
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
    // the pivot's cover
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

/// A meter's needle at `fr` (0..1) and its glass.
fn meter_needle(pr: &Pr, x: f32, y: f32, w: f32, h: f32, fr: f32) {
    let (px, py, r) = (x + w / 2.0, y + h * 1.18, h * 0.98);
    let a = (-47.0 + 94.0 * fr.clamp(0.0, 1.0)).to_radians();
    // the needle from where it shows above the pivot cover to its tip
    let t0 = (py - (y + h * 0.86)) / a.cos().max(0.2);
    let p0 = (px + a.sin() * t0, py - a.cos() * t0);
    let p1 = (px + a.sin() * r * 0.95, py - a.cos() * r * 0.95);
    let mid = (p0.0 + (p1.0 - p0.0) * 0.78, p0.1 + (p1.1 - p0.1) * 0.78);
    let wpx = pr.f.s(1.6).max(1.0);
    pr.p.line_segment(
        [pr.f.p(p0.0, p0.1), pr.f.p(mid.0, mid.1)],
        Stroke::new(wpx, Color32::from_rgb(21, 21, 21)),
    );
    pr.p.line_segment(
        [pr.f.p(mid.0, mid.1), pr.f.p(p1.0, p1.1)],
        Stroke::new(wpx, Color32::from_rgb(209, 32, 26)),
    );
    // glass
    let g = pr.f.rect(x, y, w, h * 0.45);
    kit::vgrad(
        pr.p,
        g,
        Color32::from_white_alpha(60),
        Color32::from_white_alpha(0),
    );
}

/// Numbers round a meter's arc.
fn meter_labels(pr: &Pr, x: f32, y: f32, w: f32, h: f32, labels: &[(f32, &str)], red: Option<f32>) {
    let (px, py, r) = (x + w / 2.0, y + h * 1.18, h * 0.98);
    for &(fr, t) in labels {
        let a = (-47.0 + 94.0 * fr).to_radians();
        let (lx, ly) = (px + a.sin() * r * 0.97, py - a.cos() * r * 0.97);
        let c = if red.is_some_and(|rf| fr > rf + 1e-3) {
            0xc4271c
        } else {
            0x1d1a14
        };
        pr.t(
            t,
            lx,
            ly,
            Align2::CENTER_CENTER,
            h * 0.085,
            c,
            Lettering::Condensed,
            30.0,
        );
    }
}

// -- the tape echo -------------------------------------------------------------------------------------------------------------

const TW: f32 = 1140.0;
const LID: f32 = 34.0;
const TFH: f32 = 420.0;
const TH: f32 = LID + TFH;
const TPX: f32 = 46.0;
const TPY: f32 = LID + 18.0;
const TPW: f32 = TW - 92.0;
const TPH: f32 = TFH - 36.0;
const TINK: C = 0xf1efe6;

fn space_tape() -> Spec {
    let mut s = Spec::new("tape_echo", TW, TH, tape_body, tape_art);
    s.look = look(0x3b4a3c, TINK);
    s.radius = 14.0;
    s.wide = true;
    let o = |a: f32, b: f32| (TPX + a, TPY + b);
    for (j, i) in [0usize, 1, 2, 3].into_iter().enumerate() {
        let (x, y) = o(356.0 + 110.0 * j as f32, 236.0);
        s.knobs.push(kn(
            i,
            x,
            y,
            33.0,
            Ks::SSkirt,
            Lab::new(LabPos::Above, 11.0, TINK).gap(22.0),
        ));
    }
    // the head selector: which playback heads play (`heads`, 1 .. 1+2+3) over its printed scale
    let (hx, hy) = o(900.0, 196.0);
    s.knobs.push(kn(4, hx, hy, 60.0, Ks::SSkirt, Lab::none()));
    let (jx, jy) = o(640.0, 330.0);
    s.led = Some(Led {
        x: jx,
        y: jy,
        r: 8.0,
        c: 0xe2231a,
        jewel: true,
        bezel: true,
    });
    s.switch = Sw::Area(jx - 16.0, jy - 16.0, 60.0, 32.0);
    s
}

fn tape_body(cv: &mut Cv, s: &Spec) {
    let (w, case) = (TW, s.look.c);
    // the lid seen from the front, and its handle slot
    if let Some(p) = kit::poly(&[
        (w * 0.022, 0.0),
        (w * 0.978, 0.0),
        (w, LID + 6.0),
        (0.0, LID + 6.0),
    ]) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                0.0,
                0.0,
                LID + 6.0,
                &[(0.0, 0x56665a, 1.0), (0.6, 0x45554a, 1.0), (1.0, 0x3a483e, 1.0)],
            ),
        );
    }
    cv.rect(w * 0.03, 8.0, w * 0.94, 2.0, 0.0, &kit::col(0, 0.35));
    cv.rect(
        w / 2.0 - 120.0,
        14.0,
        240.0,
        12.0,
        6.0,
        &kit::lin(
            0.0,
            14.0,
            0.0,
            26.0,
            &[(0.0, 0x2a332c, 1.0), (1.0, 0x1a201b, 1.0)],
        ),
    );
    // the case
    if let Some(p) = kit::rrect4(0.0, LID, w, TFH, [6.0, 6.0, 16.0, 16.0]) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                LID,
                0.0,
                TH,
                &[
                    (0.0, kit::lt(case, 0.1), 1.0),
                    (0.3, case, 1.0),
                    (1.0, kit::dk(case, 0.15), 1.0),
                ],
            ),
        );
        let m = cv.mask(&p);
        kit::cloth(cv, &m, (0.0, LID, w, TFH), 0x000000, 0.05, 4.0);
        cv.stroke(&p, &kit::col(0, 0.4), 1.0);
    }
    for sx in [0.0, w - 26.0] {
        cv.rect(
            sx,
            LID,
            26.0,
            TFH,
            6.0,
            &kit::lin(
                sx,
                0.0,
                sx + 26.0,
                0.0,
                &[(0.0, 0x1d241e, 1.0), (0.5, 0x2b352d, 1.0), (1.0, 0x1d241e, 1.0)],
            ),
        );
    }
    // the front panel
    cv.rect(
        TPX - 2.0,
        TPY - 2.0,
        TPW + 4.0,
        TPH + 4.0,
        7.0,
        &kit::col(0x0d0e0c, 1.0),
    );
    cv.rect(
        TPX,
        TPY,
        TPW,
        TPH,
        6.0,
        &kit::lin(
            0.0,
            TPY,
            0.0,
            TPY + TPH,
            &[(0.0, 0x25261f, 1.0), (0.4, 0x1e1f1c, 1.0), (1.0, 0x161714, 1.0)],
        ),
    );
    for (a, b) in [
        (14.0, 14.0),
        (TPW - 14.0, 14.0),
        (14.0, TPH - 14.0),
        (TPW - 14.0, TPH - 14.0),
    ] {
        kit::screw(cv, TPX + a, TPY + b, 5.0, 40.0, true);
    }
    let o = |a: f32, b: f32| (TPX + a, TPY + b);
    // the VU meter
    let (vx, vy) = o(34.0, 34.0);
    let marks: Vec<(f32, bool)> = [0.0, 0.18, 0.32, 0.45, 0.56, 0.66, 0.74, 0.82, 0.91, 1.0]
        .iter()
        .zip([true, true, true, false, true, false, false, true, false, true])
        .map(|(&a, b)| (a, b))
        .collect();
    meter_face(
        cv,
        vx,
        vy,
        236.0,
        132.0,
        &marks,
        Some(0.82),
        (0xf6e7a8, 0xd8bd64),
        0x0c0c0c,
    );
    // the tape window
    let (tx, ty) = o(34.0, 206.0);
    let (tw, th) = (236.0, 104.0);
    cv.rect(
        tx - 5.0,
        ty - 5.0,
        tw + 10.0,
        th + 10.0,
        8.0,
        &kit::lin(
            0.0,
            ty,
            0.0,
            ty + th,
            &[(0.0, 0x9ba29b, 1.0), (0.4, 0xdfe3df, 1.0), (1.0, 0x7c837c, 1.0)],
        ),
    );
    cv.rect(
        tx,
        ty,
        tw,
        th,
        5.0,
        &kit::rad(
            tx + tw / 2.0,
            ty + th * 0.4,
            tw * 0.6,
            th * 0.8,
            &[(0.0, 0x1d1a14, 1.0), (1.0, 0x070605, 1.0)],
        ),
    );
    for hx in [70.0f32, 110.0, 150.0] {
        cv.rect(
            tx + hx - 12.0,
            ty + 30.0,
            24.0,
            22.0,
            3.0,
            &kit::lin(
                0.0,
                ty + 30.0,
                0.0,
                ty + 52.0,
                &[(0.0, 0xe9ecee, 1.0), (1.0, 0x8d9399, 1.0)],
            ),
        );
    }
    // the tape
    let mut x = tx;
    let mut k = 0;
    while x < tx + tw {
        let c = [0x5a3418, 0x4a2a12, 0x6e4422][k % 3];
        cv.rect(
            x,
            ty + 52.0,
            7.0f32.min(tx + tw - x),
            13.0,
            0.0,
            &kit::col(c, 1.0),
        );
        x += 7.0;
        k += 1;
    }
    cv.circle(
        tx + 206.0,
        ty + 58.0,
        14.0,
        &kit::dome(tx + 206.0, ty + 58.0, 14.0),
    );
    cv.circle(tx + 36.0, ty + 58.0, 16.0, &kit::col(0x2a2a2a, 1.0));
    cv.circle(
        tx + 36.0,
        ty + 58.0,
        14.0,
        &kit::rad(
            tx + 33.0,
            ty + 55.0,
            14.0,
            14.0,
            &[(0.0, 0x3a3a3a, 1.0), (1.0, 0x0b0b0b, 1.0)],
        ),
    );
    cv.circle(tx + 36.0, ty + 58.0, 5.0, &kit::col(0x9aa0a6, 1.0));
    cv.rect(tx, ty, tw, th * 0.4, 5.0, &kit::col(0xffffff, 0.06));
    // the title's rules
    let (ttx, tty) = o(316.0, 30.0);
    cv.rect(
        ttx,
        tty + 76.0,
        440.0,
        2.0,
        0.0,
        &kit::lin(
            ttx,
            0.0,
            ttx + 440.0,
            0.0,
            &[(0.0, 0xc9ccc6, 1.0), (1.0, 0x7d817a, 1.0)],
        ),
    );
    cv.rect(ttx, tty + 81.0, 440.0, 1.0, 0.0, &kit::col(0x6d716a, 1.0));
    // the knobs' scales
    for j in 0..4 {
        let (kx, ky) = o(356.0 + 110.0 * j as f32, 236.0);
        for t in 0..11 {
            let a = -135.0 + 27.0 * t as f32;
            let (x0, y0) = kit::polar(kx, ky, 40.0, a);
            let (x1, y1) = kit::polar(kx, ky, 46.0, a);
            cv.line(x0, y0, x1, y1, &kit::col(0xd8d5c6, 1.0), 1.4);
        }
    }
    // the jacks
    for i in 0..2 {
        let (jx, jy) = o(350.0 + 120.0 * i as f32, 330.0);
        cv.circle(jx, jy, 10.0, &kit::chrome(jx - 10.0, jy - 10.0, 20.0, 20.0));
        cv.circle(jx, jy, 3.6, &kit::col(0x050505, 1.0));
    }
    // the head selector's scale (the knob itself is live: `heads`)
    let (hx, hy) = o(900.0, 196.0);
    cv.ring(hx, hy, 120.0, &kit::col(TINK, 0.14), 1.0);
    for t in 0..7 {
        let a = -135.0 + 45.0 * t as f32;
        let (x0, y0) = kit::polar(hx, hy, 68.0, a);
        let (x1, y1) = kit::polar(hx, hy, 76.0, a);
        cv.line(x0, y0, x1, y1, &kit::col(TINK, 1.0), 2.0);
    }
    // rubber feet
    for fx in [40.0, w - 90.0] {
        cv.rect(fx, TH - 4.0, 50.0, 10.0, 5.0, &kit::col(0x0b0b0b, 1.0));
    }
}

fn tape_art(pr: &Pr) {
    let o = |a: f32, b: f32| (TPX + a, TPY + b);
    let (vx, vy) = o(34.0, 34.0);
    let labels = [
        (0.0, "20"),
        (0.18, "10"),
        (0.32, "7"),
        (0.45, "5"),
        (0.56, "3"),
        (0.66, "2"),
        (0.74, "1"),
        (0.82, "0"),
        (0.91, "+1"),
        (1.0, "+3"),
    ];
    meter_labels(pr, vx, vy, 236.0, 132.0, &labels, Some(0.82));
    pr.t(
        "VU",
        vx + 118.0,
        vy + 132.0 * 0.62,
        Align2::CENTER_CENTER,
        20.0,
        0x1d1a14,
        Lettering::Condensed,
        60.0,
    );
    pr.t(
        "INPUT LEVEL",
        vx + 118.0,
        vy + 132.0 * 0.76,
        Align2::CENTER_CENTER,
        9.0,
        0x3a3220,
        Lettering::Condensed,
        100.0,
    );
    // the needle reads what goes in: up when it's on, with the mix
    let fr = if pr.on() { 0.45 + 0.25 * pr.u(2) } else { 0.0 };
    meter_needle(pr, vx, vy, 236.0, 132.0, fr);
    // the heads' lamps: the ones the selector plays
    let (tx, ty) = o(34.0, 206.0);
    let pick = pr.b.v[4].round().clamp(0.0, 6.0) as usize;
    let heads = HEAD_BITS[pick];
    for (i, hx) in [70.0f32, 110.0, 150.0].into_iter().enumerate() {
        let lit = heads & (1 << i) != 0 && pr.on();
        let c = pr.f.p(tx + hx, ty + 23.0);
        if lit {
            pr.p.circle_filled(c, pr.f.s(7.0), Color32::from_rgba_unmultiplied(80, 240, 130, 60));
        }
        pr.p.circle_filled(
            c,
            pr.f.s(3.0).max(1.0),
            if lit {
                Color32::from_rgb(93, 240, 138)
            } else {
                Color32::from_rgb(31, 58, 39)
            },
        );
        pr.t(
            &(i + 1).to_string(),
            tx + hx,
            ty + 84.0,
            Align2::CENTER_CENTER,
            9.0,
            0xc9c3a8,
            Lettering::Condensed,
            20.0,
        );
    }
    let playing = match heads.count_ones() {
        1 => format!("TAPE \u{b7} HEAD {} PLAYING", HEADS[pick]),
        _ => format!("TAPE \u{b7} HEADS {} PLAYING", HEADS[pick].replace('+', " + ")),
    };
    pr.t(
        &playing,
        tx,
        ty + 122.0,
        Align2::LEFT_CENTER,
        9.0,
        0xb9b6a2,
        Lettering::Condensed,
        240.0,
    );
    let (ttx, tty) = o(316.0, 30.0);
    pr.t(
        "SPACE TAPE",
        ttx,
        tty + 22.0,
        Align2::LEFT_CENTER,
        44.0,
        TINK,
        Lettering::Heavy,
        440.0,
    );
    pr.t(
        "TAPE ECHO \u{b7} THREE PLAYBACK HEADS \u{b7} WOW",
        ttx + 2.0,
        tty + 58.0,
        Align2::LEFT_CENTER,
        11.0,
        0xb9b6a2,
        Lettering::Condensed,
        440.0,
    );
    for j in 0..4 {
        let (kx, ky) = o(356.0 + 110.0 * j as f32, 236.0);
        for (t, a) in [("0", -135.0f32), ("5", 0.0), ("10", 135.0)] {
            if t == "5" {
                continue;
            }
            let (x, y) = kit::polar(kx, ky, 54.0, a);
            pr.t(
                t,
                x,
                y,
                Align2::CENTER_CENTER,
                8.0,
                0xb9b6a2,
                Lettering::Condensed,
                20.0,
            );
        }
    }
    for (i, t) in ["INPUT", "OUTPUT"].iter().enumerate() {
        let (jx, jy) = o(350.0 + 120.0 * i as f32, 330.0);
        pr.t(
            t,
            jx + 16.0,
            jy,
            Align2::LEFT_CENTER,
            9.0,
            0xb9b6a2,
            Lettering::Condensed,
            80.0,
        );
    }
    let (jx, jy) = o(640.0, 330.0);
    pr.t(
        "ON",
        jx + 18.0,
        jy,
        Align2::LEFT_CENTER,
        9.0,
        0xb9b6a2,
        Lettering::Condensed,
        30.0,
    );
    let (hx, hy) = o(900.0, 196.0);
    pr.t(
        "HEAD SELECT",
        hx,
        hy - 122.0,
        Align2::CENTER_CENTER,
        12.0,
        TINK,
        Lettering::Condensed,
        200.0,
    );
    for (t, lab) in HEADS.iter().enumerate() {
        let (x, y) = kit::polar(hx, hy, 96.0, -135.0 + 45.0 * t as f32);
        pr.t(
            lab,
            x,
            y,
            Align2::CENTER_CENTER,
            11.0,
            TINK,
            Lettering::Condensed,
            50.0,
        );
    }
    pr.t(
        "REPEAT HEADS",
        hx,
        hy + 120.0,
        Align2::CENTER_CENTER,
        9.0,
        0xb9b6a2,
        Lettering::Condensed,
        200.0,
    );
}

// -- the vibe console --------------------------------------------------------------------------------------------------------------

const VW: f32 = 528.0;
const VH: f32 = 363.0;
const VINK: C = 0x1d1d1f;

fn machine_vibe() -> Spec {
    let mut s = Spec::new("vibe", VW, VH, vibe_body, vibe_art);
    s.look = look(0x2a2a2c, VINK);
    s.radius = 18.0;
    s.wide = true;
    s.knobs = vec![
        kn(
            0,
            232.0,
            214.0,
            30.0,
            Ks::Chrome,
            Lab::new(LabPos::At(232.0, 290.0), 9.0, VINK).style(Lettering::Wide),
        ),
        kn(
            1,
            440.0,
            208.0,
            46.0,
            Ks::Chrome,
            Lab::new(LabPos::At(440.0, 296.0), 9.0, VINK).style(Lettering::Wide),
        ),
    ];
    s.choices = vec![Chs {
        i: 2,
        x: 276.0,
        y: 176.0,
        w: 108.0,
        h: 70.0,
        k: ChK::Toggle,
        lab: Lab::new(LabPos::At(330.0, 290.0), 9.0, 0x5a5a5c).text("MODE"),
        opts: &["CHORUS", "VIBRATO"],
        ink: VINK,
    }];
    s.led = Some(Led {
        x: VW - 40.0,
        y: 339.0,
        r: 7.0,
        c: 0xe2231a,
        jewel: true,
        bezel: true,
    });
    s.switch = Sw::Area(VW - 56.0, 323.0, 32.0, 32.0);
    s
}

fn vibe_body(cv: &mut Cv, s: &Spec) {
    let (w, h, c) = (VW, VH, s.look.c);
    if let Some(p) = kit::rrect(0.0, 6.0, w, h, 18.0) {
        cv.fill(&p, &kit::col(kit::dk(c, 0.5), 1.0));
    }
    if let Some(p) = kit::rrect(0.0, 0.0, w, h, 18.0) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                0.0,
                0.0,
                h,
                &[
                    (0.0, kit::lt(c, 0.14), 1.0),
                    (0.26, c, 1.0),
                    (0.7, c, 1.0),
                    (1.0, kit::dk(c, 0.14), 1.0),
                ],
            ),
        );
        cv.stroke(&p, &kit::col(0, 0.35), 1.0);
    }
    // the brushed panel
    let (px, py, pw, ph) = (18.0, 18.0, w - 36.0, 300.0);
    cv.rect(px, py + 2.0, pw, ph, 9.0, &kit::col(0, 0.5));
    if let Some(p) = kit::rrect(px, py, pw, ph, 9.0) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                py,
                0.0,
                py + ph,
                &[(0.0, 0xd6d7d3, 1.0), (0.5, 0xb9bab6, 1.0), (1.0, 0xa2a39f, 1.0)],
            ),
        );
        let m = cv.mask(&p);
        kit::brushed(cv, &m, (px, py, pw, ph), 17, 1.4);
    }
    for (x, y) in [
        (32.0, 32.0),
        (w - 32.0, 32.0),
        (32.0, py + ph - 14.0),
        (w - 32.0, py + ph - 14.0),
    ] {
        kit::screw(cv, x, y, 4.5, 60.0, true);
    }
    cv.rect(
        px,
        104.0,
        pw,
        7.0,
        0.0,
        &kit::lin(
            0.0,
            104.0,
            0.0,
            111.0,
            &[(0.0, 0x7d3a46, 1.0), (0.6, 0x6b2f3a, 1.0), (1.0, 0x4e2029, 1.0)],
        ),
    );
    // the lamp's window
    let (lx, ly, lr) = (110.0, 214.0, 52.0);
    cv.circle(lx, ly + 3.0, lr + 7.0, &kit::col(0, 0.4));
    cv.circle(
        lx,
        ly,
        lr + 7.0,
        &kit::chrome(lx - lr, ly - lr, 2.0 * lr, 2.0 * lr),
    );
    cv.circle(lx, ly, lr, &kit::col(0x2a1704, 1.0));
    // the intensity scale
    for t in 0..11 {
        let a = -135.0 + 27.0 * t as f32;
        let (x0, y0) = kit::polar(440.0, 208.0, 52.0, a);
        let (x1, y1) = kit::polar(440.0, 208.0, 58.0, a);
        cv.line(x0, y0, x1, y1, &kit::col(0x2a2a2c, 1.0), 1.6);
    }
    for t in 0..11 {
        let a = -135.0 + 27.0 * t as f32;
        let (x0, y0) = kit::polar(232.0, 214.0, 35.0, a);
        let (x1, y1) = kit::polar(232.0, 214.0, 40.0, a);
        cv.line(x0, y0, x1, y1, &kit::col(0x2a2a2c, 1.0), 1.2);
    }
    // the jacks on the lower strip
    for jx in [60.0f32, 150.0, 395.0] {
        cv.circle(jx, 339.0, 9.0, &kit::chrome(jx - 9.0, 330.0, 18.0, 18.0));
        cv.circle(jx, 339.0, 3.2, &kit::col(0x050505, 1.0));
    }
}

fn vibe_art(pr: &Pr) {
    pr.t(
        "MACHINE VIBE",
        48.0,
        58.0,
        Align2::LEFT_CENTER,
        25.0,
        VINK,
        Lettering::Wide,
        420.0,
    );
    pr.t(
        "PHOTO-CELL VIBRATO \u{b7} CHORUS",
        50.0,
        86.0,
        Align2::LEFT_CENTER,
        8.5,
        0x3a3a3c,
        Lettering::Wide,
        420.0,
    );
    // the lamp glows while it's on, under a grid of holes
    let (lx, ly, lr) = (110.0, 214.0, 52.0);
    if pr.on() {
        let c = pr.f.p(lx, ly);
        for (k, col) in [
            (1.0, Color32::from_rgba_unmultiplied(160, 70, 0, 120)),
            (0.7, Color32::from_rgba_unmultiplied(242, 154, 29, 200)),
            (0.42, Color32::from_rgb(255, 207, 94)),
            (0.18, Color32::from_rgb(255, 243, 200)),
        ] {
            pr.p.circle_filled(c, pr.f.s(lr * k), col);
        }
    }
    let step = 8.0;
    let mut y = ly - lr + step / 2.0;
    while y < ly + lr {
        let mut x = lx - lr + step / 2.0;
        while x < lx + lr {
            let d = ((x - lx).powi(2) + (y - ly).powi(2)).sqrt();
            if d < lr - 2.0 {
                pr.p.circle_stroke(
                    pr.f.p(x, y),
                    pr.f.s(3.6),
                    Stroke::new(pr.f.s(2.4), Color32::from_rgba_unmultiplied(20, 12, 4, 210)),
                );
            }
            x += step;
        }
        y += step;
    }
    pr.t(
        "LAMP",
        lx,
        284.0,
        Align2::CENTER_CENTER,
        9.0,
        0x2a2a2c,
        Lettering::Wide,
        80.0,
    );
    for t in 0..11 {
        let (x, y) = kit::polar(440.0, 208.0, 68.0, -135.0 + 27.0 * t as f32);
        pr.t(
            &t.to_string(),
            x,
            y,
            Align2::CENTER_CENTER,
            9.0,
            0x2a2a2c,
            Lettering::Condensed,
            20.0,
        );
    }
    for (t, lab) in [(0, "INPUT"), (1, "OUTPUT"), (2, "PEDAL")] {
        let jx = [60.0f32, 150.0, 395.0][t];
        pr.t(
            lab,
            jx + 14.0,
            339.0,
            Align2::LEFT_CENTER,
            9.0,
            0x9a9b98,
            Lettering::Wide,
            90.0,
        );
    }
    pr.ta(
        "quickRS",
        252.0,
        339.0,
        Align2::LEFT_CENTER,
        9.0,
        0xc9a35b,
        Lettering::Heavy,
        120.0,
    );
}

// -- the rotary cabinet ----------------------------------------------------------------------------------------------------------------

const KW: f32 = 320.0;
const KH: f32 = 450.0;
const GOLD: C = 0xe8cf9a;

fn rotary() -> Spec {
    let mut s = Spec::new("rotary", KW, KH, rotary_body, rotary_art);
    s.look = look(0x5c3a22, GOLD);
    s.radius = 8.0;
    s.ext = [6.0, 0.0, 6.0, 6.0];
    let lab = Lab::new(LabPos::At(0.0, 0.0), 7.0, GOLD);
    for (i, x) in [(1usize, 156.0f32), (2, 214.0), (3, 268.0)] {
        let mut l = lab;
        l.pos = LabPos::At(x, 426.0);
        s.knobs.push(kn(i, x, 403.0, 12.5, Ks::Brass, l));
    }
    s.choices = vec![Chs {
        i: 0,
        x: 28.0,
        y: 386.0,
        w: 80.0,
        h: 44.0,
        k: ChK::HalfMoon,
        lab: Lab::new(LabPos::At(68.0, 430.0), 7.0, GOLD),
        opts: &["SLOW", "FAST"],
        ink: 0xf2d48a,
    }];
    s.led = Some(Led {
        x: KW / 2.0 + 78.0,
        y: 40.0,
        r: 4.5,
        c: 0xe2231a,
        jewel: true,
        bezel: true,
    });
    s.switch = Sw::Area(KW / 2.0 - 64.0, 30.0, 156.0, 20.0);
    s
}

fn louvre(cv: &mut Cv, x: f32, y: f32, w: f32, h: f32) {
    cv.rect(
        x - 7.0,
        y - 7.0,
        w + 14.0,
        h + 14.0,
        6.0,
        &kit::lin(0.0, y, 0.0, y + h, &[(0.0, 0x3e2716, 1.0), (1.0, 0x2c1a0e, 1.0)]),
    );
    cv.rect(
        x,
        y,
        w,
        h,
        0.0,
        &kit::rad(
            x + w / 2.0,
            y + h / 2.0,
            w * 0.6,
            h * 0.6,
            &[(0.0, 0x24170d, 1.0), (1.0, 0x0b0705, 1.0)],
        ),
    );
}

fn slats(cv: &mut Cv, x: f32, y: f32, w: f32, h: f32) {
    let mut yy = y;
    while yy < y + h {
        let sh = 9.0f32.min(y + h - yy);
        cv.rect(
            x,
            yy,
            w,
            sh,
            0.0,
            &kit::lin(
                0.0,
                yy,
                0.0,
                yy + 9.0,
                &[(0.0, 0x7d5535, 1.0), (0.66, 0x5e3c23, 1.0), (1.0, 0x3e2716, 1.0)],
            ),
        );
        yy += 16.0;
    }
    cv.rect(x, y, w, 8.0, 0.0, &kit::col(0, 0.35));
}

fn rotary_body(cv: &mut Cv, _s: &Spec) {
    let (w, h) = (KW, KH);
    if let Some(p) = kit::rrect4(0.0, 0.0, w, h, [8.0, 8.0, 4.0, 4.0]) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                0.0,
                0.0,
                h,
                &[(0.0, 0x6d4729, 1.0), (0.45, 0x5c3a22, 1.0), (1.0, 0x4a2e1a, 1.0)],
            ),
        );
        let m = cv.mask(&p);
        kit::wood(cv, &m, (0.0, 0.0, w, h), 0x1e0f05);
        cv.stroke(&p, &kit::col(0, 0.4), 1.0);
    }
    // the top
    cv.rect(
        -6.0,
        0.0,
        w + 12.0,
        22.0,
        6.0,
        &kit::lin(0.0, 0.0, 0.0, 22.0, &[(0.0, 0x7a5232, 1.0), (1.0, 0x4f311c, 1.0)]),
    );
    cv.rect(-6.0, 22.0, w + 12.0, 3.0, 0.0, &kit::col(0, 0.3));
    // the badge
    cv.rect(w / 2.0 - 64.0, 31.0, 128.0, 20.0, 3.0, &kit::col(0, 0.4));
    cv.rect(
        w / 2.0 - 64.0,
        30.0,
        128.0,
        20.0,
        3.0,
        &kit::lin(
            0.0,
            30.0,
            0.0,
            50.0,
            &[(0.0, 0xf0d28a, 1.0), (0.55, 0xc99a3e, 1.0), (1.0, 0x9c7225, 1.0)],
        ),
    );
    // the horn behind the upper louvre
    louvre(cv, 36.0, 66.0, 248.0, 128.0);
    let (hx, hy) = (36.0 + 13.0, 66.0 + 35.0);
    let horn = kit::lin(
        0.0,
        hy,
        0.0,
        hy + 60.0,
        &[(0.0, 0xb59a7c, 1.0), (0.5, 0x7d6650, 1.0), (1.0, 0x3c2f24, 1.0)],
    );
    if let Some(p) = kit::svg_path("M 126 22 L 204 4 Q 214 30 204 56 L 126 38 Z", hx, hy, 1.0) {
        cv.fill(&p, &horn);
    }
    cv.ellipse(hx + 205.0, hy + 30.0, 7.0, 26.0, &kit::col(0x1a120c, 1.0));
    if let Some(p) = kit::svg_path("M 100 24 L 34 14 Q 28 30 34 46 L 100 36 Z", hx, hy, 1.0) {
        cv.fill(&p, &horn);
    }
    cv.circle(hx + 113.0, hy + 30.0, 15.0, &horn);
    cv.circle(hx + 113.0, hy + 30.0, 5.0, &kit::col(0x1a120c, 1.0));
    slats(cv, 36.0, 66.0, 248.0, 128.0);
    // the drum behind the lower one
    louvre(cv, 36.0, 214.0, 248.0, 150.0);
    for i in 0..4 {
        let x = 36.0 + 20.0 + i as f32 * 64.0;
        cv.rect(
            x,
            214.0,
            30.0,
            150.0,
            0.0,
            &kit::lin(
                x,
                0.0,
                x + 30.0,
                0.0,
                &[(0.0, 0x966e46, 0.0), (0.5, 0x966e46, 0.22), (1.0, 0x966e46, 0.0)],
            ),
        );
    }
    slats(cv, 36.0, 214.0, 248.0, 150.0);
    // the control plinth
    cv.rect(
        16.0,
        382.0,
        w - 32.0,
        54.0,
        6.0,
        &kit::lin(
            0.0,
            382.0,
            0.0,
            436.0,
            &[(0.0, 0x2e1d11, 1.0), (1.0, 0x1d120a, 1.0)],
        ),
    );
    if let Some(p) = kit::rrect(17.0, 383.0, w - 34.0, 52.0, 6.0) {
        cv.stroke(&p, &kit::col(0xb88a3a, 1.0), 1.5);
    }
    // feet
    for fx in [24.0, w - 44.0] {
        cv.rect(
            fx,
            h - 6.0,
            20.0,
            12.0,
            4.0,
            &kit::lin(
                0.0,
                h - 6.0,
                0.0,
                h + 6.0,
                &[(0.0, 0x2a2a2a, 1.0), (1.0, 0x0a0a0a, 1.0)],
            ),
        );
    }
}

fn rotary_art(pr: &Pr) {
    pr.t(
        "ROTARY 122",
        KW / 2.0,
        40.0,
        Align2::CENTER_CENTER,
        11.0,
        0x3a2410,
        Lettering::Condensed,
        120.0,
    );
    pr.t(
        "HORN",
        250.0,
        204.0,
        Align2::CENTER_CENTER,
        7.0,
        0x9b8a6a,
        Lettering::Condensed,
        40.0,
    );
    pr.t(
        "DRUM",
        250.0,
        374.0,
        Align2::CENTER_CENTER,
        7.0,
        0x9b8a6a,
        Lettering::Condensed,
        40.0,
    );
    if pr.on() {
        // the horn turning: an arrow round it, faster looking when fast
        let fast = pr.b.v[0] >= 0.5;
        let a = Color32::from_rgba_unmultiplied(242, 181, 60, if fast { 230 } else { 150 });
        let pts: Vec<Pos2> = (0..=24)
            .map(|i| {
                let t = std::f32::consts::PI * (i as f32 / 24.0);
                pr.f.p(160.0 + 138.0 * t.cos(), 132.0 + 22.0 * t.sin())
            })
            .collect();
        pr.p.add(eframe::egui::Shape::line(
            pts,
            Stroke::new(pr.f.s(2.2).max(1.0), a),
        ));
        let tip = [pr.f.p(22.0, 137.0), pr.f.p(36.0, 131.0), pr.f.p(33.0, 146.0)];
        pr.p.add(eframe::egui::Shape::convex_polygon(tip.to_vec(), a, Stroke::NONE));
    }
}

// -- the spring reverb box -------------------------------------------------------------------------------------------------------------

const PW: f32 = 1034.0;
const PH: f32 = 587.0;
const PCREAM: C = 0xf0e3c2;

fn spring() -> Spec {
    let mut s = Spec::new("spring", PW, PH, spring_body, spring_art);
    s.look = look(0x6b4a2e, PCREAM);
    s.radius = 16.0;
    s.wide = true;
    s.ext = [0.0, 28.0, 0.0, 8.0];
    let lab = |x: f32| Lab::new(LabPos::At(x, 42.0), 11.0, PCREAM);
    s.knobs = vec![
        kn(
            0,
            560.0,
            94.0,
            33.0,
            Ks::Numbered(0x3b2314, PCREAM, 0x6a4128),
            lab(560.0),
        ),
        kn(
            2,
            690.0,
            94.0,
            33.0,
            Ks::Numbered(0x3b2314, PCREAM, 0x6a4128),
            lab(690.0).text("MIXER"),
        ),
        kn(
            1,
            820.0,
            94.0,
            33.0,
            Ks::Numbered(0x3b2314, PCREAM, 0x6a4128),
            lab(820.0),
        ),
    ];
    s.led = Some(Led {
        x: 954.0,
        y: 90.0,
        r: 11.0,
        c: 0xe2231a,
        jewel: true,
        bezel: true,
    });
    s.switch = Sw::Area(934.0, 70.0, 40.0, 56.0);
    s
}

fn spring_body(cv: &mut Cv, _s: &Spec) {
    let (w, h) = (PW, PH);
    // the handle
    cv.rect(
        w / 2.0 - 120.0,
        -26.0,
        240.0,
        30.0,
        14.0,
        &kit::lin(
            0.0,
            -26.0,
            0.0,
            4.0,
            &[(0.0, 0x4a3020, 1.0), (1.0, 0x2a1a10, 1.0)],
        ),
    );
    for x in [w / 2.0 - 132.0, w / 2.0 + 108.0] {
        cv.rect(
            x,
            -8.0,
            24.0,
            14.0,
            3.0,
            &kit::lin(0.0, -8.0, 0.0, 6.0, &[(0.0, 0xeef0f2, 1.0), (1.0, 0x8d9399, 1.0)]),
        );
    }
    // the box in brown tolex
    if let Some(p) = kit::rrect(0.0, 5.0, w, h, 16.0) {
        cv.fill(&p, &kit::col(0x3a2716, 1.0));
    }
    if let Some(p) = kit::rrect(0.0, 0.0, w, h, 16.0) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                0.0,
                0.0,
                h,
                &[(0.0, 0x7a5634, 1.0), (0.4, 0x6b4a2e, 1.0), (1.0, 0x563a22, 1.0)],
            ),
        );
        let m = cv.mask(&p);
        kit::cloth(cv, &m, (0.0, 0.0, w, h), 0x000000, 0.07, 3.0);
        cv.stroke(&p, &kit::col(0, 0.25), 2.0);
    }
    // the control panel
    let (px, py, pw, ph) = (34.0, 30.0, w - 68.0, 112.0);
    cv.rect(px, py + 3.0, pw, ph, 5.0, &kit::col(0, 0.45));
    if let Some(p) = kit::rrect(px, py, pw, ph, 5.0) {
        cv.fill(
            &p,
            &kit::lin(
                0.0,
                py,
                0.0,
                py + ph,
                &[(0.0, 0x5a3d24, 1.0), (0.6, 0x46301c, 1.0), (1.0, 0x3a2716, 1.0)],
            ),
        );
        cv.stroke(&p, &kit::col(0xc9a86a, 1.0), 2.0);
    }
    // the grille in its cream welt
    let (gx, gy, gw, gh) = (34.0, 172.0, w - 68.0, h - 212.0);
    cv.rect(
        gx - 6.0,
        gy - 6.0,
        gw + 12.0,
        gh + 12.0,
        10.0,
        &kit::lin(
            0.0,
            gy,
            0.0,
            gy + gh,
            &[(0.0, 0xefe2c0, 1.0), (1.0, 0xcdbb8f, 1.0)],
        ),
    );
    if let Some(p) = kit::rrect(gx, gy, gw, gh, 6.0) {
        cv.fill(
            &p,
            &kit::rad(
                gx + gw / 2.0,
                gy + gh * 0.4,
                gw * 0.6,
                gh * 0.8,
                &[(0.0, 0xd6c49c, 1.0), (0.55, 0xc8b48a, 1.0), (1.0, 0xad9a70, 1.0)],
            ),
        );
        let m = cv.mask(&p);
        kit::cloth(cv, &m, (gx, gy, gw, gh), 0x5a3c19, 0.2, 4.0);
        cv.rect(
            gx,
            gy,
            gw,
            12.0,
            6.0,
            &kit::lin(
                0.0,
                gy,
                0.0,
                gy + 12.0,
                &[(0.0, 0x281a0a, 0.4), (1.0, 0x281a0a, 0.0)],
            ),
        );
    }
    // feet
    for fx in [60.0, w - 120.0] {
        cv.rect(fx, h - 2.0, 60.0, 10.0, 6.0, &kit::col(0x0d0d0d, 1.0));
    }
}

fn spring_art(pr: &Pr) {
    pr.t(
        "\u{2019}63 SPRING",
        64.0,
        72.0,
        Align2::LEFT_CENTER,
        36.0,
        PCREAM,
        Lettering::Condensed,
        420.0,
    );
    pr.t(
        "REVERBERATION UNIT \u{b7} ONE TANK, TWO SPRINGS",
        66.0,
        110.0,
        Align2::LEFT_CENTER,
        10.5,
        0xcbb68c,
        Lettering::Condensed,
        440.0,
    );
    pr.t(
        "PILOT",
        954.0,
        120.0,
        Align2::CENTER_CENTER,
        8.5,
        0xcbb68c,
        Lettering::Condensed,
        60.0,
    );
}
