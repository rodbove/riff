pub mod theme;
pub mod widgets;

use riff_dsp::amp::{self, AmpModel};
use riff_dsp::cab::{self, CabModel};
use riff_dsp::fx::Kind;
use riff_dsp::param::{Param, Unit};
use riff_dsp::rig::{MAX_POST, MAX_PRE, Slot};
use riff_dsp::tuner::Reading;
use riff_dsp::{Rig, Target, nam};

use crate::egui::{self, Align2, Id, Rect, Sense, Stroke, Ui, pos2, vec2};
use theme::*;
use widgets::*;

pub const W: f32 = 1200.0;
pub const H: f32 = 760.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Nam,
    Ir(usize),
}

pub struct Levels {
    pub input: f32,
    pub output: f32,
    pub gate: bool,
}

pub trait Host {
    fn rig(&mut self) -> &mut Rig;
    fn set(&mut self, target: Target, param: usize, value: f32);
    fn enable(&mut self, target: Target, on: bool);
    fn rebuild(&mut self);
    fn levels(&self) -> Levels;
    fn tuner(&self) -> Option<(bool, Option<Reading>)>;
    fn toggle_tuner(&mut self);
    fn presets(&self) -> &[Rig];
    fn current(&self) -> Option<usize>;
    fn load_preset(&mut self, index: usize);
    fn save_preset(&mut self, name: &str);
    fn comparing(&self) -> bool;
    fn toggle_compare(&mut self);
    fn messages(&self) -> Vec<String>;
    fn pick(&mut self, what: Pick);
    fn status(&self) -> String;
    fn setup(&mut self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct View {
    pub browser: bool,
    pub category: usize,
    pub album: usize,
    pub adding: Option<bool>,
    pub naming: Option<String>,
    drag: Option<(bool, usize, f32)>,
}

pub fn categories() -> Vec<&'static str> {
    riff_dsp::preset::BANDS
        .iter()
        .copied()
        .chain([riff_dsp::preset::USER])
        .collect()
}

pub fn draw(ui: &mut Ui, host: &mut dyn Host, view: &mut View) {
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, BACKDROP);
    let k = (full.width() / W).min(full.height() / H);
    let o = full.center() - vec2(W * k / 2.0, H * k / 2.0);
    let l = L { o, k };
    header(ui, &l, host, view);
    messages(ui, &l, host);
    let tuning = host.tuner().is_some_and(|t| t.0);
    if tuning {
        tuner(ui, &l, host);
    } else if view.browser {
        browser(ui, &l, host, view);
    } else if let Some(post) = view.adding {
        adder(ui, &l, host, view, post);
    } else {
        board(ui, &l, host, view);
    }
    amp_head(ui, &l, host);
    cabinet(ui, &l, host);
    rack(ui, &l, host, view);
    naming(ui.ctx(), host, view);
}

fn header(ui: &Ui, l: &L, host: &mut dyn Host, view: &mut View) {
    let painter = ui.painter();
    painter.rect_filled(l.r(0.0, 0.0, W, 64.0), 0.0, TOLEX);
    painter.text(
        l.p(24.0, 32.0),
        Align2::LEFT_CENTER,
        "riff",
        egui::FontId::proportional(l.s(34.0)),
        ORANGE,
    );
    let name = host.rig().name.clone();
    let preset = l.r(110.0, 14.0, 300.0, 36.0);
    if button(ui, l, "preset", preset, "", view.browser).clicked() {
        view.browser = !view.browser;
        view.adding = None;
        if view.browser {
            let (band, album) = (host.rig().category.clone(), host.rig().album.clone());
            if let Some(c) = categories().iter().position(|c| *c == band) {
                view.category = c;
                view.album = riff_dsp::preset::albums(host.presets(), &band)
                    .iter()
                    .position(|a| *a == album)
                    .unwrap_or(0);
            }
        }
    }
    ui.painter().text(
        preset.left_center() + vec2(l.s(12.0), 0.0),
        Align2::LEFT_CENTER,
        name.to_uppercase(),
        l.f(13.0),
        if view.browser { TOLEX } else { INK },
    );
    let count = host.presets().len() as i32;
    if button(ui, l, "prev", l.r(416.0, 14.0, 36.0, 36.0), "<", false).clicked() && count > 0 {
        let at = host.current().map(|c| c as i32).unwrap_or(0);
        host.load_preset((at - 1).rem_euclid(count) as usize);
    }
    if button(ui, l, "next", l.r(456.0, 14.0, 36.0, 36.0), ">", false).clicked() && count > 0 {
        let at = host.current().map(|c| c as i32).unwrap_or(-1);
        host.load_preset((at + 1).rem_euclid(count) as usize);
    }
    if button(ui, l, "save", l.r(500.0, 14.0, 64.0, 36.0), "SAVE", false).clicked() {
        view.naming = Some(name);
    }
    if button(
        ui,
        l,
        "ab",
        l.r(570.0, 14.0, 56.0, 36.0),
        "A/B",
        host.comparing(),
    )
    .clicked()
    {
        host.toggle_compare();
    }
    if let Some((on, _)) = host.tuner()
        && button(ui, l, "tuner", l.r(632.0, 14.0, 72.0, 36.0), "TUNER", on).clicked()
    {
        host.toggle_tuner();
    }
    let levels = host.levels();
    meter(ui, l, l.r(760.0, 16.0, 150.0, 12.0), levels.input, "IN");
    meter(ui, l, l.r(760.0, 36.0, 150.0, 12.0), levels.output, "OUT");
    led(ui, l, l.p(930.0, 22.0), levels.gate, GREEN);
    ui.painter()
        .text(l.p(940.0, 22.0), Align2::LEFT_CENTER, "GATE", l.f(8.0), DIM);
    let rig = host.rig();
    let (input, output) = (rig.input, rig.output);
    let in_p = Param::new("INPUT", -24.0, 24.0, 0.0, Unit::Db);
    let out_p = Param::new("OUTPUT", -60.0, 12.0, -6.0, Unit::Db);
    if let Some(v) = knob(ui, l, "in", l.p(1010.0, 28.0), 13.0, &in_p, input, TOLEX) {
        host.rig().input = v;
        host.set(Target::Input, 0, v);
    }
    if let Some(v) = knob(ui, l, "out", l.p(1065.0, 28.0), 13.0, &out_p, output, TOLEX) {
        host.rig().output = v;
        host.set(Target::Output, 0, v);
    }
    if button(
        ui,
        l,
        "setup",
        l.r(1100.0, 14.0, 80.0, 36.0),
        "SETUP",
        false,
    )
    .clicked()
    {
        host.setup();
    }
}

