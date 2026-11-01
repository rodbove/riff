mod app;
mod audio;
mod host;
mod midi;
mod presets;
mod settings;
mod setup;
#[path = "../../ui/src/mod.rs"]
mod ui;

pub use eframe::egui;

const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

struct Riff {
    app: app::App,
    view: ui::View,
}

impl eframe::App for Riff {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        self.app.tick(now);
        if !ctx.egui_wants_keyboard_input() {
            let (left, right, tune) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                    i.key_pressed(egui::Key::T),
                )
            });
            if left {
                self.app.step_preset(-1);
            }
            if right {
                self.app.step_preset(1);
            }
            if tune {
                self.app.toggle_tuner();
            }
        }
        ui::draw(ui, &mut self.app, &mut self.view);
        setup::window(&mut self.app, &ctx);
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
            .with_inner_size([ui::W, ui::H])
            .with_min_inner_size([ui::W * 0.6, ui::H * 0.6]),
        ..Default::default()
    };
    eframe::run_native(
        "riff",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Riff {
                app: app::App::new(),
                view: ui::View::default(),
            }))
        }),
    )
}
