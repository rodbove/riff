use crate::clip::tanh;
use crate::fx::Stereo;
use crate::fx::line::{Lfo, Line};
use crate::util::{OnePole, Smooth};

const MAX_DELAY_S: f32 = 2.1;

pub struct Delay {
    rate: f32,
    lines: [Line; 2],
    time: Smooth,
    feedback: f32,
    mix: Smooth,
    tone: [OnePole; 2],
    mode: usize,
    lfo: Lfo,
}

impl Delay {
    pub const DIGITAL: usize = 0;
    pub const ANALOG: usize = 1;
    pub const PING_PONG: usize = 2;

    pub fn new(rate: f32) -> Self {
        let len = (MAX_DELAY_S * rate) as usize;
        let mut lfo = Lfo::new(0.0);
        lfo.tune(0.5, rate);
        Self {
            rate,
            lines: [Line::new(len), Line::new(len)],
            time: Smooth::new(400.0, 120.0, rate),
            feedback: 0.35,
            mix: Smooth::new(0.25, 20.0, rate),
            tone: [OnePole::new(5000.0, rate); 2],
            mode: Self::DIGITAL,
            lfo,
        }
    }
}

impl Stereo for Delay {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.time.set(value.clamp(20.0, 2000.0)),
            1 => self.feedback = value.clamp(0.0, 0.95),
            2 => self.mix.set(value.clamp(0.0, 1.0)),
            3 => self.tone.iter_mut().for_each(|t| t.tune(value, self.rate)),
            _ => self.mode = value.round() as usize,
        }
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let mut d = self.time.tick() * 0.001 * self.rate;
        if self.mode == Self::ANALOG {
            d += self.lfo.tick() * 0.0015 * self.rate;
        }
        let wet = [self.lines[0].read(d), self.lines[1].read(d)];
        let fb = [self.tone[0].low(wet[0]), self.tone[1].low(wet[1])];
        let shape = |v: f32| {
            if self.mode == Self::ANALOG {
                tanh(v * 1.2) / 1.2
            } else {
                v
            }
        };
        if self.mode == Self::PING_PONG {
            let mono = 0.5 * (x[0] + x[1]);
            self.lines[0].push(shape(mono + fb[1] * self.feedback));
            self.lines[1].push(shape(fb[0] * self.feedback));
        } else {
            self.lines[0].push(shape(x[0] + fb[0] * self.feedback));
            self.lines[1].push(shape(x[1] + fb[1] * self.feedback));
        }
        let mix = self.mix.tick();
        [x[0] + wet[0] * mix, x[1] + wet[1] * mix]
    }
}

const LINES: usize = 8;
const BASE: [f32; LINES] = [
    0.0297, 0.0371, 0.0411, 0.0437, 0.0533, 0.0617, 0.0683, 0.0797,
];
const SIZES: [f32; 3] = [0.45, 0.8, 1.5];
const MAX_PREDELAY_MS: f32 = 210.0;

pub struct Reverb {
    rate: f32,
    lines: Vec<Line>,
    lengths: [f32; LINES],
    gains: [f32; LINES],
    damp: [OnePole; LINES],
    pre: Line,
    predelay: Smooth,
    decay: f32,
    damping: f32,
    size: usize,
    mix: Smooth,
}

impl Reverb {
    pub fn new(rate: f32) -> Self {
        let longest = BASE[LINES - 1] * SIZES[2] * rate;
        let mut r = Self {
            rate,
            lines: (0..LINES)
                .map(|_| Line::new(longest as usize + 8))
                .collect(),
            lengths: [0.0; LINES],
            gains: [0.0; LINES],
            damp: [OnePole::new(6000.0, rate); LINES],
            pre: Line::new((MAX_PREDELAY_MS * 0.001 * rate) as usize + 4),
            predelay: Smooth::new(20.0, 50.0, rate),
            decay: 1.8,
            damping: 0.5,
            size: 1,
            mix: Smooth::new(0.2, 20.0, rate),
        };
        r.retune();
        r
    }

    fn retune(&mut self) {
        let scale = SIZES[self.size.min(2)];
        let damp_hz = 18000.0 * (0.08f32).powf(self.damping);
        for (i, base) in BASE.iter().enumerate() {
            let len = base * scale * self.rate;
            self.lengths[i] = len;
            self.gains[i] = 10f32.powf(-3.0 * len / (self.decay.max(0.1) * self.rate));
            self.damp[i].tune(damp_hz, self.rate);
        }
    }

    pub fn tail_gain(&self) -> f32 {
        self.gains[0]
    }
}

impl Stereo for Reverb {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.size = value.round() as usize,
            1 => self.decay = value,
            2 => self.damping = value.clamp(0.0, 1.0),
            3 => {
                self.predelay.set(value.clamp(0.0, 200.0));
                return;
            }
            _ => {
                self.mix.set(value.clamp(0.0, 1.0));
                return;
            }
        }
        self.retune();
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        self.pre.push(0.5 * (x[0] + x[1]));
        let input = self
            .pre
            .read(self.predelay.tick() * 0.001 * self.rate + 1.0);
        let taps: [f32; LINES] =
            std::array::from_fn(|i| self.damp[i].low(self.lines[i].read(self.lengths[i])));
        let mut h = taps;
        let mut span = 1;
        while span < LINES {
            for i in (0..LINES).step_by(span * 2) {
                for j in i..i + span {
                    let (a, b) = (h[j], h[j + span]);
                    h[j] = a + b;
                    h[j + span] = a - b;
                }
            }
            span *= 2;
        }
        let norm = 1.0 / (LINES as f32).sqrt();
        for ((line, v), g) in self.lines.iter_mut().zip(h).zip(self.gains) {
            line.push(input + v * norm * g);
        }
        let l: f32 = taps.iter().step_by(2).sum();
        let r: f32 = taps.iter().skip(1).step_by(2).sum();
        let mix = self.mix.tick();
        let scale = 0.35;
        [x[0] + l * scale * mix, x[1] + r * scale * mix]
    }
}
