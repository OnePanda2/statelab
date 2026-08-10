//! *** VOID — THIS EXAMPLE MEASURED NON-PERMUTATIONS. DO NOT CITE IT. ***
//!
//! Superseded by `chi_invertible.rs`. Retained because `PHASE_T`'s correction
//! banner cites its output as the null that exposed the error, and deleting the
//! code behind a cited log would leave the citation unverifiable.
//!
//! Two independent defects, both verified after the fact:
//!
//! * **`chi` on 16 lanes is not a bijection** — image 65,280 of 65,536. `chi`
//!   is invertible only for ODD lane counts; Keccak and Ascon use 5.
//! * **`x ^= rot(x, r)` is never invertible on 32 bits** — GF(2) rank 31 or
//!   lower for every rotation amount tested.
//!
//! So every design below loses entropy each round. Its "nothing saturates at any
//! stride" result is fully explained by that and says nothing about reach, which
//! is what it was built to test. `chi_invertible.rs` runs the intended
//! experiment on designs that are bijections by construction, with the
//! bijectivity check wired ahead of the measurement.

//! AVENUE 2 — ITEM 2: is reach really the binding constraint? Free-reach test.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example chi_stride_search
//! ```
//!
//! ## Item 2 as originally framed was arithmetically doomed
//!
//! `PHASE_T` §4 item 2 proposed "wider-reach linear layers", on the evidence
//! that adding one cross-word XOR took a `chi` shape from 11 rounds to 6.
//!
//! **That cannot work, and the arithmetic says so before any compute is spent.**
//! Every added cross-word XOR costs 2 ops per word — 32 per round — so buying
//! reach *raises* ops/round and makes the bar harder. Against ChaCha's 192:
//!
//! ```text
//! chi (48) + one rotation-xor per word (32)  = 80 ops/round -> needs <=2 rounds
//! chi (48) + rot (64) + one neighbour (32)   = 144          -> needs  <2 rounds
//! chi (48) + rot (64) + two neighbours (64)  = 176          -> needs  <2 rounds
//! ```
//!
//! Tranche 1's `chi` shapes took 6–11 rounds. **No amount of paid reach closes
//! that**, and screening it would have burned an hour to rediscover division.
//!
//! ## What is worth testing instead: reach that costs nothing
//!
//! `chi`'s stride is free. `x[i] ^= !x[i+s] & x[i+2s]` costs **exactly the same
//! for every `s`** — it is an index change, not an operation — while reach goes
//! from ±2 to ±2s.
//!
//! So this isolates reach at **constant op count**, which the tranche-1
//! comparison could not: there, reach and cost moved together, so the 11 → 6
//! result is consistent with either "reach helps" or "more mixing helps".
//!
//! `PHASE_T` §3's mechanism claim currently rests on that one confounded
//! comparison. **This is the independent test of it**, and it is worth running
//! for that reason even though the bar is out of reach.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **Saturation improves markedly as stride rises**, at identical cost. If
//!    `PHASE_T` §3 is right that reach is binding, this is where it shows.
//! 2. **Stride 8 is a special case and should be WORSE**, not better. On 16
//!    words, `i+8` and `i+16 = i` make `chi` degenerate — the taps collide with
//!    the word itself, so the map loses its cyclic structure. Included
//!    deliberately as a **negative control**: a monotone "bigger is better"
//!    result across all strides including 8 would suggest the measurement is
//!    responding to something other than reach.
//! 3. **Nothing beats 192.** The cheapest shape here is 80 ops/round and would
//!    need to saturate in 2 rounds. ChaCha needs 2 rounds merely to reach every
//!    word.
//!
//! Prediction 1 failing would **retract `PHASE_T` §3's mechanism claim**, which
//! is a more useful outcome than another row of losing numbers.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::permutation::Permutation;
use statelab_crypto::saturation::saturation_point;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const WORDS: usize = 16;
const MAX_ROUNDS: usize = 24;
const SEEDS: [u64; 3] = [12345, 0x5EED_0002, 0xC0FF_EE03];
const BAR: usize = 192;

/// `chi` at a parameterised stride, plus the cheapest linear layer that gives
/// any intra-word diffusion at all.
///
/// Without a rotation, `chi` is bitwise: bit `i` of a word can never influence
/// bit `j`. One rotation-xor per word is therefore the floor, not a choice.
#[derive(Clone, Copy)]
struct ChiStride {
    name: &'static str,
    stride: usize,
    rot: u32,
}

impl ChiStride {
    /// chi: not+and+xor per word. linear: rot+xor per word.
    fn ops_per_round(&self) -> usize {
        3 * WORDS + 2 * WORDS
    }
}

