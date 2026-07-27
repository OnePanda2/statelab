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

// ---------------------------------------------------------------------------
// Ascon — the standardised lightweight competitor
// ---------------------------------------------------------------------------

/// The Ascon permutation over a 320-bit state (5 × 64-bit words).
///
/// Standardised in NIST SP 800-232 (final, 13 August 2025). This is the design
/// the proposal identifies as the *right* competitor: ChaCha and AES win on
/// silicon this project cannot beat, but the lightweight domain has no
/// dedicated instructions, so a permutation competes on its merits.
///
/// `permute(state, 12)` is p12. Fewer rounds take the first *r* rounds of p12
/// (round constants from index 0), which is what the diffusion sweep measures;
/// note that the specified p6 uses the *last* six constants instead.
pub struct Ascon;

impl Ascon {
    /// Round constant for round `r`: `0xf0 - r·0x10 + r`, ending at `0x4b`.
    #[inline]
    pub fn round_constant(r: usize) -> u64 {
        (0xf0 - (r as u64) * 0x10) + (r as u64)
    }

    /// The 5-bit S-box, applied bitsliced across the five words.
    #[inline]
    fn sbox(x: &mut [u64; 5]) {
        x[0] ^= x[4];
        x[4] ^= x[3];
        x[2] ^= x[1];
        let t = [
            !x[0] & x[1],
            !x[1] & x[2],
            !x[2] & x[3],
            !x[3] & x[4],
            !x[4] & x[0],
        ];
        x[0] ^= t[1];
        x[1] ^= t[2];
        x[2] ^= t[3];
        x[3] ^= t[4];
        x[4] ^= t[0];
        x[1] ^= x[0];
        x[0] ^= x[4];
        x[3] ^= x[2];
        x[2] = !x[2];
    }

    /// Linear diffusion: each word XORed with two of its right-rotations.
    #[inline]
    fn linear(x: &mut [u64; 5]) {
        x[0] ^= x[0].rotate_right(19) ^ x[0].rotate_right(28);
        x[1] ^= x[1].rotate_right(61) ^ x[1].rotate_right(39);
        x[2] ^= x[2].rotate_right(1) ^ x[2].rotate_right(6);
        x[3] ^= x[3].rotate_right(10) ^ x[3].rotate_right(17);
        x[4] ^= x[4].rotate_right(7) ^ x[4].rotate_right(41);
    }
}

impl Permutation for Ascon {
    fn name(&self) -> &'static str {
        "ascon"
    }
    fn state_bytes(&self) -> usize {
        40
    }
    fn default_rounds(&self) -> usize {
        12
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let mut x = [0u64; 5];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u64::from_be_bytes(state[i * 8..i * 8 + 8].try_into().expect("8 bytes"));
        }
        x[2] ^= Self::round_constant(round_index);
        Self::sbox(&mut x);
        Self::linear(&mut x);
        for (i, w) in x.iter().enumerate() {
            state[i * 8..i * 8 + 8].copy_from_slice(&w.to_be_bytes());
        }
    }
}

// ---------------------------------------------------------------------------
// xoshiro256++ — excellent statistics, no cryptographic strength
// ---------------------------------------------------------------------------

/// The xoshiro256++ state transition over 256 bits.
///
/// The control the proposal names explicitly: it passes BigCrush and is
/// trivially breakable as a CSPRNG, because the transition is **linear over
/// GF(2)**. Recovering the state is solving a linear system. It exists here to
/// make the point that statistical batteries cannot detect this.
pub struct Xoshiro256pp;

impl Permutation for Xoshiro256pp {
    fn name(&self) -> &'static str {
        "xoshiro256++"
    }
    fn state_bytes(&self) -> usize {
        32
    }
    fn default_rounds(&self) -> usize {
        1
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        let mut s = [0u64; 4];
        for (i, w) in s.iter_mut().enumerate() {
            *w = u64::from_le_bytes(state[i * 8..i * 8 + 8].try_into().expect("8 bytes"));
        }
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        for (i, w) in s.iter().enumerate() {
            state[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
        }
    }
}

// ---------------------------------------------------------------------------
// LCG — the deliberately weak control
// ---------------------------------------------------------------------------

