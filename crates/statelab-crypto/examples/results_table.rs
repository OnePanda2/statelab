//! The consolidated results table.
//!
//! Run:  cargo run -p statelab-crypto --release --example results_table
//!
//! Every registered permutation through every instrument, in one place, from
//! one run. Extends the original baseline-table columns
//! (permutation / rounds / dead pairs / PractRand) with **ns/byte**, an
//! explicit **sampling-adequacy** column, and a **robustness** column that did
//! not exist before this session.
//!
//! PractRand runs only when `STATELAB_PRACTRAND` and `STATELAB_STREAM` are set.
//!
//! # Two input constructions, deliberately
//!
//! The robustness column exists because a statistical verdict turned out to
//! depend on how the state is initialised, not only on what is extracted from
//! it. `wide-cross` failed 7 tests at 3 rounds on a zero-filled state and 0 on
//! a keyed one; ChaCha failed 1 and 0. Reporting only one construction hid a
//! real difference and manufactured a fake one.
//!
//!   * **keyed** — the state is filled with key material. The realistic case.
//!   * **zero-filled** — `seed ‖ counter ‖ zeros`. Low entropy and highly
//!     structured; the adversarial case, and where weak-key behaviour shows.
//!
//! The avalanche battery is unaffected by either: it draws fully random base
//! states from its own probe, so those columns never depended on this choice.

use statelab_crypto::avalanche::{avalanche_matrix, noise_floor, recommended_samples};
use statelab_crypto::bench::{calibrate_tsc_ghz, measure, CpuFeatures};
use statelab_crypto::{permutation_by_name, PERMUTATIONS};
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
const MAX_ROUNDS: usize = 12;
const AMORTISE_OVER: usize = 20;
const TIMING_PASSES: usize = 7;
/// Fixed round count for the robustness pair, so rows are comparable.
const ROBUSTNESS_ROUNDS: usize = 3;

fn practrand(system: &str, rounds: usize, keyed: bool, len: &str, bytes: u64) -> Option<usize> {
    let rng_test = std::env::var("STATELAB_PRACTRAND").ok()?;
    let stream = std::env::var("STATELAB_STREAM").ok()?;

    let mut args: Vec<String> = vec![
        "--system".into(),
        system.into(),
        "--rounds".into(),
        rounds.to_string(),
        "--extract".into(),
        "raw".into(),
        "--bytes".into(),
        bytes.to_string(),
    ];
    if keyed {
        args.push("--keyed".into());
    }

    let mut gen = Command::new(stream)
        .args(&args)
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
    let pr = std::env::var("STATELAB_PRACTRAND").is_ok();

    println!("=== StateLab consolidated results table ===\n");
    println!("   cpu       : {}", cpu.summary());
    println!("   TSC rate  : {tsc_ghz:.3} GHz (measured)");
    println!(
        "   tolerance : {TOLERANCE}   avalanche over {} seeds",
        SEEDS.len()
    );
    println!("   avalanche : random base states (never zero-filled)");
    println!(
        "   PractRand : {}",
        if pr {
            "raw state, no extraction; 1GB keyed, 256MB for the robustness pair"
        } else {
            "NOT CONFIGURED — set STATELAB_PRACTRAND and STATELAB_STREAM"
        }
    );
    println!();

    println!(
        "   {:<26} {:>6} {:>8} {:>10} {:>9} {:>11} {:>10} {:>12} {:>11}",
        "permutation",
        "state",
        "rounds",
        "dead",
        "adequate",
        "ns/B/round",
        "total ns/B",
        "PractRand",
        "robustness"
    );

    for name in PERMUTATIONS {
        let perm = permutation_by_name(name).expect("registered");
        let bits = perm.state_bytes() * 8;
        let samples = recommended_samples(bits, TOLERANCE);
        let floor = noise_floor(samples, bits * bits);
        let adequate = floor <= TOLERANCE;

        // Rounds to avalanche: first round count clean on ALL seeds.
        let mut reached = None;
        let mut dead = f64::NAN;
        for r in 1..=MAX_ROUNDS {
            let all: Vec<_> = SEEDS
                .iter()
                .map(|s| avalanche_matrix(perm.as_ref(), r, samples, *s))
                .collect();
            if all.iter().all(|m| m.is_full_avalanche(TOLERANCE)) {
                reached = Some(r);
                dead = all[0].dead_pair_fraction();
                break;
            }
            if r == MAX_ROUNDS {
                dead = all[0].dead_pair_fraction();
            }
        }

        // Cost, interleaved and amortised.
        let mut ns: Vec<f64> = Vec::new();
        for _ in 0..TIMING_PASSES {
            let mut state = vec![0u8; perm.state_bytes()];
            let t = measure(*name, perm.state_bytes(), 20_000, 3, || {
                perm.permute(black_box(&mut state), AMORTISE_OVER);
            });
            ns.push(t.ns_per_byte() / AMORTISE_OVER as f64);
        }
        ns.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        let per_round = ns[ns.len() / 2];

        // PractRand at the design's own round count, keyed, 1GB.
        let pr_main = practrand(name, perm.default_rounds(), true, "1GB", 1 << 30);

        // Robustness at a FIXED round count for every design.
        //
        // A first version evaluated each design at its OWN avalanche round
        // count, which is not a comparison: wide-cross was tested at 3 rounds
        // and ChaCha at 4, so the one given less work necessarily looked
        // worse. Holding the round count fixed is the only way the keyed and
        // zero-filled numbers mean the same thing across rows.
        let robustness = if reached.is_some() {
            let k = practrand(name, ROBUSTNESS_ROUNDS, true, "256MB", 1 << 28);
            let z = practrand(name, ROBUSTNESS_ROUNDS, false, "256MB", 1 << 28);
            match (k, z) {
                (Some(a), Some(b)) => format!("{a}/{b}"),
                _ => "-".to_string(),
            }
        } else {
            "-".to_string()
        };

        println!(
            "   {:<26} {:>6} {:>8} {:>10.4} {:>9} {:>11.4} {:>10} {:>12} {:>11}",
            name,
            perm.state_bytes(),
            reached.map_or(format!(">{MAX_ROUNDS}"), |r| r.to_string()),
            dead,
            if adequate { "yes" } else { "NO" },
            per_round,
            reached.map_or("unbounded".to_string(), |r| format!(
                "{:.3}",
                per_round * r as f64
            )),
            pr_main.map_or("-".to_string(), |f| if f == 0 {
                "clean".to_string()
            } else {
                format!("{f} fail")
            }),
            robustness
        );
    }

    println!("\n-- Reading the table --");
    println!("   rounds      first round count reaching full avalanche on ALL 5 seeds");
    println!("   dead        fraction of (input,output) bit pairs never connected");
    println!("   total ns/B  ns/B/round x rounds. A design that never avalanches is");
    println!("               unbounded however cheap its round is");
    println!("   PractRand   at the design's OWN default round count, keyed, 1GB");
    println!("   NOTE        ns/B is NOT comparable between the hand-written path");
    println!("               (chacha, ascon, ...) and the generic ArxStructure path");
    println!("               (chacha-generic, wide-cross*). Compare chacha-generic");
    println!("               against wide-cross for a like-for-like cost figure.");
    println!("   robustness  failures at a FIXED {ROBUSTNESS_ROUNDS} rounds, keyed/zero-filled.");
    println!("               A gap between the two is sensitivity to low-entropy");
    println!("               structured input — an axis the original table lacked,");
    println!("               and the one thing this session found that was not");
    println!("               already known or already published.");
}
