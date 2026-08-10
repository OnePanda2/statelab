//! ASCON vs CHACHA20 IN REAL GENERATOR SHAPE — avenue 1.5.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example generator_shape_compare
//! ```
//!
//! ## Why this exists
//!
//! `PHASE_O` §1 found that every cost figure this project had was inflated
//! 2.35x by the measurement trait's per-round load/store, and corrected
//! ChaCha20 to ~8.3–8.8 cyc/B. It marked `BASELINE_TABLE.md` §3.1's conclusion
//! that Ascon and ChaCha are *"effectively tied on diffusion efficiency"* as
//! **NOT SUPPORTED** — and then never re-measured Ascon outside the trait.
//!
//! So the project currently **does not know** what its only non-ARX comparator
//! costs. That matters now because avenue 2's most promising branch is
//! AND-based nonlinearity instead of pure ARX, and Ascon is the strongest
//! deployed example of it.
//!
//! Ascon's round is bit-exact against the Ascon team's reference implementation
//! (`systems.rs`, `ascon_round_matches_the_reference_implementation`), so this
//! measures Ascon rather than our transcription of it.
//!
//! ## The structural asymmetry, stated before the numbers
//!
//! **ChaCha emits its entire state.** 64 bytes out per 64-byte state, a rate of
//! 100%, because the feed-forward makes the whole block keystream.
//!
//! **A sponge cannot do that.** Ascon's 320-bit state is split into a rate and a
//! capacity, and only the rate is emitted; the capacity is what security rests
//! on. Ascon-Hash/XOF uses a 64-bit rate, Ascon-AEAD128 a 128-bit rate. So a
//! generator built on Ascon pays a full permutation per 8 or 16 bytes where
//! ChaCha pays one per 64.
//!
//! `PLAN_HARDWARE_INDEPENDENT.md` §1.2 called this out in July — *"a sponge with
//! capacity would emit LESS per permutation call. There is nothing to win
//! here."* This measures how much that costs rather than leaving it as an
//! argument.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **Per permutation call, Ascon p12 is cheaper than ChaCha20's block** —
//!    fewer rounds (12 vs 20) on a smaller state (320 vs 512 bits).
//! 2. **Per BYTE OF OUTPUT, Ascon is decisively worse**, because the rate
//!    asymmetry outweighs the cheaper permutation. At a 128-bit rate it emits a
//!    quarter of ChaCha's bytes per call.
//! 3. **The gap is large enough that AEAD overhead does not explain NIST's
//!    Table 12.** If prediction 2 lands near ChaCha's cost rather than well
//!    above it, then the sponge rate is not the dominant term and the
//!    AND-based branch of avenue 2 deserves more weight, not less.
//!
//! Prediction 2 failing is the interesting outcome and is why this is worth
//! measuring rather than reasoning about.

use statelab_crypto::bench::{calibrate_tsc_ghz, measure};
use statelab_crypto::generator::chacha20_block;
use statelab_crypto::systems::Ascon;
use std::hint::black_box;

const ITERS: u64 = 200_000;
const REPEATS: usize = 9;

/// Ascon's p12 permutation, register-resident: load once, twelve rounds in
/// registers, store once. The same shape `generator::chacha20_block` uses, so
/// the comparison is not contaminated by the harness tax `PHASE_O` found.
///
/// The round body is transcribed from the same reference the KAT validates
/// against, kept in the reference's temporary-array form.
#[inline(always)]
fn ascon_p12(s: &mut [u64; 5]) {
    for r in 0..12usize {
        let c = Ascon::round_constant(r);
        s[2] ^= c;
        s[0] ^= s[4];
        s[4] ^= s[3];
        s[2] ^= s[1];
        let mut t = [0u64; 5];
        t[0] = s[0] ^ (!s[1] & s[2]);
        t[1] = s[1] ^ (!s[2] & s[3]);
        t[2] = s[2] ^ (!s[3] & s[4]);
        t[3] = s[3] ^ (!s[4] & s[0]);
        t[4] = s[4] ^ (!s[0] & s[1]);
        t[1] ^= t[0];
        t[0] ^= t[4];
        t[3] ^= t[2];
        t[2] = !t[2];
        s[0] = t[0] ^ t[0].rotate_right(19) ^ t[0].rotate_right(28);
        s[1] = t[1] ^ t[1].rotate_right(61) ^ t[1].rotate_right(39);
        s[2] = t[2] ^ t[2].rotate_right(1) ^ t[2].rotate_right(6);
        s[3] = t[3] ^ t[3].rotate_right(10) ^ t[3].rotate_right(17);
        s[4] = t[4] ^ t[4].rotate_right(7) ^ t[4].rotate_right(41);
    }
}

