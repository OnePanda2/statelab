//! DIRECTED SEARCH — hill climbing on the one validated gradient.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example topology_climb -- [evals] [seed]
//! ```
//!
//! ## Why this can be built now and could not be before
//!
//! `PHASE_M` §5B established that the landscape is not flat: the best wiring
//! random search found beat ChaCha's 3-round `max_dev` on 8 of 8 disjoint seeds
//! with non-overlapping ranges. That is a real, seed-stable ordering — the
//! precondition for climbing.
//!
//! §5C then ruled diameter out as an objective: it is order-blind, and it is a
//! constraint near the connectivity floor and noise above it. So the objective
//! here is 3-round `max_dev`, the only fitness this project has validated.
//!
//! ## *** THE NUMBER THAT DICTATES THE DESIGN ***
//!
//! §5B measured the best candidate's seed-to-seed range as
//! **[0.1517 .. 0.1863] — a spread of 0.035.** The distance from 0.1517 to the
//! 0.12 tolerance is **0.032**.
//!
//! **THE MEASUREMENT NOISE IS LARGER THAN THE REMAINING DISTANCE.** A climber
//! scoring candidates on a single seed would spend most of its budget selecting
//! for that seed's fluctuations and would return a wiring that looks good on
//! seed 1 and is not better at all. This is the classic way an evolutionary
//! search optimises its own measurement instead of its object.
//!
//! Two consequences, both structural rather than cautionary notes:
//!
//! 1. **Fitness is the mean over `FITNESS_SEEDS`**, not one seed. Costs 3x per
//!    evaluation and is the only thing that makes the ranking mean anything.
//! 2. **`CONFIRM_SEEDS` is disjoint from `FITNESS_SEEDS`** — item (11). A result
//!    confirmed on the seeds it was selected on is not confirmed. Whatever this
//!    returns is re-measured on seeds it has never been scored against.
//!
//! ## What a win would be, and what it would not be
//!
//! Reaching `max_dev <= 0.12` at 3 rounds means `rounds_to_avalanche == 3`,
//! against ChaCha's 4, at identical cost — a genuine 1.33x. That is the target.
//!
//! It would still not be a result. Per `PHASE_L` §4 and item (16), avalanche is
//! a proxy; a wiring that clears it is a candidate for CLAASP, not a claim. And
//! `wide-cross` cleared exactly this bar in TASK_1 and then died on its
//! dose-response curve.

use statelab_crypto::avalanche::{
    avalanche_matrix, recommended_samples, rounds_to_avalanche, Probe,
};
use statelab_crypto::topology::{
    chacha_topology, random_topology, Partition, Topology, TopologyPermutation, ARITY,
    GROUPS_PER_ROUND, LANES, PARTITIONS,
};

const TOLERANCE: f64 = 0.12;
const ROUNDS: usize = 3;
/// Seeds the climber scores against.
const FITNESS_SEEDS: [u64; 3] = [1, 2, 3];
/// Item (11): DISJOINT from the above. Never seen during selection.
const CONFIRM_SEEDS: [u64; 5] = [101, 202, 12345, 0xDEAD_BEEF, 1 << 63];

fn dev(t: &Topology, rounds: usize, samples: usize, seed: u64) -> f64 {
    let p = TopologyPermutation {
        topology: t.clone(),
    };
    avalanche_matrix(&p, rounds, samples, seed).max_deviation()
}

/// Mean 3-round max_dev over the fitness seeds. Lower is better.
fn fitness(t: &Topology, samples: usize) -> f64 {
    FITNESS_SEEDS
        .iter()
        .map(|&s| dev(t, ROUNDS, samples, s))
        .sum::<f64>()
        / FITNESS_SEEDS.len() as f64
}

/// The best wiring random search found (`PHASE_M` §5A), the climb's start point.
fn best_random() -> Topology {
    let a: Partition = [[10, 4, 1, 12], [7, 0, 11, 14], [15, 2, 6, 9], [8, 5, 13, 3]];
    let b: Partition = [[11, 12, 0, 2], [10, 15, 1, 3], [7, 5, 4, 6], [8, 14, 9, 13]];
    Topology { partitions: [a, b] }
}

