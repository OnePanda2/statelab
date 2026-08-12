//! DOES N=5 SURVIVE THE CHEAP EXACT CHECK? — branch number for N=3/4/5.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example branch_number
//! ```
//!
//! ## Why this runs before anything expensive
//!
//! `PHASE_L` §4.1 recorded branch number on 2026-08-07 as "one of the first
//! things a new design's diffusion layer should be checked against **before any
//! more expensive statistical/empirical testing**", and it was never built.
//! `PHASE_U` then produced the project's first real-cycle result — N=5 at
//! **3.0–3.2%** faster than ChaCha20 — which is now the one candidate worth
//! spending anything on.
//!
//! Branch number is exact, needs no statistics, no seeds and no timing, and it
//! applies retroactively. If N=5 fails it, everything queued behind N=5 is
//! moot and the cost of finding that out is minutes.
//!
//! ## *** WHAT A RESULT HERE CAN AND CANNOT MEAN ***
//!
//! Both quantities computed are **UPPER BOUNDS** on the true branch number
//! (`branch.rs` explains why: the exact problem is NP-hard). Therefore:
//!
//! * **A LOW number is PROOF of a weakness** — the input achieving it is a real
//!   input, and it really does produce that little diffusion.
//! * **A MAXIMAL number PROVES NOTHING.** It says only that no weakness was
//!   found in the part of the space that was searched.
//!
//! **THIS CHECK CAN KILL N=5. IT CANNOT BLESS IT.** Whatever it returns, N=5
//! still needs the differential and linear trail bounds that only MILP/SAT can
//! give — `PHASE_L`'s wall, and the reason CLAASP is the next thing after this.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **At one round, all three designs read 5.** One column round leaves four
//!    disjoint groups of four, so 1 + 4 is the structural ceiling and every
//!    N >= 3 completes intra-group dependency inside a single quarter round.
//! 2. **At two rounds, all three designs read 17** — the maximum on 16 bundles.
//!    ChaCha's alternating column/diagonal wiring gives full word-level
//!    dependency in two rounds by construction.
//! 3. **Therefore this metric will NOT discriminate between N=3, N=4 and N=5**,
//!    and the honest report is a clean negative: no cheap kill was available.
//!    Recorded now so it cannot be dressed up afterwards as a pass for N=5.
//! 4. **N=5 will not be killed.** It shares ChaCha's wiring exactly, and
//!    `PHASE_M` found that wiring diameter-optimal.
//! 5. **The one place a surprise could come from is the bit-level bound**,
//!    which unlike the pattern model accounts for cancellation. If weight-2 or
//!    weight-3 differences cancel into fewer active words than the dependency
//!    matrix predicts, that gap is real and is the finding.

use statelab_crypto::branch::{
    bitwise_branch_witness, pattern_branch_number, LinearMap, MAX_BRANCH,
};

const DESIGNS: [usize; 3] = [3, 4, 5];

fn label(n: usize) -> String {
    if n == 4 {
        "N=4 (ChaCha)".to_string()
    } else {
        format!("N={n}")
    }
}

