//! Timing harness for the performance-envelope experiment (proposal §6.2).
//!
//! Reports both TSC cycles and wall-clock nanoseconds and cross-checks them
//! against the nominal clock. They should agree; when they do not, the number
//! to distrust is the cycle count, because the TSC is a constant-rate counter
//! rather than a core-cycle counter and turbo, frequency scaling and hypervisor
//! interposition all break the correspondence.
//!
//! A benchmark that silently measures the wrong thing is worse than no
//! benchmark, so every result carries the data needed to notice.

use std::hint::black_box;
use std::time::Instant;

/// One measurement.
#[derive(Debug, Clone)]
pub struct Timing {
    pub name: String,
    /// Median TSC ticks per iteration.
    pub ticks_per_iter: f64,
    /// Median nanoseconds per iteration.
    pub ns_per_iter: f64,
    /// Bytes produced per iteration; 0 when the work is not byte-producing.
    pub bytes_per_iter: usize,
    pub iterations: u64,
    pub repeats: usize,
}

impl Timing {
    /// TSC ticks per byte. Only meaningful when `bytes_per_iter > 0`.
    pub fn ticks_per_byte(&self) -> f64 {
        if self.bytes_per_iter == 0 {
            f64::NAN
        } else {
            self.ticks_per_iter / self.bytes_per_iter as f64
        }
    }

    /// Nanoseconds per byte.
    pub fn ns_per_byte(&self) -> f64 {
        if self.bytes_per_iter == 0 {
            f64::NAN
        } else {
            self.ns_per_iter / self.bytes_per_iter as f64
        }
    }

    /// Cycles per byte implied by wall time at `ghz`. Cross-check against
    /// [`Self::ticks_per_byte`]; large disagreement invalidates the TSC number.
    pub fn cycles_per_byte_from_wall(&self, ghz: f64) -> f64 {
        self.ns_per_byte() * ghz
    }

    /// Relative disagreement between the TSC and the wall-derived cycle count.
    ///
    /// Zero when `ghz` is the TSC's true rate. Anything above a few percent
    /// means the assumed clock is wrong and **every cycles/byte figure derived
    /// from it is wrong with it** — the nanosecond figures remain valid.
    pub fn clock_disagreement(&self, ghz: f64) -> f64 {
        let ticks = self.ticks_per_byte();
        let wall = self.cycles_per_byte_from_wall(ghz);
        if ticks == 0.0 || !ticks.is_finite() || !wall.is_finite() {
            return 0.0;
        }
        ((ticks - wall) / ticks).abs()
    }
}

/// Reads the timestamp counter, or returns 0 where unavailable.
#[inline]
fn rdtsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: `_rdtsc` has no preconditions and is available on all
        // x86_64. It reads a counter and has no memory effects.
        unsafe { core::arch::x86_64::_rdtsc() }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        0
    }
}

/// Measures the timestamp counter's frequency in GHz against the wall clock.
///
/// **Never assume a nominal clock.** An earlier version of the report drivers
/// hardcoded 2.667 GHz — correct for the machine they were written on, and
/// ~50% wrong on a modern boosting CPU, which silently corrupted every
/// cycles-per-byte figure while leaving the nanosecond figures correct.
///
/// Note what this does and does not give you. On modern x86 the TSC is a
/// *constant-rate* counter, usually pinned near the base clock, and it does
/// **not** track the core's actual boost frequency. So this returns the TSC
/// rate, which is reproducible and auditable, not the core clock. Cross-machine
/// comparisons should be made on **ns/byte**; cycles/byte is only meaningful
/// against a stated clock.
pub fn calibrate_tsc_ghz() -> f64 {
    // Long enough that timer granularity is irrelevant, short enough not to
    // annoy anyone.
    const WINDOW: std::time::Duration = std::time::Duration::from_millis(50);
    let t0 = rdtsc();
    let w0 = Instant::now();
    while w0.elapsed() < WINDOW {
        std::hint::spin_loop();
    }
    let elapsed = w0.elapsed();
    let ticks = rdtsc().wrapping_sub(t0);
    ticks as f64 / elapsed.as_nanos() as f64
}

