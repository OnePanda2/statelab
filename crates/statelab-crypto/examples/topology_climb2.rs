//! DIRECTED SEARCH, second objective — counting offenders instead of maximising.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example topology_climb2 -- [evals] [seed]
//! ```
//!
//! ## Why `topology_climb`'s objective cannot close the last 5%
//!
//! `PHASE_M` §5D left the best wiring at a confirmed 3-round `max_dev` of
//! **0.1263** against a target of **0.1200**. The remaining distance is
//! **0.0063**.
//!
//! §5B measured the single-seed spread of that statistic at ~0.035, so a
//! three-seed mean carries roughly `0.035/sqrt(3)` ≈ **0.020** of noise.
//!
//! **THE NOISE IS THREE TIMES THE REMAINING DISTANCE.** A climber on that
//! objective can no longer tell a real 0.006 improvement from a fluctuation, so
//! more budget buys accepted noise, not progress. Averaging out to ~0.002 would
//! need ~300 seeds per evaluation — about 550 s each. Infeasible, and the wrong
//! fix anyway.
//!
//! ## The objective is the problem, not the budget
//!
//! `max_deviation` is a **maximum over 262,144 cells**. Maxima are inherently
//! high-variance: the statistic is set by whichever single cell fluctuated
//! furthest, so it discards everything the other 262,143 cells know and inherits
//! the variance of an extreme-value distribution.
//!
//! This driver optimises instead the **number of cells exceeding the
//! tolerance** — the offender count. Three properties make it the right choice:
//!
//! 1. **It is a count over the whole matrix**, not an extremum, so its variance
//!    is dramatically lower and it uses all the evidence.
//! 2. **It is smooth.** Removing one offending cell registers, where `max_dev`
//!    only moves if the single worst cell happens to move.
//! 3. **Its zero is exactly the win condition.** Zero cells above tolerance
//!    means `max_dev <= tolerance` means `is_full_avalanche` means
//!    `rounds_to_avalanche == 3`. Optimising it is not optimising a proxy for a
//!    proxy — it is the same predicate, counted rather than thresholded.
//!
//! This is item (5) applied one level deeper than before. The first driver
//! reported a threshold crossing and was corrected to report the continuous
//! quantity. The continuous quantity turned out to be a **maximum**, which is
//! continuous but nearly as information-poor. The fix is a statistic that is
//! continuous *and* pools evidence.
//!
//! ## Discipline, unchanged
//!
//! Fitness seeds {1,2,3}; confirmation on disjoint seeds — item (11). `max_dev`
//! is reported alongside throughout so this run stays comparable with §5D.
//! Cost remains identical to ChaCha by construction. And a wiring that reaches
//! zero offenders is a **candidate for CLAASP**, not a result — `wide-cross`
//! cleared this exact bar in TASK_1 and died on its dose-response curve.

use statelab_crypto::avalanche::{
    avalanche_matrix, recommended_samples, rounds_to_avalanche, AvalancheMatrix, Probe,
};
use statelab_crypto::topology::{
    chacha_topology, Partition, Topology, TopologyPermutation, ARITY, GROUPS_PER_ROUND, LANES,
    PARTITIONS,
};

const TOLERANCE: f64 = 0.12;
const ROUNDS: usize = 3;
const FITNESS_SEEDS: [u64; 3] = [1, 2, 3];
const CONFIRM_SEEDS: [u64; 5] = [101, 202, 12345, 0xDEAD_BEEF, 1 << 63];

/// Cells whose flip probability sits further than `TOLERANCE` from 1/2.
/// **Zero offenders is exactly `is_full_avalanche`.**
fn offenders(m: &AvalancheMatrix) -> usize {
    m.p.iter().filter(|&&p| (p - 0.5).abs() > TOLERANCE).count()
}

fn measure(t: &Topology, samples: usize, seed: u64) -> (usize, f64) {
    let p = TopologyPermutation {
        topology: t.clone(),
    };
    let m = avalanche_matrix(&p, ROUNDS, samples, seed);
    (offenders(&m), m.max_deviation())
}

/// Mean offender count over the fitness seeds. Lower is better; 0 wins.
fn fitness(t: &Topology, samples: usize) -> (f64, f64) {
    let mut off = 0.0;
    let mut dev = 0.0;
    for &s in &FITNESS_SEEDS {
        let (o, d) = measure(t, samples, s);
        off += o as f64;
        dev += d;
    }
    let n = FITNESS_SEEDS.len() as f64;
    (off / n, dev / n)
}

