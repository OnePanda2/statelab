//! Diagnostic: why does the BIC null control read LOWER than every real
//! permutation, consistently?
//!
//! First run of `bic_report` showed every real design at 4+ rounds reading
//! max|r| in 0.076-0.083 across designs, widths, rounds and seed bases, while
//! `random_bits_bic` read 0.063-0.067. All are below the analytic floor so no
//! verdict changes — but a control that sits systematically below the thing it
//! is controlling for is either measuring something different or is built
//! differently, and guessing which would be a story rather than a test.
//!
//! Two candidate causes, separated here:
//!   (a) the null's CONSTRUCTION — it draws one bit per `next_u64()` call,
//!       where the real path draws whole states through `Probe::fill`;
//!   (b) a genuine property of real permutations that fair coins lack.
//!
//! If a byte-filled null reads like the real designs, it is (a) and the control
//! needed fixing. If it still reads low, it is (b) and needs recording as open.

use statelab_crypto::avalanche::Probe;
use statelab_crypto::bic::{bic_cells, bic_noise_floor, bic_recommended_samples, random_bits_bic};

/// The same statistic, but with the null's bits drawn the way the real path
/// draws them: whole bytes from `Probe::fill`, not one bit per u64.
fn byte_filled_null(bits: usize, samples: usize, seed: u64) -> f64 {
    let words = samples.div_ceil(64);
    let n_bytes = bits / 8;
    let mut cols = vec![0u64; bits * words];
    let mut buf = vec![0u8; n_bytes];
    let mut max_abs = 0.0f64;

    for i in 0..bits {
        cols.fill(0);
        let mut probe =
            Probe::new(seed.wrapping_add((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)));
        for s in 0..samples {
            probe.fill(&mut buf);
            for j in 0..bits {
                if buf[j / 8] >> (j % 8) & 1 == 1 {
                    cols[j * words + s / 64] |= 1u64 << (s % 64);
                }
            }
        }
        let pc: Vec<u64> = (0..bits)
            .map(|j| {
                cols[j * words..(j + 1) * words]
                    .iter()
                    .map(|w| u64::from(w.count_ones()))
                    .sum()
            })
            .collect();
        let n = samples as f64;
        for j in 0..bits {
            for k in (j + 1)..bits {
                let (nj, nk) = (pc[j], pc[k]);
                if nj == 0 || nj == samples as u64 || nk == 0 || nk == samples as u64 {
                    continue;
                }
                let n11: u64 = cols[j * words..(j + 1) * words]
                    .iter()
                    .zip(cols[k * words..(k + 1) * words].iter())
                    .map(|(a, b)| u64::from((a & b).count_ones()))
                    .sum();
                let (n11, njf, nkf) = (n11 as f64, nj as f64, nk as f64);
                let r = (n * n11 - njf * nkf) / (njf * (n - njf) * nkf * (n - nkf)).sqrt();
                if r.abs() > max_abs {
                    max_abs = r.abs();
                }
            }
        }
    }
    max_abs
}

fn main() {
    println!("BIC NULL DIAGNOSTIC — is the control's construction the outlier?\n");
    println!("  Real designs at saturated rounds read 0.076-0.083 (bic_report).");
    println!("  random_bits_bic reads 0.063-0.067. Same statistic, same cells.\n");

    for bits in [320usize, 512] {
        let samples = bic_recommended_samples(bits, 0.12);
        let floor = bic_noise_floor(samples, bic_cells(bits));
        let bitwise = random_bits_bic(bits, samples, 0xC0FF_EE00 + bits as u64);
        let bytewise = byte_filled_null(bits, samples, 0xC0FF_EE00 + bits as u64);
        println!("  bits {bits}, samples {samples}, analytic floor {floor:.4}");
        println!(
            "    null, one bit per next_u64()   max|r| {:.4}   ratio to floor {:.3}",
            bitwise.max_abs_correlation,
            bitwise.max_abs_correlation / floor
        );
        println!(
            "    null, bytes via Probe::fill    max|r| {:.4}   ratio to floor {:.3}",
            bytewise,
            bytewise / floor
        );
        println!();
    }

    println!("  If the byte-filled null matches the real designs, the bitwise null");
    println!("  was the outlier and the control needs replacing, not the finding.");
}