const BOARD: Rect = Rect {
    min: egui::Pos2 { x: 20.0, y: 88.0 },
    max: egui::Pos2 {
        x: 1180.0,
        y: 330.0,
    },
};
const PEDAL_W: f32 = 112.0;
const PEDAL_GAP: f32 = 4.0;

fn messages(ui: &Ui, l: &L, host: &mut dyn Host) {
    let painter = ui.painter();
    let msgs = host.messages();
    let note = host.rig().note.clone();
    let (text, color) = if !msgs.is_empty() {
        (msgs.join("   "), ORANGE)
    } else {
        (note, KNOB_CAP)
    };
    painter.text(l.p(24.0, 76.0), Align2::LEFT_CENTER, text, l.f(9.0), color);
    painter.text(
        l.p(W - 24.0, 76.0),
        Align2::RIGHT_CENTER,
        host.status(),
        l.f(8.0),
        DIM,
    );
}

fn floor(ui: &Ui, l: &L, area: Rect, title: &str) -> Rect {
    let r = l.r(area.min.x, area.min.y, area.width(), area.height());
    let painter = ui.painter();
    shadow(painter, l, r, l.s(10.0));
    rails(painter, l, r);
    painter.text(
        r.left_top() + vec2(l.s(12.0), l.s(4.0)),
        Align2::LEFT_TOP,
        title,
        l.f(8.0),
        DIM,
    );
    r
}

fn board(ui: &Ui, l: &L, host: &mut dyn Host, view: &mut View) {
    floor(ui, l, BOARD, "PEDALBOARD");
    let y = BOARD.min.y + 18.0;
    let h = BOARD.height() - 30.0;
    let slots = host.rig().pre.clone();
    let x_of = |i: usize| BOARD.min.x + 10.0 + i as f32 * (PEDAL_W + PEDAL_GAP);
    let mut rects = vec![l.r(x_of(0), y, PEDAL_W, h)];
    for i in 0..slots.len() {
        let mut x = x_of(i + 1);
        if let Some((false, from, dx)) = view.drag
            && from == i
        {
            x += dx / l.k;
        }
        rects.push(l.r(x, y, PEDAL_W, h));
    }
    for pair in rects.windows(2) {
        cable(
            ui.painter(),
            l,
            pos2(pair[0].max.x + l.s(3.0), jack_y(l, pair[0])),
            pos2(pair[1].min.x - l.s(3.0), jack_y(l, pair[1])),
        );
    }
    let gate = host.rig().gate.clone();
    let params = [
        Param::new("THRESH", -90.0, -10.0, -55.0, Unit::Db),
        Param::new("RELEASE", 5.0, 500.0, 60.0, Unit::Ms).curved(2.0),
    ];
    let values = [gate.threshold, gate.release];
    let out = stomp(
        ui,
        l,
        "gate",
        rects[0],
        "GATE",
        [0x1e, 0x1e, 0x1e],
        &params,
        &values,
        gate.on,
        false,
    );
    if let Some((i, v)) = out.changed {
        let g = &mut host.rig().gate;
        if i == 0 {
            g.threshold = v;
        } else {
            g.release = v;
        }
        host.set(Target::Gate, i, v);
    }
    if out.toggled {
        let on = !gate.on;
        host.rig().gate.on = on;
        host.enable(Target::Gate, on);
    }
    let mut remove = None;
    let mut drop_at = None;
    for (i, s) in slots.iter().enumerate() {
        let spec = s.kind.spec();
        let out = stomp(
            ui,
            l,
            ("pre", i),
            rects[i + 1],
            spec.name,
            spec.color,
            spec.params,
            &s.values,
            s.on,
            true,
        );
        if let Some((p, v)) = out.changed {
            host.rig().pre[i].values[p] = v;
            host.set(Target::Pre(i), p, v);
        }
        if out.toggled {
            let on = !s.on;
            host.rig().pre[i].on = on;
            host.enable(Target::Pre(i), on);
        }
        if out.removed || out.body.secondary_clicked() {
            remove = Some(i);
        }
        if out.body.dragged() {
            let dx = view
                .drag
                .filter(|d| !d.0 && d.1 == i)
                .map(|d| d.2)
                .unwrap_or(0.0);
            view.drag = Some((false, i, dx + out.body.drag_delta().x));
        }
        if out.body.drag_stopped()
            && let Some((false, from, dx)) = view.drag.take()
        {
            let shift = (dx / l.s(PEDAL_W + PEDAL_GAP)).round() as i32;
            drop_at = Some((
                from,
                (from as i32 + shift).clamp(0, slots.len() as i32 - 1) as usize,
            ));
        }
    }
    if slots.len() < MAX_PRE {
        let x = x_of(slots.len() + 1);
        if button(
            ui,
            l,
            "add-pre",
            l.r(x, y + h / 2.0 - 30.0, PEDAL_W, 60.0),
            "+ PEDAL",
            false,
        )
        .clicked()
        {
            view.adding = Some(false);
        }
    }
    ui.painter().text(
        l.p(BOARD.max.x - 12.0, BOARD.min.y + 4.0),
        Align2::RIGHT_TOP,
        "drag to reorder   X removes",
        l.f(7.5),
        DIM,
    );
    if let Some(i) = remove {
        host.rig().pre.remove(i);
        host.rebuild();
    } else if let Some((from, to)) = drop_at
        && from != to
    {
        let s = host.rig().pre.remove(from);
        host.rig().pre.insert(to, s);
        host.rebuild();
    }
}

