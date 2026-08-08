//! DID THE BLIND SPOT PROPAGATE? — the same test at 64-bit, five years later.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example cocktail_64_l
//! ```
//!
//! ## Why this exists
//!
//! `double_round_l.rs` confirmed Dr. Sobti's correction: the 2016 metric is
//! blind to the fourth rotation constant `l` within an isolated quarter round,
//! but at a double round `l` becomes visible again.
//!
//! In the same correspondence he supplied a fact that is **larger than the
//! original finding and was not part of it**:
//!
//! > the 2021 *Cocktail* paper's 64-bit variant uses rotation constants
//! > **[52, 41, 16, 0]** — a zero fourth constant again, **carried over from
//! > the MCC analysis rather than freshly searched** for Cocktail specifically.
//!
//! If that is right, a parameter chosen under a metric that could not see it
//! was carried forward into a **different primitive** (a hash function), at a
//! **different word width**, **five years later**. That is no longer a fact
//! about one paper's search; it is a parameter propagating on inherited
//! authority.
//!
//! ## What is being claimed, and by whom
//!
//! **The constants and the provenance are Dr. Sobti's report, paraphrased from
//! private correspondence, and are NOT verified here against the 2021 paper.**
//! This run cannot check where a constant came from — only a citation can. What
//! it *can* check is the part that is a property of the metric rather than of
//! anyone's account of history: **is the metric equally blind at 64 bits?**
//!
//! Those are separate claims and are reported separately (item 9, and §8.5 on
//! not letting a credible source carry an unverified premise).
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **Single round, 64-bit: all 64 matrices byte-identical.** The invariance
//!    argument is structural and width-free — Hamming weight is rotation-
//!    invariant at any width, and nothing reads `x1` again inside the round.
//!    This is therefore close to a prediction from proof, and it is the
//!    **positive control**: if it fails, the 64-bit harness is wrong.
//! 2. **Double round, 64-bit: the matrices differ.** Sobti's mechanism —
//!    addition is bit-position sensitive — is also width-free.
//! 3. **The effect size stays at or below the 32-bit case (~0.1% of the mean).**
//!    THIS IS THE ONE THAT MATTERS. At 32 bits the double-round spread was
//!    ~5x SMALLER than the ~0.5% spread Sobti himself attributes to noise. If
//!    64 bits is the same or worse, then the metric that licensed carrying
//!    `l = 0` into Cocktail could barely resolve `l` there either.
//!
//! Prediction 3 failing — a *large* 64-bit effect — would mean the double round
//! resolves `l` well at this width and carrying the constant forward was a
//! substantive choice rather than an invisible one. Recorded in advance so that
//! outcome cannot be reinterpreted afterwards.

use statelab_crypto::avalanche::Probe;
use statelab_crypto::topology::chacha_topology;

const WORDS: usize = 16;
const BITS: u32 = 64;
const TRIALS: u32 = 3000;

/// The ChaCha/MCC quarter round at 64-bit width. Structurally identical to
/// `qr_diffusion::chacha_qr`; only the word type differs.
fn qr64(x: &mut [u64; 4], rot: [u32; 4]) {
    x[0] = x[0].wrapping_add(x[1]);
    x[3] = (x[3] ^ x[0]).rotate_left(rot[0]);
    x[2] = x[2].wrapping_add(x[3]);
    x[1] = (x[1] ^ x[2]).rotate_left(rot[1]);
    x[0] = x[0].wrapping_add(x[1]);
    x[3] = (x[3] ^ x[0]).rotate_left(rot[2]);
    x[2] = x[2].wrapping_add(x[3]);
    x[1] = (x[1] ^ x[2]).rotate_left(rot[3]);
}

/// The 2016 statistic at 64-bit width over the 16-word state, `rounds`
/// partitions of ChaCha's wiring (1 = isolated quarter rounds, 2 = a full
/// column-then-diagonal double round).
fn diffusion64(rot: [u32; 4], trials: u32, seed: u64, rounds: usize) -> Vec<f64> {
    let topo = chacha_topology();
    let mut probe = Probe::new(seed);
    let mut acc = vec![0u64; WORDS * WORDS];

    let apply = |w: &mut [u64; WORDS], rounds: usize| {
        for r in 0..rounds {
            for g in &topo.partitions[r % 2] {
                let mut q = [
                    w[g[0] as usize],
                    w[g[1] as usize],
                    w[g[2] as usize],
                    w[g[3] as usize],
                ];
                qr64(&mut q, rot);
                for (k, &lane) in g.iter().enumerate() {
                    w[lane as usize] = q[k];
                }
            }
        }
    };

    for _ in 0..trials {
        let mut base = [0u64; WORDS];
        for w in base.iter_mut() {
            *w = probe.next_u64();
        }
        for i in 0..WORDS {
            let bit = (probe.next_u64() % u64::from(BITS)) as u32;
            let mut a = base;
            let mut b = base;
            b[i] ^= 1u64 << bit;
            apply(&mut a, rounds);
            apply(&mut b, rounds);
            for j in 0..WORDS {
                acc[i * WORDS + j] += u64::from((a[j] ^ b[j]).count_ones());
            }
        }
    }
    acc.iter().map(|&s| s as f64 / trials as f64).collect()
}

