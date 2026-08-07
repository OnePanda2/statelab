//! Seed-correlation and stream-independence battery — proposal §6.4 N1–N4.
//!
//! The proposal calls this "the failure class that decides T3" and it was
//! absent from the instrument entirely. Every battery already built measures a
//! permutation against *itself* — one state, one stream, one seed. None of them
//! can see a defect that only exists *between* streams, and that is precisely
//! where real generators have failed users: §3.7's catalogue is seeding
//! failures, not mixing failures.
//!
//! ## What each test is
//!
//! - **N1** streams from a seed set that deliberately contains
//!   Hamming-distance-1 pairs, checked for inter-stream correlation at equal
//!   block offsets.
//! - **N2** several seeds interleaved into one stream. Seed-lattice structure
//!   is invisible per-stream and shows up here as autocorrelation at a lag
//!   equal to the interleave width.
//! - **N3-DIFFUSION** the seed → first-output map treated as a function in its
//!   own right, measured with the same avalanche machinery used on
//!   permutations. It asks whether the *setup* diffuses, which no existing
//!   battery asks.
//! - **N4-POSITION** every output bit position as its own binary sequence — bias
//!   and autocorrelation — because low-bit weakness is classic and invisible in
//!   whole-word tests.
//!
//! ## Why N3 and N4 carry suffixes and N1/N2 do not
//!
//! The proposal's whole text on N3 and N4 is two clauses inside one sentence in
//! §6.4 — no metric, no threshold, no procedure — and each admits **two**
//! defensible readings that are not redundant with one another. The names here
//! say which reading is implemented, because an earlier version of this module
//! called them plainly "N3" and "N4" and so implied the clause had been
//! discharged when half of it had not.
//!
//! | clause | this module | elsewhere |
//! |---|---|---|
//! | N3 "as a function in its own right" | `N3-DIFFUSION` — *does it diffuse* | `N3-STATISTICAL` — vary the seed, take output[0] from each, run a battery over the concatenation. Catches seed-space clustering this is blind to. |
//! | N4 "each bit position independently" | `N4-POSITION` — per-position bias and autocorrelation | `N4-REVERSAL` — the reversal half, which is a no-op for any position-wise statistic and only means something against an order-sensitive battery. |
//!
//! `N3-STATISTICAL` and `N4-REVERSAL` need an external battery, so they live in
//! `examples/` rather than here. **`N4-POSITION` does not answer the reversal
//! question and must not be cited as though it does** — see
//! [`bit_position_profile`], which proves why it cannot.
//!
//! ## The statistic, and the trap it inherits
//!
//! All four reduce to the same shape: a grid of probabilities that should every
//! one of them be 0.5, summarised by the *maximum* deviation. That is the exact
//! shape that produced this project's first confidently wrong number — the
//! maximum of many noisy estimates sits several standard errors above the mean,
//! so an under-sampled grid fails no matter how good the design is. The fix is
//! the same fix: [`crate::avalanche::noise_floor`], and no verdict is reported
//! without [`DeviationGrid::sampling_is_adequate`] alongside it.
//!
//! ## What is measured
//!
//! The raw stream, through [`crate::stream`], so these numbers describe the
//! same object an external battery is fed. Extraction mode and input
//! construction both come from the caller's [`StreamConfig`] and are reported
//! with every result — declaring only the first is what once made a headline
//! conditional on an input nobody had varied.
//!
//! ## One input setting does not serve all four
//!
//! N1, N2 and N4-POSITION want **keyed** input: it is the realistic deployed case, and a
//! zero-filled state hands every design an artificially hard problem.
//!
//! N3-DIFFUSION wants the **opposite**, and this cost a wrong table before it was caught.
//! The keyed construction expands the seed through SplitMix64 on its way to the
//! state tail, so a keyed N3-DIFFUSION measures that expansion rather than the
//! permutation — the input-side twin of the output-side extraction trap this
//! project already proved. Run N3-DIFFUSION with `zero_frac: 1.0`. The reasoning, the two
//! discriminating measurements, and the one direction keyed N3-DIFFUSION *does* still see
//! are all in [`seed_diffusion`]'s documentation.

use crate::avalanche::{samples_for_cells, Probe};
use crate::permutation::Permutation;
use crate::stream::{emit_block, StreamConfig};

/// A grid of probabilities every one of which should be 0.5 under the null
/// hypothesis that the streams are independent and unbiased.
#[derive(Debug, Clone)]
pub struct DeviationGrid {
    pub rows: usize,
    pub cols: usize,
    /// Trials behind each cell. One value for the whole grid on purpose: a grid
    /// whose cells rest on different sample counts has no single noise floor,
    /// so the callers here truncate to a common count instead.
    pub samples: usize,
    /// Row-major, `rows * cols` entries.
    pub p: Vec<f64>,
}

