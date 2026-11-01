use eframe::egui;

use crate::app::App;
use crate::audio;
use crate::settings::{BUFFERS, RATES};

pub fn window(app: &mut App, ctx: &egui::Context) {
    if !app.setup {
        return;
    }
    let mut open = true;
    let mut apply = false;
    let inputs = audio::input_devices();
    let outputs = audio::output_devices();
    egui::Window::new("SETUP")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
            let s = &mut app.settings;
            let current_in = s.input.clone().unwrap_or_else(|| "default".into());
            let channels = inputs
                .iter()
                .find(|(n, _)| Some(n) == s.input.as_ref())
                .map(|(_, c)| *c)
                .unwrap_or(2);
            egui::Grid::new("setup").num_columns(2).show(ui, |ui| {
                ui.label("INPUT");
                egui::ComboBox::from_id_salt("in")
                    .selected_text(current_in)
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for (n, c) in &inputs {
                            if ui
                                .selectable_label(Some(n) == s.input.as_ref(), n)
                                .clicked()
                            {
                                s.input = Some(n.clone());
                                s.channel = audio::guess_channel(n, *c);
                            }
                        }
                    });
                ui.end_row();
                ui.label("GUITAR ON");
                egui::ComboBox::from_id_salt("ch")
                    .selected_text(format!("input {}", s.channel + 1))
                    .show_ui(ui, |ui| {
                        for c in 0..channels.max(1) {
                            ui.selectable_value(&mut s.channel, c, format!("input {}", c + 1));
                        }
                    });
                ui.end_row();
                ui.label("OUTPUT");
                egui::ComboBox::from_id_salt("out")
                    .selected_text(s.output.clone().unwrap_or_else(|| "default".into()))
                    .width(280.0)
                    .show_ui(ui, |ui| {
                        for n in &outputs {
                            if ui
                                .selectable_label(Some(n) == s.output.as_ref(), n)
                                .clicked()
                            {
                                s.output = Some(n.clone());
                            }
                        }
                    });
                ui.end_row();
                ui.label("RATE");
                egui::ComboBox::from_id_salt("rate")
                    .selected_text(format!("{} Hz", s.rate))
                    .show_ui(ui, |ui| {
                        for r in RATES {
                            ui.selectable_value(&mut s.rate, r, format!("{r} Hz"));
                        }
                    });
                ui.end_row();
                ui.label("BUFFER");
                egui::ComboBox::from_id_salt("buf")
                    .selected_text(format!("{} samples", s.buffer))
                    .show_ui(ui, |ui| {
                        for b in BUFFERS {
                            ui.selectable_value(&mut s.buffer, b, format!("{b} samples"));
                        }
                    });
                ui.end_row();
            });
            ui.label("Scarlett Solo: the guitar jack (INST) is input 2.");
            if let Some(e) = &app.error {
                ui.colored_label(egui::Color32::from_rgb(0xff, 0x5b, 0x14), e);
            }
            if ui.button("APPLY").clicked() {
                apply = true;
            }
        });
    if apply {
        app.restart();
        app.settings.save();
        if app.error.is_none() {
            app.setup = false;
        }
    } else if !open {
        app.setup = false;
    }
}
