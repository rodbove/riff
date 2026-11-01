pub struct Line {
    buf: Vec<f32>,
    write: usize,
}

impl Line {
    pub fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len.max(4)],
            write: 0,
        }
    }

    pub fn push(&mut self, x: f32) {
        self.buf[self.write] = x;
        self.write = (self.write + 1) % self.buf.len();
    }

    pub fn read(&self, delay: f32) -> f32 {
        let n = self.buf.len();
        let d = delay.clamp(1.0, (n - 2) as f32);
        let pos = self.write as f32 + n as f32 - d;
        let i = pos.floor() as usize;
        let t = pos - pos.floor();
        let a = self.buf[i % n];
        let b = self.buf[(i + 1) % n];
        a + (b - a) * t
    }
}

pub struct Lfo {
    phase: f32,
    step: f32,
}

impl Lfo {
    pub fn new(phase: f32) -> Self {
        Self { phase, step: 0.0 }
    }

    pub fn tune(&mut self, hz: f32, rate: f32) {
        self.step = hz / rate;
    }

    pub fn tick(&mut self) -> f32 {
        let v = (self.phase * std::f32::consts::TAU).sin();
        self.phase = (self.phase + self.step).fract();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod when_delaying {
        use super::*;

        #[test]
        fn reads_back_after_the_delay() {
            let mut l = Line::new(100);
            for i in 0..50 {
                l.push(i as f32);
            }
            assert_eq!(l.read(1.0), 49.0);
            assert_eq!(l.read(10.0), 40.0);
            assert_eq!(l.read(10.5), 39.5);
        }
    }
}
