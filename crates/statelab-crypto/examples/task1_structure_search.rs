//! Task 1 — search ARX diffusion *structures*, not constants.
//!
//! Run:  cargo run -p statelab-crypto --release --example task1_structure_search
//!
//! 5 quarter-round topologies x 4 round patterns = 20 structures, all with
//! **identical operation counts** (4 adds, 4 XORs, 4 rotations) and all at
//! ChaCha's rotation constants, so any difference is attributable to topology.
//!
//! Protocol, following the four traps this project has already fallen into:
//!   1. sampling adequacy is checked and reported for every measurement;
//!   2. nothing is concluded from one seed — candidates are re-measured across
//!      five, and the continuous max-deviation is reported, not a pass/fail
//!      that flickers at the tolerance boundary;
//!   3. timing amortises over a full 20-round permutation, never an isolated
//!      round, which would penalise nothing here but is the honest method;
//!   4. ns/byte from a measured clock, never cycles/byte from an assumed one.

use statelab_crypto::arx_structure::{ArxStructure, QrShape, RoundPattern};
use statelab_crypto::avalanche::{avalanche_matrix, noise_floor, recommended_samples};
use statelab_crypto::bench::{calibrate_tsc_ghz, measure, CpuFeatures};
use statelab_crypto::Permutation;
use std::hint::black_box;

const TOLERANCE: f64 = 0.12;
const SCREEN_SEED: u64 = 0x51A7E1AB;
const VERIFY_SEEDS: [u64; 5] = [
    0x51A7E1AB,
    0xBEEF_1234,
    0x0BAD_CAFE,
    0x1357_9BDF,
    0x2468_ACE0,
];
const MIN_ROUNDS: usize = 2;
const MAX_ROUNDS: usize = 7;
const AMORTISE_OVER: usize = 20;
const TIMING_PASSES: usize = 7;

