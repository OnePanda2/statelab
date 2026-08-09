//! WHAT DOES CHACHA20 ACTUALLY COST? — auditing the number the goal is measured against.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example harness_tax
//! ```
//!
//! ## Why this exists
//!
//! The project's goal is a PRNG that **outperforms ChaCha20 on ordinary
//! hardware**. Every speed claim it has made is denominated in the
//! `cycles/byte/round` figures from `BASELINE_TABLE.md`, where `chacha` reads
//! **1.019 cyc/B/round**. Twenty rounds of that is ~20.4 cyc/B.
//!
//! **A good scalar ChaCha20 on this class of machine is several times faster
//! than that.** So either the figure is right and ChaCha20 is slower than the
//! literature says, or **the number the goal is measured against is not
//! ChaCha20's cost — it is ChaCha20's cost plus the instrument's overhead.**
//!
//! The suspicion is structural, not vague. `Permutation::round` takes
//! `&mut [u8]`, so `systems::ChaCha::round` **loads all sixteen words from
//! bytes and stores them back on every single round**. `Permutation::permute`
//! then calls it through `&dyn` twenty times. The real generator,
//! `generator::chacha20_block`, loads once, runs twenty rounds in registers,
//! and stores once.
//!
//! That is a difference of **forty state round-trips per block**, and it sits
//! underneath every performance number this project has produced.
//!
//! ## Why it matters beyond a wrong constant
//!
//! If the load/store tax dominates, it does not merely inflate costs — **it
//! compresses differences between designs.** Two permutations whose arithmetic
//! differs by 2x can read as nearly equal through an interface where most of
//! the time is memory traffic. `PHASE_N` selected candidates on an instruction
//! count and `PHASE_M` searched wiring for round reductions; both assume the
//! cost model tracks reality.
//!
//! This is item (16) — everything is a proxy until it is checked — pointed at
//! the project's own speed instrument rather than at a design.
//!
//! ## *** PREDICTIONS, RECORDED BEFORE RUNNING ***
//!
//! 1. **The real block function is at least 2x faster per byte than the same
//!    twenty rounds through the trait.** If it is not, the harness is fine and
//!    this whole concern is withdrawn.
//! 2. **A load/store-only "round" — no arithmetic at all — costs a large
//!    fraction of a real ChaCha round.** This is the POSITIVE CONTROL for the
//!    mechanism: if arithmetic-free rounds are nearly free, the gap in (1) is
//!    something else and the diagnosis is wrong.
//! 3. **`chacha20_block` lands in the range a scalar ChaCha20 should** on a
//!    2.67 GHz Nehalem-class part. If it does not, the problem is the
//!    implementation and not the harness, which is a different and worse
//!    finding.
//!
//! Outcome 1 failing is the good case: it would mean the baseline stands.

use statelab_crypto::bench::{calibrate_tsc_ghz, measure};
use statelab_crypto::generator::chacha20_block;
use statelab_crypto::permutation::Permutation;
use statelab_crypto::systems::{Ascon, ChaCha};
use std::hint::black_box;

const ITERS: u64 = 200_000;
const REPEATS: usize = 9;

/// ChaCha's exact per-round load and store, with **no arithmetic between
/// them**. The control for prediction 2: whatever this costs is what the
/// byte-slice interface charges before a design does any work at all.
struct LoadStoreOnly;