impl DeviationGrid {
    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.p[row * self.cols + col]
    }

    /// Largest deviation from 0.5 anywhere. The strict criterion: one
    /// correlated bit position fails it, as it should.
    pub fn max_deviation(&self) -> f64 {
        self.p
            .iter()
            .map(|v| (v - 0.5).abs())
            .fold(0.0f64, f64::max)
    }

    /// Mean absolute deviation — the aggregate view, which hides isolated
    /// defects and so is always reported next to the maximum, never instead.
    pub fn mean_deviation(&self) -> f64 {
        let sum: f64 = self.p.iter().map(|v| (v - 0.5).abs()).sum();
        sum / self.p.len() as f64
    }

    /// `(row, col, deviation)` of the worst cell — the diagnostic that turns
    /// "this failed" into "this failed *here*", which is what identified the
    /// block-counter lane as the mechanism behind the counter's N1 result.
    pub fn worst_cell(&self) -> (usize, usize, f64) {
        let mut best = (0usize, 0usize, 0.0f64);
        for r in 0..self.rows {
            for c in 0..self.cols {
                let d = (self.get(r, c) - 0.5).abs();
                if d > best.2 {
                    best = (r, c, d);
                }
            }
        }
        best
    }

    /// Expected largest purely-random deviation for a grid this size.
    pub fn noise_floor(&self) -> f64 {
        crate::avalanche::noise_floor(self.samples, self.p.len())
    }

    /// Whether `tolerance` is distinguishable from sampling noise here.
    ///
    /// **A verdict without this is not a verdict.** Reported by every driver.
    pub fn sampling_is_adequate(&self, tolerance: f64) -> bool {
        self.noise_floor() <= tolerance
    }

    /// True iff every cell is within `tolerance` of 0.5. Meaningless unless
    /// [`Self::sampling_is_adequate`] also holds.
    pub fn is_independent(&self, tolerance: f64) -> bool {
        self.max_deviation() <= tolerance
    }

    /// Mean deviation at each offset within a `lane_bits`-wide lane, averaged
    /// over lanes and rows. Returns `lane_bits` values.
    ///
    /// The low-bit lens. A T-function's output bit *i* depends only on input
    /// bits `0..=i`, so its damage concentrates at small offsets, and a
    /// whole-word statistic averages that away.
    ///
    /// **Why the mean and not the maximum**, given that everywhere else here
    /// reports the maximum: a maximum over lanes saturates. One stuck lane
    /// contributes 0.5 at *every* offset, which flattens the profile to a
    /// constant and erases exactly the structure this lens exists to show.
    /// That is not hypothetical — it is what the first version of this function
    /// did, and it reported a T-function's offset 0 and offset 63 as equally
    /// bad when the whole point was to separate them. The maximum remains
    /// available through [`Self::max_deviation`]; this view is deliberately the
    /// aggregate one.
    pub fn lane_offset_profile(&self, lane_bits: usize) -> Vec<f64> {
        let mut sums = vec![0.0f64; lane_bits];
        let mut counts = vec![0usize; lane_bits];
        for r in 0..self.rows {
            for c in 0..self.cols {
                sums[c % lane_bits] += (self.get(r, c) - 0.5).abs();
                counts[c % lane_bits] += 1;
            }
        }
        sums.iter()
            .zip(&counts)
            .map(|(s, &n)| if n == 0 { 0.0 } else { s / n as f64 })
            .collect()
    }
}

/// Blocks needed before `tolerance` is distinguishable from noise on a grid of
/// `cells` cells. Thin alias so callers do not have to reach into `avalanche`.
pub fn recommended_blocks(cells: usize, tolerance: f64) -> usize {
    samples_for_cells(cells, tolerance)
}

/// The default seed set for N1.
///
/// Two groups, both required. Seeds 1..=8 are the proposal's literal "seeds
/// 1, 2, 3 … N" and already contain Hamming-distance-1 pairs among small
/// values. The rest place distance-1 pairs at the *top* and *middle* of the
/// word, because a construction that mixes the low bits of the seed and
/// neglects the high ones would pass the first group and fail users who seed
/// from a counter of process IDs or timestamps.
pub fn standard_seed_set() -> Vec<u64> {
    vec![
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        0x8000_0000_0000_0000,
        0x8000_0000_0000_0001,
        0x0000_0001_0000_0000,
        0x0000_0001_0000_0001,
    ]
}

/// Hamming distance between two seeds, for reporting which pairs are the
/// adversarial ones.
pub fn seed_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Collects `blocks` blocks of output for one seed.
fn collect_stream<P: Permutation + ?Sized>(perm: &P, cfg: &StreamConfig, blocks: usize) -> Vec<u8> {
    let out_bytes = cfg.extract.output_bytes(perm.state_bytes());
    let mut scratch = vec![0u8; perm.state_bytes()];
    let mut one = Vec::with_capacity(out_bytes);
    let mut all = Vec::with_capacity(out_bytes * blocks);
    for b in 0..blocks {
        emit_block(perm, cfg, b as u64, &mut scratch, &mut one);
        all.extend_from_slice(&one);
    }
    all
}

#[inline]
fn bit_at(buf: &[u8], bit: usize) -> bool {
    (buf[bit / 8] >> (bit % 8)) & 1 == 1
}

// ---------------------------------------------------------------------------
// N1 — inter-stream correlation
// ---------------------------------------------------------------------------

/// N1: do two streams seeded differently agree more often than chance at the
/// same block offset?
#[derive(Debug, Clone)]
pub struct SeedPairCorrelation {
    pub name: &'static str,
    pub seeds: Vec<u64>,
    /// Index pairs into `seeds`, in grid-row order.
    pub pairs: Vec<(usize, usize)>,
    /// Rows are seed pairs, columns are bit positions within a block.
    pub grid: DeviationGrid,
    pub blocks: usize,
    pub rounds: usize,
    pub zero_frac: f64,
}

