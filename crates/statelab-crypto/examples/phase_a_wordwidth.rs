//! Phase A — does 64-bit word width pay?
//!
//! Run:  cargo run -p statelab-crypto --release --example phase_a_wordwidth
//!
//! Three designs sharing one quarter-round shape and one round pattern,
//! differing only in word width and rotation constants. If word width does not
//! deliver, the hardware-independent thesis is falsified here and cheaply.

use statelab_crypto::arx64::{BLAKE2B, CHACHA64};
use statelab_crypto::avalanche::{noise_floor, recommended_samples, rounds_to_avalanche};
use statelab_crypto::bench::{calibrate_tsc_ghz, measure, CpuFeatures};
use statelab_crypto::systems::ChaCha;
use statelab_crypto::Permutation;
use std::hint::black_box;

const SEED: u64 = 0x51A7E1AB;
const TOLERANCE: f64 = 0.12;
const MAX_ROUNDS: usize = 12;

/// The kill criterion from the plan: below this, H4 is falsified.
const REQUIRED_SPEEDUP: f64 = 1.4;

fn main() {
    let cpu = CpuFeatures::detect();
    let tsc_ghz = calibrate_tsc_ghz();

    println!("=== Phase A — 64-bit word width ===\n");
    println!("   cpu features : {}", cpu.summary());
    println!("   TSC rate     : {tsc_ghz:.3} GHz (measured)");
    println!("   seed         : {SEED:#x}   tolerance: {TOLERANCE}");
    println!("   sweep        : 1..={MAX_ROUNDS} rounds\n");

    println!("   No instruction newer than SSE2/SSSE3 is assumed anywhere here.");
    println!("   These are scalar Rust; SIMD is a later phase, and would only");
    println!("   widen the gap the word-width argument predicts.\n");

    let designs: Vec<(&dyn Permutation, &str, usize)> = vec![
        (&ChaCha, "32-bit, rot 16/12/8/7", 12),
        (&CHACHA64, "64-bit, rot 32/24/16/14", 12),
        (&BLAKE2B, "64-bit, rot 32/40/48/1", 12),
    ];

    // ---- A3: instruction-count audit, by hand -----------------------------
    println!("-- A3: ops per byte, counted by hand --");
    println!(
        "   {:<12} {:>7} {:>10} {:>12} {:>12}",
        "design", "state", "ops/round", "bytes/round", "ops/byte"
    );
    for (perm, note, ops_per_qr) in &designs {
        // One round = 4 quarter-rounds, each touching 4 words.
        let bytes = perm.state_bytes();
        let ops = 4 * ops_per_qr; // 4 QRs x 12 ops
        println!(
            "   {:<12} {:>7} {:>10} {:>12} {:>12.3}   {}",
            perm.name(),
            bytes,
            ops,
            bytes,
            ops as f64 / bytes as f64,
            note
        );
    }

    // ---- A1/A2: measured diffusion and cost -------------------------------
    println!("\n-- A1/A2: measured --");
    println!(
        "   {:<12} {:>8} {:>10} {:>13} {:>12} {:>11}",
        "design", "rounds", "dead", "ns/B/round", "total ns/B", "vs chacha"
    );

    let mut results = Vec::new();
    for (perm, _, _) in &designs {
        let bits = perm.state_bytes() * 8;
        let samples = recommended_samples(bits, TOLERANCE);

        let sweep = rounds_to_avalanche(*perm, MAX_ROUNDS, samples, TOLERANCE, SEED);
        let reached = sweep.rounds_to_avalanche;
        let at = reached.unwrap_or(MAX_ROUNDS);
        let (_, _max_d, _mean_d, dead) = sweep.per_round[at - 1];

        let mut state = vec![0u8; perm.state_bytes()];
        let t = measure(perm.name(), perm.state_bytes(), 100_000, 7, || {
            perm.round(black_box(&mut state), 0);
        });
        let per_round = t.ns_per_byte();

        results.push((perm.name(), reached, per_round, samples, bits));

        let total = reached.map(|r| per_round * r as f64);
        let base = results[0].2;
        println!(
            "   {:<12} {:>8} {:>10.4} {:>13.4} {:>12} {:>11}",
            perm.name(),
            match reached {
                Some(r) => r.to_string(),
                None => format!(">{MAX_ROUNDS}"),
            },
            dead,
            per_round,
            match total {
                Some(v) => format!("{v:.3}"),
                None => "unbounded".to_string(),
            },
            if base > 0.0 {
                format!("{:.2}x", base / per_round)
            } else {
                "-".to_string()
            }
        );
    }

    println!(
        "\n   sampling: {} samples at 512 bits (noise floor {:.4}), {} at 1024 bits ({:.4})",
        results[0].3,
        noise_floor(results[0].3, 512 * 512),
        results[1].3,
        noise_floor(results[1].3, 1024 * 1024),
    );

    // ---- Harness artefact: isolated rounds punish the wider state ---------
    //
    // Measuring `round()` alone forces a load and store of the entire state on
    // every call. A 128-byte state therefore pays twice ChaCha's memory
    // traffic per round, which a real implementation never does — it keeps the
    // state in registers across all rounds. Amortising over a full permutation
    // is the fair comparison, and the gap between the two numbers is the size
    // of the artefact.
    println!("\n-- Isolated round vs amortised permutation --");
    println!(
        "   {:<12} {:>16} {:>16} {:>12}",
        "design", "isolated ns/B/rd", "amortised ns/B/rd", "vs chacha"
    );
    const AMORTISE_OVER: usize = 20;
    const PASSES: usize = 9;

    // Interleaved, not batched. Measuring all of A then all of B lets thermal
    // drift and scheduler noise land unevenly on one design; cycling A,B,C
    // repeatedly and taking per-design medians cancels it. A first version
    // measured in batches and the verdict flipped between runs.
    let mut samples: Vec<Vec<f64>> = vec![Vec::new(); designs.len()];
    for _ in 0..PASSES {
        for (i, (perm, _, _)) in designs.iter().enumerate() {
            let mut state = vec![0u8; perm.state_bytes()];
            let t = measure(perm.name(), perm.state_bytes(), 20_000, 3, || {
                perm.permute(black_box(&mut state), AMORTISE_OVER);
            });
            samples[i].push(t.ns_per_byte() / AMORTISE_OVER as f64);
        }
    }

    let stat = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        (v[0], v[v.len() / 2], v[v.len() - 1])
    };
    let mut amortised = Vec::new();
    for s in samples.iter_mut() {
        amortised.push(stat(s));
    }

    println!("   ({PASSES} interleaved passes; spread shows the resolution of this machine)");
    for (i, (perm, _, _)) in designs.iter().enumerate() {
        let (lo, med, hi) = amortised[i];
        println!(
            "   {:<12} {:>16.4} {:>16.4} {:>12}   [{:.4}..{:.4}]",
            perm.name(),
            results[i].2,
            med,
            format!("{:.2}x", amortised[0].1 / med),
            lo,
            hi
        );
    }
    println!("\n   ops/byte predicts 2.00x. The amortised median is the honest");
    println!("   comparison; the isolated column understates the wider designs,");
    println!("   which pay double memory traffic when a round is timed alone.");

    // ---- The verdict ------------------------------------------------------
    println!("\n-- Kill criterion --");
    println!("   H4 requires >={REQUIRED_SPEEDUP}x on ns/byte/round AT EQUAL rounds-to-avalanche.");
    println!("   A per-round win bought by needing more rounds is not a win.\n");

    let (_, chacha_rounds, _, _, _) = results[0];
    let chacha_cost = amortised[0].1;
    for (idx, (name, rounds, _, _, _)) in results.iter().enumerate().skip(1) {
        let cost = amortised[idx].1;
        let per_round_gain = chacha_cost / cost;
        let rounds_penalty = match (chacha_rounds, rounds) {
            (Some(a), Some(b)) => *b as f64 / a as f64,
            _ => f64::INFINITY,
        };
        let net = per_round_gain / rounds_penalty;
        let verdict = if !net.is_finite() {
            "FAILS — never reached avalanche".to_string()
        } else if net >= REQUIRED_SPEEDUP {
            format!("PASSES — net {net:.2}x")
        } else {
            format!("FAILS — net {net:.2}x below {REQUIRED_SPEEDUP}x")
        };
        println!(
            "   {:<12} per-round {:.2}x, rounds penalty {:.2}x  ->  {}",
            name, per_round_gain, rounds_penalty, verdict
        );
    }
}
