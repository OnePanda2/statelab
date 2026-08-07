//! TWO-ROUND SCREEN — and a correction to the metric before it is run.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example topology_screen2 -- [N] [seed]
//! ```
//!
//! ## Why this is NOT `topology_search` with SCREEN_ROUNDS = 2
//!
//! ChaCha at 2 rounds reads **`max_dev 0.5000`, `dead 0.0440`**. 0.5 is the
//! *saturated* value — a cell whose bit never flips or always flips. At 2 rounds
//! essentially every wiring is pinned there, so **`max_dev` is a ceiling that
//! cannot discriminate**, and a search ranking on it would see a flat landscape
//! and learn nothing. That is the same error as ranking on a threshold crossing
//! (item 5), one level down: the chosen statistic has no resolution in the
//! regime being measured.
//!
//! The quantity with resolution at 2 rounds is the **dead-pair fraction** — how
//! much of the dependency matrix is still completely unconnected. ChaCha's is
//! 0.0440. That is what this driver ranks on.
//!
//! ## The prediction, stated before running
//!
//! **No wiring will reach full avalanche at 2 rounds.** Two rounds is exactly
//! the lane-level fan-out limit: one round couples a lane to the 3 others in its
//! group (4 lanes), two rounds couples those 4 groups (up to 16). So a
//! diameter-2 wiring can just barely achieve full *dependency* in 2 rounds — and
//! full dependency is not avalanche. ChaCha is diameter 2 and still carries 4.4%
//! dead pairs at 2 rounds, because information being *able* to flow is not the
//! same as it flowing with probability 1/2. §3 of the 3-round run already showed
//! connectivity is necessary and nowhere near sufficient.
//!
//! Recording the prediction so the result cannot be reinterpreted afterwards.
//!
//! ## The question actually worth asking at 2 rounds
//!
//! **Does diameter 2 exactly characterise the zero-dead-pair set?** If yes, the
//! cheap graph property fully predicts 2-round connectivity and the expensive
//! measurement is redundant there. If no, the proxy is incomplete in a way worth
//! knowing before any directed search leans on it.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples, Probe};
use statelab_crypto::topology::{
    chacha_topology, random_topology, Topology, TopologyPermutation, LANES,
};

const TOLERANCE: f64 = 0.12;
const ROUNDS: usize = 2;

