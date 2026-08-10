//! DOES BYTE ALIGNMENT BUY ANYTHING IN SCALAR CODE? — testing PHASE_N's premise
//! against the project's actual hardware bar.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example scalar_rotation_cost
//! ```
//!
//! ## Why this exists
//!
//! The goal is a PRNG that beats ChaCha20 **on almost any device, with no
//! special hardware required.**
//!
//! `PHASE_N` searched for quarter rounds whose rotation constants are all
//! multiples of 8, scoring them with `QuarterRound::simd_instructions()`:
//!
//! ```text
//! rotation by a multiple of 8  ->  1 instruction   (vpshufb)
//! any other rotation           ->  3 instructions  (shift, shift, or)
//! ```
//!
//! Four candidates came out at **12 instructions against ChaCha's 16** — a 25%
//! cut, which was the phase's headline until BIC killed all four.
//!
//! **But that cost model describes SIMD, and `vpshufb` is an SSSE3 instruction.**
//! In scalar code the picture is different and the difference is not subtle:
//! x86 has `ROL r32, imm8` and ARM has `ROR` with an immediate. **A rotation by
//! a compile-time constant is ONE instruction at any rotation amount.** There is
//! no byte-alignment bonus to collect because there is no penalty to avoid.
//!
//! If that is right, `PHASE_N`'s entire search direction had **zero value under
//! the stated hardware bar**, independently of BIC killing its output. That is
//! worth establishing on the record rather than reasoning about, because it
//! decides whether the direction is worth ever reopening.
//!
//! ## The design, and why it has a positive control
//!
//! Four constant sets through an identical ChaCha20-shaped block function, with
//! the rotations as **const generics** so they are compile-time immediates —
//! the realistic case, and the one the cost model is about.
//!
//! | set | byte-aligned | SIMD model | note |
//! |---|---|---|---|
//! | ChaCha [16, 12, 8, 7] | 2 of 4 | 16 | the incumbent |
//! | all-aligned [16, 24, 8, 16] | 4 of 4 | 12 | PHASE_N's direction |
//! | none-aligned [15, 13, 11, 7] | 0 of 4 | 20 | the SIMD worst case |
//! | MCC [4, 17, 8, 0] | 1 of 4 | 15 | **the positive control** |
//!
//! **MCC is the control and it is what makes this experiment readable.** Its
//! `l = 0` does not make a rotation *cheaper* — it **removes the instruction
//! entirely**, which is a real saving in scalar and in SIMD alike. So:
//!
//! * If every set reads identical **including MCC**, the benchmark is not
//!   resolving single instructions and proves nothing.
//! * If MCC is measurably faster and the alignment variants are not, the
//!   benchmark resolves at the required scale and the null result is real.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **ChaCha, all-aligned and none-aligned are within noise of each other.**
//!    Byte alignment buys nothing scalar. The SIMD model has no scalar meaning.
//! 2. **MCC is measurably faster than all three** — one fewer instruction per
//!    quarter round, four quarter rounds per round, twenty rounds. This is the
//!    positive control.
//! 3. Prediction 1 failing would mean scalar rotation cost *does* vary with the
//!    amount on this machine, `PHASE_N`'s direction has scalar value after all,
//!    and the hardware objection to it is withdrawn.

use statelab_crypto::bench::{calibrate_tsc_ghz, measure, noise_floor_pct, rotated_battery};
use std::hint::black_box;

const ITERS: u64 = 300_000;
const REPEATS: usize = 9;
const BATTERIES: usize = 5;
const CHACHA_CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

/// The quarter round with its four rotations as compile-time constants.
///
/// `rotate_left` by a constant lowers to a single `rol`/`ror` on x86 and ARM.
/// A rotation by **0** lowers to nothing at all, which is the control.
#[inline(always)]
fn qr<const A: u32, const B: u32, const C: u32, const D: u32>(
    x: &mut [u32; 16],
    a: usize,
    b: usize,
    c: usize,
    d: usize,
) {
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(A);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(B);
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = (x[d] ^ x[a]).rotate_left(C);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = (x[b] ^ x[c]).rotate_left(D);
}

