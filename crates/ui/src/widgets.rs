use std::f32::consts::PI;

use riff_dsp::param::Param;

use crate::egui::{
    Align2, Color32, FontId, Id, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind,
    Ui, pos2, vec2,
};
use crate::ui::theme::*;

pub struct L {
    pub o: Pos2,
    pub k: f32,
}

impl L {
    pub fn r(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.p(x, y), vec2(w * self.k, h * self.k))
    }

    pub fn p(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.o.x + x * self.k, self.o.y + y * self.k)
    }

    pub fn f(&self, size: f32) -> FontId {
        FontId::monospace(size * self.k)
    }

    pub fn s(&self, v: f32) -> f32 {
        v * self.k
    }
}

const DRAG_RANGE: f32 = 180.0;
const FINE: f32 = 6.0;

#[derive(Clone, Copy)]
pub struct KnobLook {
    pub body: Color32,
    pub cap: Color32,
    pub pointer: Color32,
    pub skirt: Option<Color32>,
}

pub const DARK_KNOB: KnobLook = KnobLook {
    body: KNOB,
    cap: Color32::from_rgb(0x2a, 0x2a, 0x2a),
    pointer: KNOB_CAP,
    skirt: None,
};

#[allow(clippy::too_many_arguments)]
pub fn knob(
    ui: &Ui,
    l: &L,
    id: impl std::hash::Hash + std::fmt::Debug,
    center: Pos2,
    radius: f32,
    param: &Param,
    value: f32,
    face: Color32,
) -> Option<f32> {
    knob_look(ui, l, id, center, radius, param, value, face, &DARK_KNOB)
}

#[allow(clippy::too_many_arguments)]
pub fn knob_look(
    ui: &Ui,
    l: &L,
    id: impl std::hash::Hash + std::fmt::Debug,
    center: Pos2,
    radius: f32,
    param: &Param,
    value: f32,
    face: Color32,
    look: &KnobLook,
) -> Option<f32> {
    let r = l.s(radius);
    let rect = Rect::from_center_size(center, vec2(r * 2.6, r * 2.6));
    let resp = ui.interact(rect, Id::NULL.with(id), Sense::click_and_drag());
    let mut unit = param.to_unit(value);
    let mut changed = None;
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let d = -resp.drag_delta().y / l.s(DRAG_RANGE) / if fine { FINE } else { 1.0 };
        unit = (unit + d).clamp(0.0, 1.0);
        let v = param.from_unit(unit);
        if v != value {
            changed = Some(v);
        }
    }
    if resp.double_clicked() {
        changed = Some(param.default);
        unit = param.to_unit(param.default);
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            unit = (unit + scroll / l.s(DRAG_RANGE * 4.0)).clamp(0.0, 1.0);
            let v = param.from_unit(unit);
            if v != value {
                changed = Some(v);
            }
        }
    }
    let painter = ui.painter();
    let start = 0.75 * PI;
    let sweep = 1.5 * PI;
    let ink = ink_on(face);
    let outer = if look.skirt.is_some() {
        r * 1.22
    } else {
        r * 1.04
    };
    painter.circle_filled(
        center + vec2(0.0, l.s(2.5)),
        outer,
        Color32::from_black_alpha(140),
    );
    if let Some(skirt) = look.skirt {
        painter.circle_filled(center, r * 1.22, skirt);
        painter.circle_stroke(
            center,
            r * 1.22,
            Stroke::new(l.s(0.8), mix(skirt, TOLEX, 0.4)),
        );
        for t in 0..=10 {
            let a = start + sweep * t as f32 / 10.0;
            let dir = vec2(a.cos(), a.sin());
            painter.line_segment(
                [center + dir * r * 1.04, center + dir * r * 1.18],
                Stroke::new(l.s(0.9), mix(skirt, TOLEX, 0.7)),
            );
        }
    }
    painter.circle_filled(center, r, look.body);
    for t in 0..36 {
        let a = t as f32 / 36.0 * std::f32::consts::TAU;
        let dir = vec2(a.cos(), a.sin());
        painter.line_segment(
            [center + dir * r * 0.86, center + dir * r * 0.99],
            Stroke::new(l.s(1.1), mix(look.body, METAL, 0.35)),
        );
    }
    painter.circle_filled(center, r * 0.74, look.cap);
    painter.circle_stroke(
        center,
        r * 0.74,
        Stroke::new(l.s(0.8), mix(look.cap, TOLEX, 0.3)),
    );
    painter.circle_filled(
        center - vec2(r * 0.26, r * 0.3),
        r * 0.28,
        Color32::from_white_alpha(28),
    );
    let a = start + sweep * unit;
    let tip = center + vec2(a.cos(), a.sin()) * r * 0.92;
    painter.line_segment(
        [center + vec2(a.cos(), a.sin()) * r * 0.2, tip],
        Stroke::new(l.s(2.0), look.pointer),
    );
    for t in 0..=10 {
        let a = start + sweep * t as f32 / 10.0;
        let p = center + vec2(a.cos(), a.sin()) * (outer + l.s(3.0));
        painter.circle_filled(p, l.s(0.9), mix(face, ink, 0.6));
    }
    let active = resp.dragged() || resp.hovered();
    let label = if active {
        param.text(param.from_unit(unit))
    } else {
        param.name.to_string()
    };
    painter.text(
        center + vec2(0.0, outer + l.s(8.0)),
        Align2::CENTER_CENTER,
        label,
        l.f(if radius < 12.0 { 7.0 } else { 8.5 }),
        if active { ORANGE } else { ink },
    );
    changed
}