impl Permutation for ChiStride {
    fn name(&self) -> &'static str {
        self.name
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        8
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        let mut x = [0u32; WORDS];
        for (i, w) in x.iter_mut().enumerate() {
            *w = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }
        let src = x;
        let s = self.stride;
        for i in 0..WORDS {
            x[i] = src[i] ^ (!src[(i + s) % WORDS] & src[(i + 2 * s) % WORDS]);
        }
        for w in x.iter_mut() {
            *w ^= w.rotate_right(self.rot);
        }
        for (i, w) in x.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

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
    println!("AVENUE 2 ITEM 2 — IS REACH THE BINDING CONSTRAINT? (free-reach test)\n");
    println!(
        "  avalanche {samples} samples, tolerance {TOLERANCE}, {} disjoint seeds",
        SEEDS.len()
    );
    println!("  EVERY design below costs IDENTICALLY: 80 ops/round.");
    println!("  Only chi's stride changes, and a stride change is an index");
    println!("  change, not an operation. Reach is isolated from cost.\n");
    println!("  PREDICTION 1: saturation improves markedly as stride rises.");
    println!("                PHASE_T §3's mechanism claim stands or falls here.");
    println!("  PREDICTION 2: stride 8 is WORSE — on 16 words the taps collide.");
    println!("                NEGATIVE CONTROL: monotone gains including s=8 would");
    println!("                mean the measurement is tracking something else.");
    println!("  PREDICTION 3: nothing beats {BAR}. 80 ops/round needs 2 rounds,");
    println!("                and ChaCha needs 2 merely to reach every word.\n");

    let designs = [
        ChiStride {
            name: "stride 1 (tranche 1)",
            stride: 1,
            rot: 7,
        },
        ChiStride {
            name: "stride 2",
            stride: 2,
            rot: 7,
        },
        ChiStride {
            name: "stride 3",
            stride: 3,
            rot: 7,
        },
        ChiStride {
            name: "stride 5",
            stride: 5,
            rot: 7,
        },
        ChiStride {
            name: "stride 7",
            stride: 7,
            rot: 7,
        },
        ChiStride {
            name: "stride 8 (NEG CONTROL)",
            stride: 8,
            rot: 7,
        },
    ];

    println!(
        "  {:<24} {:>7} {:>9} {:>11} {:>9}",
        "design", "rounds", "ops/rnd", "TOTAL ops", "vs bar"
    );

    let mut results: Vec<(usize, Option<f64>)> = Vec::new();
    let mut winners = Vec::new();
    for d in designs.iter() {
        let ops = d.ops_per_round();
        let sat = saturates(d, samples);
        results.push((d.stride, sat));
        match sat {
            None => println!(
                "  {:<24} {:>7} {ops:>9} {:>11} {:>9}",
                d.name,
                format!(">{MAX_ROUNDS}"),
                "-",
                "-"
            ),
            Some(r) => {
                let total = r as usize * ops;
                println!(
                    "  {:<24} {r:>7.0} {ops:>9} {total:>11} {:>9}",
                    d.name,
                    format!("{:.2}x", total as f64 / BAR as f64)
                );
                if total < BAR {
                    winners.push((d.name, r, total));
                }
            }
        }
    }

    println!("\n== Verdict ==");

    let s1 = results.iter().find(|(s, _)| *s == 1).and_then(|(_, r)| *r);
    let wide: Vec<f64> = results
        .iter()
        .filter(|(s, _)| (2..=7).contains(s))
        .filter_map(|(_, r)| *r)
        .collect();
    let best_wide = wide.iter().cloned().fold(f64::INFINITY, f64::min);

    match s1 {
        Some(base) if best_wide.is_finite() && best_wide < base => {
            println!("  >>> PREDICTION 1 HOLDS. Widening the stride at IDENTICAL cost");
            println!("      took saturation from {base:.0} to {best_wide:.0} rounds.");
            println!("      PHASE_T §3 is confirmed on an unconfounded comparison:");
            println!("      REACH is the binding constraint, not the operation set.");
        }
        Some(base) => {
            println!("  >>> PREDICTION 1 FAILS. Stride 1 saturated at {base:.0} and no");
            println!("      wider stride improved on it. PHASE_T §3's mechanism claim");
            println!("      RESTS ON A CONFOUNDED COMPARISON AND IS RETRACTED — the");
            println!("      11 -> 6 result there is then about added mixing, not reach.");
        }
        None => println!("  >>> Stride 1 never saturated; prediction 1 unreadable."),
    }

    let s8 = results.iter().find(|(s, _)| *s == 8).and_then(|(_, r)| *r);
    match (s8, best_wide.is_finite()) {
        (None, _) => println!("  >>> PREDICTION 2 HOLDS. Stride 8 never saturates — the taps"),
        (Some(r), true) if r > best_wide => {
            println!("  >>> PREDICTION 2 HOLDS. Stride 8 is worse ({r:.0} rounds) than the");
            println!("      best wide stride, as the degenerate taps predict.");
        }
        (Some(r), _) => {
            println!("  >>> PREDICTION 2 FAILS. Stride 8 saturated at {r:.0}, no worse than");
            println!("      the others. The negative control did not fire, so treat");
            println!("      prediction 1's reading with suspicion.");
        }
    }
    if s8.is_none() {
        println!("      collide with the word itself on a 16-word state.");
    }

    if winners.is_empty() {
        println!("  >>> PREDICTION 3 HOLDS. Nothing beats {BAR}, as the arithmetic said.");
    } else {
        println!(
            "  >>> *** PREDICTION 3 FAILS — {} beat the bar. ***",
            winners.len()
        );
        for (n, r, t) in &winners {
            println!("      {n}: {r:.0} rounds, {t} ops");
        }
        println!("      SCREEN RESULT ONLY. Unseen seeds, BIC, rank, cyc/B, CLAASP.");
    }
}
