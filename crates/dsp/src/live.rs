use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::chain::{Chain, Target};

const COMMANDS: usize = 1024;
const TRASH: usize = 16;
const TAP_SECONDS: usize = 1;
const RECORD_SECONDS: usize = 10;

pub enum Command {
    Swap(Box<Chain>),
    Set {
        target: Target,
        param: usize,
        value: f32,
    },
    Enable {
        target: Target,
        on: bool,
    },
    Mute(bool),
    Record(bool),
    Play(Option<Track>),
}

pub type Track = Arc<Vec<[f32; 2]>>;

#[derive(Default)]
pub struct Meters {
    input: AtomicU32,
    output: AtomicU32,
    gate: AtomicBool,
    lost: AtomicU32,
    played: AtomicUsize,
}

impl Meters {
    pub fn take_lost(&self) -> u32 {
        self.lost.swap(0, Ordering::Relaxed)
    }

    pub fn played(&self) -> usize {
        self.played.load(Ordering::Relaxed)
    }

    pub fn read(&self) -> (f32, f32, bool) {
        let o = Ordering::Relaxed;
        (
            f32::from_bits(self.input.swap(0, o)),
            f32::from_bits(self.output.swap(0, o)),
            self.gate.load(o),
        )
    }
}

pub struct Runner {
    chain: Box<Chain>,
    spare: Option<Box<Chain>>,
    commands: Consumer<Command>,
    trash: Producer<Box<Chain>>,
    tap: Producer<f32>,
    meters: Arc<Meters>,
    mute: bool,
    scratch: Vec<f32>,
    recording: bool,
    dry: Producer<f32>,
    wet: Producer<[f32; 2]>,
    playing: Option<Track>,
    position: usize,
    finished: Producer<Track>,
}

pub struct Remote {
    pub commands: Producer<Command>,
    pub trash: Consumer<Box<Chain>>,
    pub tap: Consumer<f32>,
    pub meters: Arc<Meters>,
    pub dry: Consumer<f32>,
    pub wet: Consumer<[f32; 2]>,
    pub finished: Consumer<Track>,
}

impl Remote {
    pub fn send(&mut self, command: Command) {
        let _ = self.commands.push(command);
    }

    pub fn collect(&mut self) {
        while self.trash.pop().is_ok() {}
        while self.finished.pop().is_ok() {}
    }
}

pub fn live(chain: Chain) -> (Runner, Remote) {
    let rate = chain.rate() as usize;
    let (cp, cc) = RingBuffer::new(COMMANDS);
    let (tp, tc) = RingBuffer::new(TRASH);
    let (ap, ac) = RingBuffer::new(rate * TAP_SECONDS);
    let meters = Arc::new(Meters::default());
    let (dp, dc) = RingBuffer::new(rate * RECORD_SECONDS);
    let (wp, wc) = RingBuffer::new(rate * RECORD_SECONDS);
    let (fp, fc) = RingBuffer::new(TRASH);
    (
        Runner {
            chain: Box::new(chain),
            spare: None,
            commands: cc,
            trash: tp,
            tap: ap,
            meters: meters.clone(),
            mute: false,
            scratch: vec![0.0; crate::chain::BLOCK],
            recording: false,
            dry: dp,
            wet: wp,
            playing: None,
            position: 0,
            finished: fp,
        },
        Remote {
            commands: cp,
            trash: tc,
            tap: ac,
            meters,
            dry: dc,
            wet: wc,
            finished: fc,
        },
    )
}

impl Runner {
    fn discard(&mut self, old: Box<Chain>) {
        if let Err(rtrb::PushError::Full(old)) = self.trash.push(old) {
            if self.spare.is_none() {
                self.spare = Some(old);
            } else {
                std::mem::forget(old);
            }
        }
    }

    pub fn apply(&mut self, target: Target, param: usize, value: f32) {
        self.chain.set(target, param, value);
    }

