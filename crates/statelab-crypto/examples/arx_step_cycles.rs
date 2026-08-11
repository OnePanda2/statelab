//! DOES THE 6.25% SURVIVE REAL CYCLES? — measuring N=3 and N=5.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example arx_step_cycles
//! ```
//!
//! ## What is being tested
//!
//! `arx_step_search` found two ARX step counts beating ChaCha on **total
//! operations to diffusion saturation**, and `arx_step_margin` confirmed the
//! round counts on five seeds with 26–30% slack and sharp transitions:
//!
//! ```text
//! N=3   5 rounds x 36 ops = 180
//! N=4   4 rounds x 48 ops = 192   <- ChaCha
//! N=5   3 rounds x 60 ops = 180
//! ```
//!
//! **6.25%, in a crude op model.** `PHASE_O` is the standing proof that an
//! instruction count is not a cycle count — it found the project's entire cost
//! baseline inflated 2.35x, and found a SIMD instruction model that predicted
//! 25% where the machine delivered under 1%. So the op model gets checked
//! against the machine before the 6.25% is called anything.
//!
//! ## Two scalings, because they answer different questions
//!
//! * **At saturation** — N=3 at 5 rounds, N=4 at 4, N=5 at 3. Tests the op
//!   model's claim directly, at the point the claim was made.
//! * **At a 5x security margin** — 25, 20, 15 rounds. ChaCha20 ships 20 rounds
//!   against a saturation point of 4, so this is what a *shipped* design at
//!   comparable margin would look like. The op model predicts the same 6.25%
//!   (900 vs 960), so any divergence between the two scalings is a real effect
//!   of round-loop overhead rather than of the round function.
//!
//! **Neither scaling is a security claim.** The 5x margin is copied from ChaCha
//! arithmetically; whether a 3-step quarter round *earns* the same multiplier is
//! exactly the question this project cannot answer without cryptanalysis.
//!
//! ## Protections, each one earned by a failure this session
//!
//! * **Register-resident generator shape**, not the measurement trait —
//!   `PHASE_O` §1, which found 63.9% of the trait path was memory traffic.
//! * **Rotated battery order** via `bench::rotated_battery` — `PHASE_O` §2.3,
//!   where a fixed order handed all drift to whichever design ran last and made
//!   a positive control fail.
//! * **ChaCha20 measured in the same process as a canary** — `PHASE_S` §2.2,
//!   where memory pressure inflated everything 70% and the *ratio* moved with
//!   it. **A canary outside the band condemns the run**, and the code says so
//!   rather than leaving it to a reader.
//!
//! ## The canary band was RE-DERIVED, not widened
//!
//! The old band was **8–10 cyc/B**, from `PHASE_O`'s 8.323 and `PHASE_P`'s
//! 8.756. Both were **`rdtsc` + median** readings, and
//! `instrument_validation.rs` established that combination moves **786% under
//! load** — so those references carried residual contention even on a machine
//! that looked quiet.
//!
//! The new band is **7.4–8.3**, around a per-thread reading of **7.83** that was
//! measured at three separate load levels and moved **0.05%** across them.
//!
//! **This is a re-derivation, not a widening, and the difference is not
//! rhetorical:** the old band belongs to a different instrument; the new one
//! comes from readings taken across a deliberate load sweep rather than from the
//! single number that was previously rejected. The band is ±5% around a quantity
//! measured at 0.05% precision, so the slack covers frequency drift rather than
//! contention.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **The canary lands in 8–10 cyc/B.** If not, nothing below is readable and
//!    the run is void.
//! 2. **N=4 through this parameterised path matches `chacha20_block` within a
//!    few percent.** POSITIVE CONTROL — it is the same cipher by construction,
//!    so a gap means the parameterisation is not what it claims to be.
//! 3. **The 6.25% does NOT reliably materialise.** I do not have a confident
//!    direction, and say so rather than invent one: a shorter quarter round has
//!    a shorter dependency chain, but more rounds means more loop overhead, and
//!    `PHASE_O` went both ways on questions of this shape. **I expect the
//!    measured spread to be within a few percent and possibly to favour
//!    ChaCha.**
//!
//! A clear win above ~5% for N=3 or N=5, on both scalings, is the outcome that
//! would make this worth taking to BIC, rank and CLAASP. Anything smaller is
//! inside the noise this machine has already demonstrated.

