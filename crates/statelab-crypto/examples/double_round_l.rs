//! DOES `l` MATTER AT THE DOUBLE ROUND? — testing Dr. Sobti's correction.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example double_round_l
//! ```
//!
//! ## Why this exists
//!
//! `FINDINGS.md` reported that the 2016 diffusion metric is blind to the fourth
//! rotation constant `l`. Dr. Rajeev Sobti, the paper's author, **confirmed the
//! mechanism** in correspondence (2026-08-04) and added a correction that
//! changes the framing:
//!
//! > the blind spot is a property of the quarter round **in isolation**, not of
//! > the cipher as it is used. In follow-up work on the **double round** — a
//! > column round followed by a row/diagonal round — `l` starts to matter again,
//! > because the word that was invisible to `l` on its own becomes an input to
//! > another quarter round in the second half, and addition (unlike XOR) is
//! > sensitive to bit position, so once carries propagate, `l`'s effect returns.
//!
//! **That claim is currently an unverified assertion from private
//! correspondence, and the writeup is about to cite it publicly.** This
//! project's standing rule (§8.5, the fabricated-audit lesson) is that a cited
//! authority gets verified before anything is built on it — and that rule does
//! not get suspended because the authority is the original author and the claim
//! is one we would like to be true.
//!
//! It is also directly testable on the instrument that found the blind spot.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **Single quarter round: the 32 matrices are byte-identical.** This is the
//!    original finding and serves as the POSITIVE CONTROL. If it fails, the
//!    harness is wrong and nothing below it counts.
//! 2. **Double round: the matrices DIFFER across `l`.** This is Sobti's
//!    correction. If they are identical, **his correction is wrong**, the blind
//!    spot extends further than he believes, and that is a much larger finding
//!    than the original — which is precisely why it must be checked rather than
//!    accepted.
//!
//! Outcome 2 failing is the interesting case and is recorded as such in advance
//! so it cannot be reinterpreted afterwards.

use statelab_crypto::avalanche::Probe;
use statelab_crypto::qr_diffusion::chacha_qr;
use statelab_crypto::topology::chacha_topology;

const WORDS: usize = 16;
const TRIALS: u32 = 4000;

/// The 2016 metric's statistic, lifted from 4 words to the full 16-word state
/// over a **double round**: column partition then diagonal partition, which is
/// how ChaCha actually runs.
///
/// `D[i][j]` = mean number of differing bits in output word `j` caused by a
/// single-bit flip in input word `i`.
fn double_round_diffusion(rot: [u32; 4], trials: u32, seed: u64, rounds: usize) -> Vec<f64> {
    let topo = chacha_topology();
    let mut probe = Probe::new(seed);
    let mut acc = vec![0u64; WORDS * WORDS];

    let apply = |w: &mut [u32; WORDS], rounds: usize| {
        for r in 0..rounds {
            for g in &topo.partitions[r % 2] {
                let mut q = [
                    w[g[0] as usize],
                    w[g[1] as usize],
                    w[g[2] as usize],
                    w[g[3] as usize],
                ];
                chacha_qr(&mut q, rot);
                for (k, &lane) in g.iter().enumerate() {
                    w[lane as usize] = q[k];
                }
            }
        }
    };

    for _ in 0..trials {
        let mut base = [0u32; WORDS];
        for w in base.iter_mut() {
            *w = probe.next_u64() as u32;
        }
        for i in 0..WORDS {
            let bit = (probe.next_u64() % 32) as u32;
            let mut a = base;
            let mut b = base;
            b[i] ^= 1u32 << bit;
            apply(&mut a, rounds);
            apply(&mut b, rounds);
            for j in 0..WORDS {
                acc[i * WORDS + j] += u64::from((a[j] ^ b[j]).count_ones());
            }
        }
    }
    acc.iter().map(|&s| s as f64 / trials as f64).collect()
}

