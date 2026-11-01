use std::path::Path;
use std::sync::Arc;

use nam_rs::{Model, NamModel};

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
];

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

impl Mono for NamAmp {
    fn set(&mut self, param: usize, value: f32) {
        let k = (value - 5.0) / 5.0;
        match param {
            0 => self.input.set(db(value)),
            1 => self.output.set(db(value)),
            2 => self.bass.set(Coefs::low_shelf(120.0, k * 12.0, self.rate)),
            3 => self.mid.set(Coefs::peak(700.0, 0.8, k * 10.0, self.rate)),
            _ => self
                .treble
                .set(Coefs::high_shelf(3000.0, k * 12.0, self.rate)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.model.process_sample(x * self.input.tick());
        let y = self.treble.process(self.mid.process(self.bass.process(y)));
        y * self.trim * self.output.tick()
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
