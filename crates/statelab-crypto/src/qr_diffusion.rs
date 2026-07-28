//! Replication of the 2016 quarter-round diffusion metric (H5).
//!
//! Sobti et al., *Analysis of Quarter Rounds of Salsa and ChaCha Core and
//! Proposal of an Alternative Design to Maximize Diffusion*, Indian Journal of
//! Science and Technology 9(3), January 2016, searched all 32⁴ = 1,048,576
//! rotation-constant combinations and reported that **more than 58,000 of them
//! diffuse better than ChaCha's own [16, 12, 8, 7]** on the metric implemented
//! here.
//!
//! The result was published, has follow-on work, and has never been adopted:
//! every deployment of ChaCha20 still uses 16/12/8/7. Nor, as far as could be
//! found, has it ever been independently replicated.
//!
//! # The metric, as they defined it
//!
//! For random words `(a, b, c, d)`: run the quarter round, then flip one
//! random bit of one input word, run it again, and count how many bits differ
//! in each output word. `D[i][j]` is the average bit-change in output word `j`
//! caused by a single-bit flip in input word `i`, over 1000 trials. A perfect
//! quarter round would score 16 everywhere — half of 32 bits.
//!
//! # What this module is for
//!
//! Not to re-derive their answer. To ask the question they did not: their
//! metric measures **one quarter round in isolation**, 128 bits of it. What
//! decides a cipher's speed is how many **full-core rounds** are needed for
//! avalanche across all 512 bits. That those two agree is an assumption, and
//! `examples/phase_b_qr_vs_core.rs` tests it.

use crate::avalanche::Probe;

/// A quarter round parameterised by its four rotation constants.
pub type QuarterRound = fn(&mut [u32; 4], [u32; 4]);

/// ChaCha's quarter round, exactly as the 2016 paper states it:
/// `u0 = x0+x1; u3 = x3^u0; u3 <<<= i; u2 = x2+u3; u1 = x1^u2; u1 <<<= j;`
/// `y0 = u0+u1; y3 = u3^y0; y3 <<<= k; y2 = u2+y3; y1 = u1^y2; y1 <<<= l;`
pub fn chacha_qr(x: &mut [u32; 4], rot: [u32; 4]) {
    x[0] = x[0].wrapping_add(x[1]);
    x[3] = (x[3] ^ x[0]).rotate_left(rot[0]);
    x[2] = x[2].wrapping_add(x[3]);
    x[1] = (x[1] ^ x[2]).rotate_left(rot[1]);
    x[0] = x[0].wrapping_add(x[1]);
    x[3] = (x[3] ^ x[0]).rotate_left(rot[2]);
    x[2] = x[2].wrapping_add(x[3]);
    x[1] = (x[1] ^ x[2]).rotate_left(rot[3]);
}

/// Salsa20's quarter round, present only as a second validation target —
/// the paper publishes its diffusion mean too, so reproducing both numbers
/// is stronger evidence that this metric is implemented as they defined it.
pub fn salsa_qr(x: &mut [u32; 4], rot: [u32; 4]) {
    x[1] ^= x[0].wrapping_add(x[3]).rotate_left(rot[0]);
    x[2] ^= x[1].wrapping_add(x[0]).rotate_left(rot[1]);
    x[3] ^= x[2].wrapping_add(x[1]).rotate_left(rot[2]);
    x[0] ^= x[3].wrapping_add(x[2]).rotate_left(rot[3]);
}

/// The 4×4 diffusion matrix, plus the mean and standard deviation the paper
/// ranks by.
#[derive(Debug, Clone, Copy)]
pub struct Diffusion {
    /// `d[input][output]` — average bits changed in output word `output` when
    /// one bit of input word `input` is flipped.
    pub d: [[f64; 4]; 4],
    pub mean: f64,
    pub std_dev: f64,
}

