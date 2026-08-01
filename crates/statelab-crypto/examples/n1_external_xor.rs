//! External N1 — inter-stream correlation, via XOR (proposal §6.4).
//!
//! `TASK_3_STREAM_INDEPENDENCE.md` measures N1 internally: for each pair of
//! seeds and each output bit position, the rate at which the two streams agree,
//! which should be 0.5. That battery says every cryptographic design in the
//! registry has independent streams. It has never been cross-validated against
//! an external battery the way N2 now has.
//!
//! ## The construction
//!
//! PractRand consumes one stream, so two streams are tested for independence by
//! XOR-ing them. If `A` and `B` are independent and uniform, `A ⊕ B` is uniform;
//! if they are correlated, the correlation survives into the XOR. This is the
//! same null hypothesis the internal N1 tests — agreement rate 0.5 at every bit
//! position is exactly "the XOR is unbiased" — reached by a different route and
//! measured by code sharing nothing with ours.
//!
//! ```text
//! n1_external_xor <seedA> <seedB> [rounds] [system] | RNG_test stdin64
//! ```
//!
//! `rounds` 0 means the design's own default. Both streams use identical
//! settings and differ only in seed, so the seed is the single variable.
//!
//! ## Controls
//!
//! Passing the same seed twice emits all zeros, which must fail instantly. That
//! is the trivial positive control and it exists because N2 taught this project
//! that a quiet external result is worthless until the path is shown to be
//! capable of a loud one. The informative control is a low round count, where
//! the streams are individually defective.
//!
//! ## Why XOR and not interleaving
//!
//! N2 established that round-robin interleaving *masks* within-stream linear
//! structure: 3-round ChaCha fails `BRank` on 3 of 4 seeds single-stream and on
//! none of 4 interleaved. XOR should behave differently — the XOR of two
//! GF(2)-linear structures is still GF(2)-linear, so rank deficiency ought to
//! survive rather than dilute. That is a prediction this tool exists to test,
//! not an assumption it relies on.

use statelab_crypto::stream::{emit_block, StreamConfig};
use statelab_crypto::{permutation_by_name, PERMUTATIONS};
use std::io::{self, BufWriter, Write};

fn usage() -> String {
    format!(
        "n1_external_xor — XOR of two seeded streams, for external N1\n\
         \n\
         USAGE:\n    n1_external_xor <seedA> <seedB> [rounds] [system]\n\
         \n\
         rounds   0 (default) uses the design's own round count\n\
         system   defaults to chacha; one of:\n    {}\n\
         \n\
         Passing the same seed twice emits all zeros — the positive control.\n",
        PERMUTATIONS.join(", ")
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("{}", usage());
        std::process::exit(2);
    }
    let parse = |s: &String, what: &str| -> u64 {
        s.parse().unwrap_or_else(|_| {
            eprintln!("{what} must be a number, got {s}");
            std::process::exit(2);
        })
    };
    let seed_a = parse(&args[0], "seedA");
    let seed_b = parse(&args[1], "seedB");
    let rounds = args.get(2).map_or(0, |s| parse(s, "rounds") as usize);
    let system = args.get(3).map_or("chacha", |s| s.as_str());

    let Some(perm) = permutation_by_name(system) else {
        eprintln!("unknown system: {system}\navailable: {}", PERMUTATIONS.join(", "));
        std::process::exit(2);
    };
    let perm = perm.as_ref();

    // Identical but for the seed. Keyed, raw state — the honest configuration,
    // and the same one the internal N1 uses so the two are comparable.
    let base = StreamConfig {
        rounds,
        zero_frac: 0.0,
        ..StreamConfig::default()
    };
    let cfg_a = StreamConfig { seed: seed_a, ..base };
    let cfg_b = StreamConfig { seed: seed_b, ..base };

    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());
    let mut scratch = vec![0u8; perm.state_bytes()];
    let mut block_a = Vec::with_capacity(perm.state_bytes());
    let mut block_b = Vec::with_capacity(perm.state_bytes());

    let mut block: u64 = 0;
    loop {
        emit_block(perm, &cfg_a, block, &mut scratch, &mut block_a);
        emit_block(perm, &cfg_b, block, &mut scratch, &mut block_b);
        for (a, b) in block_a.iter_mut().zip(&block_b) {
            *a ^= *b;
        }
        // A closed pipe is the normal end: PractRand hit its limit.
        if out.write_all(&block_a).is_err() {
            return;
        }
        block = block.wrapping_add(1);
    }
}
