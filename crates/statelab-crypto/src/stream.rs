//! The seed → byte-stream construction, shared by the emitter binary and the
//! internal batteries.
//!
//! ## Why this is a library module and not just part of `statelab-stream`
//!
//! It used to live inside `bin/statelab_stream.rs`. That was safe only while
//! nothing else needed it. The seed-correlation battery (proposal §6.4 N1–N4)
//! needs to measure *the object PractRand is fed*, and the fastest way to
//! produce a confidently wrong answer would be to reimplement the construction
//! here, drift by one detail, and then report that the internal battery and the
//! external battery disagree — a finding about two different constructions
//! dressed up as a finding about a design.
//!
//! This is the same failure the `name()`-matches-registry-key test already
//! guards against, one level up. One definition, both consumers.
//!
//! ## The construction
//!
//! Counter mode over a fresh state per block:
//!
//! ```text
//! state = seed_le64 || block_le64 || keyed_tail
//! out   = extract(permute(state, rounds))
//! ```
//!
//! The tail is filled by expanding the seed with SplitMix64 and then zeroing a
//! `zero_frac` fraction of it. `zero_frac = 0.0` is a fully keyed, realistic
//! input; `1.0` is the adversarial `seed || counter || zeros`. It is a
//! *fraction* rather than a switch so that states of different widths receive
//! inputs of equal difficulty — zeroing "everything but the counter" hands a
//! 128-byte state 87.5% zeros against a 64-byte state's 75%, which is a harder
//! input rather than a weaker design, and that confound once changed an answer.
//!
//! Nothing here adds mixing of its own. Anything cleverer would contaminate the
//! measurement with the construction's strength instead of the permutation's.

use crate::permutation::Permutation;

/// The golden-ratio odd constant used by SplitMix64 as its increment.
const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// Domain separator applied to the seed before tail expansion, so the tail is
/// not a trivial function of the seed word that also sits in lane 0.
const TAIL_TWEAK: u64 = 0x243F_6A88_85A3_08D3;

/// The SplitMix64 finaliser — a strong 64-bit mixing function.
#[inline]
pub fn splitmix_finalise(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// What gets written out of a block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Extract {
    /// Configuration (a): raw state. The honest default.
    Raw,
    /// Configuration (b): low byte of each 8-byte lane. The sensitivity probe —
    /// low-bit weakness is classic and invisible in whole-word tests.
    LowByte,
    /// Configuration (c): a strong finaliser over the counter, which *is*
    /// SplitMix64. Exists to prove the extraction trap, never to flatter a weak
    /// design. The permutation is bypassed entirely, which is the point.
    Strong,
}

impl Extract {
    /// Parses the command-line spelling.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "raw" => Some(Self::Raw),
            "low-byte" => Some(Self::LowByte),
            "strong" => Some(Self::Strong),
            _ => None,
        }
    }

    /// Bytes emitted per block for a permutation of `state_bytes` width.
    pub fn output_bytes(self, state_bytes: usize) -> usize {
        match self {
            Self::Raw => state_bytes,
            Self::LowByte => state_bytes / 8,
            Self::Strong => 8,
        }
    }
}

/// Everything that decides what a stream *is*, independent of which permutation
/// drives it.
///
/// Both fields that bit the project before are here and neither has a silent
/// default: the extraction mode and the input construction must be declared
/// together, because declaring only the first is what made a Task 1 headline
/// conditional on an input nobody had varied.
#[derive(Clone, Copy, Debug)]
pub struct StreamConfig {
    pub seed: u64,
    /// `0` means "use the design's own default".
    pub rounds: usize,
    pub extract: Extract,
    /// Fraction of the state beyond the 16 seed+counter bytes that is zeroed.
    pub zero_frac: f64,
    /// Reverse the bit order within every emitted byte.
    ///
    /// For proposal §6.4 N4: low-bit weakness is classic, and a battery that
    /// consumes bits in one order can be blind to structure the other order
    /// exposes. Note this is a no-op for any *position-wise* statistic, since
    /// reversal only permutes which position carries which measurement — see
    /// [`crate::correlation::bit_position_profile`], which says so in its own
    /// documentation rather than claiming a test it does not perform.
    pub bit_reverse: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            rounds: 0,
            // Raw and fully keyed: the honest measurement of the permutation on
            // a realistic input. Every other setting is a deliberate departure.
            extract: Extract::Raw,
            zero_frac: 0.0,
            bit_reverse: false,
        }
    }
}

impl StreamConfig {
    /// The `seed || counter || zeros` construction the earlier reports used.
    pub fn zero_filled(seed: u64) -> Self {
        Self {
            seed,
            zero_frac: 1.0,
            ..Self::default()
        }
    }

    /// Round count this config runs `perm` at.
    pub fn effective_rounds<P: Permutation + ?Sized>(&self, perm: &P) -> usize {
        if self.rounds == 0 {
            perm.default_rounds()
        } else {
            self.rounds
        }
    }
}

