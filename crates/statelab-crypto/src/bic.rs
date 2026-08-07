//! Bit Independence Criterion (Webster & Tavares, CRYPTO '85), proposal §6.3 D2.
//!
//! SAC asks whether each output bit flips with probability 1/2 when an input bit
//! is flipped. BIC asks a strictly harder question: whether those flips are
//! *independent of each other*. A permutation can satisfy SAC perfectly and fail
//! BIC completely — if output bits `j` and `k` always flip together, both are
//! individually unbiased and the pair carries half the entropy it appears to.
//! `avalanche.rs` cannot see that: it measures one output bit at a time.
//!
//! ## The statistic, and why it is not a proportion
//!
//! For input bit `i`, the avalanche vector is `A^i = P(x) ^ P(x ^ e_i)`. BIC is
//! the correlation between `A^i_j` and `A^i_k` over random `x`. For binary
//! variables that is the phi coefficient, identical to the 2x2 contingency
//! correlation:
//!
//! ```text
//!   r = (N*n11 - nj*nk) / sqrt(nj * (N-nj) * nk * (N-nk))
//! ```
//!
//! **This is a different statistic from anything else in this crate, and it has
//! a different null.** A SAC cell is a binomial proportion: standard error
//! `0.5/sqrt(N)`. A BIC cell is a correlation coefficient: under independence
//! `N*r^2` is asymptotically chi-square with one degree of freedom, so `r` has
//! standard error `1/sqrt(N)` — **twice** the proportion's.
//!
//! Feeding this metric through [`crate::avalanche::recommended_samples`] would
//! therefore under-sample it by a factor of **four**, and the result would look
//! clean for exactly the reason methodological item (1) describes: the noise
//! floor would sit above the tolerance and no permutation could fail. That trap
//! has been paid for once in this project already. [`bic_noise_floor`] and
//! [`bic_samples_for_cells`] exist so it cannot be walked into a second time by
//! reusing machinery that looks close enough.
//!
//! ## Cell count
//!
//! The headline is a maximum over every `(input bit, output bit j, output bit k)`
//! triple, which is `bits * C(bits, 2)` cells — for a 512-bit state that is
//! 66,977,792, against SAC's 262,144. The max-of-many-noisy-estimates drift
//! grows as `sqrt(2 ln cells)`, so the cell count must come from [`bic_cells`]
//! and not from `bits * bits`.
//!
//! ## What this module does NOT establish
//!
//! One instrument, one statistic. A BIC reading that disagrees with the rest of
//! the pipeline is a question, not a finding, and the standing rule that a
//! finding needs two independently coded routes applies here unchanged.

use crate::permutation::{flip_bit, get_bit, Permutation};

/// Golden-gamma stride, used to give each input bit a disjoint base-state
/// sequence — methodological item (11). This project's own N3-STATISTICAL
/// result is that golden-gamma spacing separates where small or low-Hamming-
/// weight strides do not, which is why it is the stride used here.
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// Number of `(input bit, output pair)` cells the BIC maximum ranges over.
///
/// `bits * C(bits, 2)`, not `bits * bits`. Using the SAC cell count here would
/// under-state the multiplicity by a factor of about `bits/2` and put the noise
/// floor too low.
pub fn bic_cells(bits: usize) -> usize {
    bits * (bits * (bits - 1) / 2)
}

/// Expected size of the largest purely-chance `|r|` over `cells` estimates, each
/// from `samples` trials.
///
/// **Deliberately not [`crate::avalanche::noise_floor`].** That function inverts
/// a binomial proportion with standard error `0.5/sqrt(n)`. A correlation
/// coefficient under independence has standard error `1/sqrt(n)`, because
/// `n*r^2` is asymptotically chi-square with one degree of freedom. The two
/// differ by exactly the factor of two that would make a BIC measurement look
/// clean when it is merely under-sampled.
pub fn bic_noise_floor(samples: usize, cells: usize) -> f64 {
    if samples == 0 {
        return 1.0;
    }
    let standard_error = 1.0 / (samples as f64).sqrt();
    let spread = (2.0 * (cells.max(2) as f64).ln()).sqrt();
    (standard_error * spread).min(1.0)
}

