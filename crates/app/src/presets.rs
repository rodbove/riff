use std::path::{Path, PathBuf};

use riff_dsp::Rig;
use riff_dsp::preset::{self, USER};

use crate::settings;

pub fn dir() -> PathBuf {
    settings::library().join("presets")
}

pub struct Bank {
    pub items: Vec<Rig>,
}

impl Bank {
    pub fn load() -> Self {
        Self::with_user(read_dir(&dir()))
    }

    fn with_user(user: Vec<Rig>) -> Self {
        let mut items = preset::factory();
        items.extend(user);
        Self { items }
    }

    pub fn add(&mut self, rig: Rig) -> usize {
        match self
            .items
            .iter()
            .position(|r| r.category == USER && r.name == rig.name)
        {
            Some(i) => {
                self.items[i] = rig;
                i
            }
            None => {
                self.items.push(rig);
                self.items.len() - 1
            }
        }
    }
}

pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "preset".into() } else { out }
}

pub fn save(rig: &Rig) -> Result<PathBuf, String> {
    save_in(&dir(), rig)
}

fn save_in(dir: &Path, rig: &Rig) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", slug(&rig.name)));
    let text = serde_json::to_string_pretty(rig).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

fn read_dir(dir: &Path) -> Vec<Rig> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Rig> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .filter_map(|t| serde_json::from_str::<Rig>(&t).ok())
        .map(|mut r| {
            r.category = USER.into();
            r.fix();
            r
        })
        .collect();
    out.sort_by_key(|r| r.name.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_saving_a_user_preset {
        use super::*;

        #[test]
        fn loads_back_in_the_user_category() {
            let dir = std::env::temp_dir().join(format!("riff-presets-{}", std::process::id()));
            let rig = Rig {
                name: "My Tone!".into(),
                input: 3.0,
                ..Rig::default()
            };
            let path = save_in(&dir, &rig).unwrap();
            assert!(path.ends_with("my-tone.json"));
            let user = read_dir(&dir);
            std::fs::remove_dir_all(&dir).ok();
            let bank = Bank::with_user(user);
            let mine: Vec<usize> = (0..bank.items.len())
                .filter(|i| bank.items[*i].category == USER)
                .collect();
            assert_eq!(mine.len(), 1);
            assert_eq!(bank.items[mine[0]].input, 3.0);
        }

        #[test]
        fn replaces_a_preset_with_the_same_name() {
            let mut bank = Bank::with_user(Vec::new());
            let rig = Rig {
                name: "Mine".into(),
                category: USER.into(),
                ..Rig::default()
            };
            let a = bank.add(rig.clone());
            let b = bank.add(rig);
            assert_eq!(a, b);
        }
    }
}
