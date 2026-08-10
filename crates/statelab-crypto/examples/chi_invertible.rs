//! AND-BASED NONLINEARITY, DONE CORRECTLY — the rebuild after a real error.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example chi_invertible
//! ```
//!
//! ## The error this exists to correct
//!
//! `PHASE_T` screened five `chi`-based shapes and `chi_stride_search` screened
//! six more. **Every one of them used `chi` on 16 lanes, and `chi` is a
//! bijection only for ODD lane counts.** Verified exhaustively:
//!
//! ```text
//! chi on  3 lanes: image      8/8       BIJECTION
//! chi on  4 lanes: image     12/16      NOT INVERTIBLE
//! chi on  5 lanes: image     32/32      BIJECTION      <- Ascon, Keccak
//! chi on  6 lanes: image     56/64      NOT INVERTIBLE
//! chi on 15 lanes: image  32768/32768   BIJECTION
//! chi on 16 lanes: image  65280/65536   NOT INVERTIBLE <- what was measured
//! ```
//!
//! `chi_stride_search` compounded it: its linear layer was `x ^= rot(x, r)`,
//! which has GF(2) rank 31 or lower on 32 bits **for every rotation amount** —
//! never invertible either.
//!
//! **So none of those eleven shapes was a permutation.** They lose entropy every
//! round, which is a complete alternative explanation for taking 6–11 rounds to
//! saturate where the prediction was fewer than 4. `PHASE_T`'s conclusion that
//! "AND-based shapes lose" was **not established**, because no valid AND-based
//! shape was ever tested, and its §3 claim that reach is the binding constraint
//! rests on a comparison between two lossy maps.
//!
//! The crate has had `structural::bijectivity` since `PHASE_1`. It was not run.
//!
//! ## The fix: nonlinearity that is invertible by construction
//!
//! `chi`'s invertibility depends on lane count because it substitutes **every**
//! word simultaneously from the original values. A **Feistel** arrangement does
//! not have that problem at any width: update one half using only the *other*
//! half, then swap roles. Each step is undone by recomputing the same function
//! from words it did not touch — the identical reason ChaCha's `a += b` and
//! `d ^= a` steps are invertible.
//!
//! Same operation set (`not`, `and`, `xor`), same cost (3 ops per word), same
//! 16-word state — **invertible at every width, by construction rather than by
//! luck of parity.**
//!
//! ## Every design here is bijectivity-checked BEFORE it is measured
//!
//! Not asserted in a comment. `verify_invertible` runs at startup and the
//! example refuses to measure anything that fails:
//!
//! * the nonlinear layer, exhaustively on 1-bit lanes;
//! * the linear layer, by GF(2) rank of its 32x32 matrix over the word.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **All designs pass the bijectivity check.** If one fails, it is a bug in
//!    this file and nothing is measured — the point is that the check runs
//!    first, not that it always passes.
//! 2. **Feistel-chi saturates faster than the broken 16-lane `chi` did (6–11
//!    rounds).** If entropy loss was the real cause of `PHASE_T`'s slow
//!    saturation, removing it should show here. If the rounds are unchanged,
//!    then `PHASE_T` §3's geometric explanation was right after all and the
//!    non-invertibility was incidental.
//! 3. **Nothing beats ChaCha's 192.** The cheapest design here is 64 ops/round
//!    and needs 3 rounds; ChaCha needs 2 merely to reach every word.
//!
//! Prediction 2 is the one that matters: it decides whether `PHASE_T` §3's
//! mechanism claim is restored or stays retracted.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::permutation::Permutation;
use statelab_crypto::saturation::saturation_point;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const WORDS: usize = 16;
const HALF: usize = WORDS / 2;
const MAX_ROUNDS: usize = 20;
const SEEDS: [u64; 3] = [12345, 0x5EED_0002, 0xC0FF_EE03];
const BAR: usize = 192;

/// Linear layer over one 32-bit word.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Linear {
    /// Pure rotation. Invertible trivially — it permutes bit positions — but
    /// provides no intra-word mixing on its own. 1 op per word.
    Rotate(u32),
    /// `x ^= rot(x,a) ^ rot(x,b)`. Invertible only for some (a,b); the pair
    /// used here is rank-checked at startup. 4 ops per word.
    XorRot2(u32, u32),
}

