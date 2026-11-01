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

struct Shot {
    path: String,
    frames: u32,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

struct Riff {
    app: app::App,
    view: ui::View,
    shot: Option<Shot>,
}

impl Riff {
    fn capture(&mut self, ctx: &egui::Context) {
        let Some(shot) = self.shot.as_mut() else {
            return;
        };
        shot.frames += 1;
        if shot.frames == 20 {
            let path = shot.path.clone();
            let done = shot.done.clone();
            let ctx2 = ctx.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                egui::ScreenshotCallback::new(move |image| {
                    let [w, h] = image.size;
                    let mut out = format!("P6\n{w} {h}\n255\n").into_bytes();
                    for p in &image.pixels {
                        out.extend_from_slice(&[p.r(), p.g(), p.b()]);
                    }
                    let _ = std::fs::write(&path, out);
                    done.store(true, std::sync::atomic::Ordering::Relaxed);
                    ctx2.request_repaint();
                }),
            ));
        }
        if shot.done.load(std::sync::atomic::Ordering::Relaxed) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
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
        self.capture(&ctx);
        ctx.request_repaint_after(FRAME);
    }

    fn on_exit(&mut self) {
        if self.shot.is_none() {
            self.app.save();
        }
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
            let shot = std::env::var("RIFF_SHOT").ok().map(|path| Shot {
                path,
                frames: 0,
                done: Default::default(),
            });
            let mut app = app::App::new(shot.is_some());
            if let Ok(name) = std::env::var("RIFF_PRESET")
                && let Some(i) = app.bank.items.iter().position(|r| r.name == name)
            {
                app.load_preset(i);
            }
            Ok(Box::new(Riff {
                app,
                view: ui::View::default(),
                shot,
            }))
        }),
    )
}