/// The best wiring `topology_climb` produced (`PHASE_M` §5D).
fn best_climbed() -> Topology {
    let a: Partition = [[9, 4, 3, 12], [7, 0, 11, 14], [1, 2, 6, 10], [8, 5, 13, 15]];
    let b: Partition = [[11, 9, 0, 2], [10, 15, 1, 3], [7, 5, 4, 6], [8, 14, 12, 13]];
    Topology { partitions: [a, b] }
}

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
    let budget: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(600);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(21);
    let samples = recommended_samples(LANES * 32, TOLERANCE);
    let cells = LANES * 32 * LANES * 32;

    println!("DIRECTED SEARCH v2 — minimise OFFENDER COUNT, not max_dev\n");
    println!("  objective     cells with |p - 0.5| > {TOLERANCE}, mean over {FITNESS_SEEDS:?}");
    println!("  why           max_dev is a MAXIMUM over {cells} cells: high variance,");
    println!("                and §5D's remaining distance (0.0063) is 3x smaller than");
    println!("                that statistic's noise (~0.020). Counting pools evidence.");
    println!("  win           offenders == 0  <=>  max_dev <= {TOLERANCE}  <=>  3 rounds");
    println!("  confirm       {CONFIRM_SEEDS:?} — DISJOINT, item (11)");
    println!("  budget        {budget} evaluations, mutation seed {seed}\n");

    let cc = chacha_topology();
    let (cc_off, cc_dev) = fitness(&cc, samples);
    let mut cur = best_climbed();
    let (mut cur_off, mut cur_dev) = fitness(&cur, samples);

    println!("-- Baselines on the fitness seeds --");
    println!("   chacha              offenders {cc_off:>9.1}   max_dev {cc_dev:.4}");
    println!("   §5D best (start)    offenders {cur_off:>9.1}   max_dev {cur_dev:.4}");
    println!("   target              offenders {:>9}\n", 0);

    let start_off = cur_off;
    let mut probe = Probe::new(seed);
    let mut evals = 2usize;
    let mut accepted = 0usize;
    let t0 = std::time::Instant::now();

    println!("-- Climb --");
    while evals < budget && cur_off > 0.0 {
        let cand = mutate(&cur, &mut probe);
        if cand.diameter().is_none() {
            continue;
        }
        let (off, dev) = fitness(&cand, samples);
        evals += 1;
        if off < cur_off {
            println!(
                "   eval {evals:>4}  ACCEPT  offenders {cur_off:.1} -> {off:.1}   max_dev {cur_dev:.4} -> {dev:.4}  [{:.0}s]",
                t0.elapsed().as_secs_f64()
            );
            cur = cand;
            cur_off = off;
            cur_dev = dev;
            accepted += 1;
        }
        if evals.is_multiple_of(100) {
            println!(
                "   ...{evals}/{budget}  offenders {cur_off:.1}  max_dev {cur_dev:.4}  [{:.0}s]",
                t0.elapsed().as_secs_f64()
            );
        }
    }

    println!(
        "\n   {evals} evaluations, {accepted} accepted, {:.0}s",
        t0.elapsed().as_secs_f64()
    );
    println!("   offenders {start_off:.1} -> {cur_off:.1}   max_dev now {cur_dev:.4}");

    // ------------------------------------------------- confirmation, item (11)
    println!("\n-- CONFIRMATION on seeds NEVER used for selection — item (11) --");
    println!(
        "   {:>22}  {:>12}  {:>10}  {:>12}",
        "seed", "off (cand)", "max_dev", "off (chacha)"
    );
    let (mut co, mut cd) = (Vec::new(), Vec::new());
    let mut wins = 0usize;
    let mut clean_seeds = 0usize;
    for s in CONFIRM_SEEDS {
        let (o, d) = measure(&cur, samples, s);
        let (o2, _) = measure(&cc, samples, s);
        if o < o2 {
            wins += 1;
        }
        if o == 0 {
            clean_seeds += 1;
        }
        println!("   {s:>22}  {o:>12}  {d:>10.4}  {o2:>12}");
        co.push(o as f64);
        cd.push(d);
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "\n   candidate mean offenders {:.1}   max_dev {:.4}  [{:.4} .. {:.4}]",
        mean(&co),
        mean(&cd),
        cd.iter().cloned().fold(f64::INFINITY, f64::min),
        cd.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!("   lower than chacha on {wins}/{}", CONFIRM_SEEDS.len());
    println!(
        "   OVERFIT GAP (confirm offenders - fitness offenders): {:+.1}",
        mean(&co) - cur_off
    );
    println!(
        "\n   *** SEEDS WITH ZERO OFFENDERS: {clean_seeds}/{} ***",
        CONFIRM_SEEDS.len()
    );
    println!("   A win requires this on EVERY seed, not a majority. §5D's wiring was");
    println!("   clean on 1 of 5 and still read rounds_to_avalanche 4 everywhere.");

    // ------------------------------------------------------ the actual target
    println!("\n-- rounds-to-avalanche, the quantity the GOAL is about --");
    for s in [101u64, 202, 12345, 0xDEAD_BEEF, 1 << 63] {
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
            "   seed {s:>22}:  candidate -> {:?}   chacha -> {:?}",
            c.rounds_to_avalanche, r.rounds_to_avalanche
        );
    }

    println!("\n-- Best wiring --");
    println!("   {:?}", cur.partitions[0]);
    println!("   {:?}", cur.partitions[1]);
    println!("   diameter {:?}", cur.diameter());
    println!("\n   Three rounds on EVERY seed would be a 1.33x round-count reduction at");
    println!("   identical cost — and still a CANDIDATE FOR CLAASP, not a result.");
    println!("   wide-cross cleared this bar in TASK_1 and died on dose-response.");
}