/// Computes the diffusion matrix for `qr` at rotation constants `rot`.
///
/// `trials` is the paper's averaging count; they used 1000.
pub fn diffusion(qr: QuarterRound, rot: [u32; 4], trials: u32, seed: u64) -> Diffusion {
    let mut probe = Probe::new(seed);
    let mut acc = [[0u64; 4]; 4];

    for _ in 0..trials {
        let base = [
            probe.next_u64() as u32,
            probe.next_u64() as u32,
            probe.next_u64() as u32,
            probe.next_u64() as u32,
        ];
        let mut reference = base;
        qr(&mut reference, rot);

        for (input, row) in acc.iter_mut().enumerate() {
            // Flip one uniformly-chosen bit of this input word.
            let bit = (probe.next_u64() % 32) as u32;
            let mut flipped = base;
            flipped[input] ^= 1 << bit;
            qr(&mut flipped, rot);

            for (output, slot) in row.iter_mut().enumerate() {
                *slot += u64::from((reference[output] ^ flipped[output]).count_ones());
            }
        }
    }

    let mut d = [[0.0f64; 4]; 4];
    let n = f64::from(trials);
    for i in 0..4 {
        for j in 0..4 {
            d[i][j] = acc[i][j] as f64 / n;
        }
    }

    let flat: Vec<f64> = d.iter().flatten().copied().collect();
    let mean = flat.iter().sum::<f64>() / flat.len() as f64;
    let variance = flat.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / flat.len() as f64;

    Diffusion {
        d,
        mean,
        std_dev: variance.sqrt(),
    }
}

/// The full ChaCha core with its four rotation constants left open.
///
/// Identical to [`crate::systems::ChaCha`] at `[16, 12, 8, 7]`; the point is
/// to run the *core* at constants the 2016 search ranked highly on its
/// single-quarter-round metric, and see whether that ranking survives.
pub struct ChaChaRot {
    pub rot: [u32; 4],
}

impl crate::Permutation for ChaChaRot {
    fn name(&self) -> &'static str {
        "chacha-rot"
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        20
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let mut w = [0u32; 16];
        for (i, word) in w.iter_mut().enumerate() {
            *word = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }

        let mut qr = |a: usize, b: usize, c: usize, d: usize| {
            let mut x = [w[a], w[b], w[c], w[d]];
            chacha_qr(&mut x, self.rot);
            w[a] = x[0];
            w[b] = x[1];
            w[c] = x[2];
            w[d] = x[3];
        };

        if round_index.is_multiple_of(2) {
            qr(0, 4, 8, 12);
            qr(1, 5, 9, 13);
            qr(2, 6, 10, 14);
            qr(3, 7, 11, 15);
        } else {
            qr(0, 5, 10, 15);
            qr(1, 6, 11, 12);
            qr(2, 7, 8, 13);
            qr(3, 4, 9, 14);
        }

        for (i, word) in w.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
    }
}

/// Fixed test vectors, so every candidate is scored on identical inputs.
///
/// A paired comparison: drawing fresh randomness per candidate would add
/// noise that has nothing to do with the constants being compared, and with
/// a million candidates the best-scoring one would be substantially a winner
/// of the noise lottery.
pub struct Vectors {
    pub words: Vec<[u32; 4]>,
    pub bits: Vec<[u32; 4]>,
}

impl Vectors {
    pub fn new(trials: usize, seed: u64) -> Self {
        let mut probe = Probe::new(seed);
        let mut words = Vec::with_capacity(trials);
        let mut bits = Vec::with_capacity(trials);
        for _ in 0..trials {
            words.push([
                probe.next_u64() as u32,
                probe.next_u64() as u32,
                probe.next_u64() as u32,
                probe.next_u64() as u32,
            ]);
            bits.push([
                (probe.next_u64() % 32) as u32,
                (probe.next_u64() % 32) as u32,
                (probe.next_u64() % 32) as u32,
                (probe.next_u64() % 32) as u32,
            ]);
        }
        Self { words, bits }
    }
}