/// Builds the pre-permutation state for one block, in place.
///
/// `state` must already be `perm.state_bytes()` long; its previous contents are
/// fully overwritten for any width that is a multiple of 8.
pub fn setup_block(cfg: &StreamConfig, state: &mut [u8], block: u64) {
    let n = state.len();
    let mut z = cfg.seed ^ TAIL_TWEAK;
    for lane in state.chunks_exact_mut(8) {
        z = z.wrapping_add(GAMMA);
        lane.copy_from_slice(&splitmix_finalise(z).to_le_bytes());
    }
    let tail = n.saturating_sub(16);
    let zeroed = (cfg.zero_frac * tail as f64).round() as usize;
    state[n - zeroed..].iter_mut().for_each(|b| *b = 0);
    state[..8].copy_from_slice(&cfg.seed.to_le_bytes());
    state[8..16].copy_from_slice(&block.to_le_bytes());
}

/// Reverses the bit order within a byte. `0b0000_0001 -> 0b1000_0000`.
#[inline]
pub fn reverse_bits(b: u8) -> u8 {
    b.reverse_bits()
}

/// Produces the output bytes for one block, appending to `out`.
///
/// `out` is cleared first. `scratch` is the caller's state buffer, reused
/// across calls so a long stream does not allocate per block.
pub fn emit_block<P: Permutation + ?Sized>(
    perm: &P,
    cfg: &StreamConfig,
    block: u64,
    scratch: &mut [u8],
    out: &mut Vec<u8>,
) {
    out.clear();
    match cfg.extract {
        Extract::Strong => {
            // SplitMix64 proper: the counter advances by the golden gamma, not
            // by 1. That is not decoration — consecutive integers differ in too
            // few bits for the finaliser to separate them, and `finalise(n)`
            // fails PractRand where `finalise(n·γ)` passes. The spacing of the
            // input matters as much as the strength of the mixer.
            let z = splitmix_finalise(cfg.seed.wrapping_add(block.wrapping_mul(GAMMA)));
            out.extend_from_slice(&z.to_le_bytes());
        }
        Extract::Raw => {
            setup_block(cfg, scratch, block);
            perm.permute(scratch, cfg.effective_rounds(perm));
            out.extend_from_slice(scratch);
        }
        Extract::LowByte => {
            setup_block(cfg, scratch, block);
            perm.permute(scratch, cfg.effective_rounds(perm));
            out.extend(scratch.chunks_exact(8).map(|lane| lane[0]));
        }
    }
    if cfg.bit_reverse {
        out.iter_mut().for_each(|b| *b = reverse_bits(*b));
    }
}