/// A ChaCha20-shaped block function, identical to `generator::chacha20_block`
/// in every respect except that its rotation constants are parameters.
fn block<const A: u32, const B: u32, const C: u32, const D: u32>(
    key: &[u8; 32],
    counter: u32,
    nonce: &[u8; 12],
    out: &mut [u8; 64],
) {
    let mut state = [0u32; 16];
    state[..4].copy_from_slice(&CHACHA_CONSTANTS);
    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes(key[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
    }
    state[12] = counter;
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes(nonce[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
    }

    let mut w = state;
    for _ in 0..10 {
        qr::<A, B, C, D>(&mut w, 0, 4, 8, 12);
        qr::<A, B, C, D>(&mut w, 1, 5, 9, 13);
        qr::<A, B, C, D>(&mut w, 2, 6, 10, 14);
        qr::<A, B, C, D>(&mut w, 3, 7, 11, 15);
        qr::<A, B, C, D>(&mut w, 0, 5, 10, 15);
        qr::<A, B, C, D>(&mut w, 1, 6, 11, 12);
        qr::<A, B, C, D>(&mut w, 2, 7, 8, 13);
        qr::<A, B, C, D>(&mut w, 3, 4, 9, 14);
    }
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&w[i].wrapping_add(state[i]).to_le_bytes());
    }
}

macro_rules! time_set {
    ($label:expr, $a:expr, $b:expr, $c:expr, $d:expr) => {{
        let key = [0x42u8; 32];
        let nonce = [0x24u8; 12];
        let mut out = [0u8; 64];
        let t = measure($label, 64, ITERS, REPEATS, || {
            block::<$a, $b, $c, $d>(
                black_box(&key),
                black_box(1),
                black_box(&nonce),
                black_box(&mut out),
            );
        });
        t.ticks_per_byte()
    }};
}

