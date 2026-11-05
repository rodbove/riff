use std::time::Instant;

use nam_rs::{Model, NamModel};
use riff_dsp::{Chain, Library, preset};

const RATE: f32 = 48000.0;
const SECONDS: usize = 10;

fn guitar() -> Vec<f32> {
    let mut seed = 1u32;
    (0..RATE as usize * SECONDS)
        .map(|i| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let t = i as f32 / RATE;
            let env = (-(t % 0.25) * 12.0).exp();
            let f = if (i / 12000) % 2 == 0 { 82.4 } else { 123.6 };
            let noise = (seed as f32 / u32::MAX as f32 - 0.5) * 0.02;
            ((t * f * std::f32::consts::TAU).sin() * 0.4 + noise) * env
        })
        .collect()
}

fn measure(label: &str, input: &[f32], block: usize, mut run: impl FnMut(&[f32])) {
    let budget = block as f64 / RATE as f64;
    let mut times: Vec<f64> = input
        .chunks(block)
        .map(|b| {
            let t = Instant::now();
            run(b);
            t.elapsed().as_secs_f64() / budget * 100.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let p99 = times[times.len() * 99 / 100];
    let worst = times[times.len() - 1];
    println!("{label:<24} mean {mean:>5.1}%  p99 {p99:>5.1}%  worst {worst:>6.1}%");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: nam <file.nam> [block] [preset]");
        std::process::exit(1);
    };
    let block: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(128);
    let preset_name = args.get(2).map(String::as_str).unwrap_or("Symbolic Rhythm");
    let input = guitar();
    println!("{path}\nblock {block} at {RATE} Hz, percent of the time each block allows");

    let file = NamModel::from_file(path).unwrap_or_else(|e| panic!("{e}"));
    let mut model = Model::from_nam(&file).unwrap_or_else(|e| panic!("{e}"));
    let count = model.as_slimmable().map(|s| s.len()).unwrap_or(1);
    let mut buf = vec![0.0; block];
    for i in 0..count {
        if let Some(s) = model.as_slimmable_mut() {
            s.select(i);
        }
        measure(&format!("model {} of {count}", i + 1), &input, block, |b| {
            buf[..b.len()].copy_from_slice(b);
            model.process_buffer(&mut buf[..b.len()]);
        });
    }

    let mut rig = preset::factory()
        .into_iter()
        .find(|r| r.name == preset_name)
        .unwrap_or_default();
    rig.amp.use_nam = true;
    rig.amp.nam = Some(path.clone());
    rig.fix();
    let mut out = vec![[0.0; 2]; block];
    for (quality, label) in [(0.0, "preset, FULL"), (1.0, "preset, LITE")] {
        rig.amp.nam_values[riff_dsp::nam::QUALITY] = quality;
        let mut chain = Chain::build(&rig, RATE, &mut Library::default());
        chain.warm();
        measure(label, &input, block, |b| {
            chain.process_block(b, &mut out[..b.len()])
        });
    }
}
