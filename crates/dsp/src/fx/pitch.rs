use crate::biquad::{Biquad, Coefs};
use crate::fx::line::Line;
use crate::fx::{Mono, Stereo};
use crate::util::{Smooth, db};

const WINDOW_MS: f32 = 50.0;
pub const INTERVALS: [f32; 9] = [-12.0, -7.0, -5.0, -3.0, 3.0, 4.0, 5.0, 7.0, 12.0];

pub struct Shifter {
    line: Line,
    window: f32,
    phase: f32,
    ratio: f32,
}

impl Shifter {
    pub fn new(rate: f32) -> Self {
        let window = WINDOW_MS * 0.001 * rate;
        Self {
            line: Line::new(window as usize + 8),
            window,
            phase: 0.0,
            ratio: 1.0,
        }
    }

    pub fn set_semitones(&mut self, semis: f32) {
        self.ratio = 2f32.powf(semis / 12.0);
    }

    pub fn process(&mut self, x: f32) -> f32 {
        self.line.push(x);
        if (self.ratio - 1.0).abs() < 1e-6 {
            return x;
        }
        self.phase = (self.phase + (1.0 - self.ratio) / self.window).rem_euclid(1.0);
        let a = self.phase;
        let b = (a + 0.5).fract();
        let tap = |p: f32| self.line.read(1.0 + p * self.window) * (1.0 - (2.0 * p - 1.0).abs());
        tap(a) + tap(b)
    }
}

pub struct Tune {
    shifter: Shifter,
    mix: Smooth,
}

impl Tune {
    pub fn new(rate: f32) -> Self {
        Self {
            shifter: Shifter::new(rate),
            mix: Smooth::new(1.0, 20.0, rate),
        }
    }
}

impl Mono for Tune {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.shifter.set_semitones(value.round()),
            _ => self.mix.set(value.clamp(0.0, 1.0)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.shifter.process(x);
        let m = self.mix.tick();
        x + (y - x) * m
    }
}

pub struct Harmony {
    rate: f32,
    shifter: Shifter,
    line: Line,
    delay: f32,
    mix: Smooth,
    width: f32,
}

impl Harmony {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            shifter: Shifter::new(rate),
            line: Line::new((0.05 * rate) as usize),
            delay: 15.0,
            mix: Smooth::new(0.7, 20.0, rate),
            width: 0.8,
        }
    }
}

impl Stereo for Harmony {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => {
                let i = (value.round().max(0.0) as usize).min(INTERVALS.len() - 1);
                self.shifter.set_semitones(INTERVALS[i]);
            }
            1 => self.mix.set(value.clamp(0.0, 1.0)),
            2 => self.width = value.clamp(0.0, 1.0),
            _ => self.delay = value.clamp(0.0, 40.0),
        }
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mono = 0.5 * (x[0] + x[1]);
        let h = self.shifter.process(mono);
        self.line.push(h);
        let h = self.line.read(1.0 + self.delay * 0.001 * self.rate) * self.mix.tick();
        let w = self.width;
        [x[0] + h * (1.0 - w) * 0.7, x[1] + h * (0.7 + 0.3 * w)]
    }
}

pub struct Tremolo {
    rate: f32,
    phase: f32,
    step: f32,
    depth: f32,
    square: bool,
    spread: f32,
}

impl Tremolo {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            phase: 0.0,
            step: 0.0,
            depth: 0.6,
            square: false,
            spread: 0.0,
        }
    }
}

impl Stereo for Tremolo {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.step = value / self.rate,
            1 => self.depth = value.clamp(0.0, 1.0),
            2 => self.square = value > 0.5,
            _ => self.spread = value.clamp(0.0, 1.0),
        }
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let phase = self.phase;
        self.phase = (self.phase + self.step).fract();
        std::array::from_fn(|ch| {
            let p = phase + if ch == 1 { self.spread * 0.5 } else { 0.0 };
            let s = (p * std::f32::consts::TAU).sin();
            let s = if self.square {
                (s * 8.0).clamp(-1.0, 1.0)
            } else {
                s
            };
            x[ch] * (1.0 - self.depth * (0.5 + 0.5 * s))
        })
    }
}

pub struct Acoustic {
    filters: [Biquad; 6],
    level: Smooth,
    rate: f32,
}

impl Acoustic {
    pub fn new(rate: f32) -> Self {
        Self {
            filters: [
                Biquad::new(Coefs::highpass(80.0, 0.707, rate)),
                Biquad::default(),
                Biquad::new(Coefs::peak(220.0, 1.2, 3.0, rate)),
                Biquad::new(Coefs::peak(600.0, 1.0, -5.0, rate)),
                Biquad::default(),
                Biquad::new(Coefs::high_shelf(6000.0, 3.0, rate)),
            ],
            level: Smooth::new(1.0, 20.0, rate),
            rate,
        }
    }
}

impl Mono for Acoustic {
    fn set(&mut self, param: usize, value: f32) {
        let k = (value - 5.0) / 5.0;
        match param {
            0 => self.filters[1].set(Coefs::peak(110.0, 1.4, 6.0 + 6.0 * k, self.rate)),
            1 => self.filters[4].set(Coefs::peak(3000.0, 0.9, 4.0 + 6.0 * k, self.rate)),
            _ => self.level.set(db(value)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        self.filters.iter_mut().fold(x, |s, f| f.process(s)) * self.level.tick()
    }
}
