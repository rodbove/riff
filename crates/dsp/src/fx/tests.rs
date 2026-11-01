use super::*;
use std::f32::consts::TAU;

const RATE: f32 = 48000.0;

fn sine(hz: f32, level: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| ((i % 48000) as f32 * hz * TAU / RATE).sin() * level)
        .collect()
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
}

fn run_mono(kind: Kind, values: &[f32], input: &[f32]) -> Vec<f32> {
    let mut fx = kind.mono(values, RATE).unwrap();
    input.iter().map(|x| fx.process(*x)).collect()
}

fn run_stereo(kind: Kind, values: &[f32], input: &[f32]) -> Vec<[f32; 2]> {
    let mut fx = kind.stereo(values, RATE).unwrap();
    input.iter().map(|x| fx.process([*x, *x])).collect()
}

fn with(kind: Kind, pairs: &[(&str, f32)]) -> Vec<f32> {
    crate::rig::Slot::with(kind, pairs).values
}

mod when_listing_effects {
    use super::*;

    #[test]
    fn builds_every_kind_on_the_right_side() {
        for k in Kind::PRE {
            assert!(!k.spec().post);
            assert!(k.mono(&k.defaults(), RATE).is_some(), "{k:?}");
            assert!(k.stereo(&k.defaults(), RATE).is_none(), "{k:?}");
        }
        for k in Kind::POST {
            assert!(k.spec().post);
            assert!(k.stereo(&k.defaults(), RATE).is_some(), "{k:?}");
        }
    }

    #[test]
    fn stays_finite_with_extreme_settings() {
        let input = sine(110.0, 0.9, 24000);
        for k in Kind::PRE.iter().chain(Kind::POST) {
            for pick in [0.0f32, 1.0] {
                let values: Vec<f32> = k
                    .spec()
                    .params
                    .iter()
                    .map(|p| if pick > 0.5 { p.max } else { p.min })
                    .collect();
                if k.spec().post {
                    for f in run_stereo(*k, &values, &input) {
                        assert!(
                            f[0].is_finite() && f[1].is_finite() && f[0].abs() < 100.0,
                            "{k:?}"
                        );
                    }
                } else {
                    for y in run_mono(*k, &values, &input) {
                        assert!(y.is_finite() && y.abs() < 100.0, "{k:?}");
                    }
                }
            }
        }
    }
}

mod when_driving {
    use super::*;

    #[test]
    fn more_gain_squares_the_wave() {
        let input = sine(220.0, 0.3, 9600);
        for kind in [Kind::Overdrive, Kind::Distortion, Kind::Swedish] {
            let name = if kind == Kind::Swedish {
                "DIST"
            } else if kind == Kind::Overdrive {
                "DRIVE"
            } else {
                "GAIN"
            };
            let crest = |g: f32| {
                let out = run_mono(kind, &with(kind, &[(name, g)]), &input);
                let tail = &out[4800..];
                tail.iter().fold(0.0f32, |a, v| a.max(v.abs())) / rms(tail)
            };
            assert!(crest(10.0) < crest(0.0), "{kind:?}");
        }
    }

    #[test]
    fn the_swedish_low_and_high_knobs_shape_the_eq() {
        let low = sine(90.0, 0.01, 9600);
        let flat = run_mono(
            Kind::Swedish,
            &with(Kind::Swedish, &[("LOW", 0.0), ("DIST", 0.0)]),
            &low,
        );
        let boosted = run_mono(
            Kind::Swedish,
            &with(Kind::Swedish, &[("LOW", 10.0), ("DIST", 0.0)]),
            &low,
        );
        assert!(rms(&boosted[4800..]) > rms(&flat[4800..]) * 2.0);
    }
}

mod when_compressing {
    use super::*;

    #[test]
    fn squeezes_loud_parts_more_than_quiet_parts() {
        let values = with(
            Kind::Compressor,
            &[("THRESH", -30.0), ("RATIO", 8.0), ("MAKEUP", 0.0)],
        );
        let loud = run_mono(Kind::Compressor, &values, &sine(200.0, 0.8, 9600));
        let quiet = run_mono(Kind::Compressor, &values, &sine(200.0, 0.01, 9600));
        let gain_loud = rms(&loud[4800..]) / rms(&sine(200.0, 0.8, 4800));
        let gain_quiet = rms(&quiet[4800..]) / rms(&sine(200.0, 0.01, 4800));
        assert!(gain_loud < gain_quiet * 0.3);
    }
}

mod when_filtering {
    use super::*;

    #[test]
    fn the_wah_moves_its_peak_with_the_pedal() {
        let probe = |pos: f32, hz: f32| {
            let out = run_mono(
                Kind::Wah,
                &with(Kind::Wah, &[("PEDAL", pos)]),
                &sine(hz, 0.1, 9600),
            );
            rms(&out[4800..])
        };
        assert!(probe(0.0, 350.0) > probe(1.0, 350.0) * 2.0);
        assert!(probe(1.0, 2200.0) > probe(0.0, 2200.0) * 2.0);
    }

    #[test]
    fn the_eq_boosts_one_band() {
        let input = sine(1000.0, 0.1, 9600);
        let out = run_mono(Kind::Eq, &with(Kind::Eq, &[("1K", 12.0)]), &input);
        let gain = rms(&out[4800..]) / rms(&input[4800..]);
        assert!((gain - 3.98).abs() < 0.3, "{gain}");
        let other = run_mono(
            Kind::Eq,
            &with(Kind::Eq, &[("1K", 12.0)]),
            &sine(62.0, 0.1, 9600),
        );
        assert!(rms(&other[4800..]) < 0.08);
    }

