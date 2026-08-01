//! Diffusion battery (proposal §6.3 D1–D4): SAC, BIC, rounds-to-avalanche,
//! and the dependency matrix.
//!
//! This is the battery that answers target T1, and `rounds_to_avalanche` is the
//! headline comparative number the whole programme is organised around.
//!
//! Everything here measures the RAW PERMUTATION with no extractor, per the
//! mandatory protocol in §6.1. Feeding extracted output into these functions
//! would measure the extractor instead, which is the error that invalidated
//! the v1.0 methodology.

use crate::permutation::{flip_bit, get_bit, Permutation};

/// A deterministic, seedable generator used only to pick base states.
///
/// Deliberately not cryptographic: it must never be confused with something
/// under test, and the measurement must be reproducible from a seed.
pub struct Probe(u64);

impl Probe {
    pub fn new(seed: u64) -> Self {
        // Any nonzero state; splitmix64 tolerates a zero seed fine.
        Self(seed)
    }
    /// splitmix64.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            let v = self.next_u64().to_le_bytes();
            let n = chunk.len();
            chunk.copy_from_slice(&v[..n]);
        }
    }
}

/// The strict-avalanche dependency matrix.
///
/// `p[i][j]` is the measured probability that output bit `j` flips when input
/// bit `i` is flipped. A permutation with ideal diffusion has every entry at
/// 0.5; structure in this matrix is the defect.
#[derive(Debug, Clone)]
pub struct AvalancheMatrix {
    pub bits: usize,
    pub rounds: usize,
    pub samples: usize,
    /// Row-major, `bits * bits` entries.
    pub p: Vec<f64>,
}

impl AvalancheMatrix {
    #[inline]
    pub fn get(&self, input_bit: usize, output_bit: usize) -> f64 {
        self.p[input_bit * self.bits + output_bit]
    }

    /// Largest deviation from the ideal 0.5 anywhere in the matrix. This is the
    /// strict criterion: one stuck bit-pair fails it, as it should.
    pub fn max_deviation(&self) -> f64 {
        self.p
            .iter()
            .map(|v| (v - 0.5).abs())
            .fold(0.0f64, f64::max)
    }

    /// Mean absolute deviation from 0.5 — the aggregate view, which hides
    /// isolated defects and so is always reported alongside `max_deviation`.
    pub fn mean_deviation(&self) -> f64 {
        let sum: f64 = self.p.iter().map(|v| (v - 0.5).abs()).sum();
        sum / self.p.len() as f64
    }

    /// Fraction of (input, output) bit pairs with no observed dependency at
    /// all. For a triangular map this stays high no matter how many rounds
    /// are applied, which is exactly the defect worth naming.
    pub fn dead_pair_fraction(&self) -> f64 {
        let dead = self.p.iter().filter(|&&v| v == 0.0).count();
        dead as f64 / self.p.len() as f64
    }

    /// True iff every entry is within `tolerance` of 0.5.
    ///
    /// Meaningless unless [`Self::sampling_is_adequate`] also holds — see
    /// [`noise_floor`] for why.
    pub fn is_full_avalanche(&self, tolerance: f64) -> bool {
        self.max_deviation() <= tolerance
    }

    /// Whether this matrix has enough samples for `tolerance` to be
    /// distinguishable from sampling noise.
    pub fn sampling_is_adequate(&self, tolerance: f64) -> bool {
        noise_floor(self.samples, self.p.len()) <= tolerance
    }
}

/// Expected size of the largest purely-random deviation from 0.5, for a matrix
/// of `cells` entries each estimated from `samples` trials.
///
/// **This is the trap that a naive avalanche measurement falls into.** Each
/// cell is a binomial proportion with standard error `0.5/√samples`. The
/// headline metric is a *maximum* over cells, and the maximum of many noisy
/// estimates drifts away from the mean as the count grows — roughly
/// `√(2·ln cells)` standard errors.
///
/// For a 512-bit permutation that is 262,144 cells, about 5 standard errors.
/// At 24 samples the noise floor alone is ≈0.51, so `max_deviation` can never
/// fall below any useful tolerance no matter how perfect the permutation is.
/// Measured against that, ChaCha "fails" — which says nothing about ChaCha.
///
/// Discovered by the positive control failing when it should have passed. The
/// control earned its place.
pub fn noise_floor(samples: usize, cells: usize) -> f64 {
    if samples == 0 {
        return 0.5;
    }
    let standard_error = 0.5 / (samples as f64).sqrt();
    let spread = (2.0 * (cells.max(2) as f64).ln()).sqrt();
    standard_error * spread
}