impl SeedPairCorrelation {
    /// The worst pair, as `(seed_a, seed_b, hamming_distance, bit, deviation)`.
    pub fn worst(&self) -> (u64, u64, u32, usize, f64) {
        let (row, col, dev) = self.grid.worst_cell();
        let (i, j) = self.pairs[row];
        let (a, b) = (self.seeds[i], self.seeds[j]);
        (a, b, seed_distance(a, b), col, dev)
    }
}

/// Runs N1 over every unordered pair drawn from `seeds`.
///
/// Each stream is generated once and then compared pairwise, so cost is linear
/// in the seed count for generation and quadratic only in the (cheap) bit
/// comparison.
pub fn seed_pair_correlation<P: Permutation + ?Sized>(
    perm: &P,
    cfg: &StreamConfig,
    seeds: &[u64],
    blocks: usize,
) -> SeedPairCorrelation {
    assert!(seeds.len() >= 2, "N1 needs at least two seeds");
    assert!(blocks > 0, "N1 needs at least one block");

    let out_bytes = cfg.extract.output_bytes(perm.state_bytes());
    let cols = out_bytes * 8;

    let streams: Vec<Vec<u8>> = seeds
        .iter()
        .map(|&s| collect_stream(perm, &StreamConfig { seed: s, ..*cfg }, blocks))
        .collect();

    let mut pairs = Vec::new();
    for i in 0..seeds.len() {
        for j in (i + 1)..seeds.len() {
            pairs.push((i, j));
        }
    }

    let mut p = vec![0.0f64; pairs.len() * cols];
    for (row, &(i, j)) in pairs.iter().enumerate() {
        let (sa, sb) = (&streams[i], &streams[j]);
        for c in 0..cols {
            let mut agree = 0usize;
            for b in 0..blocks {
                let bit = b * cols + c;
                if bit_at(sa, bit) == bit_at(sb, bit) {
                    agree += 1;
                }
            }
            p[row * cols + c] = agree as f64 / blocks as f64;
        }
    }

    SeedPairCorrelation {
        name: perm.name(),
        seeds: seeds.to_vec(),
        grid: DeviationGrid {
            rows: pairs.len(),
            cols,
            samples: blocks,
            p,
        },
        pairs,
        blocks,
        rounds: cfg.effective_rounds(perm),
        zero_frac: cfg.zero_frac,
    }
}

// ---------------------------------------------------------------------------
// N3-DIFFUSION — the seed-to-first-output map, as a function in its own right
// ---------------------------------------------------------------------------

/// N3-DIFFUSION: flip one bit of the seed, and see which bits of the produced block move.
///
/// This is [`crate::avalanche::avalanche_matrix`] with the input moved from the
/// permutation's state to the *seed*, so it measures setup and permutation
/// together — which is what a caller who reseeds actually gets, and what no
/// other battery here looks at.
///
/// # The input-side extraction trap — read before choosing `zero_frac`
///
/// **N3-DIFFUSION is the one test in this module that must NOT be run on keyed input.**
///
/// The keyed construction reaches the state tail by expanding the seed through
/// SplitMix64, and SplitMix64 is a strong nonlinear mixer. It sits *between the
/// seed and the permutation*. Run N3-DIFFUSION keyed and a good number means the key
/// schedule diffused, which it does whether the permutation is ChaCha or a
/// wet paper bag.
///
/// This is not a worry, it is measured. Two discriminators, both in the tests
/// below:
///
/// - ChaCha at **one round** reads 0.0615 keyed and 0.5000 zero-filled. One
///   round cannot diffuse a seed across a 512-bit state; the clean keyed number
///   is the expansion.
/// - `xoshiro256++` reads 0.0674 keyed and 0.5000 zero-filled with a mean
///   deviation of *exactly* 0.5000 — the all-cells-0.0-or-1.0 signature of a
///   GF(2)-linear map, entirely hidden by the keyed reading.
///
/// So this is the exact twin of the trap this project already proved on the
/// output side, where a counter plus a strong finaliser passes every battery in
/// existence. A strong function on either side of a weak state map hides it.
/// The output-side version was found first and named; this is the input-side
/// version, and it is worth stating that the *honest* input for N1, N2 and N4-POSITION
/// is the *misleading* input for N3-DIFFUSION.
///
/// Pass `zero_frac: 1.0` to isolate the permutation. Keyed N3-DIFFUSION remains worth
/// reporting — it is the realistic deployed path — but only alongside the
/// isolated number, never instead of it.
///
/// One asymmetry the tests pin: keyed N3-DIFFUSION still catches *lane-local* designs
/// like `lcg`, because a permutation that never mixes across lanes leaves
/// output lane 0 a function of the raw seed alone, with no expansion mixed in.
/// It is blind specifically to designs that mix lanes but do so weakly.
#[derive(Debug, Clone)]
pub struct SeedDiffusion {
    pub name: &'static str,
    /// Rows are the 64 seed bits, columns bit positions in the output block.
    pub grid: DeviationGrid,
    /// Which block the map is evaluated at. Block 0 is the interesting one —
    /// it is what a freshly seeded generator hands out first.
    pub block: u64,
    pub rounds: usize,
    pub zero_frac: f64,
}

