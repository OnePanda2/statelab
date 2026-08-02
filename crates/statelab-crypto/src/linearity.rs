//! GF(2) linear-structure battery — the gap PractRand exposed.
//!
//! ## Why this exists
//!
//! Every substantive finding of the N1–N4 work traced to GF(2) linear
//! structure: 3-round ChaCha failing `BRank`, N2's interleaving masking it,
//! N3-STATISTICAL's low-Hamming-weight stride effect. In every case **PractRand
//! found it and this instrument could not**. The internal N2 called 3 rounds
//! clean and had no mechanism to know otherwise, because nothing here measured
//! rank over GF(2). This closes that.
//!
//! ## What it computes
//!
//! **Subspace rank** — the primary statistic. Draw `m` inputs confined to a
//! `k`-dimensional affine subspace, permute, and take the rank over GF(2) of the
//! matrix of output *differences*:
//!
//! ```text
//! y_i = P(x_i) ⊕ P(x₀)          Y is m × n
//! statistic = rank_GF(2)(Y)
//! ```
//!
//! If `P` restricted to that subspace is affine then `y_i = M·(x_i ⊕ x₀)`, so
//! every row lies in the image of a `k`-dimensional space and **rank ≤ k**. If
//! `P` is nonlinear there the rows are generically independent and
//! **rank = m**.
//!
//! ## The default is SQUARE — and the first version got this wrong
//!
//! For an `m × n` random binary matrix,
//! `P(rank = m) = ∏ᵢ₌₀^{m−1}(1 − 2^{−(n−i)})` — see [`full_rank_probability`].
//!
//! The first version of this module defaulted to `m ≪ n` (64 × 512), on the
//! reasoning that this puts the null at `1 − 2⁻⁴⁴⁸`, so full rank is essentially
//! certain and **any** deficiency is signal with no distributional comparison
//! needed. **That reasoning is true, and it is precisely why the test was
//! blind.**
//!
//! The mechanism, stated because "it was wrong" is not the useful part:
//! `rank(Y) ≤ k` follows only if `P` is **affine on the probed subspace** — a
//! wholesale collapse. Reduced-round ChaCha is never affine; its additions carry,
//! and carries are nonlinear. What it has is *weak linear correlation*, which
//! moves the rank distribution by a fraction of one dimension. A regime in which
//! the null is degenerate — full rank essentially always — has no distribution
//! left to move, so it cannot see any structure short of total collapse.
//! **Choosing a regime that removes the need for statistics removes the
//! sensitivity that statistics were buying.**
//!
//! Measured: at 64 × 512 the battery reads full rank for ChaCha at 2 rounds and
//! upward, including at 2 rounds where PractRand fails with hundreds of tests.
//! It detects 1-round ChaCha (rank 33–37) and nothing beyond. At 512 × 512 with
//! a distributional comparison it separates 3 rounds (full-rank fraction 0.140,
//! −4.65σ) from 4 and 20 rounds (0.270 and 0.280, both on the null).
//!
//! So the default is now square, and the statistic is [`rank_trials`], which
//! compares an observed full-rank fraction against the theoretical null.
//! [`subspace_rank`] remains available and is still the right tool for detecting
//! *exact* linearity — see the note on separability below.
//!
//! ## Two separable properties
//!
//! The `m ≪ n` regime is not merely insufficient; it measures a genuinely
//! different property, and running both is informative. A map can be
//! **weakly linearly correlated** without being **affine on a small subspace**,
//! and reduced-round ChaCha is exactly that: rank-deficient in distribution at
//! 512 × 512, yet full rank at 64 × 512 from 2 rounds up. `xoshiro256++` is
//! both. That separation is only visible because both regimes exist.
//!
//! ## Subspace blindness, and what is done about it
//!
//! The statistic probes a *chosen* subspace, so a defect confined to a subspace
//! that is never sampled is invisible. The default direction set mirrors the
//! stream construction (low bits of the block-counter lane) because that is what
//! reproduces the observed `BRank` behaviour — but a default that is also the
//! only shape ever tested would make the blindness structural. [`lane_bits`] and
//! [`across_lanes`] exist so the battery is run over several geometries, and the
//! report driver does exactly that.

