use std::path::Path;
use std::sync::Arc;

use nam_rs::{Model, ModelConfig, NamModel};

use crate::biquad::{Biquad, Coefs};
use crate::fx::Mono;
use crate::library::Library;
use crate::param::{Param, Unit};
use crate::util::{Smooth, db};

const TARGET_DB: f32 = -18.0;

pub const PARAMS: &[Param] = &[
    Param::new("INPUT", -24.0, 24.0, 0.0, Unit::Db),
    Param::new("OUTPUT", -24.0, 24.0, 0.0, Unit::Db),
    Param::knob("BASS", 5.0),
    Param::knob("MID", 5.0),
    Param::knob("TREBLE", 5.0),
    Param::choice("QUALITY", &["FULL", "LITE"], 0.0),
];

pub const QUALITY: usize = 5;

pub struct Capture {
    pub name: String,
    pub rate: f32,
    pub includes_cab: bool,
    pub loudness: Option<f32>,
}

impl Library {
    pub fn nam(&mut self, path: &Path) -> Result<Arc<NamModel>, String> {
        if let Some(m) = self.nams.get(path) {
            return Ok(m.clone());
        }
        let model = Arc::new(NamModel::from_file(path).map_err(|e| e.to_string())?);
        self.nams.insert(path.to_path_buf(), model.clone());
        Ok(model)
    }

    pub fn nam_slimmable(&self, path: &Path) -> bool {
        self.nams
            .get(path)
            .is_some_and(|m| matches!(m.config, ModelConfig::Slimmable(_)))
    }
}

pub fn describe(model: &NamModel, path: &Path) -> Capture {
    let meta = model.metadata_typed();
    let name = meta
        .name
        .clone()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "capture".into())
        });
    Capture {
        name,
        rate: model.expected_sample_rate() as f32,
        includes_cab: model.includes_cab().unwrap_or(false),
        loudness: model.loudness(),
    }
}

pub struct NamAmp {
    rate: f32,
    model: Model,
    input: Smooth,
    output: Smooth,
    trim: f32,
    bass: Biquad,
    mid: Biquad,
    treble: Biquad,
}

impl NamAmp {
    pub fn new(
        path: &Path,
        values: &[f32],
        rate: f32,
        library: &mut Library,
        warnings: &mut Vec<String>,
    ) -> Result<Self, String> {
        let file = library.nam(path)?;
        let info = describe(&file, path);
        let model = Model::from_nam(&file).map_err(|e| e.to_string())?;
        if (info.rate - rate).abs() > 1.0 {
            warnings.push(format!(
                "{} was captured at {} Hz but riff runs at {} Hz. Set the rate to {} Hz in SETUP.",
                info.name, info.rate, rate, info.rate
            ));
        }
        if info.includes_cab {
            warnings.push(format!(
                "{} already includes a cabinet. Turn the CAB off.",
                info.name
            ));
        }
        let trim = info.loudness.map(|l| db(TARGET_DB - l)).unwrap_or(1.0);
        let mut amp = Self {
            rate,
            model,
            input: Smooth::new(1.0, 20.0, rate),
            output: Smooth::new(1.0, 20.0, rate),
            trim,
            bass: Biquad::default(),
            mid: Biquad::default(),
            treble: Biquad::default(),
        };
        for (i, p) in PARAMS.iter().enumerate() {
            amp.set(i, p.clamp(values.get(i).copied().unwrap_or(p.default)));
        }
        amp.input.snap(amp.input.target());
        amp.output.snap(amp.output.target());
        Ok(amp)
    }
}

impl NamAmp {
    fn finish(&mut self, y: f32) -> f32 {
        let y = self.treble.process(self.mid.process(self.bass.process(y)));
        y * self.trim * self.output.tick()
    }
}

impl Mono for NamAmp {
    fn set(&mut self, param: usize, value: f32) {
        let k = (value - 5.0) / 5.0;
        match param {
            0 => self.input.set(db(value)),
            1 => self.output.set(db(value)),
            2 => self.bass.set(Coefs::low_shelf(120.0, k * 12.0, self.rate)),
            3 => self.mid.set(Coefs::peak(700.0, 0.8, k * 10.0, self.rate)),
            QUALITY => {
                if let Some(s) = self.model.as_slimmable_mut() {
                    s.select(if value >= 0.5 { 0 } else { s.len() - 1 });
                }
            }
            _ => self
                .treble
                .set(Coefs::high_shelf(3000.0, k * 12.0, self.rate)),
        }
    }

