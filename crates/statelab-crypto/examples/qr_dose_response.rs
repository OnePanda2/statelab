//! LOW-ENTROPY DOSE-RESPONSE on the four confirmed quarter-round candidates.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example qr_dose_response
//! ```
//!
//! ## Why this runs before anything else
//!
//! `TASK_1`'s `wide-cross` cleared a bar of exactly the shape `PHASE_N` just
//! cleared — measurably better than ChaCha, robust across seeds — and then
//! **died on its dose-response curve**: at 3 rounds it took 7 failures on
//! zero-heavy input against ChaCha's 1. Its advantage existed *only* where it
//! was fragile.
//!
//! `PHASE_N`'s candidates make a different claim (same rounds, fewer
//! instructions, not fewer rounds), so the failure mode is not identical. But
//! the question is: **does a byte-aligned quarter round degrade under
//! low-entropy input where ChaCha's does not?**
//!
//! There is a specific reason to suspect it might. All rotations being multiples
//! of 8 means the permutation preserves byte boundaries, and a mostly-zero input
//! state is exactly where byte-structured mixing would have the least to work
//! with — whole zero bytes stay whole zero bytes under a byte rotation.
//!
//! ## *** THIS IS NOT THE HISTORICAL DOSE-RESPONSE. NAMING MATTERS. ***
//!
//! `LOW_ENTROPY_DOSE_RESPONSE.md` varied `--zero-frac` and ran **PractRand** on
//! the emitted stream. That route needs the candidate in the permutation
//! registry, which these are deliberately not (`PHASE_N`; registering a
//! candidate makes it a design under evaluation).
//!
//! This is the **internal analogue**: the same input construction — the 16
//! seed+counter bytes stay random, a `zero_frac` fraction of the remaining 48 is
//! zeroed — measured with the avalanche battery instead of PractRand. It asks
//! the same question with a weaker instrument.
//!
//! **The external PractRand version is the stronger test and has NOT been run.**
//! A clean result here does not close the question; a dirty one closes it
//! immediately.
//!
//! Historical reference point, from the record: *"At 4 rounds every design is
//! clean at every entropy level (20 measurements, zero failures)."* That was
//! measured on chacha, chacha64, blake2b, ascon and wide-cross — **not** on a
//! byte-aligned quarter round, so it does not transfer by assumption.

use statelab_crypto::avalanche::Probe;
use statelab_crypto::permutation::{flip_bit, get_bit, Permutation};
use statelab_crypto::quarter_round::{chacha_qr, QrPermutation, QrStep, QuarterRound};

const TOLERANCE: f64 = 0.12;
const SEED_COUNTER_BYTES: usize = 16;
const SEEDS: [u64; 3] = [101, 202, 12345];
const ZERO_FRACS: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

fn qr(steps: [(u8, u8, u8, u8); 4]) -> QuarterRound {
    QuarterRound {
        steps: steps.map(|(add_to, add_from, xor_into, rot)| QrStep {
            add_to,
            add_from,
            xor_into,
            rot,
        }),
    }
}

/// The four that survived `PHASE_N`'s 5-seed confirmation. Candidate 1 from that
/// run is deliberately absent — it failed 1 of 5 and was rejected.
fn confirmed() -> Vec<(&'static str, QuarterRound)> {
    vec![
        ("chacha (control, 16 instr)", chacha_qr()),
        (
            "cand-0 (12 instr)",
            qr([(1, 2, 2, 16), (3, 2, 0, 24), (0, 3, 1, 16), (3, 1, 2, 16)]),
        ),
        (
            "cand-2 (12 instr)",
            qr([(0, 3, 1, 8), (1, 2, 3, 24), (1, 3, 3, 8), (0, 3, 2, 8)]),
        ),
        (
            "cand-3 (12 instr)",
            qr([(2, 1, 3, 16), (0, 3, 2, 16), (3, 2, 1, 24), (1, 0, 0, 16)]),
        ),
        (
            "cand-4 (12 instr, all rot 24)",
            qr([(0, 2, 3, 24), (3, 2, 2, 24), (1, 3, 2, 24), (3, 2, 0, 24)]),
        ),
    ]
}