/// Mean diffusion over fixed vectors — the ranking key for the sweep.
pub fn mean_diffusion(qr: QuarterRound, rot: [u32; 4], v: &Vectors) -> f64 {
    let mut total = 0u64;
    for (base, bit) in v.words.iter().zip(&v.bits) {
        let mut reference = *base;
        qr(&mut reference, rot);
        for input in 0..4 {
            let mut flipped = *base;
            flipped[input] ^= 1 << bit[input];
            qr(&mut flipped, rot);
            for output in 0..4 {
                total += u64::from((reference[output] ^ flipped[output]).count_ones());
            }
        }
    }
    total as f64 / (v.words.len() as f64 * 16.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Published values from Sobti et al. 2016. Reproducing all three is what
    /// licenses any later claim built on this metric — without it, a
    /// disagreement could equally mean our implementation is wrong.
    ///
    /// Tolerance is generous because their random draws are not ours; the
    /// numbers are means over 1000 trials, so agreement to a few percent is
    /// the most that can be expected.
    #[test]
    fn reproduces_the_published_diffusion_means() {
        let cases: [(&str, QuarterRound, [u32; 4], f64); 3] = [
            ("salsa [7,9,13,18]", salsa_qr, [7, 9, 13, 18], 4.0992),
            ("chacha [7,9,13,18]", chacha_qr, [7, 9, 13, 18], 6.8377),
            ("chacha [16,12,8,7]", chacha_qr, [16, 12, 8, 7], 6.6424),
        ];
        for (name, qr, rot, published) in cases {
            let got = diffusion(qr, rot, 20_000, 0xC0FFEE).mean;
            let error = (got - published).abs() / published;
            assert!(
                error < 0.05,
                "{name}: measured mean {got:.4} vs published {published:.4} \
                 ({:.1}% apart) — the metric is not implemented as they defined it",
                error * 100.0
            );
        }
    }

    /// The paper's central qualitative claim about the two published designs:
    /// ChaCha's quarter round diffuses better than Salsa's.
    #[test]
    fn chacha_quarter_round_beats_salsa_as_published() {
        let salsa = diffusion(salsa_qr, [7, 9, 13, 18], 5_000, 7).mean;
        let chacha = diffusion(chacha_qr, [16, 12, 8, 7], 5_000, 7).mean;
        assert!(
            chacha > salsa,
            "ChaCha {chacha:.4} should exceed Salsa {salsa:.4}"
        );
    }

    /// A perfect quarter round scores 16 (half of 32 bits). Both real designs
    /// are far below it, which is expected for a single quarter round and is
    /// the reason round count matters at all.
    #[test]
    fn no_real_quarter_round_approaches_the_ideal() {
        let chacha = diffusion(chacha_qr, [16, 12, 8, 7], 5_000, 3);
        assert!(chacha.mean > 5.0 && chacha.mean < 8.0);
        for row in chacha.d {
            for v in row {
                assert!((0.0..=32.0).contains(&v));
            }
        }
    }

    /// A rotation-free quarter round must diffuse worse than a rotating one —
    /// the metric has to be able to rank something down, or ranking by it
    /// means nothing.
    #[test]
    fn metric_penalises_a_degenerate_quarter_round() {
        let degenerate = diffusion(chacha_qr, [0, 0, 0, 0], 5_000, 11).mean;
        let real = diffusion(chacha_qr, [16, 12, 8, 7], 5_000, 11).mean;
        assert!(
            degenerate < real,
            "all-zero rotations {degenerate:.4} must score below ChaCha {real:.4}"
        );
    }

    /// *** The 2016 metric is blind to the fourth rotation constant. ***
    ///
    /// In ChaCha's quarter round the last operation is `x1 = (x1^x2) <<< l`,
    /// and rotation is a bijection on bit positions, so
    /// `(a <<< r) ^ (b <<< r) = (a ^ b) <<< r` and the popcount is unchanged.
    /// No other output word is touched after `l` is applied. The whole
    /// diffusion matrix is therefore *provably* invariant in `l`.
    ///
    /// The consequence for the published work is severe: the search covers
    /// 32⁴ = 1,048,576 combinations but only 32³ = 32,768 are distinguishable
    /// by the metric doing the ranking. It also explains why the proposed MCC
    /// sets its fourth constant to 0 — on this metric that is free, while at
    /// the core level it is not.
    #[test]
    fn metric_cannot_see_the_fourth_rotation_constant() {
        for l in 0..32u32 {
            let m = diffusion(chacha_qr, [16, 12, 8, l], 2_000, 99);
            let reference = diffusion(chacha_qr, [16, 12, 8, 0], 2_000, 99);
            assert_eq!(
                m.mean, reference.mean,
                "l={l} changed the mean; the invariance argument is wrong"
            );
            assert_eq!(m.d, reference.d, "l={l} changed the matrix");
        }
    }

    /// The same blindness, stated as the underlying bit fact.
    #[test]
    fn rotation_preserves_hamming_distance() {
        let (a, b) = (0x1234_5678u32, 0x9ABC_DEF0u32);
        let base = (a ^ b).count_ones();
        for r in 0..32u32 {
            assert_eq!((a.rotate_left(r) ^ b.rotate_left(r)).count_ones(), base);
        }
    }

    #[test]
    fn diffusion_is_deterministic_from_its_seed() {
        let a = diffusion(chacha_qr, [16, 12, 8, 7], 1_000, 42).mean;
        let b = diffusion(chacha_qr, [16, 12, 8, 7], 1_000, 42).mean;
        assert_eq!(a, b);
    }
}
