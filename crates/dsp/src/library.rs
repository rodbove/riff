use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nam_rs::NamModel;

pub const IR_SECONDS: f32 = 0.5;

#[derive(Default)]
pub struct Library {
    irs: HashMap<(PathBuf, u32), Arc<[f32]>>,
    pub(crate) nams: HashMap<PathBuf, Arc<NamModel>>,
}

impl Library {
    pub fn ir(&mut self, path: &Path, rate: f32) -> Result<Arc<[f32]>, String> {
        let key = (path.to_path_buf(), rate as u32);
        if let Some(ir) = self.irs.get(&key) {
            return Ok(ir.clone());
        }
        let ir: Arc<[f32]> = load_ir(path, rate)?.into();
        self.irs.insert(key, ir.clone());
        Ok(ir)
    }
}

pub fn load_ir(path: &Path, rate: f32) -> Result<Vec<f32>, String> {
    let audio = crate::wav::read(path)?;
    let mono = audio.mono();
    if mono.is_empty() {
        return Err("empty impulse response".into());
    }
    let mut ir = resample(&mono, audio.rate as f32, rate);
    ir.truncate((IR_SECONDS * rate) as usize);
    normalize(&mut ir);
    Ok(ir)
}

pub fn resample(x: &[f32], from: f32, to: f32) -> Vec<f32> {
    if (from - to).abs() < 0.5 {
        return x.to_vec();
    }
    let ratio = from / to;
    let n = ((x.len() as f32) / ratio).ceil() as usize;
    (0..n)
        .map(|i| {
            let pos = i as f32 * ratio;
            let j = pos.floor() as usize;
            let t = pos - j as f32;
            let a = x.get(j).copied().unwrap_or(0.0);
            let b = x.get(j + 1).copied().unwrap_or(0.0);
            a + (b - a) * t
        })
        .collect()
}

pub fn normalize(ir: &mut [f32]) {
    let energy = ir.iter().map(|v| v * v).sum::<f32>().sqrt();
    if energy > 1e-9 {
        for v in ir {
            *v /= energy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_loading_an_impulse_response {
        use super::*;

        #[test]
        fn reads_resamples_and_normalizes_a_wav() {
            let dir = std::env::temp_dir().join(format!("riff-ir-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("cab.wav");
            let spec = hound::WavSpec {
                channels: 2,
                sample_rate: 44100,
                bits_per_sample: 24,
                sample_format: hound::SampleFormat::Int,
            };
            let mut w = hound::WavWriter::create(&path, spec).unwrap();
            for i in 0..441 {
                let v = if i == 0 { 4_000_000 } else { 100_000 };
                w.write_sample(v).unwrap();
                w.write_sample(0).unwrap();
            }
            w.finalize().unwrap();
            let mut lib = Library::default();
            let ir = lib.ir(&path, 48000.0).unwrap();
            std::fs::remove_dir_all(&dir).ok();
            assert!((ir.len() as i32 - 480).abs() <= 1);
            let energy: f32 = ir.iter().map(|v| v * v).sum();
            assert!((energy - 1.0).abs() < 1e-3);
            assert!(ir[0] > ir[10]);
        }
    }
}
