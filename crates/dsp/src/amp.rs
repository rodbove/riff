use serde::{Deserialize, Serialize};

use crate::biquad::{Biquad, Coefs};
use crate::clip::{tanh, tube};
use crate::fx::Mono;
use crate::oversample::Oversampler8;
use crate::param::Param;
use crate::util::{Follower, Smooth, db};

const OS: f32 = Oversampler8::FACTOR as f32;
const TARGET_RMS: f32 = 0.125;
const CALIBRATE: usize = 12000;
const MAX_STAGES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmpModel {
    Clean,
    Brit,
    Mark,
    Recto,
    Block,
    Uber,
}

pub const GAIN: usize = 0;
pub const BASS: usize = 1;
pub const MID: usize = 2;
pub const TREBLE: usize = 3;
pub const PRESENCE: usize = 4;
pub const RESONANCE: usize = 5;
pub const MASTER: usize = 6;
pub const BRIGHT: usize = 7;

pub const PARAMS: &[Param] = &[
    Param::knob("GAIN", 5.0),
    Param::knob("BASS", 5.0),
    Param::knob("MID", 5.0),
    Param::knob("TREBLE", 5.0),
    Param::knob("PRESENCE", 5.0),
    Param::knob("RESONANCE", 5.0),
    Param::knob("MASTER", 5.0),
    Param::choice("BRIGHT", &["OFF", "ON"], 0.0),
];

#[derive(Clone, Copy)]
struct Stage {
    gain_db: f32,
    bias: f32,
    hp: f32,
    lp: f32,
    shelf: Option<(f32, f32)>,
}

const fn stage(gain_db: f32, bias: f32, hp: f32, lp: f32) -> Stage {
    Stage {
        gain_db,
        bias,
        hp,
        lp,
        shelf: None,
    }
}

const fn shelved(s: Stage, hz: f32, gain_db: f32) -> Stage {
    Stage {
        shelf: Some((hz, gain_db)),
        ..s
    }
}

#[derive(Clone, Copy)]
struct Stack {
    early: bool,
    bass_hz: f32,
    mid_hz: f32,
    mid_q: f32,
    scoop_db: f32,
    treble_hz: f32,
}

#[derive(Clone, Copy)]
struct Power {
    drive_db: f32,
    sag: f32,
    presence_hz: f32,
    resonance_hz: f32,
    thump_db: f32,
    air_hz: f32,
    air_db: f32,
}

pub struct Voicing {
    pub name: &'static str,
    pub like: &'static str,
    tight: f32,
    gain_range: (f32, f32),
    stages: &'static [Stage],
    stack: Stack,
    power: Power,
}

const CLEAN: Voicing = Voicing {
    name: "CLEAN TWIN",
    like: "American clean, like a Fender Twin",
    tight: 40.0,
    gain_range: (-12.0, 18.0),
    stages: &[
        shelved(stage(6.0, 0.1, 30.0, 12000.0), 120.0, 3.0),
        stage(6.0, 0.05, 20.0, 10000.0),
    ],
    stack: Stack {
        early: false,
        bass_hz: 100.0,
        mid_hz: 400.0,
        mid_q: 0.7,
        scoop_db: -6.0,
        treble_hz: 2500.0,
    },
    power: Power {
        drive_db: 0.0,
        sag: 0.1,
        presence_hz: 4000.0,
        resonance_hz: 90.0,
        thump_db: 3.0,
        air_hz: 2800.0,
        air_db: 2.5,
    },
};

const BRIT: Voicing = Voicing {
    name: "BRIT 800",
    like: "British crunch, like a Marshall JCM800",
    tight: 60.0,
    gain_range: (0.0, 24.0),
    stages: &[
        stage(12.0, 0.2, 80.0, 12000.0),
        shelved(stage(12.0, 0.15, 100.0, 11000.0), 300.0, -6.0),
        stage(8.0, 0.1, 60.0, 9500.0),
    ],
    stack: Stack {
        early: false,
        bass_hz: 120.0,
        mid_hz: 620.0,
        mid_q: 0.65,
        scoop_db: -11.0,
        treble_hz: 3000.0,
    },
    power: Power {
        drive_db: 3.0,
        sag: 0.2,
        presence_hz: 3500.0,
        resonance_hz: 100.0,
        thump_db: 4.0,
        air_hz: 2400.0,
        air_db: 4.5,
    },
};

