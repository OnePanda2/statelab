//! The two abstractions the cryptographic batteries measure.
//!
//! These are deliberately NOT `statelab_engine::DeterministicSystem`. That trait
//! models a *terminating* trajectory over arbitrary-precision integers —
//! transition, then convergence / cycle / limit. A cryptographic permutation is
//! the opposite object: fixed width, never terminating, and bijective by
//! construction. Forcing one trait to describe both would break the engine's
//! invariants for no gain, so this crate carries its own abstractions and the
//! engine is left untouched.

/// A fixed-width, round-based permutation.
///
/// State is a byte slice so the batteries can address individual bits without
/// knowing the design's internal word size. `round_index` is passed explicitly
/// because real designs vary their behaviour by round — ChaCha alternates
/// column and diagonal rounds, and round constants are common.
pub trait Permutation {
    /// Short identifier, used in reports and on the command line.
    fn name(&self) -> &'static str;

    /// State width in bytes.
    fn state_bytes(&self) -> usize;

    /// Round count the design would ship with. The batteries sweep from 1 to
    /// well beyond this, so a low value here costs nothing.
    fn default_rounds(&self) -> usize;

    /// Applies exactly one round, in place.
    ///
    /// Must be deterministic and must not read outside `state`.
    fn round(&self, state: &mut [u8], round_index: usize);

    /// Applies `rounds` rounds from round 0.
    fn permute(&self, state: &mut [u8], rounds: usize) {
        for r in 0..rounds {
            self.round(state, r);
        }
    }
}

/// A map narrow enough to enumerate exhaustively.
///
/// Structural properties (bijectivity, cycle spectrum) cannot be established by
/// sampling — they need every state visited. Real designs use 256–512-bit
/// states where that is permanently impossible, so structural work happens on
/// narrow instances and the result is used to validate a proof, never as a
/// substitute for one.
pub trait SmallMap {
    fn name(&self) -> &'static str;

    /// Applies the map to the low `bits` bits of `x`. Must return a value that
    /// also fits in `bits` bits.
    ///
    /// `bits` is at most 32; see the enumeration ceiling in the proposal.
    fn apply(&self, x: u32, bits: u32) -> u32;
}

/// Mask for the low `bits` bits. `bits == 32` is handled without overflow.
#[inline]
pub fn mask(bits: u32) -> u32 {
    if bits >= 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    }
}

/// Reads bit `i` of a byte slice, LSB-first within each byte.
#[inline]
pub fn get_bit(state: &[u8], i: usize) -> bool {
    (state[i / 8] >> (i % 8)) & 1 == 1
}

/// Flips bit `i` of a byte slice, LSB-first within each byte.
#[inline]
pub fn flip_bit(state: &mut [u8], i: usize) {
    state[i / 8] ^= 1 << (i % 8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_handles_full_width_without_overflow() {
        assert_eq!(mask(1), 0b1);
        assert_eq!(mask(8), 0xff);
        assert_eq!(mask(31), 0x7fff_ffff);
        assert_eq!(mask(32), u32::MAX);
    }

    #[test]
    fn bit_accessors_round_trip() {
        let mut s = [0u8; 4];
        for i in 0..32 {
            assert!(!get_bit(&s, i));
            flip_bit(&mut s, i);
            assert!(get_bit(&s, i));
            flip_bit(&mut s, i);
            assert!(!get_bit(&s, i));
        }
    }

    #[test]
    fn bit_zero_is_least_significant_bit_of_byte_zero() {
        let mut s = [0u8; 2];
        flip_bit(&mut s, 0);
        assert_eq!(s[0], 1);
        flip_bit(&mut s, 8);
        assert_eq!(s[1], 1);
    }
}
