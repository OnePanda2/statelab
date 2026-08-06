//! PHASE G, replication arm. Standing practice: a surprising reading gets a
//! control and an independent seed set BEFORE writeup, not after.
//!
//! Two readings from `phase_g_ascon_rounds` need it:
//!   * ascon @ 4 rounds, z = −5.93. Large, and it reproduces PHASE_F §6's
//!     −5.71 — but §6's value came from the run whose seed handling produced
//!     the false +3.4σ anomaly, so "it agrees with §6" is not by itself
//!     reassurance.
//!   * chacha @ 8 rounds, z = −2.62. chacha is clean at 4, 6 and 12. A single
//!     excursion at 8 with 30 conditions in the sweep is what multiple
//!     comparisons look like. If it is noise it should not survive a fresh
//!     seed base; if it survives, it is something else and must be chased.
//!
//! Each condition below uses a seed base disjoint from every base used in the
//! main driver (which used 1 .. 290_001) and from each other.

use statelab_crypto::linearity::{rank_trials, InputSet, RankTrialSummary};
use statelab_crypto::permutation_by_name;

const TRIALS: usize = 100;

fn line(label: &str, base: u64, s: &RankTrialSummary) {
    println!(
        "  {:<22} r{:<3} base {:>9}  full {:>3}/{:<3}  z {:>+7.2}",
        label,
        s.rounds,
        base,
        s.full_rank,
        s.trials,
        s.z_score()
    );
}

fn main() {
    println!("PHASE G — replication on fresh, disjoint seed bases\n");

    // Bases chosen well clear of the main driver's 1 .. 290_001 range.
    let cases: [(&str, usize); 6] = [
        ("ascon", 4),
        ("ascon", 4),
        ("ascon", 6),
        ("chacha", 8),
        ("chacha", 8),
        ("chacha", 8),
    ];

    for (i, (name, rounds)) in cases.iter().enumerate() {
        let base = 1_000_001 + i as u64 * 50_000;
        let p = permutation_by_name(name).expect("registered");
        let n_bits = p.state_bytes() * 8;
        let s = rank_trials(
            p.as_ref(),
            *rounds,
            &InputSet::Stride(1),
            n_bits,
            TRIALS,
            base,
        );
        line(name, base, &s);
    }

    println!("\n  Three independent chacha r8 arms. If the main driver's −2.62");
    println!("  was a multiple-comparisons excursion these should scatter about");
    println!("  zero. If it is real they should all sit low.");
}