fn main() {
    println!("DOES N=5 SURVIVE THE CHEAP EXACT CHECK?\n");
    println!("  branch number of the GF(2)-linearised round, 16 bundles of 32 bits");
    println!("  maximum attainable: {MAX_BRANCH}\n");
    println!("  BOTH COLUMNS ARE UPPER BOUNDS ON THE TRUE BRANCH NUMBER.");
    println!("  A LOW value is PROOF of a weakness. A MAXIMAL value PROVES NOTHING.");
    println!("  This check can KILL a design. It cannot BLESS one.\n");
    println!("  PREDICTION 1: every design reads 5 at one round.");
    println!("  PREDICTION 2: every design reads 17 at two rounds.");
    println!("  PREDICTION 3: the metric does NOT discriminate. Clean negative.");
    println!("  PREDICTION 4: N=5 is not killed.");
    println!("  PREDICTION 5: any surprise comes from the bit-level bound, which");
    println!("                accounts for cancellation where the pattern model");
    println!("                does not.\n");

    println!(
        "  {:<16} {:>7} {:>12} {:>12} {:>12}",
        "design", "rounds", "pattern", "bits w<=1", "bits w<=2"
    );

    // (n, rounds, pattern, w1, w2)
    let mut rows: Vec<(usize, usize, usize, usize, usize)> = Vec::new();
    for rounds in 0..=4usize {
        for n in DESIGNS {
            let m = LinearMap::build(n, rounds);
            let pat = pattern_branch_number(&m.dependency_matrix());
            let w1 = m.bitwise_branch_bound(1);
            let w2 = m.bitwise_branch_bound(2);
            rows.push((n, rounds, pat, w1, w2));
            println!("  {:<16} {rounds:>7} {pat:>12} {w1:>12} {w2:>12}", label(n));
        }
        println!();
    }

    println!("== Weight-3 exhaustive at the two decisive round counts ==");
    println!("  22,238,720 differences per cell, with cancellation.\n");
    println!("  {:<16} {:>7} {:>12}", "design", "rounds", "bits w<=3");
    let mut deep: Vec<(usize, usize, usize)> = Vec::new();
    for rounds in [1usize, 2] {
        for n in DESIGNS {
            let m = LinearMap::build(n, rounds);
            let w3 = m.bitwise_branch_bound(3);
            deep.push((n, rounds, w3));
            println!("  {:<16} {rounds:>7} {w3:>12}", label(n));
        }
    }

    // ---------------------------------------------------------------------
    // The result below separates the designs, and N=5 comes out ahead. That is
    // the direction that gets checked hardest here, not the one that gets
    // waved through. Two independent attacks on it follow.
    // ---------------------------------------------------------------------

    println!("\n== Check 1: witness re-derivation, WITHOUT the matrix ==");
    println!("  The tables above come from XORing precomputed basis columns. A");
    println!("  bug there would fabricate the whole result. Each minimum is now");
    println!("  re-derived by running its witness straight through");
    println!("  linear_rounds and recounting.\n");
    println!(
        "  {:<16} {:>7} {:>10} {:>12} {:>8}",
        "design", "rounds", "from cols", "direct eval", "agree"
    );
    let mut all_agree = true;
    for rounds in [1usize, 2, 3, 4] {
        for n in DESIGNS {
            let m = LinearMap::build(n, rounds);
            let from_cols = m.bitwise_branch_bound(2);
            let (witness, direct) = bitwise_branch_witness(&m, n, rounds);
            let mut y = witness;
            statelab_crypto::branch::linear_rounds(&mut y, n, rounds);
            let recount = statelab_crypto::branch::word_weight(&witness)
                + statelab_crypto::branch::word_weight(&y);
            let ok = from_cols == direct && direct == recount;
            all_agree &= ok;
            println!(
                "  {:<16} {rounds:>7} {from_cols:>10} {recount:>12} {:>8}",
                label(n),
                if ok { "yes" } else { "NO" }
            );
        }
    }
    if all_agree {
        println!("\n  >>> All witnesses re-derive. The matrix path is not fabricating.");
    } else {
        println!("\n  >>> *** MISMATCH. THE RESULT IS VOID. *** The matrix path and");
        println!("      direct evaluation disagree, so the tables above cannot be");
        println!("      trusted and nothing here should be reported.");
        return;
    }

    println!("\n== Check 2: the same comparison at EQUAL COST, not equal rounds ==");
    println!("  N=3/4/5 cost 36/48/60 ops per round, so a table indexed by rounds");
    println!("  compares unequal work and flatters the larger N. This project has");
    println!("  been caught by exactly that before. Rounds to reach the maximum of");
    println!("  {MAX_BRANCH}, priced in total operations:\n");
    println!(
        "  {:<16} {:>8} {:>10} {:>12} {:>10}",
        "design", "ops/rnd", "rounds", "total ops", "vs ChaCha"
    );
    let ops_per_round = |n: usize| n * 3 * 4;
    let mut totals: Vec<(usize, Option<usize>)> = Vec::new();
    for n in DESIGNS {
        let first_max = (1..=6usize).find(|r| {
            let m = LinearMap::build(n, *r);
            pattern_branch_number(&m.dependency_matrix()) == MAX_BRANCH
        });
        totals.push((n, first_max.map(|r| r * ops_per_round(n))));
    }
    let base = totals
        .iter()
        .find(|t| t.0 == 4)
        .and_then(|t| t.1)
        .expect("ChaCha reaches the maximum");
    for (n, total) in &totals {
        let first_max = total.map(|t| t / ops_per_round(*n));
        match (first_max, total) {
            (Some(r), Some(t)) => {
                let ratio = *t as f64 / base as f64;
                println!(
                    "  {:<16} {:>8} {r:>10} {t:>12} {ratio:>9.3}x",
                    label(*n),
                    ops_per_round(*n)
                );
            }
            _ => println!("  {:<16} {:>8} {:>10}", label(*n), ops_per_round(*n), ">6"),
        }
    }

    println!("\n== Verdict ==");

    let at = |r: usize| -> Vec<(usize, usize, usize, usize)> {
        rows.iter()
            .filter(|x| x.1 == r)
            .map(|x| (x.0, x.2, x.3, x.4))
            .collect()
    };

    let r1 = at(1);
    let p1 = r1.iter().all(|x| x.1 == 5 && x.2 == 5 && x.3 == 5);
    if p1 {
        println!("  >>> PREDICTION 1 HOLDS. Every design reads 5 at one round —");
        println!("      the group-limited ceiling of 1 + 4.");
    } else {
        println!("  >>> PREDICTION 1 FAILS. One-round readings differ:");
        for (n, p, a, b) in &r1 {
            println!("      {:<16} pattern {p}  w<=1 {a}  w<=2 {b}", label(*n));
        }
    }

    let r2 = at(2);
    let p2 = r2
        .iter()
        .all(|x| x.1 == MAX_BRANCH && x.2 == MAX_BRANCH && x.3 == MAX_BRANCH);
    if p2 {
        println!("  >>> PREDICTION 2 HOLDS. Every design reaches {MAX_BRANCH} at two rounds.");
    } else {
        println!("  >>> PREDICTION 2 FAILS. Two-round readings differ:");
        for (n, p, a, b) in &r2 {
            println!("      {:<16} pattern {p}  w<=1 {a}  w<=2 {b}", label(*n));
        }
    }

    // Does anything separate the designs at any round count?
    let mut discriminates = false;
    for rounds in 0..=4usize {
        let v = at(rounds);
        let first = (v[0].1, v[0].2, v[0].3);
        if v.iter().any(|x| (x.1, x.2, x.3) != first) {
            discriminates = true;
        }
    }
    for rounds in [1usize, 2] {
        let v: Vec<usize> = deep.iter().filter(|x| x.1 == rounds).map(|x| x.2).collect();
        if v.iter().any(|x| *x != v[0]) {
            discriminates = true;
        }
    }

    if discriminates {
        println!("\n  >>> *** PREDICTION 3 FAILS. THE METRIC DISCRIMINATES. ***");
        println!("      Some design reads differently from the others. That is a");
        println!("      real, exact, statistics-free difference and it is the");
        println!("      finding. Read the tables above rather than this line.");
    } else {
        println!("\n  >>> PREDICTION 3 HOLDS. N=3, N=4 and N=5 are INDISTINGUISHABLE");
        println!("      on every cell computed. The metric does not separate them.");
    }

    // The kill test, which is the reason this ran first.
    let n5_low: Vec<(usize, usize)> = rows
        .iter()
        .filter(|x| x.0 == 5 && x.1 > 0)
        .map(|x| (x.1, x.2.min(x.3).min(x.4)))
        .collect();
    let chacha_low: Vec<(usize, usize)> = rows
        .iter()
        .filter(|x| x.0 == 4 && x.1 > 0)
        .map(|x| (x.1, x.2.min(x.3).min(x.4)))
        .collect();
    let killed = n5_low.iter().zip(chacha_low.iter()).any(|(a, b)| a.1 < b.1);

    if killed {
        println!("\n  >>> *** N=5 IS KILLED. *** It reads BELOW ChaCha at some round");
        println!("      count, and because these are upper bounds, a low reading is");
        println!("      PROOF. Everything queued behind N=5 is moot.");
        for ((r, a), (_, b)) in n5_low.iter().zip(chacha_low.iter()) {
            if a < b {
                println!("      round {r}: N=5 reads {a} against ChaCha's {b}");
            }
        }
    } else {
        println!("\n  >>> PREDICTION 4 HOLDS. N=5 IS NOT KILLED. It reads at or above");
        println!("      ChaCha at every round count computed.");
        println!("      *** THIS IS NOT A PASS. *** These are upper bounds; a maximal");
        println!("      reading means only that no weakness was found in the part of");
        println!("      the space searched. N=5 still needs differential and linear");
        println!("      trail bounds from MILP/SAT before it is anything at all.");
    }
}