/// Samples needed before `tolerance` is distinguishable from noise for a
/// `bits`×`bits` matrix. Inverts [`noise_floor`], with headroom.
///
/// The `SAFETY` factor matters. `noise_floor` estimates the *expected* maximum
/// deviation, but the realised maximum fluctuates around it, so sampling at
/// exactly the point where the floor equals the tolerance makes a perfect
/// permutation fail about half the time. Doubling the sample count puts the
/// floor at `tolerance/√2` and leaves room for that fluctuation.
pub fn recommended_samples(bits: usize, tolerance: f64) -> usize {
    samples_for_cells(bits * bits, tolerance)
}

/// [`recommended_samples`] for a grid that is not square.
///
/// The seed-correlation battery (§6.4 N1–N4) builds grids of seed-pairs × bits
/// and lags × bits, which have the same max-of-many-noisy-estimates problem and
/// must not solve it a second, subtly different way. One inversion, both
/// callers — the same reasoning that moved the stream construction into
/// [`crate::stream`].
pub fn samples_for_cells(cells: usize, tolerance: f64) -> usize {
    assert!(tolerance > 0.0, "tolerance must be positive");
    const SAFETY: f64 = 2.0;
    let spread = (2.0 * (cells.max(2) as f64).ln()).sqrt();
    ((0.5 * spread / tolerance).powi(2) * SAFETY).ceil() as usize
}

/// Measures the avalanche dependency matrix for `rounds` rounds.
///
/// For each input bit, `samples` random base states are drawn, the bit is
/// flipped, both are permuted, and the differing output bits are counted.
pub fn avalanche_matrix<P: Permutation + ?Sized>(
    perm: &P,
    rounds: usize,
    samples: usize,
    seed: u64,
) -> AvalancheMatrix {
    let n_bytes = perm.state_bytes();
    let bits = n_bytes * 8;
    let mut counts = vec![0u32; bits * bits];
    let mut probe = Probe::new(seed);

    let mut base = vec![0u8; n_bytes];
    let mut a = vec![0u8; n_bytes];
    let mut b = vec![0u8; n_bytes];

    for _ in 0..samples {
        probe.fill(&mut base);
        for i in 0..bits {
            a.copy_from_slice(&base);
            b.copy_from_slice(&base);
            flip_bit(&mut b, i);
            perm.permute(&mut a, rounds);
            perm.permute(&mut b, rounds);
            let row = &mut counts[i * bits..(i + 1) * bits];
            for (j, slot) in row.iter_mut().enumerate() {
                if get_bit(&a, j) != get_bit(&b, j) {
                    *slot += 1;
                }
            }
        }
    }

    let p = counts
        .iter()
        .map(|&c| f64::from(c) / samples as f64)
        .collect();

    AvalancheMatrix {
        bits,
        rounds,
        samples,
        p,
    }
}

/// Result of sweeping the round count looking for full avalanche.
#[derive(Debug, Clone)]
pub struct AvalancheSweep {
    pub name: &'static str,
    /// Per-round `(round, max_deviation, mean_deviation, dead_pair_fraction)`.
    pub per_round: Vec<(usize, f64, f64, f64)>,
    /// First round index reaching full avalanche, or `None` if never within
    /// the sweep. `None` is a result, not a failure of the measurement.
    pub rounds_to_avalanche: Option<usize>,
    pub tolerance: f64,
}

