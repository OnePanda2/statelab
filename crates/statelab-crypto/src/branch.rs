//! Branch number — the exact, statistics-free diffusion metric this project
//! never built.
//!
//! ## Why this exists
//!
//! `PHASE_L` §4.1 quotes the research corpus naming **branch number** as "a
//! *linear-algebra* property of the diffusion layer alone (computable exactly,
//! without any probabilistic/statistical estimation), making it one of the
//! field's cleanest, least-ambiguous diffusion metrics — and correspondingly
//! **one of the first things a new design's diffusion layer should be checked
//! against before any more expensive statistical/empirical testing.**"
//!
//! That was recorded on 2026-08-07 and never acted on. Every diffusion result
//! in this project — avalanche, BIC, GF(2) rank — is statistical. This is the
//! one metric that is not, and it is cheap.
//!
//! ## The definition
//!
//! For a map `F` on a state split into `b` bundles (here: 16 words of 32 bits),
//! the **differential branch number** is
//!
//! ```text
//!     B(F) = min over x != 0 of ( wt(x) + wt(F(x)) )
//! ```
//!
//! where `wt` counts **nonzero bundles**, not bits. `B = b + 1` is the maximum
//! and characterises an MDS layer. `B = 2` means some single-bundle input
//! produces a single-bundle output — no diffusion at all.
//!
//! ## *** WHAT THIS CAN AND CANNOT CONCLUDE — READ BEFORE USING A NUMBER ***
//!
//! ARX has no separate linear layer, so the round is **linearised**: modular
//! addition is replaced by XOR, which is the standard differential
//! approximation. Rotation and XOR are already GF(2)-linear, so the linearised
//! round is an exact GF(2)-linear map on 512 bits.
//!
//! Minimising bundle weight over all `2^512 - 1` nonzero inputs is the minimum
//! bundle-weight problem for a linear code, which is **NP-hard**. So this
//! module computes two quantities, and **BOTH ARE UPPER BOUNDS ON THE TRUE
//! BRANCH NUMBER**:
//!
//! * [`pattern_branch_number`] — exhaustive over all `2^16` word-activity
//!   patterns under a **no-cancellation** model. Cancellation can only *remove*
//!   active output words, so the true value is `<=` this.
//! * [`LinearMap::bitwise_branch_bound`] — exhaustive over all input
//!   differences of Hamming weight `<= w`, using the real linearised matrix
//!   **with** cancellation. Any achievable value is an upper bound on a
//!   minimum.
//!
//! **THE ASYMMETRY IS THE WHOLE POINT, AND IT IS WHY THIS IS WORTH RUNNING
//! FIRST: a LOW number is PROOF of a weakness. A MAXIMAL number PROVES
//! NOTHING.** This check can kill a design cheaply. It cannot bless one.
//! Anything that survives it still needs the differential and linear trail
//! bounds that only MILP/SAT — `PHASE_L`'s wall — can produce.

/// ChaCha's rotation constants.
pub const ROTS: [u32; 4] = [16, 12, 8, 7];

/// The number of 32-bit bundles in the state.
pub const BUNDLES: usize = 16;

/// Maximum attainable branch number on [`BUNDLES`] bundles.
pub const MAX_BRANCH: usize = BUNDLES + 1;

/// Number of nonzero 32-bit bundles — the weight branch number is defined on.
#[must_use]
pub fn word_weight(v: &[u32; 16]) -> usize {
    v.iter().filter(|w| **w != 0).count()
}

/// One `n`-step ARX quarter round with **modular addition replaced by XOR**.
///
/// This is the standard linearisation. It is exact for rotation and XOR and an
/// approximation for addition — the approximation the whole metric rests on,
/// stated here rather than buried.
pub fn linear_quarter(w: &mut [u32; 16], n: usize, a: usize, b: usize, c: usize, d: usize) {
    for i in 0..n {
        let r = ROTS[i % 4];
        if i % 2 == 0 {
            w[a] ^= w[b];
            w[d] = (w[d] ^ w[a]).rotate_left(r);
        } else {
            w[c] ^= w[d];
            w[b] = (w[b] ^ w[c]).rotate_left(r);
        }
    }
}