use crate::avalanche::Probe;
use crate::permutation::Permutation;

/// Rank over GF(2) of a bit matrix, by Gaussian elimination.
///
/// `rows` is row-major, each row packed little-endian into `u64` words; bit `b`
/// of a row lives at `word[b / 64] >> (b % 64)`. Consumed destructively, which
/// is why it takes `&mut` — the caller owns a scratch copy.
///
/// `O(m · n · words)`. At the defaults that is 64 × 512 × 8, which is nothing.
pub fn gf2_rank(rows: &mut [Vec<u64>], n_bits: usize) -> usize {
    let mut rank = 0usize;
    for col in 0..n_bits {
        let (w, b) = (col / 64, col % 64);
        // Find a pivot at or below the current rank.
        let Some(pivot) = (rank..rows.len()).find(|&r| (rows[r][w] >> b) & 1 == 1) else {
            continue;
        };
        rows.swap(rank, pivot);
        // Eliminate this column from every other row that has it set.
        for r in 0..rows.len() {
            if r != rank && (rows[r][w] >> b) & 1 == 1 {
                for k in 0..rows[rank].len() {
                    rows[r][k] ^= rows[rank][k];
                }
            }
        }
        rank += 1;
        if rank == rows.len() {
            break;
        }
    }
    rank
}

/// How the `m` probe inputs are generated.
#[derive(Debug, Clone)]
pub enum InputSet {
    /// Random XOR-combinations of full-width direction masks. A genuine
    /// `k`-dimensional affine subspace, so the `rank ≤ k` bound applies.
    Subspace(Vec<Vec<u8>>),
    /// Block-counter lane set to `i · stride` for `i = 0..m`. An arithmetic
    /// progression rather than a subspace — carries make it not closed under
    /// XOR — so no `≤ k` bound applies, but it mirrors N3-STATISTICAL exactly
    /// and lets that finding be re-derived by a third route.
    Stride(u64),
}

/// One subspace-rank measurement.
#[derive(Debug, Clone)]
pub struct RankResult {
    pub name: &'static str,
    pub rounds: usize,
    /// `m`, the number of probe inputs.
    pub rows: usize,
    /// `n`, the state width in bits.
    pub cols: usize,
    /// `k`, when the input set is a subspace of known dimension.
    pub dim: Option<usize>,
    pub rank: usize,
    pub seed: u64,
}

impl RankResult {
    /// `m − rank`. Zero for a design with no linear structure on this subspace.
    pub fn deficiency(&self) -> usize {
        self.rows - self.rank
    }

    /// True iff the rank collapsed to the subspace dimension — the signature of
    /// a map that is affine on the probed subspace.
    pub fn collapsed_to_subspace(&self) -> bool {
        self.dim.is_some_and(|k| self.rank <= k)
    }
}

/// Direction masks selecting `count` consecutive bits starting at `first_bit`
/// of `lane` (lanes are 8 bytes wide).
///
/// The default direction set is `lane_bits(n, 1, 0, 16)` — the low 16 bits of
/// the block-counter lane, which is what the stream construction actually
/// varies.
pub fn lane_bits(state_bytes: usize, lane: usize, first_bit: usize, count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|i| {
            let mut m = vec![0u8; state_bytes];
            let bit = lane * 64 + first_bit + i;
            m[bit / 8] |= 1 << (bit % 8);
            m
        })
        .collect()
}

/// Direction masks spread one per lane, cycling through bit offsets — a
/// geometry deliberately unlike [`lane_bits`], so the battery is not only ever
/// run on one shape.
pub fn across_lanes(state_bytes: usize, count: usize) -> Vec<Vec<u8>> {
    let lanes = state_bytes / 8;
    (0..count)
        .map(|i| {
            let mut m = vec![0u8; state_bytes];
            let bit = (i % lanes) * 64 + (i / lanes) * 7 % 64;
            m[bit / 8] |= 1 << (bit % 8);
            m
        })
        .collect()
}

