//! WHICH CHANGE ACTUALLY FIXED THE CYCLE MEASUREMENT? — validating an
//! instrument before trusting a number it produced.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example instrument_validation
//! ```
//!
//! ## Why this exists
//!
//! Four consecutive cycle measurements were voided by their own canary
//! (19.563 / 24.234 / 12.594 / 11.406 cyc/B against an 8–10 gate). Two changes
//! were then made at once:
//!
//! * **the COUNTER** — `rdtsc` (constant-rate, bills descheduled time to the
//!   measurement) replaced by `QueryThreadCycleTime` (per-thread, excludes it);
//! * **the STATISTIC** — median across repeats replaced by **minimum**, since
//!   contention can only *add* time.
//!
//! The next run read ChaCha20 at **7.818 cyc/B** — *below* the gate, so it was
//! voided too.
//!
//! **Changing two things at once and then interpreting the result is exactly the
//! confound this project keeps catching in other people's work and its own.**
//! This isolates them.
//!
//! ## What is being tested
//!
//! If an instrument excludes contention, its reading must be **load-invariant**
//! — flat as the machine gets busier. So the same ChaCha20 block is measured
//! under increasing artificial load, and **all four combinations** are reported:
//! `{tsc, per-thread} × {median, minimum}`.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **tsc + median moves substantially with load.** This is the original
//!    instrument and the bias that voided four runs. **If it does NOT move, the
//!    artificial load failed to reproduce the real conditions and the whole test
//!    is inconclusive — not a pass.**
//! 2. **per-thread + minimum stays flat**, under 2%.
//! 3. **The attribution is genuinely open.** I do not know whether the counter
//!    or the statistic did the work, and refuse to guess before measuring —
//!    that guess is precisely what this exists to replace.
//!
//! ## What a pass licenses, and what it does not
//!
//! It licenses **re-deriving the canary gate from per-thread readings**. It does
//! **not** license reusing the 8–10 band, which belongs to `tsc + median`. A new
//! band must be built from new readings, and every comparison re-run underneath
//! it.

use statelab_crypto::bench::{measure_dual, pin_to_core, thread_cycles};
use statelab_crypto::generator::chacha20_block;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const ITERS: u64 = 150_000;
const REPEATS: usize = 9;

/// `(thread_min, tsc_min, thread_median, tsc_median)`, all per byte.
fn measure_canary() -> (f64, f64, f64, f64) {
    let key = [0x42u8; 32];
    let nonce = [0x24u8; 12];
    let mut out = [0u8; 64];
    let d = measure_dual("canary", 64, ITERS, REPEATS, || {
        chacha20_block(
            black_box(&key),
            black_box(1),
            black_box(&nonce),
            black_box(&mut out),
        );
    });
    let n = d.bytes_per_iter as f64;
    (
        d.thread_per_iter.unwrap_or(f64::NAN) / n,
        d.tsc_per_iter / n,
        d.thread_median_per_iter.unwrap_or(f64::NAN) / n,
        d.tsc_median_per_iter / n,
    )
}

