//! AVENUE 2 — STRUCTURALLY DIFFERENT ROUND FUNCTIONS, screened on TOTAL
//! operations to diffusion saturation.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example shape_search
//! ```
//!
//! ## What makes this a different search from every previous one
//!
//! `PHASE_M` varied wiring, `PHASE_N` varied the quarter round's rotation
//! constants, H4 varied word width, `PHASE_O` varied byte alignment. **All four
//! varied parameters inside one fixed shape: a 4-step ARX quarter round applied
//! to disjoint groups of four words.** A fifth pass in that box is the same
//! attempt again.
//!
//! This varies the shape itself — **the operation set and the step topology**:
//!
//! * nonlinearity from **AND** (Ascon/Keccak `chi`) instead of modular addition
//! * **every word touched every round** instead of quarter-round grouping
//! * **step counts other than four**
//!
//! ## The metric, and why it is not the one previous phases used
//!
//! Previous phases optimised **rounds-to-saturation** and **cost-per-round**
//! separately, which is how a design that wins one and loses the other gets
//! counted as progress. `PHASE_M` produced three wirings better than ChaCha at
//! three rounds and identical at four — better per round, zero rounds bought.
//!
//! The fitness here is the **product**:
//!
//! ```text
//! total operations to saturation = ops_per_round x rounds_to_saturation
//! ```
//!
//! ChaCha's number is the bar. A shape only counts if the product is lower.
//!
//! **The op model is deliberately crude and uniform:** every `add`, `xor`,
//! `and`, `not` and constant rotation counts as one. It is a SCREEN, not a cost
//! measurement — `PHASE_O` is the standing lesson that an instruction count is
//! not a cycle count, and anything surviving this gets measured for real.
//!
//! ## Why timing does not enter here
//!
//! `PHASE_O` §1.4 established that diffusion results are unaffected by the
//! measurement trait's per-round load/store, because rounds-to-avalanche is a
//! property of the permutation rather than of how often its state is copied.
//! Op counts are analytic. So this screen is immune to the harness tax **and**
//! to the memory-pressure contamination that corrupted `PHASE_S` §2.2.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **ChaCha saturates at 4 rounds, 48 ops/round, 192 total.** POSITIVE
//!    CONTROL — `PHASE_P` measured the 4, and if it does not reproduce, nothing
//!    below is readable.
//! 2. **`chi` shapes saturate in FEWER ROUNDS than ChaCha.** Every word is
//!    touched by nonlinearity every round, where a ChaCha round only mixes
//!    within disjoint groups of four.
//! 3. **They still LOSE on total operations**, because touching every word every
//!    round costs proportionally more per round than it saves in rounds. This is
//!    the same shape as `PHASE_S`'s Ascon result: the cheaper mechanism does not
//!    survive the accounting it is embedded in.
//!
//! **Prediction 3 failing is the entire point of running this.** A shape whose
//! product beats 192 is the first genuine candidate this project has produced
//! from a structural search, and it would be reported as such — after
//! confirmation on disjoint seeds, not before.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::permutation::Permutation;
use statelab_crypto::saturation::saturation_point;
use statelab_crypto::systems::ChaCha;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const WORDS: usize = 16;
const MAX_ROUNDS: usize = 16;
const SEEDS: [u64; 3] = [12345, 0x5EED_0002, 0xC0FF_EE03];

/// A round shape: a nonlinear layer plus a linear layer over 16 32-bit words.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nonlinear {
    /// Keccak/Ascon `chi` across all 16 words: `x[i] ^= !x[i+1] & x[i+2]`.
    /// 3 ops per word, and it is the ONLY inter-word mixing in these shapes.
    Chi,
    /// `chi` on 8 words at a time, alternating halves by round — half the cost,
    /// half the reach.
    ChiHalf,
}

#[derive(Clone, Copy)]
struct Shape {
    name: &'static str,
    nl: Nonlinear,
    /// Rotation amounts applied per word as `x ^= x.rot(a) ^ x.rot(b)`.
    /// One entry means a single rotation-xor; two means Ascon's shape.
    rots: &'static [(u32, u32)],
    /// Extra inter-word linear step: XOR each word with a rotated neighbour.
    neighbour_xor: bool,
}

impl Shape {
    /// Uniform op count per round. Stated crudely and applied identically to
    /// every shape including the control, so the comparison is internally fair
    /// even though it is not a cycle count.
    fn ops_per_round(&self) -> usize {
        let nl = match self.nl {
            // not + and + xor, per word
            Nonlinear::Chi => 3 * WORDS,
            Nonlinear::ChiHalf => 3 * (WORDS / 2),
        };
        // each (a,b) pair is 2 rotations + 2 xors per word
        let lin = self.rots.len() * 4 * WORDS;
        let nb = if self.neighbour_xor { 2 * WORDS } else { 0 };
        nl + lin + nb
    }
}

