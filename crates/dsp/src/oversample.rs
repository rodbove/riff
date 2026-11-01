use std::f64::consts::PI;

const COEFS: usize = 12;
const TRANSITION: f64 = 0.04;

pub fn design(count: usize, transition: f64) -> Vec<f64> {
    let mut k = ((1.0 - transition * 2.0) * PI / 4.0).tan();
    k *= k;
    let kksqrt = (1.0 - k * k).powf(0.25);
    let e = 0.5 * (1.0 - kksqrt) / (1.0 + kksqrt);
    let e2 = e * e;
    let e4 = e2 * e2;
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    let order = count * 2 + 1;
    (0..count)
        .map(|index| {
            let c = (index + 1) as f64;
            let num = acc_num(q, order, c) * q.powf(0.25);
            let den = acc_den(q, order, c) + 0.5;
            let ww = num / den;
            let wwsq = ww * ww;
            let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
            (1.0 - x) / (1.0 + x)
        })
        .collect()
}

fn acc_num(q: f64, order: usize, c: f64) -> f64 {
    let mut acc = 0.0;
    let mut sign = 1.0;
    for i in 0..64 {
        let i = i as f64;
        let term = q.powf(i * (i + 1.0)) * ((i * 2.0 + 1.0) * c * PI / order as f64).sin() * sign;
        acc += term;
        sign = -sign;
        if term.abs() < 1e-100 {
            break;
        }
    }
    acc
}

fn acc_den(q: f64, order: usize, c: f64) -> f64 {
    let mut acc = 0.0;
    let mut sign = -1.0;
    for i in 1..64 {
        let i = i as f64;
        let term = q.powf(i * i) * (i * 2.0 * c * PI / order as f64).cos() * sign;
        acc += term;
        sign = -sign;
        if term.abs() < 1e-100 {
            break;
        }
    }
    acc
}

#[derive(Clone, Copy, Debug, Default)]
struct Allpass {
    a: f32,
    x1: f32,
    y1: f32,
}

impl Allpass {
    fn process(&mut self, x: f32) -> f32 {
        let y = self.x1 + self.a * (x - self.y1);
        self.x1 = x;
        self.y1 = y;
        y
    }
}

#[derive(Clone, Debug)]
struct Half {
    even: [Allpass; COEFS / 2],
    odd: [Allpass; COEFS / 2],
}

impl Half {
    fn new() -> Self {
        let c = design(COEFS, TRANSITION);
        let make = |start: usize| {
            std::array::from_fn(|i| Allpass {
                a: c[start + i * 2] as f32,
                ..Allpass::default()
            })
        };
        Self {
            even: make(0),
            odd: make(1),
        }
    }

    fn up(&mut self, x: f32) -> [f32; 2] {
        let mut e = x;
        let mut o = x;
        for s in &mut self.even {
            e = s.process(e);
        }
        for s in &mut self.odd {
            o = s.process(o);
        }
        [e, o]
    }

    fn down(&mut self, pair: [f32; 2]) -> f32 {
        let mut a = pair[1];
        let mut b = pair[0];
        for s in &mut self.even {
            a = s.process(a);
        }
        for s in &mut self.odd {
            b = s.process(b);
        }
        0.5 * (a + b)
    }
}

#[derive(Clone, Debug)]
pub struct Oversampler {
    up1: Half,
    up2: Half,
    down1: Half,
    down2: Half,
}

impl Default for Oversampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Oversampler {
    pub const FACTOR: usize = 4;

    pub fn new() -> Self {
        Self {
            up1: Half::new(),
            up2: Half::new(),
            down1: Half::new(),
            down2: Half::new(),
        }
    }

    pub fn process(&mut self, x: f32, mut f: impl FnMut(f32) -> f32) -> f32 {
        let [a, b] = self.up1.up(x);
        let [a0, a1] = self.up2.up(a);
        let [b0, b1] = self.up2.up(b);
        let da = self.down2.down([f(a0), f(a1)]);
        let db = self.down2.down([f(b0), f(b1)]);
        self.down1.down([da, db])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt()
    }

    mod when_designing {
        use super::*;

        #[test]
        fn makes_stable_allpass_coefficients() {
            let c = design(COEFS, TRANSITION);
            assert_eq!(c.len(), COEFS);
            assert!(c.iter().all(|a| *a > 0.0 && *a < 1.0));
            assert!(c.windows(2).all(|w| w[0] < w[1]));
        }
    }

    mod when_resampling {
        use super::*;

        #[test]
        fn keeps_the_level_of_a_low_tone() {
            let mut o = Oversampler::new();
            let out: Vec<f32> = (0..48000)
                .map(|i| o.process((i as f32 * 1000.0 * TAU / 48000.0).sin() * 0.5, |x| x))
                .collect();
            assert!((rms(&out[4800..]) - 0.5 / 2f32.sqrt()).abs() < 0.005);
        }

        #[test]
        fn removes_images_above_the_base_band() {
            let mut h = Half::new();
            let rate = 96000.0f64;
            let mut out = Vec::new();
            for i in (0..96000).step_by(2) {
                let s = |n: usize| {
                    ((n % 12) as f64 * 40000.0 * std::f64::consts::TAU / rate).sin() as f32
                };
                out.push(h.down([s(i), s(i + 1)]));
            }
            assert!(rms(&out[2400..]) < 0.0005, "{}", rms(&out[2400..]));
        }
    }
}
