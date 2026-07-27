//! Phase A — does 64-bit word width pay?
//!
//! ChaCha uses 32-bit words. That was correct in 2008 (SSE2, 128-bit vectors,
//! 32-bit lanes, 32-bit embedded hardware still commercially relevant). On any
//! 64-bit CPU a 64-bit add costs exactly what a 32-bit add costs, so a 64-bit
//! ARX design does twice the work per instruction — 0.375 ops/byte against
//! ChaCha's 0.750.
//!
//! This module isolates that variable. All three designs measured in Phase A
//! share **one quarter-round shape and one round pattern** — ChaCha's — and
//! differ only in word width and rotation constants:
//!
//! | Design | Word | Left-rotations | Provenance |
//! |---|---|---|---|
//! | `ChaCha` (in `systems`) | 32-bit | 16, 12, 8, 7 | published |
//! | [`CHACHA64`] | 64-bit | 32, 24, 16, 14 | naive ×2 scaling — a probe, not a proposal |
//! | [`BLAKE2B`] | 64-bit | 32, 40, 48, 1 | published, analysed, deployed |
//!
//! BLAKE2b is the control that makes this experiment honest. Its permutation
//! **is** ChaCha's structure on 64-bit words — BLAKE2 borrowed the pattern
//! explicitly — with rotation constants chosen so three of four are byte
//! aligned and therefore a single shuffle instruction under SSSE3 or NEON.
//! If a published, cryptanalysed design already beats ChaCha per byte, the
//! thesis needs no new invention to be true.
//!
//! The G function and round structure here are validated by hashing against a
//! published BLAKE2b-512 digest, not by inspection.

use crate::permutation::Permutation;

/// A ChaCha-shaped ARX permutation over sixteen 64-bit words (1024 bits).
///
/// `round_index` even applies the column round, odd the diagonal round, the
/// same convention [`crate::systems::ChaCha`] uses. So one BLAKE2b hash round
/// (column + diagonal) is **two** rounds here.
pub struct Arx64 {
    pub label: &'static str,
    /// Left-rotation amounts, in quarter-round order.
    pub rot: [u32; 4],
    pub rounds: usize,
}

/// Naive ×2 scaling of ChaCha's 16/12/8/7. Isolates word width from the
/// choice of constants: only 32 and 16 are byte aligned, so this deliberately
/// does **not** collect the free-shuffle bonus. Rotation choice is Phase B.
pub const CHACHA64: Arx64 = Arx64 {
    label: "chacha64",
    rot: [32, 24, 16, 14],
    rounds: 20,
};

/// BLAKE2b's permutation. Its specification rotates **right** by 32, 24, 16,
/// 63; expressed as left-rotations that is 32, 40, 48, 1. Three of the four
/// are byte aligned, and rotate-left-by-1 is two cheap ops.
pub const BLAKE2B: Arx64 = Arx64 {
    label: "blake2b",
    rot: [32, 40, 48, 1],
    rounds: 24, // 12 BLAKE2b rounds = 24 column/diagonal half-rounds
};

impl Arx64 {
    /// The quarter round, identical in shape to ChaCha's.
    #[inline]
    fn quarter_round(&self, s: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize) {
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(self.rot[0]);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(self.rot[1]);
        s[a] = s[a].wrapping_add(s[b]);
        s[d] = (s[d] ^ s[a]).rotate_left(self.rot[2]);
        s[c] = s[c].wrapping_add(s[d]);
        s[b] = (s[b] ^ s[c]).rotate_left(self.rot[3]);
    }

    fn load(state: &[u8]) -> [u64; 16] {
        let mut w = [0u64; 16];
        for (i, word) in w.iter_mut().enumerate() {
            *word = u64::from_le_bytes(state[i * 8..i * 8 + 8].try_into().expect("8 bytes"));
        }
        w
    }

    fn store(w: &[u64; 16], state: &mut [u8]) {
        for (i, word) in w.iter().enumerate() {
            state[i * 8..i * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
    }
}

impl Permutation for Arx64 {
    fn name(&self) -> &'static str {
        self.label
    }
    fn state_bytes(&self) -> usize {
        128
    }
    fn default_rounds(&self) -> usize {
        self.rounds
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let mut w = Self::load(state);
        if round_index.is_multiple_of(2) {
            self.quarter_round(&mut w, 0, 4, 8, 12);
            self.quarter_round(&mut w, 1, 5, 9, 13);
            self.quarter_round(&mut w, 2, 6, 10, 14);
            self.quarter_round(&mut w, 3, 7, 11, 15);
        } else {
            self.quarter_round(&mut w, 0, 5, 10, 15);
            self.quarter_round(&mut w, 1, 6, 11, 12);
            self.quarter_round(&mut w, 2, 7, 8, 13);
            self.quarter_round(&mut w, 3, 4, 9, 14);
        }
        Self::store(&w, state);
    }
}

// ---------------------------------------------------------------------------
// BLAKE2b hash — present only to validate the permutation above
// ---------------------------------------------------------------------------
//
// The G function and round pattern used by `BLAKE2B` must be exactly right or
// every comparison against it is worthless. Structural inspection is not
// enough: a subtly wrong ARX round still looks like a permutation and still
// produces plausible avalanche numbers. So the same G function is driven
// through the full compression function and checked against a published
// digest.

const BLAKE2B_IV: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];

