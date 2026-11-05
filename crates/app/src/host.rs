use riff_dsp::tuner::Reading;
use riff_dsp::{Rig, Target};

use crate::app::App;
use crate::settings;
use crate::ui::{Host, Levels, Pick};
const HIGH_LOAD: f32 = 0.6;

impl Host for App {
    fn rig(&mut self) -> &mut Rig {
        &mut self.rig
    }

    fn set(&mut self, target: Target, param: usize, value: f32) {
        App::set(self, target, param, value);
    }

    fn enable(&mut self, target: Target, on: bool) {
        App::enable(self, target, on);
    }

    fn rebuild(&mut self) {
        App::rebuild(self);
    }

    fn levels(&self) -> Levels {
        Levels {
            input: self.meter_in,
            output: self.meter_out,
            gate: self.gate_open,
        }
    }

    fn tuner(&self) -> Option<(bool, Option<Reading>)> {
        Some((self.tuner_on, self.reading))
    }

    fn toggle_tuner(&mut self) {
        App::toggle_tuner(self);
    }

    fn presets(&self) -> &[Rig] {
        &self.bank.items
    }

    fn current(&self) -> Option<usize> {
        self.preset
    }

    fn load_preset(&mut self, index: usize) {
        App::load_preset(self, index);
    }

    fn save_preset(&mut self, name: &str) {
        App::save_preset(self, name);
    }

    fn comparing(&self) -> bool {
        self.compare.is_some()
    }

    fn toggle_compare(&mut self) {
        App::toggle_compare(self);
    }

    fn messages(&self) -> Vec<String> {
        let mut out: Vec<String> = self.toast.iter().map(|(t, _)| t.clone()).collect();
        out.extend(self.studio_status());
        out.extend(self.warnings.iter().cloned());
        if let Some(e) = &self.error {
            out.push(e.clone());
        }
        if self.load > HIGH_LOAD {
            out.push(format!(
                "DSP load is {:.0}%. Above 50%, crackles can occur. Set BUFFER to 256, set QUALITY to LITE on a NAM capture, or read docs/nam-performance.md.",
                self.load * 100.0
            ));
        }
        if self.audio.as_ref().and_then(|a| a.realtime()) == Some(false) {
            out.push(
                "The audio thread is not realtime, so expect crackles. See the README.".into(),
            );
        }
        out
    }

    fn pick(&mut self, what: Pick) {
        let (folder, ext, title) = match what {
            Pick::Nam => ("nam", "nam", "Load a NAM capture"),
            Pick::Ir(_) => ("ir", "wav", "Load a cabinet IR"),
        };
        let dir = settings::library().join(folder);
        let _ = std::fs::create_dir_all(&dir);
        let Some(path) = rfd::FileDialog::new()
            .set_title(title)
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
        App::rebuild(self);
    }

    fn nam_slimmable(&self) -> bool {
        self.rig
            .amp
            .nam
            .as_ref()
            .is_some_and(|p| self.library.nam_slimmable(std::path::Path::new(p)))
    }

    fn status(&self) -> String {
        match &self.audio {
            Some(a) => format!(
                "{} input {}  >  {}   {} Hz   buffer {}   about {:.1} ms   DSP {:.0}%{}   dropouts {}{}   midi {}",
                a.input,
                a.channel + 1,
                a.output,
                a.rate,
                a.buffer,
                a.latency_ms,
                self.load * 100.0,
                a.core().map(|c| format!(" {c}")).unwrap_or_default(),
                a.dropouts.load(std::sync::atomic::Ordering::Relaxed),
                match a.realtime() {
                    Some(true) => "   realtime",
                    Some(false) => "   NOT REALTIME",
                    None => "",
                },
                self.midi.port.as_deref().unwrap_or("none")
            ),
            None => "no audio device. Open SETUP.".into(),
        }
    }

    fn studio(&self) -> Option<bool> {
        Some(self.desk.open)
    }

    fn toggle_studio(&mut self) {
        App::toggle_studio(self);
    }

    fn draw_studio(&mut self, ui: &mut crate::egui::Ui, l: &crate::ui::widgets::L) {
        crate::desk::draw(ui, self, l);
    }

    fn setup(&mut self) -> bool {
        self.setup = true;
        true
    }
}