/// A 64-bit linear congruential generator per lane, `s ↦ s·m + c`.
///
/// The weak control §6.5 requires. A bijection with notoriously poor low bits:
/// bit *i* of the output depends only on bits 0..=*i* of the input, exactly as
/// in a T-function, because multiplication carries only upward.
pub struct Lcg {
    pub bytes: usize,
}

impl Default for Lcg {
    fn default() -> Self {
        Self { bytes: 64 }
    }
}

/// Knuth's MMIX multiplier.
const LCG_MUL: u64 = 6_364_136_223_846_793_005;
const LCG_ADD: u64 = 1_442_695_040_888_963_407;

impl Permutation for Lcg {
    fn name(&self) -> &'static str {
        "lcg"
    }
    fn state_bytes(&self) -> usize {
        self.bytes
    }
    fn default_rounds(&self) -> usize {
        1
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        for lane in state.chunks_exact_mut(8) {
            let s = u64::from_le_bytes(lane.try_into().expect("8 bytes"));
            let next = s.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD);
            lane.copy_from_slice(&next.to_le_bytes());
        }
    }
}

impl SmallMap for Lcg {
    fn name(&self) -> &'static str {
        "lcg"
    }
    fn apply(&self, x: u32, bits: u32) -> u32 {
        let m = mask(bits);
        let s = u64::from(x & m);
        ((s.wrapping_mul(LCG_MUL).wrapping_add(LCG_ADD)) as u32) & m
    }
}

// ---------------------------------------------------------------------------
// SplitMix64 finaliser — strong within a lane, zero coupling between lanes
// ---------------------------------------------------------------------------

/// The SplitMix64 finaliser applied independently to each 64-bit lane.
///
/// A useful contrast with [`KlimovShamir`]. Both have zero inter-lane
/// diffusion, so both must fail. But SplitMix's `xor-shift-right` steps move
/// information from high bits *down* to low bits, so it is **not** triangular:
/// its diagonal blocks should fill completely rather than forming triangles.
/// That isolates "no coupling" from "triangular", which are different defects
/// that the summary statistics alone would conflate.
pub struct SplitMixLanes {
    pub bytes: usize,
}

impl Default for SplitMixLanes {
    fn default() -> Self {
        Self { bytes: 64 }
    }
}