/// Measures the GF(2) rank of output differences over a structured input set.
pub fn subspace_rank<P: Permutation + ?Sized>(
    perm: &P,
    rounds: usize,
    input: &InputSet,
    samples: usize,
    seed: u64,
) -> RankResult {
    let n_bytes = perm.state_bytes();
    let n_bits = n_bytes * 8;
    let words = n_bits.div_ceil(64);
    let mut probe = Probe::new(seed);

    // A random base point, so the measurement is not anchored at a special
    // state. Multi-seed is the default here, per methodological item (10).
    let mut base = vec![0u8; n_bytes];
    probe.fill(&mut base);

    let apply = |state: &[u8]| {
        let mut s = state.to_vec();
        perm.permute(&mut s, rounds);
        s
    };

    let y0 = apply(&base);
    let mut rows: Vec<Vec<u64>> = Vec::with_capacity(samples);

    for i in 0..samples {
        let mut x = base.clone();
        match input {
            InputSet::Subspace(dirs) => {
                // Random nonempty XOR-combination of the directions.
                for d in dirs {
                    if probe.next_u64() & 1 == 1 {
                        for (a, b) in x.iter_mut().zip(d) {
                            *a ^= *b;
                        }
                    }
                }
            }
            InputSet::Stride(s) => {
                let c = (i as u64).wrapping_mul(*s);
                x[8..16].copy_from_slice(&c.to_le_bytes());
            }
        }
        let y = apply(&x);
        // Pack the difference into u64 words.
        let mut row = vec![0u64; words];
        for (b, (p, q)) in y.iter().zip(&y0).enumerate() {
            let d = p ^ q;
            for bit in 0..8 {
                if (d >> bit) & 1 == 1 {
                    let idx = b * 8 + bit;
                    row[idx / 64] |= 1u64 << (idx % 64);
                }
            }
        }
        rows.push(row);
    }

    let rank = gf2_rank(&mut rows, n_bits);
    RankResult {
        name: perm.name(),
        rounds,
        rows: samples,
        cols: n_bits,
        dim: match input {
            InputSet::Subspace(d) => Some(d.len()),
            InputSet::Stride(_) => None,
        },
        rank,
        seed,
    }
}

/// Mean normalised Hamming distance between `P` and its best affine
/// approximation anchored at a random base point.
///
/// Builds `M` with column *i* = `P(x₀ ⊕ eᵢ) ⊕ P(x₀)`, giving
/// `A(x) = M·(x ⊕ x₀) ⊕ P(x₀)`, which agrees with `P` at `x₀` and at every
/// `x₀ ⊕ eᵢ` by construction. Then compares `P(x)` against `A(x)` on random `x`.
///
/// **Exactly 0 iff `P` is affine.** A good permutation sits near 0.5. This is
/// the sharper known-answer target: an affine map has no tolerance band, it has
/// an exact answer.
pub fn affine_residual<P: Permutation + ?Sized>(
    perm: &P,
    rounds: usize,
    seed: u64,
    probes: usize,
) -> f64 {
    let n_bytes = perm.state_bytes();
    let n_bits = n_bytes * 8;
    let mut probe = Probe::new(seed);

    let mut base = vec![0u8; n_bytes];
    probe.fill(&mut base);

    let apply = |state: &[u8]| {
        let mut s = state.to_vec();
        perm.permute(&mut s, rounds);
        s
    };

    let y0 = apply(&base);

    // Column i of M: the derivative of P at base in direction e_i.
    let columns: Vec<Vec<u8>> = (0..n_bits)
        .map(|i| {
            let mut x = base.clone();
            x[i / 8] ^= 1 << (i % 8);
            let y = apply(&x);
            y.iter().zip(&y0).map(|(a, b)| a ^ b).collect()
        })
        .collect();

    let mut total = 0usize;
    for _ in 0..probes {
        let mut x = vec![0u8; n_bytes];
        probe.fill(&mut x);

        // A(x) = y0 XOR sum of the columns selected by (x XOR base).
        let mut approx = y0.clone();
        for i in 0..n_bits {
            let d = x[i / 8] ^ base[i / 8];
            if (d >> (i % 8)) & 1 == 1 {
                for (a, c) in approx.iter_mut().zip(&columns[i]) {
                    *a ^= *c;
                }
            }
        }

        let actual = apply(&x);
        total += actual
            .iter()
            .zip(&approx)
            .map(|(a, b)| (a ^ b).count_ones() as usize)
            .sum::<usize>();
    }

    total as f64 / (probes * n_bits) as f64
}

