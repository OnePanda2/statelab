//! CHICHI(SWAP) AT n=16 — the published χ-family permutation, measured.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example chichi_search
//! ```
//!
//! ## Why this replaces the Feistel test
//!
//! `PHASE_T` §6 measured AND-based nonlinearity at **5.25x** the bar using a
//! **Feistel** construction — sound, exhaustively verified, and **ad-hoc**. It
//! was built because χ is a bijection only at odd width and the state is 16
//! lanes.
//!
//! There is a published answer to exactly that problem.
//!
//! `[KNOWN — VERIFIED]` **Andreoli, Leander, Piccione & Stennes,
//! *"Generalizations of ChiChi: Families of Low-Latency Permutations in Any Even
//! Dimension"*, IACR ToSC 2025(3), pp. 800–826.
//! DOI 10.46586/tosc.v2025.i3.800-826**, sha256 `c69d5b64…acf060b9`. ChiChi
//! itself is due to **Belkheyar et al., EUROCRYPT 2025**, which has not been
//! located — see `NEXT_SESSION_QUEUE` item 2b.
//!
//! ## The construction — ToSC §3.2, "Family of a Swap"
//!
//! Split the state into **two halves of ODD length** `m` and `m̂`, apply χ within
//! each half, and **cross-feed position 0 between the halves**:
//!
//! ```text
//! y_0   = x̂_0 ⊕ ((1⊕x_1)∧x_2)              ŷ_0   = x_0 ⊕ ((1⊕x̂_1)∧x̂_2)
//! y_i   = x_i ⊕ ((1⊕x_{i+1})∧x_{i+2})      ŷ_i   = x̂_i ⊕ ((1⊕x̂_{i+1})∧x̂_{i+2})
//! ```
//!
//! **Theorem 1:** for any odd `m, m̂ ≥ 3`, this is a permutation. Odd + odd is
//! even, which is how the family reaches *any* even dimension. At n = 16 the
//! splits are 3+13, 5+11, 7+9 and their mirrors.
//!
//! ### The transcription trap, recorded because it nearly landed
//!
//! `pdftotext` drops the χ glyphs **and dropped the `(1⊕·)` complement**,
//! rendering the round as `x_i + x_{i+1}x_{i+2}`. Implemented literally that is
//! **not a permutation at any width** — verified: n=3 gives image 5/8, n=15
//! gives 31395/32768. The bijectivity gate caught it before a design was
//! measured. This file uses the complemented form, and the gate re-checks it.
//!
//! ## Cost — COUNTED, not inferred
//!
//! One NOT, one AND, one XOR per lane = **3 ops per lane, 48 per round at
//! n=16** — *identical to χ, and identical to the Feistel construction*.
//! **ChiChi buys the permutation property at even n for free.**
//!
//! An earlier `[SPEC]` estimate of ~8 ops/word was **2.7x too pessimistic and
//! described a different construction** (Kriepke–Kyureghyan's Γₙ family). It is
//! withdrawn in `PHASE_S` §6.5 and `PHASE_T` §6.6.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **Every split passes the bijectivity gate**, and the even-half negative
//!    control fails. The gate runs before any measurement.
//! 2. **ChiChi saturates no worse than the Feistel construction's 9 rounds**,
//!    since it costs the same and mixes across the whole state rather than
//!    between two fixed halves.
//! 3. **It still loses to ChaCha's 192.** At 48 + linear-layer ops per round it
//!    would need to saturate in ~2–3 rounds, and ChaCha needs 2 merely to reach
//!    every word.
//!
//! **Prediction 3 failing is the point of running this.** `PHASE_T`'s 5.25x is
//! flagged provisional precisely because it was measured on the wrong object; a
//! result here is what makes it final either way.

use statelab_crypto::avalanche::{avalanche_matrix, recommended_samples};
use statelab_crypto::permutation::Permutation;
use statelab_crypto::saturation::saturation_point;
use statelab_crypto::systems::ChaCha;

const TOLERANCE: f64 = 0.12;
const BITS: usize = 512;
const WORDS: usize = 16;
const MAX_ROUNDS: usize = 20;
const SEEDS: [u64; 3] = [12345, 0x5EED_0002, 0xC0FF_EE03];
const BAR: usize = 192;

/// Linear layer over one 32-bit word. Cost and invertibility both matter, so
/// both are reported and the rank is checked before use.
#[derive(Clone, Copy)]
enum Linear {
    /// Per-word distinct rotation, Keccak-ρ style. Invertible trivially (it
    /// permutes bit positions) and costs 1 op. Provides NO intra-word mixing on
    /// its own — `chi_invertible.rs` found a *uniform* rotation never saturates,
    /// so this uses a DISTINCT amount per lane, which that test did not.
    RhoStyle,
    /// `x ^= rot(x,a) ^ rot(x,b)`. 4 ops. Rank-checked.
    XorRot2(u32, u32),
}

