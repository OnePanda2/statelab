//! BIC — the Bit Independence Criterion, proposal §6.3 D2.
//!
//! `avalanche.rs`'s module doc claimed BIC from the beginning and never
//! implemented it. This driver is the first measurement of it in this project.
//!
//! ## What BIC adds over SAC, and where it adds nothing
//!
//! SAC asks whether each output bit flips with probability 1/2. BIC asks whether
//! those flips are independent of each other. **Below the round count where SAC
//! saturates, BIC fails trivially and tells you nothing new** — a permutation
//! that has not diffused yet has correlated output bits for the same reason it
//! has biased ones. The region that matters is round counts where SAC is
//! ALREADY CLEAN. For ChaCha that is 4 and up.
//!
//! So the honest question this driver answers is narrow: *once the dependency
//! matrix is saturated, is there pairwise structure left that the dependency
//! matrix cannot see?*
//!
//! ## Discipline applied here
//!
//! * **Null validation at every width used** (320 and 512), not at one width —
//!   the correction Phase G had to make when a null validated at 512 was used
//!   to license readings taken at 320.
//! * **Correlation-specific null.** `bic_noise_floor` inverts a correlation's
//!   `1/sqrt(N)` standard error, NOT a proportion's `0.5/sqrt(N)`. Reusing the
//!   SAC inversion would under-sample by 4x — methodological item (1) in new
//!   clothes.
//! * **Adequacy is printed for every row.** A BIC number without one is suspect.
//! * **Multi-seed by default** — item (10).
//! * **Disjoint seed bases per condition** — item (11).
//! * **Coverage is printed beside every headline.** A max of 0.0 taken over a
//!   matrix that never flipped is not independence, it is no diffusion.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example bic_report
//! ```
//! Takes several minutes: the pair scan is `bits * C(bits,2) * samples/64` word
//! operations and 512 bits is ~40 s per condition. Timing is printed per row so
//! the cost is visible rather than surprising.

use statelab_crypto::bic::{
    bic_cells, bic_matrix, bic_noise_floor, bic_recommended_samples, random_bits_bic, BicResult,
};
use statelab_crypto::permutation_by_name;
use std::time::Instant;

const TOLERANCE: f64 = 0.12;

fn header() {
    println!(
        "  {:<26} {:>3}  {:>7}  {:>8}  {:>8}  {:>8}  {:>6}  {:>6}",
        "condition", "r", "samples", "max|r|", "floor", "mean|r|", "cover", "secs"
    );
}

fn row(label: &str, r: &BicResult, secs: f64) {
    let verdict = if !r.sampling_is_adequate(TOLERANCE) {
        "  INADEQUATE SAMPLING"
    } else if r.coverage() <= 0.99 {
        "  LOW COVERAGE — not an independence result"
    } else if r.max_abs_correlation <= r.noise_floor() {
        ""
    } else {
        "  <-- above floor"
    };
    println!(
        "  {:<26} {:>3}  {:>7}  {:>8.4}  {:>8.4}  {:>8.4}  {:>6.3}  {:>6.1}{}",
        label,
        r.rounds,
        r.samples,
        r.max_abs_correlation,
        r.noise_floor(),
        r.mean_abs_correlation,
        r.coverage(),
        secs,
        verdict
    );
}

fn measure(name: &str, rounds: usize, samples: usize, seed: u64) -> (BicResult, f64) {
    let p = permutation_by_name(name).expect("registered");
    let t = Instant::now();
    let r = bic_matrix(p.as_ref(), rounds, samples, seed);
    (r, t.elapsed().as_secs_f64())
}