#[rustfmt::skip]
const SIGMA: [[usize; 16]; 12] = [
    [ 0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15],
    [14, 10,  4,  8,  9, 15, 13,  6,  1, 12,  0,  2, 11,  7,  5,  3],
    [11,  8, 12,  0,  5,  2, 15, 13, 10, 14,  3,  6,  7,  1,  9,  4],
    [ 7,  9,  3,  1, 13, 12, 11, 14,  2,  6,  5, 10,  4,  0, 15,  8],
    [ 9,  0,  5,  7,  2,  4, 10, 15, 14,  1, 11, 12,  6,  8,  3, 13],
    [ 2, 12,  6, 10,  0, 11,  8,  3,  4, 13,  7,  5, 15, 14,  1,  9],
    [12,  5,  1, 15, 14, 13,  4, 10,  0,  7,  6,  3,  9,  2,  8, 11],
    [13, 11,  7, 14, 12,  1,  3,  9,  5,  0, 15,  4,  8,  6,  2, 10],
    [ 6, 15, 14,  9, 11,  3,  0,  8, 12,  2, 13,  7,  1,  4, 10,  5],
    [10,  2,  8,  4,  7,  6,  1,  5, 15, 11,  9, 14,  3, 12, 13,  0],
    [ 0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15],
    [14, 10,  4,  8,  9, 15, 13,  6,  1, 12,  0,  2, 11,  7,  5,  3],
];

/// BLAKE2b's G, with message injection. With `x = y = 0` this reduces exactly
/// to [`Arx64::quarter_round`] at `BLAKE2B.rot`.
#[inline]
#[allow(clippy::too_many_arguments)]
fn g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

fn compress(h: &mut [u64; 8], block: &[u8; 128], counter: u128, last: bool) {
    let mut m = [0u64; 16];
    for (i, w) in m.iter_mut().enumerate() {
        *w = u64::from_le_bytes(block[i * 8..i * 8 + 8].try_into().expect("8 bytes"));
    }

    let mut v = [0u64; 16];
    v[..8].copy_from_slice(h);
    v[8..].copy_from_slice(&BLAKE2B_IV);
    v[12] ^= counter as u64;
    v[13] ^= (counter >> 64) as u64;
    if last {
        v[14] = !v[14];
    }

    for s in SIGMA.iter() {
        g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }

    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

/// Unkeyed BLAKE2b-512. Exists solely as a known-answer harness for the G
/// function that [`BLAKE2B`] measures.
pub fn blake2b512(input: &[u8]) -> [u8; 64] {
    let mut h = BLAKE2B_IV;
    h[0] ^= 0x0101_0000 ^ 64; // no key, 64-byte digest

    let mut offset = 0usize;
    // All blocks except the final one are full and not marked last.
    while input.len() - offset > 128 {
        let mut block = [0u8; 128];
        block.copy_from_slice(&input[offset..offset + 128]);
        offset += 128;
        compress(&mut h, &block, offset as u128, false);
    }
    let mut block = [0u8; 128];
    block[..input.len() - offset].copy_from_slice(&input[offset..]);
    compress(&mut h, &block, input.len() as u128, true);

    let mut out = [0u8; 64];
    for (i, w) in h.iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Published BLAKE2b-512 digests. If these match, the G function, the
    /// SIGMA schedule, the round pattern and the rotation constants are all
    /// exactly right — which is the only reason the permutation measured in
    /// Phase A can be trusted as a control.
    #[test]
    fn blake2b512_matches_published_digests() {
        assert_eq!(
            hex(&blake2b512(b"")),
            "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419\
             d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"
        );
        assert_eq!(
            hex(&blake2b512(b"abc")),
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
             7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
        );
    }

    /// The permutation under measurement must be the *same* ARX core the
    /// digest test validated. With zero message words, BLAKE2b's G is exactly
    /// `Arx64::quarter_round` at `BLAKE2B.rot` — checked here rather than
    /// asserted in a comment.
    #[test]
    fn blake2b_permutation_is_the_validated_g_with_zero_message() {
        let mut viag = [0u64; 16];
        let mut viaqr = [0u64; 16];
        for i in 0..16 {
            let seed = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            viag[i] = seed;
            viaqr[i] = seed;
        }
        g(&mut viag, 0, 4, 8, 12, 0, 0);
        BLAKE2B.quarter_round(&mut viaqr, 0, 4, 8, 12);
        assert_eq!(
            viag, viaqr,
            "BLAKE2B.rot must reproduce BLAKE2b's G with a zero message"
        );
    }

    /// Both 64-bit designs must carry twice the state of ChaCha, or the
    /// per-byte comparison is not measuring what it claims to.
    #[test]
    fn word_width_doubles_the_state() {
        use crate::systems::ChaCha;
        assert_eq!(ChaCha.state_bytes(), 64);
        assert_eq!(CHACHA64.state_bytes(), 128);
        assert_eq!(BLAKE2B.state_bytes(), 128);
    }

    /// A round must actually change the state, and the two rotation choices
    /// must produce different permutations — otherwise Phase A is comparing
    /// one design against itself.
    #[test]
    fn the_two_rotation_choices_are_distinct_permutations() {
        let mut a = vec![0u8; 128];
        let mut b = vec![0u8; 128];
        for i in 0..128 {
            a[i] = i as u8;
            b[i] = i as u8;
        }
        let original = a.clone();
        CHACHA64.permute(&mut a, 4);
        BLAKE2B.permute(&mut b, 4);
        assert_ne!(a, original, "a round must change the state");
        assert_ne!(a, b, "different rotation constants must differ");
    }

    /// Rotations must be in range for a 64-bit word.
    #[test]
    fn rotation_constants_are_valid() {
        for design in [&CHACHA64, &BLAKE2B] {
            for r in design.rot {
                assert!(r < 64, "{} has an out-of-range rotation {r}", design.label);
            }
        }
    }
}
