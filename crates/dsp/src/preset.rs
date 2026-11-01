use crate::amp::AmpModel;
use crate::cab::{CabModel, CabState};
use crate::fx::Kind;
use crate::rig::{AmpState, GateState, Rig, Slot};

pub const BANDS: [&str; 6] = [
    "METALLICA",
    "DREAM THEATER",
    "DEATH",
    "AVENGED SEVENFOLD",
    "ARCH ENEMY",
    "EXTRAS",
];
pub const USER: &str = "USER";

fn rig(
    name: &str,
    band: &str,
    gate: f32,
    pre: Vec<Slot>,
    amp: AmpState,
    cab: CabState,
    post: Vec<Slot>,
) -> Rig {
    Rig {
        name: name.into(),
        category: band.into(),
        input: 0.0,
        output: -6.0,
        gate: GateState {
            on: gate < 0.0,
            threshold: if gate < 0.0 { gate } else { -60.0 },
            release: 60.0,
        },
        pre,
        amp,
        cab,
        post,
    }
}

fn cab(model: CabModel, blend: f32, low_cut: f32, high_cut: f32) -> CabState {
    CabState {
        values: vec![blend, low_cut, high_cut, 0.0],
        ..CabState::with(model)
    }
}

fn od(drive: f32, tone: f32, level: f32) -> Slot {
    Slot::with(
        Kind::Overdrive,
        &[("DRIVE", drive), ("TONE", tone), ("LEVEL", level)],
    )
}

fn v_curve(depth: f32) -> Slot {
    Slot::with(
        Kind::PostEq,
        &[
            ("62", 2.0 * depth),
            ("125", 3.0 * depth),
            ("500", -3.0 * depth),
            ("1K", -3.5 * depth),
            ("2K", -depth),
            ("4K", 1.5 * depth),
            ("8K", 2.0 * depth),
        ],
    )
}

fn doubler(width: f32) -> Slot {
    Slot::with(Kind::Doubler, &[("WIDTH", width), ("DELAY", 16.0)])
}

fn delay(ms: f32, feedback: f32, mix: f32, mode: f32) -> Slot {
    Slot::with(
        Kind::Delay,
        &[
            ("TIME", ms),
            ("FEEDBACK", feedback),
            ("MIX", mix),
            ("TONE", 4500.0),
            ("MODE", mode),
        ],
    )
}

fn verb(kind: f32, decay: f32, mix: f32) -> Slot {
    Slot::with(
        Kind::Reverb,
        &[("TYPE", kind), ("DECAY", decay), ("MIX", mix)],
    )
}

fn chorus(rate: f32, depth: f32, mix: f32) -> Slot {
    Slot::with(
        Kind::Chorus,
        &[("RATE", rate), ("DEPTH", depth), ("MIX", mix)],
    )
}

fn comp() -> Slot {
    Slot::with(
        Kind::Compressor,
        &[
            ("THRESH", -28.0),
            ("RATIO", 3.0),
            ("ATTACK", 10.0),
            ("RELEASE", 200.0),
            ("MAKEUP", 6.0),
        ],
    )
}

fn amp(model: AmpModel, v: [f32; 8]) -> AmpState {
    AmpState::with(model, v)
}

const DIGITAL: f32 = 0.0;
const ANALOG: f32 = 1.0;
const PING_PONG: f32 = 2.0;
const ROOM: f32 = 0.0;
const PLATE: f32 = 1.0;
const HALL: f32 = 2.0;

const TRIMS: [(&str, f32); 7] = [
    ("Fade Clean", 7.0),
    ("Petrucci Clean", 6.0),
    ("Clean Room", 3.0),
    ("Puppets Rhythm", -8.0),
    ("Black Album Rhythm", -8.0),
    ("Swedish Death", -8.0),
    ("Amott Lead", -8.0),
];

pub fn factory() -> Vec<Rig> {
    let mut bank = bank();
    for r in &mut bank {
        if let Some((_, out)) = TRIMS.iter().find(|(n, _)| *n == r.name) {
            r.output = *out;
        }
    }
    bank
}

