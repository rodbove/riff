pub mod drive;
pub mod dynamics;
pub mod filter;
pub mod line;
pub mod modulation;
pub mod pitch;
pub mod time;

use serde::{Deserialize, Serialize};

use crate::param::{Param, Unit};

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
    Compressor,
    Overdrive,
    Distortion,
    Swedish,
    Wah,
    Eq,
    Octaver,
    Tune,
    Acoustic,
    Chorus,
    Phaser,
    Flanger,
    Delay,
    Reverb,
    Doubler,
    PostEq,
    Harmony,
    Tremolo,
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

const COMPRESSOR: &[Param] = &[
    Param::new("THRESH", -60.0, 0.0, -24.0, Unit::Db),
    Param::new("RATIO", 1.0, 20.0, 4.0, Unit::Plain).curved(2.0),
    Param::new("ATTACK", 0.1, 50.0, 5.0, Unit::Ms).curved(2.0),
    Param::new("RELEASE", 20.0, 1000.0, 150.0, Unit::Ms).curved(2.0),
    Param::new("MAKEUP", 0.0, 24.0, 6.0, Unit::Db),
    Param::new("BLEND", 0.0, 1.0, 1.0, Unit::Percent),
];

const OVERDRIVE: &[Param] = &[
    Param::knob("DRIVE", 2.0),
    Param::knob("TONE", 5.0),
    Param::knob("LEVEL", 7.0),
];

const DISTORTION: &[Param] = &[
    Param::knob("GAIN", 5.0),
    Param::knob("FILTER", 5.0),
    Param::knob("LEVEL", 6.0),
];

const SWEDISH: &[Param] = &[
    Param::knob("LEVEL", 6.0),
    Param::knob("LOW", 10.0),
    Param::knob("HIGH", 10.0),
    Param::knob("DIST", 8.0),
];

const WAH: &[Param] = &[
    Param::new("PEDAL", 0.0, 1.0, 0.5, Unit::Percent),
    Param::choice("MODE", &["MANUAL", "AUTO"], 0.0),
    Param::new("SENS", 0.0, 1.0, 0.5, Unit::Percent),
    Param::new("Q", 2.0, 10.0, 6.0, Unit::Plain),
];

