use std::f32::consts::PI;

use riff_dsp::param::Param;

use crate::egui::{
    Align2, Color32, FontId, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
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
    painter.circle_filled(
        center + vec2(0.0, l.s(2.0)),
        r,
        Color32::from_black_alpha(120),
    );
    painter.circle_filled(center, r, KNOB);
    painter.circle_stroke(center, r, Stroke::new(l.s(1.0), mix(KNOB, METAL, 0.4)));
    let a = start + sweep * unit;
    let tip = center + vec2(a.cos(), a.sin()) * r * 0.8;
    painter.line_segment(
        [center + vec2(a.cos(), a.sin()) * r * 0.25, tip],
        Stroke::new(l.s(2.0), KNOB_CAP),
    );
    for t in 0..=10 {
        let a = start + sweep * t as f32 / 10.0;
        let p = center + vec2(a.cos(), a.sin()) * (r + l.s(4.0));
        painter.circle_filled(p, l.s(0.9), mix(face, ink, 0.6));
    }
    let active = resp.dragged() || resp.hovered();
    let label = if active {
        param.text(param.from_unit(unit))
    } else {
        param.name.to_string()
    };
    painter.text(
        center + vec2(0.0, r + l.s(9.0)),
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
    painter.circle_filled(
        center,
        l.s(4.0),
        if on { color } else { mix(color, TOLEX, 0.75) },
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
        center + vec2(0.0, l.s(2.0)),
        l.s(14.0),
        Color32::from_black_alpha(140),
    );
    painter.circle_filled(center, l.s(14.0), METAL);
    painter.circle_filled(
        center,
        l.s(if down { 8.0 } else { 10.0 }),
        mix(METAL, INK, 0.5),
    );
    resp
}

pub fn meter(ui: &Ui, l: &L, rect: Rect, level: f32, label: &str) {
    let painter = ui.painter();
    painter.rect_filled(rect, l.s(2.0), TOLEX);
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
    let (cols, radius) = match count {
        0..=2 => (count.max(1), 18.0),
        3..=4 => (2, 16.0),
        5..=6 => (2, 13.0),
        _ => (3, 9.5),
    };
    let rows = count.div_ceil(cols).max(1);
    let cw = area.width() / cols as f32;
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
