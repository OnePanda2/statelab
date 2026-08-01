//! Positive control for the **external** N2 path (proposal §6.4).
//!
//! `statelab-stream --interleave 8 | RNG_test stdin64` reports no difference
//! from a single stream for every cryptographic design in the registry. That
//! result is worth nothing on its own. A test that never fires is
//! indistinguishable from a test that cannot fire, and this project has already
//! been caught once by a battery that was silent for the wrong reason — the
//! avalanche noise floor, where the positive control failing was the only thing
//! that revealed it.
//!
//! Nothing in the registry supplies the missing case. The cryptographic designs
//! genuinely have independent streams; the non-cryptographic ones fail
//! single-stream anyway, so interleaving them proves nothing about interleaving.
//! What is needed is a stream that **passes alone and fails woven**, and since
//! no design provides one, this constructs it.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example n2_external_control -- dup   | RNG_test stdin64
//! cargo run -p statelab-crypto --release --example n2_external_control -- indep | RNG_test stdin64
//! ```
//!
//! `dup`   — eight **identical** ChaCha streams interleaved. Each is ChaCha at
//!           its shipped round count and passes everything on its own; woven
//!           together the output repeats every 8 blocks. Pure lag-8 structure,
//!           and nothing else about the generator has changed.
//! `indep` — eight streams from consecutive seeds, i.e. exactly what
//!           `--interleave 8` produces. The null case.
//!
//! Measured at 256 MB: `indep` clean in 486 tests, `dup` 310 failures with
//! `p = 0`. Same permutation, same round count, same byte volume — the only
//! variable is whether the woven streams are independent. So the external N2
//! path detects the failure class N2 exists to catch, and the quiet result on
//! real designs is a statement about those designs rather than about the test.

use statelab_crypto::stream::{emit_block, StreamConfig};
use statelab_crypto::systems::ChaCha;
use statelab_crypto::Permutation;
use std::io::{self, BufWriter, Write};

/// Interleave width. Matches the `--interleave 8` used in the reports.
const WIDTH: u64 = 8;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "dup".to_string());
    let duplicate = match mode.as_str() {
        "dup" => true,
        "indep" => false,
        other => {
            eprintln!(
                "unknown mode: {other}\n\
                 usage: n2_external_control [dup|indep]\n\
                 \n\
                 dup    eight identical streams woven together — must FAIL\n\
                 indep  eight consecutively seeded streams — must PASS"
            );
            std::process::exit(2);
        }
    };

    let perm = ChaCha;
    let base = StreamConfig {
        seed: 1,
        rounds: 0, // the design's own default
        zero_frac: 0.0,
        ..StreamConfig::default()
    };

    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());
    let mut scratch = vec![0u8; perm.state_bytes()];
    let mut block_out = Vec::with_capacity(perm.state_bytes());

    let mut index: u64 = 0;
    loop {
        // In `dup` every lane reuses the same seed, so the eight woven streams
        // are byte-identical and the combined stream has period WIDTH blocks.
        let lane = index % WIDTH;
        let cfg = StreamConfig {
            seed: if duplicate { base.seed } else { base.seed + lane },
            ..base
        };
        emit_block(&perm, &cfg, index / WIDTH, &mut scratch, &mut block_out);

        // A closed pipe is the normal end: PractRand stops at its own limit.
        if out.write_all(&block_out).is_err() {
            return;
        }
        index += 1;
    }
}
