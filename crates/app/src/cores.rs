use std::path::Path;

pub const UNKNOWN: u32 = u32::MAX;

pub fn max_clocks(root: &Path) -> Vec<(usize, u32)> {
    let Ok(dir) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out: Vec<(usize, u32)> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let cpu = name.strip_prefix("cpu")?.parse().ok()?;
            let khz = std::fs::read_to_string(e.path().join("cpufreq/cpuinfo_max_freq"))
                .ok()?
                .trim()
                .parse()
                .ok()?;
            Some((cpu, khz))
        })
        .collect();
    out.sort_unstable();
    out
}

pub fn fastest(cores: &[(usize, u32)]) -> Vec<usize> {
    let Some(top) = cores.iter().map(|c| c.1).max() else {
        return Vec::new();
    };
    if cores.iter().all(|c| c.1 == top) {
        return Vec::new();
    }
    cores
        .iter()
        .filter(|c| c.1 == top)
        .map(|c| c.0)
        .collect()
}

pub fn fast_cores() -> Vec<usize> {
    fastest(&max_clocks(Path::new("/sys/devices/system/cpu")))
}

pub fn pin(cpus: &[usize]) -> bool {
    if cpus.is_empty() {
        return false;
    }
    let mut set: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    for &c in cpus {
        unsafe { libc::CPU_SET(c, &mut set) };
    }
    unsafe { libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) == 0 }
}

pub fn current() -> u32 {
    let cpu = unsafe { libc::sched_getcpu() };
    if cpu < 0 { UNKNOWN } else { cpu as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_finding_the_fastest_cores {
        use super::*;

        mod with_a_hybrid_cpu {
            use super::*;

            #[test]
            fn returns_the_cores_with_the_highest_clock() {
                let cores = [(0, 5000), (1, 5000), (2, 3700), (3, 3700)];
                assert_eq!(fastest(&cores), vec![0, 1]);
            }
        }

        mod with_equal_cores {
            use super::*;

            #[test]
            fn returns_nothing() {
                assert!(fastest(&[(0, 4000), (1, 4000)]).is_empty());
            }
        }

        mod without_clock_data {
            use super::*;

            #[test]
            fn returns_nothing() {
                assert!(fastest(&[]).is_empty());
            }
        }
    }

    mod when_reading_the_clocks {
        use super::*;

        #[test]
        fn reads_each_cpu_folder() {
            let root = std::env::temp_dir().join(format!("riff-cores-{}", std::process::id()));
            for (cpu, khz) in [(0, "5000000\n"), (10, "3700000\n")] {
                let dir = root.join(format!("cpu{cpu}/cpufreq"));
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("cpuinfo_max_freq"), khz).unwrap();
            }
            std::fs::create_dir_all(root.join("cpuidle")).unwrap();
            assert_eq!(max_clocks(&root), vec![(0, 5000000), (10, 3700000)]);
            let _ = std::fs::remove_dir_all(&root);
        }
    }
}
