//! PHASE G — the ascon observation, hardened (working prompt Part 2).
//!
//! PHASE_F_GF2_LINEARITY §6 recorded, as OPEN AND NOT A FINDING: at a fixed 4
//! rounds on the square-regime rank battery, `ascon` reads z = −5.71 and
//! `chacha64` z = −2.40 where `chacha` reads +0.47. Four reasons it was
//! withheld — the round count was confounded across designs with different
//! margins, ascon's avalanche is already saturated at 4 rounds, no round sweep
//! had been run for ascon, and there was one route only.
//!
//! This driver addresses the first three. It does NOT address the fourth: the
//! second route (PractRand BRank) is not reachable from here. Read §2.5 of the
//! working prompt — one route is not a finding, and this driver's output must
//! not be written up as one.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example phase_g_ascon_rounds
//! ```
//!
//! Design of the run:
//!   * Square regime m = n (the corrected default; the tall-thin regime tests
//!     for affine collapse, which is not the property in question here).
//!   * 100 trials per condition.
//!   * DISJOINT seed ranges per condition — methodological item (11). Every
//!     condition below draws `seed_base(i) .. seed_base(i) + TRIALS` with the
//!     bases 10_000 apart, so no two compared arms share a base state.
//!   * Null-validation control run at EVERY width the sweep uses (320, 512,
//!     1024), not only at 512. `full_rank_probability` is width-dependent, so
//!     a null validated at one width does not validate the others, and ascon
//!     is 320 bits wide where chacha is 512.

use statelab_crypto::linearity::{
    across_lanes, full_rank_probability, random_matrix_rank_trials, rank_trials, InputSet,
    RankTrialSummary,
};
use statelab_crypto::permutation_by_name;

const TRIALS: usize = 100;

/// Disjoint per condition — methodological item (11).
fn seed_base(condition_index: usize) -> u64 {
    1 + condition_index as u64 * 10_000
}

fn row(label: &str, s: &RankTrialSummary) {
    let degenerate = if s.null_is_degenerate() {
        "  <-- NULL DEGENERATE, no power"
    } else {
        ""
    };
    println!(
        "  {:<34} {:>4} rounds  {:>4}x{:<4}  full {:>3}/{:<3}  exp {:>6.3}  z {:>+7.2}{}",
        label,
        s.rounds,
        s.rows,
        s.cols,
        s.full_rank,
        s.trials,
        s.expected_full_rank_fraction(),
        s.z_score(),
        degenerate
    );
}

fn main() {
    println!("PHASE G — ascon round sweep on the GF(2) rank battery");
    println!("Square regime, {TRIALS} trials, disjoint seeds per condition.\n");

    // ---------------------------------------------------------------- controls
    println!("-- 0. NULL VALIDATION, at every width this driver uses --");
    println!("   Random binary matrices against the product-formula null. This is");
    println!("   a permanent fixture of the known-answer set, not a diagnostic.");
    println!("   Run at 320 as well as 512 because ascon is 320 bits wide and the");
    println!("   null is width-dependent — a null validated at 512 says nothing");
    println!("   about the width every ascon reading below is taken at.\n");
    for (i, n) in [320usize, 512, 1024].iter().enumerate() {
        row(
            &format!("random-matrix n={n}"),
            &random_matrix_rank_trials(*n, TRIALS, 0xC0FF_EE00 + i as u64),
        );
    }
    println!();

    let designs = ["ascon", "chacha", "chacha64"];
    for name in designs {
        let p = permutation_by_name(name).expect("registered");
        println!(
            "   {name:<10} state {:>3} B = {:>4} bits, design rounds {}",
            p.state_bytes(),
            p.state_bytes() * 8,
            p.default_rounds()
        );
    }
    println!();

    // ------------------------------------------------- 2.1 / 2.4 absolute rounds
    println!("-- A. EQUAL ABSOLUTE ROUNDS (working prompt 2.1, 2.4) --");
    println!("   The comparison PHASE_F §6 made. Reported here as one of two");
    println!("   framings, not as the framing.\n");
    let mut condition = 0usize;
    for name in designs {
        let p = permutation_by_name(name).expect("registered");
        let n_bits = p.state_bytes() * 8;
        for r in [4usize, 6, 8, 12] {
            let s = rank_trials(
                p.as_ref(),
                r,
                &InputSet::Stride(1),
                n_bits,
                TRIALS,
                seed_base(condition),
            );
            row(name, &s);
            condition += 1;
        }
        println!();
    }

    // ------------------------------------------------------- 2.2 equal fraction
    println!("-- B. EQUAL FRACTION OF DESIGN MARGIN (working prompt 2.2) --");
    println!("   ascon's permutation is specified at 12 rounds, chacha at 20.");
    println!("   Four absolute rounds is a THIRD of ascon's margin and a FIFTH of");
    println!("   chacha's, so the §6 reading compared a design at 33% of its");
    println!("   specification against one at 20% of its own. Both framings are");
    println!("   reported. Neither is silently preferred.\n");
    for frac in [0.25f64, 0.33, 0.50, 0.67] {
        println!("   fraction of design margin = {frac:.2}");
        for name in designs {
            let p = permutation_by_name(name).expect("registered");
            let n_bits = p.state_bytes() * 8;
            let r = ((p.default_rounds() as f64 * frac).round() as usize).max(1);
            let s = rank_trials(
                p.as_ref(),
                r,
                &InputSet::Stride(1),
                n_bits,
                TRIALS,
                seed_base(condition),
            );
            row(name, &s);
            condition += 1;
        }
        println!();
    }

    // ------------------------------------------------- 2.3 partial: second shape
    println!("-- C. SECOND INPUT GEOMETRY (partial 2.3 — see the caveat) --");
    println!("   `across_lanes` instead of the counter stride: a deliberately");
    println!("   different probe shape, so the reading is not only ever taken on");
    println!("   one geometry.");
    println!();
    println!("   *** THIS IS NOT THE SECOND ROUTE §2.3 ASKS FOR. *** It is the");
    println!("   same statistic, the same code and the same instrument on a");
    println!("   different input shape. A second ROUTE means an independently");
    println!("   coded measurement — PractRand BRank, or the N3-STATISTICAL");
    println!("   construction. Neither was run. Per §2.5 the ascon reading");
    println!("   therefore stays RECORDED, NOT CLAIMED.\n");
    for name in designs {
        let p = permutation_by_name(name).expect("registered");
        let n_bits = p.state_bytes() * 8;
        let dirs = across_lanes(p.state_bytes(), n_bits);
        for r in [4usize, 8] {
            let s = rank_trials(
                p.as_ref(),
                r,
                &InputSet::Subspace(dirs.clone()),
                n_bits,
                TRIALS,
                seed_base(condition),
            );
            row(&format!("{name} (across-lanes)"), &s);
            condition += 1;
        }
    }

    println!("\n-- Reference: expected full-rank fraction under the null --");
    for n in [320usize, 512, 1024] {
        println!(
            "   n={n:<5} P(full rank) = {:.6}",
            full_rank_probability(n, n)
        );
    }
    println!("\n{condition} conditions, all seed ranges disjoint.");
}
