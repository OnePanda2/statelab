//! Locating where a statistic saturates, without being fooled by a threshold it
//! straddles.
//!
//! # The failure this exists to prevent
//!
//! `PHASE_P` swept ChaCha's round count against three batteries and asked where
//! each one stops improving. The obvious test — *the first round count at which
//! the statistic is under its threshold* — produced a **false finding**:
//!
//! > *"BIC binds at 5 where avalanche cleared at 4. The statistical floor is 5,
//! > not 4, and every phase that used avalanche alone was reading a floor one
//! > round too low."*
//!
//! The table printed directly above that conclusion disproved it. BIC's readings
//! against a 0.0848 floor, from round 4 up:
//!
//! ```text
//! 4:0.0863 HI   5:0.0846 ok   6:0.0899 HI   7:0.0883 HI   8:0.0859 HI
//! 10:0.0819 ok  12:0.0821 ok  16:0.0872 HI  20:0.0815 ok
//! ```
//!
//! **ChaCha at 16 rounds cannot carry more pairwise correlation than at 5.** The
//! statistic sits *on* its threshold and crosses back and forth by ±5%, so a
//! first-crossing test reports whichever sample happened to land low. It crossed
//! **5 of the 9 times** tested above saturation.
//!
//! Two known items converged and neither was applied in advance: `PHASE_H` §5's
//! still-open null gap (the fair-coin null reads ~25% below every real
//! permutation, so ChaCha straddles at *every* round count), and `PHASE_M` item
//! (17) — every fitness statistic has a regime where it fails, and **the failure
//! mode moves**. It had already moved threshold → maximum → small-count; this
//! was *straddle*.
//!
//! # The fix
//!
//! A **stays-down** criterion. Saturated at the first point from which the
//! statistic is within band for that point *and every later one tested*. A
//! single lucky sample cannot carry it, and a single unlucky one cannot break a
//! genuine saturation that has already held.
//!
//! # Why this is in the crate and not in a driver
//!
//! It was written inline in `examples/round_budget.rs`, which is where
//! `PHASE_O`'s rotated-timing fix also lived, and item (21) is precisely that a
//! lesson recorded next to one driver is not a lesson the next driver applies.
//! Flagged unpaid three times before being paid deliberately.

/// Index of the first element from which `within` holds for it **and every
/// later element**.
///
/// Returns `None` if no suffix satisfies `within` — including the case where
/// only the final element does not.
pub fn stays_within_from<T>(series: &[T], within: impl Fn(&T) -> bool) -> Option<usize> {
    (0..series.len()).find(|&i| series[i..].iter().all(&within))
}

/// The saturation point of `(x, y)` samples against a `floor`, allowing `band`×
/// slack for a statistic that sits on its threshold.
///
/// `band` of 1.15 was used in `PHASE_P`: wide enough to absorb a ±5% straddle,
/// narrow enough that the 0.9584 reading at 3 rounds is nowhere near it.
///
/// Returns the `x` at which the series settles, not the index.
pub fn saturation_point(samples: &[(f64, f64)], floor: f64, band: f64) -> Option<f64> {
    stays_within_from(samples, |&(_, y)| y <= floor * band).map(|i| samples[i].0)
}

/// How many samples with `x >= from_x` sit **above** the raw floor.
///
/// A threshold crossed by a large fraction of the samples above saturation
/// cannot locate saturation, and reporting the count makes that visible instead
/// of leaving it to be rediscovered. In `PHASE_P` this was 5 of 9.
pub fn crossings_above(samples: &[(f64, f64)], floor: f64, from_x: f64) -> (usize, usize) {
    let considered: Vec<_> = samples.iter().filter(|(x, _)| *x >= from_x).collect();
    let over = considered.iter().filter(|(_, y)| *y > floor).count();
    (over, considered.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `PHASE_P`'s real BIC series, verbatim from
    /// `REPRO_round_budget_2026-08-09.txt`. This is a regression fixture: the
    /// exact data that produced the false finding.
    const BIC: [(f64, f64); 11] = [
        (2.0, 1.0000),
        (3.0, 0.9584),
        (4.0, 0.0863),
        (5.0, 0.0846),
        (6.0, 0.0899),
        (7.0, 0.0883),
        (8.0, 0.0859),
        (10.0, 0.0819),
        (12.0, 0.0821),
        (16.0, 0.0872),
        (20.0, 0.0815),
    ];
    const FLOOR: f64 = 0.0848;

    /// *** THE REGRESSION TEST. ***
    ///
    /// A first-crossing test says 5. It is wrong, and this pins why: the first
    /// sample under the raw floor is round 5, but rounds 6, 7, 8 and 16 are all
    /// back above it.
    #[test]
    fn first_crossing_gives_the_wrong_answer() {
        let first_under = BIC.iter().find(|(_, y)| *y <= FLOOR).map(|(x, _)| *x);
        assert_eq!(first_under, Some(5.0), "the bug reproduces");

        let later_above = BIC.iter().filter(|(x, y)| *x > 5.0 && *y > FLOOR).count();
        assert!(
            later_above > 0,
            "if nothing later were above the floor, first-crossing would be fine"
        );
    }

    /// The stays-down criterion recovers the answer the raw numbers actually
    /// support: the 3 -> 4 jump, 0.9584 -> 0.0863.
    #[test]
    fn stays_down_finds_four() {
        assert_eq!(saturation_point(&BIC, FLOOR, 1.15), Some(4.0));
    }

    /// The straddle, quantified. A threshold crossed by more than half the
    /// samples above saturation cannot locate saturation.
    #[test]
    fn the_floor_is_crossed_five_times_of_nine() {
        assert_eq!(crossings_above(&BIC, FLOOR, 4.0), (5, 9));
    }

    /// The band must not be so wide it swallows a genuinely unsaturated point.
    /// 3 rounds reads 0.9584 — eleven times the floor — and must stay out.
    #[test]
    fn the_band_does_not_swallow_round_three() {
        let round_three = BIC
            .iter()
            .find(|(x, _)| *x == 3.0)
            .expect("fixture has r=3")
            .1;
        assert!(
            round_three > FLOOR * 1.15,
            "r=3 reads {round_three}, band top is {}",
            FLOOR * 1.15
        );
        assert_ne!(saturation_point(&BIC, FLOOR, 1.15), Some(3.0));
    }

    /// A series that never settles has no saturation point.
    #[test]
    fn never_settling_returns_none() {
        let s = [(1.0, 0.01), (2.0, 0.01), (3.0, 9.0)];
        assert_eq!(saturation_point(&s, 0.05, 1.15), None);
    }

    /// A clean monotone series saturates where it first goes under and stays.
    #[test]
    fn monotone_series_is_unambiguous() {
        let s = [(1.0, 1.0), (2.0, 0.5), (3.0, 0.01), (4.0, 0.01)];
        assert_eq!(saturation_point(&s, 0.05, 1.0), Some(3.0));
    }
}