impl Linear {
    fn ops_per_word(&self) -> usize {
        match self {
            Linear::Rotate(_) => 1,
            Linear::XorRot2(_, _) => 4,
        }
    }
    #[inline]
    fn apply(&self, w: &mut u32) {
        match *self {
            Linear::Rotate(r) => *w = w.rotate_left(r),
            Linear::XorRot2(a, b) => *w ^= w.rotate_right(a) ^ w.rotate_right(b),
        }
    }
    /// GF(2) rank of the map over 32 bits. Full rank = invertible.
    fn rank(&self) -> usize {
        let mut rows: Vec<u32> = (0..32)
            .map(|j| {
                let mut v = 1u32 << j;
                self.apply(&mut v);
                v
            })
            .collect();
        let mut r = 0usize;
        for c in 0..32 {
            let Some(p) = (r..rows.len()).find(|&i| (rows[i] >> c) & 1 == 1) else {
                continue;
            };
            rows.swap(r, p);
            for i in 0..rows.len() {
                if i != r && (rows[i] >> c) & 1 == 1 {
                    rows[i] ^= rows[r];
                }
            }
            r += 1;
        }
        r
    }
}

#[derive(Clone, Copy)]
struct Design {
    name: &'static str,
    lin: Linear,
    /// Stride used inside each Feistel half.
    stride: usize,
}

impl Design {
    /// Feistel-chi: 3 ops per word. Linear: per its own count.
    fn ops_per_round(&self) -> usize {
        3 * WORDS + self.lin.ops_per_word() * WORDS
    }

    /// Update `dst` half using only `src` half — invertible at any width.
    #[inline]
    fn feistel_half(&self, x: &mut [u32; WORDS], dst: usize, src: usize) {
        let s = self.stride;
        let taps: [u32; HALF] = std::array::from_fn(|k| x[src + k]);
        for k in 0..HALF {
            let a = taps[(k + s) % HALF];
            let b = taps[(k + 2 * s) % HALF];
            x[dst + k] ^= !a & b;
        }
    }
}