pub fn button(
    ui: &Ui,
    l: &L,
    id: impl std::hash::Hash + std::fmt::Debug,
    rect: Rect,
    label: &str,
    lit: bool,
) -> Response {
    let resp = ui.interact(rect, Id::NULL.with(id), Sense::click());
    let painter = ui.painter();
    let face = if lit {
        ORANGE
    } else if resp.hovered() {
        mix(PANEL, INK, 0.12)
    } else {
        PANEL
    };
    painter.rect_filled(rect, l.s(4.0), face);
    painter.rect_stroke(
        rect,
        l.s(4.0),
        Stroke::new(l.s(1.0), FAINT),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        l.f(10.0),
        if lit { TOLEX } else { INK },
    );
    resp
}

pub fn led(ui: &Ui, l: &L, center: Pos2, on: bool, color: Color32) {
    let painter = ui.painter();
    painter.circle_filled(center, l.s(5.5), METAL);
    painter.circle_filled(center, l.s(4.4), TOLEX);
    painter.circle_filled(
        center,
        l.s(3.6),
        if on { color } else { mix(color, TOLEX, 0.75) },
    );
    painter.circle_filled(
        center - vec2(l.s(1.2), l.s(1.2)),
        l.s(1.0),
        Color32::from_white_alpha(if on { 160 } else { 40 }),
    );
    if on {
        painter.circle_filled(center, l.s(7.0), color.gamma_multiply(0.18));
    }
}

pub fn footswitch(
    ui: &Ui,
    l: &L,
    id: impl std::hash::Hash + std::fmt::Debug,
    center: Pos2,
) -> Response {
    let rect = Rect::from_center_size(center, vec2(l.s(34.0), l.s(34.0)));
    let resp = ui.interact(rect, Id::NULL.with(id), Sense::click());
    let painter = ui.painter();
    let down = resp.is_pointer_button_down_on();
    painter.circle_filled(
        center + vec2(0.0, l.s(3.0)),
        l.s(15.0),
        Color32::from_black_alpha(150),
    );
    painter.circle_filled(center, l.s(15.0), mix(METAL, TOLEX, 0.35));
    for t in 0..24 {
        let a = t as f32 / 24.0 * std::f32::consts::TAU;
        let dir = vec2(a.cos(), a.sin());
        painter.line_segment(
            [center + dir * l.s(11.5), center + dir * l.s(14.5)],
            Stroke::new(l.s(1.0), mix(METAL, TOLEX, 0.6)),
        );
    }
    let cap = center + vec2(0.0, if down { l.s(1.5) } else { 0.0 });
    painter.circle_filled(cap, l.s(10.0), METAL);
    painter.circle_filled(
        cap,
        l.s(8.0),
        mix(METAL, INK, if down { 0.2 } else { 0.45 }),
    );
    painter.circle_filled(
        cap - vec2(l.s(3.0), l.s(3.0)),
        l.s(2.5),
        Color32::from_white_alpha(70),
    );
    resp
}

