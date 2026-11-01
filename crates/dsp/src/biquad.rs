use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coefs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Default for Coefs {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

struct Pre {
    cos: f64,
    alpha: f64,
}

fn pre(hz: f32, q: f32, rate: f32) -> Pre {
    let hz = (hz as f64).clamp(1.0, rate as f64 * 0.49);
    let w = 2.0 * PI * hz / rate as f64;
    Pre {
        cos: w.cos(),
        alpha: w.sin() / (2.0 * (q as f64).max(0.01)),
    }
}

impl Coefs {
    fn norm(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    pub fn lowpass(hz: f32, q: f32, rate: f32) -> Self {
        let p = pre(hz, q, rate);
        let k = 1.0 - p.cos;
        Self::norm(
            k / 2.0,
            k,
            k / 2.0,
            1.0 + p.alpha,
            -2.0 * p.cos,
            1.0 - p.alpha,
        )
    }

    pub fn highpass(hz: f32, q: f32, rate: f32) -> Self {
        let p = pre(hz, q, rate);
        let k = 1.0 + p.cos;
        Self::norm(
            k / 2.0,
            -k,
            k / 2.0,
            1.0 + p.alpha,
            -2.0 * p.cos,
            1.0 - p.alpha,
        )
    }

    pub fn bandpass(hz: f32, q: f32, rate: f32) -> Self {
        let p = pre(hz, q, rate);
        Self::norm(
            p.alpha,
            0.0,
            -p.alpha,
            1.0 + p.alpha,
            -2.0 * p.cos,
            1.0 - p.alpha,
        )
    }

    pub fn allpass(hz: f32, q: f32, rate: f32) -> Self {
        let p = pre(hz, q, rate);
        Self::norm(
            1.0 - p.alpha,
            -2.0 * p.cos,
            1.0 + p.alpha,
            1.0 + p.alpha,
            -2.0 * p.cos,
            1.0 - p.alpha,
        )
    }

    pub fn peak(hz: f32, q: f32, gain_db: f32, rate: f32) -> Self {
        let p = pre(hz, q, rate);
        let a = 10f64.powf(gain_db as f64 / 40.0);
        Self::norm(
            1.0 + p.alpha * a,
            -2.0 * p.cos,
            1.0 - p.alpha * a,
            1.0 + p.alpha / a,
            -2.0 * p.cos,
            1.0 - p.alpha / a,
        )
    }

    pub fn low_shelf(hz: f32, gain_db: f32, rate: f32) -> Self {
        let p = pre(hz, std::f32::consts::FRAC_1_SQRT_2, rate);
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let s = 2.0 * a.sqrt() * p.alpha;
        Self::norm(
            a * ((a + 1.0) - (a - 1.0) * p.cos + s),
            2.0 * a * ((a - 1.0) - (a + 1.0) * p.cos),
            a * ((a + 1.0) - (a - 1.0) * p.cos - s),
            (a + 1.0) + (a - 1.0) * p.cos + s,
            -2.0 * ((a - 1.0) + (a + 1.0) * p.cos),
            (a + 1.0) + (a - 1.0) * p.cos - s,
        )
    }

    pub fn high_shelf(hz: f32, gain_db: f32, rate: f32) -> Self {
        let p = pre(hz, std::f32::consts::FRAC_1_SQRT_2, rate);
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let s = 2.0 * a.sqrt() * p.alpha;
        Self::norm(
            a * ((a + 1.0) + (a - 1.0) * p.cos + s),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * p.cos),
            a * ((a + 1.0) + (a - 1.0) * p.cos - s),
            (a + 1.0) - (a - 1.0) * p.cos + s,
            2.0 * ((a - 1.0) - (a + 1.0) * p.cos),
            (a + 1.0) - (a - 1.0) * p.cos - s,
        )
    }

    pub fn response_db(&self, hz: f32, rate: f32) -> f32 {
        let w = 2.0 * PI * hz as f64 / rate as f64;
        let (c1, s1) = (w.cos(), -w.sin());
        let (c2, s2) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = self.b1 * s1 + self.b2 * s2;
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = self.a1 * s1 + self.a2 * s2;
        let mag = ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt();
        (20.0 * mag.max(1e-12).log10()) as f32
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    c: Coefs,
    z1: f64,
    z2: f64,
}

impl Biquad {
    pub fn new(c: Coefs) -> Self {
        Self {
            c,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn set(&mut self, c: Coefs) {
        self.c = c;
    }

    pub fn coefs(&self) -> &Coefs {
        &self.c
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let x = x as f64;
        let c = &self.c;
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y as f32
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

pub fn chain_db(filters: &[Coefs], hz: f32, rate: f32) -> f32 {
    filters.iter().map(|c| c.response_db(hz, rate)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48000.0;

    mod when_designing_filters {
        use super::*;

        #[test]
        fn lowpass_passes_lows_and_cuts_highs() {
            let c = Coefs::lowpass(1000.0, 0.707, RATE);
            assert!(c.response_db(100.0, RATE).abs() < 0.1);
            assert!((c.response_db(1000.0, RATE) + 3.0).abs() < 0.2);
            assert!(c.response_db(10000.0, RATE) < -35.0);
        }

        #[test]
        fn peak_boosts_its_center() {
            let c = Coefs::peak(800.0, 1.0, 6.0, RATE);
            assert!((c.response_db(800.0, RATE) - 6.0).abs() < 0.05);
            assert!(c.response_db(50.0, RATE).abs() < 0.3);
        }

        #[test]
        fn shelves_reach_their_gain() {
            let low = Coefs::low_shelf(200.0, -9.0, RATE);
            assert!((low.response_db(20.0, RATE) + 9.0).abs() < 0.3);
            assert!(low.response_db(5000.0, RATE).abs() < 0.2);
            let high = Coefs::high_shelf(3000.0, 6.0, RATE);
            assert!((high.response_db(18000.0, RATE) - 6.0).abs() < 0.4);
        }

        #[test]
        fn running_matches_the_response() {
            let mut f = Biquad::new(Coefs::highpass(500.0, 0.707, RATE));
            let hz = 100.0;
            let out: Vec<f32> = (0..48000)
                .map(|i| f.process((i as f32 * hz * std::f32::consts::TAU / RATE).sin()))
                .collect();
            let peak = out[24000..].iter().fold(0.0f32, |a, v| a.max(v.abs()));
            let expected = 10f32.powf(f.coefs().response_db(hz, RATE) / 20.0);
            assert!((peak - expected).abs() < 0.01);
        }
    }
}