const EQ: &[Param] = &[
    Param::new("31", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("62", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("125", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("250", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("500", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("1K", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("2K", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("4K", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("8K", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("16K", -12.0, 12.0, 0.0, Unit::Db),
    Param::new("LEVEL", -12.0, 12.0, 0.0, Unit::Db),
];

const OCTAVER: &[Param] = &[
    Param::new("OCT", 0.0, 1.0, 0.5, Unit::Percent),
    Param::new("DRY", 0.0, 1.0, 1.0, Unit::Percent),
];

const TUNE: &[Param] = &[
    Param::new("SHIFT", -12.0, 12.0, -2.0, Unit::Semi),
    Param::new("MIX", 0.0, 1.0, 1.0, Unit::Percent),
];

const ACOUSTIC: &[Param] = &[
    Param::knob("BODY", 5.0),
    Param::knob("TOP", 5.0),
    Param::new("LEVEL", -12.0, 12.0, 0.0, Unit::Db),
];

const HARMONY: &[Param] = &[
    Param::choice(
        "INTERVAL",
        &[
            "-OCT", "-5TH", "-4TH", "-m3", "+m3", "+M3", "+4TH", "+5TH", "+OCT",
        ],
        5.0,
    ),
    Param::new("MIX", 0.0, 1.0, 0.7, Unit::Percent),
    Param::new("WIDTH", 0.0, 1.0, 0.8, Unit::Percent),
    Param::new("DELAY", 0.0, 40.0, 15.0, Unit::Ms),
];

const TREMOLO: &[Param] = &[
    Param::new("RATE", 0.5, 12.0, 5.0, Unit::Hz).curved(2.0),
    Param::new("DEPTH", 0.0, 1.0, 0.6, Unit::Percent),
    Param::choice("SHAPE", &["SINE", "SQUARE"], 0.0),
    Param::new("STEREO", 0.0, 1.0, 0.0, Unit::Percent),
];

const CHORUS: &[Param] = &[
    Param::new("RATE", 0.1, 5.0, 0.8, Unit::Hz).curved(2.0),
    Param::new("DEPTH", 0.0, 1.0, 0.5, Unit::Percent),
    Param::new("MIX", 0.0, 1.0, 0.5, Unit::Percent),
];

const PHASER: &[Param] = &[
    Param::new("RATE", 0.05, 5.0, 0.5, Unit::Hz).curved(2.0),
    Param::new("DEPTH", 0.0, 1.0, 0.7, Unit::Percent),
    Param::new("FEEDBACK", 0.0, 0.9, 0.5, Unit::Percent),
    Param::new("MIX", 0.0, 1.0, 0.5, Unit::Percent),
];

const FLANGER: &[Param] = &[
    Param::new("RATE", 0.05, 3.0, 0.3, Unit::Hz).curved(2.0),
    Param::new("DEPTH", 0.0, 1.0, 0.7, Unit::Percent),
    Param::new("FEEDBACK", -0.9, 0.9, 0.5, Unit::Percent),
    Param::new("MIX", 0.0, 1.0, 0.5, Unit::Percent),
];

const DELAY: &[Param] = &[
    Param::new("TIME", 20.0, 2000.0, 400.0, Unit::Ms).curved(2.0),
    Param::new("FEEDBACK", 0.0, 0.95, 0.35, Unit::Percent),
    Param::new("MIX", 0.0, 1.0, 0.25, Unit::Percent),
    Param::new("TONE", 1000.0, 12000.0, 5000.0, Unit::Hz).curved(2.0),
    Param::choice("MODE", &["DIGITAL", "ANALOG", "PING-PONG"], 0.0),
];

const REVERB: &[Param] = &[
    Param::choice("TYPE", &["ROOM", "PLATE", "HALL"], 1.0),
    Param::new("DECAY", 0.2, 8.0, 1.8, Unit::Plain).curved(2.0),
    Param::new("DAMP", 0.0, 1.0, 0.5, Unit::Percent),
    Param::new("PRE", 0.0, 200.0, 20.0, Unit::Ms),
    Param::new("MIX", 0.0, 1.0, 0.2, Unit::Percent),
];

const DOUBLER: &[Param] = &[
    Param::new("WIDTH", 0.0, 1.0, 0.8, Unit::Percent),
    Param::new("DELAY", 5.0, 40.0, 18.0, Unit::Ms),
];

const fn spec(
    name: &'static str,
    label: &'static str,
    color: [u8; 3],
    post: bool,
    params: &'static [Param],
) -> Spec {
    Spec {
        name,
        label,
        color,
        post,
        params,
    }
}

impl Kind {
    pub const PRE: &'static [Kind] = &[
        Kind::Boost,
        Kind::Compressor,
        Kind::Overdrive,
        Kind::Distortion,
        Kind::Swedish,
        Kind::Wah,
        Kind::Eq,
        Kind::Octaver,
        Kind::Tune,
        Kind::Acoustic,
    ];
    pub const POST: &'static [Kind] = &[
        Kind::Chorus,
        Kind::Phaser,
        Kind::Flanger,
        Kind::Delay,
        Kind::Reverb,
        Kind::Doubler,
        Kind::PostEq,
        Kind::Harmony,
        Kind::Tremolo,
    ];

    pub fn spec(self) -> Spec {
        match self {
            Kind::Boost => spec("BOOST", "clean boost", [0xd8, 0xd4, 0xc8], false, BOOST),
            Kind::Compressor => spec("COMP", "compressor", [0x3a, 0x6e, 0xc4], false, COMPRESSOR),
            Kind::Overdrive => spec(
                "OD-9",
                "tube screamer style",
                [0x2f, 0xa3, 0x5a],
                false,
                OVERDRIVE,
            ),
            Kind::Distortion => spec(
                "RODENT",
                "rat style distortion",
                [0x2a, 0x2a, 0x2a],
                false,
                DISTORTION,
            ),
            Kind::Swedish => spec(
                "SWEDISH",
                "hm-2 chainsaw",
                [0xe0, 0x6c, 0x1e],
                false,
                SWEDISH,
            ),
            Kind::Wah => spec("WAH", "cry baby style", [0x30, 0x30, 0x30], false, WAH),
            Kind::Eq => spec("EQ-10", "10 band graphic eq", [0xc8, 0xc8, 0xc8], false, EQ),
            Kind::Octaver => spec("OCTAVE", "octave down", [0x7a, 0x3d, 0xb8], false, OCTAVER),
            Kind::Tune => spec(
                "TUNE",
                "pitch shift and drop tune",
                [0x5a, 0x2d, 0x82],
                false,
                TUNE,
            ),
            Kind::Acoustic => spec(
                "ACOUSTIC",
                "acoustic guitar simulator",
                [0xb8, 0x86, 0x4b],
                false,
                ACOUSTIC,
            ),
            Kind::Harmony => spec(
                "HARMONY",
                "twin lead harmonizer",
                [0x1f, 0x6f, 0x6a],
                true,
                HARMONY,
            ),
            Kind::Tremolo => spec("TREM", "tremolo", [0x8c, 0x23, 0x2a], true, TREMOLO),
            Kind::Chorus => spec("CHORUS", "stereo chorus", [0x4a, 0xa8, 0xd8], true, CHORUS),
            Kind::Phaser => spec("PHASE", "6 stage phaser", [0xf0, 0x8c, 0x1c], true, PHASER),
            Kind::Flanger => spec("FLANGE", "flanger", [0x8a, 0x8a, 0x9a], true, FLANGER),
            Kind::Delay => spec("DELAY", "delay", [0x2b, 0x5d, 0x9e], true, DELAY),
            Kind::Reverb => spec("VERB", "reverb", [0x5e, 0x9e, 0x8a], true, REVERB),
            Kind::Doubler => spec(
                "DOUBLE",
                "stereo doubler",
                [0xb0, 0x3a, 0x48],
                true,
                DOUBLER,
            ),
            Kind::PostEq => spec("EQ-10", "post graphic eq", [0xc8, 0xc8, 0xc8], true, EQ),
        }
    }

    pub fn defaults(self) -> Vec<f32> {
        self.spec().params.iter().map(|p| p.default).collect()
    }

    pub fn mono(self, values: &[f32], rate: f32) -> Option<Box<dyn Mono>> {
        let mut fx: Box<dyn Mono> = match self {
            Kind::Boost => Box::new(drive::Boost::new(rate)),
            Kind::Compressor => Box::new(dynamics::Compressor::new(rate)),
            Kind::Overdrive => Box::new(drive::Overdrive::new(rate)),
            Kind::Distortion => Box::new(drive::Distortion::new(rate)),
            Kind::Swedish => Box::new(drive::Swedish::new(rate)),
            Kind::Wah => Box::new(filter::Wah::new(rate)),
            Kind::Eq => Box::new(filter::Eq::new(rate)),
            Kind::Octaver => Box::new(filter::Octaver::new(rate)),
            Kind::Tune => Box::new(pitch::Tune::new(rate)),
            Kind::Acoustic => Box::new(pitch::Acoustic::new(rate)),
            _ => return None,
        };
        for (i, p) in self.spec().params.iter().enumerate() {
            fx.set(i, p.clamp(values.get(i).copied().unwrap_or(p.default)));
        }
        Some(fx)
    }

    pub fn stereo(self, values: &[f32], rate: f32) -> Option<Box<dyn Stereo>> {
        let mut fx: Box<dyn Stereo> = match self {
            Kind::Chorus => Box::new(modulation::Chorus::new(rate)),
            Kind::Phaser => Box::new(modulation::Phaser::new(rate)),
            Kind::Flanger => Box::new(modulation::Flanger::new(rate)),
            Kind::Delay => Box::new(time::Delay::new(rate)),
            Kind::Reverb => Box::new(time::Reverb::new(rate)),
            Kind::Doubler => Box::new(modulation::Doubler::new(rate)),
            Kind::PostEq => Box::new(PostEq(filter::Eq::new(rate))),
            Kind::Harmony => Box::new(pitch::Harmony::new(rate)),
            Kind::Tremolo => Box::new(pitch::Tremolo::new(rate)),
            _ => return None,
        };
        for (i, p) in self.spec().params.iter().enumerate() {
            fx.set(i, p.clamp(values.get(i).copied().unwrap_or(p.default)));
        }
        Some(fx)
    }
}

pub struct PostEq(filter::Eq);

impl Stereo for PostEq {
    fn set(&mut self, param: usize, value: f32) {
        self.0.set_param(param, value);
    }

    fn process(&mut self, x: [f32; 2]) -> [f32; 2] {
        let level = self.0.level();
        [
            self.0.channel(0, x[0]) * level,
            self.0.channel(1, x[1]) * level,
        ]
    }
}

#[cfg(test)]
mod tests;