/// `(full avalanche, max_dev, dead fraction)`.
fn measure(t: &Topology, rounds: usize, samples: usize, seed: u64) -> (bool, f64, f64) {
    let p = TopologyPermutation {
        topology: t.clone(),
    };
    let m = avalanche_matrix(&p, rounds, samples, seed);
    (
        m.is_full_avalanche(TOLERANCE),
        m.max_deviation(),
        m.dead_pair_fraction(),
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1200);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let samples = recommended_samples(LANES * 32, TOLERANCE);

    println!("TWO-ROUND SCREEN — ranked on DEAD-PAIR FRACTION, not max_dev\n");
    println!("  PREDICTION, recorded before the run: no wiring reaches full");
    println!("  avalanche at 2 rounds. Two rounds is the lane fan-out limit");
    println!("  (4 -> 16) and full dependency is not avalanche.\n");
    println!("  candidates {n}, seed {seed}, samples {samples}\n");

    // ------------------------------------------------------------- control
    println!("-- CONTROL: chacha's wiring, per round --");
    let cc = chacha_topology();
    for r in 1..=4 {
        let (full, dev, dead) = measure(&cc, r, samples, 1);
        println!(
            "   r{r}  max_dev {dev:.4}  dead {dead:.4}  full avalanche {}",
            if full { "YES" } else { "no" }
        );
    }
    let (_, cc_dev, cc_dead) = measure(&cc, ROUNDS, samples, 1);
    println!("\n   Note max_dev is PINNED at 0.5000 for r1 and r2 — no resolution.");
    println!("   dead falls 0.8141 -> 0.0440 across the same rounds. That is the");
    println!("   statistic with signal here, and chacha's r2 value is {cc_dead:.4}.\n");

    // -------------------------------------------------------------- screen
    let mut probe = Probe::new(seed);
    let mut rows: Vec<(f64, f64, usize)> = Vec::new(); // (dead, max_dev, diameter)
    let mut zero_dead_by_diameter = [0usize; 8];
    let mut count_by_diameter = [0usize; 8];
    let mut hits = 0usize;
    let mut best: Option<(f64, Topology, usize)> = None;
    let t0 = std::time::Instant::now();

    for i in 0..n {
        let t = random_topology(&mut probe);
        let Some(d) = t.diameter() else { continue };
        // No pruning by diameter here: the whole question is how diameter
        // relates to the measured dead fraction, so filtering on it first would
        // assume the answer.
        count_by_diameter[d.min(7)] += 1;
        let (full, dev, dead) = measure(&t, ROUNDS, samples, 1);
        if full {
            hits += 1;
        }
        if dead == 0.0 {
            zero_dead_by_diameter[d.min(7)] += 1;
        }
        if best.as_ref().is_none_or(|(b, _, _)| dead < *b) {
            best = Some((dead, t.clone(), d));
        }
        rows.push((dead, dev, d));
        if i % 100 == 0 && i > 0 {
            println!(
                "   ...{i}/{n}  hits {hits}  best dead {:.4}  ({:.0}s)",
                best.as_ref().map(|(b, _, _)| *b).unwrap_or(f64::NAN),
                t0.elapsed().as_secs_f64()
            );
        }
    }

    // ------------------------------------------------------------- results
    println!(
        "\n-- Dead-pair fraction at {ROUNDS} rounds, {} wirings, {:.0}s --",
        rows.len(),
        t0.elapsed().as_secs_f64()
    );
    let mut dead: Vec<f64> = rows.iter().map(|(d, _, _)| *d).collect();
    dead.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !dead.is_empty() {
        let pick = |q: f64| dead[((dead.len() - 1) as f64 * q).round() as usize];
        println!("   chacha (same path)   {cc_dead:.4}   [max_dev {cc_dev:.4}]");
        println!("   ----");
        println!("   best (lowest dead)   {:.4}", dead[0]);
        println!("   5th percentile       {:.4}", pick(0.05));
        println!("   median               {:.4}", pick(0.50));
        println!("   95th percentile      {:.4}", pick(0.95));
        println!("   worst                {:.4}", dead[dead.len() - 1]);
        let better = dead.iter().filter(|&&d| d < cc_dead).count();
        println!(
            "\n   Wirings with FEWER dead pairs than chacha: {better}/{}",
            dead.len()
        );
    }

    println!("\n-- Does diameter 2 characterise the zero-dead set? --");
    for d in 0..8 {
        if count_by_diameter[d] > 0 {
            println!(
                "   diameter {d}: {:>5} wirings, {:>5} with zero dead pairs",
                count_by_diameter[d], zero_dead_by_diameter[d]
            );
        }
    }
    println!("\n   If every diameter-2 wiring has zero dead pairs and no other");
    println!("   diameter does, the graph property fully predicts 2-round");
    println!("   connectivity and measuring it there is redundant. Any mismatch");
    println!("   means the cheap proxy is incomplete — worth knowing before a");
    println!("   directed search leans on it.");

    println!("\n-- VERDICT --");
    if hits == 0 {
        println!("   NO WIRING REACHED FULL AVALANCHE AT {ROUNDS} ROUNDS. Prediction held.");
        println!("   A 2x round-count improvement is not available from wiring alone,");
        println!("   which is what the lane fan-out argument said before the run.");
    } else {
        println!("   *** {hits} WIRING(S) REACHED FULL AVALANCHE AT {ROUNDS} ROUNDS. ***");
        println!("   THE PREDICTION WAS WRONG. That is the interesting outcome and it");
        println!("   needs multi-seed confirmation before it is believed at all —");
        println!("   see topology_confirm.rs for the bar it has to clear.");
    }
    if let Some((bd, bt, bdia)) = &best {
        println!("\n   Best wiring by dead fraction {bd:.4}, diameter {bdia}:");
        println!("     {:?}", bt.partitions[0]);
        println!("     {:?}", bt.partitions[1]);
    }
    println!("\n   Avalanche is a PROXY (PHASE_L §4, item 16). Nothing here is a");
    println!("   security claim and CLAASP has not been run on anything.");
}
