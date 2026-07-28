//! Task 1 — structural search over ARX diffusion topologies.
//!
//! Not a constant search. A 2016 paper already swept all 32⁴ rotation
//! constants for ChaCha's quarter round, found tens of thousands that diffuse
//! better, and the field ignored every one of them for a decade. Repeating
//! that is neither novel nor useful (see `PRIOR_ART_ROTATION_CONSTANTS.md`).
//!
//! What this searches is the thing Salsa20 → ChaCha20 actually changed: **the
//! arrangement of the operations**. ChaCha kept Salsa's instruction count and
//! rearranged which words each quarter round touches and in what order, and
//! got faster diffusion per round for free. This looks for another such
//! rearrangement.
//!
//! # The fairness constraint
//!
//! **Every quarter-round shape here performs exactly 4 additions, 4 XORs and
//! 4 rotations.** Without that constraint a "better" structure could simply be
//! one doing more work, and the comparison would measure nothing. Cost still
//! varies through instruction-level parallelism — shapes with shorter
//! dependency chains issue better on a wide core — and that difference is
//! real and is measured separately as ns/byte.
//!
//! # What is varied
//!
//! * **Quarter-round topology** — five distinct information-flow patterns
//!   over four words, [`QrShape`].
//! * **Round pattern** — which four words are grouped, [`RoundPattern`].
//!   ChaCha alternates columns and diagonals; Salsa alternates columns and
//!   rows; diagonal step size is a further free parameter nobody appears to
//!   have swept.
//!
//! Rotation constants are held at ChaCha's [16, 12, 8, 7] throughout, so any
//! difference measured is attributable to structure alone.

use crate::permutation::Permutation;

/// How information flows between the four words of a quarter round.
///
/// All five perform 4 adds, 4 XORs, 4 rotations. All are bijections: every
/// step is an invertible operation on one word given the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrShape {
    /// ChaCha's. Two crossing pairs, each applied twice: `(a,b)→d`, `(c,d)→b`.
    /// Only `b` and `d` are ever rotated.
    DoubleCross,
    /// Salsa20's. A cyclic update where each word is XORed with a rotation of
    /// the sum of the two before it.
    SalsaCycle,
    /// A four-cycle `a→c→b→d→a`. Every word is rotated exactly once, and the
    /// chain is fully serial.
    Chain,
    /// Two independent additions issued together, then two crossing XORs.
    /// Same op count as `DoubleCross` but a shorter dependency chain, so it
    /// should extract more instruction-level parallelism.
    ParallelCross,
    /// `DoubleCross` extended so all four words are rotated, not just `b`
    /// and `d`.
    WideCross,
}

impl QrShape {
    pub const ALL: [QrShape; 5] = [
        QrShape::DoubleCross,
        QrShape::SalsaCycle,
        QrShape::Chain,
        QrShape::ParallelCross,
        QrShape::WideCross,
    ];

