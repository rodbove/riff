pub fn tanh(x: f32) -> f32 {
    if x > 3.0 {
        1.0
    } else if x < -3.0 {
        -1.0
    } else {
        let x2 = x * x;
        x * (27.0 + x2) / (27.0 + 9.0 * x2)
    }
}

pub fn tube(x: f32, bias: f32) -> f32 {
    let shape = |v: f32| {
        if v >= 0.0 {
            tanh(v)
        } else {
            tanh(v * 0.6) / 0.6
        }
    };
    shape(x + bias) - shape(bias)
}

pub fn diode(x: f32) -> f32 {
    x / (1.0 + x.abs().powf(2.5)).powf(0.4)
}

pub fn hard(x: f32, limit: f32) -> f32 {
    x.clamp(-limit, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_clipping {
        use super::*;

        #[test]
        fn stays_bounded() {
            for i in -1000..1000 {
                let x = i as f32 * 0.1;
                assert!(tanh(x).abs() <= 1.0);
                assert!(diode(x).abs() <= 1.01);
                assert!(tube(x, 0.2).abs() < 3.0);
            }
        }

        #[test]
        fn is_linear_for_small_signals() {
            assert!((tanh(0.01) - 0.01).abs() < 1e-5);
            assert!((diode(0.01) - 0.01).abs() < 1e-4);
        }

        #[test]
        fn makes_tubes_asymmetric() {
            let up = tube(2.0, 0.2);
            let down = tube(-2.0, 0.2);
            assert!((up + down).abs() > 0.1);
            assert_eq!(tube(0.0, 0.3), 0.0);
        }
    }
}
