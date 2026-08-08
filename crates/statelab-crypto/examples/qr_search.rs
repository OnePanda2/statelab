//! QUARTER-ROUND SEARCH — two arms, and a different definition of winning.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example qr_search -- [n_free] [n_aligned] [seed]
//! ```
//!
//! ## Why the target changed
//!
//! Phase M ran three searches at ChaCha's **round count** and lost all three:
//! random (1049 wirings), directed on `max_dev` (300 evals), directed on
//! offender count (600 evals). Every one produced a wiring better than ChaCha
//! at 3 rounds and identical at 4.
//!
//! So this asks a different question. **"Better than ChaCha20" can mean fewer
//! INSTRUCTIONS at the same round count**, which is the same speedup and has
//! never been attempted here.
//!
//! On SIMD a rotation by a multiple of 8 is one `vpshufb`; anything else is
//! shift + shift + or. ChaCha's 16, 12, 8, 7 is two cheap and two expensive:
//! **16 instructions per quarter round.** An all-byte-aligned quarter round is
//! **12** — 25% cheaper.
//!
//! **THE WIN CONDITION: a byte-aligned quarter round that still reaches full
//! avalanche at 4 rounds.** Same diffusion, same round count, 25% fewer
//! instructions. No need to beat 4 rounds at all.
//!
//! ## Two arms, because the comparison is the result either way
//!
//! * **free** — rotations 1..=31, the unconstrained control arm.
//! * **byte-aligned** — rotations in {8,16,24} only.
//!
//! If the aligned arm matches the free arm's hit rate, byte alignment is free
//! and ChaCha's 12 and 7 are leaving performance on the table. If it is much
//! worse, alignment costs diffusion and that is a real constraint worth
//! recording — it would also retro-justify ChaCha's choice.
//!
//! ## Discipline
//!
//! Operation count is identical to ChaCha's for every candidate by construction
//! (4 add, 4 xor, 4 rot) — items (7) and (8). Ranking uses **offender count**,
//! not `max_dev`, per item (17): a maximum over 262,144 cells is extreme-value
//! noisy where a count pools evidence, and its zero is exactly
//! `is_full_avalanche`. Degenerate steps (`x += x`, `x ^= x`) are rejected
//! before measuring, not discovered as a bad score. The positive control must
//! reproduce ChaCha's 4 rounds or the run aborts.

use statelab_crypto::avalanche::{
    avalanche_matrix, recommended_samples, rounds_to_avalanche, AvalancheMatrix, Probe,
};
use statelab_crypto::quarter_round::{chacha_qr, random_qr, QrPermutation, QuarterRound};
use statelab_crypto::topology::LANES;

const TOLERANCE: f64 = 0.12;
const ROUNDS: usize = 4;
const CONFIRM_SEEDS: [u64; 5] = [101, 202, 12345, 0xDEAD_BEEF, 1 << 63];

fn offenders(m: &AvalancheMatrix) -> usize {
    m.p.iter().filter(|&&p| (p - 0.5).abs() > TOLERANCE).count()
}

/// `(full avalanche, offenders, max_dev)` at `ROUNDS`.
fn measure(qr: &QuarterRound, samples: usize, seed: u64) -> (bool, usize, f64) {
    let p = QrPermutation::with_chacha_wiring(qr.clone());
    let m = avalanche_matrix(&p, ROUNDS, samples, seed);
    (
        m.is_full_avalanche(TOLERANCE),
        offenders(&m),
        m.max_deviation(),
    )
}

