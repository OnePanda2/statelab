//! GF(2) RANK and BIC on the four confirmed quarter-round candidates.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example qr_linearity_bic
//! ```
//!
//! ## What these two catch that avalanche does not
//!
//! `PHASE_N` screened on avalanche and §8's dose-response probed input entropy.
//! Both are **flip-probability** measurements: they see whether output bits move
//! with probability 1/2. Neither sees *structure* in how they move.
//!
//! * **GF(2) rank** (`PHASE_F`) sees **linearity**. `xoshiro256++` passes
//!   BigCrush while every cell of its avalanche matrix is 0.0 or 1.0 — a
//!   GF(2)-linear map hiding behind an output scrambler. A quarter round could
//!   be avalanche-perfect and algebraically thin in exactly that way.
//! * **BIC** (`PHASE_H`) sees **pairwise dependence**. A design can satisfy SAC
//!   perfectly while output bits `j` and `k` always flip together — both
//!   individually unbiased, the pair carrying half the entropy it appears to.
//!
//! §8's finding sharpens the motivation: these candidates are 150x–370x worse
//! than ChaCha at 3 rounds. **If that deficit has algebraic structure rather
//! than being merely slower mixing, the rank battery is where it shows.**
//!
//! ## Discipline
//!
//! * **Null validation at the width used**, as a permanent fixture — `PHASE_G`
//!   had to add this after a null validated at one width was used to license
//!   readings taken at another.
//! * **Disjoint seed bases per condition** — item (11). Shared bases produced a
//!   false +3.4-sigma anomaly once.
//! * ChaCha's own quarter round is the control in both batteries.
//! * BIC uses its own correlation-specific null (`bic_noise_floor`), never the
//!   proportion inversion — reusing the latter under-samples by exactly four.

use statelab_crypto::bic::{bic_matrix, bic_recommended_samples};
use statelab_crypto::linearity::{
    full_rank_probability, random_matrix_rank_trials, rank_trials, InputSet,
};
use statelab_crypto::quarter_round::{chacha_qr, QrPermutation, QrStep, QuarterRound};

const TOLERANCE: f64 = 0.12;
const ROUNDS: usize = 4;
const TRIALS: usize = 100;
const BITS: usize = 512;

fn qr(steps: [(u8, u8, u8, u8); 4]) -> QuarterRound {
    QuarterRound {
        steps: steps.map(|(add_to, add_from, xor_into, rot)| QrStep {
            add_to,
            add_from,
            xor_into,
            rot,
        }),
    }
}

fn designs() -> Vec<(&'static str, QuarterRound)> {
    vec![
        ("chacha (control)", chacha_qr()),
        (
            "cand-0",
            qr([(1, 2, 2, 16), (3, 2, 0, 24), (0, 3, 1, 16), (3, 1, 2, 16)]),
        ),
        (
            "cand-2",
            qr([(0, 3, 1, 8), (1, 2, 3, 24), (1, 3, 3, 8), (0, 3, 2, 8)]),
        ),
        (
            "cand-3",
            qr([(2, 1, 3, 16), (0, 3, 2, 16), (3, 2, 1, 24), (1, 0, 0, 16)]),
        ),
        (
            "cand-4 (all rot 24)",
            qr([(0, 2, 3, 24), (3, 2, 2, 24), (1, 3, 2, 24), (3, 2, 0, 24)]),
        ),
    ]
}

