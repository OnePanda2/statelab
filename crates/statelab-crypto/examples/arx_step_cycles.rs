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
//!   it. **A canary outside 8–10 cyc/B condemns the run**, and the code says so
//!   rather than leaving it to a reader.
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

use statelab_crypto::bench::{calibrate_tsc_ghz, measure, noise_floor_pct, rotated_battery};
use statelab_crypto::generator::chacha20_block;
use std::hint::black_box;

const ITERS: u64 = 150_000;
const REPEATS: usize = 9;
const BATTERIES: usize = 5;
const CHACHA_CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];
const ROTS: [u32; 4] = [16, 12, 8, 7];

/// ChaCha's wiring: column partition then diagonal partition.
const COLUMNS: [[usize; 4]; 4] = [[0, 4, 8, 12], [1, 5, 9, 13], [2, 6, 10, 14], [3, 7, 11, 15]];
const DIAGONALS: [[usize; 4]; 4] = [[0, 5, 10, 15], [1, 6, 11, 12], [2, 7, 8, 13], [3, 4, 9, 14]];

/// An `N`-step ARX quarter round, unrolled at compile time exactly as a real
/// implementation would fix it.
#[inline(always)]
fn quarter<const N: usize>(q: &mut [u32; 4]) {
    for i in 0..N {
        let r = ROTS[i % 4];
        if i % 2 == 0 {
            q[0] = q[0].wrapping_add(q[1]);
            q[3] = (q[3] ^ q[0]).rotate_left(r);
        } else {
            q[2] = q[2].wrapping_add(q[3]);
            q[1] = (q[1] ^ q[2]).rotate_left(r);
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
        let groups = if r % 2 == 0 { &COLUMNS } else { &DIAGONALS };
        for g in groups {
            let mut q = [w[g[0]], w[g[1]], w[g[2]], w[g[3]]];
            quarter::<N>(&mut q);
            for (k, &lane) in g.iter().enumerate() {
                w[lane] = q[k];
            }
        }
    }
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&w[i].wrapping_add(state[i]).to_le_bytes());
    }
}

macro_rules! time_block {
    ($n:expr, $rounds:expr) => {{
        let key = [0x42u8; 32];
        let nonce = [0x24u8; 12];
        let mut out = [0u8; 64];
        measure("blk", 64, ITERS, REPEATS, || {
            block::<$n, $rounds>(
                black_box(&key),
                black_box(1),
                black_box(&nonce),
                black_box(&mut out),
            );
        })
        .ticks_per_byte()
    }};
}

fn report(
    title: &str,
    labels: &[&str],
    ops: &[usize],
    cases: &[statelab_crypto::bench::RotatedCase],
) {
    println!("\n== {title} ==");
    println!(
        "  {:<22} {:>9} {:>10} {:>9} {:>10} {:>9}",
        "design", "model ops", "cyc/B", "spread", "vs chacha", "model says"
    );
    let base = cases
        .iter()
        .position(|c| c.label.starts_with("N=4"))
        .map(|i| cases[i].median)
        .expect("control present");
    let base_ops = ops[labels
        .iter()
        .position(|l| l.starts_with("N=4"))
        .expect("control")];
    for (i, c) in cases.iter().enumerate() {
        println!(
            "  {:<22} {:>9} {:>10.3} {:>8.2}% {:>10} {:>9}",
            c.label,
            ops[i],
            c.median,
            c.spread_pct,
            format!("{:+.2}%", 100.0 * (c.median / base - 1.0)),
            format!("{:+.2}%", 100.0 * (ops[i] as f64 / base_ops as f64 - 1.0))
        );
    }
    println!(
        "  worst per-design run-to-run noise: {:.2}%",
        noise_floor_pct(cases)
    );
}

