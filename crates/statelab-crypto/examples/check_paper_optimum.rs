//! Cross-checks our search optimum against the one the 2016 paper reports.
//!
//! The paper names [14, 24, 19, 11] as giving the highest-mean diffusion
//! matrix (mean 7.0599, sd 3.5353). Our independent sweep landed on
//! [15, 24, 19, *]. This prints both under our implementation of their metric,
//! and demonstrates that the fourth constant they specify is not determined by
//! the measurement that selected it.

use statelab_crypto::qr_diffusion::{chacha_qr, diffusion};

fn main() {
    println!("=== Our metric, 200,000 trials ===\n");
    println!("   {:<20} {:>10} {:>10}", "constants", "mean", "sd");
    for rot in [[16, 12, 8, 7], [14, 24, 19, 11], [15, 24, 19, 11]] {
        let d = diffusion(chacha_qr, rot, 200_000, 0xC0FFEE);
        println!(
            "   {:<20} {:>10.4} {:>10.4}",
            format!("{rot:?}"),
            d.mean,
            d.std_dev
        );
    }

    println!("\n=== The 4th constant of the paper's own optimum ===");
    println!("   [14, 24, 19, l] for every l in 0..32:\n");
    let mut means = Vec::new();
    for l in 0..32u32 {
        means.push(diffusion(chacha_qr, [14, 24, 19, l], 20_000, 7).mean);
    }
    let first = means[0];
    let identical = means.iter().all(|m| *m == first);
    println!("   all 32 means equal? {identical}   (value {first:.4})");
    println!("\n   The paper reports l = 11 specifically. On its own metric all 32");
    println!("   values are tied, so that choice is not determined by the search.");
}
