//! Structural battery (proposal §6.3 S1–S4): bijectivity and cycle spectrum.
//!
//! These are exhaustive, not sampled. A bijection cannot be established by
//! sampling, and a short-cycle class — which is a catastrophic weak-seed
//! defect — can hide from any amount of random probing.
//!
//! Exhaustive work is capped by the enumeration ceiling: 2³² states need a
//! ~537 MB visited-bitmap, 2⁶⁴ is permanently out of reach. So results here
//! validate a proof at narrow widths; they never substitute for one.

use crate::permutation::mask;
use crate::permutation::SmallMap;

/// Outcome of the bijectivity test over the whole of ℤ/2^bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bijectivity {
    pub bits: u32,
    pub is_bijection: bool,
    /// Number of states no input maps to. Zero iff the map is a bijection.
    pub unreached: u64,
    /// A witness collision, if one exists: two inputs with the same image.
    pub collision: Option<(u32, u32)>,
}

/// Tests whether `map` is a bijection on ℤ/2^bits by visiting every state.
///
/// # Panics
/// If `bits > 30`, which would need more than a gigabyte of bookkeeping.
pub fn bijectivity<M: SmallMap + ?Sized>(map: &M, bits: u32) -> Bijectivity {
    assert!(bits <= 30, "exhaustive enumeration is capped at 30 bits");
    let n: u64 = 1u64 << bits;
    let m = mask(bits);

    // preimage[y] = the input that mapped to y, or u32::MAX for "none yet".
    let mut preimage = vec![u32::MAX; n as usize];
    let mut collision = None;

    for x in 0..n {
        let x = x as u32;
        let y = map.apply(x, bits) & m;
        let slot = &mut preimage[y as usize];
        if *slot != u32::MAX {
            if collision.is_none() {
                collision = Some((*slot, x));
            }
        } else {
            *slot = x;
        }
    }

    let unreached = preimage.iter().filter(|&&p| p == u32::MAX).count() as u64;
    Bijectivity {
        bits,
        is_bijection: unreached == 0 && collision.is_none(),
        unreached,
        collision,
    }
}

/// The full cycle decomposition of a permutation on ℤ/2^bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleSpectrum {
    pub bits: u32,
    /// Cycle lengths, descending.
    pub lengths: Vec<u64>,
    pub longest: u64,
    pub shortest: u64,
    pub count: usize,
}

impl CycleSpectrum {
    /// True iff the whole state space is one cycle — the property
    /// Klimov–Shamir claim for their construction.
    pub fn is_single_cycle(&self) -> bool {
        self.count == 1
    }

    /// Fraction of the state space lying on cycles shorter than `threshold`.
    /// This is the weak-seed measure: a seed landing here is catastrophic
    /// regardless of how good the map's diffusion is.
    pub fn weak_seed_fraction(&self, threshold: u64) -> f64 {
        let weak: u64 = self.lengths.iter().filter(|&&l| l < threshold).sum();
        weak as f64 / (1u64 << self.bits) as f64
    }
}

/// Computes the exact cycle spectrum by visiting every state once.
///
/// # Panics
/// If `bits > 30`, or if `map` is not a bijection on this width — a
/// non-bijection has no cycle decomposition, so callers must run
/// [`bijectivity`] first.
pub fn cycle_spectrum<M: SmallMap + ?Sized>(map: &M, bits: u32) -> CycleSpectrum {
    assert!(bits <= 30, "exhaustive enumeration is capped at 30 bits");
    assert!(
        bijectivity(map, bits).is_bijection,
        "cycle_spectrum requires a bijection; run bijectivity() first"
    );

    let n: u64 = 1u64 << bits;
    let m = mask(bits);
    let mut seen = vec![false; n as usize];
    let mut lengths = Vec::new();

    for start in 0..n {
        if seen[start as usize] {
            continue;
        }
        let mut len = 0u64;
        let mut x = start as u32;
        loop {
            seen[x as usize] = true;
            x = map.apply(x, bits) & m;
            len += 1;
            if x == start as u32 {
                break;
            }
        }
        lengths.push(len);
    }

    lengths.sort_unstable_by(|a, b| b.cmp(a));
    let longest = *lengths.first().expect("at least one cycle");
    let shortest = *lengths.last().expect("at least one cycle");
    CycleSpectrum {
        bits,
        count: lengths.len(),
        lengths,
        longest,
        shortest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::{Counter, KlimovShamir};

    /// A deliberately broken map, to prove the battery can actually fail
    /// something. Without this the passing results below prove nothing.
    struct Collapse;
    impl SmallMap for Collapse {
        fn name(&self) -> &'static str {
            "collapse"
        }
        fn apply(&self, x: u32, bits: u32) -> u32 {
            // Drops the low bit: two-to-one, so half the space is unreachable.
            (x & !1) & mask(bits)
        }
    }

    #[test]
    fn battery_detects_a_non_bijection() {
        let r = bijectivity(&Collapse, 10);
        assert!(!r.is_bijection);
        assert_eq!(r.unreached, 512, "half the space must be unreachable");
        assert!(r.collision.is_some());
    }

    #[test]
    fn counter_is_a_bijection_with_one_cycle() {
        for bits in [4, 8, 12] {
            assert!(bijectivity(&Counter::default(), bits).is_bijection);
            let s = cycle_spectrum(&Counter::default(), bits);
            assert!(s.is_single_cycle());
            assert_eq!(s.longest, 1u64 << bits);
        }
    }

    /// The literature's claim about `x + (x² ∨ 5)`: a permutation with a
    /// single cycle of length 2ⁿ. Verified here rather than assumed.
    #[test]
    fn klimov_shamir_is_a_single_cycle_permutation() {
        for bits in [4, 6, 8, 10, 12, 14] {
            let ks = KlimovShamir::default();
            let b = bijectivity(&ks, bits);
            assert!(b.is_bijection, "not a bijection at {bits} bits");
            let s = cycle_spectrum(&ks, bits);
            assert!(
                s.is_single_cycle(),
                "expected one cycle at {bits} bits, found {}",
                s.count
            );
            assert_eq!(s.longest, 1u64 << bits);
            assert_eq!(s.weak_seed_fraction(1 << (bits - 1)), 0.0);
        }
    }

    #[test]
    fn weak_seed_fraction_is_computed_over_the_state_space() {
        let s = CycleSpectrum {
            bits: 4,
            lengths: vec![8, 4, 2, 2],
            longest: 8,
            shortest: 2,
            count: 4,
        };
        // Cycles shorter than 4 hold 2 + 2 = 4 of 16 states.
        assert_eq!(s.weak_seed_fraction(4), 0.25);
    }
}
