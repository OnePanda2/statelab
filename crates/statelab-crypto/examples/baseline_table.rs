//! The Gate 1 deliverable: every portable permutation under one identical
//! battery, on one machine, from one seed.
//!
//! Run:  cargo run -p statelab-crypto --release --example baseline_table
//!
//! Phase 0 concluded that the field has the constructions and lacks the
//! comparison. This is the comparison, for the designs this hardware can run.
//! AES-round-based designs are absent because the CPU has no AES-NI; that
//! omission is a property of the machine, not of the table.

use statelab_crypto::avalanche::{
    avalanche_matrix, noise_floor, recommended_samples, rounds_to_avalanche,
};
use statelab_crypto::bench::{measure, CpuFeatures};
use statelab_crypto::render::matrix_to_png;
use statelab_crypto::structural::{bijectivity, cycle_spectrum};
use statelab_crypto::systems::{Counter, KlimovShamir, Lcg};
use statelab_crypto::{permutation_by_name, SmallMap, PERMUTATIONS};
use std::hint::black_box;

const SEED: u64 = 0x51A7E1AB;
const TOLERANCE: f64 = 0.12;
const MAX_ROUNDS: usize = 24;
const NOMINAL_GHZ: f64 = 2.667;

fn main() {
    let cpu = CpuFeatures::detect();
    println!("=== StateLab baseline table (Gate 1 deliverable) ===\n");
    println!("   cpu features : {}", cpu.summary());
    println!("   seed         : {SEED:#x}");
    println!("   tolerance    : {TOLERANCE}");
    println!("   sweep        : 1..={MAX_ROUNDS} rounds");
    if !cpu.can_measure_aes_designs() {
        println!("\n   AES-NI absent: Randen, AEGIS and Rocca are omitted. Their absence");
        println!("   from this table is a hardware limitation, not a judgement.");
    }

    // ---- Structural, exhaustive at narrow widths --------------------------
    println!("\n-- Structural (exhaustive over the whole state space) --");
    println!(
        "   {:<18} {:>6} {:>10} {:>8} {:>12}",
        "map", "bits", "bijection", "cycles", "longest"
    );
    let small: Vec<&dyn SmallMap> = vec![
        &Counter { bytes: 8 },
        &Lcg { bytes: 8 },
        &KlimovShamir { bytes: 8 },
    ];
    for map in &small {
        for bits in [12u32, 16] {
            let b = bijectivity(*map, bits);
            if b.is_bijection {
                let s = cycle_spectrum(*map, bits);
                println!(
                    "   {:<18} {:>6} {:>10} {:>8} {:>12}",
                    SmallMap::name(*map),
                    bits,
                    "yes",
                    s.count,
                    s.longest
                );
            } else {
                println!(
                    "   {:<18} {:>6} {:>10} {:>8} {:>12}",
                    SmallMap::name(*map),
                    bits,
                    "NO",
                    "-",
                    "-"
                );
            }
        }
    }

    // ---- Diffusion + cost, one table --------------------------------------
    let samples = recommended_samples(512, TOLERANCE);
    println!("\n-- Diffusion and cost --");
    println!(
        "   samples={samples}  noise_floor={:.4} (<= tolerance, so results are meaningful)\n",
        noise_floor(samples, 512 * 512)
    );
    println!(
        "   {:<26} {:>6} {:>9} {:>10} {:>11} {:>12} {:>11}",
        "permutation", "state", "rounds", "dead", "mean dev", "cyc/B/round", "total cyc/B"
    );

    let mut rows = Vec::new();
    for name in PERMUTATIONS {
        let perm = permutation_by_name(name).expect("registered");
        let bits = perm.state_bytes() * 8;
        let per_sample = recommended_samples(bits, TOLERANCE);

        let sweep = rounds_to_avalanche(perm.as_ref(), MAX_ROUNDS, per_sample, TOLERANCE, SEED);
        let reached = sweep.rounds_to_avalanche;
        let at = reached.unwrap_or(MAX_ROUNDS);
        let (_, _max_d, mean_d, dead) = sweep.per_round[at - 1];

        // Cost of exactly one round, per byte of state.
        let mut state = vec![0u8; perm.state_bytes()];
        let t = measure(*name, perm.state_bytes(), 100_000, 7, || {
            perm.round(black_box(&mut state), 0);
        });
        let per_round = t.cycles_per_byte_from_wall(NOMINAL_GHZ);

        let total = match reached {
            Some(r) => format!("{:.2}", per_round * r as f64),
            None => "unbounded".to_string(),
        };
        println!(
            "   {:<26} {:>6} {:>9} {:>10.4} {:>11.4} {:>12.3} {:>11}",
            name,
            perm.state_bytes(),
            match reached {
                Some(r) => r.to_string(),
                None => format!(">{MAX_ROUNDS}"),
            },
            dead,
            mean_d,
            per_round,
            total
        );

        let m = avalanche_matrix(perm.as_ref(), at, per_sample, SEED);
        std::fs::write(format!("target/baseline-{name}.png"), matrix_to_png(&m, 1))
            .expect("write png");
        rows.push((name.to_string(), reached, per_round));
    }

    // ---- The point of the table -------------------------------------------
    println!("\n-- Reading the table --");
    println!("   'rounds' is rounds to full avalanche, not the shipped round count.");
    println!("   'total' = cyc/B/round x rounds-to-avalanche. A design that never");
    println!("   reaches avalanche has unbounded total cost however cheap its round is.");
    println!("\n   Cheapest round is not the best design:");
    let mut by_round = rows.clone();
    by_round.sort_by(|a, b| a.2.partial_cmp(&b.2).expect("no NaN"));
    for (name, reached, per_round) in by_round.iter().take(3) {
        println!(
            "     {:<26} {:>7.3} cyc/B/round   avalanche: {}",
            name,
            per_round,
            match reached {
                Some(r) => format!("{r} rounds"),
                None => "never".to_string(),
            }
        );
    }
    println!("\n   Matrices written to target/baseline-<name>.png");
}
