#![allow(semicolon_in_expressions_from_non_local_macros)]

mod editor;
#[allow(dead_code)]
#[path = "../../app/src/presets.rs"]
mod presets;
#[allow(dead_code)]
#[path = "../../app/src/settings.rs"]
mod settings;
#[path = "../../ui/src/mod.rs"]
mod ui;

use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use nih_plug::prelude::*;
use nih_plug_egui::{EguiState, create_egui_editor};
use riff_dsp::fx::Kind;
use riff_dsp::{Chain, Library, Remote, Rig, Runner, Target, live};

pub use nih_plug_egui::egui;

#[derive(Default)]
pub struct Shared {
    pub remote: Mutex<Option<Remote>>,
    pub rate: AtomicU32,
    pub generation: AtomicU64,
    wah: AtomicI32,
    delay: AtomicI32,
    reverb: AtomicI32,
    nam: AtomicBool,
}

pub fn update_layout(shared: &Shared, rig: &Rig) {
    let find = |slots: &[riff_dsp::Slot], k: Kind| {
        slots
            .iter()
            .position(|s| s.kind == k)
            .map(|i| i as i32)
            .unwrap_or(-1)
    };
    let o = Ordering::Relaxed;
    shared.wah.store(find(&rig.pre, Kind::Wah), o);
    shared.delay.store(find(&rig.post, Kind::Delay), o);
    shared.reverb.store(find(&rig.post, Kind::Reverb), o);
    shared
        .nam
        .store(rig.amp.use_nam && rig.amp.nam.is_some(), o);
}

#[derive(Params)]
pub struct RiffParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
    #[persist = "rig"]
    pub rig: Arc<RwLock<Rig>>,
    #[id = "input"]
    input: FloatParam,
    #[id = "output"]
    output: FloatParam,
    #[id = "gate"]
    gate: FloatParam,
    #[id = "gain"]
    gain: FloatParam,
    #[id = "wah"]
    wah: FloatParam,
    #[id = "delay"]
    delay: FloatParam,
    #[id = "reverb"]
    reverb: FloatParam,
}

fn db_param(name: &str, min: f32, max: f32, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min, max })
        .with_unit(" dB")
        .with_step_size(0.1)
}

fn unit_param(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

impl Default for RiffParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1200, 760),
            rig: Arc::new(RwLock::new(
                riff_dsp::preset::factory()
                    .into_iter()
                    .next()
                    .unwrap_or_default(),
            )),
            input: db_param("Input", -24.0, 24.0, 0.0),
            output: db_param("Output", -60.0, 12.0, -6.0),
            gate: db_param("Gate", -90.0, -10.0, -55.0),
            gain: FloatParam::new(
                "Amp Gain",
                5.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            )
            .with_step_size(0.1),
            wah: unit_param("Wah", 0.5),
            delay: unit_param("Delay Mix", 0.25),
            reverb: unit_param("Reverb Mix", 0.2),
        }
    }
}

pub struct RiffPlugin {
    params: Arc<RiffParams>,
    shared: Arc<Shared>,
    runner: Option<Runner>,
    input: Vec<f32>,
    output: Vec<[f32; 2]>,
    last: [f32; 7],
}

impl Default for RiffPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(RiffParams::default()),
            shared: Arc::new(Shared::default()),
            runner: None,
            input: Vec::new(),
            output: Vec::new(),
            last: [f32::NAN; 7],
        }
    }
}

impl RiffPlugin {
    fn automate(&mut self) {
        let Some(runner) = self.runner.as_mut() else {
            return;
        };
        let p = &self.params;
        let o = Ordering::Relaxed;
        let values = [
            p.input.value(),
            p.output.value(),
            p.gate.value(),
            p.gain.value(),
            p.wah.value(),
            p.delay.value(),
            p.reverb.value(),
        ];
        for (i, v) in values.iter().enumerate() {
            if self.last[i] == *v {
                continue;
            }
            let first = self.last[i].is_nan();
            self.last[i] = *v;
            if first {
                continue;
            }
            let slot = |a: &AtomicI32| {
                let s = a.load(o);
                (s >= 0).then_some(s as usize)
            };
            match i {
                0 => runner.apply(Target::Input, 0, *v),
                1 => runner.apply(Target::Output, 0, *v),
                2 => runner.apply(Target::Gate, 0, *v),
                3 if !self.shared.nam.load(o) => runner.apply(Target::Amp, 0, *v),
                4 => {
                    if let Some(s) = slot(&self.shared.wah) {
                        runner.apply(Target::Pre(s), 0, *v);
                    }
                }
                5 => {
                    if let Some(s) = slot(&self.shared.delay) {
                        runner.apply(Target::Post(s), 2, *v);
                    }
                }
                6 => {
                    if let Some(s) = slot(&self.shared.reverb) {
                        runner.apply(Target::Post(s), 4, *v);
                    }
                }
                _ => {}
            }
        }
    }
}

impl Plugin for RiffPlugin {
    const NAME: &'static str = "riff";
    const VENDOR: &'static str = "rodbove";
    const URL: &'static str = "https://github.com/rodbove/riff";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let shared = self.shared.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            editor::Editor::new(params, shared),
            |ctx, _| ctx.set_visuals(egui::Visuals::dark()),
            |ctx, _setter, state| {
                state.tick(ctx.input(|i| i.time));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        let mut view = std::mem::take(&mut state.view);
                        ui::draw(ui, state, &mut view);
                        state.view = view;
                    });
                state.store();
                ctx.request_repaint();
            },
        )
    }

    fn initialize(
        &mut self,
        _layout: &AudioIOLayout,
        config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let rate = config.sample_rate;
        let rig = self
            .params
            .rig
            .read()
            .map(|r| r.clone())
            .unwrap_or_default();
        let chain = Chain::build(&rig, rate, &mut Library::default());
        let (runner, remote) = live(chain);
        self.runner = Some(runner);
        if let Ok(mut slot) = self.shared.remote.lock() {
            *slot = Some(remote);
        }
        self.shared.rate.store(rate as u32, Ordering::Relaxed);
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        update_layout(&self.shared, &rig);
        let max = config.max_buffer_size as usize;
        self.input = vec![0.0; max];
        self.output = vec![[0.0; 2]; max];
        self.last = [f32::NAN; 7];
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.automate();
        let n = buffer.samples().min(self.input.len());
        let channels = buffer.as_slice();
        if channels.is_empty() || n == 0 {
            return ProcessStatus::Normal;
        }
        self.input[..n].copy_from_slice(&channels[0][..n]);
        let Some(runner) = self.runner.as_mut() else {
            return ProcessStatus::Normal;
        };
        runner.process(&self.input[..n], &mut self.output[..n]);
        for (i, f) in self.output[..n].iter().enumerate() {
            channels[0][i] = f[0];
            if channels.len() > 1 {
                channels[1][i] = f[1];
            }
        }
        ProcessStatus::Normal
    }
}

impl ClapPlugin for RiffPlugin {
    const CLAP_ID: &'static str = "com.rodbove.riff";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Metal guitar amps, cabs and pedals");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Mono,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for RiffPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"RiffGuitarAmpSim";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Distortion,
        Vst3SubCategory::Mono,
    ];
}

nih_export_clap!(RiffPlugin);
nih_export_vst3!(RiffPlugin);
