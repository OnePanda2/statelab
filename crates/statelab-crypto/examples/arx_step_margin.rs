//! HOW FRAGILE IS THE N=3 / N=5 RESULT? — margin analysis, not a new search.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example arx_step_margin
//! ```
//!
//! ## Why this exists
//!
//! `arx_step_search` reported N=3 (5 rounds x 36 = 180) and N=5 (3 x 60 = 180)
//! beating ChaCha's 192. **A 6.25% margin.**
//!
//! The metric is `ops_per_round x rounds_to_saturation`, and
//! **rounds_to_saturation is an integer.** So the product moves in steps of
//! `ops_per_round`:
//!
//! ```text
//! N=3:  4 rounds -> 144   5 rounds -> 180   6 rounds -> 216
//! N=5:  2 rounds -> 120   3 rounds -> 180   4 rounds -> 240
//! ```
//!
//! **One round either way is a 20–33% swing.** A 6.25% "win" sits far below the
//! granularity of the thing measuring it. If either design's saturation call was
//! marginal — max deviation barely under tolerance at the reported round — the
//! result is an artefact of where the threshold happened to fall.
//!
//! This does not re-run the search. It prints the **deviation curve** around the
//! saturation point for each design and asks how much slack the call had.
//!
//! ## What would make the result real, and what would kill it
//!
//! * **Comfortable margin** at the reported round on every seed, with the
//!   previous round clearly over tolerance: the saturation point is genuine and
//!   the 6.25% is worth taking to confirmation.
//! * **Marginal margin** — deviation within a few percent of tolerance, or
//!   seed-dependent: the round count is a coin flip, the product is noise, and
//!   **the result should be withdrawn rather than promoted.**
//!
//! This is `PHASE_M` item (17) again — every fitness statistic has a regime
//! where it fails. Here the suspected failure is **quantisation**: a continuous
//! quantity read through an integer.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::permutation::Permutation;
use statelab_crypto::topology::chacha_topology;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const WORDS: usize = 16;
const SEEDS: [u64; 5] = [12345, 0x5EED_0002, 0xC0FF_EE03, 0xA11C_E004, 0x0B0B_0005];
const ROTS: [u32; 4] = [16, 12, 8, 7];

#[derive(Clone, Copy)]
struct ArxSteps {
    name: &'static str,
    steps: usize,
}

impl ArxSteps {
    fn ops_per_round(&self) -> usize {
        self.steps * 3 * 4
    }
    #[inline]
    fn quarter(&self, q: &mut [u32; 4]) {
        for i in 0..self.steps {
            let r = ROTS[i % ROTS.len()];
            if i % 2 == 0 {
                q[0] = q[0].wrapping_add(q[1]);
                q[3] = (q[3] ^ q[0]).rotate_left(r);
            } else {
                q[2] = q[2].wrapping_add(q[3]);
                q[1] = (q[1] ^ q[2]).rotate_left(r);
            }
        }
    }
}

impl Permutation for ArxSteps {
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
        let topo = chacha_topology();
        let mut x = [0u32; WORDS];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }
        for g in &topo.partitions[round_index % 2] {
            let mut q = [
                x[g[0] as usize],
                x[g[1] as usize],
                x[g[2] as usize],
                x[g[3] as usize],
            ];
            self.quarter(&mut q);
            for (k, &lane) in g.iter().enumerate() {
                x[lane as usize] = q[k];
            }
        }
        for (i, w) in x.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

fn main() {
    let samples = recommended_samples(BITS, TOLERANCE);
    println!("MARGIN ANALYSIS — how fragile is the N=3 / N=5 result?\n");
    println!(
        "  {samples} samples, tolerance {TOLERANCE}, {} seeds",
        SEEDS.len()
    );
    println!("  TWO SEEDS BEYOND the three the screen used, so this is partly");
    println!("  a confirmation set rather than a re-read of the same data.\n");
    println!("  The product moves in steps of ops_per_round because rounds are");
    println!("  integers. One round is a 20-33% swing; the claimed win is 6.25%.\n");

    let designs = [
        ArxSteps {
            name: "N=3",
            steps: 3,
        },
        ArxSteps {
            name: "N=4 (chacha)",
            steps: 4,
        },
        ArxSteps {
            name: "N=5",
            steps: 5,
        },
    ];

    for d in designs.iter() {
        println!("== {} ({} ops/round) ==", d.name, d.ops_per_round());
        println!(
            "  {:>6} {:>10} {:>10} {:>10} {:>9}",
            "round", "worst dev", "best dev", "slack", "verdict"
        );
        for r in 1..=8usize {
            let devs: Vec<f64> = SEEDS
                .iter()
                .map(|&s| avalanche_matrix(d, r, samples, s).max_deviation())
                .collect();
            let worst = devs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let best = devs.iter().cloned().fold(f64::INFINITY, f64::min);
            let slack = (TOLERANCE - worst) / TOLERANCE * 100.0;
            let verdict = if worst <= TOLERANCE {
                if slack < 10.0 {
                    "PASS marginal"
                } else {
                    "PASS"
                }
            } else {
                "fail"
            };
            println!(
                "  {r:>6} {worst:>10.4} {best:>10.4} {:>9.1}% {verdict:>9}",
                slack
            );
        }
        println!();
    }

    println!("-- How to read this --");
    println!("   'slack' is how far the WORST seed sits below tolerance, as a");
    println!("   percentage of tolerance. A saturation call with <10% slack is");
    println!("   one unlucky seed away from moving a whole round, which moves");
    println!("   the product by 20-33%.");
    println!();
    println!("   If N=3's call at 5 rounds or N=5's at 3 rounds is marginal, or");
    println!("   if either moves on the two unseen seeds, THE 180-vs-192 RESULT");
    println!("   IS QUANTISATION NOISE AND GETS WITHDRAWN, not promoted.");
}
