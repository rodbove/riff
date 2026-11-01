use std::collections::VecDeque;

use riff_dsp::fx::Kind;
use riff_dsp::tuner::{self, Reading};
use riff_dsp::{Chain, Command, Library, Rig, Target};

use crate::audio::{self, Audio};
use crate::midi::{self, Midi};
use crate::presets::{self, Bank};
use crate::settings::{self, Settings};

const TUNER_WINDOW: usize = 4096;
const TUNER_EVERY: f64 = 0.08;
const METER_FALL: f32 = 0.85;

pub struct App {
    pub settings: Settings,
    pub rig: Rig,
    pub library: Library,
    pub warnings: Vec<String>,
    pub audio: Option<Audio>,
    pub error: Option<String>,
    pub setup: bool,
    pub bank: Bank,
    pub preset: Option<usize>,
    pub compare: Option<Rig>,
    pub midi: Midi,
    pub toast: Option<(String, f64)>,
    pub now: f64,
    pub tuner_on: bool,
    pub reading: Option<Reading>,
    pub meter_in: f32,
    pub meter_out: f32,
    pub gate_open: bool,
    tap: VecDeque<f32>,
    last_tuned: f64,
}

impl App {
    pub fn new() -> Self {
        let settings = Settings::load();
        let rig = load_current().unwrap_or_default();
        let mut app = Self {
            settings,
            rig,
            library: Library::default(),
            warnings: Vec::new(),
            audio: None,
            error: None,
            setup: false,
            bank: Bank::load(),
            preset: None,
            compare: None,
            midi: Midi::default(),
            toast: None,
            now: 0.0,
            tuner_on: false,
            reading: None,
            meter_in: 0.0,
            meter_out: 0.0,
            gate_open: false,
            tap: VecDeque::with_capacity(TUNER_WINDOW * 2),
            last_tuned: 0.0,
        };
        app.restart();
        let port = app
            .settings
            .midi
            .clone()
            .or_else(|| midi::ports().into_iter().next());
        if let Some(p) = port {
            let _ = app.midi.connect(&p, || {});
        }
        app
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), self.now + 2.5));
    }

    pub fn load_rig(&mut self, rig: Rig) {
        self.rig = rig;
        self.rig.fix();
        self.compare = None;
        self.rebuild();
    }

    pub fn load_preset(&mut self, index: usize) {
        if let Some(rig) = self.bank.items.get(index).cloned() {
            self.preset = Some(index);
            let name = rig.name.clone();
            self.load_rig(rig);
            self.say(name);
        }
    }

    pub fn step_preset(&mut self, delta: i32) {
        let n = self.bank.items.len() as i32;
        if n == 0 {
            return;
        }
        let at = self.preset.map(|p| p as i32).unwrap_or(-1);
        self.load_preset((at + delta).rem_euclid(n) as usize);
    }

    pub fn save_preset(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let mut rig = self.rig.clone();
        rig.name = name.to_string();
        rig.category = riff_dsp::preset::USER.into();
        match presets::save(&rig) {
            Ok(_) => {
                self.rig = rig.clone();
                self.preset = Some(self.bank.add(rig));
                self.say(format!("saved {name}"));
            }
            Err(e) => self.say(format!("could not save: {e}")),
        }
    }

    pub fn toggle_compare(&mut self) {
        match self.compare.take() {
            None => {
                self.compare = Some(self.rig.clone());
                self.say("A stored. Change the tone, then press A/B to compare.");
            }
            Some(mut other) => {
                std::mem::swap(&mut self.rig, &mut other);
                self.compare = Some(other);
                self.rebuild();
                self.say("switched");
            }
        }
    }

    fn midi(&mut self) {
        for event in self.midi.poll() {
            match event {
                midi::Event::Program(n) => {
                    if !self.bank.items.is_empty() {
                        self.load_preset(n as usize % self.bank.items.len());
                    }
                }
                midi::Event::Control { cc, value } if cc == midi::EXPRESSION => {
                    let v = value as f32 / 127.0;
                    if let Some(i) = self.rig.pre.iter().position(|s| s.kind == Kind::Wah) {
                        self.rig.pre[i].values[0] = v;
                        self.set(Target::Pre(i), 0, v);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn restart(&mut self) {
        self.audio = None;
        let (rig, library) = (&self.rig, &mut self.library);
        let mut warnings = Vec::new();
        let result = audio::start(&self.settings, |rate| {
            let chain = Chain::build(rig, rate, library);
            warnings = chain.warnings().to_vec();
            chain
        });
        self.warnings = warnings;
        match result {
            Ok(a) => {
                self.audio = Some(a);
                self.error = None;
            }
            Err(e) => {
                self.error = Some(e);
                self.setup = true;
            }
        }
    }

    pub fn send(&mut self, command: Command) {
        if let Some(a) = self.audio.as_mut() {
            a.remote.send(command);
        }
    }

    pub fn rebuild(&mut self) {
        self.rig.fix();
        if let Some(a) = self.audio.as_mut() {
            let chain = Chain::build(&self.rig, a.rate as f32, &mut self.library);
            self.warnings = chain.warnings().to_vec();
            a.remote.send(Command::Swap(Box::new(chain)));
        }
    }

    pub fn set(&mut self, target: Target, param: usize, value: f32) {
        self.send(Command::Set {
            target,
            param,
            value,
        });
    }

    pub fn enable(&mut self, target: Target, on: bool) {
        self.send(Command::Enable { target, on });
    }

    pub fn toggle_tuner(&mut self) {
        self.tuner_on = !self.tuner_on;
        let on = self.tuner_on;
        self.send(Command::Mute(on));
        if !on {
            self.reading = None;
        }
    }

    pub fn tick(&mut self, now: f64) {
        self.now = now;
        if self.toast.as_ref().is_some_and(|(_, until)| now > *until) {
            self.toast = None;
        }
        self.midi();
        let Some(a) = self.audio.as_mut() else {
            return;
        };
        a.remote.collect();
        let (i, o, gate) = a.remote.meters.read();
        self.meter_in = i.max(self.meter_in * METER_FALL);
        self.meter_out = o.max(self.meter_out * METER_FALL);
        self.gate_open = gate;
        while let Ok(v) = a.remote.tap.pop() {
            if self.tap.len() >= TUNER_WINDOW {
                self.tap.pop_front();
            }
            self.tap.push_back(v);
        }
        if self.tuner_on && now - self.last_tuned > TUNER_EVERY && self.tap.len() == TUNER_WINDOW {
            self.last_tuned = now;
            let (a_part, b_part) = self.tap.as_slices();
            let buf: Vec<f32> = a_part.iter().chain(b_part).copied().collect();
            let rate = self
                .audio
                .as_ref()
                .map(|a| a.rate as f32)
                .unwrap_or(48000.0);
            self.reading = tuner::detect(&buf, rate).map(tuner::reading);
        }
    }

    pub fn save(&self) {
        self.settings.save();
        let dir = settings::config_dir();
        if std::fs::create_dir_all(&dir).is_ok()
            && let Ok(text) = serde_json::to_string_pretty(&self.rig)
        {
            let _ = std::fs::write(dir.join("current.json"), text);
        }
    }
}

fn load_current() -> Option<Rig> {
    let text = std::fs::read_to_string(settings::config_dir().join("current.json")).ok()?;
    let mut rig: Rig = serde_json::from_str(&text).ok()?;
    rig.fix();
    Some(rig)
}
