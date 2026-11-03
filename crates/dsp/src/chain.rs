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
pub const BLOCK: usize = 256;

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
    dry: Vec<f32>,
    wet: Vec<f32>,
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
            dry: vec![0.0; BLOCK],
            wet: vec![0.0; BLOCK],
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
        let mut m = self.front(x);
        let aw = self.amp.wet.tick();
        if aw > SILENT || !self.amp.bypassed() {
            let y = self.amp.fx.process(m);
            m += (y - m) * aw;
        }
        self.back(m)
    }

    pub fn process_block(&mut self, input: &[f32], out: &mut [[f32; 2]]) {
        for (i, o) in input.chunks(BLOCK).zip(out.chunks_mut(BLOCK)) {
            self.block(i, o);
        }
    }

    fn block(&mut self, input: &[f32], out: &mut [[f32; 2]]) {
        let n = input.len();
        for (i, x) in input.iter().enumerate() {
            self.dry[i] = self.front(*x);
        }
        let amp = self.amp.wet.value() > SILENT || !self.amp.bypassed();
        if amp {
            self.wet[..n].copy_from_slice(&self.dry[..n]);
            self.amp.fx.process_block(&mut self.wet[..n]);
        }
        for (i, o) in out.iter_mut().enumerate() {
            let aw = self.amp.wet.tick();
            let mut m = self.dry[i];
            if amp {
                m += (self.wet[i] - m) * aw;
            }
            *o = self.back(m);
        }
    }

    fn front(&mut self, x: f32) -> f32 {
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
        m
    }

    fn back(&mut self, mut m: f32) -> [f32; 2] {
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

pub fn render(rig: &Rig, rate: f32, input: &[f32], library: &mut Library) -> Vec<[f32; 2]> {
    let _ftz = crate::denormal::NoDenormals::new();
    let mut chain = Chain::build(rig, rate, library);
    let mut scratch = vec![0.0; BLOCK];
    let mut out = vec![[0.0; 2]; input.len()];
    for (i, o) in input.chunks(BLOCK).zip(out.chunks_mut(BLOCK)) {
        for (s, x) in scratch.iter_mut().zip(i) {
            *s = x * chain.input_gain();
        }
        chain.process_block(&scratch[..i.len()], o);
    }
    out
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

    mod when_processing_in_blocks {
        use super::*;

        #[test]
        fn matches_processing_one_sample_at_a_time() {
            let mut rig = Rig::default();
            rig.pre.push(Slot::with(Kind::Overdrive, &[]));
            let mut by_sample = Chain::build(&rig, RATE, &mut Library::default());
            let mut by_block = Chain::build(&rig, RATE, &mut Library::default());
            let input: Vec<f32> = (0..3000).map(tone).collect();
            let expected: Vec<[f32; 2]> = input.iter().map(|x| by_sample.process(*x)).collect();
            let mut out = vec![[0.0; 2]; input.len()];
            for (i, o) in input.chunks(100).zip(out.chunks_mut(100)) {
                by_block.process_block(i, o);
            }
            assert_eq!(out, expected);
        }
    }
}
