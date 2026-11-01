use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

pub const BLOCK: usize = 64;
pub const MAX_IR: usize = 8192;

pub struct Convolver {
    head: Vec<f32>,
    history: [f32; BLOCK],
    hpos: usize,
    parts: Vec<Vec<Complex<f32>>>,
    fdl: Vec<Vec<Complex<f32>>>,
    fdl_pos: usize,
    input: [f32; BLOCK],
    prev: [f32; BLOCK],
    count: usize,
    tail: [f32; BLOCK],
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    time: Vec<f32>,
    acc: Vec<Complex<f32>>,
    scratch_f: Vec<Complex<f32>>,
    scratch_i: Vec<Complex<f32>>,
}

impl Convolver {
    pub fn new(ir: &[f32]) -> Self {
        let ir = &ir[..ir.len().min(MAX_IR)];
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(BLOCK * 2);
        let ifft = planner.plan_fft_inverse(BLOCK * 2);
        let bins = BLOCK + 1;
        let head = ir[..ir.len().min(BLOCK)].to_vec();
        let mut time = vec![0.0f32; BLOCK * 2];
        let mut scratch_f = fft.make_scratch_vec();
        let parts: Vec<Vec<Complex<f32>>> = ir
            .chunks(BLOCK)
            .skip(1)
            .map(|seg| {
                time.fill(0.0);
                time[..seg.len()].copy_from_slice(seg);
                let mut spec = vec![Complex::default(); bins];
                let _ = fft.process_with_scratch(&mut time, &mut spec, &mut scratch_f);
                spec
            })
            .collect();
        let fdl = vec![vec![Complex::default(); bins]; parts.len()];
        let scratch_i = ifft.make_scratch_vec();
        Self {
            head,
            history: [0.0; BLOCK],
            hpos: 0,
            parts,
            fdl,
            fdl_pos: 0,
            input: [0.0; BLOCK],
            prev: [0.0; BLOCK],
            count: 0,
            tail: [0.0; BLOCK],
            fft,
            ifft,
            time,
            acc: vec![Complex::default(); bins],
            scratch_f,
            scratch_i,
        }
    }

    pub fn reset(&mut self) {
        self.history = [0.0; BLOCK];
        self.input = [0.0; BLOCK];
        self.prev = [0.0; BLOCK];
        self.tail = [0.0; BLOCK];
        self.count = 0;
        for s in &mut self.fdl {
            s.fill(Complex::default());
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        self.history[self.hpos] = x;
        let mut y = self.tail[self.count];
        for (i, h) in self.head.iter().enumerate() {
            y += h * self.history[(self.hpos + BLOCK - i) % BLOCK];
        }
        self.hpos = (self.hpos + 1) % BLOCK;
        self.input[self.count] = x;
        self.count += 1;
        if self.count == BLOCK {
            self.block();
        }
        y
    }

    fn block(&mut self) {
        self.count = 0;
        if self.parts.is_empty() {
            return;
        }
        let k = self.parts.len();
        self.time[..BLOCK].copy_from_slice(&self.prev);
        self.time[BLOCK..].copy_from_slice(&self.input);
        let slot = &mut self.fdl[self.fdl_pos];
        let _ = self
            .fft
            .process_with_scratch(&mut self.time, slot, &mut self.scratch_f);
        self.acc.fill(Complex::default());
        for (i, h) in self.parts.iter().enumerate() {
            let s = &self.fdl[(self.fdl_pos + k - i) % k];
            for ((a, x), y) in self.acc.iter_mut().zip(s.iter()).zip(h.iter()) {
                *a += x * y;
            }
        }
        self.acc[0].im = 0.0;
        self.acc[BLOCK].im = 0.0;
        let _ = self
            .ifft
            .process_with_scratch(&mut self.acc, &mut self.time, &mut self.scratch_i);
        let scale = 1.0 / (BLOCK * 2) as f32;
        for (t, v) in self.tail.iter_mut().zip(&self.time[BLOCK..]) {
            *t = v * scale;
        }
        self.prev = self.input;
        self.fdl_pos = (self.fdl_pos + 1) % k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u32) -> Vec<f32> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                (s >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
            })
            .collect()
    }

    fn direct(x: &[f32], h: &[f32]) -> Vec<f32> {
        (0..x.len())
            .map(|n| {
                h.iter()
                    .enumerate()
                    .filter(|(k, _)| *k <= n)
                    .map(|(k, v)| v * x[n - k])
                    .sum()
            })
            .collect()
    }

    mod when_convolving {
        use super::*;

        #[test]
        fn matches_direct_convolution() {
            for len in [1, 40, 64, 65, 300, 1000] {
                let h = noise(len, 7 + len as u32);
                let x = noise(2000, 3);
                let mut c = Convolver::new(&h);
                let out: Vec<f32> = x.iter().map(|v| c.process(*v)).collect();
                let want = direct(&x, &h);
                let err = out
                    .iter()
                    .zip(&want)
                    .fold(0.0f32, |a, (o, w)| a.max((o - w).abs()));
                assert!(err < 1e-3, "len {len} err {err}");
            }
        }

        #[test]
        fn adds_no_latency() {
            let mut c = Convolver::new(&[1.0, 0.0, 0.0]);
            assert_eq!(c.process(0.7), 0.7);
        }
    }
}