impl Permutation for Design {
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
        // Two Feistel passes: each half updated from the other, so the whole
        // nonlinear layer is invertible regardless of lane count.
        self.feistel_half(&mut x, 0, HALF);
        self.feistel_half(&mut x, HALF, 0);
        for w in x.iter_mut() {
            self.lin.apply(w);
        }
        for (i, w) in x.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

/// Exhaustive bijectivity of one Feistel-chi pass on 1-bit lanes.
///
/// The nonlinear layer acts identically on every bit-column, so checking a
/// single column exhaustively over `2^16` states checks the layer.
fn feistel_chi_is_bijective(stride: usize) -> (usize, usize) {
    let n = 1usize << WORDS;
    let mut seen = vec![false; n];
    let mut hit = 0usize;
    for s in 0..n {
        let mut x: [u32; WORDS] = std::array::from_fn(|i| ((s >> i) & 1) as u32);
        let d = Design {
            name: "",
            lin: Linear::Rotate(0),
            stride,
        };
        d.feistel_half(&mut x, 0, HALF);
        d.feistel_half(&mut x, HALF, 0);
        let mut y = 0usize;
        for (i, w) in x.iter().enumerate() {
            y |= ((w & 1) as usize) << i;
        }
        if !seen[y] {
            seen[y] = true;
            hit += 1;
        }
    }
    (hit, n)
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
    println!("AND-BASED NONLINEARITY, DONE CORRECTLY\n");
    println!("  PHASE_T and chi_stride_search measured ELEVEN shapes that were");
    println!("  NOT PERMUTATIONS: chi is a bijection only on ODD lane counts and");
    println!("  they all used 16, and one used x ^= rot(x,r) which is never");
    println!("  invertible on 32 bits. Entropy loss is a complete alternative");
    println!("  explanation for their slow saturation.\n");
    println!("  Every design below is bijectivity-checked BEFORE measurement.\n");

    let designs = [
        Design {
            name: "feistel-chi + rot",
            lin: Linear::Rotate(7),
            stride: 1,
        },
        Design {
            name: "feistel-chi s3 + rot",
            lin: Linear::Rotate(7),
            stride: 3,
        },
        Design {
            name: "feistel-chi + xorrot2",
            lin: Linear::XorRot2(19, 28),
            stride: 1,
        },
        Design {
            name: "feistel-chi s3 + xorrot2",
            lin: Linear::XorRot2(19, 28),
            stride: 3,
        },
    ];

    // ---------------------------------------------------- bijectivity gate
    println!("== Bijectivity check (runs BEFORE any measurement) ==");
    let mut all_ok = true;
    for d in designs.iter() {
        let (hit, n) = feistel_chi_is_bijective(d.stride);
        let rank = d.lin.rank();
        let nl_ok = hit == n;
        let lin_ok = rank == 32;
        if !nl_ok || !lin_ok {
            all_ok = false;
        }
        println!(
            "  {:<26} nonlinear {hit}/{n} {:<14} linear rank {rank}/32 {}",
            d.name,
            if nl_ok { "BIJECTION" } else { "*** LOSSY ***" },
            if lin_ok {
                "invertible"
            } else {
                "*** SINGULAR ***"
            }
        );
    }
    if !all_ok {
        println!("\n  >>> A DESIGN IS NOT A PERMUTATION. NOTHING MEASURED.");
        println!("      That is the check doing its job, and it is a bug in this");
        println!("      file rather than a finding.");
        return;
    }
    println!("  >>> PREDICTION 1 HOLDS. All designs are permutations.\n");

    // ------------------------------------------------------------ measure
    println!(
        "  {:<26} {:>7} {:>9} {:>11} {:>9}",
        "design", "rounds", "ops/rnd", "TOTAL ops", "vs bar"
    );
    let mut best: Option<f64> = None;
    let mut winners = Vec::new();
    for d in designs.iter() {
        let ops = d.ops_per_round();
        match saturates(d, samples) {
            None => println!(
                "  {:<26} {:>7} {ops:>9} {:>11} {:>9}",
                d.name,
                format!(">{MAX_ROUNDS}"),
                "-",
                "-"
            ),
            Some(r) => {
                let total = r as usize * ops;
                best = Some(best.map_or(r, |b: f64| b.min(r)));
                println!(
                    "  {:<26} {r:>7.0} {ops:>9} {total:>11} {:>9}",
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
    match best {
        Some(b) if b < 6.0 => {
            println!("  >>> PREDICTION 2 HOLDS. Best invertible AND-based design");
            println!("      saturates at {b:.0} rounds, faster than the 6-11 the LOSSY");
            println!("      shapes needed. ENTROPY LOSS WAS A REAL PART OF PHASE_T's");
            println!("      SLOW SATURATION, so its §3 geometric explanation was at");
            println!("      best incomplete and stays retracted as stated.");
        }
        Some(b) => {
            println!("  >>> PREDICTION 2 FAILS. Best is {b:.0} rounds, no better than the");
            println!("      lossy shapes managed. Non-invertibility was NOT the cause of");
            println!("      PHASE_T's slow saturation, and its §3 reach explanation is");
            println!("      RESTORED — the error was real but did not drive that result.");
        }
        None => println!("  >>> Nothing saturated; prediction 2 unreadable."),
    }
    if winners.is_empty() {
        println!("  >>> PREDICTION 3 HOLDS. Nothing beats {BAR}.");
        println!("      AND-based nonlinearity is NOW genuinely tested and NOW");
        println!("      genuinely loses — which PHASE_T claimed without grounds.");
    } else {
        println!(
            "  >>> *** PREDICTION 3 FAILS — {} beat the bar. ***",
            winners.len()
        );
        for (n, r, t) in &winners {
            println!("      {n}: {r:.0} rounds, {t} ops");
        }
        println!("      SCREEN ONLY. BIC, rank, unseen seeds, cyc/B, CLAASP.");
    }
}
