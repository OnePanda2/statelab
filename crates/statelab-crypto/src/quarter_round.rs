//! Quarter-round search — the variable Phase M held fixed.
//!
//! `PHASE_M` varied the wiring across 1049 random samples and 900 directed
//! evaluations while holding ChaCha's quarter round **fixed**, and produced
//! three wirings measurably better than ChaCha at 3 rounds and identical to it
//! at 4. **Wiring alone does not buy the round.** This module varies the other
//! half.
//!
//! ## The parameterisation, and why cost is not a confound
//!
//! ChaCha's quarter round is exactly four repetitions of one step shape:
//!
//! ```text
//!   w[i] += w[j];  w[k] ^= w[i];  w[k] <<<= r
//! ```
//!
//! with `(i,j,k,r)` = `(a,b,d,16)`, `(c,d,b,12)`, `(a,b,d,8)`, `(c,d,b,7)`.
//!
//! A [`QuarterRound`] here is **any** four such steps. Every candidate performs
//! **exactly 4 additions, 4 XORs and 4 rotations**, regardless of indices or
//! constants — so operation count is identical to ChaCha's by construction and
//! items (7) and (8) are designed out, exactly as in `topology.rs`.
//!
//! ## *** THE AXIS THAT MAKES THIS WORTH RUNNING ***
//!
//! Three searches have now failed to beat ChaCha's **round count**. But "better
//! than ChaCha20" does not only mean fewer rounds — it can mean **fewer
//! instructions at the same round count**, which is the same 1.x speedup and has
//! never been attempted.
//!
//! On SIMD, a rotation by a multiple of 8 is a **single byte shuffle**
//! (`vpshufb`); any other amount costs **shift, shift, or** — three
//! instructions. ChaCha's constants are 16, 12, 8, 7: two cheap, two expensive.
//!
//! | | add | xor | rotations | total |
//! |---|---|---|---|---|
//! | ChaCha QR (16,12,8,7) | 4 | 4 | 1+3+1+3 = 8 | **16** |
//! | all-byte-aligned QR | 4 | 4 | 1+1+1+1 = 4 | **12** |
//!
//! **A quarter round using only byte-aligned rotations is 25% cheaper, and if
//! it still reaches full avalanche at 4 rounds it is a genuine 1.33x on
//! instruction count at identical diffusion.** That is a win on the goal's own
//! terms without needing to beat the round count at all.
//!
//! This is the sharpened form of the argument that killed H4: `PRIOR_ART_H4`
//! §5 found Nager's constants 43 and 17 are **not** multiples of 8, so none of
//! his rotations can use a byte shuffle where ChaCha's 16 and 8 can. That was
//! recorded as a reason his design loses. **It is equally a reason ChaCha's own
//! 12 and 7 cost more than they need to** — and nobody appears to have asked
//! whether they can be replaced.
//!
//! ## What is deliberately NOT varied
//!
//! * **The wiring** — fixed to ChaCha's columns/diagonals, so this is the exact
//!   mirror of Phase M and the two are separable.
//! * **Rotation constants as a free sweep** — H4' is pre-empted: Sobti &
//!   Ganesan swept all 32^4 in 2016 and the field ignored every result
//!   (`PRIOR_ART_ROTATION_CONSTANTS`). The corpus also says constant selection
//!   is the one stage to delegate to a solver, not to hand-search. **The
//!   variable here is the step STRUCTURE; the byte-aligned arm constrains
//!   constants rather than exploring them.**
//!
//! ## What a win here would NOT be
//!
//! Avalanche is a proxy (`PHASE_L` §4, item 16). A cheaper quarter round that
//! matches ChaCha's diffusion is a **candidate for CLAASP**, not a result — and
//! per `TASK_1` the next test after that is the low-entropy dose-response curve
//! that killed `wide-cross` *after* it cleared exactly this kind of bar.

use crate::permutation::Permutation;
use crate::topology::{chacha_topology, Topology, ARITY, LANES, PARTITIONS};

