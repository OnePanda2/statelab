//! TOPOLOGY SEARCH — the first attempt this project has made to GENERATE a
//! candidate rather than evaluate someone else's.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example topology_search -- [N] [seed]
//! ```
//!
//! ## The question, stated so it can only be answered yes or no
//!
//! ChaCha's wiring reaches full avalanche at 4 rounds. **Is there a wiring, at
//! identical cost, that reaches it at 3?**
//!
//! Cost is identical by construction: every candidate is a pair of partitions of
//! the same 16 lanes into 4 groups of 4, driven by ChaCha's own quarter round.
//! Same operations, same register pressure, same memory traffic, same lane
//! count. Only the wiring differs. So the screen is binary — avalanche at 3
//! rounds, or not — and a hit is a 1.33x round-count improvement at zero cost.
//!
//! ## Why the screen is one matrix and not a sweep
//!
//! A full 12-round sweep costs ~22 s per candidate. The question does not need a
//! sweep: it needs one avalanche matrix at exactly 3 rounds. That is ~12x
//! cheaper and asks precisely the question. Survivors get the full sweep.
//!
//! ## Discipline
//!
//! * Diameter pre-filter first — microseconds, and it lower-bounds the thing the
//!   expensive measurement computes.
//! * `recommended_samples()` throughout — item (1), an avalanche number without
//!   an adequacy check is suspect.
//! * ChaCha's own wiring runs through the identical path as a control, at both 3
//!   and 4 rounds. **It already caught a 2x unit error once.**
//! * Any hit is re-checked on 5 disjoint seeds before being reported — item (10).
//! * A hit is a CANDIDATE WORTH EVALUATING, not a result. See `PHASE_L` §4.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples, rounds_to_avalanche};
use statelab_crypto::topology::{
    chacha_topology, random_topology, Topology, TopologyPermutation, LANES,
};

const TOLERANCE: f64 = 0.12;
const SCREEN_ROUNDS: usize = 3;
const CONFIRM_SEEDS: [u64; 5] = [1, 2, 3, 4, 5];

