use crate::amp::Amp;
use crate::cab::Cab;
use crate::fx::{Mono, Stereo};
use crate::gate::Gate;
use crate::library::Library;
use crate::nam::NamAmp;
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
    amp: Unit<dyn Mono>,
    cab: Cab,
    cab_wet: Smooth,
    post: Vec<Unit<dyn Stereo>>,
    warnings: Vec<String>,
}

impl Chain {
    pub fn build(rig: &Rig, rate: f32, library: &mut Library) -> Self {
        let mut rig = rig.clone();
        rig.fix();
        let mut warnings = Vec::new();
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
            amp: Unit::new(
                build_amp(&rig, rate, library, &mut warnings),
                rig.amp.on,
                rate,
            ),
            cab: Cab::new(&rig.cab, rate, library),
            cab_wet: Smooth::new(if rig.cab.on { 1.0 } else { 0.0 }, FADE_MS, rate),
            post,
            warnings,
        }
        .with_warnings()
    }

    fn with_warnings(mut self) -> Self {
        let cab = std::mem::take(&mut self.cab.warnings);
        self.warnings.extend(cab);
        self
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
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
            Target::Amp => self.amp.fx.set(param, value),
            Target::Cab => self.cab.set(param, value),
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
            Target::Amp => self.amp.wet.set(v),
            Target::Cab => self.cab_wet.set(v),
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
        let aw = self.amp.wet.tick();
        if aw > SILENT || !self.amp.bypassed() {
            let y = self.amp.fx.process(m);
            m += (y - m) * aw;
        }
        let cw = self.cab_wet.tick();
        if cw > SILENT || self.cab_wet.target() > 0.0 {
            let y = self.cab.process(m);
            m += (y - m) * cw;
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

fn build_amp(
    rig: &Rig,
    rate: f32,
    library: &mut Library,
    warnings: &mut Vec<String>,
) -> Box<dyn Mono> {
    if rig.amp.use_nam
        && let Some(path) = &rig.amp.nam
    {
        match NamAmp::new(
            std::path::Path::new(path),
            &rig.amp.nam_values,
            rate,
            library,
            warnings,
        ) {
            Ok(amp) => return Box::new(amp),
            Err(e) => warnings.push(format!(
                "{path}: {e}. Using the {} model.",
                rig.amp.model.voicing().name
            )),
        }
    }
    Box::new(Amp::new(rig.amp.model, &rig.amp.values, rate))
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
            let mut rig = Rig {
                output: 0.0,
                ..Rig::default()
            };
            rig.amp.on = false;
            rig.cab.on = false;
            let mut c = Chain::build(&rig, RATE, &mut Library::default());
            assert!((peak(&mut c, 9600) - 0.2).abs() < 0.01);
        }

        #[test]
        fn applies_pedals_and_bypass() {
            let mut rig = Rig {
                output: 0.0,
                ..Rig::default()
            };
            rig.amp.on = false;
            rig.cab.on = false;
            rig.pre.push(Slot::with(Kind::Boost, &[("LEVEL", 6.0)]));
            let mut c = Chain::build(&rig, RATE, &mut Library::default());
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
            let mut c = Chain::build(&rig, RATE, &mut Library::default());
            for i in 0..4800 {
                let x = tone(i) * 4.0 * c.input_gain();
                let [l, r] = c.process(x);
                assert!(l.abs() <= 1.0 && r.abs() <= 1.0);
            }
        }
    }
}