fn mean_of(d: &[f64]) -> f64 {
    d.iter().sum::<f64>() / d.len() as f64
}

fn sweep(label: &str, base: [u32; 3], rounds: usize, seed: u64) {
    println!(
        "\n-- {label}: [{}, {}, {}, l] over l = 0..{}, {rounds} round(s) --",
        base[0],
        base[1],
        base[2],
        BITS - 1
    );
    let reference = diffusion64([base[0], base[1], base[2], 0], TRIALS, seed, rounds);
    let m0 = mean_of(&reference);

    let mut identical = 0usize;
    let mut means = Vec::new();
    for l in 0..BITS {
        let d = diffusion64([base[0], base[1], base[2], l], TRIALS, seed, rounds);
        if d == reference {
            identical += 1;
        }
        means.push(mean_of(&d));
    }
    let best = means.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let worst = means.iter().cloned().fold(f64::INFINITY, f64::min);
    let best_l = means.iter().position(|&m| m == best).unwrap();
    let spread = best - worst;

    println!("   l=0 reference mean: {m0:.4}");
    println!("   matrices byte-identical to l=0: {identical}/{BITS}");
    println!("   mean diffusion across l: [{worst:.4} .. {best:.4}]  spread {spread:.4}");
    println!(
        "   spread as % of mean: {:.4}%   (32-bit double round was ~0.11%)",
        100.0 * spread / m0
    );
    println!("   best l by mean: {best_l} ({best:.4})");
    if identical == BITS as usize {
        println!("   >>> l IS INVISIBLE at {rounds} round(s), 64-bit.");
    } else {
        println!(
            "   >>> l IS VISIBLE at {rounds} round(s), 64-bit — {} of {BITS} differ.",
            BITS as usize - identical
        );
    }
}

fn main() {
    println!("DID THE BLIND SPOT PROPAGATE? The same test at 64-bit\n");
    println!("  Cocktail (2021) reportedly carries [52, 41, 16, 0] — a zero");
    println!("  fourth constant again, per Dr. Sobti carried over from the MCC");
    println!("  analysis rather than freshly searched. THE CONSTANTS AND THE");
    println!("  PROVENANCE ARE HIS REPORT AND ARE NOT VERIFIED HERE. What is");
    println!("  testable is whether the METRIC is equally blind at 64 bits.\n");
    println!("  trials  {TRIALS} per matrix, same seed across each sweep so the");
    println!("          comparison is exact rather than statistical");
    println!("  control BLAKE2b's 64-bit constants [32, 24, 16, 63] — a real");
    println!("          64-bit ARX design whose fourth constant is NONZERO and");
    println!("          was deliberately chosen. If l were resolvable at this");
    println!("          width, this is the design that would show it.\n");
    println!("  PREDICTION 1: 64/64 identical at one round (structural, width-free)");
    println!("  PREDICTION 2: they differ at the double round (Sobti's mechanism)");
    println!("  PREDICTION 3: the double-round effect stays at or below ~0.1% of");
    println!("                the mean — i.e. still well under the ~0.5% Sobti");
    println!("                himself calls noise. THIS IS THE ONE THAT MATTERS.\n");

    println!("== CONTROL: single round (isolated quarter rounds), 64-bit ==");
    sweep("Cocktail [52,41,16,l]", [52, 41, 16], 1, 12345);
    sweep("BLAKE2b [32,24,16,l]", [32, 24, 16], 1, 12345);

    println!("\n== TEST: double round (column then diagonal), 64-bit ==");
    sweep("Cocktail [52,41,16,l]", [52, 41, 16], 2, 12345);
    sweep("BLAKE2b [32,24,16,l]", [32, 24, 16], 2, 12345);

    println!("\n-- How to read this --");
    println!("   The control establishes that the 2016 metric's blind spot is");
    println!("   width-free: it is not a 32-bit accident, and any later design");
    println!("   scored the same way inherits it.");
    println!();
    println!("   The test measures how well the double round — the regime that");
    println!("   DOES see l — resolves it at 64 bits. Read the percentage, not");
    println!("   the spread: what matters is the effect size against the ~0.5%");
    println!("   that the original paper's own author attributes to noise.");
    println!();
    println!("   NOT SHOWN HERE, AND NOT KNOWABLE FROM A RUN: whether Cocktail's");
    println!("   l = 0 was in fact inherited rather than searched. That is Dr.");
    println!("   Sobti's account and needs the 2021 paper to confirm. The");
    println!("   citation is still an open item.");
}