pub fn meter(ui: &Ui, l: &L, rect: Rect, level: f32, label: &str) {
    let painter = ui.painter();
    painter.rect_filled(rect, l.s(2.0), FAINT);
    let db = 20.0 * level.max(1e-6).log10();
    let t = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let mut fill = rect.shrink(l.s(2.0));
    fill.set_width(fill.width() * t);
    let color = if db > -1.0 {
        RED
    } else if db > -9.0 {
        ORANGE
    } else {
        GREEN
    };
    painter.rect_filled(fill, l.s(1.0), color);
    painter.text(
        rect.left_center() - vec2(l.s(4.0), 0.0),
        Align2::RIGHT_CENTER,
        label,
        l.f(8.0),
        DIM,
    );
}

pub fn knob_grid(count: usize, area: Rect, l: &L) -> (Vec<Pos2>, f32) {
    let (cols, radius): (usize, f32) = match count {
        0..=2 => (count.max(1), 18.0),
        3..=4 => (2, 16.0),
        5..=6 => (2, 13.0),
        7..=9 => (3, 9.5),
        _ => (4, 8.5),
    };
    let rows = count.div_ceil(cols).max(1);
    let cw = area.width() / cols as f32;
    let fit = ((area.height() / rows as f32 / l.k - 24.0) / 2.0).max(6.0);
    let radius = radius.min(fit).min((cw / l.k - 12.0) / 2.6);
    let rh = (area.height() / rows as f32).min(l.s(radius * 2.0 + 30.0));
    let top = area.min.y + (area.height() - rh * rows as f32) / 2.0;
    let points = (0..count)
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            pos2(
                area.min.x + cw * (c as f32 + 0.5),
                top + rh * (r as f32 + 0.5) - l.s(4.0),
            )
        })
        .collect();
    (points, radius)
}

pub fn bevel(painter: &Painter, rect: Rect, radius: f32, light: bool) {
    let (top, bottom) = if light {
        (
            Color32::from_white_alpha(120),
            Color32::from_black_alpha(50),
        )
    } else {
        (Color32::from_white_alpha(35), Color32::from_black_alpha(80))
    };
    let inset = radius * 0.8;
    let w = (rect.height() * 0.02).clamp(1.0, 2.0);
    painter.line_segment(
        [
            pos2(rect.min.x + inset, rect.min.y + w * 0.5),
            pos2(rect.max.x - inset, rect.min.y + w * 0.5),
        ],
        Stroke::new(w, top),
    );
    painter.line_segment(
        [
            pos2(rect.min.x + inset, rect.max.y - w * 0.5),
            pos2(rect.max.x - inset, rect.max.y - w * 0.5),
        ],
        Stroke::new(w, bottom),
    );
}

pub fn screw(painter: &Painter, c: Pos2, r: f32, base: Color32) {
    painter.circle_filled(c + vec2(0.0, r * 0.25), r, Color32::from_black_alpha(90));
    painter.circle_filled(c, r, mix(base, METAL, 0.6));
    painter.circle_stroke(c, r, Stroke::new(r * 0.15, mix(base, TOLEX, 0.5)));
    let d = vec2(r * 0.55, -r * 0.55);
    painter.line_segment([c - d, c + d], Stroke::new(r * 0.3, mix(base, TOLEX, 0.7)));
}

pub fn shadow(painter: &Painter, l: &L, rect: Rect, radius: f32) {
    for i in 0..5 {
        let e = l.s(1.0 + i as f32 * 1.5);
        painter.rect_filled(
            rect.expand(e).translate(vec2(0.0, l.s(5.0))),
            radius + e,
            Color32::from_black_alpha(34 - 6 * i as u8),
        );
    }
}

