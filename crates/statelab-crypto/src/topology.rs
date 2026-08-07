//! Topology search — the generation side of the gap `PHASE_L_CORPUS_ANALYSIS.md`
//! §3 identifies.
//!
//! The corpus states, twice and independently, that automated search over the
//! *wiring structure* of an ARX permutation — as opposed to over rotation
//! constants given a fixed structure — "remains a comparatively underexplored,
//! genuinely open research direction", because current tooling (CLAASP, the
//! Window Heuristic) is built to EVALUATE a human-proposed topology, not to
//! GENERATE one. This project's asset is precisely the evaluation half.
//!
//! ## The experimental design, and why it has no cost confound
//!
//! A candidate differs from ChaCha in **wiring only**:
//!
//! * 16 lanes of 32 bits — ChaCha's, and SIMD-divisible per the corpus's
//!   "single most consequential hidden decision" (lane count must divide evenly
//!   into common SIMD register widths; ChaCha's 16 words map onto four 128-bit
//!   registers).
//! * 32-bit words — the narrowest native register width across "almost any
//!   device", which is the corpus's stated decision procedure for word size.
//! * **ChaCha's own quarter round, unchanged.**
//! * A round is a **partition of all 16 lanes into 4 disjoint groups of 4**, and
//!   a topology is **two** such partitions applied alternately.
//!
//! That last constraint is what removes the cost confound. ChaCha is exactly
//! this shape — a column partition and a diagonal partition, alternating — so a
//! candidate performs **the same 4 quarter-rounds per round, over the same 16
//! lanes, with the same register pressure and memory traffic**. Any difference
//! in rounds-to-avalanche is attributable to wiring and nothing else.
//! Methodological items (7) and (8) are designed out rather than corrected for.
//!
//! ## *** THE UNIT, AND THE 2x PHANTOM IT ALMOST PRODUCED ***
//!
//! The first version of this module applied all 8 groups per `round()` call — a
//! full double round — while the registered `chacha` permutation's round is a
//! **half** round (4 quarter-rounds). The positive control caught it
//! immediately: ChaCha's own wiring measured 2 where the registry measures 4.
//!
//! Nothing was wrong with the measurement. The **units** differed by a factor of
//! two, and without the control every candidate would have been reported as
//! twice as good as ChaCha on a mismatch alone. One round here is now one
//! ChaCha round, and [`chacha_topology`] must measure 4 or the harness is wrong.
//!
//! ## What a good result here IS NOT
//!
//! `PHASE_L` §4 quotes the corpus directly: SAC/BIC/avalanche are **proxies**,
//! and "a design can score excellently on SAC/BIC/avalanche testing while still
//! having an exploitable low-probability-but-nonzero differential trail that
//! only a genuine MILP/SAT trail search would surface". Anything ranked highly
//! here is **a candidate worth evaluating properly** — the corpus warns
//! explicitly that treating search output as pre-validated "is a real, avoidable
//! mistake". Phase 7 via CLAASP is the gate and it has not been run.

use crate::permutation::Permutation;

pub const LANES: usize = 16;
pub const ARITY: usize = 4;
/// Groups per round: a partition of all 16 lanes into 4 disjoint groups of 4.
pub const GROUPS_PER_ROUND: usize = LANES / ARITY;
/// ChaCha alternates two partitions (columns, then diagonals).
pub const PARTITIONS: usize = 2;

/// One round's wiring: a partition of the 16 lanes into 4 groups of 4.
pub type Partition = [[u8; ARITY]; GROUPS_PER_ROUND];

/// A candidate topology: two partitions, applied alternately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology {
    pub partitions: [Partition; PARTITIONS],
}

/// ChaCha's own wiring — columns, then diagonals.
///
/// The search's **positive control**. It must measure 4 rounds to avalanche,
/// matching the registered `chacha`. If it does not, the harness is wrong and no
/// candidate ranking below it means anything.
pub fn chacha_topology() -> Topology {
    Topology {
        partitions: [
            [[0, 4, 8, 12], [1, 5, 9, 13], [2, 6, 10, 14], [3, 7, 11, 15]],
            [[0, 5, 10, 15], [1, 6, 11, 12], [2, 7, 8, 13], [3, 4, 9, 14]],
        ],
    }
}

fn is_partition(p: &Partition) -> bool {
    let mut seen = [false; LANES];
    for g in p {
        for &l in g {
            let l = l as usize;
            if l >= LANES || seen[l] {
                return false;
            }
            seen[l] = true;
        }
    }
    seen.iter().all(|&s| s)
}

impl Topology {
    /// Both halves are genuine partitions — every lane used exactly once per
    /// round. This is what equalises cost against ChaCha.
    pub fn is_balanced(&self) -> bool {
        self.partitions.iter().all(is_partition)
    }

    /// Co-occurrence graph over **both** partitions: lanes sharing any group.
    fn adjacency(&self) -> [[bool; LANES]; LANES] {
        let mut adj = [[false; LANES]; LANES];
        for p in &self.partitions {
            for g in p {
                for (i, &a) in g.iter().enumerate() {
                    for &b in g.iter().skip(i + 1) {
                        adj[a as usize][b as usize] = true;
                        adj[b as usize][a as usize] = true;
                    }
                }
            }
        }
        adj
    }