impl Linear {
    fn ops_per_word(&self) -> usize {
        match self {
            Linear::RhoStyle => 1,
            Linear::XorRot2(_, _) => 4,
        }
    }
    #[inline]
    fn apply(&self, w: &mut u32, lane: usize) {
        match *self {
            // Distinct per-lane offsets; the triangular numbers Keccak uses.
            Linear::RhoStyle => {
                let r = ((lane * (lane + 1) / 2) % 31 + 1) as u32;
                *w = w.rotate_left(r);
            }
            Linear::XorRot2(a, b) => *w ^= w.rotate_right(a) ^ w.rotate_right(b),
        }
    }
    /// Full GF(2) rank over 32 bits means invertible. Lane-independent for
    /// `XorRot2`; for `RhoStyle` every lane is a rotation, always invertible.
    fn rank(&self, lane: usize) -> usize {
        let mut rows: Vec<u32> = (0..32)
            .map(|j| {
                let mut v = 1u32 << j;
                self.apply(&mut v, lane);
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
struct ChiChi {
    name: &'static str,
    /// Length of the first half. The second is `WORDS - m`. Theorem 1 requires
    /// both odd.
    m: usize,
    lin: Linear,
}

impl ChiChi {
    fn ops_per_round(&self) -> usize {
        3 * WORDS + self.lin.ops_per_word() * WORDS
    }

    /// The ToSC §3.2 swap family, applied bitsliced across 32-bit lanes.
    #[inline]
    fn nonlinear(&self, x: &mut [u32; WORDS]) {
        let m = self.m;
        let mh = WORDS - m;
        let src = *x;
        // First half: chi within, position 0 fed from the other half's position 0.
        for i in 0..m {
            let t = !src[(i + 1) % m] & src[(i + 2) % m];
            x[i] = if i == 0 { src[m] ^ t } else { src[i] ^ t };
        }
        // Second half, symmetric.
        for i in 0..mh {
            let a = m + (i + 1) % mh;
            let b = m + (i + 2) % mh;
            let t = !src[a] & src[b];
            x[m + i] = if i == 0 { src[0] ^ t } else { src[m + i] ^ t };
        }
    }
}

impl Permutation for ChiChi {
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
        self.nonlinear(&mut x);
        for (lane, w) in x.iter_mut().enumerate() {
            self.lin.apply(w, lane);
        }
        for (i, w) in x.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

/// Exhaustive bijectivity of the nonlinear layer on one bit-column.
///
/// The layer acts identically on every column, so `2^16` states settles it.
fn nonlinear_is_bijective(m: usize) -> (usize, usize) {
    let n = 1usize << WORDS;
    let mut seen = vec![false; n];
    let mut hit = 0usize;
    let d = ChiChi {
        name: "",
        m,
        lin: Linear::RhoStyle,
    };
    for s in 0..n {
        let mut x: [u32; WORDS] = std::array::from_fn(|i| ((s >> i) & 1) as u32);
        d.nonlinear(&mut x);
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
    println!("CHICHI(SWAP) AT n=16 — the published chi-family permutation\n");
    println!("  ToSC 2025(3) pp.800-826, DOI 10.46586/tosc.v2025.i3.800-826");
    println!("  Construction from §3.2; Theorem 1 requires BOTH halves odd.\n");
    println!(
        "  avalanche {samples} samples, tol {TOLERANCE}, {} disjoint seeds",
        SEEDS.len()
    );
    println!("  fitness = ops_per_round x rounds_to_saturation. Bar = ChaCha's {BAR}.\n");
    println!("  PREDICTION 1: every odd split passes the gate; even splits fail.");
    println!("  PREDICTION 2: saturates no worse than the Feistel build's 9 rounds.");
    println!("  PREDICTION 3: still loses to {BAR}. FAILING THIS IS THE POINT —");
    println!("                PHASE_T's 5.25x is flagged provisional because it");
    println!("                was measured on the wrong object.\n");

    // ------------------------------------------------ gate, before measurement
    println!("== Bijectivity gate (Theorem 1) ==");
    let mut gate_ok = true;
    for m in [3usize, 5, 7, 9, 11, 13] {
        let (hit, n) = nonlinear_is_bijective(m);
        let ok = hit == n;
        if !ok {
            gate_ok = false;
        }
        println!(
            "  odd split m={m:2}, mhat={:2}:  {hit}/{n}  {}",
            WORDS - m,
            if ok { "BIJECTION" } else { "*** LOSSY ***" }
        );
    }
    println!("  -- negative control: even splits, which Theorem 1 EXCLUDES --");
    let mut neg_ok = true;
    for m in [4usize, 6, 8] {
        let (hit, n) = nonlinear_is_bijective(m);
        if hit == n {
            neg_ok = false;
        }
        println!(
            "  even split m={m:2}, mhat={:2}: {hit}/{n}  {}",
            WORDS - m,
            if hit == n {
                "*** BIJECTION — CONTROL FAILED ***"
            } else {
                "not invertible (as predicted)"
            }
        );
    }
    if !gate_ok {
        println!("\n  >>> AN ODD SPLIT IS NOT A PERMUTATION. Theorem 1 contradicted,");
        println!("      which means THIS FILE'S TRANSCRIPTION IS WRONG. Nothing measured.");
        return;
    }
    if !neg_ok {
        println!("\n  >>> NEGATIVE CONTROL FAILED — an even split came out a bijection.");
        println!("      The gate is not discriminating and cannot license the rows above.");
        return;
    }
    println!("  >>> PREDICTION 1 HOLDS. Theorem 1 reproduced, control fired.\n");

    // ------------------------------------------------------ linear layer ranks
    for (label, lin) in [
        ("rho-style", Linear::RhoStyle),
        ("xorrot2(19,28)", Linear::XorRot2(19, 28)),
    ] {
        let bad: Vec<usize> = (0..WORDS).filter(|&l| lin.rank(l) != 32).collect();
        println!(
            "  linear {label:<16} rank 32/32 on all lanes: {}",
            if bad.is_empty() {
                "yes".to_string()
            } else {
                format!("NO — lanes {bad:?}")
            }
        );
        if !bad.is_empty() {
            println!("  >>> SINGULAR LINEAR LAYER. Nothing measured.");
            return;
        }
    }
    println!();

    // -------------------------------------------------------------- measure
    let designs = [
        ChiChi {
            name: "chichi 7+9  + rho",
            m: 7,
            lin: Linear::RhoStyle,
        },
        ChiChi {
            name: "chichi 5+11 + rho",
            m: 5,
            lin: Linear::RhoStyle,
        },
        ChiChi {
            name: "chichi 3+13 + rho",
            m: 3,
            lin: Linear::RhoStyle,
        },
        ChiChi {
            name: "chichi 7+9  + xr2",
            m: 7,
            lin: Linear::XorRot2(19, 28),
        },
        ChiChi {
            name: "chichi 5+11 + xr2",
            m: 5,
            lin: Linear::XorRot2(19, 28),
        },
        ChiChi {
            name: "chichi 3+13 + xr2",
            m: 3,
            lin: Linear::XorRot2(19, 28),
        },
    ];

    let cc = saturates(&ChaCha, samples);
    println!(
        "  {:<22} {:>7} {:>9} {:>11} {:>9}",
        "design", "rounds", "ops/rnd", "TOTAL ops", "vs bar"
    );
    match cc {
        Some(r) => println!(
            "  {:<22} {r:>7.0} {:>9} {:>11} {:>9}",
            "chacha (CONTROL)",
            48,
            r as usize * 48,
            "1.00x"
        ),
        None => {
            println!("  chacha CONTROL never saturated — screen invalid.");
            return;
        }
    }

    let mut winners = Vec::new();
    let mut best = f64::INFINITY;
    for d in designs.iter() {
        let ops = d.ops_per_round();
        match saturates(d, samples) {
            None => println!(
                "  {:<22} {:>7} {ops:>9} {:>11} {:>9}",
                d.name,
                format!(">{MAX_ROUNDS}"),
                "-",
                "-"
            ),
            Some(r) => {
                best = best.min(r);
                let total = r as usize * ops;
                println!(
                    "  {:<22} {r:>7.0} {ops:>9} {total:>11} {:>9}",
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
    if best <= 9.0 {
        println!("  >>> PREDICTION 2 HOLDS. Best ChiChi saturates at {best:.0} rounds,");
        println!("      no worse than the Feistel construction's 9.");
    } else if best.is_finite() {
        println!("  >>> PREDICTION 2 FAILS. Best is {best:.0} rounds, WORSE than the");
        println!("      ad-hoc Feistel build managed at 9 — the published construction");
        println!("      diffuses more slowly here despite costing the same.");
    } else {
        println!("  >>> PREDICTION 2 unreadable — nothing saturated.");
    }
    if winners.is_empty() {
        println!("  >>> PREDICTION 3 HOLDS. Nothing beats {BAR}. The AND-based branch");
        println!("      now loses on the PUBLISHED, state-of-the-art construction, not");
        println!("      on an ad-hoc one. PHASE_T §6.3's provisional flag can be lifted.");
    } else {
        println!(
            "  >>> *** PREDICTION 3 FAILS — {} beat the bar. ***",
            winners.len()
        );
        for (n, r, t) in &winners {
            println!("      {n}: {r:.0} rounds, {t} ops vs {BAR}");
        }
        println!("      SCREEN ONLY. Unseen seeds, BIC, GF(2) rank, real cyc/B, CLAASP.");
    }
}