impl Permutation for Shape {
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
        let mut x = [0u32; WORDS];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }

        // ---- nonlinear layer: the only inter-word mixing that is not linear
        let src = x;
        match self.nl {
            Nonlinear::Chi => {
                for i in 0..WORDS {
                    x[i] = src[i] ^ (!src[(i + 1) % WORDS] & src[(i + 2) % WORDS]);
                }
            }
            Nonlinear::ChiHalf => {
                let off = (round_index % 2) * (WORDS / 2);
                for k in 0..WORDS / 2 {
                    let i = off + k;
                    let a = off + (k + 1) % (WORDS / 2);
                    let b = off + (k + 2) % (WORDS / 2);
                    x[i] = src[i] ^ (!src[a] & src[b]);
                }
            }
        }

        // ---- linear layer, word-local (Ascon's shape)
        for w in x.iter_mut() {
            for &(a, b) in self.rots {
                *w ^= w.rotate_right(a) ^ w.rotate_right(b);
            }
        }

        // ---- optional inter-word linear step
        if self.neighbour_xor {
            let src = x;
            for i in 0..WORDS {
                x[i] ^= src[(i + 7) % WORDS].rotate_left(11);
            }
        }

        for (i, w) in x.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

/// ChaCha's ops per round: 4 quarter rounds x (4 add + 4 xor + 4 rot).
const CHACHA_OPS: usize = 4 * 12;

/// Rounds at which `perm` saturates and stays, worst case over disjoint seeds.
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
    println!("AVENUE 2 — SHAPE SEARCH, screened on TOTAL ops to saturation\n");
    println!(
        "  avalanche {samples} samples, tolerance {TOLERANCE}, {} disjoint seeds",
        SEEDS.len()
    );
    println!("  saturation by the crate's STAYS-DOWN criterion, worst seed wins");
    println!("  op model: add/xor/and/not/rot all count 1. A SCREEN, NOT A COST.\n");
    println!("  PREDICTION 1: ChaCha saturates at 4, 48 ops/round, 192 total.");
    println!("                POSITIVE CONTROL.");
    println!("  PREDICTION 2: chi shapes saturate in FEWER rounds than ChaCha.");
    println!("  PREDICTION 3: they still LOSE on total ops — touching every word");
    println!("                every round costs more per round than it saves in");
    println!("                rounds. PREDICTION 3 FAILING IS THE POINT.\n");

    let shapes = [
        Shape {
            name: "chi + 2rot (ascon-like)",
            nl: Nonlinear::Chi,
            rots: &[(19, 28), (7, 41)],
            neighbour_xor: false,
        },
        Shape {
            name: "chi + 1rot",
            nl: Nonlinear::Chi,
            rots: &[(19, 28)],
            neighbour_xor: false,
        },
        Shape {
            name: "chi + 1rot + nbr",
            nl: Nonlinear::Chi,
            rots: &[(19, 28)],
            neighbour_xor: true,
        },
        Shape {
            name: "chi-half + 1rot",
            nl: Nonlinear::ChiHalf,
            rots: &[(19, 28)],
            neighbour_xor: false,
        },
        Shape {
            name: "chi-half + 1rot + nbr",
            nl: Nonlinear::ChiHalf,
            rots: &[(19, 28)],
            neighbour_xor: true,
        },
    ];

    // ---- control first
    let cc_sat = saturates(&ChaCha, samples);
    let cc_total = cc_sat.map(|r| r as usize * CHACHA_OPS);
    println!(
        "  {:<26} {:>7} {:>9} {:>11} {:>9}",
        "shape", "rounds", "ops/rnd", "TOTAL ops", "vs chacha"
    );
    match (cc_sat, cc_total) {
        (Some(r), Some(t)) => println!(
            "  {:<26} {r:>7.0} {CHACHA_OPS:>9} {t:>11} {:>9}",
            "chacha (CONTROL)", "1.00x"
        ),
        _ => println!("  chacha (CONTROL)  NEVER SATURATES — screen is invalid"),
    }

    let Some(bar) = cc_total else {
        println!("\n  >>> CONTROL FAILED. Nothing below can be read.");
        return;
    };

    let mut winners = Vec::new();
    for s in shapes.iter() {
        let ops = s.ops_per_round();
        match saturates(s, samples) {
            None => println!(
                "  {:<26} {:>7} {ops:>9} {:>11} {:>9}",
                s.name, ">16", "-", "-"
            ),
            Some(r) => {
                let total = r as usize * ops;
                let ratio = total as f64 / bar as f64;
                println!(
                    "  {:<26} {r:>7.0} {ops:>9} {total:>11} {:>9}",
                    s.name,
                    format!("{ratio:.2}x")
                );
                if total < bar {
                    winners.push((s.name, r, total));
                }
            }
        }
    }

    println!("\n== Verdict ==");
    if cc_sat == Some(4.0) {
        println!("  >>> PREDICTION 1 HOLDS. ChaCha saturates at 4, reproducing PHASE_P.");
    } else {
        println!("  >>> PREDICTION 1 FAILS — chacha saturated at {cc_sat:?}, not 4.");
        println!("      The screen is not comparable with PHASE_P and is suspect.");
    }
    if winners.is_empty() {
        println!("  >>> PREDICTION 3 HOLDS. No shape beats {bar} total ops.");
        println!("      The cheaper nonlinearity does not survive the accounting,");
        println!("      the same way Ascon's cheaper permutation did not survive");
        println!("      the sponge rate (PHASE_S §2.1). A clean negative.");
    } else {
        println!(
            "  >>> *** PREDICTION 3 FAILS. {} SHAPE(S) BEAT THE BAR. ***",
            winners.len()
        );
        for (n, r, t) in &winners {
            println!("      {n}: {r:.0} rounds, {t} ops vs chacha's {bar}");
        }
        println!("      THIS IS A SCREEN, NOT A FINDING. Required before any claim:");
        println!("      confirmation on unseen seeds, BIC and GF(2) rank, a real");
        println!("      cyc/B measurement (item 16, PHASE_O), and CLAASP.");
    }
}