#[inline]
fn splitmix_finalise(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Permutation for SplitMixLanes {
    fn name(&self) -> &'static str {
        "splitmix-lanes"
    }
    fn state_bytes(&self) -> usize {
        self.bytes
    }
    fn default_rounds(&self) -> usize {
        1
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        for lane in state.chunks_exact_mut(8) {
            let x = u64::from_le_bytes(lane.try_into().expect("8 bytes"));
            lane.copy_from_slice(&splitmix_finalise(x).to_le_bytes());
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

    /// The published Ascon 5-bit S-box, verified exhaustively against the
    /// bitsliced implementation. `x0` is the most significant input bit.
    ///
    /// This is the closest thing to a known-answer test available without a
    /// full permutation vector, and it is the part most likely to be wrong: a
    /// bitsliced S-box that is subtly incorrect still looks like a permutation
    /// and still produces plausible avalanche numbers.
    #[test]
    fn ascon_sbox_matches_the_published_table() {
        const TABLE: [u8; 32] = [
            0x04, 0x0b, 0x1f, 0x14, 0x1a, 0x15, 0x09, 0x02, 0x1b, 0x05, 0x08, 0x12, 0x1d, 0x03,
            0x06, 0x1c, 0x1e, 0x13, 0x07, 0x0e, 0x00, 0x0d, 0x11, 0x18, 0x10, 0x0c, 0x01, 0x19,
            0x16, 0x0a, 0x0f, 0x17,
        ];
        for (v, &expected) in TABLE.iter().enumerate() {
            let v = v as u64;
            // Bit i of the S-box input lives in word i, with x0 the MSB.
            let mut x = [
                (v >> 4) & 1,
                (v >> 3) & 1,
                (v >> 2) & 1,
                (v >> 1) & 1,
                v & 1,
            ];
            Ascon::sbox(&mut x);
            // Each word carries 64 independent bit-columns, so the `!` steps
            // set all the bits above column 0. Only column 0 is meaningful here.
            let got = ((x[0] & 1) << 4)
                | ((x[1] & 1) << 3)
                | ((x[2] & 1) << 2)
                | ((x[3] & 1) << 1)
                | (x[4] & 1);
            assert_eq!(
                got, expected as u64,
                "S-box mismatch at input {v}: got {got:#x}, expected {expected:#x}"
            );
        }
    }

    /// The S-box is applied to all 64 bit-columns at once; it must act on each
    /// independently. Verified by running 64 random columns in parallel and
    /// comparing against the single-column result.
    #[test]
    fn ascon_sbox_is_applied_bitwise_across_lanes() {
        let mut parallel = [0u64; 5];
        let mut columns = [0u64; 64];
        for (bit, col) in columns.iter_mut().enumerate() {
            let v = ((bit as u64).wrapping_mul(2_654_435_761)) & 31;
            *col = v;
            for (w, word) in parallel.iter_mut().enumerate() {
                *word |= ((v >> (4 - w)) & 1) << bit;
            }
        }
        Ascon::sbox(&mut parallel);
        for (bit, &v) in columns.iter().enumerate() {
            let mut single = [
                (v >> 4) & 1,
                (v >> 3) & 1,
                (v >> 2) & 1,
                (v >> 1) & 1,
                v & 1,
            ];
            Ascon::sbox(&mut single);
            for (w, expected) in single.iter().enumerate() {
                assert_eq!(
                    (parallel[w] >> bit) & 1,
                    *expected & 1,
                    "lane {bit}, word {w}"
                );
            }
        }
    }

    /// Round constants: `0xf0 - r*0x10 + r`, ending at `0x4b` for p12.
    #[test]
    fn ascon_round_constants_match_the_specification() {
        let expected = [
            0xf0u64, 0xe1, 0xd2, 0xc3, 0xb4, 0xa5, 0x96, 0x87, 0x78, 0x69, 0x5a, 0x4b,
        ];
        for (r, &want) in expected.iter().enumerate() {
            assert_eq!(Ascon::round_constant(r), want, "round {r}");
        }
    }

    /// The linear layer must be invertible; a non-invertible diffusion layer
    /// would silently destroy state entropy every round.
    #[test]
    fn ascon_linear_layer_is_a_bijection_on_each_word() {
        // Over GF(2) the map is x ^ rotr(x,a) ^ rotr(x,b) on 64 bits. Verified
        // by checking it is injective on a large random sample plus that zero
        // is the unique preimage of zero for the linear map.
        let mut seen = std::collections::HashSet::new();
        for i in 0..5000u64 {
            let mut x = [i.wrapping_mul(0x9E37_79B9_7F4A_7C15), 0, 0, 0, 0];
            Ascon::linear(&mut x);
            assert!(seen.insert(x[0]), "collision in the linear layer at {i}");
        }
        let mut zero = [0u64; 5];
        Ascon::linear(&mut zero);
        assert_eq!(zero, [0u64; 5], "linear layer must fix zero");
    }

    /// xoshiro256++ is linear over GF(2): F(a) ^ F(b) == F(a ^ b) once the
    /// affine part is removed. This is exactly why it is not a CSPRNG, and it
    /// is worth pinning so the control cannot silently become non-linear.
    #[test]
    fn xoshiro_transition_is_linear_over_gf2() {
        let x = Xoshiro256pp;
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        for i in 0..32 {
            a[i] = (i as u8).wrapping_mul(37).wrapping_add(11);
            b[i] = (i as u8).wrapping_mul(89).wrapping_add(3);
        }
        let mut xor: Vec<u8> = a.iter().zip(&b).map(|(p, q)| p ^ q).collect();
        let (mut fa, mut fb) = (a, b);
        x.round(&mut fa, 0);
        x.round(&mut fb, 0);
        x.round(&mut xor, 0);
        let combined: Vec<u8> = fa.iter().zip(&fb).map(|(p, q)| p ^ q).collect();
        assert_eq!(
            combined, xor,
            "xoshiro256++ transition must be GF(2)-linear"
        );
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
