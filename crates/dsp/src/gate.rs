use crate::util::{Follower, db};

const HYSTERESIS_DB: f32 = 6.0;
const HOLD_MS: f32 = 30.0;
const ATTACK_MS: f32 = 0.5;

#[derive(Clone, Debug)]
pub struct Gate {
    rate: f32,
    open_level: f32,
    close_level: f32,
    follower: Follower,
    open: bool,
    hold: u32,
    hold_len: u32,
    gain: f32,
    attack: f32,
    release: f32,
}

impl Gate {
    pub fn new(threshold_db: f32, release_ms: f32, rate: f32) -> Self {
        let mut g = Self {
            rate,
            open_level: 0.0,
            close_level: 0.0,
            follower: Follower::new(0.1, 5.0, rate),
            open: false,
            hold: 0,
            hold_len: (HOLD_MS * 0.001 * rate) as u32,
            gain: 0.0,
            attack: 0.0,
            release: 0.0,
        };
        g.set_threshold(threshold_db);
        g.set_release(release_ms);
        g.attack = 1.0 - (-1.0 / (ATTACK_MS * 0.001 * rate)).exp();
        g
    }

    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.open_level = db(threshold_db);
        self.close_level = db(threshold_db - HYSTERESIS_DB);
    }

    pub fn set_release(&mut self, ms: f32) {
        self.release = 1.0 - (-1.0 / (ms.max(1.0) * 0.001 * self.rate)).exp();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn process(&mut self, key: f32, x: f32) -> f32 {
        let env = self.follower.process(key);
        if env > self.open_level {
            self.open = true;
            self.hold = self.hold_len;
        } else if env < self.close_level {
            if self.hold > 0 {
                self.hold -= 1;
            } else {
                self.open = false;
            }
        }
        let (target, c) = if self.open {
            (1.0, self.attack)
        } else {
            (0.0, self.release)
        };
        self.gain += (target - self.gain) * c;
        x * self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const RATE: f32 = 48000.0;

    fn run(g: &mut Gate, level: f32, n: usize) -> f32 {
        let mut last = 0.0f32;
        for i in 0..n {
            let x = (i as f32 * 200.0 * TAU / RATE).sin() * level;
            last = last.max(g.process(x, x).abs());
        }
        last
    }

    mod when_gating {
        use super::*;

        #[test]
        fn opens_for_playing_and_closes_for_hiss() {
            let mut g = Gate::new(-40.0, 50.0, RATE);
            assert!(run(&mut g, 0.3, 4800) > 0.25);
            assert!(g.is_open());
            run(&mut g, 0.001, 48000);
            assert!(!g.is_open());
            assert!(run(&mut g, 0.001, 4800) < 1e-4);
        }

        #[test]
        fn stays_open_inside_the_hysteresis_band() {
            let mut g = Gate::new(-40.0, 50.0, RATE);
            run(&mut g, 0.3, 4800);
            run(&mut g, db(-43.0), 24000);
            assert!(g.is_open());
            let mut closed = Gate::new(-40.0, 50.0, RATE);
            run(&mut closed, db(-43.0), 24000);
            assert!(!closed.is_open());
        }
    }
}
