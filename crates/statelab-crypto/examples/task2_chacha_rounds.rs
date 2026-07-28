//! Task 2 — how far down can ChaCha's round count go on OUR evidence?
//!
//! Run:  cargo run -p statelab-crypto --release --example task2_chacha_rounds
//!
//! PractRand is invoked only if `STATELAB_PRACTRAND` points at `RNG_test.exe`,
//! and `STATELAB_STREAM` at the built `statelab-stream`. Length defaults to 1GB
//! to match the existing baseline table; override with `STATELAB_PR_LEN`.
//!
//! # What is fed to PractRand
//!
//! **Raw permutation state, in counter mode. No extraction.** This must be
//! stated with every result: the same project already demonstrated that a bare
//! counter fails PractRand instantly on raw state and passes a full gigabyte
//! through a strong extractor. A statistical verdict without the input stated
//! is not a verdict.
//!
//! # What this can and cannot decide
//!
//! It can locate where statistical structure disappears. It **cannot** locate
//! where the cipher becomes secure — those are different questions and the
//! second is decided by cryptanalysis, not by any battery. See the closing
//! section of the output.

use statelab_crypto::avalanche::{avalanche_matrix, noise_floor, recommended_samples};
use statelab_crypto::bench::{calibrate_tsc_ghz, measure, CpuFeatures};
use statelab_crypto::systems::ChaCha;
use statelab_crypto::Permutation;
use std::hint::black_box;
use std::process::{Command, Stdio};

const TOLERANCE: f64 = 0.12;
const SEEDS: [u64; 5] = [
    0x51A7E1AB,
    0xBEEF_1234,
    0x0BAD_CAFE,
    0x1357_9BDF,
    0x2468_ACE0,
];
const AMORTISE_OVER: usize = 20;
const TIMING_PASSES: usize = 7;

/// Runs `statelab-stream | RNG_test` and returns the failure count, or `None`
/// when the tools are not configured.
fn practrand(rounds: usize, len: &str) -> Option<usize> {
    let rng_test = std::env::var("STATELAB_PRACTRAND").ok()?;
    let stream = std::env::var("STATELAB_STREAM").ok()?;
    let bytes: u64 = match len {
        "1GB" => 1 << 30,
        "256MB" => 1 << 28,
        "64MB" => 1 << 26,
        _ => 1 << 30,
    };

    let mut gen = Command::new(stream)
        .args([
            "--system",
            "chacha",
            "--rounds",
            &rounds.to_string(),
            "--extract",
            "raw",
            "--bytes",
            &bytes.to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let out = Command::new(rng_test)
        .args(["stdin64", "-tlmax", len, "-tf", "2"])
        .stdin(gen.stdout.take()?)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let _ = gen.wait();

    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| l.contains("FAIL"))
            .count(),
    )
}

