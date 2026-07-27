//! Phase 1 acceptance run: exercises every battery on every registered
//! permutation and writes the avalanche matrices as PPM images.
//!
//! Run with:  cargo run -p statelab-crypto --release --example phase1_report
//!
//! This is a report driver, not a test. It prints numbers for the record; the
//! assertions that must hold live in the unit tests.

use statelab_crypto::avalanche::{
    avalanche_matrix, noise_floor, recommended_samples, rounds_to_avalanche,
};
use statelab_crypto::render::{matrix_to_png, matrix_to_ppm};
use statelab_crypto::structural::{bijectivity, cycle_spectrum};
use statelab_crypto::systems::{ChaCha, Counter, KlimovShamir, KlimovShamirTransposed};
use statelab_crypto::{Permutation, SmallMap, PERMUTATIONS};

fn main() {
    let seed: u64 = 0x51A7E1AB;
    let tolerance = 0.12;

    println!("=== StateLab Phase 1 — cryptographic instrument acceptance run ===\n");

    // ---- Structural battery (exhaustive, narrow widths) -------------------
    println!("-- Structural: bijectivity and cycle spectrum (exhaustive) --");
    let small: Vec<(&str, &dyn SmallMap)> = vec![
        ("counter", &Counter { bytes: 8 }),
        ("klimov-shamir", &KlimovShamir { bytes: 8 }),
    ];
    for (name, map) in &small {
        for bits in [8u32, 12, 16] {
            let b = bijectivity(*map, bits);
            let line = if b.is_bijection {
                let s = cycle_spectrum(*map, bits);
                format!(
                    "bijection=yes cycles={} longest={} weak_seed_frac={:.4}",
                    s.count,
                    s.longest,
                    s.weak_seed_fraction(1 << (bits / 2))
                )
            } else {
                format!("bijection=NO unreached={}", b.unreached)
            };
            println!("  {name:<26} {bits:>2} bits  {line}");
        }
    }

    // ---- Diffusion battery ------------------------------------------------
    println!("\n-- Diffusion: rounds to full avalanche (tolerance {tolerance}) --");
    let samples = recommended_samples(512, tolerance);
    println!(
        "   samples={samples}  noise_floor={:.4}  (must be <= tolerance)\n",
        noise_floor(samples, 512 * 512)
    );

    let perms: Vec<Box<dyn Permutation>> = vec![
        Box::new(Counter::default()),
        Box::new(ChaCha),
        Box::new(KlimovShamir::default()),
        Box::new(KlimovShamirTransposed::default()),
    ];

    println!(
        "   {:<26} {:>6} {:>10} {:>10} {:>10}",
        "permutation", "rounds", "max_dev", "mean_dev", "dead_pairs"
    );
    const MAX_ROUNDS: usize = 16;

    for p in &perms {
        // The sweep must run at the SAME sample count as the final measurement.
        // Sweeping cheaply to "find the shape" first does not work: at 64
        // samples the noise floor is ~0.31, far above any useful tolerance, so
        // the sweep reports no avalanche for everything including ChaCha. That
        // is the trap `noise_floor` exists to name, and it is easy to walk into
        // twice.
        let sweep = rounds_to_avalanche(p.as_ref(), MAX_ROUNDS, samples, tolerance, seed);
        let reached = sweep.rounds_to_avalanche;
        let target = reached.unwrap_or(MAX_ROUNDS);
        let (_, max_d, mean_d, dead) = sweep.per_round[target - 1];

        println!(
            "   {:<26} {:>6} {:>10.4} {:>10.4} {:>10.4}  {}",
            p.name(),
            match reached {
                Some(r) => r.to_string(),
                None => format!(">{MAX_ROUNDS}"),
            },
            max_d,
            mean_d,
            dead,
            match reached {
                Some(r) => format!("AVALANCHE at round {r}"),
                None => format!("no avalanche within {MAX_ROUNDS} rounds"),
            }
        );

        let m = avalanche_matrix(p.as_ref(), target, samples, seed);
        std::fs::write(
            format!("target/avalanche-{}.ppm", p.name()),
            matrix_to_ppm(&m, 1),
        )
        .expect("write ppm");
        std::fs::write(
            format!("target/avalanche-{}.png", p.name()),
            matrix_to_png(&m, 1),
        )
        .expect("write png");
    }

    println!("\n   Matrices written to target/avalanche-<name>.ppm");
    println!("   Registered permutations: {}", PERMUTATIONS.join(", "));
}
