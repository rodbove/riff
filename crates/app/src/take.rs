use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use riff_dsp::{Library, Rig, chain, wav};

use crate::settings;

pub struct Take {
    pub preset: String,
    pub stamp: String,
    pub rate: u32,
    pub dry: Vec<f32>,
    pub wet: Vec<f32>,
    pub lost: u32,
}

impl Take {
    pub fn new(preset: &str, rate: u32) -> Self {
        Self {
            preset: preset.to_string(),
            stamp: stamp(),
            rate,
            dry: Vec::with_capacity(rate as usize * 60),
            wet: Vec::with_capacity(rate as usize * 120),
            lost: 0,
        }
    }

    pub fn seconds(&self) -> f32 {
        self.dry.len() as f32 / self.rate.max(1) as f32
    }

    pub fn save(self, done: Sender<String>) {
        std::thread::spawn(move || {
            let dir = folder();
            let base = format!("{} {}", self.stamp, clean(&self.preset));
            let dry = dir.join(format!("{base} DI.wav"));
            let wet = dir.join(format!("{base} riff.wav"));
            let result = wav::write(&dry, self.rate, 1, &self.dry)
                .and_then(|_| wav::write(&wet, self.rate, 2, &self.wet));
            let _ = done.send(match result {
                Ok(()) if self.lost > 0 => format!(
                    "saved {base}, but {} samples were lost. Keep riff open while you record.",
                    self.lost
                ),
                Ok(()) => format!("saved {base} DI.wav and riff.wav"),
                Err(e) => format!("could not save the take: {e}"),
            });
        });
    }
}

pub fn folder() -> PathBuf {
    settings::library().join("recordings")
}

pub fn reamp(path: PathBuf, rig: Rig, done: Sender<String>) {
    std::thread::spawn(move || {
        let _ = done.send(match render(&path, &rig) {
            Ok(name) => format!("saved {name}"),
            Err(e) => format!("could not reamp: {e}"),
        });
    });
}

fn render(path: &Path, rig: &Rig) -> Result<String, String> {
    let audio = wav::read(path)?;
    let out = chain::render(
        rig,
        audio.rate as f32,
        &audio.mono(),
        &mut Library::default(),
    );
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "take".into());
    let stem = stem.strip_suffix(" DI").unwrap_or(&stem);
    let name = format!("{stem} - {} riff.wav", clean(&rig.name));
    let interleaved: Vec<f32> = out.iter().flatten().copied().collect();
    wav::write(&folder().join(&name), audio.rate, 2, &interleaved)?;
    Ok(name)
}

fn clean(name: &str) -> String {
    name.chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(unix)]
fn stamp() -> String {
    let tm = unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::MaybeUninit::<libc::tm>::zeroed();
        libc::localtime_r(&t, tm.as_mut_ptr());
        tm.assume_init()
    };
    format!(
        "{:04}-{:02}-{:02} {:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

#[cfg(not(unix))]
fn stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_naming_files {
        use super::*;

        #[test]
        fn replaces_characters_that_paths_do_not_allow() {
            assert_eq!(clean("Broken, Beat & Scarred"), "Broken, Beat & Scarred");
            assert_eq!(clean("AC/DC: Back?"), "AC-DC- Back-");
        }
    }

    mod when_reamping_a_take {
        use super::*;

        #[test]
        fn renders_every_sample_through_the_preset() {
            let dir = std::env::temp_dir().join(format!("riff-reamp-{}", std::process::id()));
            let input = dir.join("2026-11-03 120000 As I Am DI.wav");
            let tone: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin() * 0.2).collect();
            wav::write(&input, 48000, 1, &tone).unwrap();
            let audio = wav::read(&input).unwrap();
            let out = chain::render(
                &Rig::default(),
                48000.0,
                &audio.mono(),
                &mut Library::default(),
            );
            std::fs::remove_dir_all(&dir).ok();
            assert_eq!(out.len(), tone.len());
            assert!(out.iter().all(|f| f[0].is_finite() && f[1].is_finite()));
            assert!(out.iter().any(|f| f[0].abs() > 0.001));
        }
    }
}