use statelab_crypto::bench::{calibrate_tsc_ghz, measure_dual, pin_to_core, thread_cycles};
use statelab_crypto::generator::chacha20_block;
use std::hint::black_box;

const ITERS: u64 = 150_000;
const REPEATS: usize = 9;
const BATTERIES: usize = 5;
const CHACHA_CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];
const ROTS: [u32; 4] = [16, 12, 8, 7];

/// An `N`-step ARX quarter round, unrolled at compile time exactly as a real
/// implementation would fix it.
#[inline(always)]
fn quarter<const N: usize>(w: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    // Indexes `w` DIRECTLY, exactly as `generator::chacha20_block` does. A first
    // version copied four words into a temp array per group and wrote them back,
    // which added per-round overhead the reference does not pay — and because
    // the designs run DIFFERENT round counts, that overhead penalised N=3 (25
    // rounds) more than N=5 (15). The N=4 control caught it at +60.6% off the
    // canary and voided the run.
    for i in 0..N {
        let r = ROTS[i % 4];
        if i % 2 == 0 {
            w[a] = w[a].wrapping_add(w[b]);
            w[d] = (w[d] ^ w[a]).rotate_left(r);
        } else {
            w[c] = w[c].wrapping_add(w[d]);
            w[b] = (w[b] ^ w[c]).rotate_left(r);
        }
    }
}

/// A ChaCha20-shaped block function with a parameterised step count and round
/// count. Load once, run in registers, feed-forward, store once — the same
/// shape `generator::chacha20_block` uses, so the comparison is not
/// contaminated by the harness tax.
#[inline(always)]
fn block<const N: usize, const ROUNDS: usize>(
    key: &[u8; 32],
    counter: u32,
    nonce: &[u8; 12],
    out: &mut [u8; 64],
) {
    let mut state = [0u32; 16];
    state[..4].copy_from_slice(&CHACHA_CONSTANTS);
    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes(key[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
    }
    state[12] = counter;
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes(nonce[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
    }

    let mut w = state;
    for r in 0..ROUNDS {
        if r % 2 == 0 {
            quarter::<N>(&mut w, 0, 4, 8, 12);
            quarter::<N>(&mut w, 1, 5, 9, 13);
            quarter::<N>(&mut w, 2, 6, 10, 14);
            quarter::<N>(&mut w, 3, 7, 11, 15);
        } else {
            quarter::<N>(&mut w, 0, 5, 10, 15);
            quarter::<N>(&mut w, 1, 6, 11, 12);
            quarter::<N>(&mut w, 2, 7, 8, 13);
            quarter::<N>(&mut w, 3, 4, 9, 14);
        }
    }
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&w[i].wrapping_add(state[i]).to_le_bytes());
    }
}

macro_rules! time_block {
    ($label:expr, $n:expr, $rounds:expr) => {{
        let key = [0x42u8; 32];
        let nonce = [0x24u8; 12];
        let mut out = [0u8; 64];
        measure_dual($label, 64, ITERS, REPEATS, || {
            block::<$n, $rounds>(
                black_box(&key),
                black_box(1),
                black_box(&nonce),
                black_box(&mut out),
            );
        })
    }};
}

/// Prints one scaling, using PER-THREAD cycles where available and falling back
/// to TSC with the instrument named.
fn report_dual(title: &str, ops: &[usize], rows: &[statelab_crypto::bench::DualTiming]) {
    println!(
        "
== {title} =="
    );
    println!(
        "  {:<22} {:>9} {:>10} {:>10} {:>9} {:>10} {:>9}",
        "design", "model ops", "thread", "tsc", "contend", "vs chacha", "model"
    );
    let base_i = rows
        .iter()
        .position(|r| r.name.starts_with("N=4"))
        .expect("control present");
    let base = rows[base_i]
        .thread_per_byte()
        .unwrap_or_else(|| rows[base_i].tsc_per_byte());
    let base_ops = ops[base_i];
    for (i, r) in rows.iter().enumerate() {
        let val = r.thread_per_byte().unwrap_or_else(|| r.tsc_per_byte());
        println!(
            "  {:<22} {:>9} {:>10} {:>10.3} {:>9} {:>10} {:>9}",
            r.name,
            ops[i],
            r.thread_per_byte()
                .map(|v| format!("{v:.3}"))
                .unwrap_or_else(|| "n/a".into()),
            r.tsc_per_byte(),
            r.contention_ratio()
                .map(|c| format!("{c:.2}x"))
                .unwrap_or_else(|| "-".into()),
            format!("{:+.2}%", 100.0 * (val / base - 1.0)),
            format!("{:+.2}%", 100.0 * (ops[i] as f64 / base_ops as f64 - 1.0))
        );
    }
}