    pub fn process(&mut self, input: &[f32], out: &mut [[f32; 2]]) {
        let _ftz = crate::denormal::NoDenormals::new();
        if let Some(old) = self.spare.take() {
            self.discard(old);
        }
        while let Ok(command) = self.commands.pop() {
            match command {
                Command::Swap(mut chain) => {
                    std::mem::swap(&mut self.chain, &mut chain);
                    self.discard(chain);
                }
                Command::Set {
                    target,
                    param,
                    value,
                } => self.chain.set(target, param, value),
                Command::Enable { target, on } => self.chain.enable(target, on),
                Command::Mute(m) => self.mute = m,
                Command::Record(on) => self.recording = on,
                Command::Play(track) => {
                    if let Some(old) = std::mem::replace(&mut self.playing, track) {
                        let _ = self.finished.push(old);
                    }
                    self.position = 0;
                    self.meters.played.store(0, Ordering::Relaxed);
                }
            }
        }
        let mut peak_in = 0.0f32;
        let mut peak_out = 0.0f32;
        for (input, out) in input
            .chunks(crate::chain::BLOCK)
            .zip(out.chunks_mut(crate::chain::BLOCK))
        {
            let n = input.len();
            for (s, x) in self.scratch[..n].iter_mut().zip(input) {
                let x = x * self.chain.input_gain();
                peak_in = peak_in.max(x.abs());
                let _ = self.tap.push(x);
                *s = x;
            }
            self.chain.process_block(&self.scratch[..n], &mut out[..n]);
            if self.recording {
                let mut lost = 0;
                for (x, y) in input.iter().zip(out.iter()) {
                    if self.dry.push(*x).is_err() | self.wet.push(*y).is_err() {
                        lost += 1;
                    }
                }
                if lost > 0 {
                    self.meters.lost.fetch_add(lost, Ordering::Relaxed);
                }
            }
            if let Some(track) = &self.playing {
                let from = self.position.min(track.len());
                let to = (self.position + n).min(track.len());
                for (o, p) in out.iter_mut().zip(&track[from..to]) {
                    o[0] += p[0];
                    o[1] += p[1];
                }
                self.position += n;
                self.meters.played.store(self.position, Ordering::Relaxed);
            }
            for o in out.iter_mut() {
                if self.mute {
                    *o = [0.0; 2];
                }
                peak_out = peak_out.max(o[0].abs()).max(o[1].abs());
            }
        }
        let o = Ordering::Relaxed;
        self.meters.input.fetch_max(peak_in.to_bits(), o);
        self.meters.output.fetch_max(peak_out.to_bits(), o);
        self.meters.gate.store(self.chain.gate_open(), o);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::Kind;
    use crate::library::Library;
    use crate::rig::{Rig, Slot};
    use assert_no_alloc::assert_no_alloc;

    const RATE: f32 = 48000.0;

    fn rig() -> Rig {
        let mut rig = Rig::default();
        rig.pre.push(Slot::new(Kind::Boost));
        rig
    }

    mod when_running_live {
        use super::*;

        mod with_a_track_playing {
            use super::*;

            #[test]
            fn mixes_it_into_the_output_without_allocating() {
                let silent = Rig {
                    output: -60.0,
                    ..Rig::default()
                };
                let (mut runner, mut remote) =
                    live(Chain::build(&silent, RATE, &mut Library::default()));
                let track: Track = Arc::new(vec![[0.5, -0.5]; 600]);
                remote.send(Command::Play(Some(track.clone())));
                let input = vec![0.0f32; 256];
                let mut out = vec![[0.0f32; 2]; 256];
                assert_no_alloc(|| runner.process(&input, &mut out));
                assert!((out[10][0] - 0.5).abs() < 0.01);
                assert!((out[10][1] + 0.5).abs() < 0.01);
                assert_eq!(remote.meters.played(), 256);
                remote.send(Command::Play(None));
                assert_no_alloc(|| runner.process(&input, &mut out));
                assert!(out[10][0].abs() < 0.01);
                assert_eq!(remote.finished.pop().ok().map(|t| t.len()), Some(600));
            }
        }

        #[test]
        fn processes_and_swaps_without_allocating() {
            let (mut runner, mut remote) =
                live(Chain::build(&rig(), RATE, &mut Library::default()));
            let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
            let mut out = vec![[0.0f32; 2]; 256];
            remote.send(Command::Swap(Box::new(Chain::build(
                &rig(),
                RATE,
                &mut Library::default(),
            ))));
            remote.send(Command::Set {
                target: Target::Pre(0),
                param: 0,
                value: 3.0,
            });
            assert_no_alloc(|| {
                for _ in 0..20 {
                    runner.process(&input, &mut out);
                }
            });
            remote.collect();
            let (peak_in, peak_out, _) = remote.meters.read();
            assert!(peak_in > 0.25);
            assert!(peak_out > 0.0);
        }

        #[test]
        fn runs_a_nam_capture_without_allocating() {
            let dir = std::env::temp_dir().join(format!("riff-live-nam-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("tiny.nam");
            std::fs::write(&path, crate::nam::TINY_LSTM).unwrap();
            let mut rig = rig();
            rig.amp.use_nam = true;
            rig.amp.nam = Some(path.to_string_lossy().to_string());
            let chain = Chain::build(&rig, RATE, &mut Library::default());
            assert!(chain.warnings().is_empty(), "{:?}", chain.warnings());
            let (mut runner, _remote) = live(chain);
            let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
            let mut out = vec![[0.0f32; 2]; 256];
            assert_no_alloc(|| {
                for _ in 0..20 {
                    runner.process(&input, &mut out);
                }
            });
            assert!(out.iter().all(|f| f[0].is_finite()));
        }

        #[test]
        fn mutes_the_output_for_the_tuner() {
            let (mut runner, mut remote) =
                live(Chain::build(&rig(), RATE, &mut Library::default()));
            remote.send(Command::Mute(true));
            let input = vec![0.5f32; 64];
            let mut out = vec![[1.0f32; 2]; 64];
            runner.process(&input, &mut out);
            assert!(out.iter().all(|f| *f == [0.0, 0.0]));
            assert_eq!(remote.tap.slots(), 64);
        }
    }

    mod when_recording {
        use super::*;

        #[test]
        fn captures_the_raw_input_and_the_output_without_allocating() {
            let (mut runner, mut remote) =
                live(Chain::build(&rig(), RATE, &mut Library::default()));
            let input: Vec<f32> = (0..300).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
            let mut out = vec![[0.0f32; 2]; 300];
            remote.send(Command::Record(true));
            assert_no_alloc(|| runner.process(&input, &mut out));
            let dry: Vec<f32> = std::iter::from_fn(|| remote.dry.pop().ok()).collect();
            let wet: Vec<[f32; 2]> = std::iter::from_fn(|| remote.wet.pop().ok()).collect();
            assert_eq!(dry, input);
            assert_eq!(wet, out);
            assert_eq!(remote.meters.take_lost(), 0);
        }
    }

    mod when_not_recording {
        use super::*;

        #[test]
        fn captures_nothing() {
            let (mut runner, mut remote) =
                live(Chain::build(&rig(), RATE, &mut Library::default()));
            let input = vec![0.1f32; 64];
            let mut out = vec![[0.0f32; 2]; 64];
            runner.process(&input, &mut out);
            assert!(remote.dry.pop().is_err() && remote.wet.pop().is_err());
        }
    }
}
