use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use riff_dsp::fx::Kind;
use riff_dsp::tuner::{self, Reading};
use riff_dsp::{Chain, Command, Library, Rig, Target};

use crate::presets::{self, Bank};
use crate::settings;
use crate::ui::{Host, Levels, Pick, View};
use crate::{RiffParams, Shared};

const TUNER_WINDOW: usize = 4096;
const TUNER_EVERY: f64 = 0.08;
const METER_FALL: f32 = 0.85;

pub struct Editor {
    params: Arc<RiffParams>,
    shared: Arc<Shared>,
    pub view: View,
    rig: Rig,
    seen: u64,
    library: Library,
    bank: Bank,
    preset: Option<usize>,
    compare: Option<Rig>,
    toast: Option<(String, f64)>,
    warnings: Vec<String>,
    now: f64,
    tuner_on: bool,
    reading: Option<Reading>,
    tap: VecDeque<f32>,
    last_tuned: f64,
    levels: (f32, f32, bool),
}

impl Editor {
    pub fn new(params: Arc<RiffParams>, shared: Arc<Shared>) -> Self {
        let rig = params.rig.read().map(|r| r.clone()).unwrap_or_default();
        Self {
            params,
            shared,
            view: View::default(),
            rig,
            seen: u64::MAX,
            library: Library::default(),
            bank: Bank::load(),
            preset: None,
            compare: None,
            toast: None,
            warnings: Vec::new(),
            now: 0.0,
            tuner_on: false,
            reading: None,
            tap: VecDeque::with_capacity(TUNER_WINDOW),
            last_tuned: 0.0,
            levels: (0.0, 0.0, false),
        }
    }

    fn send(&mut self, command: Command) {
        if let Ok(mut slot) = self.shared.remote.lock()
            && let Some(r) = slot.as_mut()
        {
            r.send(command);
        }
    }

    fn say(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), self.now + 2.5));
    }

    pub fn tick(&mut self, now: f64) {
        self.now = now;
        if self.toast.as_ref().is_some_and(|(_, until)| now > *until) {
            self.toast = None;
        }
        let generation = self.shared.generation.load(Ordering::Relaxed);
        if generation != self.seen {
            self.seen = generation;
            if let Ok(r) = self.params.rig.read() {
                self.rig = r.clone();
            }
        }
        let rate = self.shared.rate.load(Ordering::Relaxed) as f32;
        let mut samples = Vec::new();
        if let Ok(mut slot) = self.shared.remote.lock()
            && let Some(r) = slot.as_mut()
        {
            r.collect();
            let (i, o, g) = r.meters.read();
            self.levels = (
                i.max(self.levels.0 * METER_FALL),
                o.max(self.levels.1 * METER_FALL),
                g,
            );
            while let Ok(v) = r.tap.pop() {
                samples.push(v);
            }
        }
        for v in samples {
            if self.tap.len() >= TUNER_WINDOW {
                self.tap.pop_front();
            }
            self.tap.push_back(v);
        }
        if self.tuner_on && now - self.last_tuned > TUNER_EVERY && self.tap.len() == TUNER_WINDOW {
            self.last_tuned = now;
            let (a, b) = self.tap.as_slices();
            let buf: Vec<f32> = a.iter().chain(b).copied().collect();
            self.reading = tuner::detect(&buf, rate.max(1.0)).map(tuner::reading);
        }
    }

    pub fn store(&mut self) {
        if let Ok(mut r) = self.params.rig.write()
            && *r != self.rig
        {
            *r = self.rig.clone();
        }
        crate::update_layout(&self.shared, &self.rig);
    }
}

impl Host for Editor {
    fn rig(&mut self) -> &mut Rig {
        &mut self.rig
    }

    fn set(&mut self, target: Target, param: usize, value: f32) {
        self.send(Command::Set {
            target,
            param,
            value,
        });
    }

    fn enable(&mut self, target: Target, on: bool) {
        self.send(Command::Enable { target, on });
    }

    fn rebuild(&mut self) {
        self.rig.fix();
        let rate = self.shared.rate.load(Ordering::Relaxed) as f32;
        if rate <= 0.0 {
            return;
        }
        let chain = Chain::build(&self.rig, rate, &mut self.library);
        self.warnings = chain.warnings().to_vec();
        self.send(Command::Swap(Box::new(chain)));
        self.store();
    }

    fn levels(&self) -> Levels {
        Levels {
            input: self.levels.0,
            output: self.levels.1,
            gate: self.levels.2,
        }
    }

    fn tuner(&self) -> Option<(bool, Option<Reading>)> {
        Some((self.tuner_on, self.reading))
    }

    fn toggle_tuner(&mut self) {
        self.tuner_on = !self.tuner_on;
        let on = self.tuner_on;
        self.send(Command::Mute(on));
        if !on {
            self.reading = None;
        }
    }

    fn presets(&self) -> &[Rig] {
        &self.bank.items
    }

    fn current(&self) -> Option<usize> {
        self.preset
    }

    fn load_preset(&mut self, index: usize) {
        if let Some(rig) = self.bank.items.get(index).cloned() {
            self.preset = Some(index);
            let name = rig.name.clone();
            self.rig = rig;
            self.compare = None;
            self.rebuild();
            self.say(name);
        }
    }

    fn save_preset(&mut self, name: &str) {
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
                self.store();
                self.say(format!("saved {name}"));
            }
            Err(e) => self.say(format!("could not save: {e}")),
        }
    }

    fn comparing(&self) -> bool {
        self.compare.is_some()
    }

    fn toggle_compare(&mut self) {
        match self.compare.take() {
            None => {
                self.compare = Some(self.rig.clone());
                self.say("A stored. Change the tone, then press A/B to compare.");
            }
            Some(mut other) => {
                std::mem::swap(&mut self.rig, &mut other);
                self.compare = Some(other);
                self.rebuild();
            }
        }
    }

    fn messages(&self) -> Vec<String> {
        let mut out: Vec<String> = self.toast.iter().map(|(t, _)| t.clone()).collect();
        out.extend(self.warnings.iter().cloned());
        out
    }

    fn pick(&mut self, what: Pick) {
        let (folder, ext) = match what {
            Pick::Nam => ("nam", "nam"),
            Pick::Ir(_) => ("ir", "wav"),
        };
        let dir = settings::library().join(folder);
        let _ = std::fs::create_dir_all(&dir);
        let Some(path) = rfd::FileDialog::new()
            .set_directory(&dir)
            .add_filter(ext, &[ext])
            .pick_file()
        else {
            return;
        };
        let path = path.to_string_lossy().to_string();
        match what {
            Pick::Nam => {
                self.rig.amp.nam = Some(path);
                self.rig.amp.use_nam = true;
            }
            Pick::Ir(0) => self.rig.cab.ir_a = Some(path),
            Pick::Ir(_) => self.rig.cab.ir_b = Some(path),
        }
        self.rebuild();
    }

    fn status(&self) -> String {
        let has_wah = self.rig.pre.iter().any(|s| s.kind == Kind::Wah);
        format!(
            "plugin  {} Hz{}",
            self.shared.rate.load(Ordering::Relaxed),
            if has_wah {
                "   automate WAH from the host"
            } else {
                ""
            }
        )
    }
}
