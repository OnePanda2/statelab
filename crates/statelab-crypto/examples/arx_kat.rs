//! REFERENCE VECTORS for the CLAASP cross-check.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example arx_kat
//! ```
//!
//! ## Why this exists
//!
//! CLAASP will be asked for differential and linear bounds on the N=5 design.
//! **A bound is only about the object the model actually encodes**, and the
//! model will be a hand-written Python file in CLAASP's component algebra. If
//! it encodes a slightly different permutation — a rotation direction flipped,
//! a step ordering wrong, the diagonal wiring transposed — it will still
//! produce a bound, and that bound will look exactly as real as a correct one.
//!
//! **This project has already been saved once by exactly this check**: a
//! `pdftotext` transcription dropped the `(1 XOR .)` complement from ChiChi and
//! the bijectivity gate caught it before anything was built.
//!
//! So the CLAASP model must reproduce these vectors before any bound it emits
//! is quoted. This prints the **bare permutation** — no constants, no counter,
//! no feed-forward — because that is what `ChachaPermutation` models.

use statelab_crypto::branch::ROTS;

/// The `N`-step ARX quarter round, identical to `arx_step_cycles.rs` but on a
/// caller-supplied state so a known input can be pushed through it.
fn quarter(w: &mut [u32; 16], n: usize, a: usize, b: usize, c: usize, d: usize) {
    for i in 0..n {
        let r = ROTS[i % 4];
        if i % 2 == 0 {
            w[a] = w[a].wrapping_add(w[b]);
            w[d] = (w[d] ^ w[a]).rotate_left(r);
        } else {
            w[c] = w[c].wrapping_add(w[d]);
            w[b] = (w[b] ^ w[c]).rotate_left(r);
        }
    }
}

fn permute(w: &mut [u32; 16], n: usize, rounds: usize) {
    for r in 0..rounds {
        if r % 2 == 0 {
            for k in 0..4 {
                quarter(w, n, k, 4 + k, 8 + k, 12 + k);
            }
        } else {
            quarter(w, n, 0, 5, 10, 15);
            quarter(w, n, 1, 6, 11, 12);
            quarter(w, n, 2, 7, 8, 13);
            quarter(w, n, 3, 4, 9, 14);
        }
    }
}

/// A fixed, arbitrary input state. Not ChaCha's constants — the point is to
/// exercise the permutation, not a keystream.
fn input_state() -> [u32; 16] {
    let mut w = [0u32; 16];
    for (i, x) in w.iter_mut().enumerate() {
        *x = 0x0100_0000u32
            .wrapping_mul(i as u32 + 1)
            .wrapping_add(0x89ab_cdefu32.rotate_left(i as u32));
    }
    w
}

fn main() {
    let inp = input_state();
    println!("REFERENCE VECTORS FOR THE CLAASP CROSS-CHECK");
    println!("bare permutation: no constants, no counter, NO FEED-FORWARD\n");
    println!("rotations (left): {ROTS:?}");
    println!("columns:   [0,4,8,12] [1,5,9,13] [2,6,10,14] [3,7,11,15]");
    println!("diagonals: [0,5,10,15] [1,6,11,12] [2,7,8,13] [3,4,9,14]");
    println!("step i even: w[a]+=w[b]; w[d]=(w[d]^w[a])<<<ROTS[i%4]");
    println!("step i odd : w[c]+=w[d]; w[b]=(w[b]^w[c])<<<ROTS[i%4]\n");

    let hex = |w: &[u32; 16]| {
        w.iter()
            .map(|x| format!("{x:08x}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    println!("INPUT = [");
    println!("    {}", hex(&inp));
    println!("]\n");

    for n in [4usize, 5] {
        let tag = if n == 4 { "N4_CHACHA" } else { "N5" };
        for rounds in [1usize, 2, 3, 4, 8] {
            let mut w = inp;
            permute(&mut w, n, rounds);
            println!("{tag}_R{rounds} = [");
            println!("    {}", hex(&w));
            println!("]");
        }
        println!();
    }

    println!("A CLAASP model that does not reproduce these EXACTLY is encoding a");
    println!("different permutation, and any bound it emits is about that other");
    println!("object. Do not quote a bound until this passes.");
}