/// The paper's own scoring: mean and standard deviation over the matrix.
fn score(d: &[f64]) -> (f64, f64) {
    let n = d.len() as f64;
    let mean = d.iter().sum::<f64>() / n;
    let var = d.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    (mean, var.sqrt())
}

fn sweep(label: &str, base: [u32; 3], rounds: usize, seed: u64) {
    println!(
        "\n-- {label}: [{}, {}, {}, l] over l = 0..31, {rounds} round(s) --",
        base[0], base[1], base[2]
    );
    let reference = double_round_diffusion([base[0], base[1], base[2], 0], TRIALS, seed, rounds);
    let (m0, s0) = score(&reference);

    let mut identical = 0usize;
    let mut means = Vec::new();
    for l in 0..32u32 {
        let d = double_round_diffusion([base[0], base[1], base[2], l], TRIALS, seed, rounds);
        if d == reference {
            identical += 1;
        }
        means.push(score(&d).0);
    }
    let best = means.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let worst = means.iter().cloned().fold(f64::INFINITY, f64::min);
    let best_l = means.iter().position(|&m| m == best).unwrap();

    println!("   l=0 reference: mean {m0:.4}  stddev {s0:.4}");
    println!("   matrices byte-identical to l=0: {identical}/32");
    println!(
        "   mean diffusion across l: [{worst:.4} .. {best:.4}]  spread {:.4}",
        best - worst
    );
    println!("   best l by mean: {best_l} ({best:.4})");
    if identical == 32 {
        println!("   >>> l IS INVISIBLE at {rounds} round(s).");
    } else {
        println!(
            "   >>> l IS VISIBLE at {rounds} round(s) — {} of 32 differ.",
            32 - identical
        );
    }
}

fn main() {
    println!("DOES l MATTER AT THE DOUBLE ROUND? Testing Dr. Sobti's correction\n");
    println!("  metric  the 2016 statistic, lifted to the 16-word state");
    println!("  trials  {TRIALS} per matrix, same seed across the sweep so the");
    println!("          comparison is exact rather than statistical");
    println!();
    println!("  PREDICTION 1 (control): at ONE quarter round the 32 matrices are");
    println!("              byte-identical. This is FINDINGS.md's original result.");
    println!("  PREDICTION 2 (the test): at a DOUBLE round they DIFFER. This is");
    println!("              Sobti's correction. If they do NOT differ, his");
    println!("              correction is wrong and the blind spot is larger than");
    println!("              anyone has claimed — recorded in advance so that");
    println!("              outcome cannot be reinterpreted afterwards.\n");

    // ---- CONTROL: one round is one partition = 4 independent quarter rounds.
    // Each of the 16 words is touched by exactly one QR, so this is the
    // isolated-quarter-round regime FINDINGS.md measured.
    println!("== CONTROL: single round (one partition, isolated quarter rounds) ==");
    sweep("MCC [4,17,8,l]", [4, 17, 8], 1, 12345);
    sweep("ChaCha [16,12,8,l]", [16, 12, 8], 1, 12345);

    // ---- THE TEST: two rounds = column then diagonal = a full double round.
    println!("\n== TEST: double round (column then diagonal) ==");
    sweep("MCC [4,17,8,l]", [4, 17, 8], 2, 12345);
    sweep("ChaCha [16,12,8,l]", [16, 12, 8], 2, 12345);

    println!("\n-- How to read this --");
    println!("   The control must show 32/32 identical. The test shows whether");
    println!("   Sobti's mechanism — the l-invisible word becoming an input to a");
    println!("   second quarter round, where ADDITION is bit-position sensitive —");
    println!("   actually restores l's influence.");
    println!();
    println!("   This verifies the DIRECTION of his claim on our own instrument.");
    println!("   It does NOT reproduce his specific numbers (1.7% of a million");
    println!("   rotation sets beating [4,17,8,0], versus 24-28% for Salsa and");
    println!("   ChaCha) — those come from his follow-up paper's own search and");
    println!("   remain UNVERIFIED here. Cite them to him, not to this run.");
}
