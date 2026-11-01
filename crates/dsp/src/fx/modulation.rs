use crate::fx::Stereo;
use crate::fx::line::{Lfo, Line};
use crate::util::Smooth;

const MOD_LINE_MS: f32 = 50.0;

fn line(rate: f32) -> Line {
    Line::new((MOD_LINE_MS * 0.001 * rate) as usize)
}

pub struct Chorus {
    rate: f32,
    lines: [Line; 2],
    lfos: [Lfo; 2],
    depth: f32,
    mix: Smooth,
}

impl Chorus {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            lines: [line(rate), line(rate)],
            lfos: [Lfo::new(0.0), Lfo::new(0.5)],
            depth: 0.5,
            mix: Smooth::new(0.5, 20.0, rate),
        }
    }
}

impl Stereo for Chorus {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.lfos.iter_mut().for_each(|l| l.tune(value, self.rate)),
            1 => self.depth = value.clamp(0.0, 1.0),
            _ => self.mix.set(value.clamp(0.0, 1.0)),
        }
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mix = self.mix.tick();
        let ms = 0.001 * self.rate;
        std::array::from_fn(|ch| {
            self.lines[ch].push(x[ch]);
            let d = (7.0 + 5.0 * self.depth * self.lfos[ch].tick()) * ms;
            let wet = self.lines[ch].read(d);
            x[ch] * (1.0 - mix * 0.5) + wet * mix
        })
    }
}

pub struct Flanger {
    rate: f32,
    lines: [Line; 2],
    lfos: [Lfo; 2],
    depth: f32,
    feedback: f32,
    last: [f32; 2],
    mix: Smooth,
}

impl Flanger {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            lines: [line(rate), line(rate)],
            lfos: [Lfo::new(0.0), Lfo::new(0.25)],
            depth: 0.7,
            feedback: 0.5,
            last: [0.0; 2],
            mix: Smooth::new(0.5, 20.0, rate),
        }
    }
}

impl Stereo for Flanger {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.lfos.iter_mut().for_each(|l| l.tune(value, self.rate)),
            1 => self.depth = value.clamp(0.0, 1.0),
            2 => self.feedback = value.clamp(-0.95, 0.95),
            _ => self.mix.set(value.clamp(0.0, 1.0)),
        }
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mix = self.mix.tick();
        let ms = 0.001 * self.rate;
        std::array::from_fn(|ch| {
            self.lines[ch].push(x[ch] + self.last[ch] * self.feedback);
            let lfo = 0.5 + 0.5 * self.lfos[ch].tick();
            let d = (0.5 + 4.5 * self.depth * lfo) * ms;
            let wet = self.lines[ch].read(d);
            self.last[ch] = wet;
            x[ch] * (1.0 - mix * 0.5) + wet * mix
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Allpass1 {
    a: f32,
    x1: f32,
    y1: f32,
}

impl Allpass1 {
    fn process(&mut self, x: f32) -> f32 {
        let y = self.a * x + self.x1 - self.a * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

const PHASER_STAGES: usize = 6;

pub struct Phaser {
    rate: f32,
    stages: [[Allpass1; PHASER_STAGES]; 2],
    lfos: [Lfo; 2],
    depth: f32,
    feedback: f32,
    last: [f32; 2],
    mix: Smooth,
}

impl Phaser {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            stages: [[Allpass1::default(); PHASER_STAGES]; 2],
            lfos: [Lfo::new(0.0), Lfo::new(0.25)],
            depth: 0.7,
            feedback: 0.5,
            last: [0.0; 2],
            mix: Smooth::new(0.5, 20.0, rate),
        }
    }
}

impl Stereo for Phaser {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.lfos.iter_mut().for_each(|l| l.tune(value, self.rate)),
            1 => self.depth = value.clamp(0.0, 1.0),
            2 => self.feedback = value.clamp(0.0, 0.9),
            _ => self.mix.set(value.clamp(0.0, 1.0)),
        }
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mix = self.mix.tick();
        let rate = self.rate;
        std::array::from_fn(|ch| {
            let lfo = 0.5 + 0.5 * self.lfos[ch].tick();
            let hz = 200.0 * (10.0f32).powf(lfo * self.depth);
            let t = (std::f32::consts::PI * hz / rate).tan();
            let a = (t - 1.0) / (t + 1.0);
            let mut s = x[ch] + self.last[ch] * self.feedback;
            for st in &mut self.stages[ch] {
                st.a = a;
                s = st.process(s);
            }
            self.last[ch] = s;
            x[ch] * (1.0 - mix * 0.5) + s * mix * 0.7
        })
    }
}

pub struct Doubler {
    rate: f32,
    line: Line,
    lfo: Lfo,
    delay: Smooth,
    width: Smooth,
}

impl Doubler {
    pub fn new(rate: f32) -> Self {
        let mut lfo = Lfo::new(0.0);
        lfo.tune(0.3, rate);
        Self {
            rate,
            line: line(rate),
            lfo,
            delay: Smooth::new(18.0, 50.0, rate),
            width: Smooth::new(0.8, 20.0, rate),
        }
    }
}

impl Stereo for Doubler {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.width.set(value.clamp(0.0, 1.0)),
            _ => self.delay.set(value.clamp(5.0, 40.0)),
        }
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mono = 0.5 * (x[0] + x[1]);
        self.line.push(mono);
        let d = (self.delay.tick() + self.lfo.tick()) * 0.001 * self.rate;
        let copy = self.line.read(d);
        let w = self.width.tick();
        [x[0], x[1] + (copy - x[1]) * w]
    }
}
