use realfft::RealFftPlanner;
use riff_dsp::amp::{Amp, AmpModel};
use riff_dsp::fx::Mono;

fn main() {
    let rate = 48000.0f32;
    let n = 1 << 15;
    for tone in [1500.0f32, 3000.0, 5000.0] {
        for model in AmpModel::ALL {
            let mut amp = Amp::new(model, &[10.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 0.0], rate);
            for i in 0..n {
                amp.process((i as f32 * tone * std::f32::consts::TAU / rate).sin() * 0.2);
            }
            let mut buf: Vec<f32> = (n..2 * n)
                .map(|i| {
                    let w = 0.5 - 0.5 * (i as f32 * std::f32::consts::TAU / n as f32).cos();
                    amp.process((i as f32 * tone * std::f32::consts::TAU / rate).sin() * 0.2) * w
                })
                .collect();
            let mut planner = RealFftPlanner::<f32>::new();
            let fft = planner.plan_fft_forward(n);
            let mut spec = fft.make_output_vec();
            fft.process(&mut buf, &mut spec).unwrap();
            let bin = rate / n as f32;
            let (mut harm, mut alias) = (0.0f64, 0.0f64);
            for (k, c) in spec.iter().enumerate() {
                let f = k as f32 * bin;
                if f < 20.0 || f > 20000.0 {
                    continue;
                }
                let p = c.norm_sqr() as f64;
                let h = (f / tone).round();
                if h >= 1.0 && (f - h * tone).abs() < 40.0 {
                    harm += p;
                } else {
                    alias += p;
                }
            }
            println!(
                "{tone:>6} Hz {:<10} alias {:>6.1} dB",
                model.voicing().name,
                10.0 * (alias / harm).log10()
            );
        }
    }
}
