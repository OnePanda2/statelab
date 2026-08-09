//! HOW MANY ROUNDS DOES A PRNG ACTUALLY NEED? — quality and cost against round
//! count, on the same instrument.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example round_budget
//! ```
//!
//! ## Why this exists
//!
//! `PHASE_O` §3 established the position: every structural lever this project
//! has searched is worth single-digit percent or nothing, and **the only lever
//! with order-of-magnitude headroom is round count.** ChaCha20 ships 20. The
//! best public cryptanalysis reaches ~7. The gap is security margin.
//!
//! This driver supplies the half of that question the instrument can actually
//! answer, and **only** that half.
//!
//! ## *** WHAT THIS CANNOT DO — READ BEFORE READING THE TABLE ***
//!
//! **Statistical saturation is a LOWER bound on rounds. It is not a security
//! argument and it never becomes one.**
//!
//! ChaCha at 7 rounds is broken by published differential-linear cryptanalysis,
//! and it passes every battery in this crate at 7 rounds comfortably. A design
//! that saturates avalanche at 4 has established that **below 4 it is certainly
//! broken** — nothing whatsoever about whether 4 is safe.
//!
//! So this table can rule round counts OUT. It cannot rule any round count IN.
//! That is item (16) — everything here is a proxy — stated at its sharpest,
//! because this is the measurement most likely to be misread as permission.
//!
//! ## What it does establish
//!
//! Three distinct statistical properties measured at each round count, plus
//! real cost. If they saturate at *different* round counts, the largest is the
//! instrument's floor and the others were insufficient on their own —
//! which is exactly what `PHASE_N` found when BIC killed four candidates that
//! avalanche had passed.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **Avalanche saturates at 4 rounds.** `BASELINE_TABLE.md` already says so.
//!    **POSITIVE CONTROL** — if this does not reproduce, the sweep is wrong.
//! 2. **BIC saturates LATER than avalanche.** In `PHASE_N` BIC caught pairwise
//!    structure that avalanche scored as perfect. If that generalises, BIC is
//!    the binding statistical constraint and the project's minimum round count
//!    has been understated by every previous phase.
//! 3. **GF(2) rank is clean at every round count tested**, including low ones —
//!    ChaCha is not linear at any depth, so rank should not be the constraint.
//! 4. **Cost is linear in rounds**, so the speed column is predictable and the
//!    interesting variation is all in the quality columns.
//!
//! Prediction 2 failing — BIC saturating at 4 alongside avalanche — would mean
//! every statistical measure agrees, and the entire 16-round gap to ChaCha20 is
//! cryptanalytic margin with no statistical component at all. That is the
//! cleaner result and it is recorded in advance as the alternative.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::bench::{calibrate_tsc_ghz, measure};
use statelab_crypto::bic::{bic_matrix, bic_recommended_samples};
use statelab_crypto::linearity::{random_matrix_rank_trials, rank_trials, InputSet};
use statelab_crypto::systems::ChaCha;
use std::hint::black_box;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const RANK_TRIALS: usize = 100;
const COST_BATTERIES: usize = 3;
const BIC_SEEDS: [u64; 3] = [1, 909, 0xBEEF_0001];
const ROUNDS: [usize; 11] = [2, 3, 4, 5, 6, 7, 8, 10, 12, 16, 20];

const CHACHA_CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

#[inline(always)]
fn qr(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(16);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(12);
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(8);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(7);
}

/// The real generator shape — load once, run in registers, store once — with
/// the round count as a parameter. Costs measured here are comparable with
/// `harness_tax.rs`'s 8.444 cyc/B for the full 20, NOT with
/// `BASELINE_TABLE.md`, which measures the instrument (`PHASE_O` §1).
fn block(rounds: usize, key: &[u8; 32], counter: u32, nonce: &[u8; 12], out: &mut [u8; 64]) {
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
    for r in 0..rounds {
        if r % 2 == 0 {
            qr(&mut w, 0, 4, 8, 12);
            qr(&mut w, 1, 5, 9, 13);
            qr(&mut w, 2, 6, 10, 14);
            qr(&mut w, 3, 7, 11, 15);
        } else {
            qr(&mut w, 0, 5, 10, 15);
            qr(&mut w, 1, 6, 11, 12);
            qr(&mut w, 2, 7, 8, 13);
            qr(&mut w, 3, 4, 9, 14);
        }
    }
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&w[i].wrapping_add(state[i]).to_le_bytes());
    }
}

