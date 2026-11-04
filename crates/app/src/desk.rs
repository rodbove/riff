use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use riff_dsp::{Command, Library, Rig, chain};

use crate::app::App;
use crate::egui::{self, Align2, Color32, Id, Key, Rect, Sense, Stroke, Ui, pos2, vec2};
use crate::settings;
use crate::studio::{self, At, Clip, Session, Source};
use crate::take::{self, Take};
use crate::ui::theme::*;
use crate::ui::widgets::{L, button};

const TOP: f32 = 92.0;
const RULER: f32 = 136.0;
const LANES: f32 = 160.0;
const LANE: f32 = 72.0;
const HEAD: f32 = 20.0;
const HEAD_W: f32 = 176.0;
const LEFT: f32 = 200.0;
const RIGHT: f32 = 1180.0;
const BOTTOM: f32 = 748.0;
const EDGE: f32 = 6.0;
const MAX_TRACKS: usize = 7;
const VIEW_SECONDS: f32 = 20.0;

enum Drag {
    Move { at: At, grab: f32 },
    Front(At),
    Back(At),
    Head,
    Volume(usize),
}

struct Recording {
    track: usize,
    from: usize,
    take: Take,
}

pub enum Job {
    Say(String),
    Reamped(Source),
}

pub struct Desk {
    pub open: bool,
    pub session: Option<Session>,
    pub playhead: usize,
    playing: Option<(usize, riff_dsp::live::Track)>,
    recording: Option<Recording>,
    pub armed: usize,
    selected: Option<At>,
    zoom: f32,
    scroll: f32,
    drag: Option<Drag>,
    jobs: (Sender<Job>, Receiver<Job>),
}

impl Default for Desk {
    fn default() -> Self {
        Self {
            open: false,
            session: None,
            playhead: 0,
            playing: None,
            recording: None,
            armed: 0,
            selected: None,
            zoom: 0.0,
            scroll: 0.0,
            drag: None,
            jobs: channel(),
        }
    }
}

fn folder() -> PathBuf {
    settings::library().join("studio")
}

fn fmt_time(samples: usize, rate: u32) -> String {
    let t = samples as f64 / rate.max(1) as f64;
    let m = (t / 60.0) as u32;
    format!("{m}:{:06.3}", t - m as f64 * 60.0)
}

impl App {
    fn rate(&self) -> u32 {
        self.audio.as_ref().map(|a| a.rate).unwrap_or(48000)
    }

    pub fn toggle_studio(&mut self) {
        self.desk.open = !self.desk.open;
        if self.desk.open && self.desk.session.is_none() {
            let rate = self.rate();
            let session = match studio::load(&folder()) {
                Ok(s) if s.rate == rate => s,
                Ok(_) => {
                    self.say("the saved session uses another sample rate, so a new one starts");
                    Session::new(rate)
                }
                Err(_) => Session::new(rate),
            };
            self.desk.zoom = rate as f32 * VIEW_SECONDS / (RIGHT - LEFT);
            self.desk.session = Some(session);
        }
        if !self.desk.open {
            self.stop();
            self.save_session();
        }
    }

    pub fn save_session(&self) {
        if let Some(s) = &self.desk.session
            && let Err(e) = studio::save(&folder(), s)
        {
            eprintln!("could not save the studio session: {e}");
        }
    }

    pub fn playing(&self) -> bool {
        self.desk.playing.is_some()
    }

    pub fn play(&mut self) {
        if self.desk.playing.is_some() {
            self.stop();
            return;
        }
        let Some(s) = &self.desk.session else {
            return;
        };
        let mix: riff_dsp::live::Track = Arc::new(s.mix(self.desk.playhead, None));
        self.send(Command::Play(Some(mix.clone())));
        self.desk.playing = Some((self.desk.playhead, mix));
    }

    pub fn stop(&mut self) {
        if self.desk.recording.is_some() {
            self.finish_recording();
        }
        if let Some((from, _)) = self.desk.playing.take() {
            self.send(Command::Play(None));
            let _ = from;
        }
    }