fn main() {
    println!("BIC — Bit Independence Criterion (Webster & Tavares), first measurement");
    println!("Correlation coefficient with a correlation-specific null. NOT a proportion.\n");

    println!("-- Scale of the problem --");
    println!(
        "  {:<10} {:>5} {:>14} {:>10} {:>12}",
        "design", "bits", "cells", "samples", "SAC samples"
    );
    for (name, bits) in [("ascon", 320usize), ("chacha", 512), ("chacha64", 1024)] {
        println!(
            "  {:<10} {:>5} {:>14} {:>10} {:>12}",
            name,
            bits,
            bic_cells(bits),
            bic_recommended_samples(bits, TOLERANCE),
            statelab_crypto::avalanche::recommended_samples(bits, TOLERANCE)
        );
    }
    println!();
    println!("  Cells are bits*C(bits,2), not bits*bits: for 512 bits that is 66,977,792");
    println!("  against SAC's 262,144. The sample counts differ by more than the cell");
    println!("  count alone explains, because a correlation's standard error is");
    println!("  1/sqrt(N) where a proportion's is 0.5/sqrt(N) — a factor of four in N.");
    println!();
    println!("  *** chacha64 (1024 bits) is NOT measured below. *** Its pair scan is");
    println!("  eight times chacha's, roughly five minutes per condition, and it is");
    println!("  omitted for cost rather than for any methodological reason. Stated so");
    println!("  the omission is visible; add it if the cost is acceptable.\n");

    // ------------------------------------------------------------------ null
    println!("-- 0. NULL VALIDATION, at every width measured below --");
    println!("   Independent fair coins through the identical pair machinery. The");
    println!("   maximum must land near the predicted floor. Without this the");
    println!("   battery cannot tell 'independent' from 'the null model is wrong'.\n");
    header();
    for bits in [320usize, 512] {
        let samples = bic_recommended_samples(bits, TOLERANCE);
        let t = Instant::now();
        let r = random_bits_bic(bits, samples, 0xC0FF_EE00 + bits as u64);
        row(
            &format!("random-bits n={bits}"),
            &r,
            t.elapsed().as_secs_f64(),
        );
        let ratio = r.max_abs_correlation / bic_noise_floor(samples, bic_cells(bits));
        println!("      realised/predicted = {ratio:.3}  (1.0 is the null behaving)");
    }
    println!();
    println!("   *** THIS NULL IS NOT THE PASS THRESHOLD. *** It reads ~0.063-0.067");
    println!("   where every real design below reads ~0.076-0.083. The hypothesis");
    println!("   that the control's own construction caused this was tested and");
    println!("   ELIMINATED (bic_null_diagnostic: a byte-filled null reads the same).");
    println!("   Fair coins are simply the wrong null object for a bijection, whose");
    println!("   avalanche vector is never zero. Thresholding on this empirical null");
    println!("   would put EVERY design 'above chance' and manufacture a finding out");
    println!("   of the difference between a permutation and a coin. The analytic");
    println!("   floor is the threshold, and it is conservative in the safe direction.");
    println!("   The size of the gap is recorded as OPEN and unexplained.\n");

    // -------------------------------------------------------------- chacha
    println!("-- A. CHACHA ROUND SWEEP (512 bits) --");
    println!("   SAC saturates at 4 rounds. Rounds 2 and 3 are shown for shape and");
    println!("   are NOT independent evidence of anything: an undiffused permutation");
    println!("   fails BIC for the same reason it fails SAC. Read rounds 4+.\n");
    header();
    let samples_512 = bic_recommended_samples(512, TOLERANCE);
    for (idx, rounds) in [2usize, 3, 4, 6, 8].iter().enumerate() {
        let seed = 1 + idx as u64 * 100_000; // disjoint per condition — item (11)
        let (r, secs) = measure("chacha", *rounds, samples_512, seed);
        row("chacha", &r, secs);
    }
    println!();

    // --------------------------------------------------------------- ascon
    println!("-- B. ASCON ROUND SWEEP (320 bits) --");
    println!("   Included because Phase G found a GF(2) rank deficiency in ascon at");
    println!("   3-4 rounds, resolving by 6. BIC is an independently coded statistic");
    println!("   on a different property, so agreement would be informative and");
    println!("   disagreement equally so. Ascon is specified at 12 rounds.\n");
    header();
    let samples_320 = bic_recommended_samples(320, TOLERANCE);
    for (idx, rounds) in [2usize, 3, 4, 6, 12].iter().enumerate() {
        let seed = 900_001 + idx as u64 * 100_000;
        let (r, secs) = measure("ascon", *rounds, samples_320, seed);
        row("ascon", &r, secs);
    }
    println!();

    // ----------------------------------------------------------- multi-seed
    println!("-- C. MULTI-SEED ON THE SATURATED ROUND COUNTS — item (10) --");
    println!("   Single seed is a default violation, not a hardening step. Three");
    println!("   disjoint bases at the first round count where SAC is clean.\n");
    header();
    for (idx, seed) in [2_000_001u64, 3_000_001, 4_000_001].iter().enumerate() {
        let (r, secs) = measure("chacha", 4, samples_512, *seed);
        row(&format!("chacha seed base {}", idx + 1), &r, secs);
    }
    for (idx, seed) in [5_000_001u64, 6_000_001, 7_000_001].iter().enumerate() {
        let (r, secs) = measure("ascon", 4, samples_320, *seed);
        row(&format!("ascon seed base {}", idx + 1), &r, secs);
    }

    println!("\n-- How to read this --");
    println!("   `max|r|` at or below `floor` means the worst output-bit pair in the");
    println!("   whole matrix is no more correlated than chance predicts for a");
    println!("   maximum over this many cells. That is a PASS.");
    println!();
    println!("   It is a statement about diffusion, not about security. Phase 7");
    println!("   (differential/linear bounds) is untouched and nothing here bears");
    println!("   on it. And this is ONE route: a BIC result that disagreed with the");
    println!("   rest of the pipeline would be a question, not a finding.");
}