/// Samples needed before `tolerance` is distinguishable from chance correlation
/// over `cells` cells. Inverts [`bic_noise_floor`], with the same doubling of
/// headroom [`crate::avalanche::samples_for_cells`] uses and for the same
/// reason: the realised maximum fluctuates about its expectation, so sampling
/// exactly at the crossing point fails a perfect permutation half the time.
pub fn bic_samples_for_cells(cells: usize, tolerance: f64) -> usize {
    assert!(tolerance > 0.0, "tolerance must be positive");
    const SAFETY: f64 = 2.0;
    let spread = (2.0 * (cells.max(2) as f64).ln()).sqrt();
    ((spread / tolerance).powi(2) * SAFETY).ceil() as usize
}

/// Convenience: samples needed for a `bits`-wide permutation at `tolerance`.
pub fn bic_recommended_samples(bits: usize, tolerance: f64) -> usize {
    bic_samples_for_cells(bic_cells(bits), tolerance)
}

/// Outcome of a BIC measurement.
#[derive(Debug, Clone)]
pub struct BicResult {
    pub name: &'static str,
    pub bits: usize,
    pub rounds: usize,
    pub samples: usize,
    /// `bits * C(bits, 2)`.
    pub cells: usize,
    /// The headline: largest `|r|` over every measured cell.
    pub max_abs_correlation: f64,
    /// `(input bit, output bit j, output bit k)` where the maximum occurred.
    pub max_at: (usize, usize, usize),
    /// Mean `|r|` over cells with a defined correlation.
    pub mean_abs_correlation: f64,
    /// Cells whose correlation is undefined because an output bit never flipped
    /// (or always flipped) for that input bit, making a marginal degenerate.
    ///
    /// **Not zero-filled and not skipped silently.** At low round counts most of
    /// the matrix is in this state, and a BIC of 0.0 computed over a handful of
    /// live cells is not evidence of independence — it is evidence of no
    /// diffusion. [`BicResult::coverage`] is the number to read alongside the
    /// headline.
    pub undefined_cells: usize,
}

impl BicResult {
    /// Fraction of cells that had a defined correlation.
    ///
    /// A BIC verdict on a permutation with low coverage says nothing. Check this
    /// before reading `max_abs_correlation`.
    pub fn coverage(&self) -> f64 {
        if self.cells == 0 {
            return 0.0;
        }
        (self.cells - self.undefined_cells) as f64 / self.cells as f64
    }

    /// The chance-level maximum for this many samples and cells.
    pub fn noise_floor(&self) -> f64 {
        bic_noise_floor(self.samples, self.cells)
    }

    /// Whether `tolerance` is actually resolvable at this sample count.
    ///
    /// Same discipline as the SAC battery: a verdict taken below this line is
    /// measuring the sampling, not the permutation.
    pub fn sampling_is_adequate(&self, tolerance: f64) -> bool {
        self.noise_floor() < tolerance
    }

    /// `N * r^2` at the maximum — the chi-square statistic for the worst pair.
    ///
    /// Reported for scale, not as a p-value: with tens of millions of cells the
    /// multiplicity correction is what matters, and that is what comparing
    /// against [`BicResult::noise_floor`] does.
    pub fn max_chi_square(&self) -> f64 {
        self.samples as f64 * self.max_abs_correlation * self.max_abs_correlation
    }

    /// Independence verdict: the worst cell sits within `tolerance`, the
    /// sampling can resolve `tolerance`, and the matrix is actually live.
    pub fn is_independent(&self, tolerance: f64) -> bool {
        self.sampling_is_adequate(tolerance)
            && self.coverage() > 0.99
            && self.max_abs_correlation <= tolerance
    }
}

/// Column-major bitset: `bits` columns of `samples` bits each.
struct BitColumns {
    words: usize,
    data: Vec<u64>,
}