/// Avalanche at `rounds`, but with base states whose entropy beyond the first
/// `SEED_COUNTER_BYTES` is reduced. Returns `(offenders, max_dev)`.
///
/// This mirrors the `--zero-frac` construction: the seed+counter region stays
/// random, and `zero_frac` of the remainder is zeroed.
fn low_entropy_avalanche<P: Permutation + ?Sized>(
    perm: &P,
    rounds: usize,
    samples: usize,
    seed: u64,
    zero_frac: f64,
) -> (usize, f64) {
    let n_bytes = perm.state_bytes();
    let bits = n_bytes * 8;
    let tail = n_bytes - SEED_COUNTER_BYTES;
    let zeroed = ((tail as f64) * zero_frac).round() as usize;

    let mut counts = vec![0u32; bits * bits];
    let mut probe = Probe::new(seed);
    let (mut base, mut a, mut b) = (vec![0u8; n_bytes], vec![0u8; n_bytes], vec![0u8; n_bytes]);

    for _ in 0..samples {
        probe.fill(&mut base);
        // Zero the low-entropy tail, leaving seed+counter intact.
        for byte in base.iter_mut().skip(SEED_COUNTER_BYTES).take(zeroed) {
            *byte = 0;
        }
        for i in 0..bits {
            a.copy_from_slice(&base);
            b.copy_from_slice(&base);
            flip_bit(&mut b, i);
            perm.permute(&mut a, rounds);
            perm.permute(&mut b, rounds);
            for (j, slot) in counts[i * bits..(i + 1) * bits].iter_mut().enumerate() {
                if get_bit(&a, j) != get_bit(&b, j) {
                    *slot += 1;
                }
            }
        }
    }

    let mut offenders = 0usize;
    let mut max_dev: f64 = 0.0;
    for &c in &counts {
        let d = (f64::from(c) / samples as f64 - 0.5).abs();
        if d > TOLERANCE {
            offenders += 1;
        }
        max_dev = max_dev.max(d);
    }
    (offenders, max_dev)
}

fn main() {
    // Deliberately below recommended_samples: this sweep is 75 matrices and the
    // comparison is BETWEEN designs at equal sampling, not against an absolute
    // tolerance. The noise floor applies equally to every cell of the table.
    let samples = 300usize;

    println!("LOW-ENTROPY DOSE-RESPONSE — the test that killed wide-cross\n");
    println!("  designs      chacha's quarter round + the 4 confirmed candidates");
    println!("  wiring       ChaCha's, fixed");
    println!("  rounds       4 (the round count PHASE_N's claim is made at)");
    println!("  zero_frac    fraction of the 48 bytes beyond seed+counter zeroed");
    println!(
        "  samples      {samples} per cell, {} seeds per cell",
        SEEDS.len()
    );
    println!();
    println!("  *** NOT the historical PractRand dose-response. This is the");
    println!("  internal analogue — same input construction, weaker instrument.");
    println!("  A clean result here does NOT close the question. ***\n");
    println!("  Suspicion being tested: byte-aligned rotations preserve byte");
    println!("  boundaries, and whole zero bytes stay whole zero bytes under a");
    println!("  byte rotation. Low-entropy input is where that would show.\n");

    for rounds in [4usize, 3] {
        println!("== {rounds} ROUNDS ==");
        println!(
            "  {:<32} {:>10} {:>10} {:>10} {:>10} {:>10}",
            "design", "zf=0.00", "zf=0.25", "zf=0.50", "zf=0.75", "zf=1.00"
        );
        for (label, q) in confirmed() {
            let p = QrPermutation::with_chacha_wiring(q);
            let mut cells = Vec::new();
            for &zf in &ZERO_FRACS {
                // Mean offenders across seeds — item (10).
                let mean: f64 = SEEDS
                    .iter()
                    .map(|&s| low_entropy_avalanche(&p, rounds, samples, s, zf).0 as f64)
                    .sum::<f64>()
                    / SEEDS.len() as f64;
                cells.push(mean);
            }
            print!("  {label:<32}");
            for c in &cells {
                print!(" {c:>10.1}");
            }
            println!();
        }
        println!();
    }

    println!("-- How to read this --");
    println!("  Offender counts (cells outside tolerance). LOWER IS BETTER.");
    println!("  The wide-cross signature is a design that matches chacha at");
    println!("  zf=0.00 and degrades faster as zero_frac rises — an advantage");
    println!("  that exists only where the design is fragile.");
    println!();
    println!("  A candidate tracking chacha across the whole row survives this");
    println!("  test. It does NOT thereby become a result: the external PractRand");
    println!("  version is stronger and unrun, and CLAASP is the actual gate.");
    println!("  Avalanche is a proxy (PHASE_L §4, item 16).");
}