    pub fn record(&mut self) {
        if self.desk.recording.is_some() {
            self.stop();
            return;
        }
        self.stop();
        let Some(a) = self.audio.as_mut() else {
            self.say("no audio device. Open SETUP.");
            return;
        };
        let rate = a.rate;
        let Some(s) = self.desk.session.as_mut() else {
            return;
        };
        if s.tracks.is_empty() {
            s.add_track();
        }
        let track = self.desk.armed.min(s.tracks.len() - 1);
        while a.remote.dry.pop().is_ok() {}
        while a.remote.wet.pop().is_ok() {}
        a.remote.meters.take_lost();
        let from = self.desk.playhead;
        let mix: riff_dsp::live::Track = Arc::new(s.mix(from, Some(track)));
        a.remote.send(Command::Play(Some(mix.clone())));
        a.remote.send(Command::Record(true));
        self.desk.playing = Some((from, mix));
        self.desk.recording = Some(Recording {
            track,
            from,
            take: Take::new(rate),
        });
    }

    fn finish_recording(&mut self) {
        let Some(mut r) = self.desk.recording.take() else {
            return;
        };
        self.send(Command::Record(false));
        let mut latency = 0;
        if let Some(a) = self.audio.as_mut() {
            crate::app::drain(a, &mut r.take);
            latency = (a.latency_ms * a.rate as f32 / 1000.0) as usize;
        }
        let Some(s) = self.desk.session.as_mut() else {
            return;
        };
        let wet: Vec<[f32; 2]> = r.take.wet.chunks(2).map(|f| [f[0], f[1]]).collect();
        let len = r.take.dry.len().min(wet.len());
        if len <= latency {
            self.say("nothing recorded");
            return;
        }
        let source = Source::new(s.id(), r.take.dry, wet);
        let clip = Clip {
            source: source.clone(),
            start: r.from,
            offset: latency,
            len: len - latency,
        };
        let at = s.place(r.track, clip);
        self.desk.selected = Some(at);
        let rate = s.rate;
        let lost = r.take.lost;
        let done = self.desk.jobs.0.clone();
        std::thread::spawn(move || {
            if let Err(e) = studio::write_source(&folder(), &source, rate) {
                let _ = done.send(Job::Say(format!("could not save the take: {e}")));
            }
        });
        if lost > 0 {
            self.say(format!(
                "{lost} samples were lost. Keep riff open while you record."
            ));
        }
        self.save_session();
    }

    pub fn studio_tick(&mut self) {
        while let Ok(job) = self.desk.jobs.1.try_recv() {
            match job {
                Job::Say(text) => self.toast = Some((text, self.now + 6.0)),
                Job::Reamped(source) => {
                    if let Some(s) = self.desk.session.as_mut() {
                        s.replace(source);
                        self.toast = Some(("reamp done".into(), self.now + 4.0));
                    }
                    self.save_session();
                }
            }
        }
        if let Some(r) = self.desk.recording.as_mut()
            && let Some(a) = self.audio.as_mut()
        {
            crate::app::drain(a, &mut r.take);
        }
        let Some((from, mix)) = &self.desk.playing else {
            return;
        };
        let played = self
            .audio
            .as_ref()
            .map(|a| a.remote.meters.played())
            .unwrap_or(0);
        self.desk.playhead = from + played;
        if self.desk.recording.is_none() && played >= mix.len() {
            self.stop();
        }
    }

    fn split(&mut self) {
        let at = self.desk.selected;
        let pos = self.desk.playhead;
        let Some(s) = self.desk.session.as_mut() else {
            return;
        };
        let at = at
            .filter(|a| s.clip(*a).is_some_and(|c| c.start < pos && pos < c.end()))
            .or_else(|| s.clip_at(self.desk.armed, pos));
        if let Some(at) = at
            && s.split(at, pos)
        {
            self.desk.selected = Some(At {
                track: at.track,
                clip: at.clip + 1,
            });
        }
    }

    fn delete(&mut self) {
        if let (Some(at), Some(s)) = (self.desk.selected.take(), self.desk.session.as_mut()) {
            s.delete(at);
        }
    }

    fn undo(&mut self) {
        if let Some(s) = self.desk.session.as_mut() {
            if s.undo() {
                self.desk.selected = None;
            } else {
                self.say("nothing to undo");
            }
        }
    }

