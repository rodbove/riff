use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const RATES: [u32; 3] = [44100, 48000, 96000];
pub const BUFFERS: [u32; 5] = [32, 64, 128, 256, 512];

/// How riff talks to the audio interface. Linux offers JACK (normally PipeWire's JACK layer) and
/// plain ALSA; other systems use their native API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backend {
    #[cfg_attr(target_os = "linux", default)]
    Jack,
    #[cfg_attr(not(target_os = "linux"), default)]
    System,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Backend::Jack => "PIPEWIRE / JACK",
            Backend::System if cfg!(target_os = "linux") => "ALSA",
            Backend::System => "SYSTEM",
        }
    }

    pub fn all() -> &'static [Backend] {
        if cfg!(target_os = "linux") {
            &[Backend::Jack, Backend::System]
        } else {
            &[Backend::System]
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub backend: Backend,
    pub input: Option<String>,
    pub channel: usize,
    pub output: Option<String>,
    pub rate: u32,
    pub buffer: u32,
    pub midi: Option<String>,
    pub fast_cores: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: Backend::default(),
            input: None,
            channel: 1,
            output: None,
            rate: 48000,
            buffer: 128,
            midi: None,
            fast_cores: true,
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

/// The user's music folder: `XDG_MUSIC_DIR` from `user-dirs.dirs` on Linux, else `~/Music`.
fn music() -> PathBuf {
    if !cfg!(target_os = "macos") {
        let file = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"))
            .join("user-dirs.dirs");
        if let Some(dir) = std::fs::read_to_string(file)
            .ok()
            .and_then(|t| xdg_music(&t, &home()))
        {
            return dir;
        }
    }
    home().join("Music")
}

fn xdg_music(text: &str, home: &std::path::Path) -> Option<PathBuf> {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("XDG_MUSIC_DIR="))?;
    let value = line["XDG_MUSIC_DIR=".len()..].trim_matches('"');
    let path = match value.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(value),
    };
    (path.is_absolute() && path != home).then_some(path)
}

pub fn library() -> PathBuf {
    music().join("riff")
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

#[cfg(test)]
mod tests {
    use super::*;

    mod when_finding_the_music_folder {
        use super::*;

        #[test]
        fn reads_the_xdg_user_dir() {
            let home = std::path::Path::new("/home/me");
            let text =
                "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_MUSIC_DIR=\"$HOME/Música\"\n";
            assert_eq!(
                xdg_music(text, home),
                Some(PathBuf::from("/home/me/Música"))
            );
            assert_eq!(
                xdg_music("XDG_MUSIC_DIR=\"/data/music\"", home),
                Some(PathBuf::from("/data/music"))
            );
        }

        #[test]
        fn ignores_a_music_dir_that_is_just_home() {
            let home = std::path::Path::new("/home/me");
            assert_eq!(xdg_music("XDG_MUSIC_DIR=\"$HOME/\"", home), None);
            assert_eq!(xdg_music("", home), None);
        }
    }

    mod when_reading_old_settings {
        use super::*;

        #[test]
        fn uses_the_default_backend() {
            let s: Settings = serde_json::from_str(r#"{"rate":44100}"#).unwrap();
            assert_eq!(s.backend, Backend::default());
            assert_eq!(s.rate, 44100);
        }
    }
}