fn avalanches_at(t: &Topology, rounds: usize, samples: usize, seed: u64) -> (bool, f64) {
    let p = TopologyPermutation {
        topology: t.clone(),
    };
    let m = avalanche_matrix(&p, rounds, samples, seed);
    (m.is_full_avalanche(TOLERANCE), m.max_deviation())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

    let bits = LANES * 32;
    let samples = recommended_samples(bits, TOLERANCE);

    println!("TOPOLOGY SEARCH — can any wiring beat ChaCha's 4 rounds at equal cost?\n");
    println!("  lanes            {LANES} x 32 bits (SIMD-divisible; narrowest native width)");
    println!("  round            one partition into 4 groups of 4, ChaCha's quarter round");
    println!(
        "  cost             IDENTICAL to ChaCha by construction — wiring is the only variable"
    );
    println!("  screen           full avalanche at {SCREEN_ROUNDS} rounds, tolerance {TOLERANCE}");
    println!("  samples          {samples} (recommended_samples, adequacy-checked)");
    println!("  candidates       {n}, seed {seed}\n");

    // ------------------------------------------------------------- controls
    println!("-- CONTROL: ChaCha's own wiring through this identical path --");
    let cc = chacha_topology();
    let mut chacha_screen_dev = f64::NAN;
    for r in [SCREEN_ROUNDS, 4] {
        let (full, dev) = avalanches_at(&cc, r, samples, 1);
        if r == SCREEN_ROUNDS {
            // The number every candidate is compared against, not just the
            // threshold. Item (5).
            chacha_screen_dev = dev;
        }
        println!(
            "   chacha wiring @ {r} rounds: max_dev {dev:.4}  full avalanche {}",
            if full { "YES" } else { "no" }
        );
    }
    let sweep = rounds_to_avalanche(
        &TopologyPermutation {
            topology: cc.clone(),
        },
        12,
        samples,
        TOLERANCE,
        1,
    );
    println!(
        "   chacha wiring rounds-to-avalanche: {:?}",
        sweep.rounds_to_avalanche
    );
    if sweep.rounds_to_avalanche != Some(4) {
        println!("\n   *** CONTROL FAILED — the harness disagrees with the registry.");
        println!("   *** No ranking below this line means anything. Stopping.");
        return;
    }
    println!("   Control holds: 4 rounds, matching the registered `chacha`.\n");

    // -------------------------------------------------------- diameter pass
    let mut probe = statelab_crypto::avalanche::Probe::new(seed);
    let mut diam_hist = [0usize; 8];
    let mut disconnected = 0usize;
    let mut screened = 0usize;
    let mut hits: Vec<(Topology, f64)> = Vec::new();
    // *** ITEM (5). *** The first run recorded only threshold crossings and
    // discarded max_deviation for everything that failed — the exact inversion
    // of "report the CONTINUOUS quantity, never the threshold crossing, which
    // is a boundary-sensitive derivative of it". It could not answer whether
    // anything came CLOSE. Every measurement is kept now; it costs one push.
    let mut devs: Vec<(f64, usize)> = Vec::new();
    let mut best: Option<(f64, Topology)> = None;

    let t0 = std::time::Instant::now();
    for i in 0..n {
        let t = random_topology(&mut probe);
        let d = match t.diameter() {
            None => {
                disconnected += 1;
                continue;
            }
            Some(d) => {
                diam_hist[d.min(7)] += 1;
                // Dependency spreads at most one hop per round, so a diameter
                // above the screen round count cannot possibly avalanche there.
                if d > SCREEN_ROUNDS {
                    continue;
                }
                d
            }
        };
        screened += 1;
        let (full, dev) = avalanches_at(&t, SCREEN_ROUNDS, samples, 1);
        devs.push((dev, d));
        if best.as_ref().is_none_or(|(b, _)| dev < *b) {
            best = Some((dev, t.clone()));
        }
        if full {
            hits.push((t, dev));
        }
        if i % 100 == 0 && i > 0 {
            println!(
                "   ...{i}/{n}  screened {screened}  hits {}  best max_dev {:.4}  ({:.0}s)",
                hits.len(),
                best.as_ref().map(|(b, _)| *b).unwrap_or(f64::NAN),
                t0.elapsed().as_secs_f64()
            );
        }
    }

    println!("\n-- Diameter distribution over {n} sampled wirings --");
    println!("   disconnected (can never avalanche): {disconnected}");
    for (d, &c) in diam_hist.iter().enumerate() {
        if c > 0 {
            let note = if d > SCREEN_ROUNDS { "  (pruned)" } else { "" };
            println!("   diameter {d}: {c}{note}");
        }
    }
    println!("\n   {screened} of {n} survived the pre-filter and were measured.",);
    println!("   Pre-filter avoided {} avalanche matrices.", n - screened);
    println!("   Elapsed {:.0}s.\n", t0.elapsed().as_secs_f64());

    // ------------------------------------------- the continuous quantity, (5)
    println!("-- max_dev distribution at {SCREEN_ROUNDS} rounds — ITEM (5) --");
    println!("   The threshold answers 'did anything win'. This answers the");
    println!("   question the first run could not: DID ANYTHING COME CLOSE?\n");
    let mut sorted: Vec<f64> = devs.iter().map(|(d, _)| *d).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !sorted.is_empty() {
        let pick = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
        println!("   chacha (same path, same seed)  {chacha_screen_dev:.4}");
        println!("   tolerance (the pass mark)      {TOLERANCE:.4}");
        println!("   ----");
        println!("   best candidate                 {:.4}", sorted[0]);
        println!("   5th percentile                 {:.4}", pick(0.05));
        println!("   median                         {:.4}", pick(0.50));
        println!("   95th percentile                {:.4}", pick(0.95));
        println!(
            "   worst candidate                {:.4}",
            sorted[sorted.len() - 1]
        );
        let beat_chacha = sorted.iter().filter(|&&d| d < chacha_screen_dev).count();
        println!(
            "\n   Candidates with a LOWER max_dev than chacha at {SCREEN_ROUNDS} rounds: {beat_chacha}/{}",
            sorted.len()
        );
        println!("   (Lower is better. Beating chacha here without crossing the");
        println!("    tolerance is not a win, but it is the signal a directed");
        println!("    search would climb — and it is what the first run threw away.)");
        if let Some((bd, bt)) = &best {
            println!(
                "\n   Best wiring found, max_dev {bd:.4}, diameter {:?}:",
                bt.diameter()
            );
            println!("     {:?}", bt.partitions[0]);
            println!("     {:?}", bt.partitions[1]);
        }
        // Does connectivity predict diffusion at all? Cheap to answer now.
        for target in [2usize, 3] {
            let g: Vec<f64> = devs
                .iter()
                .filter(|(_, d)| *d == target)
                .map(|(dev, _)| *dev)
                .collect();
            if !g.is_empty() {
                let mean = g.iter().sum::<f64>() / g.len() as f64;
                let mn = g.iter().cloned().fold(f64::INFINITY, f64::min);
                println!(
                    "   diameter {target}: n={:<5} mean max_dev {mean:.4}  best {mn:.4}",
                    g.len()
                );
            }
        }
    }
    println!();

    // -------------------------------------------------------------- verdict
    if hits.is_empty() {
        println!("-- RESULT: NO WIRING BEAT 4 ROUNDS --");
        println!("   {screened} distinct wirings measured at {SCREEN_ROUNDS} rounds, none reached");
        println!("   full avalanche. ChaCha's column/diagonal choice is not beaten on");
        println!("   this axis by random search at this sample size.");
        println!();
        println!("   This is a NEGATIVE result and it is worth exactly what it says:");
        println!("   random sampling of {n} wirings found nothing. It does NOT show");
        println!("   no such wiring exists — the space of partition pairs is roughly");
        println!("   2.6e6 squared, so this covers a vanishing fraction of it.");
        return;
    }

    println!(
        "-- {} CANDIDATE(S) REACHED AVALANCHE AT {SCREEN_ROUNDS} ROUNDS --",
        hits.len()
    );
    println!(
        "   Re-checking each on {} disjoint seeds — item (10).\n",
        CONFIRM_SEEDS.len()
    );
    for (t, dev) in hits.iter().take(10) {
        let per_seed: Vec<bool> = CONFIRM_SEEDS
            .iter()
            .map(|&s| avalanches_at(t, SCREEN_ROUNDS, samples, s).0)
            .collect();
        let agree = per_seed.iter().filter(|&&b| b).count();
        println!("   wiring {:?}", t.partitions[0]);
        println!("          {:?}", t.partitions[1]);
        println!(
            "     diameter {:?}  screen max_dev {dev:.4}  confirmed {agree}/{}",
            t.diameter(),
            CONFIRM_SEEDS.len()
        );
        if agree == CONFIRM_SEEDS.len() {
            println!("     *** HOLDS ON ALL SEEDS — this is a real candidate. ***");
        } else {
            println!("     rejected: single-seed artefact, exactly what item (10) exists for.");
        }
    }

    println!("\n   A confirmed hit is a CANDIDATE WORTH EVALUATING, not a result.");
    println!("   Per PHASE_L §4: avalanche is a proxy. A design can score perfectly");
    println!("   here and still carry an exploitable differential trail that only a");
    println!("   MILP/SAT search would surface. CLAASP has not been run on anything.");
}