struct Stomp {
    changed: Option<(usize, f32)>,
    toggled: bool,
    removed: bool,
    body: egui::Response,
}

#[allow(clippy::too_many_arguments)]
fn stomp(
    ui: &Ui,
    l: &L,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    rect: Rect,
    name: &str,
    color: [u8; 3],
    params: &[Param],
    values: &[f32],
    on: bool,
    removable: bool,
) -> Stomp {
    let body = ui.interact(rect, Id::NULL.with(("body", id)), Sense::click_and_drag());
    let face = rgb(color);
    let ink = ink_on(face);
    enclosure(ui.painter(), l, rect, face);
    let label = Rect::from_min_max(
        pos2(rect.min.x + l.s(10.0), rect.max.y - l.s(68.0)),
        pos2(rect.max.x - l.s(10.0), rect.max.y - l.s(46.0)),
    );
    let painter = ui.painter();
    painter.rect_filled(label, l.s(3.0), mix(face, TOLEX, 0.25));
    painter.text(label.center(), Align2::CENTER_CENTER, name, l.f(11.0), ink);
    let knobs = Rect::from_min_max(
        rect.min + vec2(l.s(6.0), l.s(18.0)),
        pos2(rect.max.x - l.s(6.0), rect.max.y - l.s(90.0)),
    );
    let (points, radius) = knob_grid(params.len(), knobs, l);
    let mut changed = None;
    for (i, (p, c)) in params.iter().zip(points).enumerate() {
        let v = values.get(i).copied().unwrap_or(p.default);
        if let Some(nv) = knob(ui, l, ("k", id, i), c, radius, p, v, face) {
            changed = Some((i, nv));
        }
    }
    led(
        ui,
        l,
        pos2(rect.center().x, rect.max.y - l.s(78.0)),
        on,
        RED,
    );
    let toggled = footswitch(
        ui,
        l,
        ("fs", id),
        pos2(rect.center().x, rect.max.y - l.s(24.0)),
    )
    .clicked();
    let removed = removable
        && close_button(
            ui,
            l,
            ("x", id),
            pos2(rect.max.x - l.s(4.0), rect.min.y + l.s(4.0)),
        )
        .clicked();
    Stomp {
        changed,
        toggled,
        removed,
        body,
    }
}

fn browser(ui: &Ui, l: &L, host: &mut dyn Host, view: &mut View) {
    floor(ui, l, BOARD, "PRESETS");
    let cats = categories();
    let tab_w = (BOARD.width() - 20.0) / cats.len() as f32;
    for (i, c) in cats.iter().enumerate() {
        let rect = l.r(
            BOARD.min.x + 10.0 + i as f32 * tab_w,
            BOARD.min.y + 16.0,
            tab_w - 6.0,
            26.0,
        );
        if button(ui, l, ("cat", i), rect, c, view.category == i).clicked() {
            view.category = i;
            view.album = 0;
        }
    }
    let cat = cats[view.category.min(cats.len() - 1)];
    let albums = riff_dsp::preset::albums(host.presets(), cat);
    let album = albums
        .get(view.album.min(albums.len().saturating_sub(1)))
        .cloned();
    let mut x = BOARD.min.x + 10.0;
    for (i, a) in albums.iter().enumerate() {
        let label = if a.is_empty() {
            "ALL".to_string()
        } else {
            a.to_uppercase()
        };
        let w = (label.len() as f32 * 7.2 + 24.0).max(70.0);
        let rect = l.r(x, BOARD.min.y + 48.0, w, 24.0);
        if button(ui, l, ("album", i), rect, &label, album.as_ref() == Some(a)).clicked() {
            view.album = i;
        }
        x += w + 6.0;
    }
    let items: Vec<(usize, String)> = host
        .presets()
        .iter()
        .enumerate()
        .filter(|(_, r)| r.category == cat && Some(&r.album) == album.as_ref())
        .map(|(i, r)| (i, r.name.clone()))
        .collect();
    if items.is_empty() {
        ui.painter().text(
            l.p(BOARD.center().x, BOARD.center().y + 20.0),
            Align2::CENTER_CENTER,
            "no presets here yet. SAVE stores the current tone.",
            l.f(10.0),
            DIM,
        );
    }
    let current = host.current();
    let w = 278.0;
    let mut load = None;
    for (n, (i, name)) in items.iter().enumerate() {
        let (c, row) = (n % 4, n / 4);
        let rect = l.r(
            BOARD.min.x + 10.0 + c as f32 * (w + 9.0),
            BOARD.min.y + 80.0 + row as f32 * 32.0,
            w,
            28.0,
        );
        if button(
            ui,
            l,
            ("p", *i),
            rect,
            &name.to_uppercase(),
            current == Some(*i),
        )
        .clicked()
        {
            load = Some(*i);
        }
    }
    if let Some(i) = load {
        host.load_preset(i);
        view.browser = false;
    }
}

