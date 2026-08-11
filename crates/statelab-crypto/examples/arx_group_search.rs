//! AVENUE 2, ARX AXIS 2 — does GROUP SIZE buy anything? Cost held constant.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example arx_group_search
//! ```
//!
//! ## The gap this fills
//!
//! `PHASE_T` §8 audited avenue 2 and found the design space had a hole:
//! **every ARX experiment used ChaCha's wiring, and every non-ChaCha-wiring
//! experiment was AND-based.** The two axes were varied independently and never
//! crossed.
//!
//! `PHASE_M` does **not** cover this. It searched *which of the sixteen words go
//! into each disjoint group of four*, with **group size, group count and overlap
//! fixed at ChaCha's values throughout.**
//!
//! `FINDING_REACH_GOVERNS.md` points straight here: if reach per round is the
//! governing variable, **group size is the most direct control over it that
//! exists.**
//!
//! ## Why this experiment is unconfounded
//!
//! A group of `g` words running `g` ARX steps costs `3g` operations, and there
//! are `16/g` groups per round. So:
//!
//! ```text
//!   g = 4   4 groups x 12 ops = 48
//!   g = 8   2 groups x 24 ops = 48
//!   g = 16  1 group  x 48 ops = 48
//! ```
//!
//! `g` must divide 16, and the three-operand step needs three distinct words, so
//! the available space is exactly **{4, 8, 16}**. `g = 2` is degenerate — at that
//! width `(i+2) mod 2 == i` and the step reads its own target, losing
//! invertibility. **The gate caught that on the first run and refused to
//! measure**, which is recorded here rather than silently dropped.
//!
//! **Cost per round is 48 for every group size, by construction.** Nothing is
//! traded off; only reach changes. `PHASE_T` §3's neighbour comparison was
//! confounded because it bought reach *and* added mixing — this cannot be.
//!
//! ## *** THE FITNESS METRIC AND KILL CRITERION, REGISTERED BEFORE RUNNING ***
//!
//! **Fitness:** `ops_per_round × rounds_to_saturation`. ChaCha's
//! `4 × 48 = 192` is the bar, unchanged from every other avenue 2 screen.
//!
//! Since cost is pinned at 48, fitness collapses to `48 × rounds`, so:
//!
//! > **A group size wins if and only if it saturates in 3 rounds or fewer.**
//!
//! **KILL CRITERION:** if no group size saturates in ≤ 3 rounds, on **every**
//! seed, the ARX mixing-graph axis is **closed**, and with axis 1 already
//! reported that closes the ARX branch of avenue 2 entirely.
//!
//! ## The step function, and why it is invertible at any group size
//!
//! Salsa's shape: each step modifies **one** word as a function of others.
//!
//! ```text
//! step i:   x[i] ^= (x[(i+1) mod g] + x[(i+2) mod g]) <<< r_i
//! ```
//!
//! A sequence of single-word updates is invertible regardless of index pattern:
//! undo the steps in reverse order, and at each reverse step the two source
//! words hold exactly what they held before that forward step, because that step
//! did not touch them. **The same reason ChaCha's `a += b` and `d ^= a` are
//! invertible.** The gate re-checks it rather than trusting the argument.
//!
//! ## What g = 4 here is NOT
//!
//! **It is not ChaCha.** This family is Salsa-shaped (`x ^= (y+z) <<< r`), where
//! ChaCha's quarter round is a different four-step sequence. `g = 4` is the
//! *within-family reference row*, and any gap between it and ChaCha is a
//! property of the step shape, not of group size. **ChaCha is measured
//! separately as the external bar.** Conflating the two would make the whole
//! table unreadable.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **Ops per round is exactly 48 for every group size.** Arithmetic, and it
//!    is the control that makes the comparison unconfounded. If the printed
//!    figure is not 48 across the board, the construction is wrong and nothing
//!    below is readable.
//! 2. **Rounds fall as group size rises**, monotonically. This is the reach
//!    hypothesis' prediction, and this is the cleanest test of it the project
//!    has — no cost is traded.
//! 3. **Nothing saturates in ≤ 3 rounds, so nothing beats the bar.** Two rounds
//!    is the floor for information to reach every word at all, and `PHASE_M`
//!    found ChaCha's wiring already diameter-optimal within its template.
//!
//! **Prediction 3 failing would be the first genuinely new above-bar ARX result
//! outside axis 1's step-count finding.** It would be a SCREEN result and gets
//! the same immediate skepticism every good result in this project gets:
//! confirmation on unseen seeds, BIC, GF(2) rank, real cycles, then CLAASP.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::permutation::Permutation;
use statelab_crypto::saturation::saturation_point;
use statelab_crypto::systems::ChaCha;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const WORDS: usize = 16;
const MAX_ROUNDS: usize = 20;
const SEEDS: [u64; 3] = [12345, 0x5EED_0002, 0xC0FF_EE03];
const BAR: usize = 192;
const ROTS: [u32; 4] = [16, 12, 8, 7];

