//! AVENUE 2 — ITEM 1: does the quarter round need FOUR steps?
//!
//! ```text
//! cargo run -p statelab-crypto --release --example arx_step_search
//! ```
//!
//! ## Why this exists, and why it was owed
//!
//! Avenue 2's remit named three axes: AND-based nonlinearity, mixing graphs
//! that are not quarter-round grouping, and **step counts other than four**.
//! `PHASE_T` covered the first two and skipped the third — every shape it
//! screened was `chi`-based. `PHASE_T` §4 recorded that gap as owed. This pays
//! it.
//!
//! ## The question, stated as arithmetic before it is measured
//!
//! ChaCha's quarter round is four steps, each `add + xor + rot` = 3 ops, so
//! 12 ops per quarter round and **48 per round over four groups**. It saturates
//! at 4 rounds. **192 total operations — the bar.**
//!
//! A quarter round with `N` steps costs `12N` per round, so it beats the bar iff
//!
//! ```text
//! 12N x rounds_to_saturation  <  192      i.e.   rounds < 16 / N
//! ```
//!
//! | N | ops/round | rounds needed to WIN |
//! |---|---|---|
//! | 2 | 24 | fewer than 8 |
//! | 3 | 36 | 5 or fewer |
//! | 4 | 48 | **ChaCha: 4. This is the control.** |
//! | 5 | 60 | 3 or fewer |
//! | 6 | 72 | 2 or fewer |
//! | 8 | 96 | 1 |
//!
//! `[HYP]` **Fewer steps per quarter round may be the better trade.** ChaCha's
//! column/diagonal alternation is what supplies inter-group reach, and `PHASE_T`
//! §3 established that reach — not local mixing — is the binding constraint on
//! saturation. A cheaper quarter round buys more alternations for the same
//! budget. Two steps at 7 rounds would be 168 against ChaCha's 192.
//!
//! ## The built-in control
//!
//! **N = 4 with ChaCha's rotation constants IS ChaCha**, driven through this
//! example's own parameterised code path rather than through `systems::ChaCha`.
//! It must land on 4 rounds and 192 ops. If it does not, the parameterisation is
//! wrong and every other row is meaningless — the same role the positive control
//! played in `PHASE_O` §2.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **N = 4 reproduces ChaCha exactly: 4 rounds, 192 ops.** POSITIVE CONTROL.
//! 2. **Saturation rounds fall as N rises**, monotonically — more mixing per
//!    round is more mixing.
//! 3. **The product has a minimum at or near N = 4.** ChaCha's four steps are
//!    not arbitrary, and after `PHASE_M` found its wiring diameter-optimal and
//!    `PHASE_T` found its operation set unbeaten, the prior that its step count
//!    is also near-optimal is strong.
//!
//! **Prediction 3 failing is the point of running this.** If N = 2 or N = 3
//! wins, that is a genuinely cheaper design in the same construction, found by
//! a structural search — and it would be a screen result requiring confirmation
//! on unseen seeds, BIC, rank, real cyc/B and CLAASP before being called
//! anything at all.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::permutation::Permutation;
use statelab_crypto::saturation::saturation_point;
use statelab_crypto::topology::chacha_topology;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const WORDS: usize = 16;
const MAX_ROUNDS: usize = 20;
const SEEDS: [u64; 3] = [12345, 0x5EED_0002, 0xC0FF_EE03];

/// ChaCha's rotation constants, cycled when a quarter round has more than four
/// steps. Using ChaCha's own set keeps the ONLY variable the step count —
/// `PHASE_N` already searched rotation constants and this is not that search.
const ROTS: [u32; 4] = [16, 12, 8, 7];

/// An ARX quarter round with a parameterised number of steps, on ChaCha's
/// wiring. Step `i` alternates between ChaCha's two sub-patterns exactly as its
/// four-step quarter round does.
#[derive(Clone, Copy)]
struct ArxSteps {
    name: &'static str,
    steps: usize,
}

impl ArxSteps {
    /// 3 ops per step (add, xor, rot), 4 groups per round.
    fn ops_per_round(&self) -> usize {
        self.steps * 3 * 4
    }

    #[inline]
    fn quarter(&self, q: &mut [u32; 4]) {
        for i in 0..self.steps {
            let r = ROTS[i % ROTS.len()];
            if i % 2 == 0 {
                // a += b; d ^= a; d <<<= r
                q[0] = q[0].wrapping_add(q[1]);
                q[3] = (q[3] ^ q[0]).rotate_left(r);
            } else {
                // c += d; b ^= c; b <<<= r
                q[2] = q[2].wrapping_add(q[3]);
                q[1] = (q[1] ^ q[2]).rotate_left(r);
            }
        }
    }
}

impl Permutation for ArxSteps {
    fn name(&self) -> &'static str {
        self.name
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        8
    }
    fn round(&self, state: &mut [u8], round_index: usize) {
        let topo = chacha_topology();
        let mut x = [0u32; WORDS];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }
        for g in &topo.partitions[round_index % 2] {
            let mut q = [
                x[g[0] as usize],
                x[g[1] as usize],
                x[g[2] as usize],
                x[g[3] as usize],
            ];
            self.quarter(&mut q);
            for (k, &lane) in g.iter().enumerate() {
                x[lane as usize] = q[k];
            }
        }
        for (i, w) in x.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