pub fn jack_y(l: &L, rect: Rect) -> f32 {
    rect.min.y + l.s(40.0)
}

pub fn enclosure(painter: &Painter, l: &L, rect: Rect, face: Color32) {
    let radius = l.s(9.0);
    shadow(painter, l, rect, radius);
    for x in [rect.min.x - l.s(2.0), rect.max.x + l.s(2.0)] {
        let j = Rect::from_center_size(pos2(x, jack_y(l, rect)), vec2(l.s(10.0), l.s(14.0)));
        painter.rect_filled(j, l.s(2.0), mix(METAL, TOLEX, 0.3));
        painter.rect_filled(j.shrink(l.s(3.0)), l.s(1.0), TOLEX);
    }
    painter.rect_filled(
        rect.translate(vec2(0.0, l.s(4.0))),
        radius,
        mix(face, TOLEX, 0.55),
    );
    painter.rect_filled(rect, radius, face);
    painter.rect_stroke(
        rect.shrink(l.s(1.0)),
        radius,
        Stroke::new(l.s(1.0), Color32::from_white_alpha(45)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(l.s(1.0), mix(face, TOLEX, 0.45)),
        StrokeKind::Inside,
    );
    bevel(painter, rect, radius, true);
    for (dx, dy) in [(8.0, 8.0), (-8.0, 8.0), (8.0, -8.0), (-8.0, -8.0)] {
        let x = if dx > 0.0 {
            rect.min.x + l.s(dx)
        } else {
            rect.max.x + l.s(dx)
        };
        let y = if dy > 0.0 {
            rect.min.y + l.s(dy)
        } else {
            rect.max.y + l.s(dy)
        };
        screw(painter, pos2(x, y), l.s(2.6), face);
    }
}

pub fn cable(painter: &Painter, l: &L, from: Pos2, to: Pos2) {
    let sag = l.s(26.0);
    let c1 = from + vec2(l.s(10.0), sag);
    let c2 = to + vec2(-l.s(10.0), sag);
    let points: Vec<Pos2> = (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let u = 1.0 - t;
            let p = from.to_vec2() * (u * u * u)
                + c1.to_vec2() * (3.0 * u * u * t)
                + c2.to_vec2() * (3.0 * u * t * t)
                + to.to_vec2() * (t * t * t);
            p.to_pos2()
        })
        .collect();
    painter.add(Shape::line(
        points.clone(),
        Stroke::new(l.s(5.0), Color32::from_black_alpha(120)),
    ));
    painter.add(Shape::line(
        points,
        Stroke::new(l.s(3.2), mix(TOLEX, PANEL, 0.6)),
    ));
}

pub fn close_button(
    ui: &Ui,
    l: &L,
    id: impl std::hash::Hash + std::fmt::Debug,
    center: Pos2,
) -> Response {
    let rect = Rect::from_center_size(center, vec2(l.s(18.0), l.s(18.0)));
    let resp = ui.interact(rect, Id::NULL.with(id), Sense::click());
    let painter = ui.painter();
    let r = l.s(7.0);
    let face = if resp.hovered() {
        RED
    } else {
        mix(TOLEX, PANEL, 0.5)
    };
    painter.circle_filled(
        center + vec2(0.0, l.s(1.0)),
        r,
        Color32::from_black_alpha(120),
    );
    painter.circle_filled(center, r, face);
    painter.circle_stroke(center, r, Stroke::new(l.s(1.0), METAL));
    let d = l.s(3.0);
    let ink = if resp.hovered() { INK } else { METAL };
    painter.line_segment(
        [center + vec2(-d, -d), center + vec2(d, d)],
        Stroke::new(l.s(1.6), ink),
    );
    painter.line_segment(
        [center + vec2(-d, d), center + vec2(d, -d)],
        Stroke::new(l.s(1.6), ink),
    );
    resp
}

