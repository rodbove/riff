use riff_dsp::amp::{Amp, AmpModel};
use riff_dsp::biquad::{Biquad, Coefs};
use riff_dsp::cab::CabModel;
use riff_dsp::fx::Mono;

const RATE: f32 = 48000.0;

struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

fn pluck(len: usize, hz: f32, level: f32, tau: f32, click: f32, seed: u32) -> Vec<f32> {
    let mut noise = Noise(seed | 1);
    let mut hp = Biquad::new(Coefs::highpass(2000.0, 0.7, RATE));
    (0..len)
        .map(|i| {
            let t = i as f32 / RATE;
            let env = (-t / tau).exp() * (t * 2000.0).min(1.0);
            let mut s = 0.0;
            for h in 1..24 {
                let f = hz * h as f32;
                if f > 8000.0 {
                    break;
                }
                s += (t * f * std::f32::consts::TAU).sin() / h as f32 * (-t * h as f32 * 0.8).exp();
            }
            let tick = if i < 96 {
                hp.process(noise.next()) * click
            } else {
                0.0
            };
            (s * 0.5 * env + tick) * level
        })
        .collect()
}

fn rms(v: &[f32]) -> f32 {
    (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32)
        .sqrt()
        .max(1e-9)
}

fn db(x: f32) -> f32 {
    20.0 * x.log10()
}

fn run(amp: &mut Amp, input: &[f32], cab: &[f32]) -> Vec<f32> {
    let dry: Vec<f32> = input.iter().map(|&x| amp.process(x)).collect();
    let mut out = vec![0.0; dry.len()];
    for (i, o) in out.iter_mut().enumerate() {
        let mut acc = 0.0;
        for (k, h) in cab.iter().enumerate().take(i + 1) {
            acc += h * dry[i - k];
        }
        *o = acc;
    }
    out
}

fn bands(v: &[f32]) -> (f32, f32, f32) {
    let mut low = Biquad::new(Coefs::lowpass(220.0, 0.7, RATE));
    let mut mh = Biquad::new(Coefs::highpass(220.0, 0.7, RATE));
    let mut ml = Biquad::new(Coefs::lowpass(1800.0, 0.7, RATE));
    let mut hh = Biquad::new(Coefs::highpass(1800.0, 0.7, RATE));
    let l: Vec<f32> = v.iter().map(|&x| low.process(x)).collect();
    let m: Vec<f32> = v.iter().map(|&x| ml.process(mh.process(x))).collect();
    let h: Vec<f32> = v.iter().map(|&x| hh.process(x)).collect();
    (rms(&l), rms(&m), rms(&h))
}

fn main() {
    let gain: f32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(7.0);
    let cab: Vec<f32> = CabModel::V30
        .render(false, RATE)
        .into_iter()
        .take(1024)
        .collect();
    println!(
        "gain {gain}  (dyn: dB out change for 18 dB in, higher = more touch; blur: tail/head of 16th palm mutes, lower = tighter; click: pick noise vs note, lower = quieter)"
    );
    for model in AmpModel::ALL {
        let knobs = [gain, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 0.0];
        let note = |level: f32| pluck(14400, 82.4, level, 0.6, 0.0, 7);
        let mut a = Amp::new(model, &knobs, RATE);
        let soft = rms(&run(&mut a, &note(0.05), &cab)[2400..]);
        let mut a = Amp::new(model, &knobs, RATE);
        let hard = rms(&run(&mut a, &note(0.4), &cab)[2400..]);
        let slot = (RATE * 60.0 / 180.0 / 4.0) as usize;
        let mut mutes = Vec::new();
        for k in 0..8 {
            mutes.extend(pluck(slot, 82.4, 0.3, 0.05, 0.6, 11 + k));
        }
        let mut a = Amp::new(model, &knobs, RATE);
        let out = run(&mut a, &mutes, &cab);
        let (mut head, mut tail) = (0.0, 0.0);
        for k in 2..8 {
            let s = &out[k * slot..(k + 1) * slot];
            head += rms(&s[..slot / 4]);
            tail += rms(&s[slot * 3 / 4..]);
        }
        let (lo, mi, hi) = bands(&out[slot * 2..]);
        let mut a = Amp::new(model, &knobs, RATE);
        let with = rms(&run(&mut a, &pluck(4800, 82.4, 0.3, 0.6, 0.6, 3), &cab)[..240]);
        let mut a = Amp::new(model, &knobs, RATE);
        let without = rms(&run(&mut a, &pluck(4800, 82.4, 0.3, 0.6, 0.0, 3), &cab)[..240]);
        println!(
            "{:<10} dyn {:>5.1}  blur {:>5.2}  click {:>5.1}  low/mid {:>5.1}  high/mid {:>5.1}",
            model.voicing().name,
            db(hard) - db(soft),
            tail / head,
            db(with) - db(without),
            db(lo) - db(mi),
            db(hi) - db(mi),
        );
    }
    println!("cab V30 on-axis response (dB, re 1 kHz):");
    let ir = CabModel::V30.render(false, RATE);
    let mut line = String::new();
    let at = |hz: f32| {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (n, h) in ir.iter().enumerate() {
            let w = std::f64::consts::TAU * hz as f64 * n as f64 / RATE as f64;
            re += *h as f64 * w.cos();
            im -= *h as f64 * w.sin();
        }
        10.0 * (re * re + im * im).log10() as f32
    };
    let base = at(1000.0);
    for hz in [
        80.0, 120.0, 200.0, 300.0, 450.0, 650.0, 1000.0, 1500.0, 2200.0, 3000.0, 4000.0, 5000.0,
        6500.0, 8000.0,
    ] {
        line += &format!("{hz:.0}:{:+.1} ", at(hz) - base);
    }
    println!("{line}");
    let mut ripple = String::new();
    for k in 0..40 {
        let hz = 150.0 + k as f32 * 30.0;
        ripple += &format!("{:+.0} ", at(hz) - base);
    }
    println!("150-1320 Hz in 30 Hz steps: {ripple}");
}