/// `rounds` rounds of the linearised ChaCha-shaped permutation with an `n`-step
/// quarter round, alternating column and diagonal wiring exactly as
/// `arx_step_cycles.rs` does.
pub fn linear_rounds(w: &mut [u32; 16], n: usize, rounds: usize) {
    for r in 0..rounds {
        if r % 2 == 0 {
            for k in 0..4 {
                linear_quarter(w, n, k, 4 + k, 8 + k, 12 + k);
            }
        } else {
            linear_quarter(w, n, 0, 5, 10, 15);
            linear_quarter(w, n, 1, 6, 11, 12);
            linear_quarter(w, n, 2, 7, 8, 13);
            linear_quarter(w, n, 3, 4, 9, 14);
        }
    }
}

/// A GF(2)-linear map on the 512-bit state, stored as the images of the 512
/// basis vectors.
pub struct LinearMap {
    cols: Vec<[u32; 16]>,
}

impl LinearMap {
    /// Builds the map for `rounds` rounds of an `n`-step design by pushing each
    /// of the 512 basis vectors through [`linear_rounds`].
    #[must_use]
    pub fn build(n: usize, rounds: usize) -> Self {
        let mut cols = Vec::with_capacity(512);
        for j in 0..512 {
            let mut x = [0u32; 16];
            x[j / 32] = 1u32 << (j % 32);
            linear_rounds(&mut x, n, rounds);
            cols.push(x);
        }
        Self { cols }
    }

    /// Applies the map by XORing the columns selected by the set bits of `x`.
    #[must_use]
    pub fn apply(&self, x: &[u32; 16]) -> [u32; 16] {
        let mut out = [0u32; 16];
        for j in 0..512 {
            if x[j / 32] >> (j % 32) & 1 == 1 {
                for (o, c) in out.iter_mut().zip(self.cols[j].iter()) {
                    *o ^= *c;
                }
            }
        }
        out
    }

    /// Word-level dependency matrix: bit `j` of entry `i` is set when output
    /// word `i` depends on **any** bit of input word `j`.
    #[must_use]
    pub fn dependency_matrix(&self) -> [u16; 16] {
        let mut a = [0u16; 16];
        for (j, col) in self.cols.iter().enumerate() {
            let in_word = j / 32;
            for (i, w) in col.iter().enumerate() {
                if *w != 0 {
                    a[i] |= 1 << in_word;
                }
            }
        }
        a
    }

    /// Exhaustive minimum of `wt(x) + wt(Mx)` over every input difference of
    /// Hamming weight `1..=max_weight`, using the real matrix **with**
    /// cancellation.
    ///
    /// An **upper bound** on the true branch number: every value it considers
    /// is achievable, but it does not consider every input.
    #[must_use]
    pub fn bitwise_branch_bound(&self, max_weight: usize) -> usize {
        let mut best = usize::MAX;
        let xor = |a: &[u32; 16], b: &[u32; 16]| -> [u32; 16] {
            let mut o = *a;
            for (x, y) in o.iter_mut().zip(b.iter()) {
                *x ^= *y;
            }
            o
        };
        // A weight-k bit pattern touches at most k words; count them exactly.
        let in_weight = |idx: &[usize]| -> usize {
            let mut mask = 0u16;
            for j in idx {
                mask |= 1 << (j / 32);
            }
            mask.count_ones() as usize
        };

        for j in 0..512 {
            best = best.min(1 + word_weight(&self.cols[j]));
        }
        if max_weight < 2 {
            return best;
        }
        for j in 0..512 {
            for k in (j + 1)..512 {
                let v = xor(&self.cols[j], &self.cols[k]);
                best = best.min(in_weight(&[j, k]) + word_weight(&v));
            }
        }
        if max_weight < 3 {
            return best;
        }
        for j in 0..512 {
            for k in (j + 1)..512 {
                let jk = xor(&self.cols[j], &self.cols[k]);
                for l in (k + 1)..512 {
                    let v = xor(&jk, &self.cols[l]);
                    best = best.min(in_weight(&[j, k, l]) + word_weight(&v));
                }
            }
        }
        best
    }
}

/// The minimising input difference for [`LinearMap::bitwise_branch_bound`] at
/// weight `<= 2`, returned as the raw 512-bit difference so a caller can
/// re-derive the claim by direct evaluation instead of trusting the matrix.
#[must_use]
pub fn bitwise_branch_witness(m: &LinearMap, n: usize, rounds: usize) -> ([u32; 16], usize) {
    let mut best = (usize::MAX, [0u32; 16]);
    let mut consider = |bits: &[usize]| {
        let mut x = [0u32; 16];
        for j in bits {
            x[j / 32] ^= 1u32 << (j % 32);
        }
        let mut y = x;
        linear_rounds(&mut y, n, rounds);
        let total = word_weight(&x) + word_weight(&y);
        if total < best.0 {
            best = (total, x);
        }
    };
    for j in 0..512 {
        consider(&[j]);
    }
    for j in 0..512 {
        for k in (j + 1)..512 {
            consider(&[j, k]);
        }
    }
    let _ = m;
    (best.1, best.0)
}