impl Permutation for LoadStoreOnly {
    fn name(&self) -> &'static str {
        "load-store-only"
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        20
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        let mut w = [0u32; 16];
        for (i, word) in w.iter_mut().enumerate() {
            *word = u32::from_le_bytes(state[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }
        // Nothing happens here. That is the point.
        let w = black_box(w);
        for (i, word) in w.iter().enumerate() {
            state[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
    }
}

fn row(label: &str, cyc_per_byte: f64, reference: f64) {
    println!(
        "  {label:<38} {cyc_per_byte:>9.3} {:>11}",
        if reference > 0.0 {
            format!("{:.2}x", cyc_per_byte / reference)
        } else {
            "-".to_string()
        }
    );
}

fn main() {
    let ghz = calibrate_tsc_ghz();
    println!("WHAT DOES CHACHA20 ACTUALLY COST?\n");
    println!("  TSC calibrated at {ghz:.4} GHz");
    println!("  {ITERS} iterations x {REPEATS} repeats, median repeat");
    println!("  All figures are cycles/byte at the calibrated TSC rate.\n");
    println!("  BASELINE_TABLE.md records chacha at 1.019 cyc/B/round, i.e.");
    println!("  ~20.4 cyc/B for the 20 rounds ChaCha20 actually ships.\n");

    // ---------------------------------------------------------- the real thing
    let key = [0x42u8; 32];
    let nonce = [0x24u8; 12];
    let mut out = [0u8; 64];
    let real = measure("chacha20_block", 64, ITERS, REPEATS, || {
        chacha20_block(
            black_box(&key),
            black_box(1),
            black_box(&nonce),
            black_box(&mut out),
        );
    });
    let real_cpb = real.ticks_per_byte();

    // ------------------------------------------------- the same work, via trait
    let chacha = ChaCha;
    let mut state = [0u8; 64];
    let trait20 = measure("ChaCha::permute(20) via trait", 64, ITERS, REPEATS, || {
        chacha.permute(black_box(&mut state), 20);
    });

    // ------------------------------------------------------------- the control
    let nul = LoadStoreOnly;
    let mut state2 = [0u8; 64];
    let tax = measure("LoadStoreOnly::permute(20)", 64, ITERS, REPEATS, || {
        nul.permute(black_box(&mut state2), 20);
    });

    // ---------------------------------------- a second design through the same
    let ascon = Ascon;
    let ascon_bytes = ascon.state_bytes();
    let mut state3 = vec![0u8; ascon_bytes];
    let ascon20 = measure(
        "Ascon::permute(20) via trait",
        ascon_bytes,
        ITERS,
        REPEATS,
        || {
            ascon.permute(black_box(&mut state3), 20);
        },
    );

    println!("== 20 rounds, 64-byte state ==");
    println!("  {:<38} {:>9} {:>11}", "path", "cyc/B", "vs real");
    row("chacha20_block (real generator)", real_cpb, 0.0);
    row(
        "ChaCha::permute(20) via trait",
        trait20.ticks_per_byte(),
        real_cpb,
    );
    row(
        "LoadStoreOnly::permute(20)  [CONTROL]",
        tax.ticks_per_byte(),
        real_cpb,
    );
    row(
        "Ascon::permute(20) via trait",
        ascon20.ticks_per_byte(),
        real_cpb,
    );

    let trait_cpb = trait20.ticks_per_byte();
    let tax_cpb = tax.ticks_per_byte();
    let ratio = trait_cpb / real_cpb;
    let tax_share = 100.0 * tax_cpb / trait_cpb;

    println!("\n== Verdict ==");
    println!("  real ChaCha20 block          {real_cpb:.3} cyc/B");
    println!("  same 20 rounds via the trait {trait_cpb:.3} cyc/B   ({ratio:.2}x)");
    println!("  of which pure load/store     {tax_cpb:.3} cyc/B   ({tax_share:.1}% of it)");
    println!();
    if ratio >= 2.0 {
        println!("  >>> PREDICTION 1 HOLDS. The trait path costs {ratio:.2}x the real");
        println!("      block function. BASELINE_TABLE.md's cyc/B figures are");
        println!("      instrument cost, NOT design cost.");
    } else {
        println!("  >>> PREDICTION 1 FAILS at {ratio:.2}x. The baseline stands and");
        println!("      this concern is withdrawn.");
    }
    if tax_share >= 25.0 {
        println!("  >>> PREDICTION 2 HOLDS. {tax_share:.1}% of the trait path is memory");
        println!("      traffic with no arithmetic in it. The mechanism is confirmed:");
        println!("      the byte-slice interface, not the design.");
    } else {
        println!("  >>> PREDICTION 2 FAILS at {tax_share:.1}%. The gap is not load/store");
        println!("      and the diagnosis is wrong.");
    }

    println!("\n-- What this does and does not say --");
    println!("   It does NOT say the diffusion results are wrong. Rounds-to-");
    println!("   avalanche is a property of the permutation and is unaffected by");
    println!("   how many times the state is copied to measure it.");
    println!();
    println!("   It DOES say that any comparison of DESIGNS on speed, made");
    println!("   through this interface, is measuring a quantity in which a");
    println!("   fixed per-round overhead is mixed with the design's own work —");
    println!("   which flatters slow designs and hides real differences.");
    println!();
    println!("   And it says the number to beat is the FIRST row, not the second.");
}
