use serde::{Deserialize, Serialize};

use crate::amp::{self, AmpModel};
use crate::cab::CabState;
use crate::fx::Kind;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GateState {
    pub on: bool,
    pub threshold: f32,
    pub release: f32,
}

impl Default for GateState {
    fn default() -> Self {
        Self {
            on: true,
            threshold: -55.0,
            release: 80.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub kind: Kind,
    #[serde(default = "on")]
    pub on: bool,
    #[serde(default)]
    pub values: Vec<f32>,
}

fn on() -> bool {
    true
}

impl Slot {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            on: true,
            values: kind.defaults(),
        }
    }

    pub fn with(kind: Kind, pairs: &[(&str, f32)]) -> Self {
        let mut slot = Self::new(kind);
        let spec = kind.spec();
        for (name, v) in pairs {
            if let Some(i) = spec.params.iter().position(|p| p.name == *name) {
                slot.values[i] = spec.params[i].clamp(*v);
            }
        }
        slot
    }

    pub fn value(&self, i: usize) -> f32 {
        let spec = self.kind.spec();
        self.values
            .get(i)
            .copied()
            .unwrap_or_else(|| spec.params.get(i).map(|p| p.default).unwrap_or(0.0))
    }

    pub fn fix(&mut self) {
        let spec = self.kind.spec();
        self.values.resize(spec.params.len(), 0.0);
        for (v, p) in self.values.iter_mut().zip(spec.params) {
            *v = p.clamp(*v);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmpState {
    pub on: bool,
    pub model: AmpModel,
    pub values: Vec<f32>,
    pub use_nam: bool,
    pub nam: Option<String>,
    pub nam_values: Vec<f32>,
}

impl Default for AmpState {
    fn default() -> Self {
        Self {
            on: true,
            model: AmpModel::Brit,
            values: amp::PARAMS.iter().map(|p| p.default).collect(),
            use_nam: false,
            nam: None,
            nam_values: crate::nam::PARAMS.iter().map(|p| p.default).collect(),
        }
    }
}

impl AmpState {
    pub fn with(model: AmpModel, values: [f32; 8]) -> Self {
        Self {
            model,
            values: values.to_vec(),
            ..Self::default()
        }
    }

    pub fn fix(&mut self) {
        self.values.resize(amp::PARAMS.len(), 0.0);
        for (v, p) in self.values.iter_mut().zip(amp::PARAMS) {
            *v = p.clamp(*v);
        }
        self.nam_values.resize(crate::nam::PARAMS.len(), 0.0);
        for (v, p) in self.nam_values.iter_mut().zip(crate::nam::PARAMS) {
            *v = p.clamp(*v);
        }
    }
}

pub const MAX_PRE: usize = 8;
pub const MAX_POST: usize = 6;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rig {
    pub name: String,
    pub category: String,
    pub input: f32,
    pub output: f32,
    pub gate: GateState,
    pub pre: Vec<Slot>,
    pub amp: AmpState,
    pub cab: CabState,
    pub post: Vec<Slot>,
}

impl Default for Rig {
    fn default() -> Self {
        Self {
            name: "Init".into(),
            category: "USER".into(),
            input: 0.0,
            output: -6.0,
            gate: GateState::default(),
            pre: Vec::new(),
            amp: AmpState::default(),
            cab: CabState::default(),
            post: Vec::new(),
        }
    }
}

impl Rig {
    pub fn fix(&mut self) {
        self.pre.truncate(MAX_PRE);
        self.post.truncate(MAX_POST);
        self.pre.retain(|s| !s.kind.spec().post);
        self.post.retain(|s| s.kind.spec().post);
        for s in self.pre.iter_mut().chain(self.post.iter_mut()) {
            s.fix();
        }
        self.amp.fix();
        self.cab.fix();
        self.input = self.input.clamp(-24.0, 24.0);
        self.output = self.output.clamp(-60.0, 12.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_loading_a_rig {
        use super::*;

        #[test]
        fn fills_missing_values_and_clamps() {
            let mut rig: Rig =
                serde_json::from_str(r#"{"pre":[{"kind":"Boost","values":[99.0]}],"input":80}"#)
                    .unwrap();
            rig.fix();
            assert_eq!(rig.pre[0].values.len(), Kind::Boost.spec().params.len());
            assert_eq!(rig.pre[0].values[0], 24.0);
            assert!(rig.pre[0].on);
            assert_eq!(rig.input, 24.0);
        }

        #[test]
        fn round_trips_through_json() {
            let mut rig = Rig::default();
            rig.pre.push(Slot::with(Kind::Boost, &[("LEVEL", 3.0)]));
            let back: Rig = serde_json::from_str(&serde_json::to_string(&rig).unwrap()).unwrap();
            assert_eq!(back, rig);
        }
    }
}
