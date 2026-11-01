use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const RATES: [u32; 3] = [44100, 48000, 96000];
pub const BUFFERS: [u32; 5] = [32, 64, 128, 256, 512];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub input: Option<String>,
    pub channel: usize,
    pub output: Option<String>,
    pub rate: u32,
    pub buffer: u32,
    pub midi: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            input: None,
            channel: 1,
            output: None,
            rate: 48000,
            buffer: 128,
            midi: None,
        }
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn config_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support/riff")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"))
            .join("riff")
    }
}

pub fn library() -> PathBuf {
    home().join("Music/riff")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(config_dir().join("settings.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = config_dir();
        if std::fs::create_dir_all(&dir).is_ok()
            && let Ok(text) = serde_json::to_string_pretty(self)
        {
            let _ = std::fs::write(dir.join("settings.json"), text);
        }
    }
}
