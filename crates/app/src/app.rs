use std::collections::VecDeque;

use riff_dsp::tuner::{self, Reading};
use riff_dsp::{Chain, Command, Library, Rig, Target};

use crate::audio::{self, Audio};
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
            tuner_on: false,
            reading: None,
            meter_in: 0.0,
            meter_out: 0.0,
            gate_open: false,
            tap: VecDeque::with_capacity(TUNER_WINDOW * 2),
            last_tuned: 0.0,
        };
        app.restart();
        app
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