/// Times `f`, run `iterations` times per repeat, reporting the median repeat.
///
/// The median rather than the mean, because scheduler preemption produces
/// occasional enormous outliers that would dominate an average. `black_box`
/// wraps the result so the optimiser cannot delete the work being measured.
pub fn measure<F, R>(
    name: impl Into<String>,
    bytes_per_iter: usize,
    iterations: u64,
    repeats: usize,
    mut f: F,
) -> Timing
where
    F: FnMut() -> R,
{
    // Warm up caches, branch predictors and any lazily-initialised state.
    for _ in 0..(iterations / 4).max(1) {
        black_box(f());
    }

    let mut ticks = Vec::with_capacity(repeats);
    let mut nanos = Vec::with_capacity(repeats);

    for _ in 0..repeats {
        let t0 = rdtsc();
        let w0 = Instant::now();
        for _ in 0..iterations {
            black_box(f());
        }
        let elapsed = w0.elapsed();
        let t1 = rdtsc();
        ticks.push((t1.wrapping_sub(t0)) as f64 / iterations as f64);
        nanos.push(elapsed.as_nanos() as f64 / iterations as f64);
    }

    ticks.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
    nanos.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));

    Timing {
        name: name.into(),
        ticks_per_iter: ticks[repeats / 2],
        ns_per_iter: nanos[repeats / 2],
        bytes_per_iter,
        iterations,
        repeats,
    }
}

/// x86 hardware features relevant to the envelope experiment.
///
/// Recorded with every result set. Cycle counts from a machine lacking AES-NI
/// or AVX say nothing about designs built on those instructions, and a results
/// table without this context invites exactly that misreading.
#[derive(Debug, Clone, Default)]
pub struct CpuFeatures {
    pub sse2: bool,
    pub ssse3: bool,
    pub aes: bool,
    pub pclmulqdq: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub gfni: bool,
    pub vaes: bool,
}

impl CpuFeatures {
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            Self {
                sse2: is_x86_feature_detected!("sse2"),
                ssse3: is_x86_feature_detected!("ssse3"),
                aes: is_x86_feature_detected!("aes"),
                pclmulqdq: is_x86_feature_detected!("pclmulqdq"),
                avx: is_x86_feature_detected!("avx"),
                avx2: is_x86_feature_detected!("avx2"),
                avx512f: is_x86_feature_detected!("avx512f"),
                gfni: is_x86_feature_detected!("gfni"),
                vaes: is_x86_feature_detected!("vaes"),
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            Self::default()
        }
    }

    /// Whether the AES-round-based designs (AEGIS, Rocca, Randen) can be
    /// measured meaningfully here at all.
    pub fn can_measure_aes_designs(&self) -> bool {
        self.aes
    }

    /// Whether the instruction-co-design hypothesis H2 can be tested here.
    pub fn can_measure_h2(&self) -> bool {
        self.gfni && self.avx512f
    }

    pub fn summary(&self) -> String {
        let mut on = Vec::new();
        for (name, present) in [
            ("sse2", self.sse2),
            ("ssse3", self.ssse3),
            ("aes", self.aes),
            ("pclmulqdq", self.pclmulqdq),
            ("avx", self.avx),
            ("avx2", self.avx2),
            ("avx512f", self.avx512f),
            ("gfni", self.gfni),
            ("vaes", self.vaes),
        ] {
            if present {
                on.push(name);
            }
        }
        if on.is_empty() {
            "none detected".to_string()
        } else {
            on.join(" ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_reports_the_requested_shape() {
        let t = measure("noop", 64, 1000, 5, || black_box(1u64 + 1));
        assert_eq!(t.bytes_per_iter, 64);
        assert_eq!(t.iterations, 1000);
        assert_eq!(t.repeats, 5);
        assert!(t.ns_per_iter >= 0.0);
    }

    #[test]
    fn per_byte_is_nan_when_no_bytes_are_produced() {
        let t = measure("nobytes", 0, 100, 3, || black_box(0u8));
        assert!(t.ticks_per_byte().is_nan());
        assert!(t.ns_per_byte().is_nan());
    }

    #[test]
    fn more_work_takes_longer() {
        let light = measure("light", 0, 2000, 7, || {
            let mut x = black_box(1u64);
            for _ in 0..4 {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
            }
            x
        });
        let heavy = measure("heavy", 0, 2000, 7, || {
            let mut x = black_box(1u64);
            for _ in 0..64 {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
            }
            x
        });
        // A 16x work difference must survive timer noise, or the harness is
        // not measuring anything.
        assert!(
            heavy.ns_per_iter > light.ns_per_iter * 2.0,
            "harness failed to distinguish 16x more work: light={:.1}ns heavy={:.1}ns",
            light.ns_per_iter,
            heavy.ns_per_iter
        );
    }

    #[test]
    fn feature_detection_reports_something_coherent() {
        let f = CpuFeatures::detect();
        // Any x86_64 CPU has SSE2; it is part of the baseline ISA.
        #[cfg(target_arch = "x86_64")]
        assert!(f.sse2, "x86_64 always has SSE2");
        // H2 needs both, so the helper must not claim more than the parts.
        assert_eq!(f.can_measure_h2(), f.gfni && f.avx512f);
        assert!(!f.summary().is_empty());
    }
}
