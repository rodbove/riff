use crate::fx::Mono;
use crate::util::{Smooth, db, to_db};

pub struct Compressor {
    rate: f32,
    threshold: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup: Smooth,
    blend: Smooth,
    env: f32,
}

fn coef(ms: f32, rate: f32) -> f32 {
    1.0 - (-1.0 / (ms.max(0.05) * 0.001 * rate)).exp()
}

impl Compressor {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            threshold: -24.0,
            ratio: 4.0,
            attack: coef(5.0, rate),
            release: coef(150.0, rate),
            makeup: Smooth::new(1.0, 20.0, rate),
            blend: Smooth::new(1.0, 20.0, rate),
            env: 0.0,
        }
    }

    pub fn reduction_db(&self) -> f32 {
        let level = to_db(self.env);
        let over = level - self.threshold;
        if over > 0.0 {
            over * (1.0 - 1.0 / self.ratio)
        } else {
            0.0
        }
    }
}

impl Mono for Compressor {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.threshold = value,
            1 => self.ratio = value.max(1.0),
            2 => self.attack = coef(value, self.rate),
            3 => self.release = coef(value, self.rate),
            4 => self.makeup.set(db(value)),
            _ => self.blend.set(value.clamp(0.0, 1.0)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let level = x.abs();
        let c = if level > self.env {
            self.attack
        } else {
            self.release
        };
        self.env += (level - self.env) * c;
        let wet = x * db(-self.reduction_db()) * self.makeup.tick();
        let b = self.blend.tick();
        x + (wet - x) * b
    }
}