    fn import(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import a WAV file into the armed track")
            .set_directory(take::folder())
            .add_filter("wav", &["wav"])
            .pick_file()
        else {
            return;
        };
        let Some(s) = self.desk.session.as_mut() else {
            return;
        };
        match studio::import(&path, s.rate) {
            Ok((dry, wet)) => {
                let source = Source::new(s.id(), dry, wet);
                let track = self.desk.armed.min(s.tracks.len().saturating_sub(1));
                if s.tracks.is_empty() {
                    s.add_track();
                }
                let at = s.place(track, Clip::whole(source.clone(), self.desk.playhead));
                self.desk.selected = Some(at);
                if let Err(e) = studio::write_source(&folder(), &source, s.rate) {
                    self.say(format!("could not save the clip: {e}"));
                }
                self.save_session();
            }
            Err(e) => self.say(format!("could not import: {e}")),
        }
    }

    fn reamp_track(&mut self) {
        let Some(s) = self.desk.session.as_ref() else {
            return;
        };
        let track = self.desk.armed.min(s.tracks.len().saturating_sub(1));
        if s.tracks.is_empty() || s.tracks[track].clips.is_empty() {
            self.say("the armed track has no clips");
            return;
        }
        let sources = s.sources(track);
        let rig: Rig = self.rig.clone();
        let rate = s.rate;
        let done = self.desk.jobs.0.clone();
        std::thread::spawn(move || {
            let mut library = Library::default();
            for src in sources {
                let wet = chain::render(&rig, rate as f32, &src.dry, &mut library);
                let new = src.rewet(wet);
                if let Err(e) = studio::write_source(&folder(), &new, rate) {
                    let _ = done.send(Job::Say(format!("could not save the clip: {e}")));
                }
                let _ = done.send(Job::Reamped(new));
            }
        });
        self.say(format!(
            "reamping {} with {}",
            s.tracks[track].name, self.rig.name
        ));
    }

    fn export(&mut self, di: bool) {
        let Some(s) = self.desk.session.as_ref() else {
            return;
        };
        let track = self.desk.armed.min(s.tracks.len().saturating_sub(1));
        if s.length() == 0 {
            self.say("nothing to export");
            return;
        }
        let name = if di {
            format!(
                "{} {} DI.wav",
                take::stamp(),
                take::clean(&s.tracks[track].name.to_lowercase())
            )
        } else {
            format!("{} mix.wav", take::stamp())
        };
        let _ = std::fs::create_dir_all(take::folder());
        let Some(path) = rfd::FileDialog::new()
            .set_title(if di {
                "Export the armed track as DI"
            } else {
                "Export the mix"
            })
            .set_directory(take::folder())
            .set_file_name(&name)
            .add_filter("wav", &["wav"])
            .save_file()
        else {
            return;
        };
        let rate = s.rate;
        let (channels, samples): (u16, Vec<f32>) = if di {
            (1, s.di(track))
        } else {
            (2, s.mix(0, None).into_iter().flatten().collect())
        };
        let done = self.desk.jobs.0.clone();
        std::thread::spawn(move || {
            let text = match riff_dsp::wav::write(&path, rate, channels, &samples) {
                Ok(()) => format!("exported {}", path.display()),
                Err(e) => format!("could not export: {e}"),
            };
            let _ = done.send(Job::Say(text));
        });
    }

    pub fn studio_status(&self) -> Option<String> {
        let r = self.desk.recording.as_ref()?;
        let s = r.take.seconds() as u32;
        Some(format!(
            "RECORDING {}:{:02}  press REC or SPACE to stop",
            s / 60,
            s % 60
        ))
    }
}

struct Map {
    l: L,
    zoom: f32,
    scroll: f32,
}

impl Map {
    fn x(&self, pos: usize) -> f32 {
        self.l
            .p(LEFT + (pos as f32 - self.scroll) / self.zoom, 0.0)
            .x
    }

    fn pos(&self, x: f32) -> usize {
        let logical = (x - self.l.o.x) / self.l.k;
        (self.scroll + (logical - LEFT) * self.zoom).max(0.0) as usize
    }

    fn lane(&self, track: usize) -> Rect {
        self.l
            .r(LEFT, LANES + track as f32 * LANE, RIGHT - LEFT, LANE - 4.0)
    }
}