/// How the round splits the state into groups.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Partition {
    /// Consecutive runs: `{0..g}, {g..2g}, …`
    Blocks,
    /// Strided: every `16/g`-th word, the shape ChaCha's columns have.
    Strided,
}

#[derive(Clone, Copy)]
struct GroupDesign {
    name: &'static str,
    /// Words per group. Must divide 16.
    g: usize,
}

impl GroupDesign {
    /// `g` steps per group x 3 ops, times `16/g` groups = 48, for every `g`.
    fn ops_per_round(&self) -> usize {
        let groups = WORDS / self.g;
        groups * self.g * 3
    }

    fn lanes(&self, part: Partition, which: usize) -> Vec<usize> {
        let g = self.g;
        let groups = WORDS / g;
        match part {
            Partition::Blocks => (0..g).map(|k| which * g + k).collect(),
            Partition::Strided => (0..g).map(|k| which + k * groups).collect(),
        }
    }

    /// Salsa-shaped ARX over one group. Each step modifies exactly one word.
    ///
    /// **Requires `g >= 3`.** At `g = 2`, `(i+2) mod 2 == i`, so the step reads
    /// `x[i]` as one of its own operands and the one-word-modified-using-only-
    /// others property that guarantees invertibility is lost. The gate caught
    /// this before anything was measured; `g = 2` is excluded as a degeneracy of
    /// this step shape, not as a result about small groups.
    #[inline]
    fn mix(&self, q: &mut [u32]) {
        let g = q.len();
        for i in 0..g {
            let r = ROTS[i % ROTS.len()];
            let a = q[(i + 1) % g];
            let b = q[(i + 2) % g];
            q[i] ^= a.wrapping_add(b).rotate_left(r);
        }
    }
}

impl Permutation for GroupDesign {
    fn name(&self) -> &'static str {
        self.name
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        8
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let mut x = [0u32; WORDS];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }
        let part = if round_index.is_multiple_of(2) {
            Partition::Strided
        } else {
            Partition::Blocks
        };
        for which in 0..(WORDS / self.g) {
            let lanes = self.lanes(part, which);
            let mut q: Vec<u32> = lanes.iter().map(|&l| x[l]).collect();
            self.mix(&mut q);
            for (k, &l) in lanes.iter().enumerate() {
                x[l] = q[k];
            }
        }
        for (i, w) in x.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

