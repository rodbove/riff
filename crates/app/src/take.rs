use std::path::PathBuf;

use crate::settings;

pub struct Take {
    pub rate: u32,
    pub dry: Vec<f32>,
    pub wet: Vec<f32>,
    pub lost: u32,
}

impl Take {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            dry: Vec::with_capacity(rate as usize * 60),
            wet: Vec::with_capacity(rate as usize * 120),
            lost: 0,
        }
    }

    pub fn seconds(&self) -> f32 {
        self.dry.len() as f32 / self.rate.max(1) as f32
    }
}

pub fn folder() -> PathBuf {
    settings::library().join("recordings")
}

pub fn clean(name: &str) -> String {
    name.chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(unix)]
pub fn stamp() -> String {
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
pub fn stamp() -> String {
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
}
