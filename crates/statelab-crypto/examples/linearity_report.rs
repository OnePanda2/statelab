//! GF(2) linear-structure battery — the instrument gap PractRand exposed.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example linearity_report -- known
//! cargo run -p statelab-crypto --release --example linearity_report -- full
//! ```
//!
//! `known` runs only the answers that are known before the program runs. `full`
//! adds the ones that are not. The split is enforced by the driver rather than
//! left to intention, because an instrument is proved before it is trusted.

use statelab_crypto::linearity::{
    across_lanes, affine_residual, full_rank_probability, lane_bits, random_matrix_rank_trials,
    rank_trials, subspace_rank, InputSet, RankTrialSummary,
};
use statelab_crypto::permutation_by_name;
use statelab_crypto::systems::{ChaCha, Xoshiro256pp};
use statelab_crypto::Permutation;

/// Multi-seed from the first run — methodological item (10).
const SEEDS: [u64; 5] = [1, 2, 3, 4, 5];
/// Trials per condition in the square regime.
const TRIALS: usize = 100;
/// Subspace dimension for the tall-thin regime.
const K: usize = 16;
/// Rows for the tall-thin regime.
const TALL: usize = 64;
const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// Seed bases, disjoint per condition — methodological item (11). Sharing a
/// seed set across the arms of a comparison correlates them and manufactures
/// significance when the arms are pooled.
fn seed_base(condition_index: usize) -> u64 {
    1 + condition_index as u64 * 10_000
}

struct Identity;
impl Permutation for Identity {
    fn name(&self) -> &'static str {
        "identity"
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        1
    }
    fn round(&self, _s: &mut [u8], _r: usize) {}
}

/// xorshift64 per lane: GF(2)-linear, invertible, rank known in advance.
struct PlantedLinear;
impl Permutation for PlantedLinear {
    fn name(&self) -> &'static str {
        "planted-linear"
    }
    fn state_bytes(&self) -> usize {
        64
    }
    fn default_rounds(&self) -> usize {
        1
    }
    fn round(&self, state: &mut [u8], _r: usize) {
        for lane in state.chunks_exact_mut(8) {
            let mut x = u64::from_le_bytes(lane.try_into().unwrap());
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            lane.copy_from_slice(&x.to_le_bytes());
        }
    }
}

fn geometries(n: usize, k: usize) -> Vec<(&'static str, Vec<Vec<u8>>)> {
    vec![
        ("counter-lane low bits", lane_bits(n, 1, 0, k)),
        ("counter-lane HIGH bits", lane_bits(n, 1, 64 - k, k)),
        ("across lanes", across_lanes(n, k)),
    ]
}

fn row(label: &str, s: &RankTrialSummary) {
    println!(
        "   {:<30} {:>10.3} {:>10.3} {:>8.2}   d0={} d1={} d2={} d3={} d4+={}",
        label,
        s.full_rank_fraction(),
        s.mean_deficiency,
        s.z_score(),
        s.histogram[0],
        s.histogram[1],
        s.histogram[2],
        s.histogram[3],
        s.histogram[4]
    );
}

fn header() {
    println!(
        "   {:<30} {:>10} {:>10} {:>8}   histogram",
        "condition", "full-rank", "mean def", "z"
    );
}