/// Runs N3-DIFFUSION, averaging over `samples` random base seeds drawn from `probe_seed`.
///
/// Averaging over base seeds is what turns a deterministic map into a
/// probability, exactly as random base states do for the permutation avalanche.
pub fn seed_diffusion<P: Permutation + ?Sized>(
    perm: &P,
    cfg: &StreamConfig,
    block: u64,
    samples: usize,
    probe_seed: u64,
) -> SeedDiffusion {
    assert!(samples > 0, "N3-DIFFUSION needs at least one base seed");
    const SEED_BITS: usize = 64;

    let out_bytes = cfg.extract.output_bytes(perm.state_bytes());
    let cols = out_bytes * 8;
    let mut counts = vec![0u32; SEED_BITS * cols];

    let mut probe = Probe::new(probe_seed);
    let mut scratch = vec![0u8; perm.state_bytes()];
    let mut a = Vec::with_capacity(out_bytes);
    let mut b = Vec::with_capacity(out_bytes);

    for _ in 0..samples {
        let base = probe.next_u64();
        for i in 0..SEED_BITS {
            let flipped = base ^ (1u64 << i);
            emit_block(
                perm,
                &StreamConfig { seed: base, ..*cfg },
                block,
                &mut scratch,
                &mut a,
            );
            emit_block(
                perm,
                &StreamConfig {
                    seed: flipped,
                    ..*cfg
                },
                block,
                &mut scratch,
                &mut b,
            );
            let row = &mut counts[i * cols..(i + 1) * cols];
            for (j, slot) in row.iter_mut().enumerate() {
                if bit_at(&a, j) != bit_at(&b, j) {
                    *slot += 1;
                }
            }
        }
    }

    let p = counts
        .iter()
        .map(|&c| f64::from(c) / samples as f64)
        .collect();

    SeedDiffusion {
        name: perm.name(),
        grid: DeviationGrid {
            rows: SEED_BITS,
            cols,
            samples,
            p,
        },
        block,
        rounds: cfg.effective_rounds(perm),
        zero_frac: cfg.zero_frac,
    }
}

// ---------------------------------------------------------------------------
// N4-POSITION — every bit position as its own sequence
// ---------------------------------------------------------------------------

/// N4-POSITION: bias and autocorrelation, measured per bit position rather than per word.
#[derive(Debug, Clone)]
pub struct BitPositionProfile {
    pub name: &'static str,
    /// One row; columns are bit positions. Cell is `P(bit = 1)`.
    pub bias: DeviationGrid,
    /// Rows are lags `1..=max_lag`, columns bit positions. Cell is the
    /// probability that position `c` agrees with itself `r+1` blocks later.
    pub autocorr: DeviationGrid,
    pub blocks: usize,
    pub max_lag: usize,
    pub rounds: usize,
    pub zero_frac: f64,
}

impl BitPositionProfile {
    /// The larger of the two headline deviations — the number to quote.
    pub fn max_deviation(&self) -> f64 {
        self.bias.max_deviation().max(self.autocorr.max_deviation())
    }

    /// Adequate only if *both* grids are.
    pub fn sampling_is_adequate(&self, tolerance: f64) -> bool {
        self.bias.sampling_is_adequate(tolerance) && self.autocorr.sampling_is_adequate(tolerance)
    }
}

/// Runs N4-POSITION on a single stream.
///
/// **This is the position half of §6.4's N4 clause, and only that half.**
///
/// The clause reads "reversed bit order **and** each bit position
/// independently". This implements the second conjunct. It cannot implement the
/// first, and the reason is a proof rather than a preference: every statistic
/// here is computed per position and then maximised, so reversing bit order
/// merely permutes *which position carries which measurement* and the maximum
/// is exactly invariant. Running a reversed pass here would return an identical
/// number and manufacture a false sense of coverage — which is why the test
/// `n4_is_invariant_under_bit_reversal_as_documented` pins the invariance
/// instead of pretending to test reversal.
///
/// Reversal is only meaningful against an **order-sensitive** battery. That is
/// `N4-REVERSAL`, run externally through PractRand via
/// [`StreamConfig::bit_reverse`] and `statelab-stream --bit-reverse`.
///
/// **Do not cite `N4-POSITION` as having answered the reversal question.**
pub fn bit_position_profile<P: Permutation + ?Sized>(
    perm: &P,
    cfg: &StreamConfig,
    blocks: usize,
    max_lag: usize,
) -> BitPositionProfile {
    assert!(max_lag >= 1, "N4-POSITION needs at least one lag");
    assert!(
        blocks > max_lag * 2,
        "N4-POSITION needs blocks well beyond max_lag; got {blocks} blocks for lag {max_lag}"
    );

    let out_bytes = cfg.extract.output_bytes(perm.state_bytes());
    let cols = out_bytes * 8;
    let stream = collect_stream(perm, cfg, blocks);
    profile_from_blocks(perm.name(), &stream, cols, blocks, max_lag, cfg)
}