pub fn draw(ui: &mut Ui, app: &mut App, l: &L) {
    if app.desk.session.is_none() {
        return;
    }
    keys(ui, app);
    transport(ui, app, l);
    let painter = ui.painter().clone();
    painter.rect_filled(
        l.r(20.0, RULER - 4.0, RIGHT - 20.0, BOTTOM - RULER + 4.0),
        l.s(6.0),
        PANEL,
    );
    let timeline = l.r(LEFT, RULER, RIGHT - LEFT, BOTTOM - RULER);
    scroll(ui, app, timeline, l);
    let map = Map {
        l: L { o: l.o, k: l.k },
        zoom: app.desk.zoom,
        scroll: app.desk.scroll,
    };
    follow(app, &map);
    ruler(ui, app, &map);
    let count = app
        .desk
        .session
        .as_ref()
        .map(|s| s.tracks.len())
        .unwrap_or(0);
    for t in 0..count {
        track_head(ui, app, l, t);
        lane(ui, app, &map, t);
    }
    if count < MAX_TRACKS {
        let y = LANES + count as f32 * LANE;
        if button(
            ui,
            l,
            "add-track",
            l.r(HEAD, y, HEAD_W - 4.0, 30.0),
            "+ TRACK",
            false,
        )
        .clicked()
            && let Some(s) = app.desk.session.as_mut()
        {
            app.desk.armed = s.add_track();
        }
    }
    let ph = map.x(app.desk.playhead);
    if ph >= l.p(LEFT, 0.0).x && ph <= l.p(RIGHT, 0.0).x {
        painter.line_segment(
            [pos2(ph, l.p(0.0, RULER).y), pos2(ph, l.p(0.0, BOTTOM).y)],
            Stroke::new(l.s(1.5), INK),
        );
    }
    if !ui.input(|i| i.pointer.any_down()) {
        if app.desk.drag.is_some()
            && let Some(s) = app.desk.session.as_ref()
            && s.length() > 0
        {
            app.save_session();
        }
        app.desk.drag = None;
    }
}

fn keys(ui: &Ui, app: &mut App) {
    if ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let (space, rec, split, del, undo, home) = ui.input(|i| {
        (
            i.key_pressed(Key::Space),
            i.key_pressed(Key::R),
            i.key_pressed(Key::S),
            i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
            i.modifiers.command && i.key_pressed(Key::Z),
            i.key_pressed(Key::Home) || i.key_pressed(Key::Enter),
        )
    });
    if space {
        app.play();
    }
    if rec {
        app.record();
    }
    if split {
        app.split();
    }
    if del {
        app.delete();
    }
    if undo {
        app.undo();
    }
    if home && !app.playing() {
        app.desk.playhead = 0;
        app.desk.scroll = 0.0;
    }
}

fn transport(ui: &Ui, app: &mut App, l: &L) {
    let row = |x: f32, w: f32| l.r(x, TOP, w, 36.0);
    if button(ui, l, "to-start", row(20.0, 40.0), "|<", false).clicked() && !app.playing() {
        app.desk.playhead = 0;
        app.desk.scroll = 0.0;
    }
    let playing = app.playing() && app.desk.recording.is_none();
    if button(
        ui,
        l,
        "play",
        row(64.0, 64.0),
        if playing { "STOP" } else { "PLAY" },
        playing,
    )
    .clicked()
    {
        app.play();
    }
    let recording = app.desk.recording.is_some();
    let rec = row(132.0, 56.0);
    if button(ui, l, "rec", rec, "REC", recording).clicked() {
        app.record();
    }
    if !recording {
        ui.painter()
            .circle_filled(rec.left_center() + vec2(l.s(10.0), 0.0), l.s(3.5), RED);
    }
    let rate = app.desk.session.as_ref().map(|s| s.rate).unwrap_or(48000);
    ui.painter().text(
        l.p(196.0, TOP + 18.0),
        Align2::LEFT_CENTER,
        fmt_time(app.desk.playhead, rate),
        l.f(15.0),
        if recording { RED } else { ORANGE },
    );
    if button(ui, l, "split", row(320.0, 60.0), "SPLIT", false).clicked() {
        app.split();
    }
    if button(ui, l, "delete", row(384.0, 64.0), "DELETE", false).clicked() {
        app.delete();
    }
    if button(ui, l, "undo", row(452.0, 56.0), "UNDO", false).clicked() {
        app.undo();
    }
    if button(ui, l, "import", row(540.0, 68.0), "IMPORT", false).clicked() {
        app.import();
    }
    if button(ui, l, "reamp", row(612.0, 104.0), "REAMP TRACK", false).clicked() {
        app.reamp_track();
    }
    if button(ui, l, "export-mix", row(748.0, 100.0), "EXPORT MIX", false).clicked() {
        app.export(false);
    }
    if button(ui, l, "export-di", row(852.0, 92.0), "EXPORT DI", false).clicked() {
        app.export(true);
    }
    let zoom_at = |app: &mut App, factor: f32| {
        let center = app.desk.playhead as f32;
        let before = (center - app.desk.scroll) / app.desk.zoom;
        app.desk.zoom = (app.desk.zoom * factor).clamp(2.0, rate as f32 * 2.0);
        app.desk.scroll = (center - before * app.desk.zoom).max(0.0);
    };
    if button(ui, l, "zoom-out", row(1100.0, 38.0), "-", false).clicked() {
        zoom_at(app, 1.5);
    }
    if button(ui, l, "zoom-in", row(1142.0, 38.0), "+", false).clicked() {
        zoom_at(app, 1.0 / 1.5);
    }
}