fn main() {
    let pinned = pin_to_core(0);
    println!("WHICH CHANGE ACTUALLY FIXED THE CYCLE MEASUREMENT?\n");
    println!("  measurement thread pinned to core 0: {pinned}");
    println!("  {ITERS} iters x {REPEATS} repeats per load level\n");

    if thread_cycles().is_none() {
        println!("  Per-thread counter unavailable here. Nothing to validate.");
        return;
    }

    println!("  Two changes were made at once — the COUNTER and the STATISTIC.");
    println!("  Interpreting a result after changing two things is the confound");
    println!("  this project keeps catching elsewhere. This isolates them.\n");
    println!("  PREDICTION 1: tsc+median moves substantially with load. IF IT DOES");
    println!("                NOT, the load failed to reproduce real conditions and");
    println!("                the test is INCONCLUSIVE, not a pass.");
    println!("  PREDICTION 2: per-thread+minimum stays flat, under 2%.");
    println!("  PREDICTION 3: attribution is OPEN — I do not know which change did");
    println!("                the work and will not guess before measuring.\n");

    println!(
        "  {:<14} {:>11} {:>11} {:>11} {:>11}",
        "load", "tsc-median", "tsc-min", "thr-median", "thr-min"
    );

    // (extra_threads, thr_min, tsc_min, thr_med, tsc_med)
    let mut rows: Vec<(usize, f64, f64, f64, f64)> = Vec::new();

    for extra in [0usize, 3, 6] {
        let stop = Arc::new(AtomicBool::new(false));
        let mut handles = Vec::new();
        for _ in 0..extra {
            let s = stop.clone();
            handles.push(std::thread::spawn(move || {
                // Pin the LOAD to the SAME core as the measurement. A first
                // version let it roam across other cores and barely contended,
                // so the test declared itself inconclusive rather than being
                // reinterpreted as a pass.
                pin_to_core(0);
                let mut x = 1u64;
                while !s.load(Ordering::Relaxed) {
                    for _ in 0..10_000 {
                        x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                        black_box(x);
                    }
                }
            }));
        }
        std::thread::sleep(std::time::Duration::from_millis(300));

        let (thr_min, tsc_min, thr_med, tsc_med) = measure_canary();

        stop.store(true, Ordering::Relaxed);
        for h in handles {
            let _ = h.join();
        }

        rows.push((extra, thr_min, tsc_min, thr_med, tsc_med));
        println!(
            "  {:<14} {tsc_med:>11.3} {tsc_min:>11.3} {thr_med:>11.3} {thr_min:>11.3}",
            format!("+{extra} threads")
        );
    }

    let spread = |f: fn(&(usize, f64, f64, f64, f64)) -> f64| -> f64 {
        let lo = rows.iter().map(f).fold(f64::INFINITY, f64::min);
        let hi = rows.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
        100.0 * (hi - lo) / lo
    };
    let s_tsc_med = spread(|r| r.4);
    let s_tsc_min = spread(|r| r.2);
    let s_thr_med = spread(|r| r.3);
    let s_thr_min = spread(|r| r.1);

    println!("\n== Verdict ==");
    println!("  spread across load levels:");
    println!("    tsc + median   (ORIGINAL instrument) : {s_tsc_med:>6.2}%");
    println!("    tsc + minimum  (statistic changed)   : {s_tsc_min:>6.2}%");
    println!("    thread + median (counter changed)    : {s_thr_med:>6.2}%");
    println!("    thread + minimum (both, current)     : {s_thr_min:>6.2}%");
    println!();

    if s_tsc_med <= 10.0 {
        println!("  >>> PREDICTION 1 FAILS. INCONCLUSIVE, NOT A PASS.");
        println!("      Even the original tsc+median moved only {s_tsc_med:.1}% under this");
        println!("      artificial load, so the load did NOT reproduce the conditions");
        println!("      that voided the real runs. The per-thread figures may look");
        println!("      flat ({s_thr_min:.2}%) but that is UNVALIDATED — the test failed to");
        println!("      create the thing it was built to detect.");
        println!("      The honest fallback is the owner's scheduled quiet window.");
        return;
    }

    println!("  >>> PREDICTION 1 HOLDS. The original instrument moved {s_tsc_med:.1}%,");
    println!("      reproducing the bias that voided four runs.");

    let stat_only = s_tsc_med - s_tsc_min;
    let counter_only = s_tsc_med - s_thr_med;
    println!("\n  ATTRIBUTION (points of spread removed):");
    println!("    changing the STATISTIC alone : {stat_only:>6.2}");
    println!("    changing the COUNTER alone   : {counter_only:>6.2}");
    if stat_only > counter_only * 1.5 {
        println!("    >>> The STATISTIC did most of the work. Minimum-across-repeats");
        println!("        is the fix; the per-thread counter is confirmation.");
    } else if counter_only > stat_only * 1.5 {
        println!("    >>> The COUNTER did most of the work, as designed.");
    } else {
        println!("    >>> Both contribute comparably; neither alone is sufficient.");
    }

    if s_thr_min < 2.0 {
        println!("\n  >>> PREDICTION 2 HOLDS. thread+minimum is flat at {s_thr_min:.2}%.");
        println!("      LOAD-INVARIANT. This licenses RE-DERIVING the canary gate");
        println!("      from per-thread readings. It does NOT license reusing the");
        println!("      8-10 band, which belongs to tsc+median. Build the new band");
        println!("      from new readings and re-run comparisons underneath it.");
    } else {
        println!("\n  >>> PREDICTION 2 FAILS. thread+minimum moved {s_thr_min:.2}% with load,");
        println!("      so it does not exclude contention as claimed. The 7.818");
        println!("      reading is not trustworthy and the scheduled quiet window");
        println!("      is the honest answer.");
    }
}