/// Minimal local move: swap one lane between two groups of one partition.
/// Preserves the partition property by construction, so every neighbour is
/// legal and no rejection sampling is needed.
fn mutate(t: &Topology, probe: &mut Probe) -> Topology {
    let mut out = t.clone();
    let p = (probe.next_u64() % PARTITIONS as u64) as usize;
    let g1 = (probe.next_u64() % GROUPS_PER_ROUND as u64) as usize;
    let mut g2 = (probe.next_u64() % GROUPS_PER_ROUND as u64) as usize;
    while g2 == g1 {
        g2 = (probe.next_u64() % GROUPS_PER_ROUND as u64) as usize;
    }
    let i = (probe.next_u64() % ARITY as u64) as usize;
    let j = (probe.next_u64() % ARITY as u64) as usize;
    let tmp = out.partitions[p][g1][i];
    out.partitions[p][g1][i] = out.partitions[p][g2][j];
    out.partitions[p][g2][j] = tmp;
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let budget: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(300);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(7);
    let samples = recommended_samples(LANES * 32, TOLERANCE);

    println!("DIRECTED SEARCH — hill climbing 3-round max_dev\n");
    println!("  objective     minimise mean max_dev at {ROUNDS} rounds");
    println!(
        "  fitness seeds {FITNESS_SEEDS:?}  (mean of {} — noise > remaining distance)",
        FITNESS_SEEDS.len()
    );
    println!("  confirm seeds {CONFIRM_SEEDS:?}");
    println!("                *** DISJOINT from fitness seeds — item (11) ***");
    println!("  target        {TOLERANCE:.4}  (= rounds_to_avalanche 3, vs chacha's 4)");
    println!("  budget        {budget} evaluations, mutation seed {seed}");
    println!("  samples       {samples}\n");

    let cc = chacha_topology();
    let cc_fit = fitness(&cc, samples);
    println!("-- Baselines on the fitness seeds --");
    println!("   chacha                 {cc_fit:.4}");

    let mut probe = Probe::new(seed);
    let mut cur = best_random();
    let mut cur_fit = fitness(&cur, samples);
    println!("   best-random (start)    {cur_fit:.4}");
    println!("   improvement to beat    {:.4}\n", cur_fit - TOLERANCE);

    let start = cur_fit;
    let mut evals = 2usize;
    let mut accepted = 0usize;
    let mut stalls = 0usize;
    let t0 = std::time::Instant::now();

    println!("-- Climb --");
    while evals < budget {
        let cand = mutate(&cur, &mut probe);
        // A mutation can disconnect the graph; such a wiring can never
        // avalanche and is rejected without paying for a measurement.
        if cand.diameter().is_none() {
            continue;
        }
        let f = fitness(&cand, samples);
        evals += 1;
        if f < cur_fit {
            println!(
                "   eval {evals:>4}  ACCEPT  {cur_fit:.4} -> {f:.4}  ({:+.4})  [{:.0}s]",
                f - cur_fit,
                t0.elapsed().as_secs_f64()
            );
            cur = cand;
            cur_fit = f;
            accepted += 1;
            stalls = 0;
        } else {
            stalls += 1;
            // Random restart from a fresh wiring if the local optimum holds.
            if stalls >= 40 {
                let fresh = random_topology(&mut probe);
                if fresh.diameter().is_some() {
                    let ff = fitness(&fresh, samples);
                    evals += 1;
                    if ff < cur_fit {
                        println!("   eval {evals:>4}  RESTART improved {cur_fit:.4} -> {ff:.4}");
                        cur = fresh;
                        cur_fit = ff;
                    }
                }
                stalls = 0;
            }
        }
    }

    println!(
        "\n   {evals} evaluations, {accepted} accepted, {:.0}s",
        t0.elapsed().as_secs_f64()
    );
    println!(
        "   start {start:.4} -> end {cur_fit:.4}   ({:+.4})",
        cur_fit - start
    );

    // ------------------------------------------------- confirmation, item (11)
    println!("\n-- CONFIRMATION on seeds NEVER used for selection — item (11) --");
    println!("   A result confirmed on the seeds it was selected on is not");
    println!("   confirmed. If these numbers are worse than the fitness score, the");
    println!("   climb overfitted and the improvement is partly or wholly noise.\n");
    println!("   {:>22}  {:>10}  {:>10}", "seed", "climbed", "chacha");
    let (mut cw, mut cch) = (Vec::new(), Vec::new());
    let mut wins = 0usize;
    for s in CONFIRM_SEEDS {
        let a = dev(&cur, ROUNDS, samples, s);
        let b = dev(&cc, ROUNDS, samples, s);
        if a < b {
            wins += 1;
        }
        println!("   {s:>22}  {a:>10.4}  {b:>10.4}");
        cw.push(a);
        cch.push(b);
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "\n   climbed mean {:.4}  [{:.4} .. {:.4}]",
        mean(&cw),
        cw.iter().cloned().fold(f64::INFINITY, f64::min),
        cw.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!(
        "   chacha  mean {:.4}  [{:.4} .. {:.4}]",
        mean(&cch),
        cch.iter().cloned().fold(f64::INFINITY, f64::min),
        cch.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!("   climbed lower on {wins}/{}", CONFIRM_SEEDS.len());
    let overfit = mean(&cw) - cur_fit;
    println!("\n   OVERFIT GAP (confirm mean - fitness score): {overfit:+.4}",);
    println!("   Near zero means the climb found real structure. Substantially");
    println!("   positive means it selected for the fitness seeds' noise.");

    // ------------------------------------------------------ the actual target
    println!("\n-- rounds-to-avalanche, the quantity the GOAL is about --");
    for s in [101u64, 202, 12345] {
        let c = rounds_to_avalanche(
            &TopologyPermutation {
                topology: cur.clone(),
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
            "   seed {s}:  climbed -> {:?}   chacha -> {:?}",
            c.rounds_to_avalanche, r.rounds_to_avalanche
        );
    }

    println!("\n-- Best wiring found --");
    println!("   {:?}", cur.partitions[0]);
    println!("   {:?}", cur.partitions[1]);
    println!("   diameter {:?}", cur.diameter());
    println!("\n   Clearing 0.12 would mean rounds_to_avalanche 3 against chacha's 4.");
    println!("   It would still be a CANDIDATE FOR CLAASP, not a result — avalanche");
    println!("   is a proxy (PHASE_L §4, item 16), and wide-cross cleared this exact");
    println!("   bar in TASK_1 before dying on its dose-response curve.");
}