fn main() {
    let ghz = calibrate_tsc_ghz();
    println!("DOES THE 6.25% SURVIVE REAL CYCLES?\n");
    println!("  TSC calibrated at {ghz:.4} GHz");
    println!("  {ITERS} iters x {REPEATS} repeats, {BATTERIES} batteries, ORDER ROTATED");
    println!("  register-resident generator shape, NOT the measurement trait\n");
    println!("  PREDICTION 1: canary in 8-10 cyc/B, else the run is VOID.");
    println!("  PREDICTION 2: N=4 here matches chacha20_block. CONTROL.");
    println!("  PREDICTION 3: the 6.25% does NOT reliably materialise. No");
    println!("                confident direction claimed; spread likely a few");
    println!("                percent and possibly favouring ChaCha.\n");

    // ---------------------------------------------------------------- canary
    let key = [0x42u8; 32];
    let nonce = [0x24u8; 12];
    let mut out = [0u8; 64];
    let canary = measure("canary", 64, ITERS, REPEATS, || {
        chacha20_block(
            black_box(&key),
            black_box(1),
            black_box(&nonce),
            black_box(&mut out),
        );
    })
    .ticks_per_byte();
    println!("== Canary: generator::chacha20_block ==");
    println!("  {canary:.3} cyc/B  (expected 8-10; PHASE_O read 8.323, PHASE_P 8.756)");
    if !(8.0..=10.0).contains(&canary) {
        println!("\n  >>> *** RUN VOID. *** The canary is outside 8-10 cyc/B, which");
        println!("      means this machine is not in the state every other cost");
        println!("      figure in the project was taken in. PHASE_S §2.2: under");
        println!("      memory pressure the RATIO moved, not just the absolutes,");
        println!("      so nothing here can be salvaged by normalising. Quiesce");
        println!("      the machine (shut down WSL) and re-run.");
        return;
    }
    println!("  >>> PREDICTION 1 HOLDS. Machine is in a comparable state.");

    // ------------------------------------------------- at saturation rounds
    let sat_labels = ["N=3 @5", "N=4 @4 (CONTROL)", "N=5 @3"];
    let sat_ops = [5 * 36usize, 4 * 48, 3 * 60];
    let sat = rotated_battery(&sat_labels, BATTERIES, |i| match i {
        0 => time_block!(3, 5),
        1 => time_block!(4, 4),
        _ => time_block!(5, 3),
    });
    report("At diffusion saturation", &sat_labels, &sat_ops, &sat);

    // ------------------------------------------------ at a 5x margin, shipped
    let mar_labels = ["N=3 @25", "N=4 @20 (CONTROL)", "N=5 @15"];
    let mar_ops = [25 * 36usize, 20 * 48, 15 * 60];
    let mar = rotated_battery(&mar_labels, BATTERIES, |i| match i {
        0 => time_block!(3, 25),
        1 => time_block!(4, 20),
        _ => time_block!(5, 15),
    });
    report(
        "At a 5x security margin (ChaCha20-equivalent)",
        &mar_labels,
        &mar_ops,
        &mar,
    );

    // ------------------------------------------------------------- verdict
    let ctrl = mar
        .iter()
        .find(|c| c.label.starts_with("N=4"))
        .expect("ctrl");
    println!("\n== Verdict ==");
    let ctrl_gap = 100.0 * (ctrl.median / canary - 1.0);
    if ctrl_gap.abs() <= 5.0 {
        println!(
            "  >>> PREDICTION 2 HOLDS. N=4 @20 reads {:.3} against the canary's",
            ctrl.median
        );
        println!("      {canary:.3} — {ctrl_gap:+.2}%. The parameterised path IS ChaCha20.");
    } else {
        println!("  >>> PREDICTION 2 FAILS. N=4 @20 is {ctrl_gap:+.2}% from the canary,");
        println!("      so the parameterised path is NOT reproducing ChaCha20 and");
        println!("      every row above is measuring something else. VOID.");
        return;
    }

    let noise = noise_floor_pct(&mar).max(noise_floor_pct(&sat));
    let mut real_wins = Vec::new();
    for c in mar.iter().filter(|c| !c.label.starts_with("N=4")) {
        let gain = 100.0 * (1.0 - c.median / ctrl.median);
        if gain > noise {
            real_wins.push((c.label.clone(), gain));
        }
    }
    println!("  Model predicted -6.25% for both N=3 and N=5. Machine noise floor {noise:.2}%.");
    if real_wins.is_empty() {
        println!("  >>> PREDICTION 3 HOLDS. No step count beats ChaCha by more than");
        println!("      this machine's own run-to-run noise. THE 6.25% IS AN ARTEFACT");
        println!("      OF THE OP MODEL and does not survive contact with the CPU.");
        println!("      The screen result is withdrawn, not promoted.");
    } else {
        println!("  >>> *** PREDICTION 3 FAILS. Measured win exceeding the noise floor: ***");
        for (l, g) in &real_wins {
            println!("      {l}: {g:.2}% faster than ChaCha20-equivalent");
        }
        println!("      STILL NOT A CANDIDATE. Required next: BIC, GF(2) rank,");
        println!("      confirmation on unseen seeds, and CLAASP. And note that");
        println!("      PRIOR_ART_ROTATION_CONSTANTS §0 found a published set of");
        println!("      58,000 better constants that NOTHING ADOPTED — the bar for");
        println!("      displacing an incumbent is not 'measurably faster'.");
    }
}
