use crate::biquad::{Biquad, Coefs};
use crate::fx::Mono;
use crate::util::{Follower, OnePole, Smooth, db};

pub const EQ_BANDS: [f32; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
const RETUNE: u32 = 16;
const WAH_LOW: f32 = 350.0;
const WAH_HIGH: f32 = 2200.0;

pub struct Eq {
    rate: f32,
    bands: [[Biquad; 10]; 2],
    level: Smooth,
}

impl Eq {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            bands: [[Biquad::default(); 10]; 2],
            level: Smooth::new(1.0, 20.0, rate),
        }
    }

    pub fn set_param(&mut self, param: usize, value: f32) {
        if param < EQ_BANDS.len() {
            let c = Coefs::peak(EQ_BANDS[param], 1.1, value, self.rate);
            for ch in &mut self.bands {
                ch[param].set(c);
            }
        } else {
            self.level.set(db(value));
        }
    }

    pub fn channel(&mut self, ch: usize, x: f32) -> f32 {
        self.bands[ch].iter_mut().fold(x, |s, b| b.process(s))
    }

    pub fn level(&mut self) -> f32 {
        self.level.tick()
    }
}

impl Mono for Eq {
    fn set(&mut self, param: usize, value: f32) {
        self.set_param(param, value);
    }

    fn process(&mut self, x: f32) -> f32 {
        self.channel(0, x) * self.level()
    }
}

pub struct Wah {
    rate: f32,
    filter: Biquad,
    position: Smooth,
    auto: bool,
    sensitivity: f32,
    q: f32,
    follower: Follower,
    tick: u32,
}

impl Wah {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            filter: Biquad::default(),
            position: Smooth::new(0.5, 15.0, rate),
            auto: false,
            sensitivity: 0.5,
            q: 6.0,
            follower: Follower::new(5.0, 120.0, rate),
            tick: 0,
        }
    }

    pub fn center(position: f32) -> f32 {
        WAH_LOW * (WAH_HIGH / WAH_LOW).powf(position.clamp(0.0, 1.0))
    }
}

impl Mono for Wah {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.position.set(value.clamp(0.0, 1.0)),
            1 => self.auto = value > 0.5,
            2 => self.sensitivity = value.clamp(0.0, 1.0),
            _ => self.q = value.max(0.5),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let env = self.follower.process(x);
        let manual = self.position.tick();
        if self.tick == 0 {
            let pos = if self.auto {
                (env * (2.0 + 30.0 * self.sensitivity)).min(1.0)
            } else {
                manual
            };
            self.filter
                .set(Coefs::bandpass(Self::center(pos), self.q, self.rate));
        }
        self.tick = (self.tick + 1) % RETUNE;
        self.filter.process(x) * 2.5 + x * 0.1
    }
}

pub struct Octaver {
    track: [Biquad; 2],
    smooth: OnePole,
    follower: Follower,
    flip: f32,
    high: bool,
    sub: Smooth,
    dry: Smooth,
}

impl Octaver {
    pub fn new(rate: f32) -> Self {
        Self {
            track: [Biquad::new(Coefs::lowpass(300.0, 0.707, rate)); 2],
            smooth: OnePole::new(800.0, rate),
            follower: Follower::new(2.0, 60.0, rate),
            flip: 1.0,
            high: false,
            sub: Smooth::new(0.5, 20.0, rate),
            dry: Smooth::new(1.0, 20.0, rate),
        }
    }
}

impl Mono for Octaver {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.sub.set(value.clamp(0.0, 1.0)),
            _ => self.dry.set(value.clamp(0.0, 1.0)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let first = self.track[0].process(x);
        let t = self.track[1].process(first);
        let env = self.follower.process(t);
        let threshold = env * 0.3;
        if !self.high && t > threshold {
            self.high = true;
            self.flip = -self.flip;
        } else if self.high && t < -threshold {
            self.high = false;
        }
        let sub = self.smooth.low(self.flip * env);
        x * self.dry.tick() + sub * self.sub.tick() * 2.0
    }
}
