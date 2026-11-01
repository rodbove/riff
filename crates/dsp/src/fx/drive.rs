use crate::biquad::{Biquad, Coefs};
use crate::clip::{diode, hard, tanh};
use crate::fx::Mono;
use crate::oversample::Oversampler;

const FACTOR: usize = Oversampler::FACTOR;
use crate::util::{OnePole, Smooth, db};

fn knob(v: f32) -> f32 {
    (v / 10.0).clamp(0.0, 1.0)
}

pub struct Boost {
    rate: f32,
    gain: Smooth,
    tight: Biquad,
}

impl Boost {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            gain: Smooth::new(1.0, 20.0, rate),
            tight: Biquad::default(),
        }
    }
}

impl Mono for Boost {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.gain.set(db(value)),
            _ => self.tight.set(Coefs::highpass(value, 0.707, self.rate)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        self.tight.process(x) * self.gain.tick()
    }
}

pub struct Overdrive {
    rate: f32,
    os: Oversampler,
    tight: OnePole,
    branch: OnePole,
    tone: OnePole,
    top: OnePole,
    gain: Smooth,
    level: Smooth,
}

impl Overdrive {
    pub fn new(rate: f32) -> Self {
        let fast = rate * FACTOR as f32;
        Self {
            rate,
            os: Oversampler::new(),
            tight: OnePole::new(100.0, rate),
            branch: OnePole::new(720.0, fast),
            tone: OnePole::new(2000.0, rate),
            top: OnePole::new(6000.0, rate),
            gain: Smooth::new(1.0, 20.0, fast),
            level: Smooth::new(1.0, 20.0, rate),
        }
    }
}

impl Mono for Overdrive {
    fn set(&mut self, param: usize, value: f32) {
        let t = knob(value);
        match param {
            0 => self.gain.set(db(20.0 + 22.0 * t)),
            1 => self
                .tone
                .tune(700.0 * (5000.0f32 / 700.0).powf(t), self.rate),
            _ => self.level.set(db((value - 5.0) * 2.4)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let x = self.tight.high(x);
        let Overdrive {
            os, branch, gain, ..
        } = self;
        let y = os.process(x, |s| s + diode(branch.high(s) * gain.tick()) * 0.5);
        self.top.low(self.tone.low(y)) * self.level.tick()
    }
}

pub struct Distortion {
    rate: f32,
    os: Oversampler,
    input: OnePole,
    gbw: OnePole,
    filter: OnePole,
    gain: Smooth,
    level: Smooth,
}

impl Distortion {
    pub fn new(rate: f32) -> Self {
        let fast = rate * FACTOR as f32;
        Self {
            rate,
            os: Oversampler::new(),
            input: OnePole::new(60.0, rate),
            gbw: OnePole::new(20000.0, fast),
            filter: OnePole::new(5000.0, rate),
            gain: Smooth::new(1.0, 20.0, fast),
            level: Smooth::new(1.0, 20.0, rate),
        }
    }
}

impl Mono for Distortion {
    fn set(&mut self, param: usize, value: f32) {
        let t = knob(value);
        match param {
            0 => {
                let g = 6.0 + 54.0 * t;
                self.gain.set(db(g));
                let fc = (1.0e6 / db(g)).clamp(2000.0, 20000.0);
                self.gbw.tune(fc, self.rate * FACTOR as f32);
            }
            1 => self
                .filter
                .tune(22000.0 * (475.0f32 / 22000.0).powf(t), self.rate),
            _ => self.level.set(db((value - 5.0) * 2.4)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let x = self.input.high(x);
        let Distortion { os, gbw, gain, .. } = self;
        let y = os.process(x, |s| {
            hard(diode(gbw.low(s * gain.tick()) * 1.5), 1.0) * 0.6
        });
        self.filter.low(y) * self.level.tick()
    }
}

pub struct Swedish {
    rate: f32,
    os: Oversampler,
    input: OnePole,
    low: Biquad,
    mid: Biquad,
    high: Biquad,
    top: Biquad,
    gain: Smooth,
    level: Smooth,
}

impl Swedish {
    pub fn new(rate: f32) -> Self {
        let fast = rate * FACTOR as f32;
        Self {
            rate,
            os: Oversampler::new(),
            input: OnePole::new(100.0, rate),
            low: Biquad::default(),
            mid: Biquad::new(Coefs::peak(400.0, 0.9, -6.0, rate)),
            high: Biquad::default(),
            top: Biquad::new(Coefs::lowpass(6000.0, 0.707, rate)),
            gain: Smooth::new(1.0, 20.0, fast),
            level: Smooth::new(1.0, 20.0, rate),
        }
    }
}

impl Mono for Swedish {
    fn set(&mut self, param: usize, value: f32) {
        let t = knob(value);
        match param {
            0 => self.level.set(db((value - 5.0) * 2.4)),
            1 => self.low.set(Coefs::peak(90.0, 1.0, 18.0 * t, self.rate)),
            2 => self.high.set(Coefs::peak(1300.0, 1.5, 16.0 * t, self.rate)),
            _ => self.gain.set(db(20.0 + 30.0 * t)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let x = self.input.high(x);
        let Swedish { os, gain, .. } = self;
        let y = os.process(x, |s| diode(tanh(s * gain.tick()) * 2.0) * 0.4);
        let y = self.high.process(self.mid.process(self.low.process(y)));
        self.top.process(y) * self.level.tick()
    }
}