impl BitColumns {
    fn new(bits: usize, samples: usize) -> Self {
        let words = samples.div_ceil(64);
        Self {
            words,
            data: vec![0u64; bits * words],
        }
    }

    fn clear(&mut self) {
        self.data.fill(0);
    }

    #[inline]
    fn set(&mut self, column: usize, sample: usize) {
        self.data[column * self.words + sample / 64] |= 1u64 << (sample % 64);
    }

    #[inline]
    fn column(&self, c: usize) -> &[u64] {
        &self.data[c * self.words..(c + 1) * self.words]
    }
}

/// The phi coefficient of a 2x2 table, or `None` if a marginal is degenerate.
#[inline]
fn phi(n: u64, n11: u64, nj: u64, nk: u64) -> Option<f64> {
    if nj == 0 || nj == n || nk == 0 || nk == n {
        return None;
    }
    let (n, n11, nj, nk) = (n as f64, n11 as f64, nj as f64, nk as f64);
    let numerator = n * n11 - nj * nk;
    let denominator = (nj * (n - nj) * nk * (n - nk)).sqrt();
    Some(numerator / denominator)
}

/// Running BIC statistics, accumulated across every input bit.
#[derive(Default)]
struct PairAccumulator {
    max_abs: f64,
    max_at: (usize, usize, usize),
    abs_total: f64,
    undefined: usize,
}

/// Reduces a filled column set to the BIC statistics for one input bit.
fn scan_pairs(
    cols: &BitColumns,
    bits: usize,
    samples: usize,
    input_bit: usize,
    acc: &mut PairAccumulator,
) {
    let n = samples as u64;
    let popcounts: Vec<u64> = (0..bits)
        .map(|j| {
            cols.column(j)
                .iter()
                .map(|w| u64::from(w.count_ones()))
                .sum()
        })
        .collect();

    for (j, &nj) in popcounts.iter().enumerate() {
        let col_j = cols.column(j);
        for (k, &nk) in popcounts.iter().enumerate().skip(j + 1) {
            if nj == 0 || nj == n || nk == 0 || nk == n {
                acc.undefined += 1;
                continue;
            }
            let col_k = cols.column(k);
            let n11: u64 = col_j
                .iter()
                .zip(col_k.iter())
                .map(|(a, b)| u64::from((a & b).count_ones()))
                .sum();
            match phi(n, n11, nj, nk) {
                Some(r) => {
                    let a = r.abs();
                    acc.abs_total += a;
                    if a > acc.max_abs {
                        acc.max_abs = a;
                        acc.max_at = (input_bit, j, k);
                    }
                }
                None => acc.undefined += 1,
            }
        }
    }
}

/// Measures the Bit Independence Criterion for `rounds` rounds.
///
/// Each input bit draws its own base-state sequence from a disjoint region of
/// the probe's output (golden-gamma stride) — methodological item (11), so the
/// cells being maximised over do not share base states across input bits.
///
/// Cost is dominated by the pair scan: `bits * C(bits,2) * ceil(samples/64)`
/// word operations. For a 512-bit state that is billions of word-ops, seconds
/// rather than milliseconds. This is a report-driver measurement, not something
/// to call in a tight loop.
pub fn bic_matrix<P: Permutation + ?Sized>(
    perm: &P,
    rounds: usize,
    samples: usize,
    seed: u64,
) -> BicResult {
    let n_bytes = perm.state_bytes();
    let bits = n_bytes * 8;
    let cells = bic_cells(bits);

    let mut cols = BitColumns::new(bits, samples);
    let mut base = vec![0u8; n_bytes];
    let mut a = vec![0u8; n_bytes];
    let mut b = vec![0u8; n_bytes];

    let mut acc = PairAccumulator::default();

    for i in 0..bits {
        // Disjoint base states per input bit, not a shared sequence.
        let mut probe =
            crate::avalanche::Probe::new(seed.wrapping_add((i as u64).wrapping_mul(GOLDEN_GAMMA)));
        cols.clear();

        for s in 0..samples {
            probe.fill(&mut base);
            a.copy_from_slice(&base);
            b.copy_from_slice(&base);
            flip_bit(&mut b, i);
            perm.permute(&mut a, rounds);
            perm.permute(&mut b, rounds);
            for j in 0..bits {
                if get_bit(&a, j) != get_bit(&b, j) {
                    cols.set(j, s);
                }
            }
        }

        scan_pairs(&cols, bits, samples, i, &mut acc);
    }

    let defined = cells.saturating_sub(acc.undefined);
    let mean_abs_correlation = if defined == 0 {
        0.0
    } else {
        acc.abs_total / defined as f64
    };

    BicResult {
        name: perm.name(),
        bits,
        rounds,
        samples,
        cells,
        max_abs_correlation: acc.max_abs,
        max_at: acc.max_at,
        mean_abs_correlation,
        undefined_cells: acc.undefined,
    }
}