fn rack(ui: &Ui, l: &L, host: &mut dyn Host, view: &mut View) {
    floor(ui, l, RACK, "EFFECTS RACK");
    let slots = host.rig().post.clone();
    let mut remove = None;
    for (i, s) in slots.iter().enumerate() {
        let rect = l.r(
            RACK.min.x + 10.0 + i as f32 * (UNIT_W + 8.0),
            RACK.min.y + 16.0,
            UNIT_W,
            RACK.height() - 26.0,
        );
        let spec = s.kind.spec();
        let face = mix(rgb(spec.color), TOLEX, 0.55);
        rack_unit(ui.painter(), l, rect, face);
        let head = Rect::from_min_size(
            rect.min + vec2(l.s(14.0), 0.0),
            vec2(rect.width() - l.s(28.0), l.s(26.0)),
        );
        let resp = ui.interact(head, Id::NULL.with(("unit-head", i)), Sense::click());
        let painter = ui.painter();
        painter.text(
            head.left_center() + vec2(l.s(4.0), l.s(2.0)),
            Align2::LEFT_CENTER,
            spec.name,
            l.f(11.0),
            INK,
        );
        led(
            ui,
            l,
            pos2(head.max.x - l.s(26.0), head.center().y + l.s(2.0)),
            s.on,
            GREEN,
        );
        if close_button(
            ui,
            l,
            ("ux", i),
            pos2(head.max.x - l.s(8.0), head.center().y + l.s(2.0)),
        )
        .clicked()
        {
            remove = Some(i);
        }
        let area = Rect::from_min_max(
            rect.min + vec2(l.s(16.0), l.s(30.0)),
            rect.max - vec2(l.s(16.0), l.s(8.0)),
        );
        let (points, radius) = knob_grid(spec.params.len(), area, l);
        for (p_i, (p, c)) in spec.params.iter().zip(points).enumerate() {
            if let Some(nv) = knob(ui, l, ("post", i, p_i), c, radius, p, s.value(p_i), face) {
                host.rig().post[i].values[p_i] = nv;
                host.set(Target::Post(i), p_i, nv);
            }
        }
        if resp.clicked() {
            let on = !s.on;
            host.rig().post[i].on = on;
            host.enable(Target::Post(i), on);
        }
        if resp.secondary_clicked() {
            remove = Some(i);
        }
    }
    if slots.len() < MAX_POST {
        let x = RACK.min.x + 10.0 + slots.len() as f32 * (UNIT_W + 8.0);
        if button(
            ui,
            l,
            "add-post",
            l.r(x, RACK.center().y - 25.0, UNIT_W, 50.0),
            "+ EFFECT",
            false,
        )
        .clicked()
        {
            view.adding = Some(true);
            view.browser = false;
        }
    }
    ui.painter().text(
        l.p(RACK.max.x - 12.0, RACK.min.y + 4.0),
        Align2::RIGHT_TOP,
        "click a name to switch   X removes",
        l.f(7.5),
        DIM,
    );
    if let Some(i) = remove {
        host.rig().post.remove(i);
        host.rebuild();
    }
}

fn adder(ui: &Ui, l: &L, host: &mut dyn Host, view: &mut View, post: bool) {
    let r = floor(ui, l, BOARD, if post { "ADD EFFECT" } else { "ADD PEDAL" });
    let kinds = if post { Kind::POST } else { Kind::PRE };
    let w = 180.0;
    for (i, k) in kinds.iter().enumerate() {
        let (c, row) = (i % 6, i / 6);
        let rect = l.r(
            BOARD.min.x + 12.0 + c as f32 * (w + 8.0),
            BOARD.min.y + 30.0 + row as f32 * 70.0,
            w,
            60.0,
        );
        let spec = k.spec();
        if button(ui, l, ("add", post, i), rect, "", false).clicked() {
            let rig = host.rig();
            if post {
                rig.post.push(Slot::new(*k));
            } else {
                rig.pre.push(Slot::new(*k));
            }
            host.rebuild();
            view.adding = None;
        }
        let painter = ui.painter();
        painter.rect_filled(
            Rect::from_min_size(
                rect.min + vec2(l.s(6.0), l.s(6.0)),
                vec2(l.s(10.0), rect.height() - l.s(12.0)),
            ),
            l.s(2.0),
            rgb(spec.color),
        );
        painter.text(
            rect.min + vec2(l.s(24.0), l.s(14.0)),
            Align2::LEFT_CENTER,
            spec.name,
            l.f(11.0),
            INK,
        );
        painter.text(
            rect.min + vec2(l.s(24.0), l.s(36.0)),
            Align2::LEFT_CENTER,
            spec.label,
            l.f(8.5),
            DIM,
        );
    }
    if button(
        ui,
        l,
        "add-cancel",
        l.r(BOARD.max.x - 100.0, BOARD.max.y - 44.0, 88.0, 32.0),
        "CANCEL",
        false,
    )
    .clicked()
    {
        view.adding = None;
    }
    let _ = r;
}