    /// Diameter of the co-occurrence graph, or `None` if disconnected.
    ///
    /// **Lower-bounds rounds-to-avalanche**, because dependency spreads at most
    /// one graph hop per round. A disconnected graph never reaches full
    /// dependency at any round count — the corpus's point that a single grouping
    /// alone "can never mix data *between* groups, no matter how many rounds
    /// run". Costs microseconds where the avalanche measurement costs seconds,
    /// which is what makes it worth filtering on first.
    pub fn diameter(&self) -> Option<usize> {
        let adj = self.adjacency();
        let mut worst = 0usize;
        for start in 0..LANES {
            let mut dist = [usize::MAX; LANES];
            dist[start] = 0;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start);
            while let Some(u) = queue.pop_front() {
                for v in 0..LANES {
                    if adj[u][v] && dist[v] == usize::MAX {
                        dist[v] = dist[u] + 1;
                        queue.push_back(v);
                    }
                }
            }
            for d in dist {
                if d == usize::MAX {
                    return None;
                }
                worst = worst.max(d);
            }
        }
        Some(worst)
    }
}

/// ChaCha's quarter round, unchanged. Held fixed so the search varies wiring and
/// nothing else.
#[inline]
fn quarter_round(w: &mut [u32; LANES], a: usize, b: usize, c: usize, d: usize) {
    w[a] = w[a].wrapping_add(w[b]);
    w[d] = (w[d] ^ w[a]).rotate_left(16);
    w[c] = w[c].wrapping_add(w[d]);
    w[b] = (w[b] ^ w[c]).rotate_left(12);
    w[a] = w[a].wrapping_add(w[b]);
    w[d] = (w[d] ^ w[a]).rotate_left(8);
    w[c] = w[c].wrapping_add(w[d]);
    w[b] = (w[b] ^ w[c]).rotate_left(7);
}

/// A candidate permutation: ChaCha's quarter round over an arbitrary wiring.
pub struct TopologyPermutation {
    pub topology: Topology,
}

impl Permutation for TopologyPermutation {
    fn name(&self) -> &'static str {
        "topology-candidate"
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
        // One round = one partition = 4 quarter-rounds, matching ChaCha's unit.
        for g in &self.topology.partitions[round_index % PARTITIONS] {
            quarter_round(
                &mut w,
                g[0] as usize,
                g[1] as usize,
                g[2] as usize,
                g[3] as usize,
            );
        }
        for (i, wi) in w.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&wi.to_le_bytes());
        }
    }
}

fn random_partition(probe: &mut crate::avalanche::Probe) -> Partition {
    let mut lanes: [u8; LANES] = std::array::from_fn(|i| i as u8);
    for i in (1..LANES).rev() {
        let j = (probe.next_u64() % (i as u64 + 1)) as usize;
        lanes.swap(i, j);
    }
    let mut p: Partition = [[0u8; ARITY]; GROUPS_PER_ROUND];
    for (gi, chunk) in lanes.chunks(ARITY).enumerate() {
        p[gi].copy_from_slice(chunk);
    }
    p
}

/// Draws a balanced topology: two independent random partitions of the 16 lanes.
pub fn random_topology(probe: &mut crate::avalanche::Probe) -> Topology {
    Topology {
        partitions: [random_partition(probe), random_partition(probe)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avalanche::{recommended_samples, rounds_to_avalanche, Probe};

    fn samples() -> usize {
        recommended_samples(LANES * 32, 0.12)
    }

    #[test]
    fn chachas_own_topology_is_balanced_and_has_diameter_two() {
        let t = chacha_topology();
        assert!(t.is_balanced(), "ChaCha's wiring must be two partitions");
        assert_eq!(
            t.diameter(),
            Some(2),
            "column-then-diagonal joins every lane pair within two hops"
        );
    }

    #[test]
    fn the_positive_control_reproduces_chachas_four_rounds() {
        // THE unit check. This failing once already caught a 2x phantom
        // improvement caused by measuring double rounds against half rounds.
        let p = TopologyPermutation {
            topology: chacha_topology(),
        };
        let sweep = rounds_to_avalanche(&p, 12, samples(), 0.12, 1);
        assert_eq!(
            sweep.rounds_to_avalanche,
            Some(4),
            "ChaCha's wiring here must measure what the registry measures"
        );
    }

    #[test]
    fn a_disconnected_topology_is_rejected_by_the_diameter_filter() {
        // Both partitions keep lanes 0-7 and 8-15 apart forever.
        let half: Partition = [[0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11], [12, 13, 14, 15]];
        let other: Partition = [[0, 2, 1, 3], [4, 6, 5, 7], [8, 10, 9, 11], [12, 14, 13, 15]];
        let t = Topology {
            partitions: [half, other],
        };
        assert!(t.is_balanced(), "still a valid partition pair");
        assert_eq!(t.diameter(), None, "disconnected must report None");
    }

    #[test]
    fn generated_topologies_are_balanced() {
        let mut probe = Probe::new(7);
        for _ in 0..500 {
            assert!(random_topology(&mut probe).is_balanced());
        }
    }

    #[test]
    fn diameter_lower_bounds_rounds_to_avalanche() {
        // The corpus's claim, checked rather than assumed: dependency spreads at
        // most one graph hop per round, so no topology can avalanche in fewer
        // rounds than its diameter.
        let mut probe = Probe::new(11);
        for _ in 0..8 {
            let t = random_topology(&mut probe);
            let Some(d) = t.diameter() else { continue };
            let p = TopologyPermutation { topology: t };
            if let Some(r) = rounds_to_avalanche(&p, 12, samples(), 0.12, 1).rounds_to_avalanche {
                assert!(r >= d, "rounds {r} below diameter {d} — bound violated");
            }
        }
    }
}