pub fn rails(painter: &Painter, l: &L, rect: Rect) {
    painter.rect_filled(rect, l.s(10.0), mix(TOLEX, PANEL, 0.35));
    let slats = 5;
    let h = rect.height() / slats as f32;
    for i in 0..slats {
        let slat = Rect::from_min_size(
            pos2(rect.min.x + l.s(6.0), rect.min.y + h * i as f32 + h * 0.18),
            vec2(rect.width() - l.s(12.0), h * 0.5),
        );
        painter.rect_filled(slat, l.s(3.0), mix(METAL, TOLEX, 0.86));
        painter.line_segment(
            [
                slat.left_top() + vec2(l.s(3.0), 0.5),
                slat.right_top() + vec2(-l.s(3.0), 0.5),
            ],
            Stroke::new(1.0_f32, Color32::from_white_alpha(28)),
        );
    }
}

pub fn rack_unit(painter: &Painter, l: &L, rect: Rect, face: Color32) {
    shadow(painter, l, rect, l.s(4.0));
    painter.rect_filled(rect, l.s(4.0), face);
    let lines = (rect.height() / l.s(3.0)) as usize;
    for i in 0..lines {
        let y = rect.min.y + l.s(3.0) * i as f32;
        painter.line_segment(
            [
                pos2(rect.min.x + l.s(14.0), y),
                pos2(rect.max.x - l.s(14.0), y),
            ],
            Stroke::new(
                1.0_f32,
                Color32::from_white_alpha(if i % 2 == 0 { 7 } else { 3 }),
            ),
        );
    }
    for x in [rect.min.x, rect.max.x - l.s(12.0)] {
        let ear = Rect::from_min_size(pos2(x, rect.min.y), vec2(l.s(12.0), rect.height()));
        painter.rect_filled(ear, l.s(3.0), mix(METAL, TOLEX, 0.45));
        for y in [rect.min.y + l.s(14.0), rect.max.y - l.s(14.0)] {
            let hole = Rect::from_center_size(pos2(ear.center().x, y), vec2(l.s(5.0), l.s(8.0)));
            painter.rect_filled(hole, l.s(2.5), TOLEX);
        }
    }
    bevel(painter, rect, l.s(4.0), false);
}

pub fn toggle(
    ui: &Ui,
    l: &L,
    id: impl std::hash::Hash + std::fmt::Debug,
    center: Pos2,
    on: bool,
    label: &str,
    ink: Color32,
) -> Response {
    let rect = Rect::from_center_size(center, vec2(l.s(24.0), l.s(34.0)));
    let resp = ui.interact(rect, Id::NULL.with(id), Sense::click());
    let painter = ui.painter();
    painter.circle_filled(
        center + vec2(0.0, l.s(1.5)),
        l.s(7.5),
        Color32::from_black_alpha(110),
    );
    painter.circle_filled(center, l.s(7.5), mix(METAL, TOLEX, 0.2));
    for t in 0..6 {
        let a = t as f32 / 6.0 * std::f32::consts::TAU;
        let p = center + vec2(a.cos(), a.sin()) * l.s(7.5);
        painter.circle_filled(p, l.s(1.2), mix(METAL, TOLEX, 0.35));
    }
    painter.circle_filled(center, l.s(4.0), mix(METAL, INK, 0.3));
    let dir = if on { -1.0 } else { 1.0 };
    let tip = center + vec2(0.0, dir * l.s(13.0));
    painter.line_segment(
        [center, tip],
        Stroke::new(l.s(4.0), mix(METAL, TOLEX, 0.15)),
    );
    painter.circle_filled(tip, l.s(3.2), METAL);
    painter.circle_filled(
        tip - vec2(l.s(1.0), l.s(1.0)),
        l.s(1.0),
        Color32::from_white_alpha(120),
    );
    painter.text(
        center + vec2(0.0, l.s(24.0)),
        Align2::CENTER_CENTER,
        label,
        l.f(7.0),
        ink,
    );
    resp
}

