//! `statelab-stream` — emits raw bytes from any registered permutation.
//!
//! Exists so the external batteries work with no glue code:
//!
//! ```text
//! statelab-stream --system chacha --rounds 20 | RNG_test stdin64
//! statelab-stream --system counter --extract sha-ish   # negative control
//! ```
//!
//! ## The extraction trap
//!
//! `--extract` selects what is written to stdout, and the default is `raw`,
//! meaning the permutation's own state. That default is deliberate: a strong
//! extractor makes a counter pass every statistical test in existence, so the
//! honest measurement is of raw state. The other modes exist to run the three
//! configurations the protocol requires (proposal §6.1 M2) — never to make a
//! weak permutation look acceptable.

use statelab_crypto::{permutation_by_name, Permutation, PERMUTATIONS};
use std::io::{self, BufWriter, Write};

/// What gets written to stdout.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Extract {
    /// Configuration (a): raw state. The honest measurement.
    Raw,
    /// Configuration (b): low byte of each 8-byte lane. The sensitivity probe
    /// — low-bit weakness is classic and invisible in whole-word tests.
    LowByte,
}

struct Args {
    system: String,
    rounds: usize,
    extract: Extract,
    /// Bytes to emit, or `None` to stream until the pipe closes.
    limit: Option<u64>,
    seed: u64,
}

fn usage() -> String {
    format!(
        "statelab-stream — raw byte stream from a StateLab permutation\n\
         \n\
         USAGE:\n    statelab-stream [--system NAME] [--rounds N] [--extract raw|low-byte]\n\
         \x20                   [--bytes N] [--seed N]\n\
         \n\
         SYSTEMS:\n    {}\n\
         \n\
         NOTE: --extract defaults to `raw` on purpose. Statistical tests run\n\
         over a strong extraction measure the extractor, not the permutation.\n",
        PERMUTATIONS.join(", ")
    )
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        system: "chacha".to_string(),
        rounds: 0, // 0 means "use the design's default"
        extract: Extract::Raw,
        limit: None,
        seed: 0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "-h" | "--help" => return Err(usage()),
            "--system" => a.system = value()?,
            "--rounds" => a.rounds = value()?.parse().map_err(|_| "--rounds must be a number")?,
            "--bytes" => a.limit = Some(value()?.parse().map_err(|_| "--bytes must be a number")?),
            "--seed" => a.seed = value()?.parse().map_err(|_| "--seed must be a number")?,
            "--extract" => {
                a.extract = match value()?.as_str() {
                    "raw" => Extract::Raw,
                    "low-byte" => Extract::LowByte,
                    other => return Err(format!("unknown --extract mode: {other}")),
                }
            }
            other => return Err(format!("unknown flag: {other}\n\n{}", usage())),
        }
    }
    Ok(a)
}

/// Counter mode: the state is `seed || block_counter`, permuted per block. The
/// simplest construction that turns a permutation into a stream without adding
/// mixing of its own — anything cleverer would contaminate the measurement.
fn run(args: &Args, perm: &dyn Permutation, out: &mut impl Write) -> io::Result<()> {
    let n = perm.state_bytes();
    let rounds = if args.rounds == 0 {
        perm.default_rounds()
    } else {
        args.rounds
    };

    let mut written: u64 = 0;
    let mut block: u64 = 0;
    let mut state = vec![0u8; n];
    let mut emit = Vec::with_capacity(n);

    loop {
        // Fresh state per block, so the permutation is measured rather than
        // whatever feedback chain we might otherwise have invented.
        state.iter_mut().for_each(|b| *b = 0);
        state[..8].copy_from_slice(&args.seed.to_le_bytes());
        state[8..16].copy_from_slice(&block.to_le_bytes());
        perm.permute(&mut state, rounds);

        emit.clear();
        match args.extract {
            Extract::Raw => emit.extend_from_slice(&state),
            Extract::LowByte => emit.extend(state.chunks_exact(8).map(|lane| lane[0])),
        }

        let slice = match args.limit {
            Some(limit) if written + emit.len() as u64 > limit => {
                &emit[..(limit - written) as usize]
            }
            _ => &emit[..],
        };

        // A closed pipe is the normal way this ends: `head`, or PractRand
        // stopping at its own limit. Exit quietly rather than reporting it.
        if let Err(e) = out.write_all(slice) {
            return if e.kind() == io::ErrorKind::BrokenPipe {
                Ok(())
            } else {
                Err(e)
            };
        }

        written += slice.len() as u64;
        if args.limit.is_some_and(|l| written >= l) {
            return out.flush();
        }
        block = block.wrapping_add(1);
    }
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };

    let Some(perm) = permutation_by_name(&args.system) else {
        eprintln!(
            "unknown system: {}\navailable: {}",
            args.system,
            PERMUTATIONS.join(", ")
        );
        std::process::exit(2);
    };

    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());
    if let Err(e) = run(&args, perm.as_ref(), &mut out) {
        eprintln!("stream failed: {e}");
        std::process::exit(1);
    }
}