/// One step: `w[add_to] += w[add_from]; w[xor_into] ^= w[add_to];
/// w[xor_into] <<<= rot`. Indices are lane positions **within a group**, 0..3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QrStep {
    pub add_to: u8,
    pub add_from: u8,
    pub xor_into: u8,
    pub rot: u8,
}

/// Four steps. Always 4 adds, 4 XORs, 4 rotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarterRound {
    pub steps: [QrStep; 4],
}

/// ChaCha's own quarter round. **The positive control.**
pub fn chacha_qr() -> QuarterRound {
    QuarterRound {
        steps: [
            QrStep {
                add_to: 0,
                add_from: 1,
                xor_into: 3,
                rot: 16,
            },
            QrStep {
                add_to: 2,
                add_from: 3,
                xor_into: 1,
                rot: 12,
            },
            QrStep {
                add_to: 0,
                add_from: 1,
                xor_into: 3,
                rot: 8,
            },
            QrStep {
                add_to: 2,
                add_from: 3,
                xor_into: 1,
                rot: 7,
            },
        ],
    }
}

impl QuarterRound {
    /// A step is degenerate if it adds a word to itself (`x += x`, which is a
    /// left shift and adds no nonlinearity) or XORs a word into itself
    /// (`x ^= x`, which zeroes it). Both must be rejected before measuring, not
    /// discovered as a mysteriously bad score.
    pub fn is_legal(&self) -> bool {
        self.steps.iter().all(|s| {
            s.add_to != s.add_from
                && s.xor_into != s.add_to
                && s.rot >= 1
                && s.rot <= 31
                && (s.add_to as usize) < ARITY
                && (s.add_from as usize) < ARITY
                && (s.xor_into as usize) < ARITY
        })
    }

    /// Every lane must be written at least once, or it is never mixed and the
    /// quarter round cannot possibly diffuse.
    pub fn writes_every_lane(&self) -> bool {
        let mut written = [false; ARITY];
        for s in &self.steps {
            written[s.add_to as usize] = true;
            written[s.xor_into as usize] = true;
        }
        written.iter().all(|&w| w)
    }

    /// Every rotation is a multiple of 8 — one `vpshufb` each on SIMD.
    pub fn is_byte_aligned(&self) -> bool {
        self.steps.iter().all(|s| s.rot % 8 == 0)
    }

    /// Instructions per quarter round under the standard SIMD cost model:
    /// add 1, xor 1, rotation 1 if byte-aligned else 3 (shift, shift, or).
    ///
    /// ChaCha scores 16. An all-byte-aligned quarter round scores 12.
    pub fn simd_instructions(&self) -> usize {
        self.steps
            .iter()
            .map(|s| 2 + if s.rot % 8 == 0 { 1 } else { 3 })
            .sum()
    }
}

/// ChaCha's wiring driving an arbitrary quarter round — the mirror of
/// `topology::TopologyPermutation`, which drove an arbitrary wiring with
/// ChaCha's quarter round.
pub struct QrPermutation {
    pub qr: QuarterRound,
    pub topology: Topology,
}

impl QrPermutation {
    /// The configuration under test: ChaCha's wiring, a candidate quarter round.
    pub fn with_chacha_wiring(qr: QuarterRound) -> Self {
        Self {
            qr,
            topology: chacha_topology(),
        }
    }
}

impl Permutation for QrPermutation {
    fn name(&self) -> &'static str {
        "qr-candidate"
    }
    fn state_bytes(&self) -> usize {
        LANES * 4
    }
    fn default_rounds(&self) -> usize {
        20
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let mut w = [0u32; LANES];
        for (i, wi) in w.iter_mut().enumerate() {
            *wi = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for g in &self.topology.partitions[round_index % PARTITIONS] {
            for s in &self.qr.steps {
                let at = g[s.add_to as usize] as usize;
                let af = g[s.add_from as usize] as usize;
                let xi = g[s.xor_into as usize] as usize;
                w[at] = w[at].wrapping_add(w[af]);
                w[xi] = (w[xi] ^ w[at]).rotate_left(s.rot as u32);
            }
        }
        for (i, wi) in w.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&wi.to_le_bytes());
        }
    }
}

