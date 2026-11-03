#[cfg(target_arch = "x86_64")]
const FTZ_DAZ: u32 = 0x8040;

pub struct NoDenormals {
    #[cfg(target_arch = "x86_64")]
    saved: u32,
}

impl NoDenormals {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let saved = read();
            write(saved | FTZ_DAZ);
            Self { saved }
        }
        #[cfg(not(target_arch = "x86_64"))]
        Self {}
    }
}

impl Drop for NoDenormals {
    fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        write(self.saved);
    }
}

#[cfg(target_arch = "x86_64")]
fn read() -> u32 {
    let mut csr = 0u32;
    unsafe {
        std::arch::asm!("stmxcsr [{}]", in(reg) &mut csr, options(nostack, preserves_flags));
    }
    csr
}

#[cfg(target_arch = "x86_64")]
fn write(csr: u32) {
    unsafe {
        std::arch::asm!("ldmxcsr [{}]", in(reg) &csr, options(nostack, readonly, preserves_flags));
    }
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use std::hint::black_box;

    use super::*;

    mod when_the_guard_is_active {
        use super::*;

        #[test]
        fn flushes_denormal_results_to_zero() {
            let _guard = NoDenormals::new();
            assert_eq!(black_box(f32::MIN_POSITIVE) * black_box(0.5), 0.0);
        }
    }

    mod when_the_guard_is_dropped {
        use super::*;

        #[test]
        fn restores_the_previous_mode() {
            drop(NoDenormals::new());
            assert!(black_box(f32::MIN_POSITIVE) * black_box(0.5) > 0.0);
        }
    }
}