    #[test]
    fn the_octaver_adds_a_note_an_octave_down() {
        let input = sine(220.0, 0.4, 24000);
        let out = run_mono(
            Kind::Octaver,
            &with(Kind::Octaver, &[("OCT", 1.0), ("DRY", 0.0)]),
            &input,
        );
        let tail = &out[12000..];
        let crossings = tail
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        let expected = 110.0 * 2.0 * tail.len() as f32 / RATE;
        assert!(
            (crossings as f32 - expected).abs() < expected * 0.15,
            "{crossings} vs {expected}"
        );
    }
}

mod when_repeating {
    use super::*;

    #[test]
    fn the_delay_echoes_after_its_time() {
        let mut input = vec![0.0f32; 48000];
        input[0] = 1.0;
        let values = with(
            Kind::Delay,
            &[
                ("TIME", 250.0),
                ("MIX", 1.0),
                ("FEEDBACK", 0.0),
                ("TONE", 12000.0),
            ],
        );
        let mut fx = Kind::Delay.stereo(&values, RATE).unwrap();
        for _ in 0..48000 {
            fx.process([0.0, 0.0]);
        }
        let out: Vec<[f32; 2]> = input.iter().map(|x| fx.process([*x, *x])).collect();
        let peak = (1..out.len())
            .max_by(|a, b| out[*a][0].abs().total_cmp(&out[*b][0].abs()))
            .unwrap();
        assert!((peak as i32 - 12000).abs() < 30, "{peak}");
    }

    #[test]
    fn the_reverb_tail_decays_with_its_time() {
        let mut input = vec![0.0f32; 96000];
        input[0] = 1.0;
        let tail = |decay: f32| {
            let out = run_stereo(
                Kind::Reverb,
                &with(Kind::Reverb, &[("DECAY", decay), ("MIX", 1.0)]),
                &input,
            );
            let late: Vec<f32> = out[48000..].iter().map(|f| f[0]).collect();
            rms(&late)
        };
        assert!(tail(6.0) > tail(0.5) * 10.0);
        assert!(tail(0.5) < 1e-3);
    }
}

mod when_widening {
    use super::*;

    #[test]
    fn modulation_and_doubling_make_left_and_right_differ() {
        let input = sine(330.0, 0.3, 24000);
        for kind in [Kind::Chorus, Kind::Doubler, Kind::Phaser] {
            let out = run_stereo(kind, &kind.defaults(), &input);
            let diff: Vec<f32> = out[12000..].iter().map(|f| f[0] - f[1]).collect();
            assert!(rms(&diff) > 0.02, "{kind:?}");
        }
    }
}

mod when_shifting_pitch {
    use super::*;

    fn strongest(v: &[f32]) -> f32 {
        let power = |hz: f32| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (n, x) in v.iter().enumerate() {
                let w = std::f64::consts::TAU * hz as f64 * n as f64 / RATE as f64;
                re += *x as f64 * w.cos();
                im += *x as f64 * w.sin();
            }
            re * re + im * im
        };
        (500..6000)
            .map(|i| i as f32 * 0.1)
            .max_by(|a, b| power(*a).total_cmp(&power(*b)))
            .unwrap()
    }

    #[test]
    fn the_tune_pedal_moves_a_note_down() {
        for (shift, expected) in [(-12.0, 100.0), (-2.0, 178.18), (5.0, 266.97)] {
            let out = run_mono(
                Kind::Tune,
                &with(Kind::Tune, &[("SHIFT", shift)]),
                &sine(200.0, 0.3, 24000),
            );
            let hz = strongest(&out[12000..24000]);
            assert!((hz / expected - 1.0).abs() < 0.03, "{shift}: {hz}");
        }
    }

    #[test]
    fn the_harmony_adds_a_second_voice() {
        let input = sine(200.0, 0.3, 24000);
        let out = run_stereo(
            Kind::Harmony,
            &with(
                Kind::Harmony,
                &[("INTERVAL", 8.0), ("MIX", 1.0), ("WIDTH", 1.0)],
            ),
            &input,
        );
        let right: Vec<f32> = out.iter().map(|f| f[1] - 0.0).collect();
        let left: Vec<f32> = out.iter().map(|f| f[0]).collect();
        let diff: Vec<f32> = right.iter().zip(&left).map(|(r, l)| r - l).collect();
        let hz = strongest(&diff[12000..24000]);
        assert!((hz / 400.0 - 1.0).abs() < 0.03, "{hz}");
    }
}

mod when_pulsing {
    use super::*;

    #[test]
    fn the_tremolo_moves_the_volume_at_its_rate() {
        let input = vec![0.5f32; 48000];
        let out = run_stereo(
            Kind::Tremolo,
            &with(Kind::Tremolo, &[("RATE", 4.0), ("DEPTH", 1.0)]),
            &input,
        );
        let levels: Vec<f32> = out.iter().map(|f| f[0]).collect();
        let low = levels.iter().cloned().fold(1.0, f32::min);
        let high = levels.iter().cloned().fold(0.0, f32::max);
        assert!(low < 0.05 && high > 0.45);
        let dips = levels
            .windows(2)
            .filter(|w| w[0] >= 0.25 && w[1] < 0.25)
            .count();
        assert_eq!(dips, 4);
    }
}