/// The N4-POSITION statistic over an already-materialised block sequence.
///
/// Split out so N2 can feed it an interleaved stream without regenerating it.
fn profile_from_blocks(
    name: &'static str,
    stream: &[u8],
    cols: usize,
    blocks: usize,
    max_lag: usize,
    cfg: &StreamConfig,
) -> BitPositionProfile {
    let mut bias = vec![0.0f64; cols];
    for (c, slot) in bias.iter_mut().enumerate() {
        let ones = (0..blocks)
            .filter(|&b| bit_at(stream, b * cols + c))
            .count();
        *slot = ones as f64 / blocks as f64;
    }

    // Every lag is measured over the same number of block pairs. Letting lag 1
    // use more pairs than lag `max_lag` would leave the grid without a single
    // honest sample count, and therefore without a single noise floor.
    let pairs = blocks - max_lag;
    let mut auto = vec![0.0f64; max_lag * cols];
    for lag in 1..=max_lag {
        for c in 0..cols {
            let mut agree = 0usize;
            for b in 0..pairs {
                if bit_at(stream, b * cols + c) == bit_at(stream, (b + lag) * cols + c) {
                    agree += 1;
                }
            }
            auto[(lag - 1) * cols + c] = agree as f64 / pairs as f64;
        }
    }

    BitPositionProfile {
        name,
        bias: DeviationGrid {
            rows: 1,
            cols,
            samples: blocks,
            p: bias,
        },
        autocorr: DeviationGrid {
            rows: max_lag,
            cols,
            samples: pairs,
            p: auto,
        },
        blocks,
        max_lag,
        rounds: cfg.rounds,
        zero_frac: cfg.zero_frac,
    }
}

// ---------------------------------------------------------------------------
// N2 — interleaved multi-seed streams
// ---------------------------------------------------------------------------

/// N2: `n` seeds emitted round-robin into one stream.
///
/// The mechanism this exposes: with `n` streams interleaved, block *b* of
/// stream *i* and block *b* of stream *j* land `j - i` apart in the combined
/// stream. Any correlation between the two therefore appears as autocorrelation
/// at a lag below `n` — a defect that is invisible when each stream is tested
/// alone, which is exactly what §6.4 says N2 is for.
#[derive(Debug, Clone)]
pub struct InterleavedStreams {
    pub name: &'static str,
    pub seeds: Vec<u64>,
    pub profile: BitPositionProfile,
}

impl InterleavedStreams {
    /// Whether the worst autocorrelation lands at a lag strictly below the
    /// interleave width — the signature of *seed-lattice* structure rather than
    /// ordinary within-stream structure.
    pub fn worst_lag_is_cross_stream(&self) -> bool {
        let (row, _, _) = self.profile.autocorr.worst_cell();
        row + 1 < self.seeds.len()
    }
}