fn main() {
    let ghz = calibrate_tsc_ghz();
    println!("ASCON vs CHACHA20 IN REAL GENERATOR SHAPE\n");
    println!("  TSC calibrated at {ghz:.4} GHz");
    println!("  {ITERS} iterations x {REPEATS} repeats, median repeat");
    println!("  Both register-resident — NOT through the measurement trait,");
    println!("  whose per-round load/store inflated everything 2.35x (PHASE_O).\n");
    println!("  PREDICTION 1: per CALL, Ascon p12 is cheaper than ChaCha20.");
    println!("  PREDICTION 2: per BYTE, Ascon is decisively worse — the sponge");
    println!("                rate asymmetry outweighs the cheaper permutation.");
    println!("  PREDICTION 3: the gap is too large for AEAD overhead to explain");
    println!("                NIST IR 8454 Table 12. If prediction 2 FAILS, the");
    println!("                AND-based branch of avenue 2 gains weight.\n");

    // ---- ChaCha20: 64 bytes out per call, rate 100%.
    let key = [0x42u8; 32];
    let nonce = [0x24u8; 12];
    let mut out = [0u8; 64];
    let cc = measure("chacha20_block", 64, ITERS, REPEATS, || {
        chacha20_block(
            black_box(&key),
            black_box(1),
            black_box(&nonce),
            black_box(&mut out),
        );
    });
    let cc_call = cc.ticks_per_iter;
    let cc_pb = cc.ticks_per_byte();

    // ---- Ascon p12: one call, then attributed to each candidate rate.
    let mut st = [0x0123_4567_89ab_cdefu64; 5];
    let asc = measure("ascon_p12", 1, ITERS, REPEATS, || {
        ascon_p12(black_box(&mut st));
    });
    let asc_call = asc.ticks_per_iter;

    println!("== Per permutation call ==");
    println!("  {:<28} {:>10} {:>10}", "primitive", "cycles", "state");
    println!(
        "  {:<28} {cc_call:>10.1} {:>10}",
        "ChaCha20 block (20 rounds)", "512 bit"
    );
    println!(
        "  {:<28} {asc_call:>10.1} {:>10}",
        "Ascon p12 (12 rounds)", "320 bit"
    );
    let call_ratio = cc_call / asc_call;
    println!("  Ascon's permutation is {call_ratio:.2}x cheaper per call.");

    println!("\n== Per byte of keystream ==");
    println!(
        "  {:<28} {:>8} {:>12} {:>12}",
        "generator", "bytes", "cyc/byte", "vs ChaCha20"
    );
    println!(
        "  {:<28} {:>8} {cc_pb:>12.3} {:>12}",
        "ChaCha20", 64, "1.00x"
    );
    for (label, rate) in [
        ("Ascon, 64-bit rate (Hash/XOF)", 8.0),
        ("Ascon, 128-bit rate (AEAD128)", 16.0),
    ] {
        let pb = asc_call / rate;
        println!(
            "  {label:<28} {:>8} {pb:>12.3} {:>12}",
            rate as usize,
            format!("{:.2}x", pb / cc_pb)
        );
    }

    let best_ascon = asc_call / 16.0;
    println!("\n== Verdict ==");
    if call_ratio > 1.0 {
        println!("  >>> PREDICTION 1 HOLDS. Ascon's permutation is genuinely the");
        println!("      cheaper primitive, by {call_ratio:.2}x per call.");
    } else {
        println!("  >>> PREDICTION 1 FAILS at {call_ratio:.2}x — Ascon's p12 costs MORE");
        println!("      per call than ChaCha20's block despite fewer rounds on less");
        println!("      state. That would be a finding about the S-box, not the rate.");
    }
    if best_ascon > cc_pb {
        println!("  >>> PREDICTION 2 HOLDS. Per byte, even at its BEST rate Ascon");
        println!(
            "      costs {:.2}x ChaCha20. The cheaper permutation does not",
            best_ascon / cc_pb
        );
        println!("      survive the sponge's rate: ChaCha emits its whole state,");
        println!("      a sponge emits only its rate.");
    } else {
        println!("  >>> PREDICTION 2 FAILS. Ascon is competitive per byte at");
        println!(
            "      {:.2}x. The rate asymmetry is NOT dominant, and the",
            best_ascon / cc_pb
        );
        println!("      AND-based branch of avenue 2 deserves more weight.");
    }

    println!("\n-- What this does and does not say --");
    println!("   It measures the PERMUTATION in generator shape, with no AEAD,");
    println!("   no padding and no domain separation — so it is a LOWER BOUND on");
    println!("   what an Ascon-based generator would cost, and the comparison is");
    println!("   generous to Ascon.");
    println!();
    println!("   It says nothing about security. Ascon is a NIST standard with a");
    println!("   far smaller state; ChaCha20 is not being called better, only");
    println!("   faster per byte at these parameters.");
}