fn tuner(ui: &Ui, l: &L, host: &mut dyn Host) {
    let r = floor(ui, l, BOARD, "TUNER  output muted");
    let Some((_, reading)) = host.tuner() else {
        return;
    };
    let painter = ui.painter();
    let center = r.center();
    match reading {
        Some(t) => {
            let ok = t.cents.abs() < 3.0;
            painter.text(
                center - vec2(0.0, l.s(30.0)),
                Align2::CENTER_CENTER,
                format!("{}{}", t.name(), t.octave),
                egui::FontId::proportional(l.s(64.0)),
                if ok { GREEN } else { INK },
            );
            painter.text(
                center + vec2(0.0, l.s(28.0)),
                Align2::CENTER_CENTER,
                format!("{:+.1} cents   {:.2} Hz", t.cents, t.hz),
                l.f(12.0),
                DIM,
            );
            let bar =
                Rect::from_center_size(center + vec2(0.0, l.s(70.0)), vec2(l.s(500.0), l.s(10.0)));
            painter.rect_filled(bar, l.s(3.0), TOLEX);
            painter.line_segment(
                [
                    pos2(bar.center().x, bar.min.y - l.s(6.0)),
                    pos2(bar.center().x, bar.max.y + l.s(6.0)),
                ],
                Stroke::new(l.s(1.0), DIM),
            );
            let x = bar.center().x + (t.cents / 50.0).clamp(-1.0, 1.0) * bar.width() / 2.0;
            painter.circle_filled(
                pos2(x, bar.center().y),
                l.s(8.0),
                if ok { GREEN } else { ORANGE },
            );
        }
        None => {
            painter.text(
                center,
                Align2::CENTER_CENTER,
                "play a string",
                l.f(16.0),
                DIM,
            );
        }
    }
}

const AMP: Rect = Rect {
    min: egui::Pos2 { x: 20.0, y: 344.0 },
    max: egui::Pos2 { x: 790.0, y: 534.0 },
};

enum Band {
    Weave(egui::Color32, egui::Color32, f32),
    Diamond,
}

struct AmpLook {
    tolex: egui::Color32,
    piping: Option<egui::Color32>,
    band: Band,
    panel: egui::Color32,
    ink: egui::Color32,
    logo: egui::Color32,
    knob: KnobLook,
}