const MARK: Voicing = Voicing {
    name: "MARK IIC",
    like: "Mesa Mark IIC+ lead channel",
    tight: 120.0,
    gain_range: (0.0, 26.0),
    stages: &[
        shelved(stage(14.0, 0.25, 130.0, 12000.0), 250.0, -4.0),
        shelved(stage(13.0, 0.2, 110.0, 11000.0), 300.0, -6.0),
        stage(12.0, 0.15, 80.0, 10000.0),
        stage(8.0, 0.1, 60.0, 9000.0),
    ],
    stack: Stack {
        early: true,
        bass_hz: 90.0,
        mid_hz: 620.0,
        mid_q: 0.6,
        scoop_db: -16.0,
        treble_hz: 2500.0,
    },
    power: Power {
        drive_db: 4.0,
        sag: 0.15,
        presence_hz: 4000.0,
        resonance_hz: 80.0,
        thump_db: 6.0,
        air_hz: 2200.0,
        air_db: 5.5,
    },
};

const RECTO: Voicing = Voicing {
    name: "RECTO",
    like: "Mesa Dual Rectifier modern channel",
    tight: 95.0,
    gain_range: (0.0, 23.0),
    stages: &[
        stage(12.0, 0.3, 90.0, 12000.0),
        shelved(stage(13.0, 0.2, 110.0, 11000.0), 300.0, -6.0),
        shelved(stage(12.0, 0.2, 80.0, 10000.0), 250.0, -4.0),
        stage(8.0, 0.1, 50.0, 9000.0),
    ],
    stack: Stack {
        early: false,
        bass_hz: 110.0,
        mid_hz: 660.0,
        mid_q: 0.63,
        scoop_db: -20.0,
        treble_hz: 2800.0,
    },
    power: Power {
        drive_db: 6.0,
        sag: 0.18,
        presence_hz: 3200.0,
        resonance_hz: 85.0,
        thump_db: 4.4,
        air_hz: 2200.0,
        air_db: 6.4,
    },
};

const BLOCK: Voicing = Voicing {
    name: "BLOCK 51",
    like: "Peavey 5150 lead channel",
    tight: 100.0,
    gain_range: (0.0, 26.0),
    stages: &[
        shelved(stage(15.0, 0.3, 120.0, 12000.0), 250.0, -4.0),
        shelved(stage(13.0, 0.25, 110.0, 11000.0), 300.0, -6.0),
        stage(12.0, 0.2, 80.0, 10000.0),
        stage(8.0, 0.15, 60.0, 9000.0),
    ],
    stack: Stack {
        early: false,
        bass_hz: 100.0,
        mid_hz: 600.0,
        mid_q: 0.6,
        scoop_db: -18.0,
        treble_hz: 3200.0,
    },
    power: Power {
        drive_db: 5.0,
        sag: 0.2,
        presence_hz: 4500.0,
        resonance_hz: 95.0,
        thump_db: 5.0,
        air_hz: 2200.0,
        air_db: 6.0,
    },
};

const UBER: Voicing = Voicing {
    name: "UBER",
    like: "German high gain, like a Bogner Uberschall or Engl",
    tight: 110.0,
    gain_range: (0.0, 24.0),
    stages: &[
        shelved(stage(14.0, 0.3, 120.0, 12000.0), 250.0, -4.0),
        shelved(stage(12.0, 0.25, 110.0, 11000.0), 300.0, -6.0),
        stage(11.0, 0.2, 80.0, 10500.0),
        stage(9.0, 0.2, 70.0, 10000.0),
        stage(6.0, 0.1, 60.0, 9000.0),
    ],
    stack: Stack {
        early: false,
        bass_hz: 100.0,
        mid_hz: 620.0,
        mid_q: 0.6,
        scoop_db: -18.0,
        treble_hz: 3500.0,
    },
    power: Power {
        drive_db: 5.0,
        sag: 0.2,
        presence_hz: 4800.0,
        resonance_hz: 90.0,
        thump_db: 6.0,
        air_hz: 2200.0,
        air_db: 6.0,
    },
};

impl AmpModel {
    pub const ALL: [AmpModel; 6] = [
        AmpModel::Clean,
        AmpModel::Brit,
        AmpModel::Mark,
        AmpModel::Recto,
        AmpModel::Block,
        AmpModel::Uber,
    ];

