//! Lifecycle harness — proposal §9.4, mode 2 (the same seed reaching two
//! places it should not).
//!
//! `PHASE_I_SEEDING_GAP_ANALYSIS.md` found the existing battery would have
//! reported clean on both CVEs §3.7 names, and that for the fork/snapshot mode
//! the detector already existed while the trigger did not. This is the trigger.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example lifecycle_report
//! ```
//!
//! **`fork(2)` is modelled, not called.** This tests whether a CONSTRUCTION is
//! fork-safe, not whether an IMPLEMENTATION wires up its reseed hook. See the
//! `lifecycle` module header before citing anything here.

use statelab_crypto::lifecycle::{run, Scenario, CONSTRUCTIONS, SCENARIOS};

const SEEDS: [u64; 5] = [1, 2, 12345, 0xDEAD_BEEF, 1 << 63];

fn main() {
    println!("LIFECYCLE HARNESS — fork, sibling fork, and VM snapshot");
    println!("Collision oracle: any shared 32-byte window between two instances.");
    println!("Chance collision probability ~2^-256, so a hit is structural.\n");

    println!("  Constructions:");
    for (label, _) in CONSTRUCTIONS {
        println!("    {label}");
    }
    println!("\n  Verdict key:  SAFE = no two instances shared output");
    println!("                COLLIDE = at least one pair emitted the same bytes");
    println!(
        "  Multi-seed: every cell is {} seeds — item (10).\n",
        SEEDS.len()
    );

    println!(
        "  {:<34} {:>20} {:>20} {:>22}",
        "scenario", "naive-rekey", "fast-key-erasure", "fke + wipe-on-fork"
    );
    println!("  {}", "-".repeat(98));

    for scenario in SCENARIOS {
        let mut cells: Vec<String> = Vec::new();
        for (_, build) in CONSTRUCTIONS {
            // Every seed must agree, or the cell is reported as unstable rather
            // than collapsed to a majority verdict.
            let verdicts: Vec<bool> = SEEDS
                .iter()
                .map(|&s| run(scenario, build, s).is_safe())
                .collect();
            let all_safe = verdicts.iter().all(|&v| v);
            let none_safe = verdicts.iter().all(|&v| !v);
            cells.push(match (all_safe, none_safe) {
                (true, _) => "SAFE".to_string(),
                (_, true) => "COLLIDE".to_string(),
                _ => format!(
                    "UNSTABLE {}/{}",
                    verdicts.iter().filter(|&&v| v).count(),
                    SEEDS.len()
                ),
            });
        }
        let expected = match scenario.expected_collision() {
            Some(true) => "  [control: must COLLIDE]",
            Some(false) => "  [control: must be SAFE]",
            None => "",
        };
        println!(
            "  {:<34} {:>20} {:>20} {:>22}{}",
            scenario.name(),
            cells[0],
            cells[1],
            cells[2],
            expected
        );
    }

    // ---------------------------------------------------------------- detail
    println!("\n-- Detail: who collided with whom, fast-key-erasure, seed 1 --");
    for scenario in SCENARIOS {
        let r = run(scenario, CONSTRUCTIONS[1].1, 1);
        if r.collisions.is_empty() {
            continue;
        }
        println!("  {}", scenario.name());
        for c in &r.collisions {
            println!(
                "      {:<28} <-> {:<28} {} shared windows",
                c.a, c.b, c.shared_windows
            );
        }
    }

    println!("\n-- What these results mean --");
    println!("  1. FORWARD SECRECY DOES NOT IMPLY FORK SAFETY. Fast key erasure");
    println!("     is the construction H1 was closed on and has been in the Linux");
    println!("     kernel since 2022. A forked child inherits the same key AND the");
    println!("     same unconsumed buffer, so it re-emits its parent's output.");
    println!("     The property it provides is orthogonal to this one.");
    println!();
    println!("  2. THE PLATFORM WIPE ALONE IS NOT SUFFICIENT. With MADV_WIPEONFORK");
    println!("     modelled but no entropy delivered, the child knows it forked and");
    println!("     has nothing to reseed from. Siblings still collide.");
    println!();
    println!("  3. SIBLINGS ARE THE HARD CASE, AND THE REASON IS STRUCTURAL. Two");
    println!("     children of one parent have byte-identical memory, so ANY");
    println!("     function of their own state returns the same answer for both.");
    println!("     Fresh entropy must come from outside the process image. This is");
    println!("     not a detail of the model — it is why userspace alone cannot fix");
    println!("     fork safety.");
    println!();
    println!("  4. NOTHING SURVIVES A VM SNAPSHOT. No fork occurs, so no wipe and");
    println!("     no notification fire; from inside the guest nothing happened.");
    println!("     §3.7 calls this genuinely unsolved at the architecture level and");
    println!("     this reproduces that rather than contradicting it.");
    println!();
    println!("  A SAFE verdict asserts only that no two instances shared output. It");
    println!("  says nothing about output quality and nothing about security.");
    println!("  Phase 7 is untouched by any of this.");

    // Controls last, so a broken harness is visible after the table rather than
    // trusted before it.
    println!("\n-- Control check --");
    let mut ok = true;
    for scenario in [Scenario::DistinctSeeds, Scenario::SameSeed] {
        let want = scenario.expected_collision().expect("control");
        for (label, build) in CONSTRUCTIONS {
            for &s in &SEEDS {
                let got_collision = !run(scenario, build, s).is_safe();
                if got_collision != want {
                    println!(
                        "  *** CONTROL FAILED: {} / {label} / seed {s}",
                        scenario.name()
                    );
                    ok = false;
                }
            }
        }
    }
    if ok {
        println!("  Both controls behave for all constructions at all seeds.");
        println!("  The negative control passing is what makes a COLLIDE meaningful;");
        println!("  the positive control failing is what makes a SAFE meaningful.");
    }
}
