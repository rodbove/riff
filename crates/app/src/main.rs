mod app;
mod audio;
mod settings;
mod setup;

use eframe::egui;
use riff_dsp::Target;

const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

struct Riff {
    app: app::App,
}

impl eframe::App for Riff {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.app.tick(now);
        let app = &mut self.app;
        ui.horizontal(|ui| {
            ui.heading("riff");
            match &app.audio {
                Some(a) => ui.label(format!(
                    "{} input {}  >  {}   {} Hz   ~{:.1} ms   dropouts {}",
                    a.input,
                    a.channel + 1,
                    a.output,
                    a.rate,
                    a.latency_ms,
                    a.dropouts.load(std::sync::atomic::Ordering::Relaxed)
                )),
                None => ui.label("no audio"),
            };
            if ui.button("SETUP").clicked() {
                app.setup = true;
            }
            if ui.selectable_label(app.tuner_on, "TUNER").clicked() {
                app.toggle_tuner();
            }
        });
        ui.add(egui::ProgressBar::new(app.meter_in.min(1.0)).text("IN"));
        ui.add(egui::ProgressBar::new(app.meter_out.min(1.0)).text("OUT"));
        if app.tuner_on {
            match app.reading {
                Some(r) => ui.heading(format!("{}{}  {:+.0} cents", r.name(), r.octave, r.cents)),
                None => ui.heading("--"),
            };
        }
        let mut input = app.rig.input;
        if ui
            .add(egui::Slider::new(&mut input, -24.0..=24.0).text("INPUT dB"))
            .changed()
        {
            app.rig.input = input;
            app.set(Target::Input, 0, input);
        }
        let mut output = app.rig.output;
        if ui
            .add(egui::Slider::new(&mut output, -60.0..=12.0).text("OUTPUT dB"))
            .changed()
        {
            app.rig.output = output;
            app.set(Target::Output, 0, output);
        }
        let mut gate = app.rig.gate.on;
        if ui.checkbox(&mut gate, "GATE").changed() {
            app.rig.gate.on = gate;
            app.enable(Target::Gate, gate);
        }
        let mut threshold = app.rig.gate.threshold;
        if ui
            .add(egui::Slider::new(&mut threshold, -90.0..=-10.0).text("GATE dB"))
            .changed()
        {
            app.rig.gate.threshold = threshold;
            app.set(Target::Gate, 0, threshold);
        }
        let mut boost = !app.rig.pre.is_empty();
        if ui.checkbox(&mut boost, "BOOST").changed() {
            app.rig.pre.clear();
            if boost {
                app.rig
                    .pre
                    .push(riff_dsp::Slot::new(riff_dsp::fx::Kind::Boost));
            }
            app.rebuild();
        }
        setup::window(app, &ctx);
        ctx.request_repaint_after(FRAME);
    }

    fn on_exit(&mut self) {
        self.app.save();
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("riff")
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([800.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "riff",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Riff {
                app: app::App::new(),
            }))
        }),
    )
}