    pub fn voicing(self) -> &'static Voicing {
        match self {
            AmpModel::Clean => &CLEAN,
            AmpModel::Brit => &BRIT,
            AmpModel::Mark => &MARK,
            AmpModel::Recto => &RECTO,
            AmpModel::Block => &BLOCK,
            AmpModel::Uber => &UBER,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct StageState {
    hp: Biquad,
    lp: Biquad,
    shelf: Option<Biquad>,
    gain: f32,
    bias: f32,
    shift: f32,
}

const SHIFT_DEPTH: f32 = 0.05;
const SHIFT_ATTACK_MS: f32 = 10.0;
const SHIFT_RELEASE_MS: f32 = 150.0;

pub struct Amp {
    voicing: &'static Voicing,
    rate: f32,
    knobs: [f32; 8],
    os: Oversampler8,
    tight: Biquad,
    bright: Biquad,
    pre_gain: Smooth,
    stages: [StageState; MAX_STAGES],
    count: usize,
    bass: Biquad,
    mid: Biquad,
    treble: Biquad,
    presence: Biquad,
    resonance: Biquad,
    drive: Smooth,
    level: Smooth,
    sag: Follower,
    shift_attack: f32,
    shift_release: f32,
    load_low: Biquad,
    load_high: Biquad,
    transformer: Biquad,
    trim: f32,
}

impl Amp {
    pub fn new(model: AmpModel, values: &[f32], rate: f32) -> Self {
        let v = model.voicing();
        let fast = rate * OS;
        let mut stages = [StageState::default(); MAX_STAGES];
        for (s, st) in v.stages.iter().zip(stages.iter_mut()) {
            *st = StageState {
                hp: Biquad::new(Coefs::highpass(s.hp, 0.707, fast)),
                lp: Biquad::new(Coefs::lowpass(s.lp, 0.707, fast)),
                shelf: s
                    .shelf
                    .map(|(hz, g)| Biquad::new(Coefs::low_shelf(hz, g, fast))),
                gain: db(s.gain_db),
                bias: s.bias,
                shift: 0.0,
            };
        }
        let mut amp = Self {
            voicing: v,
            rate,
            knobs: std::array::from_fn(|i| PARAMS[i].default),
            os: Oversampler8::new(),
            tight: Biquad::new(Coefs::highpass(v.tight, 0.707, fast)),
            bright: Biquad::default(),
            pre_gain: Smooth::new(1.0, 30.0, fast),
            stages,
            count: v.stages.len().min(MAX_STAGES),
            bass: Biquad::default(),
            mid: Biquad::default(),
            treble: Biquad::default(),
            presence: Biquad::default(),
            resonance: Biquad::default(),
            drive: Smooth::new(1.0, 30.0, fast),
            level: Smooth::new(1.0, 30.0, fast),
            sag: Follower::new(5.0, 80.0, fast),
            shift_attack: 1.0 - (-1.0 / (SHIFT_ATTACK_MS * 0.001 * fast)).exp(),
            shift_release: 1.0 - (-1.0 / (SHIFT_RELEASE_MS * 0.001 * fast)).exp(),
            load_low: Biquad::new(Coefs::peak(110.0, 1.1, v.power.thump_db, fast)),
            load_high: Biquad::new(Coefs::high_shelf(v.power.air_hz, v.power.air_db, fast)),
            transformer: Biquad::new(Coefs::lowpass(10200.0, 0.6, fast)),
            trim: 1.0,
        };
        for (i, p) in PARAMS.iter().enumerate() {
            amp.apply(i, p.default);
        }
        amp.settle();
        amp.trim = amp.calibrate();
        for (i, p) in PARAMS.iter().enumerate() {
            amp.apply(i, p.clamp(values.get(i).copied().unwrap_or(p.default)));
        }
        amp.settle();
        amp
    }

    fn settle(&mut self) {
        self.pre_gain.snap(self.pre_gain.target());
        self.drive.snap(self.drive.target());
        self.level.snap(self.level.target());
    }

    fn calibrate(&mut self) -> f32 {
        let mut sum = 0.0;
        let mut count = 0;
        for i in 0..CALIBRATE {
            let t = i as f32 / self.rate;
            let x = 0.2
                * ((t * 110.0 * std::f32::consts::TAU).sin()
                    + 0.5 * (t * 220.0 * std::f32::consts::TAU).sin()
                    + 0.25 * (t * 330.0 * std::f32::consts::TAU).sin());
            let y = self.run(x);
            if i > CALIBRATE / 2 {
                sum += y * y;
                count += 1;
            }
        }
        let rms = (sum / count as f32).sqrt().max(1e-6);
        self.reset();
        TARGET_RMS / rms
    }

    fn reset(&mut self) {
        self.os = Oversampler8::new();
        self.tight.reset();
        self.bright.reset();
        for s in &mut self.stages {
            s.hp.reset();
            s.lp.reset();
            if let Some(b) = s.shelf.as_mut() {
                b.reset();
            }
            s.shift = 0.0;
        }
        for b in [
            &mut self.bass,
            &mut self.mid,
            &mut self.treble,
            &mut self.presence,
            &mut self.resonance,
            &mut self.load_low,
            &mut self.load_high,
            &mut self.transformer,
        ] {
            b.reset();
        }
    }

    fn apply(&mut self, param: usize, value: f32) {
        let fast = self.rate * OS;
        let v = self.voicing;
        if param < self.knobs.len() {
            self.knobs[param] = value;
        }
        let k = |i: usize| (self.knobs[i] - 5.0) / 5.0;
        match param {
            GAIN => {
                let t = (self.knobs[GAIN] / 10.0).clamp(0.0, 1.0);
                let (lo, hi) = v.gain_range;
                self.pre_gain.set(db(lo + (hi - lo) * t));
            }
            BASS => self
                .bass
                .set(Coefs::low_shelf(v.stack.bass_hz, k(BASS) * 12.0, fast)),
            MID => self.mid.set(Coefs::peak(
                v.stack.mid_hz,
                v.stack.mid_q,
                v.stack.scoop_db + k(MID) * 10.0,
                fast,
            )),
            TREBLE => self
                .treble
                .set(Coefs::high_shelf(v.stack.treble_hz, k(TREBLE) * 12.0, fast)),
            PRESENCE => self.presence.set(Coefs::high_shelf(
                v.power.presence_hz,
                k(PRESENCE) * 8.0,
                fast,
            )),
            RESONANCE => self.resonance.set(Coefs::peak(
                v.power.resonance_hz,
                1.2,
                k(RESONANCE) * 8.0,
                fast,
            )),
            MASTER => {
                self.drive.set(db(v.power.drive_db + k(MASTER) * 12.0));
                self.level.set(db(k(MASTER) * 6.0));
            }
            BRIGHT => {
                let on = self.knobs[BRIGHT] > 0.5;
                self.bright
                    .set(Coefs::high_shelf(2500.0, if on { 6.0 } else { 0.0 }, fast));
            }
            _ => {}
        }
    }

    fn run(&mut self, x: f32) -> f32 {
        let Amp {
            voicing,
            os,
            tight,
            bright,
            pre_gain,
            stages,
            count,
            bass,
            mid,
            treble,
            presence,
            resonance,
            drive,
            level,
            sag,
            trim,
            shift_attack,
            shift_release,
            load_low,
            load_high,
            transformer,
            ..
        } = self;
        let early = voicing.stack.early;
        let sag_amount = voicing.power.sag;
        let y = os.process(x, |s| {
            let mut s = tight.process(s);
            s = bright.process(s) * pre_gain.tick();
            if early {
                s = treble.process(mid.process(bass.process(s)));
            }
            for st in &mut stages[..*count] {
                s = st.hp.process(s);
                if let Some(b) = st.shelf.as_mut() {
                    s = b.process(s);
                }
                let drive = s * st.gain;
                let bias = st.bias - st.shift;
                s = tube(drive, bias);
                let over = ((drive + bias).max(0.0) * SHIFT_DEPTH).min(0.6);
                let c = if over > st.shift {
                    *shift_attack
                } else {
                    *shift_release
                };
                st.shift += (over - st.shift) * c;
                s = st.lp.process(s) * 0.5;
            }
            if !early {
                s = treble.process(mid.process(bass.process(s)));
            }
            s = resonance.process(presence.process(s));
            let d = drive.tick() * (1.0 - sag_amount * sag.level().min(1.0));
            let p = tanh(s * d);
            sag.process(p);
            let p = load_high.process(load_low.process(p));
            transformer.process(p) * level.tick()
        });
        y * *trim
    }
}

impl Mono for Amp {
    fn set(&mut self, param: usize, value: f32) {
        if let Some(p) = PARAMS.get(param) {
            self.apply(param, p.clamp(value));
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        self.run(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const RATE: f32 = 48000.0;

    fn chord(i: usize) -> f32 {
        let t = i as f32 / RATE;
        0.15 * ((t * 82.4 * TAU).sin() + (t * 123.5 * TAU).sin() + 0.5 * (t * 164.8 * TAU).sin())
    }

    fn rms(amp: &mut Amp, input: impl Fn(usize) -> f32, n: usize) -> f32 {
        let out: Vec<f32> = (0..n).map(|i| amp.process(input(i))).collect();
        let tail = &out[n / 2..];
        (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt()
    }

    fn band(amp: &mut Amp, hz: f32) -> f32 {
        rms(amp, |i| (i as f32 * hz * TAU / RATE).sin() * 0.001, 9600)
    }

    mod when_playing_each_model {
        use super::*;

        #[test]
        fn keeps_a_similar_level_at_default_knobs() {
            for m in AmpModel::ALL {
                let mut a = Amp::new(m, &[], RATE);
                let level = rms(&mut a, chord, 24000);
                assert!(level > 0.06 && level < 0.25, "{m:?} {level}");
            }
        }

        #[test]
        fn stays_bounded_and_finite_at_full_settings() {
            for m in AmpModel::ALL {
                let mut a = Amp::new(m, &[10.0; 8], RATE);
                for i in 0..24000 {
                    let y = a.process(chord(i) * 6.0);
                    assert!(y.is_finite() && y.abs() < 4.0, "{m:?} {y}");
                }
            }
        }

        #[test]
        fn more_gain_means_more_distortion() {
            for m in [AmpModel::Brit, AmpModel::Recto, AmpModel::Block] {
                let thd = |gain: f32| {
                    let mut a = Amp::new(m, &[gain], RATE);
                    let hz = 220.0;
                    let out: Vec<f32> = (0..24000)
                        .map(|i| a.process((i as f32 * hz * TAU / RATE).sin() * 0.2))
                        .collect();
                    let tail = &out[12000..];
                    let (mut re, mut im) = (0.0f64, 0.0f64);
                    for (n, v) in tail.iter().enumerate() {
                        let w = std::f64::consts::TAU * hz as f64 * n as f64 / RATE as f64;
                        re += *v as f64 * w.cos();
                        im += *v as f64 * w.sin();
                    }
                    let fundamental = 2.0 * (re * re + im * im) / (tail.len() as f64).powi(2);
                    let total =
                        tail.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / tail.len() as f64;
                    1.0 - (fundamental / total).min(1.0)
                };
                assert!(thd(10.0) > thd(0.0), "{m:?}");
            }
        }
    }

    mod when_picking_hard {
        use super::*;

        #[test]
        fn the_amp_sags_after_a_loud_hit_and_recovers() {
            let mut a = Amp::new(AmpModel::Brit, &[3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 10.0], RATE);
            let tone = |i: usize, level: f32| (i as f32 * 196.0 * TAU / RATE).sin() * level;
            for i in 0..9600 {
                a.process(tone(i, 0.8));
            }
            let window = |a: &mut Amp, start: usize| {
                let v: Vec<f32> = (start..start + 2400)
                    .map(|i| a.process(tone(i, 0.01)))
                    .collect();
                (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
            };
            for i in 9600..10800 {
                a.process(tone(i, 0.01));
            }
            let right_after = window(&mut a, 10800);
            for i in 13200..36000 {
                a.process(tone(i, 0.01));
            }
            let later = window(&mut a, 36000);
            assert!(later > right_after * 1.03, "{right_after} {later}");
        }
    }

    mod when_turning_the_tone_stack {
        use super::*;

        #[test]
        fn bass_and_treble_move_their_bands() {
            let mut flat = Amp::new(AmpModel::Clean, &[0.0], RATE);
            let mut bassy = Amp::new(AmpModel::Clean, &[0.0, 10.0], RATE);
            assert!(band(&mut bassy, 80.0) > band(&mut flat, 80.0) * 1.5);
            let mut bright = Amp::new(AmpModel::Clean, &[0.0, 5.0, 5.0, 10.0], RATE);
            let mut dark = Amp::new(AmpModel::Clean, &[0.0, 5.0, 5.0, 0.0], RATE);
            assert!(band(&mut bright, 5000.0) > band(&mut dark, 5000.0) * 2.0);
        }

        #[test]
        fn tight_models_cut_more_low_end() {
            let mut tight = Amp::new(AmpModel::Mark, &[0.0], RATE);
            let mut loose = Amp::new(AmpModel::Clean, &[0.0], RATE);
            let ratio = |a: &mut Amp| band(a, 50.0) / band(a, 800.0);
            assert!(ratio(&mut tight) < ratio(&mut loose));
        }
    }
}