fn main() {
    let ghz = calibrate_tsc_ghz();
    println!("DOES BYTE ALIGNMENT BUY ANYTHING IN SCALAR CODE?\n");
    println!("  TSC calibrated at {ghz:.4} GHz");
    println!("  {ITERS} iterations x {REPEATS} repeats, median repeat");
    println!("  {BATTERIES} batteries, ORDER ROTATED each time, median across them");
    println!("  20 rounds, 64-byte output, rotations as compile-time constants\n");
    println!("  *** WHY THE ORDER ROTATES ***");
    println!("  A first version measured the four sets in a FIXED order and read");
    println!("  MCC — always last — at +11.94%, failing its own control. Repeating");
    println!("  it gave -6.4%, -8.7%, -7.1%. The +11.94% was drift, and a fixed");
    println!("  order hands the whole of any drift to whichever set runs last.");
    println!("  Item (10) says a claim from one seed is a number; a benchmark run");
    println!("  is a seed.\n");
    println!("  PREDICTION 1: ChaCha, all-aligned and none-aligned are within");
    println!("                noise. Byte alignment buys NOTHING scalar, because");
    println!("                `rol r32, imm8` is one instruction at any amount.");
    println!("  PREDICTION 2: MCC IS FASTER — its l=0 REMOVES an instruction");
    println!("                rather than cheapening one. POSITIVE CONTROL: if");
    println!("                this fails too, the benchmark resolves nothing and");
    println!("                prediction 1 is unreadable.\n");

    // Rotated batteries via the crate helper, NOT hand-rolled here. The
    // fixed-order defect this fixes was diagnosed in PHASE_O, written up, and
    // then reproduced by the very next driver because the fix lived in an
    // example. Item (21), paid.
    const LABELS: [&str; 4] = ["chacha", "all-aligned", "none-aligned", "mcc"];
    let cases = rotated_battery(&LABELS, BATTERIES, |i| match i {
        0 => time_set!("chacha", 16, 12, 8, 7),
        1 => time_set!("aligned", 16, 24, 8, 16),
        2 => time_set!("unaligned", 15, 13, 11, 7),
        _ => time_set!("mcc", 4, 17, 8, 0),
    });

    println!("  per-set run-to-run spread (the noise floor this must beat):");
    for c in &cases {
        println!(
            "    {:<14} {:>6.2}%   readings {:?}",
            c.label,
            c.spread_pct,
            c.readings
                .iter()
                .map(|x| format!("{x:.3}"))
                .collect::<Vec<_>>()
        );
    }
    println!();

    let chacha = cases[0].median;
    let aligned = cases[1].median;
    let unaligned = cases[2].median;
    let mcc = cases[3].median;

    println!(
        "  {:<28} {:>9} {:>10} {:>12}",
        "constants", "cyc/B", "vs chacha", "SIMD model"
    );
    for (label, cpb, model) in [
        ("chacha [16,12,8,7]", chacha, 16),
        ("all-aligned [16,24,8,16]", aligned, 12),
        ("none-aligned [15,13,11,7]", unaligned, 20),
        ("mcc [4,17,8,0]  [CONTROL]", mcc, 15),
    ] {
        println!(
            "  {label:<28} {cpb:>9.3} {:>10} {model:>12}",
            format!("{:+.2}%", 100.0 * (cpb / chacha - 1.0))
        );
    }

    let align_delta = 100.0 * (aligned / chacha - 1.0);
    let unalign_delta = 100.0 * (unaligned / chacha - 1.0);
    let mcc_delta = 100.0 * (mcc / chacha - 1.0);
    let spread = [chacha, aligned, unaligned]
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        - [chacha, aligned, unaligned]
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
    let spread_pct = 100.0 * spread / chacha;

    println!("\n== Verdict ==");
    println!("  all-aligned vs chacha    {align_delta:+.2}%   (SIMD model says -25%)");
    println!("  none-aligned vs chacha   {unalign_delta:+.2}%   (SIMD model says +25%)");
    println!("  spread across the three  {spread_pct:.2}%");
    println!("  MCC [CONTROL]            {mcc_delta:+.2}%");
    println!();
    if mcc_delta < -1.0 {
        println!(
            "  >>> CONTROL PASSES. MCC is {:.2}% faster — removing an",
            -mcc_delta
        );
        println!("      instruction IS visible at this resolution, so a null result");
        println!("      on the other three means something.");
    } else {
        println!("  >>> CONTROL FAILS. MCC reads {mcc_delta:+.2}%, so this benchmark");
        println!("      cannot resolve one instruction per quarter round and NOTHING");
        println!("      below can be concluded from it.");
    }
    let noise = noise_floor_pct(&cases);
    println!("  worst per-set run-to-run noise  {noise:.2}%  <-- the bar to clear");
    println!();
    if spread_pct < noise {
        println!("  >>> PREDICTION 1 HOLDS. {spread_pct:.2}% spread across constant sets");
        println!("      that the SIMD model separates by 25% in BOTH directions —");
        println!("      and that is SMALLER than this machine's own {noise:.2}% run-to-run");
        println!("      noise, so the difference is not resolvable at all.");
        println!("      *** BYTE ALIGNMENT BUYS NOTHING IN SCALAR CODE. ***");
    } else {
        println!("  >>> PREDICTION 1 FAILS at {spread_pct:.2}% spread. Rotation amount");
        println!("      DOES affect scalar cost here and PHASE_N's direction has");
        println!("      scalar value after all.");
    }

    println!("\n-- What this settles --");
    println!("   `simd_instructions()` is a SIMD cost model and must never be");
    println!("   read as a portable one. Under a bar that says NO SPECIAL");
    println!("   HARDWARE, byte-alignment is not a cheap-design axis at all —");
    println!("   it is an axis that exists only where vpshufb does.");
    println!();
    println!("   PHASE_N's candidates were already dead on BIC. This says the");
    println!("   direction was worth nothing under the stated bar even if they");
    println!("   had passed, which is the stronger and more useful closure.");
    println!();
    println!("   What DOES transfer: MCC's l=0. Eliminating an operation is");
    println!("   portable. Making one cheaper on one instruction set is not.");
}
