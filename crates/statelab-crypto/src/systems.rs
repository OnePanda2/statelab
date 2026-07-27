//! Permutations under measurement.
//!
//! The set is chosen so that the instrument can be validated against known
//! outcomes before it is trusted on anything new (proposal §6.5):
//!
//! * [`Counter`] — a bijection with no diffusion whatsoever. The negative
//!   control required by §6.1 M4. If a battery cannot distinguish this from a
//!   real permutation, the battery is broken.
//! * [`ChaCha`] — the incumbent baseline, and the thing we are trying to beat.
//! * [`KlimovShamir`] — the canonical single-cycle T-function. The literature
//!   says it is bijective, single-cycle, and triangular. All three are
//!   checkable here, and the third is the defect that killed the family.
//! * [`KlimovShamirTransposed`] — the same map plus the byte transposition the
//!   literature identifies as the anti-triangular repair.

use crate::permutation::{mask, Permutation, SmallMap};

// ---------------------------------------------------------------------------
// Counter — the negative control
// ---------------------------------------------------------------------------

/// Increments the state as a little-endian integer. Perfectly bijective,
/// single-cycle, and useless: input bit *i* can never affect output bit *j*
/// for *j < i*, and carries reach high bits only with vanishing probability.
pub struct Counter {
    pub bytes: usize,
}

impl Default for Counter {
    fn default() -> Self {
        Self { bytes: 64 }
    }
}

impl Permutation for Counter {
    fn name(&self) -> &'static str {
        "counter"
    }
    fn state_bytes(&self) -> usize {
        self.bytes
    }
    fn default_rounds(&self) -> usize {
        1
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        for byte in state.iter_mut() {
            let (next, carry) = byte.overflowing_add(1);
            *byte = next;
            if !carry {
                return;
            }
        }
    }
}

impl SmallMap for Counter {
    fn name(&self) -> &'static str {
        "counter"
    }
    fn apply(&self, x: u32, bits: u32) -> u32 {
        x.wrapping_add(1) & mask(bits)
    }
}

// ---------------------------------------------------------------------------
// ChaCha — the baseline
// ---------------------------------------------------------------------------

/// The ChaCha permutation over a 512-bit state (16 little-endian `u32` words).
///
/// One `round` here is one ChaCha *round* (half a double round): even indices
/// apply the column round, odd indices the diagonal round. So 20 ChaCha rounds
/// — the shipped configuration — is `rounds = 20`.
///
/// Note this is the bare permutation, without the feed-forward addition that
/// the full block function performs. The batteries measure permutations.
pub struct ChaCha;

impl ChaCha {
    #[inline]
    fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(16);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(12);
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(8);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(7);
    }

    fn load(state: &[u8]) -> [u32; 16] {
        let mut w = [0u32; 16];
        for (i, word) in w.iter_mut().enumerate() {
            let b = &state[i * 4..i * 4 + 4];
            *word = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        w
    }

    fn store(w: &[u32; 16], state: &mut [u8]) {
        for (i, word) in w.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
    }
}

impl Permutation for ChaCha {
    fn name(&self) -> &'static str {
        "chacha"
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        20
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let mut w = Self::load(state);
        if round_index.is_multiple_of(2) {
            // Column round.
            Self::quarter_round(&mut w, 0, 4, 8, 12);
            Self::quarter_round(&mut w, 1, 5, 9, 13);
            Self::quarter_round(&mut w, 2, 6, 10, 14);
            Self::quarter_round(&mut w, 3, 7, 11, 15);
        } else {
            // Diagonal round.
            Self::quarter_round(&mut w, 0, 5, 10, 15);
            Self::quarter_round(&mut w, 1, 6, 11, 12);
            Self::quarter_round(&mut w, 2, 7, 8, 13);
            Self::quarter_round(&mut w, 3, 4, 9, 14);
        }
        Self::store(&w, state);
    }
}

// ---------------------------------------------------------------------------
// Klimov–Shamir — the T-function, and the graveyard's central lesson
// ---------------------------------------------------------------------------

/// `x ↦ x + (x² ∨ C) mod 2ⁿ`, the Klimov–Shamir single-cycle T-function.
///
/// A permutation with a single cycle of length 2ⁿ iff bits 0 and 2 of `C` are
/// set, so the conventional `C = 5` is used. Applied independently to each
/// 64-bit lane of the state, which means there is no inter-lane diffusion at
/// all — the avalanche matrix should show one triangular block per lane and
/// nothing off the diagonal.
pub struct KlimovShamir {
    pub bytes: usize,
}

impl Default for KlimovShamir {
    fn default() -> Self {
        Self { bytes: 64 }
    }
}

/// The constant `C`. Bits 0 and 2 set, as the single-cycle condition requires.
pub const KS_C: u64 = 5;

#[inline]
fn ks_step(x: u64) -> u64 {
    x.wrapping_add(x.wrapping_mul(x) | KS_C)
}

impl Permutation for KlimovShamir {
    fn name(&self) -> &'static str {
        "klimov-shamir"
    }
    fn state_bytes(&self) -> usize {
        self.bytes
    }
    fn default_rounds(&self) -> usize {
        1
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        for lane in state.chunks_exact_mut(8) {
            let x = u64::from_le_bytes(lane.try_into().expect("chunk is 8 bytes"));
            lane.copy_from_slice(&ks_step(x).to_le_bytes());
        }
    }
}