fn scroll(ui: &Ui, app: &mut App, timeline: Rect, l: &L) {
    let hover = ui
        .input(|i| i.pointer.hover_pos())
        .filter(|p| timeline.contains(*p));
    let Some(p) = hover else {
        return;
    };
    let (delta, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.command));
    if zoom && delta.y != 0.0 {
        let logical = (p.x - l.o.x) / l.k - LEFT;
        let at = app.desk.scroll + logical * app.desk.zoom;
        let rate = app.rate() as f32;
        app.desk.zoom = (app.desk.zoom * (-delta.y * 0.004).exp()).clamp(2.0, rate * 2.0);
        app.desk.scroll = (at - logical * app.desk.zoom).max(0.0);
    } else {
        let d = if delta.x != 0.0 { delta.x } else { delta.y };
        app.desk.scroll = (app.desk.scroll - d / l.k * app.desk.zoom).max(0.0);
    }
}

fn follow(app: &mut App, map: &Map) {
    if !app.playing() {
        return;
    }
    let right = map.l.p(RIGHT - 40.0, 0.0).x;
    if map.x(app.desk.playhead) > right {
        app.desk.scroll = app.desk.playhead as f32 - 40.0 * app.desk.zoom;
    }
}

fn ruler(ui: &Ui, app: &mut App, map: &Map) {
    let l = &map.l;
    let rect = l.r(LEFT, RULER, RIGHT - LEFT, 20.0);
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, TOLEX);
    let rate = app.rate() as f32;
    let seconds_per_px = map.zoom / rate;
    let step = [0.1f32, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]
        .into_iter()
        .find(|s| s / seconds_per_px >= 70.0)
        .unwrap_or(120.0);
    let first = (map.scroll / rate / step).floor() as i64;
    let mut n = first;
    loop {
        let t = n as f32 * step;
        let x = map.x((t * rate) as usize);
        if x > rect.right() {
            break;
        }
        if x >= rect.left() {
            painter.line_segment(
                [pos2(x, rect.bottom() - l.s(6.0)), pos2(x, rect.bottom())],
                Stroke::new(l.s(1.0), DIM),
            );
            let m = (t / 60.0) as u32;
            let label = if step < 1.0 {
                format!("{m}:{:04.1}", t - m as f32 * 60.0)
            } else {
                format!("{m}:{:02}", (t - m as f32 * 60.0) as u32)
            };
            painter.text(
                pos2(x + l.s(3.0), rect.top() + l.s(2.0)),
                Align2::LEFT_TOP,
                label,
                l.f(8.0),
                DIM,
            );
        }
        n += 1;
    }
    let resp = ui.interact(rect, Id::NULL.with("studio-ruler"), Sense::click_and_drag());
    if (resp.clicked() || resp.drag_started()) && !app.playing() {
        app.desk.drag = Some(Drag::Head);
    }
    if matches!(app.desk.drag, Some(Drag::Head))
        && let Some(p) = resp.interact_pointer_pos()
    {
        app.desk.playhead = map.pos(p.x);
    }
}

