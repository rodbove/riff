use crate::fx::{Mono, Stereo};
use crate::gate::Gate;
use crate::rig::Rig;
use crate::util::{DcBlock, Smooth, db};

const FADE_MS: f32 = 8.0;
const SILENT: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Input,
    Output,
    Gate,
    Pre(usize),
    Amp,
    Cab,
    Post(usize),
}

struct Unit<T: ?Sized> {
    fx: Box<T>,
    wet: Smooth,
}

impl<T: ?Sized> Unit<T> {
    fn new(fx: Box<T>, on: bool, rate: f32) -> Self {
        Self {
            fx,
            wet: Smooth::new(if on { 1.0 } else { 0.0 }, FADE_MS, rate),
        }
    }

    fn bypassed(&self) -> bool {
        self.wet.target() == 0.0
    }
}

pub struct Chain {
    rate: f32,
    input: Smooth,
    output: Smooth,
    dc: DcBlock,
    gate: Gate,
    gate_wet: Smooth,
    pre: Vec<Unit<dyn Mono>>,
    post: Vec<Unit<dyn Stereo>>,
}

impl Chain {
    pub fn build(rig: &Rig, rate: f32) -> Self {
        let mut rig = rig.clone();
        rig.fix();
        let pre = rig
            .pre
            .iter()
            .filter_map(|s| {
                s.kind
                    .mono(&s.values, rate)
                    .map(|fx| Unit::new(fx, s.on, rate))
            })
            .collect();
        let post = rig
            .post
            .iter()
            .filter_map(|s| {
                s.kind
                    .stereo(&s.values, rate)
                    .map(|fx| Unit::new(fx, s.on, rate))
            })
            .collect();
        Self {
            rate,
            input: Smooth::new(db(rig.input), 20.0, rate),
            output: Smooth::new(db(rig.output), 20.0, rate),
            dc: DcBlock::new(rate),
            gate: Gate::new(rig.gate.threshold, rig.gate.release, rate),
            gate_wet: Smooth::new(if rig.gate.on { 1.0 } else { 0.0 }, FADE_MS, rate),
            pre,
            post,
        }
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    pub fn set(&mut self, target: Target, param: usize, value: f32) {
        match target {
            Target::Input => self.input.set(db(value)),
            Target::Output => self.output.set(db(value)),
            Target::Gate => match param {
                0 => self.gate.set_threshold(value),
                _ => self.gate.set_release(value),
            },
            Target::Pre(i) => {
                if let Some(u) = self.pre.get_mut(i) {
                    u.fx.set(param, value);
                }
            }
            Target::Post(i) => {
                if let Some(u) = self.post.get_mut(i) {
                    u.fx.set(param, value);
                }
            }
            Target::Amp | Target::Cab => {}
        }
    }

    pub fn enable(&mut self, target: Target, on: bool) {
        let v = if on { 1.0 } else { 0.0 };
        match target {
            Target::Gate => self.gate_wet.set(v),
            Target::Pre(i) => {
                if let Some(u) = self.pre.get_mut(i) {
                    u.wet.set(v);
                }
            }
            Target::Post(i) => {
                if let Some(u) = self.post.get_mut(i) {
                    u.wet.set(v);
                }
            }
            _ => {}
        }
    }

    pub fn gate_open(&self) -> bool {
        self.gate.is_open()
    }

    pub fn input_gain(&mut self) -> f32 {
        self.input.tick()
    }

    pub fn process(&mut self, x: f32) -> [f32; 2] {
        let key = self.dc.process(x);
        let mut m = key;
        for u in &mut self.pre {
            let w = u.wet.tick();
            if w < SILENT && u.bypassed() {
                continue;
            }
            let y = u.fx.process(m);
            m += (y - m) * w;
        }
        let gw = self.gate_wet.tick();
        if gw > SILENT || self.gate_wet.target() > 0.0 {
            let g = self.gate.process(key, m);
            m += (g - m) * gw;
        }
        let mut s = [m, m];
        for u in &mut self.post {
            let w = u.wet.tick();
            if w < SILENT && u.bypassed() {
                continue;
            }
            let y = u.fx.process(s);
            s = [s[0] + (y[0] - s[0]) * w, s[1] + (y[1] - s[1]) * w];
        }
        let out = self.output.tick();
        [limit(s[0] * out), limit(s[1] * out)]
    }
}

pub fn limit(x: f32) -> f32 {
    if x.abs() < 0.8 {
        x
    } else {
        x.signum() * (0.8 + 0.2 * crate::clip::tanh((x.abs() - 0.8) / 0.2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::Kind;
    use crate::rig::Slot;
    use std::f32::consts::TAU;

    const RATE: f32 = 48000.0;

    fn tone(i: usize) -> f32 {
        (i as f32 * 220.0 * TAU / RATE).sin() * 0.2
    }

    fn peak(chain: &mut Chain, n: usize) -> f32 {
        (0..n)
            .map(|i| {
                let x = tone(i) * chain.input_gain();
                chain.process(x)[0].abs()
            })
            .skip(n / 2)
            .fold(0.0, f32::max)
    }

    mod when_running_a_rig {
        use super::*;

        #[test]
        fn passes_a_clean_signal_at_the_output_level() {
            let rig = Rig {
                output: 0.0,
                ..Rig::default()
            };
            let mut c = Chain::build(&rig, RATE);
            assert!((peak(&mut c, 9600) - 0.2).abs() < 0.01);
        }

        #[test]
        fn applies_pedals_and_bypass() {
            let mut rig = Rig {
                output: 0.0,
                ..Rig::default()
            };
            rig.pre.push(Slot::with(Kind::Boost, &[("LEVEL", 6.0)]));
            let mut c = Chain::build(&rig, RATE);
            assert!((peak(&mut c, 9600) - 0.4).abs() < 0.02);
            c.enable(Target::Pre(0), false);
            assert!((peak(&mut c, 9600) - 0.2).abs() < 0.01);
        }

        #[test]
        fn never_exceeds_full_scale() {
            let rig = Rig {
                output: 12.0,
                input: 24.0,
                ..Rig::default()
            };
            let mut c = Chain::build(&rig, RATE);
            for i in 0..4800 {
                let x = tone(i) * 4.0 * c.input_gain();
                let [l, r] = c.process(x);
                assert!(l.abs() <= 1.0 && r.abs() <= 1.0);
            }
        }
    }
}
