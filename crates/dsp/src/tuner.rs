const THRESHOLD: f32 = 0.12;
const MAX_HZ: f32 = 1400.0;
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub hz: f32,
    pub note: usize,
    pub octave: i32,
    pub cents: f32,
}

impl Reading {
    pub fn name(&self) -> &'static str {
        NAMES[self.note % 12]
    }
}

pub fn reading(hz: f32) -> Reading {
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    let nearest = midi.round();
    let n = nearest as i32;
    Reading {
        hz,
        note: n.rem_euclid(12) as usize,
        octave: n.div_euclid(12) - 1,
        cents: (midi - nearest) * 100.0,
    }
}

pub fn detect(buf: &[f32], rate: f32) -> Option<f32> {
    let w = buf.len() / 2;
    let min_tau = ((rate / MAX_HZ) as usize).max(2);
    if w <= min_tau + 2 {
        return None;
    }
    let level = buf.iter().fold(0.0f32, |a, v| a.max(v.abs()));
    if level < 1e-3 {
        return None;
    }
    let mut d = vec![0.0f32; w];
    for (tau, slot) in d.iter_mut().enumerate().skip(1) {
        let mut sum = 0.0;
        for j in 0..w {
            let diff = buf[j] - buf[j + tau];
            sum += diff * diff;
        }
        *slot = sum;
    }
    let mut cmnd = vec![1.0f32; w];
    let mut running = 0.0;
    for tau in 1..w {
        running += d[tau];
        cmnd[tau] = if running > 0.0 {
            d[tau] * tau as f32 / running
        } else {
            1.0
        };
    }
    let mut tau = min_tau;
    while tau < w - 1 {
        if cmnd[tau] < THRESHOLD {
            while tau + 1 < w - 1 && cmnd[tau + 1] < cmnd[tau] {
                tau += 1;
            }
            break;
        }
        tau += 1;
    }
    if tau >= w - 1 {
        return None;
    }
    let (a, b, c) = (cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]);
    let denom = a - 2.0 * b + c;
    let shift = if denom.abs() > 1e-12 {
        0.5 * (a - c) / denom
    } else {
        0.0
    };
    Some(rate / (tau as f32 + shift))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const RATE: f32 = 48000.0;

    fn tone(hz: f32, rich: bool) -> Vec<f32> {
        (0..4096)
            .map(|i| {
                let t = i as f32 / RATE;
                let base = (t * hz * TAU).sin();
                if rich {
                    base + 0.5 * (t * hz * 2.0 * TAU).sin() + 0.3 * (t * hz * 3.0 * TAU).sin()
                } else {
                    base
                }
            })
            .map(|v| v * 0.3)
            .collect()
    }

    fn cents(a: f32, b: f32) -> f32 {
        1200.0 * (a / b).log2()
    }

    mod when_reading_a_string {
        use super::*;

        #[test]
        fn finds_guitar_notes_within_a_cent() {
            for hz in [82.407, 110.0, 146.83, 196.0, 246.94, 329.63, 61.74] {
                for rich in [false, true] {
                    let got = detect(&tone(hz, rich), RATE).unwrap();
                    assert!(cents(got, hz).abs() < 1.0, "{hz} {rich} -> {got}");
                }
            }
        }

        #[test]
        fn ignores_silence() {
            assert_eq!(detect(&[0.0; 4096], RATE), None);
        }
    }

    mod when_naming {
        use super::*;

        #[test]
        fn names_the_note_and_offset() {
            let r = reading(82.407);
            assert_eq!(r.name(), "E");
            assert_eq!(r.octave, 2);
            assert!(r.cents.abs() < 0.1);
            let sharp = reading(442.0);
            assert_eq!(sharp.name(), "A");
            assert!((sharp.cents - 7.85).abs() < 0.1);
        }
    }
}