/// Builds the interleaved stream and runs the N4-POSITION statistic on it.
///
/// `blocks_per_seed` blocks are taken from each seed, so the combined stream is
/// `blocks_per_seed * seeds.len()` blocks long.
pub fn interleaved_streams<P: Permutation + ?Sized>(
    perm: &P,
    cfg: &StreamConfig,
    seeds: &[u64],
    blocks_per_seed: usize,
) -> InterleavedStreams {
    assert!(seeds.len() >= 2, "N2 needs at least two seeds");
    let out_bytes = cfg.extract.output_bytes(perm.state_bytes());
    let cols = out_bytes * 8;

    let streams: Vec<Vec<u8>> = seeds
        .iter()
        .map(|&s| collect_stream(perm, &StreamConfig { seed: s, ..*cfg }, blocks_per_seed))
        .collect();

    let total_blocks = blocks_per_seed * seeds.len();
    let mut woven = Vec::with_capacity(total_blocks * out_bytes);
    for b in 0..blocks_per_seed {
        for s in &streams {
            woven.extend_from_slice(&s[b * out_bytes..(b + 1) * out_bytes]);
        }
    }

    // Sweep past the interleave width so a cross-stream lag and an ordinary
    // within-stream lag (which is `seeds.len()` in the woven stream) can be
    // told apart rather than conflated.
    let max_lag = seeds.len() * 2;
    let profile = profile_from_blocks(perm.name(), &woven, cols, total_blocks, max_lag, cfg);

    InterleavedStreams {
        name: perm.name(),
        seeds: seeds.to_vec(),
        profile,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::Extract;
    use crate::systems::{ChaCha, Counter, KlimovShamir, Lcg, Xoshiro256pp};

    fn keyed(seed: u64) -> StreamConfig {
        StreamConfig {
            seed,
            ..StreamConfig::default()
        }
    }

    /// Sample counts chosen so the noise floor is comfortably under the
    /// tolerance. Deliberately not "a round number that felt like enough" —
    /// that is how the avalanche battery first went wrong.
    const TOL: f64 = 0.12;

    // -- N1 ----------------------------------------------------------------

    /// Positive control. ChaCha's streams from different seeds must be
    /// independent; if this fails the battery is not measuring independence.
    #[test]
    fn n1_chacha_streams_are_independent() {
        let seeds = standard_seed_set();
        let pairs = seeds.len() * (seeds.len() - 1) / 2;
        let blocks = recommended_blocks(pairs * 512, TOL).max(512);
        let r = seed_pair_correlation(&ChaCha, &keyed(0), &seeds, blocks);
        assert!(
            r.grid.sampling_is_adequate(TOL),
            "under-sampled: floor {:.4} > tolerance {TOL}",
            r.grid.noise_floor()
        );
        assert!(
            r.grid.is_independent(TOL),
            "ChaCha N1 failed: max deviation {:.4} at {:?}",
            r.grid.max_deviation(),
            r.worst()
        );
    }

    /// Negative control, and the sharper half of the bracket. A counter barely
    /// touches its state, so two streams at the same block offset share the
    /// block-counter lane verbatim — they agree there in every single block.
    /// A battery that does not scream at this is measuring nothing.
    #[test]
    fn n1_counter_streams_are_maximally_correlated() {
        let seeds = vec![1u64, 2, 3];
        let r = seed_pair_correlation(&Counter::default(), &keyed(0), &seeds, 256);
        assert!(
            r.grid.max_deviation() > 0.45,
            "a counter's streams must be visibly correlated, got {:.4}",
            r.grid.max_deviation()
        );
        // And the mechanism, not just the verdict: the block-counter lane —
        // bits 64..128 — is shared verbatim between any two streams at the same
        // offset, so *every* bit in it agrees in *every* block.
        //
        // Asserted as "the whole lane is maximal", not "the worst cell lies in
        // this lane". The seed lane is equally maximal here (a constant seed
        // makes those bits agree too), so the argmax is a tie broken by scan
        // order and asserting on it tests nothing. That over-specific version
        // is what this test said first, and it failed for a reason that had
        // nothing to do with the claim.
        for row in 0..r.grid.rows {
            for bit in 64..128 {
                let dev = (r.grid.get(row, bit) - 0.5).abs();
                assert!(
                    dev > 0.45,
                    "block-counter bit {bit} of pair {row} should agree always, dev {dev:.4}"
                );
            }
        }
    }

    #[test]
    fn n1_grid_shape_matches_the_pair_count() {
        let seeds = vec![1u64, 2, 3, 4];
        let r = seed_pair_correlation(&ChaCha, &keyed(0), &seeds, 64);
        assert_eq!(r.pairs.len(), 6);
        assert_eq!(r.grid.rows, 6);
        assert_eq!(r.grid.cols, 512);
        assert_eq!(r.grid.p.len(), 6 * 512);
    }

    /// The seed set must actually contain the adversarial case it claims to.
    #[test]
    fn standard_seed_set_contains_distance_one_pairs_across_the_word() {
        let s = standard_seed_set();
        let mut low = false;
        let mut mid = false;
        let mut high = false;
        for i in 0..s.len() {
            for j in (i + 1)..s.len() {
                if seed_distance(s[i], s[j]) == 1 {
                    let bit = (s[i] ^ s[j]).trailing_zeros();
                    match bit {
                        0..=15 => low = true,
                        16..=47 => mid = true,
                        _ => high = true,
                    }
                }
            }
        }
        assert!(
            low && mid && high,
            "need distance-1 pairs at low, mid and high bits"
        );
    }

    // -- N3-DIFFUSION ----------------------------------------------------------------

    /// Positive control: one seed bit must move about half the output.
    ///
    /// Run isolated. A keyed run would pass here for the wrong reason and would
    /// therefore control nothing — as the one-round test below demonstrates.
    #[test]
    fn n3_chacha_seed_map_diffuses() {
        let samples = recommended_blocks(64 * 512, TOL);
        let r = seed_diffusion(&ChaCha, &isolated(0), 0, samples, 99);
        assert!(r.grid.sampling_is_adequate(TOL));
        assert!(
            r.grid.is_independent(TOL),
            "ChaCha N3-DIFFUSION failed: max deviation {:.4}",
            r.grid.max_deviation()
        );
    }

    /// Negative control: a counter passes the seed through to lane 0 almost
    /// untouched, so flipping seed bit *i* flips output bit *i* and nothing
    /// else. That is an identity, and it must be visible as such.
    #[test]
    fn n3_counter_seed_map_is_essentially_the_identity() {
        let r = seed_diffusion(&Counter::default(), &keyed(0), 0, 64, 5);
        assert!(
            r.grid.max_deviation() > 0.45,
            "a counter's seed map must not look diffusing, got {:.4}",
            r.grid.max_deviation()
        );
    }

    fn isolated(seed: u64) -> StreamConfig {
        StreamConfig {
            zero_frac: 1.0,
            ..keyed(seed)
        }
    }

    /// The input-side extraction trap, pinned.
    ///
    /// A permutation given one round cannot have diffused anything. If keyed N3-DIFFUSION
    /// calls that clean, keyed N3-DIFFUSION is measuring the key schedule — and this test
    /// exists so that conclusion cannot be quietly lost the way the noise floor
    /// nearly was.
    #[test]
    fn n3_keyed_input_calls_one_round_chacha_clean_which_is_impossible() {
        let one_round = StreamConfig {
            rounds: 1,
            ..keyed(0)
        };
        let k = seed_diffusion(&ChaCha, &one_round, 0, 256, 0x5EED);
        let z = seed_diffusion(
            &ChaCha,
            &StreamConfig {
                zero_frac: 1.0,
                ..one_round
            },
            0,
            256,
            0x5EED,
        );
        assert!(
            k.grid.max_deviation() < 0.2,
            "keyed N3-DIFFUSION is expected to look clean at one round — that is the trap"
        );
        assert!(
            z.grid.max_deviation() > 0.45,
            "isolated N3-DIFFUSION must expose one round as undiffused, got {:.4}",
            z.grid.max_deviation()
        );
    }

    /// The same trap on a design whose defect is already documented elsewhere.
    ///
    /// §5.5 records `xoshiro256++` as GF(2)-linear, with every avalanche cell
    /// exactly 0.0 or 1.0 and a mean deviation of exactly 0.5. Isolated N3-DIFFUSION must
    /// reproduce that signature bit for bit; keyed N3-DIFFUSION must miss it entirely.
    #[test]
    fn n3_isolated_reproduces_the_gf2_linear_signature() {
        let z = seed_diffusion(&Xoshiro256pp, &isolated(0), 0, 256, 0x5EED);
        assert_eq!(
            z.grid.mean_deviation(),
            0.5,
            "a linear seed map must put every cell at 0.0 or 1.0"
        );
        assert!(z.grid.p.iter().all(|&v| v == 0.0 || v == 1.0));

        let k = seed_diffusion(&Xoshiro256pp, &keyed(0), 0, 256, 0x5EED);
        assert!(
            k.grid.max_deviation() < 0.2,
            "keyed N3-DIFFUSION is expected to hide it — that is why the isolated run exists"
        );
    }

    /// The asymmetry worth knowing: keyed N3-DIFFUSION is not useless, it is blind in one
    /// specific direction. A lane-local design leaves output lane 0 a function
    /// of the raw seed alone, so no amount of key expansion elsewhere hides it.
    #[test]
    fn keyed_n3_still_catches_a_lane_local_design() {
        let r = seed_diffusion(&Lcg::default(), &keyed(0), 0, 256, 0x5EED);
        assert!(
            r.grid.max_deviation() > 0.45,
            "lane-local designs stay visible under keyed N3-DIFFUSION, got {:.4}",
            r.grid.max_deviation()
        );
    }

    // -- N4-POSITION ----------------------------------------------------------------

    #[test]
    fn n4_chacha_has_no_biased_or_autocorrelated_bit_position() {
        let blocks = recommended_blocks(8 * 512, TOL).max(1024);
        let r = bit_position_profile(&ChaCha, &keyed(1), blocks, 8);
        assert!(r.sampling_is_adequate(TOL));
        assert!(
            r.max_deviation() <= TOL,
            "ChaCha N4-POSITION failed: bias {:.4}, autocorr {:.4}",
            r.bias.max_deviation(),
            r.autocorr.max_deviation()
        );
    }

    /// A counter's seed lane is constant across every block of a stream, so
    /// those positions are stuck — maximal bias and maximal autocorrelation.
    #[test]
    fn n4_counter_has_stuck_bit_positions() {
        let r = bit_position_profile(&Counter::default(), &keyed(1), 1024, 8);
        assert!(r.bias.max_deviation() > 0.45);
        assert!(r.autocorr.max_deviation() > 0.45);
    }

    /// A lane-local design in counter mode leaves almost the whole output
    /// frozen, and N4-POSITION must see it.
    ///
    /// Counter mode rebuilds the state per block as `seed || block || keyed`,
    /// so only lane 1 changes from one block to the next. A permutation that
    /// never mixes across lanes therefore emits six constant lanes out of
    /// eight, and every bit in them is stuck. ChaCha has no stuck bit at all.
    #[test]
    fn n4_flags_a_lane_local_design_as_mostly_frozen() {
        let cfg = StreamConfig {
            seed: 1,
            rounds: 4,
            ..StreamConfig::default()
        };
        let r = bit_position_profile(&KlimovShamir::default(), &cfg, 1024, 4);
        let stuck = r.bias.p.iter().filter(|v| (*v - 0.5).abs() > 0.45).count();
        assert!(
            stuck as f64 / r.bias.p.len() as f64 > 0.7,
            "expected most bit positions frozen, got {stuck}/{}",
            r.bias.p.len()
        );
    }

    /// The reason N4-POSITION exists: a T-function's output bit *i* depends only on
    /// input bits `0..=i`, so its weakness concentrates in the low offsets.
    ///
    /// Measured on lane 1 alone. That restriction is not tuning — in counter
    /// mode lane 1 is the *only* lane whose input varies between blocks, so it
    /// is the only place a lane-local design's triangularity is observable at
    /// all. The frozen lanes carry no offset information, and averaging them in
    /// dilutes the signal with a constant.
    #[test]
    fn n4_flags_low_bits_of_a_t_function_in_the_counter_lane() {
        let cfg = StreamConfig {
            seed: 1,
            rounds: 4,
            ..StreamConfig::default()
        };
        let r = bit_position_profile(&KlimovShamir::default(), &cfg, 1024, 4);
        let rows = r.autocorr.rows;
        let counter_lane = DeviationGrid {
            rows,
            cols: 64,
            samples: r.autocorr.samples,
            p: (0..rows)
                .flat_map(|row| (64..128).map(move |c| (row, c)))
                .map(|(row, c)| r.autocorr.get(row, c))
                .collect(),
        };
        let offsets = counter_lane.lane_offset_profile(64);
        assert!(
            offsets[0] > 0.45,
            "bit 0 of the counter lane should be perfectly predictable, got {:.4}",
            offsets[0]
        );
        assert!(
            offsets[0] > offsets[63],
            "expected low bits worse than high bits: {:.4} vs {:.4}",
            offsets[0],
            offsets[63]
        );
    }

    #[test]
    fn n4_lag_grids_share_one_sample_count() {
        let r = bit_position_profile(&ChaCha, &keyed(1), 512, 8);
        assert_eq!(r.autocorr.samples, 512 - 8);
        assert_eq!(r.autocorr.rows, 8);
        assert_eq!(r.bias.samples, 512);
        assert_eq!(r.bias.rows, 1);
    }

    /// Stated in `bit_position_profile`'s documentation and worth pinning:
    /// reversal permutes positions, so the maximised statistic cannot move.
    #[test]
    fn n4_is_invariant_under_bit_reversal_as_documented() {
        let plain = keyed(1);
        let reversed = StreamConfig {
            bit_reverse: true,
            ..plain
        };
        let a = bit_position_profile(&ChaCha, &plain, 512, 4);
        let b = bit_position_profile(&ChaCha, &reversed, 512, 4);
        assert_eq!(a.bias.max_deviation(), b.bias.max_deviation());
        assert_eq!(a.autocorr.max_deviation(), b.autocorr.max_deviation());
    }

    // -- N2 ----------------------------------------------------------------

    #[test]
    fn n2_chacha_interleaves_cleanly() {
        let seeds = vec![1u64, 2, 3, 4];
        let r = interleaved_streams(&ChaCha, &keyed(0), &seeds, 512);
        assert!(r.profile.sampling_is_adequate(TOL));
        assert!(
            r.profile.max_deviation() <= TOL,
            "ChaCha N2 failed: {:.4}",
            r.profile.max_deviation()
        );
        assert_eq!(r.profile.blocks, 512 * 4);
    }

    /// Interleaving a counter puts correlated blocks a short lag apart, and the
    /// worst lag must land below the interleave width — the signature that
    /// distinguishes seed-lattice structure from ordinary within-stream
    /// structure.
    #[test]
    fn n2_counter_shows_cross_stream_structure() {
        let seeds = vec![1u64, 2, 3, 4];
        let r = interleaved_streams(&Counter::default(), &keyed(0), &seeds, 256);
        assert!(r.profile.max_deviation() > 0.45);
        assert!(
            r.worst_lag_is_cross_stream(),
            "expected the worst lag below the interleave width"
        );
    }

    // -- shared machinery --------------------------------------------------

    #[test]
    fn worst_cell_finds_the_planted_outlier() {
        let mut g = DeviationGrid {
            rows: 3,
            cols: 4,
            samples: 100,
            p: vec![0.5; 12],
        };
        g.p[2 * 4 + 1] = 0.9;
        assert_eq!(g.worst_cell(), (2, 1, 0.4));
        assert_eq!(g.max_deviation(), 0.4);
    }

    #[test]
    fn lane_offset_profile_averages_within_each_offset() {
        let g = DeviationGrid {
            rows: 1,
            cols: 4,
            samples: 100,
            // offsets 0 and 1 repeat at columns 2 and 3
            p: vec![0.5, 0.6, 0.8, 0.5],
        };
        let prof = g.lane_offset_profile(2);
        assert_eq!(prof.len(), 2);
        assert!((prof[0] - 0.15).abs() < 1e-12, "mean of 0.0 and 0.3");
        assert!((prof[1] - 0.05).abs() < 1e-12, "mean of 0.1 and 0.0");
    }

    /// The saturation that forced the mean: one stuck lane must not flatten the
    /// profile. Under a maximum it would read 0.5 at every offset.
    #[test]
    fn lane_offset_profile_survives_one_stuck_lane() {
        let g = DeviationGrid {
            rows: 1,
            cols: 4,
            samples: 100,
            // lane 0 (cols 0,1) fully stuck; lane 1 (cols 2,3) shows structure
            p: vec![1.0, 1.0, 0.9, 0.5],
        };
        let prof = g.lane_offset_profile(2);
        assert!(
            prof[0] > prof[1],
            "offset structure must survive a stuck lane: {:.4} vs {:.4}",
            prof[0],
            prof[1]
        );
    }

    /// The whole battery is worthless if it silently accepts an under-sampled
    /// grid, which is the failure that started this project's methodology.
    #[test]
    fn adequacy_rejects_an_undersampled_grid() {
        let g = DeviationGrid {
            rows: 66,
            cols: 512,
            samples: 24,
            p: vec![0.5; 66 * 512],
        };
        assert!(!g.sampling_is_adequate(0.05));
        assert!(
            g.max_deviation() <= 0.05,
            "the cells themselves are perfect"
        );
        // Perfect cells, inadequate sampling: the verdict must not be trusted.
        let n = recommended_blocks(66 * 512, 0.05);
        let g2 = DeviationGrid { samples: n, ..g };
        assert!(g2.sampling_is_adequate(0.05));
    }

    /// Low-byte extraction narrows the block, and every grid must follow.
    #[test]
    fn extraction_mode_changes_the_grid_width() {
        let cfg = StreamConfig {
            extract: Extract::LowByte,
            ..keyed(0)
        };
        let r = seed_pair_correlation(&ChaCha, &cfg, &[1, 2], 128);
        assert_eq!(r.grid.cols, 64, "8 lanes, one byte each");
    }
}