impl SmallMap for KlimovShamir {
    fn name(&self) -> &'static str {
        "klimov-shamir"
    }
    fn apply(&self, x: u32, bits: u32) -> u32 {
        let m = mask(bits);
        let x = u64::from(x & m);
        (ks_step(x) as u32) & m
    }
}

/// Klimov–Shamir followed by a byte transposition.
///
/// The literature's stated repair for triangular T-functions is to interleave
/// the arithmetic with bitwise transpositions — byte swapping specifically,
/// with rotation helping only "to a small degree". This composes the map with
/// an 8×8 byte transpose across lanes, which is the cheapest operation that
/// carries high bits of one lane into low positions of another.
///
/// It exists to test one claim: whether the anti-triangular layer, rather than
/// the exotic component, is what actually produces the diffusion.
pub struct KlimovShamirTransposed {
    pub bytes: usize,
}

impl Default for KlimovShamirTransposed {
    fn default() -> Self {
        Self { bytes: 64 }
    }
}

impl Permutation for KlimovShamirTransposed {
    fn name(&self) -> &'static str {
        "klimov-shamir-transposed"
    }
    fn state_bytes(&self) -> usize {
        self.bytes
    }
    fn default_rounds(&self) -> usize {
        8
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        KlimovShamir { bytes: self.bytes }.round(state, round_index);
        transpose_8x8(state);
    }
}

/// Transposes the state as a sequence of 8×8 byte matrices: byte `i` of lane
/// `j` swaps with byte `j` of lane `i`. Its own inverse, so it is a bijection.
pub fn transpose_8x8(state: &mut [u8]) {
    for block in state.chunks_exact_mut(64) {
        for i in 0..8 {
            for j in (i + 1)..8 {
                block.swap(i * 8 + j, j * 8 + i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permutation::flip_bit;

    /// RFC 8439 §2.1.1 quarter-round test vector. Without this the whole
    /// baseline could be a plausible-looking but wrong ChaCha, and every
    /// comparison against it would be meaningless.
    #[test]
    fn chacha_quarter_round_matches_rfc8439() {
        let mut s = [0u32; 16];
        s[0] = 0x1111_1111;
        s[1] = 0x0102_0304;
        s[2] = 0x9b8d_6f43;
        s[3] = 0x0123_4567;
        ChaCha::quarter_round(&mut s, 0, 1, 2, 3);
        assert_eq!(s[0], 0xea2a_92f4);
        assert_eq!(s[1], 0xcb1c_f8ce);
        assert_eq!(s[2], 0x4581_472e);
        assert_eq!(s[3], 0x5881_c4bb);
    }

    #[test]
    fn chacha_load_store_round_trips() {
        let mut state: Vec<u8> = (0..64u8).collect();
        let original = state.clone();
        let w = ChaCha::load(&state);
        ChaCha::store(&w, &mut state);
        assert_eq!(state, original);
    }

    #[test]
    fn transpose_is_an_involution() {
        let mut state: Vec<u8> = (0..64u8).collect();
        let original = state.clone();
        transpose_8x8(&mut state);
        assert_ne!(state, original, "transpose should actually move bytes");
        transpose_8x8(&mut state);
        assert_eq!(state, original);
    }

    /// The single-cycle condition from the literature: bits 0 and 2 of C set.
    #[test]
    fn ks_constant_satisfies_single_cycle_condition() {
        assert_eq!(KS_C & 1, 1, "least significant bit must be set");
        assert_eq!(
            (KS_C >> 2) & 1,
            1,
            "third-least significant bit must be set"
        );
    }

    /// The defining property of a T-function: output bit i depends only on
    /// input bits 0..=i. So flipping a HIGH input bit must never change a
    /// LOWER output bit. This is the structure that killed the family.
    #[test]
    fn klimov_shamir_is_triangular() {
        let ks = KlimovShamir { bytes: 8 };
        for flipped in 0..64usize {
            let mut a = [0x5au8; 8];
            let mut b = a;
            flip_bit(&mut b, flipped);
            ks.round(&mut a, 0);
            ks.round(&mut b, 0);
            let x = u64::from_le_bytes(a);
            let y = u64::from_le_bytes(b);
            let diff = x ^ y;
            // No bit strictly below `flipped` may differ.
            let below = if flipped == 0 {
                0
            } else {
                (1u64 << flipped) - 1
            };
            assert_eq!(
                diff & below,
                0,
                "flipping input bit {flipped} changed a lower output bit"
            );
        }
    }

    #[test]
    fn counter_has_no_diffusion_from_low_bit() {
        let c = Counter { bytes: 8 };
        let mut a = [0u8; 8];
        let mut b = [0u8; 8];
        flip_bit(&mut b, 3);
        c.round(&mut a, 0);
        c.round(&mut b, 0);
        // Exactly the flipped bit differs; a counter diffuses nothing.
        assert_eq!(u64::from_le_bytes(a) ^ u64::from_le_bytes(b), 1 << 3);
    }
}