fn track_head(ui: &Ui, app: &mut App, l: &L, t: usize) {
    let y = LANES + t as f32 * LANE;
    let rect = l.r(HEAD, y, HEAD_W, LANE - 4.0);
    let armed = app.desk.armed == t;
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        l.s(4.0),
        if armed { mix(TOLEX, RED, 0.12) } else { TOLEX },
    );
    let Some(s) = app.desk.session.as_mut() else {
        return;
    };
    let audible = s.audible(t);
    let tr = &mut s.tracks[t];
    painter.text(
        l.p(HEAD + 8.0, y + 12.0),
        Align2::LEFT_CENTER,
        &tr.name,
        l.f(10.0),
        if audible { INK } else { DIM },
    );
    let small = |x: f32, w: f32| l.r(HEAD + x, y + 26.0, w, 20.0);
    if button(ui, l, ("arm", t), small(6.0, 34.0), "REC", armed).clicked() {
        app.desk.armed = t;
    }
    let Some(s) = app.desk.session.as_mut() else {
        return;
    };
    if button(ui, l, ("mute", t), small(44.0, 26.0), "M", s.tracks[t].mute).clicked() {
        s.remember();
        s.tracks[t].mute = !s.tracks[t].mute;
    }
    if button(ui, l, ("solo", t), small(74.0, 26.0), "S", s.tracks[t].solo).clicked() {
        s.remember();
        s.tracks[t].solo = !s.tracks[t].solo;
    }
    if button(ui, l, ("remove", t), small(146.0, 24.0), "X", false).clicked() {
        s.remove_track(t);
        if app.desk.armed >= s.tracks.len() {
            app.desk.armed = s.tracks.len().saturating_sub(1);
        }
        app.desk.selected = None;
        return;
    }
    let vol = l.r(HEAD + 6.0, y + 50.0, HEAD_W - 12.0, 14.0);
    let resp = ui.interact(vol, Id::NULL.with(("volume", t)), Sense::click_and_drag());
    if resp.drag_started() {
        s.remember();
        app.desk.drag = Some(Drag::Volume(t));
    }
    if resp.double_clicked() {
        s.remember();
        s.tracks[t].volume = 0.0;
    }
    if matches!(app.desk.drag, Some(Drag::Volume(v)) if v == t) {
        let d = resp.drag_delta().x / l.k * 0.25;
        s.tracks[t].volume = (s.tracks[t].volume + d).clamp(-60.0, 12.0);
    }
    let v = s.tracks[t].volume;
    let painter = ui.painter();
    painter.rect_filled(vol, l.s(2.0), FAINT);
    let mut fill = vol;
    fill.set_width(vol.width() * ((v + 60.0) / 72.0).clamp(0.0, 1.0));
    painter.rect_filled(fill, l.s(2.0), mix(FAINT, ORANGE, 0.5));
    painter.text(
        vol.center(),
        Align2::CENTER_CENTER,
        format!("VOL {v:+.1} dB"),
        l.f(8.0),
        INK,
    );
}

fn lane(ui: &Ui, app: &mut App, map: &Map, t: usize) {
    let l = &map.l;
    let rect = map.lane(t);
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, l.s(3.0), mix(PANEL, TOLEX, 0.5));
    let Some(s) = app.desk.session.as_ref() else {
        return;
    };
    let audible = s.audible(t);
    for (ci, c) in s.tracks[t].clips.iter().enumerate() {
        let x0 = map.x(c.start);
        let x1 = map.x(c.end());
        if x1 < rect.left() || x0 > rect.right() {
            continue;
        }
        let r = Rect::from_x_y_ranges(x0..=x1, rect.top() + l.s(2.0)..=rect.bottom() - l.s(2.0));
        let selected = app.desk.selected == Some(At { track: t, clip: ci });
        let base = if audible { ORANGE } else { DIM };
        painter.rect_filled(
            r,
            l.s(3.0),
            base.gamma_multiply(if selected { 0.45 } else { 0.25 }),
        );
        painter.rect_stroke(
            r,
            l.s(3.0),
            Stroke::new(l.s(if selected { 2.0 } else { 1.0 }), base),
            egui::StrokeKind::Inside,
        );
        waveform(&painter, map, c, r, base);
    }
    if let Some(r) = &app.desk.recording
        && r.track == t
    {
        let x0 = map.x(r.from);
        let x1 = map.x(app.desk.playhead);
        let rr = Rect::from_x_y_ranges(
            x0..=x1.max(x0 + 1.0),
            rect.top() + l.s(2.0)..=rect.bottom() - l.s(2.0),
        );
        painter.rect_filled(rr, l.s(3.0), RED.gamma_multiply(0.35));
    }
    interact(ui, app, map, t, rect);
}