fn known_answers() {
    println!("=== GF(2) linearity battery — KNOWN ANSWERS ===\n");
    println!("   square regime m = n, {TRIALS} trials, disjoint seeds per condition");
    println!(
        "   theoretical null P(full rank) = {:.4}  (512x512)",
        full_rank_probability(512, 512)
    );
    println!("   z < 0 means MORE rank-deficient than chance — the direction");
    println!("   that indicates linear structure.\n");

    println!("-- 1. The null itself: random matrices --");
    header();
    row(
        "random 512x512",
        &random_matrix_rank_trials(512, TRIALS, 0xC0FFEE),
    );
    println!("   must sit on the null. Identity and planted-linear below prove");
    println!("   the routine detects COLLAPSE; this proves it reproduces the");
    println!("   DISTRIBUTION, which is what every square-regime verdict rests on.\n");

    println!("-- 2. Exactly-linear maps: tall-thin regime, rank must equal k --");
    println!(
        "   {:<30} {:<24} {:>18}  {:>9}",
        "design", "geometry", "rank (5 seeds)", "residual"
    );
    let cases: Vec<(&str, &dyn Permutation, usize)> = vec![
        ("identity", &Identity, 1),
        ("planted-linear", &PlantedLinear, 1),
        ("xoshiro256++", &Xoshiro256pp, 4),
    ];
    for (label, perm, rounds) in cases {
        let n = perm.state_bytes();
        let mut res: Vec<f64> = SEEDS
            .iter()
            .map(|&s| affine_residual(perm, rounds, s, 32))
            .collect();
        res.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (i, (gname, dirs)) in geometries(n, K).into_iter().enumerate() {
            let ranks: Vec<String> = SEEDS
                .iter()
                .map(|&s| {
                    subspace_rank(perm, rounds, &InputSet::Subspace(dirs.clone()), TALL, s)
                        .rank
                        .to_string()
                })
                .collect();
            println!(
                "   {:<30} {:<24} {:>18}  {:>9}",
                if i == 0 { label } else { "" },
                gname,
                ranks.join(" "),
                if i == 0 {
                    format!("{:.4}", res[res.len() / 2])
                } else {
                    String::new()
                }
            );
        }
    }
    println!("   expected: rank = k = {K} exactly, residual exactly 0.0000.");
    println!("   A routine returning min(m,n) regardless would read {TALL} here.\n");

    println!("-- 3. Negative control: 20-round ChaCha, both regimes --");
    header();
    row(
        "chacha r20, square 512x512",
        &rank_trials(&ChaCha, 20, &InputSet::Stride(1), 512, TRIALS, seed_base(0)),
    );
    let tall = subspace_rank(&ChaCha, 20, &InputSet::Stride(1), TALL, 1);
    println!(
        "   chacha r20, tall {TALL}x512      rank {} of {TALL}, residual {:.4}",
        tall.rank,
        affine_residual(&ChaCha, 20, 1, 32)
    );
    println!("   without this every check above would be satisfied by a routine");
    println!("   that always collapses.");
}

fn full() {
    println!("\n\n=== NOT KNOWN IN ADVANCE ===\n");
    println!("   Ground truth already held, from two other instruments:");
    println!("     PractRand BRank   3 rounds fails on 3/4 seeds, flags 4/4;");
    println!("                       zero anomalies of any kind at 4 rounds");
    println!("     N3-STATISTICAL    low-Hamming-weight strides fail at 3 rounds,");
    println!("                       weight >= 8 and golden gamma clean\n");

    println!("-- A. Round sweep, square regime, consecutive counters --");
    header();
    for (i, r) in [2usize, 3, 4, 6, 20].iter().enumerate() {
        row(
            &format!("chacha {r} rounds"),
            &rank_trials(&ChaCha, *r, &InputSet::Stride(1), 512, TRIALS, seed_base(i)),
        );
    }

    println!("\n-- B. Stride sweep at 3 rounds — third route to N3-STATISTICAL --");
    header();
    for (i, s) in [1u64, 2, 16, 256, 65536, 255, GAMMA].iter().enumerate() {
        row(
            &format!("stride {s} (w={})", s.count_ones()),
            &rank_trials(
                &ChaCha,
                3,
                &InputSet::Stride(*s),
                512,
                TRIALS,
                seed_base(10 + i),
            ),
        );
    }

    println!("\n-- C. The tall-thin regime is a DIFFERENT property, not a worse one --");
    println!(
        "   {:<30} {:>18}",
        "chacha rounds", "rank, 64x512 (5 seeds)"
    );
    for r in [1usize, 2, 3, 4] {
        let ranks: Vec<String> = SEEDS
            .iter()
            .map(|&s| {
                subspace_rank(&ChaCha, r, &InputSet::Stride(1), TALL, s)
                    .rank
                    .to_string()
            })
            .collect();
        println!("   {:<30} {:>18}", format!("{r} rounds"), ranks.join(" "));
    }
    println!("   Full rank from 2 rounds up, while section A finds 3 rounds");
    println!("   deficient. Both are correct: 'weakly linearly correlated' and");
    println!("   'affine on a small subspace' are SEPARABLE properties, and");
    println!("   reduced-round ChaCha has the first without the second.");
    println!("   xoshiro256++ has both. That separation is only visible because");
    println!("   both regimes exist, and it was found by trying the wrong one.");

    println!("\n-- D. Registry sweep at 4 rounds, square regime --");
    header();
    for (i, name) in ["chacha", "chacha64", "blake2b", "ascon", "wide-cross"]
        .iter()
        .enumerate()
    {
        let p = permutation_by_name(name).unwrap();
        let n = p.state_bytes() * 8;
        row(
            &format!("{name} ({n} bits)"),
            &rank_trials(
                p.as_ref(),
                4,
                &InputSet::Stride(1),
                n,
                TRIALS,
                seed_base(20 + i),
            ),
        );
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "known".into());
    known_answers();
    match mode.as_str() {
        "known" => println!("\n(known-answer mode: stopping before the unknowns)"),
        "full" => full(),
        other => {
            eprintln!("unknown mode: {other} (use known|full)");
            std::process::exit(2);
        }
    }
}