    pub fn label(self) -> &'static str {
        match self {
            QrShape::DoubleCross => "double-cross",
            QrShape::SalsaCycle => "salsa-cycle",
            QrShape::Chain => "chain",
            QrShape::ParallelCross => "parallel-cross",
            QrShape::WideCross => "wide-cross",
        }
    }

    #[inline]
    fn apply(self, w: &mut [u32; 16], i: [usize; 4], r: [u32; 4]) {
        let (a, b, c, d) = (i[0], i[1], i[2], i[3]);
        match self {
            QrShape::DoubleCross => {
                w[a] = w[a].wrapping_add(w[b]);
                w[d] = (w[d] ^ w[a]).rotate_left(r[0]);
                w[c] = w[c].wrapping_add(w[d]);
                w[b] = (w[b] ^ w[c]).rotate_left(r[1]);
                w[a] = w[a].wrapping_add(w[b]);
                w[d] = (w[d] ^ w[a]).rotate_left(r[2]);
                w[c] = w[c].wrapping_add(w[d]);
                w[b] = (w[b] ^ w[c]).rotate_left(r[3]);
            }
            QrShape::SalsaCycle => {
                w[b] ^= w[a].wrapping_add(w[d]).rotate_left(r[0]);
                w[c] ^= w[b].wrapping_add(w[a]).rotate_left(r[1]);
                w[d] ^= w[c].wrapping_add(w[b]).rotate_left(r[2]);
                w[a] ^= w[d].wrapping_add(w[c]).rotate_left(r[3]);
            }
            QrShape::Chain => {
                w[a] = w[a].wrapping_add(w[b]);
                w[c] = (w[c] ^ w[a]).rotate_left(r[0]);
                w[b] = w[b].wrapping_add(w[c]);
                w[d] = (w[d] ^ w[b]).rotate_left(r[1]);
                w[c] = w[c].wrapping_add(w[d]);
                w[a] = (w[a] ^ w[c]).rotate_left(r[2]);
                w[d] = w[d].wrapping_add(w[a]);
                w[b] = (w[b] ^ w[d]).rotate_left(r[3]);
            }
            QrShape::ParallelCross => {
                w[a] = w[a].wrapping_add(w[b]);
                w[c] = w[c].wrapping_add(w[d]);
                w[b] = (w[b] ^ w[c]).rotate_left(r[0]);
                w[d] = (w[d] ^ w[a]).rotate_left(r[1]);
                w[a] = w[a].wrapping_add(w[b]);
                w[c] = w[c].wrapping_add(w[d]);
                w[b] = (w[b] ^ w[c]).rotate_left(r[2]);
                w[d] = (w[d] ^ w[a]).rotate_left(r[3]);
            }
            QrShape::WideCross => {
                w[a] = w[a].wrapping_add(w[b]);
                w[d] = (w[d] ^ w[a]).rotate_left(r[0]);
                w[c] = w[c].wrapping_add(w[d]);
                w[b] = (w[b] ^ w[c]).rotate_left(r[1]);
                w[b] = w[b].wrapping_add(w[c]);
                w[a] = (w[a] ^ w[b]).rotate_left(r[2]);
                w[d] = w[d].wrapping_add(w[a]);
                w[c] = (w[c] ^ w[d]).rotate_left(r[3]);
            }
        }
    }
}

/// Which four words a quarter round is applied to, on the odd (second) round.
///
/// Even rounds are always the column round `{i, i+4, i+8, i+12}`; what varies
/// is how the second round re-groups them so information crosses between
/// columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundPattern {
    /// `{i, 4+(i+s)%4, 8+(i+2s)%4, 12+(i+3s)%4}`. ChaCha is `s = 1`.
    /// `s = 2` and `s = 3` are equally valid and, as far as could be found,
    /// unswept.
    Diagonal(usize),
    /// Salsa20's: rows `{4j, 4j+1, 4j+2, 4j+3}`.
    Row,
}

impl RoundPattern {
    pub const ALL: [RoundPattern; 4] = [
        RoundPattern::Diagonal(1),
        RoundPattern::Diagonal(2),
        RoundPattern::Diagonal(3),
        RoundPattern::Row,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RoundPattern::Diagonal(1) => "diag-1",
            RoundPattern::Diagonal(2) => "diag-2",
            RoundPattern::Diagonal(3) => "diag-3",
            RoundPattern::Diagonal(_) => "diag-?",
            RoundPattern::Row => "row",
        }
    }

    fn groups(self) -> [[usize; 4]; 4] {
        let mut g = [[0usize; 4]; 4];
        match self {
            RoundPattern::Diagonal(s) => {
                for (i, row) in g.iter_mut().enumerate() {
                    for (k, slot) in row.iter_mut().enumerate() {
                        *slot = 4 * k + (i + k * s) % 4;
                    }
                }
            }
            RoundPattern::Row => {
                for (j, row) in g.iter_mut().enumerate() {
                    for (k, slot) in row.iter_mut().enumerate() {
                        *slot = 4 * j + k;
                    }
                }
            }
        }
        g
    }
}

/// A 512-bit ARX permutation defined by its topology.
///
/// At `DoubleCross` + `Diagonal(1)` + `[16, 12, 8, 7]` this is exactly
/// ChaCha, which is asserted by a unit test against
/// [`crate::systems::ChaCha`].
pub struct ArxStructure {
    pub qr: QrShape,
    pub pattern: RoundPattern,
    pub rot: [u32; 4],
    /// Name reported by `Permutation::name()`. Must match the key this is
    /// registered under in `permutation_by_name`, or the stream binary and the
    /// registry disagree about what is being measured.
    pub label: &'static str,
}

impl ArxStructure {
    pub fn new(qr: QrShape, pattern: RoundPattern) -> Self {
        Self {
            qr,
            pattern,
            rot: [16, 12, 8, 7],
            label: "arx-structure",
        }
    }

    /// Same structure, reporting a specific registered name.
    pub fn named(qr: QrShape, pattern: RoundPattern, label: &'static str) -> Self {
        Self {
            label,
            ..Self::new(qr, pattern)
        }
    }

