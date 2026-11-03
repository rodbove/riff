use std::path::Path;

pub struct Audio {
    pub rate: u32,
    pub channels: usize,
    pub samples: Vec<f32>,
}

impl Audio {
    pub fn mono(&self) -> Vec<f32> {
        self.samples
            .chunks(self.channels.max(1))
            .map(|f| f[0])
            .collect()
    }
}

pub fn read(path: &Path) -> Result<Audio, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample.max(1) - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?
        }
    };
    Ok(Audio {
        rate: spec.sample_rate,
        channels: spec.channels.max(1) as usize,
        samples,
    })
}

pub fn write(path: &Path, rate: u32, channels: u16, samples: &[f32]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for s in samples {
        w.write_sample(*s).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_writing_and_reading_back {
        use super::*;

        #[test]
        fn keeps_the_rate_channels_and_samples() {
            let dir = std::env::temp_dir().join(format!("riff-wav-{}", std::process::id()));
            let path = dir.join("take.wav");
            let samples = [0.0, 0.5, -0.25, 1.0, 0.125, -1.0];
            write(&path, 48000, 2, &samples).unwrap();
            let audio = read(&path).unwrap();
            std::fs::remove_dir_all(&dir).ok();
            assert_eq!((audio.rate, audio.channels), (48000, 2));
            assert_eq!(audio.samples, samples);
            assert_eq!(audio.mono(), vec![0.0, -0.25, 0.125]);
        }
    }
}