/// Worst-case saturation across disjoint seeds.
fn saturates(perm: &dyn Permutation, samples: usize) -> Option<f64> {
    let mut worst: Option<f64> = None;
    for &seed in &SEEDS {
        let pts: Vec<(f64, f64)> = (1..=MAX_ROUNDS)
            .map(|r| {
                let m = avalanche_matrix(perm, r, samples, seed);
                (r as f64, m.max_deviation())
            })
            .collect();
        let r = saturation_point(&pts, TOLERANCE, 1.0)?;
        worst = Some(worst.map_or(r, |w: f64| w.max(r)));
    }
    worst
}

fn main() {
    let samples = recommended_samples(BITS, TOLERANCE);
    const BAR: usize = 192;

    println!("AVENUE 2 ITEM 1 — DOES THE QUARTER ROUND NEED FOUR STEPS?\n");
    println!(
        "  avalanche {samples} samples, tolerance {TOLERANCE}, {} disjoint seeds",
        SEEDS.len()
    );
    println!("  ChaCha's wiring and rotation constants throughout — the ONLY");
    println!("  variable is the step count. PHASE_N already searched constants.\n");
    println!("  bar = ChaCha's 4 rounds x 48 ops = {BAR} total");
    println!("  an N-step quarter round wins iff rounds < 16/N\n");
    println!("  PREDICTION 1: N=4 reproduces ChaCha — 4 rounds, 192. CONTROL.");
    println!("  PREDICTION 2: saturation rounds fall monotonically as N rises.");
    println!("  PREDICTION 3: the product bottoms at or near N=4. FAILING THIS");
    println!("                IS THE POINT — a win at N=2 or N=3 is a genuinely");
    println!("                cheaper design in the same construction.\n");

    let designs = [
        ArxSteps {
            name: "N=2",
            steps: 2,
        },
        ArxSteps {
            name: "N=3",
            steps: 3,
        },
        ArxSteps {
            name: "N=4 (chacha, CONTROL)",
            steps: 4,
        },
        ArxSteps {
            name: "N=5",
            steps: 5,
        },
        ArxSteps {
            name: "N=6",
            steps: 6,
        },
        ArxSteps {
            name: "N=8",
            steps: 8,
        },
    ];

    println!(
        "  {:<24} {:>7} {:>9} {:>11} {:>9}",
        "design", "rounds", "ops/rnd", "TOTAL ops", "vs bar"
    );

    let mut control_ok = false;
    let mut winners: Vec<(&str, f64, usize)> = Vec::new();
    let mut rounds_seen: Vec<(usize, f64)> = Vec::new();

    for d in designs.iter() {
        let ops = d.ops_per_round();
        match saturates(d, samples) {
            None => println!(
                "  {:<24} {:>7} {ops:>9} {:>11} {:>9}",
                d.name,
                format!(">{MAX_ROUNDS}"),
                "-",
                "-"
            ),
            Some(r) => {
                let total = r as usize * ops;
                rounds_seen.push((d.steps, r));
                println!(
                    "  {:<24} {r:>7.0} {ops:>9} {total:>11} {:>9}",
                    d.name,
                    format!("{:.2}x", total as f64 / BAR as f64)
                );
                if d.steps == 4 && r == 4.0 && total == BAR {
                    control_ok = true;
                }
                if total < BAR {
                    winners.push((d.name, r, total));
                }
            }
        }
    }

    println!("\n== Verdict ==");
    if control_ok {
        println!("  >>> PREDICTION 1 HOLDS. N=4 reproduces ChaCha exactly through");
        println!("      this example's own parameterised path — 4 rounds, {BAR} ops.");
    } else {
        println!("  >>> CONTROL FAILED. N=4 did not reproduce ChaCha's 4 rounds /");
        println!("      {BAR} ops. THE PARAMETERISATION IS WRONG AND NO ROW BELOW");
        println!("      CAN BE READ. Everything else here is void.");
        return;
    }

    let monotone = rounds_seen.windows(2).all(|w| w[1].1 <= w[0].1);
    if monotone {
        println!("  >>> PREDICTION 2 HOLDS. Rounds fall monotonically with N.");
    } else {
        println!("  >>> PREDICTION 2 FAILS. Rounds do NOT fall monotonically with N:");
        println!("      {rounds_seen:?}");
        println!("      More mixing per round is not always fewer rounds.");
    }

    if winners.is_empty() {
        println!("  >>> PREDICTION 3 HOLDS. No step count beats {BAR}. ChaCha's four");
        println!("      steps sit at or near the optimum of the product, alongside");
        println!("      its wiring (PHASE_M) and its operation set (PHASE_T).");
    } else {
        println!(
            "  >>> *** PREDICTION 3 FAILS. {} DESIGN(S) BEAT THE BAR. ***",
            winners.len()
        );
        for (n, r, t) in &winners {
            println!("      {n}: {r:.0} rounds, {t} ops vs the bar's {BAR}");
        }
        println!("      A SCREEN RESULT, NOT A FINDING. Before any claim: unseen");
        println!("      seeds, BIC, GF(2) rank, real cyc/B (PHASE_O), and CLAASP.");
    }
}