fn waveform(painter: &egui::Painter, map: &Map, c: &Clip, r: Rect, color: Color32) {
    let peaks = &c.source.peaks;
    let mid = r.center().y;
    let half = r.height() * 0.45;
    let step = (map.l.k).max(1.0);
    let clip = painter.clip_rect();
    let mut x = r.left().max(clip.left());
    let end = r.right().min(clip.right());
    let stroke = Stroke::new(step, color);
    while x < end {
        let a = map.pos(x).max(c.start) - c.start + c.offset;
        let b = (map.pos(x + step).max(c.start) - c.start + c.offset).min(c.offset + c.len);
        let pa = a / studio::PEAK_BLOCK;
        let pb = (b / studio::PEAK_BLOCK + 1).min(peaks.len());
        let p = peaks
            .get(pa.min(pb)..pb)
            .map(|s| s.iter().fold(0.0f32, |m, v| m.max(*v)))
            .unwrap_or(0.0)
            .min(1.0);
        let h = (p * half).max(0.5);
        painter.line_segment([pos2(x, mid - h), pos2(x, mid + h)], stroke);
        x += step;
    }
}

fn interact(ui: &Ui, app: &mut App, map: &Map, t: usize, rect: Rect) {
    let resp = ui.interact(rect, Id::NULL.with(("lane", t)), Sense::click_and_drag());
    let l = &map.l;
    if resp.drag_started() || resp.clicked() {
        let Some(p) = resp.interact_pointer_pos() else {
            return;
        };
        let pos = map.pos(p.x);
        let Some(s) = app.desk.session.as_mut() else {
            return;
        };
        let edge = EDGE * l.k;
        let hit = s.tracks[t]
            .clips
            .iter()
            .enumerate()
            .rev()
            .find_map(|(ci, c)| {
                let (x0, x1) = (map.x(c.start), map.x(c.end()));
                (p.x >= x0 - edge && p.x <= x1 + edge).then_some((ci, x0, x1))
            });
        app.desk.armed = t;
        match hit {
            Some((ci, x0, x1)) => {
                let at = At { track: t, clip: ci };
                app.desk.selected = Some(at);
                if resp.drag_started() {
                    s.remember();
                    app.desk.drag = Some(if (p.x - x0).abs() <= edge {
                        Drag::Front(at)
                    } else if (p.x - x1).abs() <= edge {
                        Drag::Back(at)
                    } else {
                        let start = s.tracks[t].clips[ci].start as f32;
                        Drag::Move {
                            at,
                            grab: pos as f32 - start,
                        }
                    });
                } else if app.desk.playing.is_none() {
                    app.desk.playhead = pos;
                }
            }
            None => {
                app.desk.selected = None;
                if app.desk.playing.is_none() {
                    app.desk.playhead = pos;
                }
            }
        }
    }
    if let Some(p) = ui.input(|i| i.pointer.hover_pos())
        && rect.contains(p)
        && let Some(s) = app.desk.session.as_ref()
    {
        let edge = EDGE * l.k;
        let near = s.tracks[t]
            .clips
            .iter()
            .any(|c| (p.x - map.x(c.start)).abs() <= edge || (p.x - map.x(c.end())).abs() <= edge);
        if near {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
    let Some(p) = ui.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    let pos = map.pos(p.x);
    let Some(s) = app.desk.session.as_mut() else {
        return;
    };
    match app.desk.drag {
        Some(Drag::Move { at, grab }) if at.track == t => {
            s.shift(at, (pos as f32 - grab).max(0.0) as usize);
        }
        Some(Drag::Front(at)) if at.track == t => s.trim_front(at, pos),
        Some(Drag::Back(at)) if at.track == t => s.trim_back(at, pos),
        _ => {}
    }
}
