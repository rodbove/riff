use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::biquad::{Biquad, Coefs};
use crate::conv::Convolver;
use crate::library::{Library, normalize};
use crate::param::{Param, Unit};
use crate::util::{Smooth, db};

const IR_LEN: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CabModel {
    V30,
    Greenback,
    OpenBack,
    Single,
}

pub const BLEND: usize = 0;
pub const LOW_CUT: usize = 1;
pub const HIGH_CUT: usize = 2;
pub const LEVEL: usize = 3;

pub const PARAMS: &[Param] = &[
    Param::new("MIC BLEND", 0.0, 1.0, 0.3, Unit::Percent),
    Param::new("LOW CUT", 20.0, 200.0, 60.0, Unit::Hz).curved(2.0),
    Param::new("HIGH CUT", 3000.0, 16000.0, 9000.0, Unit::Hz).curved(2.0),
    Param::new("LEVEL", -12.0, 12.0, 0.0, Unit::Db),
];

enum Shape {
    Hp(f32, f32),
    Lp(f32, f32),
    Peak(f32, f32, f32),
    High(f32, f32),
}

impl CabModel {
    pub const ALL: [CabModel; 4] = [
        CabModel::V30,
        CabModel::Greenback,
        CabModel::OpenBack,
        CabModel::Single,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CabModel::V30 => "4X12 V30",
            CabModel::Greenback => "4X12 GREEN",
            CabModel::OpenBack => "2X12 OPEN",
            CabModel::Single => "1X12",
        }
    }

    fn shapes(self) -> Vec<Shape> {
        use Shape::*;
        match self {
            CabModel::V30 => vec![
                Hp(75.0, 1.2),
                Peak(110.0, 1.5, 3.0),
                Peak(400.0, 1.2, -3.0),
                Peak(2500.0, 1.5, 4.0),
                Lp(5500.0, 0.8),
                Lp(5500.0, 0.6),
            ],
            CabModel::Greenback => vec![
                Hp(70.0, 1.1),
                Peak(100.0, 1.5, 2.5),
                Peak(1600.0, 1.2, 3.0),
                Peak(3000.0, 2.0, -2.0),
                Lp(4500.0, 0.8),
                Lp(4500.0, 0.6),
            ],
            CabModel::OpenBack => vec![
                Hp(90.0, 0.7),
                Peak(120.0, 1.2, 1.0),
                Peak(2000.0, 1.2, 2.0),
                Lp(6000.0, 0.8),
                Lp(6000.0, 0.6),
            ],
            CabModel::Single => vec![
                Hp(110.0, 0.7),
                Peak(1200.0, 1.0, 2.0),
                Lp(5000.0, 0.8),
                Lp(5000.0, 0.6),
            ],
        }
    }

    pub fn render(self, off_axis: bool, rate: f32) -> Vec<f32> {
        let mut shapes = self.shapes();
        let lp_scale = if off_axis { 0.8 } else { 1.15 };
        for s in &mut shapes {
            if let Shape::Lp(hz, _) = s {
                *hz *= lp_scale;
            }
        }
        shapes.push(if off_axis {
            Shape::High(2500.0, -4.0)
        } else {
            Shape::High(3000.0, 3.0)
        });
        let mut filters: Vec<Biquad> = shapes
            .iter()
            .map(|s| {
                Biquad::new(match *s {
                    Shape::Hp(hz, q) => Coefs::highpass(hz, q, rate),
                    Shape::Lp(hz, q) => Coefs::lowpass(hz, q, rate),
                    Shape::Peak(hz, q, g) => Coefs::peak(hz, q, g, rate),
                    Shape::High(hz, g) => Coefs::high_shelf(hz, g, rate),
                })
            })
            .collect();
        let len = (IR_LEN as f32 * rate / 48000.0) as usize;
        let mut ir: Vec<f32> = (0..len)
            .map(|i| {
                let mut x = if i == 0 { 1.0 } else { 0.0 };
                for f in &mut filters {
                    x = f.process(x);
                }
                x
            })
            .collect();
        normalize(&mut ir);
        ir
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CabState {
    pub on: bool,
    pub model: CabModel,
    pub ir_a: Option<String>,
    pub ir_b: Option<String>,
    pub values: Vec<f32>,
}

impl Default for CabState {
    fn default() -> Self {
        Self {
            on: true,
            model: CabModel::V30,
            ir_a: None,
            ir_b: None,
            values: PARAMS.iter().map(|p| p.default).collect(),
        }
    }
}

impl CabState {
    pub fn with(model: CabModel) -> Self {
        Self {
            model,
            ..Self::default()
        }
    }

    pub fn fix(&mut self) {
        self.values.resize(PARAMS.len(), 0.0);
        for (v, p) in self.values.iter_mut().zip(PARAMS) {
            *v = p.clamp(*v);
        }
    }
}

pub struct Cab {
    rate: f32,
    a: Convolver,
    b: Convolver,
    blend: Smooth,
    low: Biquad,
    high: Biquad,
    level: Smooth,
    pub warnings: Vec<String>,
}

fn mic(
    path: &Option<String>,
    model: CabModel,
    off_axis: bool,
    rate: f32,
    library: &mut Library,
    warnings: &mut Vec<String>,
) -> Arc<[f32]> {
    if let Some(p) = path {
        match library.ir(Path::new(p), rate) {
            Ok(ir) => return ir,
            Err(e) => warnings.push(format!("{p}: {e}")),
        }
    }
    model.render(off_axis, rate).into()
}

impl Cab {
    pub fn new(state: &CabState, rate: f32, library: &mut Library) -> Self {
        let mut warnings = Vec::new();
        let a = mic(
            &state.ir_a,
            state.model,
            false,
            rate,
            library,
            &mut warnings,
        );
        let b = mic(&state.ir_b, state.model, true, rate, library, &mut warnings);
        let mut cab = Self {
            rate,
            a: Convolver::new(&a),
            b: Convolver::new(&b),
            blend: Smooth::new(0.0, 20.0, rate),
            low: Biquad::default(),
            high: Biquad::default(),
            level: Smooth::new(1.0, 20.0, rate),
            warnings,
        };
        for (i, p) in PARAMS.iter().enumerate() {
            cab.set(
                i,
                p.clamp(state.values.get(i).copied().unwrap_or(p.default)),
            );
        }
        cab.blend.snap(cab.blend.target());
        cab.level.snap(cab.level.target());
        cab
    }

    pub fn set(&mut self, param: usize, value: f32) {
        match param {
            BLEND => self.blend.set(value.clamp(0.0, 1.0)),
            LOW_CUT => self.low.set(Coefs::highpass(value, 0.707, self.rate)),
            HIGH_CUT => self.high.set(Coefs::lowpass(value, 0.707, self.rate)),
            LEVEL => self.level.set(db(value)),
            _ => {}
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let t = self.blend.tick();
        let a = self.a.process(x);
        let b = self.b.process(x);
        let y = a + (b - a) * t;
        self.high.process(self.low.process(y)) * self.level.tick()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48000.0;

    fn response(ir: &[f32], hz: f32) -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (n, h) in ir.iter().enumerate() {
            let w = 2.0 * std::f64::consts::PI * hz as f64 * n as f64 / RATE as f64;
            re += *h as f64 * w.cos();
            im -= *h as f64 * w.sin();
        }
        (20.0 * (re * re + im * im).sqrt().log10()) as f32
    }

    mod when_rendering_a_cab {
        use super::*;

        #[test]
        fn rolls_off_lows_and_highs_like_a_speaker() {
            for m in CabModel::ALL {
                let ir = m.render(false, RATE);
                let mid = response(&ir, 1000.0);
                assert!(response(&ir, 30.0) < mid - 6.0, "{m:?}");
                assert!(response(&ir, 12000.0) < mid - 20.0, "{m:?}");
            }
        }

        #[test]
        fn darkens_off_axis() {
            let on = CabModel::V30.render(false, RATE);
            let off = CabModel::V30.render(true, RATE);
            let tilt = |ir: &[f32]| response(ir, 4000.0) - response(ir, 800.0);
            assert!(tilt(&off) < tilt(&on) - 4.0);
        }

        #[test]
        fn has_a_low_resonance_on_closed_backs() {
            let ir = CabModel::V30.render(false, RATE);
            assert!(response(&ir, 110.0) > response(&ir, 400.0));
        }
    }

    mod when_a_user_ir_is_missing {
        use super::*;

        #[test]
        fn falls_back_to_the_model_and_warns() {
            let state = CabState {
                ir_a: Some("/nope/missing.wav".into()),
                ..CabState::default()
            };
            let mut lib = Library::default();
            let mut cab = Cab::new(&state, RATE, &mut lib);
            assert_eq!(cab.warnings.len(), 1);
            let y: f32 = (0..512)
                .map(|i| cab.process(if i == 0 { 1.0 } else { 0.0 }).abs())
                .sum();
            assert!(y > 0.0);
        }
    }
}