fn bank() -> Vec<Rig> {
    use AmpModel::*;
    use CabModel::*;
    vec![
        rig(
            "Puppets Rhythm",
            BANDS[0],
            -50.0,
            vec![],
            amp(Mark, [7.0, 5.0, 4.0, 7.0, 6.0, 6.0, 5.0, 0.0]),
            cab(V30, 0.3, 70.0, 9000.0),
            vec![v_curve(1.0), doubler(0.7)],
        ),
        rig(
            "Black Album Rhythm",
            BANDS[0],
            -55.0,
            vec![od(1.0, 6.0, 7.0)],
            amp(Recto, [6.5, 6.0, 4.0, 6.0, 6.0, 6.0, 5.0, 0.0]),
            cab(V30, 0.4, 70.0, 8500.0),
            vec![doubler(0.8)],
        ),
        rig(
            "Kirk Lead",
            BANDS[0],
            -58.0,
            vec![Slot::with(Kind::Wah, &[("PEDAL", 0.6)]), od(2.0, 6.0, 6.0)],
            amp(Mark, [8.0, 5.0, 6.0, 6.0, 6.0, 5.0, 5.0, 0.0]),
            cab(V30, 0.3, 80.0, 9000.0),
            vec![delay(380.0, 0.3, 0.2, ANALOG), verb(PLATE, 1.6, 0.15)],
        ),
        rig(
            "Fade Clean",
            BANDS[0],
            0.0,
            vec![comp()],
            amp(Clean, [3.0, 5.0, 5.0, 6.0, 5.0, 5.0, 5.0, 1.0]),
            cab(OpenBack, 0.3, 60.0, 10000.0),
            vec![
                chorus(0.6, 0.4, 0.4),
                delay(450.0, 0.3, 0.2, DIGITAL),
                verb(HALL, 2.5, 0.2),
            ],
        ),
        rig(
            "Petrucci Rhythm",
            BANDS[1],
            -55.0,
            vec![od(0.5, 6.0, 8.0)],
            amp(Mark, [6.5, 4.5, 5.0, 6.5, 6.0, 6.0, 5.0, 0.0]),
            cab(V30, 0.35, 75.0, 9000.0),
            vec![v_curve(0.9), doubler(0.7)],
        ),
        rig(
            "Petrucci Lead",
            BANDS[1],
            -60.0,
            vec![od(1.0, 6.0, 8.0)],
            amp(Mark, [7.5, 4.5, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0]),
            cab(V30, 0.35, 80.0, 9500.0),
            vec![delay(420.0, 0.35, 0.25, PING_PONG), verb(PLATE, 2.2, 0.2)],
        ),
        rig(
            "Petrucci Clean",
            BANDS[1],
            0.0,
            vec![comp()],
            amp(Clean, [3.0, 5.0, 6.0, 6.0, 5.0, 5.0, 5.0, 0.0]),
            cab(OpenBack, 0.4, 60.0, 11000.0),
            vec![
                chorus(0.8, 0.5, 0.5),
                delay(500.0, 0.35, 0.25, PING_PONG),
                verb(HALL, 3.0, 0.25),
            ],
        ),
        rig(
            "Symbolic Rhythm",
            BANDS[2],
            -52.0,
            vec![od(3.0, 5.0, 8.0)],
            amp(Brit, [7.0, 6.0, 6.0, 6.0, 6.0, 5.0, 5.0, 0.0]),
            cab(V30, 0.3, 70.0, 8500.0),
            vec![doubler(0.6)],
        ),
        rig(
            "Chuck Lead",
            BANDS[2],
            -58.0,
            vec![od(4.0, 6.0, 8.0)],
            amp(Brit, [8.0, 5.0, 7.0, 6.0, 6.0, 5.0, 5.0, 0.0]),
            cab(V30, 0.3, 80.0, 9000.0),
            vec![delay(350.0, 0.3, 0.2, ANALOG), verb(HALL, 2.0, 0.18)],
        ),
        rig(
            "A7X Rhythm",
            BANDS[3],
            -58.0,
            vec![od(0.5, 6.0, 8.0)],
            amp(Uber, [6.5, 6.0, 5.0, 6.0, 6.0, 6.0, 5.0, 0.0]),
            cab(V30, 0.3, 75.0, 9000.0),
            vec![doubler(0.8)],
        ),
        rig(
            "Synyster Lead",
            BANDS[3],
            -60.0,
            vec![od(1.0, 6.0, 8.0)],
            amp(Block, [7.5, 5.0, 6.5, 6.0, 6.0, 5.0, 5.0, 0.0]),
            cab(V30, 0.35, 80.0, 9500.0),
            vec![delay(375.0, 0.4, 0.25, PING_PONG), verb(HALL, 2.8, 0.22)],
        ),
        rig(
            "Amott Rhythm",
            BANDS[4],
            -55.0,
            vec![od(0.5, 5.5, 8.0)],
            amp(Block, [6.5, 6.0, 5.5, 6.0, 6.0, 6.0, 5.0, 0.0]),
            cab(V30, 0.3, 75.0, 9000.0),
            vec![doubler(0.8)],
        ),
        rig(
            "Amott Lead",
            BANDS[4],
            -60.0,
            vec![od(2.0, 6.0, 8.0)],
            amp(Block, [7.5, 5.0, 7.0, 6.0, 5.5, 5.0, 5.0, 0.0]),
            cab(V30, 0.35, 80.0, 9500.0),
            vec![delay(400.0, 0.4, 0.25, ANALOG), verb(HALL, 2.5, 0.2)],
        ),
        rig(
            "Swedish Death",
            BANDS[5],
            -50.0,
            vec![Slot::with(
                Kind::Swedish,
                &[("LEVEL", 6.0), ("LOW", 10.0), ("HIGH", 10.0), ("DIST", 9.0)],
            )],
            amp(Brit, [3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 0.0]),
            cab(Greenback, 0.3, 60.0, 8000.0),
            vec![],
        ),
        rig(
            "Modern Djent",
            BANDS[5],
            -50.0,
            vec![od(0.0, 6.0, 9.0)],
            amp(Block, [6.0, 5.0, 5.0, 6.5, 6.5, 5.0, 5.0, 0.0]),
            cab(V30, 0.3, 90.0, 9500.0),
            vec![doubler(0.9)],
        ),
        rig(
            "Clean Room",
            BANDS[5],
            0.0,
            vec![],
            amp(Clean, [3.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 0.0]),
            cab(OpenBack, 0.3, 60.0, 11000.0),
            vec![verb(ROOM, 0.8, 0.2)],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;
    use crate::library::Library;
    use std::f32::consts::TAU;

    const RATE: f32 = 48000.0;

    fn riff(i: usize) -> f32 {
        let t = i as f32 / RATE;
        let gate = if (i / 6000).is_multiple_of(2) {
            1.0
        } else {
            0.3
        };
        gate * 0.15
            * ((t * 82.4 * TAU).sin() + (t * 123.5 * TAU).sin() + 0.4 * (t * 247.0 * TAU).sin())
    }

    fn level(rig: &Rig) -> f32 {
        let mut c = Chain::build(rig, RATE, &mut Library::default());
        let mut sum = 0.0;
        let n = 48000;
        for i in 0..n {
            let x = riff(i) * c.input_gain();
            let [l, r] = c.process(x);
            assert!(
                l.is_finite() && r.is_finite() && l.abs() <= 1.0,
                "{}",
                rig.name
            );
            if i > n / 2 {
                sum += l * l + r * r;
            }
        }
        (sum / n as f32).sqrt()
    }

    mod when_loading_the_factory_bank {
        use super::*;

        #[test]
        fn covers_every_band_with_unique_names() {
            let bank = factory();
            for band in BANDS {
                assert!(
                    bank.iter().filter(|r| r.category == band).count() >= 2,
                    "{band}"
                );
            }
            for (i, r) in bank.iter().enumerate() {
                assert!(
                    !bank[i + 1..].iter().any(|o| o.name == r.name),
                    "{}",
                    r.name
                );
            }
        }

        #[test]
        fn plays_every_preset_at_a_similar_level() {
            let levels: Vec<(String, f32)> = factory()
                .iter()
                .map(|r| (r.name.clone(), level(r)))
                .collect();
            let loudest = levels.iter().map(|l| l.1).fold(0.0, f32::max);
            for (name, l) in &levels {
                assert!(*l > loudest / 2.0, "{name} {l} vs {loudest}");
            }
        }

        #[test]
        fn survives_a_json_round_trip() {
            for r in factory() {
                let back: Rig = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
                assert_eq!(back, r);
            }
        }

        #[test]
        fn keeps_every_value_inside_its_range() {
            for r in factory() {
                let mut fixed = r.clone();
                fixed.fix();
                assert_eq!(fixed, r, "{}", r.name);
            }
        }
    }
}