/// Sweeps rounds `1..=max_rounds` and reports the first to reach full
/// avalanche — target T1's headline number.
///
/// A `None` result means the design did not converge within the sweep, which
/// for a triangular map is the expected and correct answer.
pub fn rounds_to_avalanche<P: Permutation + ?Sized>(
    perm: &P,
    max_rounds: usize,
    samples: usize,
    tolerance: f64,
    seed: u64,
) -> AvalancheSweep {
    let mut per_round = Vec::with_capacity(max_rounds);
    let mut first = None;
    for r in 1..=max_rounds {
        let m = avalanche_matrix(perm, r, samples, seed);
        let (max_d, mean_d, dead) = (
            m.max_deviation(),
            m.mean_deviation(),
            m.dead_pair_fraction(),
        );
        per_round.push((r, max_d, mean_d, dead));
        if first.is_none() && m.is_full_avalanche(tolerance) {
            first = Some(r);
        }
    }
    AvalancheSweep {
        name: perm.name(),
        per_round,
        rounds_to_avalanche: first,
        tolerance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::{ChaCha, Counter, KlimovShamir};

    #[test]
    fn probe_is_deterministic_from_its_seed() {
        let mut a = Probe::new(42);
        let mut b = Probe::new(42);
        for _ in 0..16 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Probe::new(1).next_u64(), Probe::new(2).next_u64());
    }

    /// The negative control required by §6.1 M4. A counter must never reach
    /// avalanche; if it does, the battery is measuring nothing.
    #[test]
    fn counter_never_reaches_avalanche() {
        let sweep = rounds_to_avalanche(&Counter { bytes: 8 }, 8, 32, 0.05, 7);
        assert_eq!(
            sweep.rounds_to_avalanche, None,
            "a counter must never reach full avalanche"
        );
        let (_, max_d, _, dead) = sweep.per_round[7];
        assert!(max_d > 0.4, "counter deviation should stay near maximal");
        assert!(dead > 0.5, "most bit pairs in a counter never interact");
    }

    /// ChaCha must reach avalanche. The positive control; together with the
    /// counter test it brackets the battery.
    ///
    /// Sampled at the level [`recommended_samples`] requires. An earlier
    /// version of this test used 24 samples and failed — not because ChaCha is
    /// weak, but because the noise floor at 24 samples exceeds any useful
    /// tolerance. Sweeping rounds here would be needlessly slow, so it checks
    /// a fixed round count instead.
    #[test]
    fn chacha_reaches_avalanche_at_its_shipped_round_count() {
        let tolerance = 0.12;
        let samples = recommended_samples(512, tolerance);
        let m = avalanche_matrix(&ChaCha, 8, samples, 11);
        assert!(
            m.sampling_is_adequate(tolerance),
            "test is under-sampled: noise floor {:.3} exceeds tolerance {tolerance}",
            noise_floor(samples, m.p.len())
        );
        assert!(
            m.is_full_avalanche(tolerance),
            "ChaCha failed avalanche at 8 rounds: max deviation {:.4}, dead pairs {:.4}",
            m.max_deviation(),
            m.dead_pair_fraction()
        );
        assert_eq!(
            m.dead_pair_fraction(),
            0.0,
            "ChaCha must leave no bit pair unconnected"
        );
    }

    /// The noise floor is why the positive control originally failed. Pinned so
    /// the reasoning cannot be quietly lost.
    #[test]
    fn noise_floor_falls_with_samples_and_rises_with_matrix_size() {
        // 512-bit matrix at 24 samples: noise alone exceeds any useful bound.
        assert!(noise_floor(24, 512 * 512) > 0.4);
        // More samples shrink it.
        assert!(noise_floor(4096, 512 * 512) < 0.05);
        // Bigger matrices raise it at fixed sample count.
        assert!(noise_floor(1024, 512 * 512) > noise_floor(1024, 64 * 64));
        // recommended_samples inverts it.
        let n = recommended_samples(512, 0.10);
        assert!(noise_floor(n, 512 * 512) <= 0.10);
    }

    /// `recommended_samples` became a thin wrapper when the correlation battery
    /// needed the non-square case. Pinned so the delegation stays exact rather
    /// than drifting into two nearly-identical formulas.
    #[test]
    fn recommended_samples_is_the_square_case_of_samples_for_cells() {
        for bits in [64usize, 320, 512, 1024] {
            for tol in [0.05, 0.10, 0.12] {
                assert_eq!(
                    recommended_samples(bits, tol),
                    samples_for_cells(bits * bits, tol)
                );
            }
        }
        // And the non-square case still inverts the floor it came from.
        let n = samples_for_cells(66 * 512, 0.12);
        assert!(noise_floor(n, 66 * 512) <= 0.12);
    }

    /// The graveyard's central lesson, measured rather than asserted: a
    /// T-function leaves a large fraction of bit pairs permanently unconnected.
    #[test]
    fn klimov_shamir_leaves_dead_bit_pairs() {
        let m = avalanche_matrix(&KlimovShamir { bytes: 64 }, 4, 16, 3);
        assert!(
            m.dead_pair_fraction() > 0.5,
            "expected a T-function to leave most bit pairs unconnected, got {}",
            m.dead_pair_fraction()
        );
        assert!(!m.is_full_avalanche(0.10));
    }

    #[test]
    fn matrix_accessor_is_row_major_by_input_bit() {
        let m = AvalancheMatrix {
            bits: 2,
            rounds: 1,
            samples: 1,
            p: vec![0.0, 0.25, 0.75, 1.0],
        };
        assert_eq!(m.get(0, 0), 0.0);
        assert_eq!(m.get(0, 1), 0.25);
        assert_eq!(m.get(1, 0), 0.75);
        assert_eq!(m.get(1, 1), 1.0);
        assert_eq!(m.max_deviation(), 0.5);
        assert_eq!(m.dead_pair_fraction(), 0.25);
    }
}