/// Exhaustive branch number over all `2^16 - 1` nonzero word-activity patterns
/// under a **no-cancellation** model: output word `i` is active when any input
/// word it depends on is active.
///
/// Cancellation can only remove active output words, so this is an **upper
/// bound** on the true branch number.
#[must_use]
pub fn pattern_branch_number(a: &[u16; 16]) -> usize {
    let mut best = usize::MAX;
    for v in 1u32..(1 << 16) {
        let v = v as u16;
        let mut out = 0u16;
        for (i, row) in a.iter().enumerate() {
            if row & v != 0 {
                out |= 1 << i;
            }
        }
        let total = v.count_ones() as usize + out.count_ones() as usize;
        best = best.min(total);
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_rounds_is_the_identity_and_has_branch_number_two() {
        let m = LinearMap::build(4, 0);
        // Identity: a one-word input gives a one-word output.
        assert_eq!(m.bitwise_branch_bound(1), 2);
        assert_eq!(pattern_branch_number(&m.dependency_matrix()), 2);
    }

    #[test]
    fn the_linearised_round_is_actually_linear() {
        // The metric is meaningless if the map it is computed on is not linear.
        let m = LinearMap::build(5, 3);
        let x = [0x9e37_79b9u32; 16];
        let mut y = [0u32; 16];
        for (i, w) in y.iter_mut().enumerate() {
            *w = 0x1234_5678u32.wrapping_mul(i as u32 + 1);
        }
        let mut xy = [0u32; 16];
        for i in 0..16 {
            xy[i] = x[i] ^ y[i];
        }
        let lhs = m.apply(&xy);
        let (fx, fy) = (m.apply(&x), m.apply(&y));
        let mut rhs = [0u32; 16];
        for i in 0..16 {
            rhs[i] = fx[i] ^ fy[i];
        }
        assert_eq!(lhs, rhs);
    }

    #[test]
    fn the_matrix_reproduces_direct_evaluation() {
        // Guards against the column basis being built with the wrong bit order.
        let m = LinearMap::build(3, 5);
        let mut x = [0u32; 16];
        for (i, w) in x.iter_mut().enumerate() {
            *w = 0xdead_beefu32.rotate_left(i as u32);
        }
        let via_matrix = m.apply(&x);
        let mut direct = x;
        linear_rounds(&mut direct, 3, 5);
        assert_eq!(via_matrix, direct);
    }

    #[test]
    fn pattern_branch_number_hits_its_known_endpoints() {
        // Every output depends on every input: any single active input word
        // activates all 16, so 1 + 16.
        let all = [0xffffu16; 16];
        assert_eq!(pattern_branch_number(&all), MAX_BRANCH);
        // A pure word permutation diffuses nothing.
        let mut perm = [0u16; 16];
        for (i, row) in perm.iter_mut().enumerate() {
            *row = 1 << ((i + 1) % 16);
        }
        assert_eq!(pattern_branch_number(&perm), 2);
    }

    #[test]
    fn one_column_round_confines_diffusion_to_its_group() {
        // A single column round leaves four disjoint groups of four, so the
        // best attainable is 1 + 4. This is a structural fact about the wiring
        // and it anchors the sweep in the example.
        let m = LinearMap::build(4, 1);
        let a = m.dependency_matrix();
        assert_eq!(pattern_branch_number(&a), 5);
        // Output word 0 must depend only on the column {0, 4, 8, 12}.
        assert_eq!(a[0], 1 << 0 | 1 << 4 | 1 << 8 | 1 << 12);
    }

    #[test]
    fn bounds_never_exceed_the_no_cancellation_model() {
        // Cancellation can only lower the weight, so the bit-level bound must
        // not come out above the pattern bound. A violation means one of the
        // two is computed wrongly.
        for n in [3usize, 4, 5] {
            for rounds in [1usize, 2, 3] {
                let m = LinearMap::build(n, rounds);
                let pat = pattern_branch_number(&m.dependency_matrix());
                let bit = m.bitwise_branch_bound(1);
                assert!(
                    bit <= pat,
                    "n={n} rounds={rounds}: bit bound {bit} exceeds pattern bound {pat}"
                );
            }
        }
    }
}