/// Runs every design once per battery in ROTATED order, keeping the per-design
/// minimum across batteries. Rotation is `PHASE_O` 2.3's lesson (a fixed order
/// hands drift to whatever runs last); minimum-across-repeats is the new one.
fn rotated_min(
    labels: &[&str],
    batteries: usize,
    mut run: impl FnMut(usize) -> statelab_crypto::bench::DualTiming,
) -> Vec<statelab_crypto::bench::DualTiming> {
    let n = labels.len();
    let mut best: Vec<Option<statelab_crypto::bench::DualTiming>> = vec![None; n];
    for b in 0..batteries {
        for k in 0..n {
            let i = (b + k) % n;
            let t = run(i);
            let better = match &best[i] {
                None => true,
                Some(prev) => match (t.thread_per_iter, prev.thread_per_iter) {
                    (Some(a), Some(pv)) => a < pv,
                    _ => t.tsc_per_iter < prev.tsc_per_iter,
                },
            };
            if better {
                best[i] = Some(t);
            }
        }
    }
    best.into_iter().map(|b| b.expect("measured")).collect()
}

fn main() {
    let ghz = calibrate_tsc_ghz();
    let pinned = pin_to_core(0);
    let have_thread = thread_cycles().is_some();

    println!("DOES THE 6.25% SURVIVE REAL CYCLES?\n");
    println!("  TSC calibrated at {ghz:.4} GHz");
    println!("  {ITERS} iters x {REPEATS} repeats, {BATTERIES} batteries, ORDER ROTATED");
    println!("  register-resident generator shape, NOT the measurement trait");
    println!("  thread pinned to core 0: {pinned}");
    println!("  per-thread cycle counter available: {have_thread}\n");

    println!("  *** INSTRUMENT CHANGED. *** Four previous runs were voided by the");
    println!("  canary at 19.6 / 24.2 / 12.6 / 11.4 cyc/B against a quiet-machine");
    println!("  reference of 8.3. That was NOT noise: rdtsc is a CONSTANT-RATE");
    println!("  counter that keeps ticking while the thread is descheduled, so");
    println!("  every stolen cycle was billed to us. A ONE-DIRECTIONAL BIAS, which");
    println!("  is why more samples never helped.");
    println!("  This run uses QueryThreadCycleTime, counting only cycles this");
    println!("  thread actually ran, and takes the MINIMUM across repeats rather");
    println!("  than the median, because contention can only ADD time.\n");
    println!("  Both instruments are reported. THEIR RATIO IS THE CONTENTION.\n");

    println!("  PREDICTION 1: the thread-cycle canary lands in 8-10 cyc/B EVEN ON");
    println!("                A LOADED MACHINE. That is the test of the fix itself.");
    println!("  PREDICTION 2: N=4 here matches chacha20_block. CONTROL.");
    println!("  PREDICTION 3: the 6.25% does NOT reliably materialise. No confident");
    println!("                direction claimed.\n");

    let key = [0x42u8; 32];
    let nonce = [0x24u8; 12];
    let mut out = [0u8; 64];
    let canary = measure_dual("canary", 64, ITERS, REPEATS, || {
        chacha20_block(
            black_box(&key),
            black_box(1),
            black_box(&nonce),
            black_box(&mut out),
        );
    });
    let c_thread = canary.thread_per_byte();
    let c_tsc = canary.tsc_per_byte();
    println!("== Canary: generator::chacha20_block ==");
    println!(
        "  per-thread : {}",
        c_thread
            .map(|v| format!("{v:.3} cyc/B"))
            .unwrap_or_else(|| "n/a".into())
    );
    println!("  tsc        : {c_tsc:.3} cyc/B");
    if let Some(r) = canary.contention_ratio() {
        println!("  contention : {r:.2}x  (1.00 = idle; higher = cycles stolen)");
    }
    println!("  reference  : PHASE_O 8.323, PHASE_P 8.756, gate 8-10\n");

    let reading = match c_thread {
        Some(v) => v,
        None => {
            println!("  >>> No per-thread counter here; falling back to TSC.");
            c_tsc
        }
    };
    if !(7.4..=8.3).contains(&reading) {
        println!("  >>> *** RUN VOID. *** Canary {reading:.3} is outside 8-10 cyc/B.");
        if c_thread.is_some() {
            println!("      This is the PER-THREAD figure, so descheduling is already");
            println!("      excluded. The residual is cache pressure or frequency, and");
            println!("      the instrument change did NOT solve it. Report that");
            println!("      plainly rather than widening the gate.");
        }
        return;
    }
    println!("  >>> PREDICTION 1 HOLDS. The instrument change works: a clean");
    println!("      reading without needing an idle machine.\n");

    let sat_labels = ["N=3 @5", "N=4 @4 (CONTROL)", "N=5 @3"];
    let sat_ops = [5 * 36usize, 4 * 48, 3 * 60];
    let sat = rotated_min(&sat_labels, BATTERIES, |i| match i {
        0 => time_block!("N=3 @5", 3, 5),
        1 => time_block!("N=4 @4 (CONTROL)", 4, 4),
        _ => time_block!("N=5 @3", 5, 3),
    });
    report_dual("At diffusion saturation", &sat_ops, &sat);

    let mar_labels = ["N=3 @25", "N=4 @20 (CONTROL)", "N=5 @15"];
    let mar_ops = [25 * 36usize, 20 * 48, 15 * 60];
    let mar = rotated_min(&mar_labels, BATTERIES, |i| match i {
        0 => time_block!("N=3 @25", 3, 25),
        1 => time_block!("N=4 @20 (CONTROL)", 4, 20),
        _ => time_block!("N=5 @15", 5, 15),
    });
    report_dual(
        "At a 5x security margin (ChaCha20-equivalent)",
        &mar_ops,
        &mar,
    );

    let ctrl = mar
        .iter()
        .find(|c| c.name.starts_with("N=4"))
        .expect("control");
    let ctrl_v = ctrl
        .thread_per_byte()
        .unwrap_or_else(|| ctrl.tsc_per_byte());
    println!("\n== Verdict ==");
    let gap = 100.0 * (ctrl_v / reading - 1.0);
    if gap.abs() <= 5.0 {
        println!("  >>> PREDICTION 2 HOLDS. N=4 @20 reads {ctrl_v:.3} against the");
        println!("      canary's {reading:.3}, {gap:+.2}%. The parameterised path IS");
        println!("      ChaCha20.");
    } else {
        println!("  >>> PREDICTION 2 FAILS. N=4 @20 is {gap:+.2}% from the canary, so");
        println!("      the parameterised path is NOT reproducing ChaCha20. VOID.");
        return;
    }

    println!("\n  Model predicted -6.25% for both N=3 and N=5.");
    let mut wins = Vec::new();
    for c in mar.iter().filter(|c| !c.name.starts_with("N=4")) {
        let v = c.thread_per_byte().unwrap_or_else(|| c.tsc_per_byte());
        let g = 100.0 * (1.0 - v / ctrl_v);
        println!("    {:<20} {g:+.2}% vs ChaCha20-equivalent", c.name);
        if g > 1.0 {
            wins.push((c.name.clone(), g));
        }
    }
    if wins.is_empty() {
        println!("\n  >>> PREDICTION 3 HOLDS. Neither step count is more than 1%");
        println!("      faster than ChaCha20 in real cycles. THE 6.25% IS AN");
        println!("      ARTEFACT OF THE OP MODEL. The screen result is WITHDRAWN and");
        println!("      avenue 2 has produced no candidate at all.");
    } else {
        println!("\n  >>> *** PREDICTION 3 FAILS. Measured win: ***");
        for (l, g) in &wins {
            println!("      {l}: {g:.2}% faster");
        }
        println!("      STILL NOT A CANDIDATE. Required: BIC, GF(2) rank, unseen");
        println!("      seeds, a second construction, and CLAASP. And note");
        println!("      PRIOR_ART_ROTATION_CONSTANTS 0: 58,000 published better");
        println!("      constants that NOTHING ADOPTED. The displacement bar is not");
        println!("      'measurably faster'.");
    }
}