fn run_arm(
    label: &str,
    n: usize,
    byte_aligned: bool,
    samples: usize,
    probe: &mut Probe,
) -> (usize, Vec<QuarterRound>, f64, usize) {
    println!("\n-- ARM: {label} ({n} candidates) --");
    let t0 = std::time::Instant::now();
    let mut hits: Vec<QuarterRound> = Vec::new();
    let mut best_off = usize::MAX;
    let mut best_dev = f64::INFINITY;
    for i in 0..n {
        let qr = random_qr(probe, byte_aligned);
        let (full, off, dev) = measure(&qr, samples, 1);
        if off < best_off {
            best_off = off;
            best_dev = dev;
        }
        if full {
            println!(
                "   *** HIT at candidate {i}: full avalanche at {ROUNDS} rounds, \
                 {} instructions ***",
                qr.simd_instructions()
            );
            hits.push(qr);
        }
        if i % 100 == 0 && i > 0 {
            println!(
                "   ...{i}/{n}  hits {}  best offenders {best_off}  [{:.0}s]",
                hits.len(),
                t0.elapsed().as_secs_f64()
            );
        }
    }
    println!(
        "   {label}: {} hit(s) in {n}, best offenders {best_off}, best max_dev {best_dev:.4}, {:.0}s",
        hits.len(),
        t0.elapsed().as_secs_f64()
    );
    (hits.len(), hits, best_dev, best_off)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n_free: usize = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(250);
    let n_aligned: usize = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(350);
    let seed: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let samples = recommended_samples(LANES * 32, TOLERANCE);

    println!("QUARTER-ROUND SEARCH — can a CHEAPER quarter round match ChaCha?\n");
    println!("  wiring        ChaCha's, FIXED (this is the mirror of Phase M)");
    println!("  quarter round 4 steps of `w[i] += w[j]; w[k] ^= w[i]; w[k] <<<= r`");
    println!("  op count      4 add + 4 xor + 4 rot for EVERY candidate — identical");
    println!("  screen        full avalanche at {ROUNDS} rounds (ChaCha's own count)");
    println!("  ranked on     offender count, not max_dev — item (17)");
    println!("  samples       {samples}\n");
    println!("  WIN = a BYTE-ALIGNED quarter round reaching full avalanche at {ROUNDS}");
    println!("        rounds: 12 instructions against ChaCha's 16, same diffusion.\n");

    // ------------------------------------------------------------- control
    let cc = chacha_qr();
    let (cc_full, cc_off, cc_dev) = measure(&cc, samples, 1);
    let sweep = rounds_to_avalanche(
        &QrPermutation::with_chacha_wiring(cc.clone()),
        12,
        samples,
        TOLERANCE,
        1,
    );
    println!("-- CONTROL: ChaCha's own quarter round through this path --");
    println!(
        "   instructions {}   byte-aligned {}   rotations 16,12,8,7",
        cc.simd_instructions(),
        cc.is_byte_aligned()
    );
    println!(
        "   at {ROUNDS} rounds: full avalanche {cc_full}, offenders {cc_off}, max_dev {cc_dev:.4}"
    );
    println!("   rounds-to-avalanche {:?}", sweep.rounds_to_avalanche);
    if sweep.rounds_to_avalanche != Some(4) {
        println!("\n   *** CONTROL FAILED — harness disagrees with the registry. STOPPING.");
        return;
    }
    println!("   Control holds.\n");

    let mut probe = Probe::new(seed);
    let (free_hits, _free, free_dev, free_off) =
        run_arm("free rotations 1..31", n_free, false, samples, &mut probe);
    let (al_hits, aligned, al_dev, al_off) = run_arm(
        "byte-aligned {8,16,24}",
        n_aligned,
        true,
        samples,
        &mut probe,
    );

    // ------------------------------------------------------------- verdict
    println!("\n-- COMPARISON --");
    println!(
        "   {:<26} {:>8} {:>10} {:>12} {:>12}",
        "arm", "hits", "of", "best offend", "best max_dev"
    );
    println!(
        "   {:<26} {:>8} {:>10} {:>12} {:>12.4}",
        "free (16 instr)", free_hits, n_free, free_off, free_dev
    );
    println!(
        "   {:<26} {:>8} {:>10} {:>12} {:>12.4}",
        "byte-aligned (12 instr)", al_hits, n_aligned, al_off, al_dev
    );
    println!(
        "   {:<26} {:>8} {:>10} {:>12} {:>12.4}",
        "chacha", "-", "-", cc_off, cc_dev
    );

    if al_hits == 0 {
        println!("\n-- RESULT: NO BYTE-ALIGNED QUARTER ROUND MATCHED CHACHA --");
        println!("   {n_aligned} byte-aligned candidates, none reached full avalanche at");
        println!("   {ROUNDS} rounds. On this sample, restricting rotations to multiples");
        println!("   of 8 costs diffusion — which would RETRO-JUSTIFY ChaCha's 12 and 7");
        println!("   as buying something real, not as an oversight.");
        println!();
        println!("   Read against the free arm's {free_hits} hit(s) before concluding:");
        println!("   if the free arm also found nothing, the screen is simply hard and");
        println!("   this says nothing about alignment specifically.");
        return;
    }

    // ------------------------------------------------- confirm, items (10)(11)
    println!("\n-- *** {al_hits} BYTE-ALIGNED HIT(S). CONFIRMING — items (10), (11) *** --");
    println!("   A single-seed hit is a number, not a finding.\n");
    for (idx, qr) in aligned.iter().take(5).enumerate() {
        println!(
            "   candidate {idx}: {} instructions",
            qr.simd_instructions()
        );
        for s in &qr.steps {
            println!(
                "      w[{}] += w[{}];  w[{}] ^= w[{}];  w[{}] <<<= {}",
                s.add_to, s.add_from, s.xor_into, s.add_to, s.xor_into, s.rot
            );
        }
        let mut clean = 0usize;
        for &sd in &CONFIRM_SEEDS {
            let (full, off, dev) = measure(qr, samples, sd);
            if full {
                clean += 1;
            }
            println!("      seed {sd:>22}: full {full}, offenders {off}, max_dev {dev:.4}");
        }
        println!(
            "      *** CLEAN ON {clean}/{} SEEDS ***",
            CONFIRM_SEEDS.len()
        );
        if clean == CONFIRM_SEEDS.len() {
            let sw = rounds_to_avalanche(
                &QrPermutation::with_chacha_wiring(qr.clone()),
                12,
                samples,
                TOLERANCE,
                101,
            );
            println!(
                "      rounds-to-avalanche {:?} (chacha: Some(4))",
                sw.rounds_to_avalanche
            );
            println!("      >>> MATCHES CHACHA'S DIFFUSION AT 12 INSTRUCTIONS VS 16.");
            println!("      >>> That is 25% fewer instructions per quarter round.");
        } else {
            println!("      rejected: single-seed artefact — exactly what item (10) is for.");
        }
        println!();
    }

    println!("   A confirmed hit is a CANDIDATE FOR CLAASP, not a result. Avalanche is");
    println!("   a proxy (PHASE_L §4, item 16), and per TASK_1 the next test is the");
    println!("   low-entropy dose-response curve that killed wide-cross AFTER it");
    println!("   cleared exactly this kind of bar.");
}