/// One round must be a bijection. Checked on a narrow instance: 16 lanes of ONE
/// bit each is not meaningful for ARX (carries need width), so this instead
/// checks invertibility structurally by round-tripping random states through a
/// hand-written inverse.
fn round_is_invertible(d: &GroupDesign) -> bool {
    let mut probe = statelab_crypto::avalanche::Probe::new(0x1_5EED_1234);
    for _ in 0..256 {
        let mut before = [0u8; 64];
        probe.fill(&mut before);
        for round_index in 0..2usize {
            let mut after = before;
            d.round(&mut after, round_index);
            // invert
            let mut x = [0u32; WORDS];
            for (i, w) in x.iter_mut().enumerate() {
                *w = u32::from_le_bytes(after[i * 4..i * 4 + 4].try_into().expect("4"));
            }
            let part = if round_index.is_multiple_of(2) {
                Partition::Strided
            } else {
                Partition::Blocks
            };
            for which in (0..(WORDS / d.g)).rev() {
                let lanes = d.lanes(part, which);
                let mut q: Vec<u32> = lanes.iter().map(|&l| x[l]).collect();
                let g = q.len();
                for i in (0..g).rev() {
                    let r = ROTS[i % ROTS.len()];
                    let a = q[(i + 1) % g];
                    let b = q[(i + 2) % g];
                    q[i] ^= a.wrapping_add(b).rotate_left(r);
                }
                for (k, &l) in lanes.iter().enumerate() {
                    x[l] = q[k];
                }
            }
            let mut back = [0u8; 64];
            for (i, w) in x.iter().enumerate() {
                back[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
            }
            if back != before {
                return false;
            }
        }
    }
    true
}

fn saturates(perm: &dyn Permutation, samples: usize) -> Option<f64> {
    let mut worst: Option<f64> = None;
    for &seed in &SEEDS {
        let pts: Vec<(f64, f64)> = (1..=MAX_ROUNDS)
            .map(|r| {
                let m = avalanche_matrix(perm, r, samples, seed);
                (r as f64, m.max_deviation())
            })
            .collect();
        let r = saturation_point(&pts, TOLERANCE, 1.0)?;
        worst = Some(worst.map_or(r, |w: f64| w.max(r)));
    }
    worst
}

fn main() {
    let samples = recommended_samples(BITS, TOLERANCE);
    println!("AVENUE 2, ARX AXIS 2 — DOES GROUP SIZE BUY ANYTHING?\n");
    println!(
        "  avalanche {samples} samples, tol {TOLERANCE}, {} disjoint seeds\n",
        SEEDS.len()
    );
    println!("  *** COST IS PINNED AT 48 OPS/ROUND FOR EVERY GROUP SIZE. ***");
    println!("  g steps x 3 ops x (16/g) groups = 48, identically. Nothing is");
    println!("  traded off; ONLY REACH CHANGES.\n");
    println!("  FITNESS  ops_per_round x rounds_to_saturation. Bar = ChaCha's {BAR}.");
    println!("  Cost pinned, so a group size WINS IFF IT SATURATES IN <= 3 ROUNDS.");
    println!("  KILL CRITERION: if none does, on every seed, THE ARX MIXING-GRAPH");
    println!("  AXIS IS CLOSED and with axis 1 that closes avenue 2's ARX branch.\n");
    println!("  PREDICTION 1: ops/round is 48 for every g. Arithmetic control.");
    println!("  PREDICTION 2: rounds fall monotonically as g rises (reach).");
    println!("  PREDICTION 3: nothing reaches <=3 rounds, so nothing beats {BAR}.");
    println!("                FAILING 3 IS THE FIRST NEW ABOVE-BAR ARX RESULT");
    println!("                OUTSIDE AXIS 1 — and gets the same skepticism.\n");

    // g must divide 16, and the 3-operand step needs g >= 3, so the available
    // space under this step shape is exactly {4, 8, 16}. g=2 is degenerate:
    // (i+2) mod 2 == i, so the step reads its own target. The gate caught it on
    // the first run and refused to measure — excluded as a property of the step
    // shape, not as a result about small groups.
    let designs = [
        GroupDesign {
            name: "g=4  (4 groups, ref)",
            g: 4,
        },
        GroupDesign {
            name: "g=8  (2 groups)",
            g: 8,
        },
        GroupDesign {
            name: "g=16 (all-to-all)",
            g: 16,
        },
    ];

    // ---------------------------------------------- gate, before measurement
    println!("== Invertibility gate (round-trip, 256 random states x 2 partitions) ==");
    let mut gate_ok = true;
    for d in designs.iter() {
        let ok = round_is_invertible(d);
        if !ok {
            gate_ok = false;
        }
        println!(
            "  {:<24} {}",
            d.name,
            if ok { "INVERTIBLE" } else { "*** LOSSY ***" }
        );
    }
    if !gate_ok {
        println!("\n  >>> A ROUND IS NOT A PERMUTATION. Nothing measured.");
        return;
    }
    println!("  >>> all rounds are permutations.\n");

    // ---------------------------------------------------- prediction 1 check
    let all48 = designs.iter().all(|d| d.ops_per_round() == 48);
    if all48 {
        println!("  >>> PREDICTION 1 HOLDS. 48 ops/round for every group size —");
        println!("      the comparison is unconfounded.\n");
    } else {
        println!("  >>> PREDICTION 1 FAILS. Cost is NOT constant across group");
        println!("      sizes, so reach is confounded with cost. VOID.");
        for d in designs.iter() {
            println!("      {:<24} {}", d.name, d.ops_per_round());
        }
        return;
    }

    // ------------------------------------------------------------- measure
    println!(
        "  {:<24} {:>7} {:>9} {:>11} {:>9}",
        "design", "rounds", "ops/rnd", "TOTAL ops", "vs bar"
    );
    let cc = saturates(&ChaCha, samples);
    match cc {
        Some(r) => println!(
            "  {:<24} {r:>7.0} {:>9} {:>11} {:>9}",
            "chacha (EXTERNAL BAR)",
            48,
            r as usize * 48,
            "1.00x"
        ),
        None => {
            println!("  chacha never saturated — screen invalid.");
            return;
        }
    }

    let mut rounds_by_g: Vec<(usize, f64)> = Vec::new();
    let mut winners = Vec::new();
    for d in designs.iter() {
        let ops = d.ops_per_round();
        match saturates(d, samples) {
            None => println!(
                "  {:<24} {:>7} {ops:>9} {:>11} {:>9}",
                d.name,
                format!(">{MAX_ROUNDS}"),
                "-",
                "-"
            ),
            Some(r) => {
                rounds_by_g.push((d.g, r));
                let total = r as usize * ops;
                println!(
                    "  {:<24} {r:>7.0} {ops:>9} {total:>11} {:>9}",
                    d.name,
                    format!("{:.2}x", total as f64 / BAR as f64)
                );
                if total < BAR {
                    winners.push((d.name, r, total));
                }
            }
        }
    }

    println!("\n== Verdict ==");
    let monotone = rounds_by_g.windows(2).all(|w| w[1].1 <= w[0].1);
    if monotone && rounds_by_g.len() > 1 {
        println!("  >>> PREDICTION 2 HOLDS. Rounds fall monotonically with group");
        println!("      size at constant cost — REACH, on the cleanest test of it");
        println!("      this project has run. FINDING_REACH_GOVERNS gains a fourth");
        println!("      confirmation, and an unconfounded one.");
    } else {
        println!("  >>> PREDICTION 2 FAILS. Rounds do NOT fall monotonically with");
        println!("      group size: {rounds_by_g:?}");
        println!("      Reach alone does not determine saturation, and");
        println!("      FINDING_REACH_GOVERNS needs narrowing.");
    }

    if winners.is_empty() {
        println!("  >>> PREDICTION 3 HOLDS. Nothing saturates in <=3 rounds.");
        println!("      *** KILL CRITERION MET: THE ARX MIXING-GRAPH AXIS IS CLOSED. ***");
        println!("      With axis 1 reported, that is avenue 2's ARX branch complete.");
    } else {
        println!(
            "  >>> *** PREDICTION 3 FAILS — {} BEAT THE BAR. ***",
            winners.len()
        );
        for (n, r, t) in &winners {
            println!("      {n}: {r:.0} rounds, {t} ops vs {BAR}");
        }
        println!("      SCREEN RESULT ONLY, and the first new above-bar ARX finding");
        println!("      outside axis 1. Required before it is believed: unseen seeds,");
        println!("      BIC, GF(2) rank, real cyc/B, a SECOND construction, CLAASP.");
    }
}