pub fn jewel(painter: &Painter, l: &L, center: Pos2, on: bool, color: Color32) {
    painter.circle_filled(center, l.s(9.0), METAL);
    painter.circle_stroke(
        center,
        l.s(9.0),
        Stroke::new(l.s(0.8), mix(METAL, TOLEX, 0.5)),
    );
    let lens = if on { color } else { mix(color, TOLEX, 0.7) };
    painter.circle_filled(center, l.s(6.5), lens);
    for t in 0..8 {
        let a = t as f32 / 8.0 * std::f32::consts::TAU;
        let d = vec2(a.cos(), a.sin());
        painter.line_segment(
            [center + d * l.s(2.0), center + d * l.s(6.3)],
            Stroke::new(l.s(0.6), mix(lens, TOLEX, 0.35)),
        );
    }
    painter.circle_filled(
        center - vec2(l.s(2.0), l.s(2.0)),
        l.s(1.6),
        Color32::from_white_alpha(if on { 200 } else { 60 }),
    );
    if on {
        painter.circle_filled(center, l.s(14.0), color.gamma_multiply(0.15));
    }
}

pub fn jack(painter: &Painter, l: &L, center: Pos2, label: &str, ink: Color32) {
    painter.circle_filled(center, l.s(8.0), METAL);
    painter.circle_stroke(
        center,
        l.s(8.0),
        Stroke::new(l.s(0.8), mix(METAL, TOLEX, 0.5)),
    );
    painter.circle_filled(center, l.s(4.0), TOLEX);
    painter.text(
        center + vec2(0.0, l.s(16.0)),
        Align2::CENTER_CENTER,
        label,
        l.f(7.0),
        ink,
    );
}

pub fn weave(painter: &Painter, l: &L, rect: Rect, base: Color32, thread: Color32, step: f32) {
    painter.rect_filled(rect, l.s(3.0), base);
    let s = l.s(step);
    let mut x = rect.min.x;
    while x < rect.max.x {
        painter.line_segment(
            [pos2(x, rect.min.y), pos2(x, rect.max.y)],
            Stroke::new(s * 0.35, thread),
        );
        x += s;
    }
    let mut y = rect.min.y;
    while y < rect.max.y {
        painter.line_segment(
            [pos2(rect.min.x, y), pos2(rect.max.x, y)],
            Stroke::new(s * 0.2, thread.gamma_multiply(0.7)),
        );
        y += s;
    }
}

pub fn diamond_plate(painter: &Painter, l: &L, rect: Rect) {
    painter.rect_filled(rect, l.s(3.0), Color32::from_rgb(0xb4, 0xb6, 0xb8));
    let s = l.s(9.0);
    let mut row = 0;
    let mut y = rect.min.y + s * 0.5;
    while y < rect.max.y {
        let mut x = rect.min.x + if row % 2 == 0 { s * 0.5 } else { s };
        while x < rect.max.x - s * 0.3 {
            let d = if (row + (x / s) as i32) % 2 == 0 {
                vec2(s * 0.32, -s * 0.18)
            } else {
                vec2(s * 0.32, s * 0.18)
            };
            let c = pos2(x, y);
            painter.line_segment(
                [c - d, c + d],
                Stroke::new(s * 0.18, Color32::from_rgb(0xd8, 0xda, 0xdc)),
            );
            painter.line_segment(
                [c - d + vec2(0.0, s * 0.08), c + d + vec2(0.0, s * 0.08)],
                Stroke::new(s * 0.08, Color32::from_black_alpha(60)),
            );
            x += s;
        }
        y += s * 0.75;
        row += 1;
    }
}

pub fn tolex(painter: &Painter, l: &L, rect: Rect, color: Color32, radius: f32) {
    painter.rect_filled(rect, radius, color);
    let s = l.s(5.0);
    let mut i = 0;
    let mut x = rect.min.x - rect.height();
    while x < rect.max.x {
        let a = pos2(x.max(rect.min.x), rect.max.y - (x.max(rect.min.x) - x));
        let b_x = (x + rect.height()).min(rect.max.x);
        let b = pos2(b_x, rect.max.y - (b_x - x));
        painter.line_segment(
            [a, b],
            Stroke::new(
                1.0_f32,
                Color32::from_white_alpha(if i % 3 == 0 { 7 } else { 3 }),
            ),
        );
        x += s;
        i += 1;
    }
}
