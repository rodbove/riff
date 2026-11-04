use std::path::Path;

use riff_dsp::{Library, Rig, chain, preset, wav};

fn usage() -> ! {
    eprintln!("usage:");
    eprintln!("  render list");
    eprintln!("  render dump <preset name>");
    eprintln!("  render render <rig.json> <di.wav> <out.wav> [seconds]");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("list") => {
            for r in preset::factory() {
                println!("{}", r.name);
            }
        }
        Some("dump") if args.len() == 2 => {
            let Some(r) = preset::factory().into_iter().find(|r| r.name == args[1]) else {
                eprintln!("no factory preset named {}", args[1]);
                std::process::exit(1)
            };
            println!("{}", serde_json::to_string_pretty(&r).unwrap());
        }
        Some("render") if args.len() >= 4 => {
            let text = std::fs::read_to_string(&args[1]).unwrap_or_else(|e| {
                eprintln!("{}: {e}", args[1]);
                std::process::exit(1)
            });
            let rig: Rig = serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("{}: {e}", args[1]);
                std::process::exit(1)
            });
            let di = wav::read(Path::new(&args[2])).unwrap_or_else(|e| {
                eprintln!("{}: {e}", args[2]);
                std::process::exit(1)
            });
            let mono = di.mono();
            let n = args
                .get(4)
                .and_then(|s| s.parse::<f32>().ok())
                .map(|s| ((s * di.rate as f32) as usize).min(mono.len()))
                .unwrap_or(mono.len());
            let out = chain::render(&rig, di.rate as f32, &mono[..n], &mut Library::default());
            let flat: Vec<f32> = out.iter().flatten().copied().collect();
            if let Err(e) = wav::write(Path::new(&args[3]), di.rate, 2, &flat) {
                eprintln!("{}: {e}", args[3]);
                std::process::exit(1)
            }
        }
        _ => usage(),
    }
}