fn main() {
    let ghz = calibrate_tsc_ghz();
    let av_samples = recommended_samples(BITS, TOLERANCE);
    let bic_samples = bic_recommended_samples(BITS, TOLERANCE);

    println!("HOW MANY ROUNDS DOES A PRNG ACTUALLY NEED?\n");
    println!("  TSC calibrated at {ghz:.4} GHz");
    println!("  avalanche  {av_samples} samples, tolerance {TOLERANCE}");
    println!(
        "  BIC        {bic_samples} samples x {} disjoint seeds",
        BIC_SEEDS.len()
    );
    println!("  rank       {RANK_TRIALS} trials, square regime {BITS}x{BITS}");
    println!("  cost       real generator shape, NOT the trait path (PHASE_O §1)\n");
    println!("  *** THIS TABLE CAN RULE ROUND COUNTS OUT. IT CANNOT RULE ANY IN. ***");
    println!("  ChaCha7 is BROKEN by published cryptanalysis and passes every");
    println!("  battery here at 7 rounds comfortably. Statistical saturation is a");
    println!("  LOWER bound and never becomes a security argument.\n");

    // Rank null validation at the width used — permanent fixture since PHASE_G.
    let null = random_matrix_rank_trials(BITS, RANK_TRIALS, 0xC0FF_EE00);
    println!(
        "  rank null validation at n={BITS}: full {}/{}  z {:+.2}\n",
        null.full_rank,
        null.trials,
        null.z_score()
    );

    let chacha = ChaCha;
    let key = [0x42u8; 32];
    let nonce = [0x24u8; 12];
    let mut out = [0u8; 64];

    // ---- Cost, measured FIRST and in rotated order.
    //
    // A first version timed each round count once, inline, in a fixed order and
    // produced a non-monotonic column (5 rounds reading slower than 6). Same
    // defect `scalar_rotation_cost.rs` already fixed and this driver did not
    // inherit: a benchmark run is a seed, and fixed order hands drift to
    // whichever measurement runs last.
    let mut cost: Vec<Vec<f64>> = vec![Vec::new(); ROUNDS.len()];
    for b in 0..COST_BATTERIES {
        for k in 0..ROUNDS.len() {
            let i = (b + k) % ROUNDS.len();
            let r = ROUNDS[i];
            let t = measure("blk", 64, 200_000, 9, || {
                block(
                    black_box(r),
                    black_box(&key),
                    black_box(1),
                    black_box(&nonce),
                    black_box(&mut out),
                );
            });
            cost[i].push(t.ticks_per_byte());
        }
    }
    let cost: Vec<f64> = cost
        .into_iter()
        .map(|mut v| {
            v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
            v[v.len() / 2]
        })
        .collect();

    println!(
        "  {:>6} {:>10} {:>10} {:>12} {:>9} {:>9} {:>8}",
        "rounds", "max_dev", "dead", "BIC max|r|", "floor", "rank z", "cyc/B"
    );

    // Series, not first-crossing flags. A statistic that straddles a hard
    // threshold crosses it repeatedly; "first clean" reads noise as signal.
    let mut av_series: Vec<(usize, f64)> = Vec::new();
    let mut bic_series: Vec<(usize, f64, f64)> = Vec::new();

    for (idx, &r) in ROUNDS.iter().enumerate() {
        let m = avalanche_matrix(&chacha, r, av_samples, 12345);
        let max_dev = m.max_deviation();
        let dead = m.dead_pair_fraction();
        av_series.push((r, max_dev));

        // Multi-seed BIC — item (10). Worst seed is the reading.
        let mut worst = f64::NEG_INFINITY;
        let mut floor = 0.0;
        for (i, &s) in BIC_SEEDS.iter().enumerate() {
            let b = bic_matrix(&chacha, r, bic_samples, s + i as u64 * 7);
            floor = b.noise_floor();
            worst = worst.max(b.max_abs_correlation);
        }
        let bic_pass = worst <= floor;
        bic_series.push((r, worst, floor));

        let rank = rank_trials(&chacha, r, &InputSet::Stride(1), BITS, RANK_TRIALS, 1);
        let z = rank.z_score();

        println!(
            "  {r:>6} {max_dev:>10.4} {dead:>10.4} {:>12} {floor:>9.4} {z:>9.2} {:>8.3}",
            format!("{worst:.4}{}", if bic_pass { " ok" } else { " HI" }),
            cost[idx]
        );
    }

    // ---- Saturation by a STAYS-DOWN criterion, not a first crossing.
    //
    // A first-crossing test is wrong for any statistic that straddles its
    // threshold. BIC does exactly that here: ChaCha reads 0.079-0.090 against a
    // 0.0848 floor at EVERY round from 4 to 20, so it crosses back and forth and
    // "first clean" reports whichever round happened to land low. That is
    // PHASE_H §5's open item (the fair-coin null reads ~25% below every real
    // permutation) surfacing as a threshold artefact, and PHASE_M item (17): the
    // failure mode moves as the regime changes.
    //
    // Saturated at the first R such that the statistic stays within the band for
    // R and EVERY larger round tested.
    const BAND: f64 = 1.15;

    let saturates_at = |ok: &dyn Fn(usize) -> bool| -> Option<usize> {
        (0..ROUNDS.len())
            .find(|&i| ROUNDS[i..].iter().all(|&r| ok(r)))
            .map(|i| ROUNDS[i])
    };

    let av_sat = saturates_at(&|r| {
        av_series
            .iter()
            .find(|(rr, _)| *rr == r)
            .is_some_and(|(_, d)| *d <= TOLERANCE)
    });
    let bic_sat = saturates_at(&|r| {
        bic_series
            .iter()
            .find(|(rr, _, _)| *rr == r)
            .is_some_and(|(_, w, f)| *w <= f * BAND)
    });

    println!(
        "
== Where each measure saturates (stays-down, band {BAND:.2}x floor) =="
    );
    match av_sat {
        Some(r) => println!("  avalanche  saturates at {r} rounds and stays"),
        None => println!("  avalanche  never saturates in the tested range"),
    }
    match bic_sat {
        Some(r) => println!("  BIC        saturates at {r} rounds and stays"),
        None => println!("  BIC        never saturates in the tested range"),
    }

    let tested = bic_series.iter().filter(|(r, _, _)| *r >= 4).count();
    let crossings = bic_series
        .iter()
        .filter(|(r, _, _)| *r >= 4)
        .filter(|(_, w, f)| w > f)
        .count();
    println!("  BIC crosses its RAW floor {crossings} of {tested} times at r>=4 — which is");
    println!("  why the raw threshold cannot locate a saturation point at all.");

    match (av_sat, bic_sat) {
        (Some(a), Some(b)) if b > a => {
            println!(
                "
  >>> PREDICTION 2 HOLDS. BIC binds at {b}, avalanche cleared at {a}."
            );
        }
        (Some(a), Some(b)) if b == a => {
            println!(
                "
  >>> PREDICTION 2 FAILS, in the cleaner direction recorded in"
            );
            println!("      advance. Avalanche and BIC BOTH saturate at {a}. Every");
            println!("      statistical measure this crate has agrees, so the entire gap");
            println!("      from {a} to 20 is CRYPTANALYTIC margin with NO statistical");
            println!("      component whatsoever. The batteries have nothing further to");
            println!("      say about round count above {a}, and saying otherwise would");
            println!("      be reading their noise.");
        }
        _ => println!(
            "
  >>> Mixed or incomplete saturation — read the table directly."
        ),
    }

    println!("\n-- How to use this --");
    println!("   The floor above is NECESSARY, not sufficient. The binding");
    println!("   constraint on a shipped round count is cryptanalysis, and this");
    println!("   crate does none. What this establishes is the point below which");
    println!("   no argument about threat models can rescue a design, and the");
    println!("   real cost of every round above it.");
}
