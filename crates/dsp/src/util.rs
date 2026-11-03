use std::f32::consts::TAU;

pub fn db(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn to_db(gain: f32) -> f32 {
    20.0 * gain.abs().max(1e-9).log10()
}

fn coef(ms: f32, rate: f32) -> f32 {
    1.0 - (-1.0 / (ms.max(0.01) * 0.001 * rate)).exp()
}

#[derive(Clone, Copy, Debug)]
pub struct Smooth {
    value: f32,
    target: f32,
    coef: f32,
}

impl Smooth {
    pub fn new(value: f32, ms: f32, rate: f32) -> Self {
        Self {
            value,
            target: value,
            coef: coef(ms, rate),
        }
    }

    pub fn set(&mut self, target: f32) {
        self.target = target;
    }

    pub fn snap(&mut self, value: f32) {
        self.value = value;
        self.target = value;
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn tick(&mut self) -> f32 {
        self.value += (self.target - self.value) * self.coef;
        self.value
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DcBlock {
    x1: f32,
    y1: f32,
    r: f32,
}

impl DcBlock {
    pub fn new(rate: f32) -> Self {
        Self {
            x1: 0.0,
            y1: 0.0,
            r: 1.0 - TAU * 10.0 / rate,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + self.r * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    a: f32,
    z: f32,
}

impl OnePole {
    pub fn new(hz: f32, rate: f32) -> Self {
        let mut p = Self::default();
        p.tune(hz, rate);
        p
    }

    pub fn tune(&mut self, hz: f32, rate: f32) {
        self.a = 1.0 - (-TAU * hz.clamp(1.0, rate * 0.49) / rate).exp();
    }

    pub fn low(&mut self, x: f32) -> f32 {
        self.z += self.a * (x - self.z);
        self.z
    }

    pub fn high(&mut self, x: f32) -> f32 {
        x - self.low(x)
    }

    pub fn reset(&mut self) {
        self.z = 0.0;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Follower {
    attack: f32,
    release: f32,
    env: f32,
}

impl Follower {
    pub fn new(attack_ms: f32, release_ms: f32, rate: f32) -> Self {
        Self {
            attack: coef(attack_ms, rate),
            release: coef(release_ms, rate),
            env: 0.0,
        }
    }

    pub fn set_release(&mut self, ms: f32, rate: f32) {
        self.release = coef(ms, rate);
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let level = x.abs();
        let c = if level > self.env {
            self.attack
        } else {
            self.release
        };
        self.env += (level - self.env) * c;
        self.env
    }

    pub fn level(&self) -> f32 {
        self.env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_converting_levels {
        use super::*;

        #[test]
        fn round_trips_decibels() {
            assert!((db(-6.0) - 0.501).abs() < 1e-3);
            assert!((to_db(db(12.0)) - 12.0).abs() < 1e-4);
        }
    }

    mod when_smoothing {
        use super::*;

        #[test]
        fn reaches_the_target() {
            let mut s = Smooth::new(0.0, 5.0, 48000.0);
            s.set(1.0);
            for _ in 0..4800 {
                s.tick();
            }
            assert!((s.tick() - 1.0).abs() < 1e-3);
        }
    }

    mod when_blocking_dc {
        use super::*;

        #[test]
        fn removes_an_offset() {
            let mut d = DcBlock::new(48000.0);
            let mut y = 0.0;
            for _ in 0..48000 {
                y = d.process(0.5);
            }
            assert!(y.abs() < 1e-3);
        }
    }
}
