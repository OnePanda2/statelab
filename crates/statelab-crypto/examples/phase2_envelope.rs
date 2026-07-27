//! Phase 2 — performance envelope and the H1-narrow measurement.
//!
//! Run:  cargo run -p statelab-crypto --release --example phase2_envelope
//!
//! Reports what this machine can measure, and states plainly what it cannot.
//! A benchmark table without its hardware context invites exactly the
//! misreading that would make the numbers worse than useless.

use statelab_crypto::bench::{measure, CpuFeatures};
use statelab_crypto::generator::{FastKeyErasureRng, ForwardSecureRng, NaiveRekeyRng};
use statelab_crypto::systems::{ChaCha, KlimovShamir};
use statelab_crypto::Permutation;
use std::hint::black_box;

/// Nominal clock, used only to cross-check the TSC against wall time.
const NOMINAL_GHZ: f64 = 2.667;

fn main() {
    let cpu = CpuFeatures::detect();

    println!("=== StateLab Phase 2 — performance envelope ===\n");
    println!("-- Measurement environment --");
    println!("   features: {}", cpu.summary());
    println!(
        "   AES-based designs measurable here (AEGIS/Rocca/Randen): {}",
        yes_no(cpu.can_measure_aes_designs())
    );
    println!(
        "   H2 measurable here (GFNI + AVX-512):                    {}",
        yes_no(cpu.can_measure_h2())
    );

    if !cpu.can_measure_aes_designs() || !cpu.can_measure_h2() {
        println!("\n   *** PARTIAL ENVIRONMENT ***");
        if !cpu.can_measure_aes_designs() {
            println!("   No AES-NI. Every AES-round-based incumbent is unmeasurable here,");
            println!("   and any ChaCha-versus-AES comparison from this machine would");
            println!("   flatter ChaCha for a reason that has nothing to do with design.");
        }
        if !cpu.can_measure_h2() {
            println!("   No GFNI/AVX-512. H2 cannot be tested. Gate 2 MUST NOT fire on");
            println!("   this hardware: absence of a measurement is not a negative result.");
        }
    }

    // ---- Envelope: cost per round of the permutations we do have -----------
    println!("\n-- Envelope: cost per round (portable, no hardware crypto) --");
    println!(
        "   {:<26} {:>12} {:>12} {:>12}",
        "permutation", "ticks/byte", "ns/byte", "cyc/byte*"
    );

    let state_bytes = ChaCha.state_bytes();
    let mut state = vec![0u8; state_bytes];
    let t = measure("chacha-1round", state_bytes, 200_000, 9, || {
        ChaCha.round(black_box(&mut state), 0);
    });
    println!(
        "   {:<26} {:>12.3} {:>12.3} {:>12.3}",
        "chacha (1 round)",
        t.ticks_per_byte(),
        t.ns_per_byte(),
        t.cycles_per_byte_from_wall(NOMINAL_GHZ)
    );

    let t20 = measure("chacha-20round", state_bytes, 20_000, 9, || {
        ChaCha.permute(black_box(&mut state), 20);
    });
    println!(
        "   {:<26} {:>12.3} {:>12.3} {:>12.3}",
        "chacha (20 rounds)",
        t20.ticks_per_byte(),
        t20.ns_per_byte(),
        t20.cycles_per_byte_from_wall(NOMINAL_GHZ)
    );

    let ks = KlimovShamir::default();
    let mut ks_state = vec![0u8; ks.state_bytes()];
    let tks = measure("ks-1round", ks.state_bytes(), 200_000, 9, || {
        ks.round(black_box(&mut ks_state), 0);
    });
    println!(
        "   {:<26} {:>12.3} {:>12.3} {:>12.3}",
        "klimov-shamir (1 round)",
        tks.ticks_per_byte(),
        tks.ns_per_byte(),
        tks.cycles_per_byte_from_wall(NOMINAL_GHZ)
    );
    println!("   * cycles/byte derived from wall time at {NOMINAL_GHZ} GHz nominal.");

    // ---- Why cost per round is the wrong metric on its own -----------------
    println!("\n-- Total cost = cycles/round x rounds-to-security --");
    println!("   Cost per round alone ranks designs backwards. Rounds-to-avalanche");
    println!("   is taken from the Phase 1 run at the same seed.\n");
    println!(
        "   {:<26} {:>12} {:>16} {:>14}",
        "permutation", "cyc/byte/rd", "rounds needed", "total cyc/byte"
    );

    let chacha_per_round = t.cycles_per_byte_from_wall(NOMINAL_GHZ);
    println!(
        "   {:<26} {:>12.3} {:>16} {:>14.2}",
        "chacha",
        chacha_per_round,
        "4 (ships 20)",
        chacha_per_round * 20.0
    );
    let ks_per_round = tks.cycles_per_byte_from_wall(NOMINAL_GHZ);
    println!(
        "   {:<26} {:>12.3} {:>16} {:>14}",
        "klimov-shamir", ks_per_round, ">16 (unreached)", "unbounded"
    );
    println!(
        "\n   Klimov-Shamir is {:.1}x cheaper per round than ChaCha and strictly",
        chacha_per_round / ks_per_round
    );
    println!("   worse overall, because it never reaches the security target at all.");

    // ---- H1-narrow: what did fast key erasure already recover? -------------
    println!("\n-- H1-narrow: cost of a forward-secure 32-byte request --");
    println!("   The claim H1 rested on was that forward secrecy costs an extra");
    println!("   step per request. Measured against the construction that removed");
    println!("   that cost in 2017 and shipped in Linux in 2022.\n");

    println!(
        "   {:<42} {:>10} {:>12} {:>14}",
        "construction", "blocks/req", "ns/request", "ticks/request"
    );

    const REQ: usize = 32;
    let mut buf = [0u8; REQ];

    let mut naive = NaiveRekeyRng::new([0x42; 32]);
    let tn = measure("naive", 0, 20_000, 9, || {
        naive.fill(black_box(&mut buf));
    });
    let naive_blocks = blocks_per_request(&mut NaiveRekeyRng::new([0x42; 32]), REQ, 1000);
    println!(
        "   {:<42} {:>10.3} {:>12.1} {:>14.1}",
        NaiveRekeyRng::new([0; 32]).name(),
        naive_blocks,
        tn.ns_per_iter,
        tn.ticks_per_iter
    );

    for buffer_bytes in [256usize, 1024, 4096] {
        let mut fke = FastKeyErasureRng::new([0x42; 32], buffer_bytes);
        let tf = measure("fke", 0, 20_000, 9, || {
            fke.fill(black_box(&mut buf));
        });
        let blocks = blocks_per_request(
            &mut FastKeyErasureRng::new([0x42; 32], buffer_bytes),
            REQ,
            1000,
        );
        println!(
            "   {:<42} {:>10.3} {:>12.1} {:>14.1}   ({:.2}x faster)",
            format!("fast-key-erasure, {buffer_bytes}B buffer"),
            blocks,
            tf.ns_per_iter,
            tf.ticks_per_iter,
            tn.ns_per_iter / tf.ns_per_iter
        );
    }

    println!("\n   Interpretation: the ratio above is the slack H1 hoped to exploit,");
    println!("   already taken by an existing, deployed construction.");
}

/// Averages block invocations per request over `n` requests.
fn blocks_per_request<G: ForwardSecureRng>(g: &mut G, request: usize, n: u64) -> f64 {
    let mut buf = vec![0u8; request];
    for _ in 0..n {
        g.fill(&mut buf);
    }
    g.blocks_used() as f64 / n as f64
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "NO"
    }
}