fn main() {
    println!("GF(2) RANK and BIC on the four confirmed quarter-round candidates\n");
    println!("  rounds {ROUNDS} (the round count PHASE_N's claim is made at)");
    println!("  wiring ChaCha's, fixed — only the quarter round differs\n");
    println!("  These see what avalanche cannot: rank sees LINEARITY, BIC sees");
    println!("  PAIRWISE DEPENDENCE. §8 found the candidates 150x-370x worse than");
    println!("  ChaCha at 3 rounds; if that deficit is ALGEBRAIC rather than just");
    println!("  slower mixing, the rank battery is where it shows.\n");

    // ---------------------------------------------------- rank null control
    println!("== GF(2) RANK, square regime {BITS}x{BITS}, {TRIALS} trials ==");
    println!("-- Null validation at the width used (permanent fixture) --");
    let null = random_matrix_rank_trials(BITS, TRIALS, 0xC0FF_EE00);
    println!(
        "   random-matrix n={BITS}: full {}/{}  exp {:.3}  z {:+.2}",
        null.full_rank,
        null.trials,
        null.expected_full_rank_fraction(),
        null.z_score()
    );
    println!(
        "   P(full rank) under the product-formula null = {:.6}\n",
        full_rank_probability(BITS, BITS)
    );

    println!(
        "  {:<24} {:>10} {:>10} {:>10}",
        "design", "full/100", "z", "verdict"
    );
    for (condition, (label, q)) in designs().into_iter().enumerate() {
        let p = QrPermutation::with_chacha_wiring(q);
        // Disjoint seed base per condition — item (11).
        let s = rank_trials(
            &p,
            ROUNDS,
            &InputSet::Stride(1),
            BITS,
            TRIALS,
            1 + condition as u64 * 10_000,
        );
        let z = s.z_score();
        let verdict = if z < -3.0 {
            "DEFICIENT"
        } else if z < -2.0 {
            "marginal"
        } else {
            "clean"
        };
        println!(
            "  {label:<24} {:>10} {z:>10.2} {verdict:>10}",
            format!("{}/{}", s.full_rank, s.trials)
        );
    }
    println!("\n   Negative z means MORE rank-deficient than chance — the direction");
    println!("   that indicates linear structure. PHASE_G read ascon at -5.93 and");
    println!("   still declined to call it a finding on one route.\n");

    // --------------------------------------------------------------- BIC
    let samples = bic_recommended_samples(BITS, TOLERANCE);
    println!("== BIC (Webster-Tavares correlation), {samples} samples ==");
    println!(
        "  {:<24} {:>10} {:>10} {:>10} {:>10}",
        "design", "max|r| range", "floor", "coverage", "verdict"
    );
    // Multi-seed by default — item (10). A kill is a claim and a claim from one
    // seed is a number. Three disjoint seeds; every one is reported.
    const BIC_SEEDS: [u64; 3] = [1, 909, 0xBEEF_0001];
    for (label, q) in designs() {
        let p = QrPermutation::with_chacha_wiring(q.clone());
        let mut devs = Vec::new();
        let mut floor = 0.0;
        let mut cover = 1.0f64;
        for &s in &BIC_SEEDS {
            let r = bic_matrix(&p, ROUNDS, samples, s);
            floor = r.noise_floor();
            cover = cover.min(r.coverage());
            devs.push(r.max_abs_correlation);
        }
        let worst = devs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let best = devs.iter().cloned().fold(f64::INFINITY, f64::min);
        let verdict = if cover <= 0.99 {
            "LOW COVER"
        } else if worst <= floor {
            "clean"
        } else if best > floor {
            "FAILS ALL"
        } else {
            "UNSTABLE"
        };
        println!(
            "  {label:<24} {:>10} {floor:>10.4} {cover:>10.3} {verdict:>10}",
            format!("{best:.3}-{worst:.3}")
        );
    }

    println!("\n-- How to read this --");
    println!("   RANK: a candidate clean here has no wholesale linear structure");
    println!("   the battery can see at this round count and geometry. PHASE_G's");
    println!("   ascon reading was geometry-specific, so clean on one probe is not");
    println!("   clean everywhere.");
    println!();
    println!("   BIC: max|r| at or below the floor means the worst output-bit pair");
    println!("   is no more correlated than chance predicts over 66,977,792 cells.");
    println!("   NOTE the standing open item (PHASE_H §5): the fair-coin null reads");
    println!("   ~25% below every real permutation and that gap is UNEXPLAINED, so");
    println!("   the analytic floor is the threshold, not the empirical null.");
    println!();
    println!("   Neither is a security claim. Both are proxies (item 16). CLAASP");
    println!("   is the gate and it has not run.");
}