/// `P(rank = rows)` for a uniformly random `rows × cols` binary matrix:
/// `∏ᵢ₌₀^{rows−1} (1 − 2^{−(cols−i)})`.
///
/// Square: ≈ 0.2888, so a random matrix is rank-deficient about 71% of the
/// time and deficiency is *normal*. Tall-thin (`rows ≪ cols`): ≈ 1, so the null
/// is degenerate. Which regime you are in decides whether a deficiency means
/// anything — see the module note.
pub fn full_rank_probability(rows: usize, cols: usize) -> f64 {
    (0..rows)
        .map(|i| 1.0 - 2f64.powi(-((cols - i) as i32)))
        .product()
}

/// Many independent rank measurements, summarised against the theoretical null.
#[derive(Debug, Clone)]
pub struct RankTrialSummary {
    pub name: &'static str,
    pub rounds: usize,
    pub rows: usize,
    pub cols: usize,
    pub trials: usize,
    pub full_rank: usize,
    pub mean_deficiency: f64,
    /// Deficiency histogram, index 4 meaning "4 or more".
    pub histogram: [usize; 5],
}

impl RankTrialSummary {
    pub fn full_rank_fraction(&self) -> f64 {
        self.full_rank as f64 / self.trials as f64
    }

    pub fn expected_full_rank_fraction(&self) -> f64 {
        full_rank_probability(self.rows, self.cols)
    }

    /// Standard normal score of the full-rank count against the binomial null.
    /// **Negative means more rank-deficient than chance** — the direction that
    /// indicates linear structure.
    pub fn z_score(&self) -> f64 {
        let p = self.expected_full_rank_fraction();
        let n = self.trials as f64;
        let sd = (n * p * (1.0 - p)).sqrt();
        if sd == 0.0 {
            return 0.0;
        }
        (self.full_rank as f64 - n * p) / sd
    }

    /// Whether the null is degenerate, i.e. full rank is so nearly certain that
    /// the test has no distribution to detect a shift in. **A summary in this
    /// regime cannot see weak correlation, only total collapse.**
    pub fn null_is_degenerate(&self) -> bool {
        self.expected_full_rank_fraction() > 0.99
    }
}

fn summarise(
    name: &'static str,
    rounds: usize,
    rows: usize,
    cols: usize,
    ranks: &[usize],
) -> RankTrialSummary {
    let mut histogram = [0usize; 5];
    let mut deficiency_total = 0usize;
    let mut full_rank = 0usize;
    for &r in ranks {
        let d = rows - r;
        histogram[d.min(4)] += 1;
        deficiency_total += d;
        if d == 0 {
            full_rank += 1;
        }
    }
    RankTrialSummary {
        name,
        rounds,
        rows,
        cols,
        trials: ranks.len(),
        full_rank,
        mean_deficiency: deficiency_total as f64 / ranks.len() as f64,
        histogram,
    }
}