/// Draws a legal quarter round. `byte_aligned` restricts rotations to
/// {8, 16, 24}; otherwise 1..=31.
pub fn random_qr(probe: &mut crate::avalanche::Probe, byte_aligned: bool) -> QuarterRound {
    loop {
        let mut steps = [QrStep {
            add_to: 0,
            add_from: 0,
            xor_into: 0,
            rot: 8,
        }; 4];
        for step in steps.iter_mut() {
            let add_to = (probe.next_u64() % ARITY as u64) as u8;
            let mut add_from = (probe.next_u64() % ARITY as u64) as u8;
            while add_from == add_to {
                add_from = (probe.next_u64() % ARITY as u64) as u8;
            }
            let mut xor_into = (probe.next_u64() % ARITY as u64) as u8;
            while xor_into == add_to {
                xor_into = (probe.next_u64() % ARITY as u64) as u8;
            }
            let rot = if byte_aligned {
                [8u8, 16, 24][(probe.next_u64() % 3) as usize]
            } else {
                1 + (probe.next_u64() % 31) as u8
            };
            *step = QrStep {
                add_to,
                add_from,
                xor_into,
                rot,
            };
        }
        let qr = QuarterRound { steps };
        if qr.is_legal() && qr.writes_every_lane() {
            return qr;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avalanche::{recommended_samples, rounds_to_avalanche, Probe};

    #[test]
    fn chachas_quarter_round_is_legal_and_costs_sixteen() {
        let qr = chacha_qr();
        assert!(qr.is_legal());
        assert!(qr.writes_every_lane());
        assert!(!qr.is_byte_aligned(), "12 and 7 are not multiples of 8");
        assert_eq!(qr.simd_instructions(), 16, "4 add + 4 xor + 1+3+1+3");
    }

    #[test]
    fn an_all_byte_aligned_quarter_round_costs_twelve() {
        // The whole point of the byte-aligned arm: 25% cheaper per quarter round.
        let mut qr = chacha_qr();
        qr.steps[1].rot = 16;
        qr.steps[3].rot = 8;
        assert!(qr.is_byte_aligned());
        assert_eq!(qr.simd_instructions(), 12);
    }

    #[test]
    fn the_positive_control_reproduces_chachas_four_rounds() {
        // ChaCha's QR on ChaCha's wiring must measure what the registry
        // measures, or nothing below it means anything. This caught a 2x unit
        // error in topology.rs and it is kept here for the same reason.
        let p = QrPermutation::with_chacha_wiring(chacha_qr());
        let samples = recommended_samples(LANES * 32, 0.12);
        assert_eq!(
            rounds_to_avalanche(&p, 12, samples, 0.12, 1).rounds_to_avalanche,
            Some(4)
        );
    }

    #[test]
    fn degenerate_steps_are_rejected() {
        let mut qr = chacha_qr();
        qr.steps[0].add_from = qr.steps[0].add_to; // x += x
        assert!(!qr.is_legal());
        let mut qr = chacha_qr();
        qr.steps[0].xor_into = qr.steps[0].add_to; // x ^= x zeroes it
        assert!(!qr.is_legal());
    }

    #[test]
    fn generated_quarter_rounds_are_legal_and_respect_the_arm() {
        let mut probe = Probe::new(3);
        for _ in 0..500 {
            let free = random_qr(&mut probe, false);
            assert!(free.is_legal() && free.writes_every_lane());
            let aligned = random_qr(&mut probe, true);
            assert!(aligned.is_legal() && aligned.writes_every_lane());
            assert!(aligned.is_byte_aligned());
            assert_eq!(aligned.simd_instructions(), 12);
        }
    }
}