    pub fn describe(&self) -> String {
        format!("{}/{}", self.qr.label(), self.pattern.label())
    }

    /// The column grouping used on every even round.
    fn columns() -> [[usize; 4]; 4] {
        let mut g = [[0usize; 4]; 4];
        for (i, row) in g.iter_mut().enumerate() {
            for (k, slot) in row.iter_mut().enumerate() {
                *slot = i + 4 * k;
            }
        }
        g
    }
}

impl Permutation for ArxStructure {
    fn name(&self) -> &'static str {
        self.label
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        20
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let mut w = [0u32; 16];
        for (i, word) in w.iter_mut().enumerate() {
            *word = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }

        let groups = if round_index.is_multiple_of(2) {
            Self::columns()
        } else {
            self.pattern.groups()
        };
        for g in groups {
            self.qr.apply(&mut w, g, self.rot);
        }

        for (i, word) in w.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::ChaCha;

    /// The baseline configuration must reproduce ChaCha exactly, or every
    /// comparison against it is measuring two different things.
    #[test]
    fn baseline_configuration_is_chacha() {
        let arx = ArxStructure::new(QrShape::DoubleCross, RoundPattern::Diagonal(1));
        for rounds in [1usize, 2, 3, 4, 8, 20] {
            let a0: Vec<u8> = (0..64u8)
                .map(|i| i.wrapping_mul(31).wrapping_add(7))
                .collect();
            let (mut a, mut b) = (a0.clone(), a0);
            ChaCha.permute(&mut a, rounds);
            arx.permute(&mut b, rounds);
            assert_eq!(a, b, "divergence at {rounds} rounds");
        }
    }

    /// Every group must cover all 16 words exactly once, or the round leaves
    /// part of the state untouched and the whole comparison is void.
    #[test]
    fn every_pattern_partitions_the_state() {
        let mut patterns = vec![RoundPattern::Row];
        for s in 1..4 {
            patterns.push(RoundPattern::Diagonal(s));
        }
        for p in patterns {
            let mut seen = [0u8; 16];
            for g in p.groups() {
                for idx in g {
                    seen[idx] += 1;
                }
            }
            assert_eq!(
                seen,
                [1u8; 16],
                "{} does not partition the state: {seen:?}",
                p.label()
            );
        }
        let mut seen = [0u8; 16];
        for g in ArxStructure::columns() {
            for idx in g {
                seen[idx] += 1;
            }
        }
        assert_eq!(seen, [1u8; 16], "column round does not partition");
    }

    /// Diagonal(1) must reproduce ChaCha's published diagonal groups.
    #[test]
    fn diagonal_one_matches_chachas_groups() {
        let g = RoundPattern::Diagonal(1).groups();
        assert_eq!(g[0], [0, 5, 10, 15]);
        assert_eq!(g[1], [1, 6, 11, 12]);
        assert_eq!(g[2], [2, 7, 8, 13]);
        assert_eq!(g[3], [3, 4, 9, 14]);
    }

    /// Every shape must actually change the state, and no two shapes may be
    /// accidental duplicates of each other.
    #[test]
    fn all_shapes_are_distinct_and_non_trivial() {
        let mut outputs = Vec::new();
        for qr in QrShape::ALL {
            let arx = ArxStructure::new(qr, RoundPattern::Diagonal(1));
            let mut s: Vec<u8> = (0..64u8).collect();
            let original = s.clone();
            arx.permute(&mut s, 2);
            assert_ne!(s, original, "{} did not change the state", qr.label());
            assert!(
                !outputs.contains(&s),
                "{} duplicates an earlier shape",
                qr.label()
            );
            outputs.push(s);
        }
    }

    /// Each shape must be a bijection. Verified by exhaustively checking that
    /// a single quarter round is injective over a large sample — a collision
    /// would mean state entropy is destroyed every round.
    #[test]
    fn every_shape_is_injective_on_sampled_inputs() {
        use std::collections::HashSet;
        for qr in QrShape::ALL {
            let mut seen = HashSet::new();
            for n in 0..20_000u32 {
                let mut w = [0u32; 16];
                w[0] = n.wrapping_mul(0x9E37_79B9);
                w[4] = n ^ 0xA5A5_A5A5;
                w[8] = n.rotate_left(11);
                w[12] = !n;
                qr.apply(&mut w, [0, 4, 8, 12], [16, 12, 8, 7]);
                assert!(
                    seen.insert([w[0], w[4], w[8], w[12]]),
                    "{} collided at n={n}",
                    qr.label()
                );
            }
        }
    }
}
