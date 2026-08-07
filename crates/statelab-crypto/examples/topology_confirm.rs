//! Multi-seed confirmation of the best wiring the search found — item (10).
//!
//! ```text
//! cargo run -p statelab-crypto --release --example topology_confirm
//! ```
//!
//! `topology_search` reported a best candidate at `max_dev 0.1517` against
//! ChaCha's `0.2024` at 3 rounds, from **one seed**. The search only re-checks
//! candidates that CROSS the threshold, and none did, so the near-misses left
//! the run unconfirmed. Item (10): single-seed is a standing default violation,
//! not a hardening step. A 25% improvement over ChaCha measured once is a
//! number, not a finding.
//!
//! This driver answers three questions the search could not:
//!
//! 1. **Does 0.1517 hold across disjoint seeds, or was it a fluctuation?**
//! 2. **Does the candidate beat ChaCha ON THE SAME SEEDS**, paired rather than
//!    compared against a single reference number?
//! 3. **What is its actual rounds-to-avalanche?** Better at 3 rounds is
//!    worthless if it is worse at 4 — the round count is what the goal is
//!    about, not the deviation at one round.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples, rounds_to_avalanche};
use statelab_crypto::topology::{chacha_topology, Partition, Topology, TopologyPermutation, LANES};

const TOLERANCE: f64 = 0.12;
const SEEDS: [u64; 8] = [1, 2, 3, 4, 5, 12345, 0xDEAD_BEEF, 1 << 63];

/// The best wiring from `topology_search -- 1200 1`, transcribed from its
/// output. Diameter 3.
fn best_found() -> Topology {
    let a: Partition = [[10, 4, 1, 12], [7, 0, 11, 14], [15, 2, 6, 9], [8, 5, 13, 3]];
    let b: Partition = [[11, 12, 0, 2], [10, 15, 1, 3], [7, 5, 4, 6], [8, 14, 9, 13]];
    Topology { partitions: [a, b] }
}

fn dev_at(t: &Topology, rounds: usize, samples: usize, seed: u64) -> f64 {
    let p = TopologyPermutation {
        topology: t.clone(),
    };
    avalanche_matrix(&p, rounds, samples, seed).max_deviation()
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn main() {
    let samples = recommended_samples(LANES * 32, TOLERANCE);
    let cand = best_found();
    let cc = chacha_topology();

    println!("MULTI-SEED CONFIRMATION OF THE BEST WIRING — item (10)\n");
    println!(
        "  candidate diameter {:?}, chacha diameter {:?}",
        cand.diameter(),
        cc.diameter()
    );
    println!("  samples {samples}, tolerance {TOLERANCE}\n");

    println!("-- max_dev at 3 rounds, PAIRED on identical seeds --");
    println!(
        "  {:>22}  {:>10}  {:>10}  {:>10}",
        "seed", "candidate", "chacha", "delta"
    );
    let (mut cand_devs, mut cc_devs) = (Vec::new(), Vec::new());
    let mut candidate_wins = 0usize;
    for s in SEEDS {
        let a = dev_at(&cand, 3, samples, s);
        let b = dev_at(&cc, 3, samples, s);
        if a < b {
            candidate_wins += 1;
        }
        println!("  {s:>22}  {a:>10.4}  {b:>10.4}  {:>+10.4}", a - b);
        cand_devs.push(a);
        cc_devs.push(b);
    }
    println!(
        "\n  candidate mean {:.4}   chacha mean {:.4}   candidate lower on {candidate_wins}/{} seeds",
        mean(&cand_devs),
        mean(&cc_devs),
        SEEDS.len()
    );
    println!(
        "  candidate range [{:.4} .. {:.4}]",
        cand_devs.iter().cloned().fold(f64::INFINITY, f64::min),
        cand_devs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!(
        "  chacha    range [{:.4} .. {:.4}]",
        cc_devs.iter().cloned().fold(f64::INFINITY, f64::min),
        cc_devs.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!("\n  If the ranges OVERLAP, the single-seed 0.1517 was a fluctuation and");
    println!("  the 'improvement' is noise. Non-overlapping ranges across 8 disjoint");
    println!("  seeds is what a real per-round advantage looks like — the same bar");
    println!("  wide-cross had to clear in TASK_1.\n");

    // ------------------------------------------------------ the real question
    println!("-- rounds-to-avalanche, the quantity the GOAL is about --");
    println!("  Lower max_dev at 3 rounds is worthless if the round count is not");
    println!("  lower. This is the number that would matter.\n");
    for s in [1u64, 2, 3, 4, 5] {
        let c = rounds_to_avalanche(
            &TopologyPermutation {
                topology: cand.clone(),
            },
            12,
            samples,
            TOLERANCE,
            s,
        );
        let r = rounds_to_avalanche(
            &TopologyPermutation {
                topology: cc.clone(),
            },
            12,
            samples,
            TOLERANCE,
            s,
        );
        println!(
            "  seed {s}:  candidate -> {:?}   chacha -> {:?}",
            c.rounds_to_avalanche, r.rounds_to_avalanche
        );
    }

    println!("\n  A candidate matching chacha at 4 rounds is NOT an improvement, however");
    println!("  much better its 3-round deviation looks. TASK_1's wide-cross cleared");
    println!("  the round count and still died on its dose-response curve.");
    println!("  And per PHASE_L §4 / item (16), avalanche is a PROXY. Nothing here is");
    println!("  a security claim; CLAASP has not been run on anything.");
}