/// **Null-validation control, and a permanent fixture of the known-answer set.**
///
/// Runs the identical pair machinery over avalanche vectors drawn as independent
/// fair coins. The maximum must land near [`bic_noise_floor`]. Without this the
/// battery cannot distinguish "the permutation is independent" from "the null
/// model is wrong", which is exactly the hole the GF(2) rank battery fell into
/// before its own null control was added.
///
/// # *** DO NOT USE THIS AS THE PASS THRESHOLD. ***
///
/// Measured 2026-08-06: this control reads `max|r|` ≈ 0.063–0.067 where **every**
/// real design at a saturated round count reads ≈ 0.076–0.083 — consistently,
/// across two widths, three designs, five round counts and three seed bases. The
/// analytic floor sits above both, so no verdict changes.
///
/// The leading hypothesis — that the control's own construction was the outlier,
/// drawing one bit per `next_u64()` where the measured path draws whole states
/// through [`crate::avalanche::Probe::fill`] — **was tested and eliminated**: a
/// byte-filled null reads 0.0633 and 0.0643, indistinguishable from this one.
/// See `examples/bic_null_diagnostic.rs`.
///
/// The residual explanation is that fair coins are the wrong null object. The
/// measured thing is a *bijection*, whose avalanche vectors carry algebraic
/// constraints independent coins do not have — most obviously that `A` is never
/// zero. Whether that accounts for the whole gap is **untested and open**.
///
/// The operational consequence is the important part: thresholding on this
/// empirical null instead of [`bic_noise_floor`] would put every design tested
/// several standard deviations "above chance" and manufacture a finding out of
/// the difference between a permutation and a coin. Compare against the analytic
/// floor, which is conservative in the safe direction.
pub fn random_bits_bic(bits: usize, samples: usize, seed: u64) -> BicResult {
    let cells = bic_cells(bits);
    let mut cols = BitColumns::new(bits, samples);
    let mut acc = PairAccumulator::default();

    for i in 0..bits {
        let mut probe =
            crate::avalanche::Probe::new(seed.wrapping_add((i as u64).wrapping_mul(GOLDEN_GAMMA)));
        cols.clear();
        for s in 0..samples {
            for j in 0..bits {
                if probe.next_u64() & 1 == 1 {
                    cols.set(j, s);
                }
            }
        }
        scan_pairs(&cols, bits, samples, i, &mut acc);
    }

    let defined = cells.saturating_sub(acc.undefined);
    BicResult {
        name: "random-bits (null control)",
        bits,
        rounds: 0,
        samples,
        cells,
        max_abs_correlation: acc.max_abs,
        max_at: acc.max_at,
        mean_abs_correlation: if defined == 0 {
            0.0
        } else {
            acc.abs_total / defined as f64
        },
        undefined_cells: acc.undefined,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permutation::Permutation;

    /// 32-bit toy permutation: four ARX-ish rounds over four bytes. Small enough
    /// that the pair scan is instant, real enough to diffuse.
    struct Toy;
    impl Permutation for Toy {
        fn name(&self) -> &'static str {
            "toy32"
        }
        fn state_bytes(&self) -> usize {
            4
        }
        fn default_rounds(&self) -> usize {
            8
        }
        fn round(&self, state: &mut [u8], _r: usize) {
            let mut v = u32::from_le_bytes(state[..4].try_into().unwrap());
            v = v.wrapping_mul(0x9E37_79B9).rotate_left(11);
            v ^= v >> 15;
            v = v.wrapping_add(0x85EB_CA6B).rotate_left(7);
            state[..4].copy_from_slice(&v.to_le_bytes());
        }
    }

    /// Deliberately BIC-violating: output bit 1 is forced equal to output bit 0,
    /// so those two always flip together. SAC cannot see this; BIC must.
    struct PlantedPair;
    impl Permutation for PlantedPair {
        fn name(&self) -> &'static str {
            "planted-pair"
        }
        fn state_bytes(&self) -> usize {
            4
        }
        fn default_rounds(&self) -> usize {
            8
        }
        fn round(&self, state: &mut [u8], _r: usize) {
            let mut v = u32::from_le_bytes(state[..4].try_into().unwrap());
            v = v.wrapping_mul(0x9E37_79B9).rotate_left(11);
            v ^= v >> 15;
            v = v.wrapping_add(0x85EB_CA6B).rotate_left(7);
            // Copy bit 0 into bit 1: perfectly correlated output pair.
            v = (v & !0b10) | ((v & 1) << 1);
            state[..4].copy_from_slice(&v.to_le_bytes());
        }
    }

    #[test]
    fn the_bic_null_is_not_the_proportion_null() {
        // The whole reason this module exists. A correlation coefficient's
        // standard error is 1/sqrt(n); a proportion's is 0.5/sqrt(n). Reusing
        // the SAC inversion would under-sample BIC by exactly 4x.
        let cells = bic_cells(512);
        let bic = bic_samples_for_cells(cells, 0.12);
        let proportion = crate::avalanche::samples_for_cells(cells, 0.12);
        // Exactly 4x before rounding; both sides take a ceiling, and
        // ceil(4x) <= 4*ceil(x) < ceil(4x) + 4, so the slack is under four.
        assert!(
            bic <= proportion * 4 && proportion * 4 - bic < 4,
            "BIC must demand four times the samples of a proportion at equal \
             cells (up to ceiling slack): bic {bic}, proportion*4 {}",
            proportion * 4
        );
        // The underlying relation, free of rounding: the floors differ by two,
        // and samples go as the square of the floor.
        let fb = bic_noise_floor(1_000, cells);
        let fp = crate::avalanche::noise_floor(1_000, cells);
        assert!((fb / fp - 2.0).abs() < 1e-12, "floors {fb} vs {fp}");
    }

    #[test]
    fn bic_cells_counts_pairs_not_squares() {
        assert_eq!(bic_cells(4), 4 * 6);
        assert_eq!(bic_cells(512), 512 * (512 * 511 / 2));
        // And it is emphatically not bits*bits.
        assert_ne!(bic_cells(512), 512 * 512);
    }

    #[test]
    fn noise_floor_falls_as_the_root_of_the_sample_count() {
        let cells = bic_cells(32);
        let a = bic_noise_floor(1_000, cells);
        let b = bic_noise_floor(4_000, cells);
        // Four times the samples, half the floor.
        assert!((a / b - 2.0).abs() < 1e-9, "a={a} b={b}");
        // And it is twice the proportion floor at the same cells and samples.
        let p = crate::avalanche::noise_floor(1_000, cells);
        assert!((a / p - 2.0).abs() < 1e-9);
    }

    #[test]
    fn noise_floor_is_clamped_to_a_correlation_range() {
        assert!(bic_noise_floor(1, bic_cells(64)) <= 1.0);
        assert_eq!(bic_noise_floor(0, 10), 1.0);
    }

    #[test]
    fn the_null_control_lands_near_its_predicted_floor() {
        let bits = 32;
        let samples = 2_000;
        let r = random_bits_bic(bits, samples, 0x00C0_FFEE);
        let floor = r.noise_floor();
        assert!(
            r.coverage() > 0.99,
            "fair coins should leave no degenerate marginals, got {}",
            r.coverage()
        );
        // The realised max fluctuates about the expected max; a factor of 1.5
        // either way is generous and still fails a wrong null model.
        assert!(
            r.max_abs_correlation < floor * 1.5,
            "max {} against floor {floor}",
            r.max_abs_correlation
        );
        assert!(
            r.max_abs_correlation > floor * 0.5,
            "max {} suspiciously far below floor {floor} — check the statistic",
            r.max_abs_correlation
        );
    }

    #[test]
    fn a_planted_correlated_pair_is_detected() {
        let samples = bic_recommended_samples(32, 0.25).min(3_000);
        let planted = bic_matrix(&PlantedPair, 8, samples, 7);
        assert!(
            planted.max_abs_correlation > 0.99,
            "a bit copied onto another must read as near-perfect correlation, got {}",
            planted.max_abs_correlation
        );
        // And it must be found at the planted pair, not somewhere incidental.
        let (_, j, k) = planted.max_at;
        assert_eq!((j.min(k), j.max(k)), (0, 1), "found at {j},{k}");
    }

    #[test]
    fn a_diffusing_permutation_is_not_flagged_where_the_planted_one_is() {
        let samples = bic_recommended_samples(32, 0.25).min(3_000);
        let toy = bic_matrix(&Toy, 8, samples, 7);
        let planted = bic_matrix(&PlantedPair, 8, samples, 7);
        assert!(
            toy.max_abs_correlation < planted.max_abs_correlation,
            "positive control must exceed the ordinary case: toy {} planted {}",
            toy.max_abs_correlation,
            planted.max_abs_correlation
        );
        assert!(toy.coverage() > 0.99, "coverage {}", toy.coverage());
    }

    #[test]
    fn a_dead_permutation_reports_low_coverage_rather_than_perfect_independence() {
        // Zero rounds: nothing diffuses, so for input bit i exactly one output
        // bit ever flips. Almost every marginal is degenerate. The headline
        // must NOT read as clean independence.
        struct Identity;
        impl Permutation for Identity {
            fn name(&self) -> &'static str {
                "identity"
            }
            fn state_bytes(&self) -> usize {
                4
            }
            fn default_rounds(&self) -> usize {
                1
            }
            fn round(&self, _state: &mut [u8], _r: usize) {}
        }
        let r = bic_matrix(&Identity, 1, 256, 1);
        assert!(
            r.coverage() < 0.01,
            "coverage should collapse, got {}",
            r.coverage()
        );
        assert!(
            !r.is_independent(0.25),
            "a permutation that does not diffuse must not pass BIC"
        );
    }

    #[test]
    fn phi_matches_a_hand_computed_two_by_two_table() {
        // n=100, n11=40, nj=50, nk=50 -> (100*40 - 2500)/sqrt(50*50*50*50)
        //                              = 1500/2500 = 0.6
        let r = phi(100, 40, 50, 50).expect("non-degenerate");
        assert!((r - 0.6).abs() < 1e-12, "got {r}");
        // Perfect agreement.
        assert!((phi(100, 50, 50, 50).unwrap() - 1.0).abs() < 1e-12);
        // Perfect disagreement.
        assert!((phi(100, 0, 50, 50).unwrap() + 1.0).abs() < 1e-12);
        // Degenerate marginals are None, never 0.0.
        assert!(phi(100, 0, 0, 50).is_none());
        assert!(phi(100, 50, 100, 50).is_none());
    }

    #[test]
    fn sampling_adequacy_gates_the_verdict() {
        let r = BicResult {
            name: "synthetic",
            bits: 512,
            rounds: 4,
            samples: 8,
            cells: bic_cells(512),
            max_abs_correlation: 0.0,
            max_at: (0, 0, 1),
            mean_abs_correlation: 0.0,
            undefined_cells: 0,
        };
        // A perfect maximum at eight samples is meaningless: the floor is above 1.
        assert!(!r.sampling_is_adequate(0.12));
        assert!(!r.is_independent(0.12));
    }
}