    fn warmup(&self) -> usize {
        self.model.receptive_field()
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.model.process_sample(x * self.input.tick());
        self.finish(y)
    }

    fn process_block(&mut self, io: &mut [f32]) {
        for s in io.iter_mut() {
            *s *= self.input.tick();
        }
        self.model.process_buffer(io);
        for s in io.iter_mut() {
            *s = self.finish(*s);
        }
    }
}

#[cfg(test)]
pub(crate) const TINY_LSTM: &str = r#"{
    "version": "0.5.4", "architecture": "LSTM", "sample_rate": 48000,
    "config": { "input_size": 1, "hidden_size": 1, "num_layers": 1 },
    "weights": [1.0,0.0, 0.0,0.0, 2.0,0.0, 0.0,0.0, 0.0,0.0,0.0,0.0, 0.0, 0.0, 3.0, 0.5],
    "metadata": { "name": "tiny" }
}"#;

#[cfg(test)]
mod tests {
    use super::*;

    pub fn fixture(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("riff-nam-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tiny.nam");
        std::fs::write(&path, TINY_LSTM).unwrap();
        path
    }

    mod when_loading_a_capture {
        use super::*;

        #[test]
        fn runs_the_model() {
            let path = fixture("run");
            let mut lib = Library::default();
            let mut warnings = Vec::new();
            let mut amp = NamAmp::new(&path, &[], 48000.0, &mut lib, &mut warnings).unwrap();
            assert!(warnings.is_empty());
            let y = amp.process(0.5);
            assert!(y.is_finite() && y.abs() > 0.1);
        }

        #[test]
        fn warns_when_the_rate_does_not_match() {
            let path = fixture("rate");
            let mut lib = Library::default();
            let mut warnings = Vec::new();
            NamAmp::new(&path, &[], 44100.0, &mut lib, &mut warnings).unwrap();
            assert_eq!(warnings.len(), 1);
            assert!(warnings[0].contains("48000"));
        }

        mod with_an_a2_capture {
            use super::*;

            fn a2() -> std::path::PathBuf {
                let small = TINY_LSTM.replace("2.0,0.0", "0.5,0.0");
                let json = format!(
                    r#"{{"version":"0.7.0","architecture":"SlimmableContainer","sample_rate":48000,
                    "config":{{"submodels":[{{"max_value":0.5,"model":{small}}},{{"max_value":1.0,"model":{TINY_LSTM}}}]}},
                    "weights":[],"metadata":{{"name":"a2"}}}}"#
                );
                let path = fixture("a2").with_file_name("a2.nam");
                std::fs::write(&path, json).unwrap();
                path
            }

            #[test]
            fn is_slimmable() {
                let path = a2();
                let mut lib = Library::default();
                NamAmp::new(&path, &[], 48000.0, &mut lib, &mut Vec::new()).unwrap();
                assert!(lib.nam_slimmable(&path));
            }

            #[test]
            fn runs_the_small_model_on_lite() {
                let path = a2();
                let mut lib = Library::default();
                let mut full = NamAmp::new(&path, &[], 48000.0, &mut lib, &mut Vec::new()).unwrap();
                let mut values: Vec<f32> = PARAMS.iter().map(|p| p.default).collect();
                values[QUALITY] = 1.0;
                let mut lite = NamAmp::new(&path, &values, 48000.0, &mut lib, &mut Vec::new()).unwrap();
                let (f, l) = (full.process(0.5), lite.process(0.5));
                assert!((f - l).abs() > 1e-3, "full {f} lite {l}");
            }
        }

        mod with_a_capture_without_submodels {
            use super::*;

            #[test]
            fn is_not_slimmable() {
                let path = fixture("slim");
                let mut lib = Library::default();
                NamAmp::new(&path, &[], 48000.0, &mut lib, &mut Vec::new()).unwrap();
                assert!(!lib.nam_slimmable(&path));
            }

            #[test]
            fn ignores_the_quality_setting() {
                let path = fixture("quality");
                let mut lib = Library::default();
                let mut amp = NamAmp::new(&path, &[], 48000.0, &mut lib, &mut Vec::new()).unwrap();
                let full = amp.process(0.5);
                amp.set(QUALITY, 1.0);
                assert!(amp.process(0.5).is_finite() && full.is_finite());
            }
        }

        #[test]
        fn reports_a_missing_file() {
            let mut lib = Library::default();
            let mut warnings = Vec::new();
            let r = NamAmp::new(
                Path::new("/nope.nam"),
                &[],
                48000.0,
                &mut lib,
                &mut warnings,
            );
            assert!(r.is_err());
        }
    }
}
