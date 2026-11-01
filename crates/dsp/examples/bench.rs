use std::time::Instant;

use riff_dsp::{Chain, Library, preset};

fn main() {
    let rate = 48000.0;
    let seconds = 5.0;
    let n = (rate * seconds) as usize;
    let input: Vec<f32> = (0..n)
        .map(|i| (i as f32 * 110.0 * std::f32::consts::TAU / rate).sin() * 0.2)
        .collect();
    for rig in preset::factory() {
        let mut chain = Chain::build(&rig, rate, &mut Library::default());
        let start = Instant::now();
        let mut sink = 0.0f32;
        for x in &input {
            sink += chain.process(*x)[0];
        }
        let used = start.elapsed().as_secs_f32() / seconds;
        println!(
            "{:<20} {:>5.1}% of one core {}",
            rig.name,
            used * 100.0,
            if sink.is_nan() { "nan" } else { "" }
        );
    }
}