fn main() {
    let cpu = CpuFeatures::detect();
    let tsc_ghz = calibrate_tsc_ghz();
    let samples = recommended_samples(512, TOLERANCE);
    let floor = noise_floor(samples, 512 * 512);

    println!("=== Task 1 — ARX structural search ===\n");
    println!("   cpu           : {}", cpu.summary());
    println!("   TSC rate      : {tsc_ghz:.3} GHz (measured)");
    println!("   tolerance     : {TOLERANCE}   samples: {samples}   noise floor: {floor:.4}");
    println!(
        "   adequacy      : {}",
        if floor <= TOLERANCE {
            "OK — floor below tolerance"
        } else {
            "INADEQUATE — results below are meaningless"
        }
    );
    println!("   rotations     : [16, 12, 8, 7] for every structure (held fixed)");
    println!("   op count      : 4 add + 4 xor + 4 rot per quarter round, all variants\n");
    println!("   ARX only. No AES-NI, GFNI, AVX-512 or vector intrinsics anywhere.\n");

    // ---- 1. Screen all 20 structures --------------------------------------
    println!(
        "-- 1. Screen: first round count reaching full avalanche (seed {SCREEN_SEED:#x}) --\n"
    );
    println!(
        "   {:<30} {:>8} {:>12} {:>12}",
        "structure", "rounds", "max dev", "dead pairs"
    );

    let mut screened: Vec<(String, QrShape, RoundPattern, Option<usize>, f64)> = Vec::new();
    for qr in QrShape::ALL {
        for pattern in RoundPattern::ALL {
            let s = ArxStructure::new(qr, pattern);
            let mut reached = None;
            let mut dev = f64::NAN;
            let mut dead = f64::NAN;
            for r in MIN_ROUNDS..=MAX_ROUNDS {
                let m = avalanche_matrix(&s, r, samples, SCREEN_SEED);
                if m.is_full_avalanche(TOLERANCE) {
                    reached = Some(r);
                    dev = m.max_deviation();
                    dead = m.dead_pair_fraction();
                    break;
                }
                if r == MAX_ROUNDS {
                    dev = m.max_deviation();
                    dead = m.dead_pair_fraction();
                }
            }
            let label = s.describe();
            let marker = if qr == QrShape::DoubleCross && pattern == RoundPattern::Diagonal(1) {
                "   <- ChaCha"
            } else {
                ""
            };
            println!(
                "   {:<30} {:>8} {:>12.4} {:>12.4}{}",
                label,
                reached.map_or(format!(">{MAX_ROUNDS}"), |r| r.to_string()),
                dev,
                dead,
                marker
            );
            screened.push((label, qr, pattern, reached, dev));
        }
    }

    let chacha_rounds = screened
        .iter()
        .find(|(_, q, p, _, _)| *q == QrShape::DoubleCross && *p == RoundPattern::Diagonal(1))
        .and_then(|(_, _, _, r, _)| *r)
        .expect("ChaCha must avalanche within the sweep");
    println!("\n   ChaCha's baseline: {chacha_rounds} rounds.");

    // ---- 2. Verify anything that screened better, across five seeds -------
    let candidates: Vec<_> = screened
        .iter()
        .filter(|(_, q, p, r, _)| {
            !(*q == QrShape::DoubleCross && *p == RoundPattern::Diagonal(1))
                && matches!(r, Some(x) if *x < chacha_rounds)
        })
        .collect();

    println!(
        "\n-- 2. Multi-seed verification at {} rounds --",
        chacha_rounds - 1
    );
    if candidates.is_empty() {
        println!("\n   NOTHING screened better than ChaCha. No candidate to verify.");
        println!("   Reporting ChaCha and the closest structures for the record instead.\n");
    } else {
        println!(
            "   {} structure(s) screened better. A single seed is not a result —",
            candidates.len()
        );
        println!("   this project has already seen a 2/5-vs-3/5 flicker at the boundary.\n");
    }

    let target = chacha_rounds.saturating_sub(1).max(MIN_ROUNDS);
    println!(
        "   {:<30} {:>9} {:>9} {:>9} {:>8}",
        "structure", "min dev", "median", "max dev", "passes"
    );
    let mut verify: Vec<(String, QrShape, RoundPattern)> = candidates
        .iter()
        .map(|(l, q, p, _, _)| (l.clone(), *q, *p))
        .collect();
    // Always include ChaCha, plus the three best non-ChaCha structures, so the
    // table shows the field rather than only the winners.
    verify.push((
        "double-cross/diag-1".to_string(),
        QrShape::DoubleCross,
        RoundPattern::Diagonal(1),
    ));
    for (l, q, p, r, _) in screened.iter() {
        if verify.len() >= 6 {
            break;
        }
        if *r == Some(chacha_rounds) && !verify.iter().any(|(x, _, _)| x == l) {
            verify.push((l.clone(), *q, *p));
        }
    }

    for (label, qr, pattern) in &verify {
        let s = ArxStructure::new(*qr, *pattern);
        let mut devs: Vec<f64> = VERIFY_SEEDS
            .iter()
            .map(|seed| avalanche_matrix(&s, target, samples, *seed).max_deviation())
            .collect();
        let passes = devs.iter().filter(|d| **d <= TOLERANCE).count();
        devs.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        println!(
            "   {:<30} {:>9.4} {:>9.4} {:>9.4} {:>6}/{}",
            label,
            devs[0],
            devs[devs.len() / 2],
            devs[devs.len() - 1],
            passes,
            VERIFY_SEEDS.len()
        );
    }

    // ---- 3. Cost: fewer rounds is only a win if the round is not dearer ---
    println!("\n-- 3. Cost, interleaved over {TIMING_PASSES} passes --");
    println!("   A structure reaching avalanche sooner is worthless if its round");
    println!("   costs proportionally more. Total = ns/B/round x rounds.\n");
    println!(
        "   {:<30} {:>14} {:>8} {:>13}",
        "structure", "ns/B/round", "rounds", "total ns/B"
    );

    let timed: Vec<(String, QrShape, RoundPattern, Option<usize>)> = screened
        .iter()
        .filter(|(l, _, _, _, _)| verify.iter().any(|(v, _, _)| v == l))
        .map(|(l, q, p, r, _)| (l.clone(), *q, *p, *r))
        .collect();

    let mut samples_ns: Vec<Vec<f64>> = vec![Vec::new(); timed.len()];
    for _ in 0..TIMING_PASSES {
        for (i, (_, qr, pattern, _)) in timed.iter().enumerate() {
            let s = ArxStructure::new(*qr, *pattern);
            let mut state = vec![0u8; s.state_bytes()];
            let t = measure("s", s.state_bytes(), 20_000, 3, || {
                s.permute(black_box(&mut state), AMORTISE_OVER);
            });
            samples_ns[i].push(t.ns_per_byte() / AMORTISE_OVER as f64);
        }
    }

    for (i, (label, _, _, rounds)) in timed.iter().enumerate() {
        let v = &mut samples_ns[i];
        v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        let per_round = v[v.len() / 2];
        println!(
            "   {:<30} {:>14.4} {:>8} {:>13}",
            label,
            per_round,
            rounds.map_or("?".to_string(), |r| r.to_string()),
            rounds.map_or("-".to_string(), |r| format!("{:.3}", per_round * r as f64))
        );
    }

    println!("\n   Rotation constants were held at ChaCha's throughout. A structure");
    println!("   that screens well here has NOT been tuned; its constants were chosen");
    println!("   for a different topology entirely.");
}