/// Runs `trials` independent [`subspace_rank`] measurements and summarises them
/// against the null.
///
/// **Seeds are `seed_base .. seed_base + trials`, and callers comparing several
/// conditions must give each condition a disjoint range.** Sharing a seed set
/// across arms correlates them — a base state that yields a deficient matrix
/// tends to do so at every round count — so pooling shared-seed arms counts the
/// same accident repeatedly. That produced a spurious +3.4σ "anomaly" at deep
/// round counts before it was caught; see methodological item (11).
pub fn rank_trials<P: Permutation + ?Sized>(
    perm: &P,
    rounds: usize,
    input: &InputSet,
    samples: usize,
    trials: usize,
    seed_base: u64,
) -> RankTrialSummary {
    let ranks: Vec<usize> = (0..trials as u64)
        .map(|t| subspace_rank(perm, rounds, input, samples, seed_base + t).rank)
        .collect();
    summarise(perm.name(), rounds, samples, perm.state_bytes() * 8, &ranks)
}

/// The null-validation control: rank statistics of genuinely random matrices.
///
/// A standing check, not a one-off diagnostic. The identity and planted-matrix
/// controls prove the routine detects *collapse*; this proves it reproduces the
/// *distribution*, which is what the square-regime verdict rests on. Without it
/// there is no way to tell "the routine is wrong" from "the reading is wrong",
/// and that distinction was needed in practice.
pub fn random_matrix_rank_trials(n: usize, trials: usize, seed: u64) -> RankTrialSummary {
    let words = n.div_ceil(64);
    let mut probe = Probe::new(seed);
    let ranks: Vec<usize> = (0..trials)
        .map(|_| {
            let mut rows: Vec<Vec<u64>> = (0..n)
                .map(|_| (0..words).map(|_| probe.next_u64()).collect())
                .collect();
            gf2_rank(&mut rows, n)
        })
        .collect();
    summarise("random-matrix", 0, n, n, &ranks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::{ChaCha, Xoshiro256pp};

    /// Does nothing. Known answer: rank equals the subspace dimension exactly.
    struct Identity;
    impl Permutation for Identity {
        fn name(&self) -> &'static str {
            "identity"
        }
        fn state_bytes(&self) -> usize {
            64
        }
        fn default_rounds(&self) -> usize {
            1
        }
        fn round(&self, _state: &mut [u8], _r: usize) {}
    }

    /// xorshift64 per lane — GF(2)-linear and invertible, with a rank answer
    /// known before the test runs.
    struct PlantedLinear;
    impl Permutation for PlantedLinear {
        fn name(&self) -> &'static str {
            "planted-linear"
        }
        fn state_bytes(&self) -> usize {
            64
        }
        fn default_rounds(&self) -> usize {
            1
        }
        fn round(&self, state: &mut [u8], _r: usize) {
            for lane in state.chunks_exact_mut(8) {
                let mut x = u64::from_le_bytes(lane.try_into().unwrap());
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                lane.copy_from_slice(&x.to_le_bytes());
            }
        }
    }

    // -- the rank routine itself -------------------------------------------

    #[test]
    fn gf2_rank_matches_hand_computed_cases() {
        // Empty and zero matrices.
        assert_eq!(gf2_rank(&mut [], 64), 0);
        assert_eq!(gf2_rank(&mut [vec![0u64], vec![0u64]], 64), 0);
        // Two independent rows.
        assert_eq!(gf2_rank(&mut [vec![0b01u64], vec![0b10u64]], 64), 2);
        // Third row is the XOR of the first two — still rank 2.
        assert_eq!(
            gf2_rank(&mut [vec![0b01u64], vec![0b10u64], vec![0b11u64]], 64),
            2
        );
        // Duplicate rows.
        assert_eq!(gf2_rank(&mut [vec![0b101u64], vec![0b101u64]], 64), 1);
        // Spanning a second word.
        assert_eq!(
            gf2_rank(&mut [vec![0, 1u64], vec![1u64, 0], vec![1u64, 1]], 128),
            2
        );
    }

    #[test]
    fn gf2_rank_is_capped_by_both_dimensions() {
        let mut rows: Vec<Vec<u64>> = (0..8).map(|i| vec![1u64 << i]).collect();
        assert_eq!(gf2_rank(&mut rows, 4), 4, "capped by column count");
    }

    // -- known answers -----------------------------------------------------

    /// The identity map moves output differences exactly as far as input
    /// differences, so the rank is the subspace dimension. Any implementation
    /// that returned `min(m, n)` regardless would fail here.
    #[test]
    fn identity_collapses_rank_to_the_subspace_dimension() {
        for seed in [1u64, 2, 3, 4, 5] {
            let dirs = lane_bits(64, 1, 0, 16);
            let r = subspace_rank(&Identity, 1, &InputSet::Subspace(dirs), 64, seed);
            assert_eq!(r.rank, 16, "identity must give rank = k exactly");
            assert_eq!(r.deficiency(), 48);
            assert!(r.collapsed_to_subspace());
        }
    }

    /// A planted invertible GF(2) map: rank is preserved exactly, so the answer
    /// is again the subspace dimension and again known in advance.
    #[test]
    fn planted_linear_map_preserves_rank_exactly() {
        for seed in [1u64, 2, 3, 4, 5] {
            let dirs = lane_bits(64, 1, 0, 16);
            let r = subspace_rank(&PlantedLinear, 1, &InputSet::Subspace(dirs), 64, seed);
            assert_eq!(r.rank, 16, "an invertible linear map preserves rank");
        }
    }

    #[test]
    fn identity_and_planted_linear_have_zero_affine_residual() {
        for seed in [1u64, 2, 3] {
            assert_eq!(affine_residual(&Identity, 1, seed, 32), 0.0);
            assert_eq!(affine_residual(&PlantedLinear, 1, seed, 32), 0.0);
        }
    }

    /// The registry's real linear map. §5.5 established `xoshiro256++` as
    /// GF(2)-linear by a *separate* method — its avalanche cells are all exactly
    /// 0.0 or 1.0 with mean deviation exactly 0.5000. Two independent routes
    /// must agree, and this is the one that would expose a rank routine which
    /// only ever reports full rank.
    #[test]
    fn xoshiro_is_exactly_affine_by_both_statistics() {
        for seed in [1u64, 2, 3, 4, 5] {
            let dirs = lane_bits(32, 1, 0, 8);
            let r = subspace_rank(&Xoshiro256pp, 4, &InputSet::Subspace(dirs), 32, seed);
            assert!(
                r.collapsed_to_subspace(),
                "a GF(2)-linear map must collapse to its subspace dimension, got rank {} for k=8",
                r.rank
            );
            assert_eq!(
                affine_residual(&Xoshiro256pp, 4, seed, 32),
                0.0,
                "a linear map has an exactly zero affine residual"
            );
        }
    }

    /// The negative half of the bracket: a well-diffused permutation must show
    /// no rank deficiency at all. Without this the tests above would be
    /// satisfied by a routine that always reports a collapse.
    #[test]
    fn well_diffused_chacha_is_full_rank_and_maximally_nonaffine() {
        for seed in [1u64, 2, 3, 4, 5] {
            let dirs = lane_bits(64, 1, 0, 16);
            let r = subspace_rank(&ChaCha, 20, &InputSet::Subspace(dirs), 64, seed);
            assert_eq!(r.rank, 64, "20-round ChaCha must be full rank");
            assert_eq!(r.deficiency(), 0);
            let res = affine_residual(&ChaCha, 20, seed, 32);
            assert!(
                (res - 0.5).abs() < 0.05,
                "expected residual near 0.5, got {res}"
            );
        }
    }

    // -- the null itself, a standing control ------------------------------

    /// **The null-validation control.** Identity and planted-linear prove the
    /// routine detects *collapse*; this proves it reproduces the *distribution*,
    /// which is what every square-regime verdict rests on.
    ///
    /// Without it there is no way to separate "the routine is wrong" from "the
    /// reading is wrong", and that distinction was needed in practice: a
    /// spurious deep-round anomaly appeared and this control is what showed the
    /// fault was in the reading rather than the code.
    ///
    /// Run at n = 128 rather than 512 to stay cheap in debug builds; the
    /// product converges by n ≈ 20, so the target is the same 0.2888.
    #[test]
    fn random_matrices_reproduce_the_theoretical_rank_distribution() {
        let trials = 400;
        let s = random_matrix_rank_trials(128, trials, 0xC0FFEE);
        let expected = full_rank_probability(128, 128);
        assert!(
            (expected - 0.2888).abs() < 0.001,
            "theory should be 0.2888, got {expected}"
        );
        // 3.5 sigma either way: tight enough to catch a broken routine, loose
        // enough not to flake.
        let sd = (expected * (1.0 - expected) / trials as f64).sqrt();
        assert!(
            (s.full_rank_fraction() - expected).abs() < 3.5 * sd,
            "full-rank fraction {:.4} is not consistent with the null {expected:.4}",
            s.full_rank_fraction()
        );
        // Shape, not just the headline: deficiency 1 must be the mode, and
        // deficiency 3+ must be rare.
        assert!(
            s.histogram[1] > s.histogram[0] && s.histogram[1] > s.histogram[2],
            "deficiency 1 should be the mode, got {:?}",
            s.histogram
        );
        assert!(
            s.histogram[3] + s.histogram[4] < trials / 20,
            "deficiency 3+ should be rare, got {:?}",
            s.histogram
        );
    }

    #[test]
    fn full_rank_probability_matches_both_regimes() {
        // Square: the classic constant.
        assert!((full_rank_probability(512, 512) - 0.2888).abs() < 0.001);
        assert!((full_rank_probability(64, 64) - 0.2888).abs() < 0.001);
        // Tall-thin: the null is degenerate, which is exactly the trap.
        assert!(full_rank_probability(64, 512) > 0.999_999);
    }

    /// The regime warning must be machine-checkable, not just prose: a summary
    /// computed where the null is degenerate cannot see weak correlation.
    #[test]
    fn degenerate_null_is_flagged() {
        let tall = rank_trials(&ChaCha, 3, &InputSet::Stride(1), 64, 4, 1);
        assert!(
            tall.null_is_degenerate(),
            "64x512 must be flagged as a degenerate null"
        );
        let square = rank_trials(&ChaCha, 3, &InputSet::Stride(1), 512, 2, 1);
        assert!(
            !square.null_is_degenerate(),
            "512x512 must not be flagged as degenerate"
        );
    }

    /// The separability result, pinned: reduced-round ChaCha is weakly
    /// correlated but **not** affine on a small subspace. Full rank at 64×512
    /// from 2 rounds up, while the square regime finds it deficient at 3.
    #[test]
    fn reduced_round_chacha_is_weakly_correlated_but_not_affine() {
        for seed in [1u64, 2, 3, 4, 5] {
            let r = subspace_rank(&ChaCha, 3, &InputSet::Stride(1), 64, seed);
            assert_eq!(
                r.rank, 64,
                "3-round ChaCha is NOT affine on a 64-row subspace"
            );
        }
        // And at one round it is detectably close to affine, so the tall-thin
        // regime is not simply blind — it has a threshold, in the wrong place.
        let one = subspace_rank(&ChaCha, 1, &InputSet::Stride(1), 64, 1);
        assert!(
            one.rank < 64,
            "1-round ChaCha should be rank-deficient even at 64x512, got {}",
            one.rank
        );
    }

    #[test]
    fn direction_sets_have_the_requested_shape() {
        let low = lane_bits(64, 1, 0, 16);
        assert_eq!(low.len(), 16);
        assert!(low.iter().all(|m| m.iter().map(|b| b.count_ones()).sum::<u32>() == 1));
        // Lane 1 is bytes 8..16.
        assert!(low.iter().all(|m| m[8..16].iter().any(|&b| b != 0)));

        let spread = across_lanes(64, 16);
        assert_eq!(spread.len(), 16);
        let touched = spread
            .iter()
            .flat_map(|m| m.iter().enumerate().filter(|(_, &b)| b != 0).map(|(i, _)| i / 8))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(touched.len() > 1, "across_lanes must span several lanes");
    }
}