/// Convenience wrapper allocating its own scratch buffer. For callers that
/// fetch a handful of blocks rather than stream them.
pub fn block_bytes<P: Permutation + ?Sized>(
    perm: &P,
    cfg: &StreamConfig,
    block: u64,
) -> Vec<u8> {
    let mut scratch = vec![0u8; perm.state_bytes()];
    let mut out = Vec::with_capacity(cfg.extract.output_bytes(perm.state_bytes()));
    emit_block(perm, cfg, block, &mut scratch, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::{ChaCha, Counter};

    /// Golden vector.
    ///
    /// Every PractRand result already recorded was produced by the construction
    /// this module took over. If an edit here changes a byte, those results stop
    /// describing this code and nobody finds out from a passing test suite. This
    /// pins the first block so that cannot happen quietly.
    ///
    /// Captured from `statelab-stream --system chacha --seed 1 --keyed` as it
    /// stood *before* this module existed, so it pins the old behaviour rather
    /// than merely restating the new code.
    #[test]
    fn keyed_chacha_first_block_is_pinned() {
        let cfg = StreamConfig {
            seed: 1,
            ..StreamConfig::default()
        };
        let got = block_bytes(&ChaCha, &cfg, 0);
        assert_eq!(got.len(), 64);
        assert_eq!(
            &got[..],
            &GOLDEN_CHACHA_SEED1_BLOCK0[..],
            "keyed ChaCha block 0 drifted; recorded PractRand results no longer \
             describe this construction"
        );
    }

    const GOLDEN_CHACHA_SEED1_BLOCK0: [u8; 64] = [
        0xe6, 0x74, 0x13, 0xc1, 0xc2, 0x1f, 0x3d, 0xc6, 0xde, 0x3d, 0x4e, 0x0a, 0x04, 0x71, 0xb4,
        0x56, 0x3b, 0xc5, 0x83, 0xda, 0xd2, 0xf0, 0xbe, 0x68, 0xf2, 0x84, 0xbd, 0x72, 0x4a, 0x31,
        0xdd, 0x19, 0x2a, 0xfe, 0x7b, 0x97, 0x71, 0x20, 0x1b, 0xfc, 0x4f, 0xea, 0xb3, 0x99, 0x07,
        0x31, 0xa4, 0xae, 0xbf, 0x2f, 0x15, 0xed, 0xf8, 0x5e, 0x72, 0xcd, 0x13, 0xd1, 0xc0, 0xdd,
        0xfe, 0x62, 0x67, 0x66,
    ];

    /// The seed and the block counter occupy lanes 0 and 1 verbatim, before
    /// permutation. Several batteries reason about that placement, so it is
    /// asserted rather than assumed.
    #[test]
    fn seed_and_counter_occupy_the_first_two_lanes() {
        let cfg = StreamConfig {
            seed: 0xDEAD_BEEF_1234_5678,
            ..StreamConfig::default()
        };
        let mut state = [0u8; 64];
        setup_block(&cfg, &mut state, 0x00FF_00FF_00FF_00FF);
        assert_eq!(&state[..8], &0xDEAD_BEEF_1234_5678u64.to_le_bytes());
        assert_eq!(&state[8..16], &0x00FF_00FF_00FF_00FFu64.to_le_bytes());
    }

    /// `zero_frac` is a fraction of the tail, so equal values mean equal
    /// difficulty across different state widths. This is the fix that changed
    /// an answer once; it is worth a test.
    #[test]
    fn zero_frac_scales_with_state_width() {
        for width in [64usize, 128] {
            let cfg = StreamConfig {
                seed: 5,
                zero_frac: 0.5,
                ..StreamConfig::default()
            };
            let mut state = vec![0u8; width];
            setup_block(&cfg, &mut state, 0);
            let tail = width - 16;
            // Counted over the tail only. `zero_frac` governs the tail and
            // nothing else, and the seed and counter lanes are themselves
            // mostly zero bytes for small values — counting the whole state
            // mixes those in and stops measuring the thing under test.
            let zeros = state[16..].iter().filter(|&&b| b == 0).count();
            assert!(
                zeros >= tail / 2 && zeros < tail / 2 + 8,
                "width {width}: expected about {} zero tail bytes, got {zeros}",
                tail / 2
            );
        }
    }

    #[test]
    fn zero_frac_endpoints_match_their_descriptions() {
        let mut state = vec![0u8; 64];
        setup_block(&StreamConfig::zero_filled(9), &mut state, 3);
        assert!(
            state[16..].iter().all(|&b| b == 0),
            "zero_frac 1.0 must leave seed || counter || zeros"
        );

        let mut state = vec![0u8; 64];
        setup_block(
            &StreamConfig {
                seed: 9,
                ..StreamConfig::default()
            },
            &mut state,
            3,
        );
        assert!(
            state[16..].iter().any(|&b| b != 0),
            "zero_frac 0.0 must leave the tail keyed"
        );
    }

    #[test]
    fn bit_reverse_is_an_involution_on_the_output() {
        let plain = StreamConfig {
            seed: 4,
            ..StreamConfig::default()
        };
        let reversed = StreamConfig {
            bit_reverse: true,
            ..plain
        };
        let a = block_bytes(&ChaCha, &plain, 7);
        let b = block_bytes(&ChaCha, &reversed, 7);
        assert_ne!(a, b, "bit reversal should change the bytes");
        let back: Vec<u8> = b.iter().map(|&x| reverse_bits(x)).collect();
        assert_eq!(a, back);
    }

    #[test]
    fn extraction_widths_are_what_the_modes_advertise() {
        assert_eq!(Extract::Raw.output_bytes(64), 64);
        assert_eq!(Extract::LowByte.output_bytes(64), 8);
        assert_eq!(Extract::Strong.output_bytes(64), 8);
        for (mode, width) in [
            (Extract::Raw, 64),
            (Extract::LowByte, 8),
            (Extract::Strong, 8),
        ] {
            let cfg = StreamConfig {
                extract: mode,
                ..StreamConfig::default()
            };
            assert_eq!(block_bytes(&ChaCha, &cfg, 0).len(), width);
        }
    }

    /// The strong extractor ignores the permutation entirely — that is what
    /// makes it a proof of the extraction trap rather than a measurement.
    #[test]
    fn strong_extraction_is_independent_of_the_permutation() {
        let cfg = StreamConfig {
            seed: 3,
            extract: Extract::Strong,
            ..StreamConfig::default()
        };
        assert_eq!(
            block_bytes(&ChaCha, &cfg, 11),
            block_bytes(&Counter::default(), &cfg, 11)
        );
    }

    #[test]
    fn extract_parses_only_its_advertised_spellings() {
        assert_eq!(Extract::parse("raw"), Some(Extract::Raw));
        assert_eq!(Extract::parse("low-byte"), Some(Extract::LowByte));
        assert_eq!(Extract::parse("strong"), Some(Extract::Strong));
        assert_eq!(Extract::parse("Raw"), None);
        assert_eq!(Extract::parse(""), None);
    }
}