fn c(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

fn amp_look(model: Option<AmpModel>) -> AmpLook {
    let black = c(0x14, 0x14, 0x14);
    let chrome = c(0xc4, 0xc4, 0xc0);
    let black_weave = Band::Weave(c(0x16, 0x16, 0x16), c(0x26, 0x26, 0x26), 3.0);
    match model {
        Some(AmpModel::Clean) => AmpLook {
            tolex: black,
            piping: Some(c(0xb8, 0xb8, 0xb0)),
            band: Band::Weave(c(0x8e, 0x8e, 0x8a), c(0xc4, 0xc4, 0xbc), 3.0),
            panel: c(0x18, 0x18, 0x18),
            ink: c(0xe8, 0xe6, 0xe0),
            logo: c(0x1a, 0x1a, 0x1a),
            knob: KnobLook {
                body: black,
                cap: c(0x22, 0x22, 0x22),
                pointer: c(0xf0, 0xf0, 0xf0),
                skirt: Some(chrome),
            },
        },
        Some(AmpModel::Brit) => AmpLook {
            tolex: black,
            piping: Some(c(0xee, 0xee, 0xe8)),
            band: black_weave,
            panel: c(0xc9, 0xa9, 0x4a),
            ink: c(0x1a, 0x16, 0x0c),
            logo: c(0xf4, 0xf2, 0xec),
            knob: KnobLook {
                body: black,
                cap: c(0xd6, 0xb8, 0x5a),
                pointer: c(0x14, 0x12, 0x0a),
                skirt: None,
            },
        },
        Some(AmpModel::Mark) => AmpLook {
            tolex: c(0x10, 0x10, 0x10),
            piping: None,
            band: black_weave,
            panel: c(0x1c, 0x1c, 0x1c),
            ink: c(0xe8, 0xe8, 0xe8),
            logo: c(0xd8, 0xd8, 0xd8),
            knob: KnobLook {
                body: black,
                cap: c(0x1e, 0x1e, 0x1e),
                pointer: c(0xf4, 0xf4, 0xf4),
                skirt: None,
            },
        },
        Some(AmpModel::Recto) => AmpLook {
            tolex: c(0x10, 0x10, 0x10),
            piping: None,
            band: Band::Diamond,
            panel: c(0x16, 0x16, 0x16),
            ink: c(0xe8, 0xe8, 0xe8),
            logo: c(0x18, 0x18, 0x18),
            knob: KnobLook {
                body: black,
                cap: chrome,
                pointer: c(0x14, 0x14, 0x14),
                skirt: None,
            },
        },
        Some(AmpModel::Block) => AmpLook {
            tolex: c(0x12, 0x12, 0x12),
            piping: None,
            band: Band::Weave(c(0x1a, 0x1a, 0x1a), c(0x2c, 0x2c, 0x2c), 2.5),
            panel: c(0x1e, 0x1e, 0x20),
            ink: c(0xd8, 0xd8, 0xdc),
            logo: c(0xd0, 0xd2, 0xd6),
            knob: KnobLook {
                body: black,
                cap: chrome,
                pointer: c(0x14, 0x14, 0x14),
                skirt: None,
            },
        },
        Some(AmpModel::Uber) => AmpLook {
            tolex: c(0x0e, 0x0e, 0x0e),
            piping: Some(c(0xc8, 0x2a, 0x1e)),
            band: Band::Weave(c(0x22, 0x22, 0x22), c(0x34, 0x34, 0x34), 3.5),
            panel: c(0x12, 0x12, 0x12),
            ink: c(0xff, 0x7a, 0x3a),
            logo: c(0xff, 0x7a, 0x3a),
            knob: KnobLook {
                body: black,
                cap: c(0x1a, 0x1a, 0x1a),
                pointer: c(0xff, 0x7a, 0x3a),
                skirt: None,
            },
        },
        None => AmpLook {
            tolex: c(0x26, 0x28, 0x2c),
            piping: Some(c(0x2f, 0xb8, 0xa8)),
            band: Band::Weave(c(0x16, 0x1c, 0x1e), c(0x22, 0x2c, 0x30), 2.5),
            panel: c(0x12, 0x26, 0x26),
            ink: c(0x8c, 0xe8, 0xdc),
            logo: c(0x8c, 0xe8, 0xdc),
            knob: KnobLook {
                body: black,
                cap: c(0x1c, 0x2c, 0x2c),
                pointer: c(0x8c, 0xe8, 0xdc),
                skirt: None,
            },
        },
    }
}

fn corners(painter: &egui::Painter, l: &L, rect: Rect) {
    let s = l.s(13.0);
    for (x, y) in [
        (rect.min.x, rect.min.y),
        (rect.max.x - s, rect.min.y),
        (rect.min.x, rect.max.y - s),
        (rect.max.x - s, rect.max.y - s),
    ] {
        let r = Rect::from_min_size(pos2(x, y), vec2(s, s));
        painter.rect_filled(r, l.s(3.0), METAL);
        painter.circle_filled(r.center(), l.s(1.6), mix(METAL, TOLEX, 0.6));
    }
}

fn amp_head(ui: &Ui, l: &L, host: &mut dyn Host) {
    let state = host.rig().amp.clone();
    let use_nam = state.use_nam && state.nam.is_some();
    let look = amp_look(if use_nam { None } else { Some(state.model) });
    let painter = ui.painter();
    let handle = l.r(AMP.center().x - 80.0, AMP.min.y, 160.0, 12.0);
    painter.rect_filled(handle, l.s(6.0), c(0x24, 0x20, 0x1c));
    painter.line_segment(
        [
            handle.left_top() + vec2(l.s(10.0), l.s(2.0)),
            handle.right_top() + vec2(-l.s(10.0), l.s(2.0)),
        ],
        Stroke::new(l.s(1.0), egui::Color32::from_white_alpha(30)),
    );
    for x in [handle.min.x, handle.max.x - l.s(16.0)] {
        painter.rect_filled(
            Rect::from_min_size(pos2(x, handle.min.y + l.s(2.0)), vec2(l.s(16.0), l.s(12.0))),
            l.s(2.0),
            METAL,
        );
    }
    let head = l.r(
        AMP.min.x,
        AMP.min.y + 10.0,
        AMP.width(),
        AMP.height() - 10.0,
    );
    shadow(painter, l, head, l.s(10.0));
    tolex(painter, l, head, look.tolex, l.s(10.0));
    if let Some(p) = look.piping {
        painter.rect_stroke(
            head.shrink(l.s(7.0)),
            l.s(7.0),
            Stroke::new(l.s(1.6), p),
            egui::StrokeKind::Inside,
        );
    }
    corners(painter, l, head);
    let band = l.r(AMP.min.x + 16.0, AMP.min.y + 24.0, AMP.width() - 32.0, 56.0);
    match look.band {
        Band::Weave(base, thread, step) => weave(painter, l, band, base, thread, step),
        Band::Diamond => diamond_plate(painter, l, band),
    }
    painter.rect_stroke(
        band,
        l.s(3.0),
        Stroke::new(l.s(1.0), egui::Color32::from_black_alpha(140)),
        egui::StrokeKind::Inside,
    );
    let title = if use_nam {
        state
            .nam
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .map(|s| s.to_string_lossy().to_uppercase())
            .unwrap_or_default()
    } else {
        state.model.voicing().name.to_string()
    };
    let logo_pos = band.left_center() + vec2(l.s(18.0), -l.s(4.0));
    painter.text(
        logo_pos + vec2(l.s(1.0), l.s(1.5)),
        Align2::LEFT_CENTER,
        if use_nam { "NAM" } else { title.as_str() },
        egui::FontId::proportional(l.s(26.0)),
        egui::Color32::from_black_alpha(150),
    );
    painter.text(
        logo_pos,
        Align2::LEFT_CENTER,
        if use_nam { "NAM" } else { title.as_str() },
        egui::FontId::proportional(l.s(26.0)),
        look.logo,
    );
    let subtitle = if use_nam {
        title.clone()
    } else {
        state.model.voicing().like.to_uppercase()
    };
    painter.text(
        band.left_bottom() + vec2(l.s(20.0), -l.s(8.0)),
        Align2::LEFT_CENTER,
        subtitle,
        l.f(7.5),
        match look.band {
            Band::Weave(base, _, _) => mix(ink_on(base), base, 0.3),
            Band::Diamond => c(0x30, 0x30, 0x30),
        },
    );
    let mut rebuild = false;
    let chip_w = 54.0;
    let chips_x = AMP.max.x - 26.0 - 7.0 * (chip_w + 4.0);
    for (i, m) in AmpModel::ALL.iter().enumerate() {
        let rect = l.r(
            chips_x + i as f32 * (chip_w + 4.0),
            AMP.min.y + 40.0,
            chip_w,
            24.0,
        );
        let short = m.voicing().name.split(' ').next().unwrap_or("");
        if button(
            ui,
            l,
            ("model", i),
            rect,
            short,
            !use_nam && state.model == *m,
        )
        .clicked()
        {
            let amp = &mut host.rig().amp;
            amp.model = *m;
            amp.use_nam = false;
            rebuild = true;
        }
    }
    let nam_rect = l.r(
        chips_x + 6.0 * (chip_w + 4.0),
        AMP.min.y + 40.0,
        chip_w,
        24.0,
    );
    let nam_btn = button(ui, l, "nam", nam_rect, "NAM", use_nam);
    if nam_btn.clicked() {
        if state.nam.is_some() && !state.use_nam {
            host.rig().amp.use_nam = true;
            rebuild = true;
        } else {
            host.pick(Pick::Nam);
        }
    }
    if nam_btn.secondary_clicked() {
        host.pick(Pick::Nam);
    }
    let panel = l.r(
        AMP.min.x + 16.0,
        AMP.min.y + 86.0,
        AMP.width() - 32.0,
        AMP.height() - 98.0,
    );
    let painter = ui.painter();
    painter.rect_filled(panel, l.s(4.0), look.panel);
    bevel(painter, panel, l.s(4.0), true);
    for (x, y) in [
        (panel.min.x + l.s(7.0), panel.min.y + l.s(7.0)),
        (panel.max.x - l.s(7.0), panel.min.y + l.s(7.0)),
        (panel.min.x + l.s(7.0), panel.max.y - l.s(7.0)),
        (panel.max.x - l.s(7.0), panel.max.y - l.s(7.0)),
    ] {
        screw(painter, pos2(x, y), l.s(2.4), look.panel);
    }
    let row_y = panel.center().y - l.s(8.0);
    jack(
        painter,
        l,
        pos2(panel.min.x + l.s(34.0), row_y),
        "INPUT",
        look.ink,
    );
    let params: &[Param] = if use_nam {
        nam::PARAMS
    } else {
        &amp::PARAMS[..7]
    };
    let values = if use_nam {
        state.nam_values.clone()
    } else {
        state.values.clone()
    };
    for (i, p) in params.iter().enumerate() {
        let center = pos2(panel.min.x + l.s(96.0 + i as f32 * 72.0), row_y);
        let v = values.get(i).copied().unwrap_or(p.default);
        if let Some(nv) = knob_look(
            ui,
            l,
            ("amp", use_nam, i),
            center,
            17.0,
            p,
            v,
            look.panel,
            &look.knob,
        ) {
            let amp = &mut host.rig().amp;
            if use_nam {
                amp.nam_values[i] = nv;
            } else {
                amp.values[i] = nv;
            }
            host.set(Target::Amp, i, nv);
        }
    }
    if !use_nam {
        let bright = state.values.get(amp::BRIGHT).copied().unwrap_or(0.0) > 0.5;
        let at = pos2(
            panel.min.x + l.s(96.0 + 7.0 * 72.0 - 12.0),
            row_y - l.s(4.0),
        );
        if toggle(ui, l, "bright", at, bright, "BRIGHT", look.ink).clicked() {
            let v = if bright { 0.0 } else { 1.0 };
            host.rig().amp.values[amp::BRIGHT] = v;
            host.set(Target::Amp, amp::BRIGHT, v);
        }
    }
    let on = state.on;
    jewel(
        ui.painter(),
        l,
        pos2(panel.max.x - l.s(86.0), row_y - l.s(4.0)),
        on,
        RED,
    );
    if toggle(
        ui,
        l,
        "amp-on",
        pos2(panel.max.x - l.s(40.0), row_y - l.s(4.0)),
        on,
        "POWER",
        look.ink,
    )
    .clicked()
    {
        host.rig().amp.on = !on;
        host.enable(Target::Amp, !on);
    }
    if rebuild {
        host.rebuild();
    }
}

struct CabLook {
    tolex: egui::Color32,
    piping: Option<egui::Color32>,
    grille: egui::Color32,
    thread: egui::Color32,
    speakers: usize,
    height: f32,
}

fn cab_look(model: CabModel) -> CabLook {
    match model {
        CabModel::V30 => CabLook {
            tolex: c(0x12, 0x12, 0x12),
            piping: Some(c(0xe8, 0xe8, 0xe2)),
            grille: c(0x15, 0x15, 0x15),
            thread: c(0x2e, 0x2e, 0x2e),
            speakers: 4,
            height: 178.0,
        },
        CabModel::Greenback => CabLook {
            tolex: c(0x14, 0x14, 0x14),
            piping: Some(c(0xe8, 0xe8, 0xe2)),
            grille: c(0x5e, 0x5c, 0x55),
            thread: c(0x8e, 0x8b, 0x80),
            speakers: 4,
            height: 178.0,
        },
        CabModel::OpenBack => CabLook {
            tolex: c(0x16, 0x16, 0x16),
            piping: None,
            grille: c(0x8e, 0x8e, 0x8a),
            thread: c(0xbe, 0xbe, 0xb6),
            speakers: 2,
            height: 132.0,
        },
        CabModel::Single => CabLook {
            tolex: c(0x5a, 0x3c, 0x24),
            piping: Some(c(0x2a, 0x1c, 0x10)),
            grille: c(0x84, 0x6c, 0x48),
            thread: c(0xa2, 0x88, 0x5e),
            speakers: 1,
            height: 140.0,
        },
    }
}

fn cabinet(ui: &Ui, l: &L, host: &mut dyn Host) {
    let state = host.rig().cab.clone();
    let look = cab_look(state.model);
    let painter = ui.painter();
    let w = if look.speakers == 1 { 140.0 } else { 178.0 };
    let bx = CAB.min.x + 6.0 + (178.0 - w) / 2.0;
    let boxr = l.r(bx, CAB.max.y - 6.0 - look.height, w, look.height);
    shadow(painter, l, boxr, l.s(8.0));
    tolex(painter, l, boxr, look.tolex, l.s(8.0));
    let grille = boxr.shrink(l.s(11.0));
    weave(painter, l, grille, look.grille, look.thread, 2.5);
    let cols = if look.speakers >= 2 { 2 } else { 1 };
    let rows = look.speakers.div_ceil(cols);
    let cw = grille.width() / cols as f32;
    let rh = grille.height() / rows as f32;
    for i in 0..look.speakers {
        let center = pos2(
            grille.min.x + cw * ((i % cols) as f32 + 0.5),
            grille.min.y + rh * ((i / cols) as f32 + 0.5),
        );
        let r = cw.min(rh) * 0.42;
        painter.circle_filled(center, r, egui::Color32::from_black_alpha(70));
        painter.circle_stroke(
            center,
            r,
            Stroke::new(l.s(1.2), egui::Color32::from_white_alpha(14)),
        );
        painter.circle_stroke(
            center,
            r * 0.7,
            Stroke::new(l.s(1.0), egui::Color32::from_black_alpha(35)),
        );
        painter.circle_filled(center, r * 0.22, egui::Color32::from_black_alpha(70));
    }
    if let Some(p) = look.piping {
        painter.rect_stroke(
            grille.expand(l.s(2.0)),
            l.s(3.0),
            Stroke::new(l.s(1.0), p.gamma_multiply(0.8)),
            egui::StrokeKind::Outside,
        );
    }
    corners(painter, l, boxr);
    painter.text(
        grille.left_top() + vec2(l.s(6.0), l.s(8.0)),
        Align2::LEFT_CENTER,
        state.model.name(),
        l.f(8.0),
        egui::Color32::from_white_alpha(200),
    );
    let panel = l.r(
        CAB.min.x + 192.0,
        CAB.min.y + 6.0,
        CAB.width() - 196.0,
        CAB.height() - 12.0,
    );
    let painter = ui.painter();
    shadow(painter, l, panel, l.s(6.0));
    painter.rect_filled(panel, l.s(6.0), PANEL);
    bevel(painter, panel, l.s(6.0), false);
    let mut rebuild = false;
    let px = CAB.min.x + 200.0;
    for (i, m) in CabModel::ALL.iter().enumerate() {
        let rect = l.r(
            px + (i % 2) as f32 * 88.0,
            CAB.min.y + 12.0 + (i / 2) as f32 * 26.0,
            84.0,
            22.0,
        );
        if button(ui, l, ("cab", i), rect, m.name(), state.model == *m).clicked() {
            host.rig().cab.model = *m;
            rebuild = true;
        }
    }
    for mic in 0..2 {
        let path = if mic == 0 { &state.ir_a } else { &state.ir_b };
        let label = match path {
            Some(p) => std::path::Path::new(p)
                .file_stem()
                .map(|s| s.to_string_lossy().to_uppercase())
                .unwrap_or_default(),
            None => {
                if mic == 0 {
                    "MIC A ON AXIS".into()
                } else {
                    "MIC B OFF AXIS".into()
                }
            }
        };
        let rect = l.r(px, CAB.min.y + 70.0 + mic as f32 * 26.0, 134.0, 22.0);
        let resp = button(ui, l, ("ir", mic), rect, &label, path.is_some());
        if resp.clicked() {
            host.pick(Pick::Ir(mic));
        }
        if resp.secondary_clicked() && path.is_some() {
            let cab = &mut host.rig().cab;
            if mic == 0 {
                cab.ir_a = None;
            } else {
                cab.ir_b = None;
            }
            rebuild = true;
        }
    }
    let on = state.on;
    if toggle(
        ui,
        l,
        "cab-on",
        l.p(px + 156.0, CAB.min.y + 88.0),
        on,
        "CAB",
        INK,
    )
    .clicked()
    {
        host.rig().cab.on = !on;
        host.enable(Target::Cab, !on);
    }
    for (i, p) in cab::PARAMS.iter().enumerate() {
        let center = l.p(px + 16.0 + i as f32 * 44.0, CAB.min.y + 148.0);
        let v = state.values.get(i).copied().unwrap_or(p.default);
        if let Some(nv) = knob(ui, l, ("cabk", i), center, 12.0, p, v, PANEL) {
            host.rig().cab.values[i] = nv;
            host.set(Target::Cab, i, nv);
        }
    }
    if rebuild {
        host.rebuild();
    }
}

const CAB: Rect = Rect {
    min: egui::Pos2 { x: 800.0, y: 344.0 },
    max: egui::Pos2 {
        x: 1180.0,
        y: 534.0,
    },
};

const RACK: Rect = Rect {
    min: egui::Pos2 { x: 20.0, y: 546.0 },
    max: egui::Pos2 {
        x: 1180.0,
        y: 748.0,
    },
};
const UNIT_W: f32 = 182.0;

fn naming(ctx: &egui::Context, host: &mut dyn Host, view: &mut View) {
    let Some(mut name) = view.naming.take() else {
        return;
    };
    let mut open = true;
    let mut done = false;
    egui::Window::new("SAVE PRESET")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            let edit = ui.text_edit_singleline(&mut name);
            if !edit.has_focus() && !edit.lost_focus() {
                edit.request_focus();
            }
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("SAVE").clicked() || enter {
                done = true;
            }
        });
    if done {
        host.save_preset(&name);
    } else if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        view.naming = Some(name);
    }
}
