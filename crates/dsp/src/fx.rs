use serde::{Deserialize, Serialize};

use crate::param::{Param, Unit};
use crate::util::{Smooth, db};

pub trait Mono: Send {
    fn set(&mut self, param: usize, value: f32);
    fn process(&mut self, x: f32) -> f32;
}

pub trait Stereo: Send {
    fn set(&mut self, param: usize, value: f32);
    fn process(&mut self, x: [f32; 2]) -> [f32; 2];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Boost,
}

pub struct Spec {
    pub name: &'static str,
    pub label: &'static str,
    pub color: [u8; 3],
    pub post: bool,
    pub params: &'static [Param],
}

const BOOST: &[Param] = &[
    Param::new("LEVEL", -12.0, 24.0, 6.0, Unit::Db),
    Param::new("TIGHT", 20.0, 400.0, 20.0, Unit::Hz).curved(2.0),
];

impl Kind {
    pub const PRE: &'static [Kind] = &[Kind::Boost];
    pub const POST: &'static [Kind] = &[];

    pub fn spec(self) -> Spec {
        match self {
            Kind::Boost => Spec {
                name: "BOOST",
                label: "clean boost",
                color: [0xd8, 0xd4, 0xc8],
                post: false,
                params: BOOST,
            },
        }
    }

    pub fn defaults(self) -> Vec<f32> {
        self.spec().params.iter().map(|p| p.default).collect()
    }

    pub fn mono(self, values: &[f32], rate: f32) -> Option<Box<dyn Mono>> {
        let mut fx: Box<dyn Mono> = match self {
            Kind::Boost => Box::new(Boost::new(rate)),
        };
        apply(fx.as_mut(), self, values);
        Some(fx)
    }

    pub fn stereo(self, _values: &[f32], _rate: f32) -> Option<Box<dyn Stereo>> {
        None
    }
}

fn apply(fx: &mut dyn Mono, kind: Kind, values: &[f32]) {
    for (i, p) in kind.spec().params.iter().enumerate() {
        fx.set(i, p.clamp(values.get(i).copied().unwrap_or(p.default)));
    }
}

pub struct Boost {
    rate: f32,
    gain: Smooth,
    tight: crate::biquad::Biquad,
}

impl Boost {
    pub fn new(rate: f32) -> Self {
        Self {
            rate,
            gain: Smooth::new(1.0, 20.0, rate),
            tight: crate::biquad::Biquad::default(),
        }
    }
}

impl Mono for Boost {
    fn set(&mut self, param: usize, value: f32) {
        match param {
            0 => self.gain.set(db(value)),
            _ => self
                .tight
                .set(crate::biquad::Coefs::highpass(value, 0.707, self.rate)),
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        self.tight.process(x) * self.gain.tick()
    }
}