fn main() {
    let cpu = CpuFeatures::detect();
    let tsc_ghz = calibrate_tsc_ghz();
    let samples = recommended_samples(512, TOLERANCE);
    let floor = noise_floor(samples, 512 * 512);
    let pr_len = std::env::var("STATELAB_PR_LEN").unwrap_or_else(|_| "1GB".to_string());
    let pr_available = std::env::var("STATELAB_PRACTRAND").is_ok();

    println!("=== Task 2 — ChaCha round reduction on this project's own evidence ===\n");
    println!("   cpu        : {}", cpu.summary());
    println!("   TSC rate   : {tsc_ghz:.3} GHz (measured, not assumed)");
    println!("   tolerance  : {TOLERANCE}   samples: {samples}   noise floor: {floor:.4}");
    println!(
        "   adequacy   : {}",
        if floor <= TOLERANCE {
            "OK"
        } else {
            "INADEQUATE"
        }
    );
    println!(
        "   avalanche  : max deviation, median over {} seeds",
        SEEDS.len()
    );
    println!(
        "   PractRand  : {}",
        if pr_available {
            format!("{pr_len}, RAW STATE (no extraction), counter mode")
        } else {
            "not configured — set STATELAB_PRACTRAND and STATELAB_STREAM".to_string()
        }
    );
    println!();

    // Timing per round, interleaved and amortised.
    let mut ns_samples: Vec<f64> = Vec::new();
    for _ in 0..TIMING_PASSES {
        let mut state = vec![0u8; ChaCha.state_bytes()];
        let t = measure("chacha", ChaCha.state_bytes(), 20_000, 3, || {
            ChaCha.permute(black_box(&mut state), AMORTISE_OVER);
        });
        ns_samples.push(t.ns_per_byte() / AMORTISE_OVER as f64);
    }
    ns_samples.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    let ns_per_round = ns_samples[ns_samples.len() / 2];

    println!(
        "   {:>6} {:>10} {:>10} {:>9} {:>10} {:>12} {:>10}",
        "rounds", "aval med", "aval max", "passes", "adequate", "PractRand", "ns/byte"
    );

    let mut clean_avalanche: Option<usize> = None;
    let mut clean_practrand: Option<usize> = None;

    for rounds in 4..=20usize {
        let mut devs: Vec<f64> = SEEDS
            .iter()
            .map(|s| avalanche_matrix(&ChaCha, rounds, samples, *s).max_deviation())
            .collect();
        let passes = devs.iter().filter(|d| **d <= TOLERANCE).count();
        devs.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        let median = devs[devs.len() / 2];
        let worst = devs[devs.len() - 1];

        if clean_avalanche.is_none() && passes == SEEDS.len() {
            clean_avalanche = Some(rounds);
        }

        let pr = practrand(rounds, &pr_len);
        if clean_practrand.is_none() && pr == Some(0) {
            clean_practrand = Some(rounds);
        }

        println!(
            "   {:>6} {:>10.4} {:>10.4} {:>7}/{} {:>10} {:>12} {:>10.4}",
            rounds,
            median,
            worst,
            passes,
            SEEDS.len(),
            if floor <= TOLERANCE { "yes" } else { "NO" },
            pr.map_or("-".to_string(), |f| if f == 0 {
                "clean".to_string()
            } else {
                format!("{f} fail")
            }),
            ns_per_round * rounds as f64
        );
    }

    // ---- Interpretation ---------------------------------------------------
    println!("\n-- Where the measurements bottom out --");
    println!(
        "   Lowest round count clean on avalanche across all {} seeds : {}",
        SEEDS.len(),
        clean_avalanche.map_or("none".to_string(), |r| r.to_string())
    );
    println!(
        "   Lowest round count clean on PractRand at {pr_len}            : {}",
        clean_practrand.map_or("not measured".to_string(), |r| r.to_string())
    );
    println!("   Per-round cost: {ns_per_round:.4} ns/byte");

    println!("\n-- *** WHAT THIS DOES NOT SAY *** --");
    println!("   Neither number above is a safe round count, and treating either as");
    println!("   one would be the most dangerous error this project could make.");
    println!();
    println!("   Statistical batteries and avalanche both go quiet FAR BELOW the");
    println!("   cryptanalytic frontier. Published differential-linear attacks on");
    println!("   ChaCha reach 7 rounds, with a distinguisher at 7.5 — several rounds");
    println!("   ABOVE where every measurement here is already clean. A cipher that");
    println!("   looks perfect to this pipeline can be broken by an adversary the");
    println!("   pipeline cannot model.");
    println!();
    println!("   So this project's instrument CANNOT answer 'is ChaCha safely");
    println!("   reducible'. It can only say where structure stops being visible to");
    println!("   it. The answer to the safety question comes from cryptanalysis,");
    println!("   which is why ChaCha20 ships 20 rounds against a 7-round frontier,");
    println!("   and why ChaCha12 and ChaCha8 are argued for on cryptanalytic margin");
    println!("   rather than on any battery result.");
}
