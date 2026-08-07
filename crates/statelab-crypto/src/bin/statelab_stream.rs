//! `statelab-stream` — emits raw bytes from any registered permutation.
//!
//! Exists so the external batteries work with no glue code:
//!
//! ```text
//! statelab-stream --system chacha --rounds 20 | RNG_test stdin64
//! statelab-stream --system counter --extract strong | RNG_test stdin64
//! statelab-stream --system chacha --interleave 8 | RNG_test stdin64
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
//!
//! The `strong` mode exists to *prove* that, not to flatter anything. Pairing
//! it with `--system counter` reproduces SplitMix64 — a published, widely used
//! generator whose state map is a bare counter and whose output passes every
//! statistical battery. Same state evolution, opposite verdicts, decided
//! entirely by the output function.
//!
//! ## Where the construction lives
//!
//! In [`statelab_crypto::stream`], not here. The seed-correlation battery needs
//! to measure the object this binary emits, and a second implementation would
//! eventually differ from this one by some detail nobody noticed — producing a
//! disagreement between the internal and external batteries that looked like a
//! finding and was really a bug.

use statelab_crypto::stream::{emit_block, Extract, StreamConfig};
use statelab_crypto::{permutation_by_name, Permutation, PERMUTATIONS};
use std::io::{self, BufWriter, Write};

struct Args {
    cfg: StreamConfig,
    system: String,
    /// Bytes to emit, or `None` to stream until the pipe closes.
    limit: Option<u64>,
    /// Emit blocks round-robin from this many consecutive seeds, for §6.4 N2.
    /// `1` is the ordinary single-stream case.
    interleave: u64,
}

fn usage() -> String {
    format!(
        "statelab-stream — raw byte stream from a StateLab permutation\n\
         \n\
         USAGE:\n    statelab-stream [--system NAME] [--rounds N] [--extract raw|low-byte|strong]\n\
         \x20                   [--bytes N] [--seed N] [--interleave N] [--bit-reverse]\n\
         \x20                   [--keyed | --zero-frac F]\n\
         \n\
         SYSTEMS:\n    {}\n\
         \n\
         NOTE: --extract defaults to `raw` on purpose. Statistical tests run\n\
         over a strong extraction measure the extractor, not the permutation.\n\
         \n\
         NOTE: --zero-frac defaults to 1.0, the `seed || counter || zeros`\n\
         construction the earlier reports used. Pass --keyed for the realistic\n\
         fully-keyed input. The default is kept only so previously recorded\n\
         results keep describing this binary.\n",
        PERMUTATIONS.join(", ")
    )
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        cfg: StreamConfig {
            seed: 0,
            rounds: 0, // 0 means "use the design's default"
            extract: Extract::Raw,
            // NOT StreamConfig::default(), which is 0.0. Changing this would
            // silently reinterpret every PractRand result already recorded.
            zero_frac: 1.0,
            bit_reverse: false,
        },
        system: "chacha".to_string(),
        limit: None,
        interleave: 1,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "-h" | "--help" => return Err(usage()),
            "--keyed" => a.cfg.zero_frac = 0.0,
            "--bit-reverse" => a.cfg.bit_reverse = true,
            "--zero-frac" => {
                a.cfg.zero_frac = value()?
                    .parse::<f64>()
                    .map_err(|_| "--zero-frac must be a number in [0,1]")?;
                if !(0.0..=1.0).contains(&a.cfg.zero_frac) {
                    return Err("--zero-frac must be in [0,1]".to_string());
                }
            }
            "--system" => a.system = value()?,
            "--rounds" => {
                a.cfg.rounds = value()?.parse().map_err(|_| "--rounds must be a number")?
            }
            "--bytes" => a.limit = Some(value()?.parse().map_err(|_| "--bytes must be a number")?),
            "--seed" => a.cfg.seed = value()?.parse().map_err(|_| "--seed must be a number")?,
            "--interleave" => {
                a.interleave = value()?
                    .parse()
                    .map_err(|_| "--interleave must be a number")?;
                if a.interleave == 0 {
                    return Err("--interleave must be at least 1".to_string());
                }
            }
            "--extract" => {
                let raw = value()?;
                a.cfg.extract =
                    Extract::parse(&raw).ok_or_else(|| format!("unknown --extract mode: {raw}"))?;
            }
            other => return Err(format!("unknown flag: {other}\n\n{}", usage())),
        }
    }
    Ok(a)
}

/// Emits blocks until the limit or a closed pipe.
///
/// With `--interleave n`, block *b* is taken from seed `seed + (b mod n)` and
/// block index `b / n`, so `n` independent streams are woven round-robin. That
/// is the N2 construction: seed-lattice structure is invisible when each stream
/// is tested alone and appears here as short-lag correlation.
fn run(args: &Args, perm: &dyn Permutation, out: &mut impl Write) -> io::Result<()> {
    let mut written: u64 = 0;
    let mut index: u64 = 0;
    let mut scratch = vec![0u8; perm.state_bytes()];
    let mut emit = Vec::with_capacity(perm.state_bytes());

    loop {
        let cfg = StreamConfig {
            seed: args.cfg.seed.wrapping_add(index % args.interleave),
            ..args.cfg
        };
        emit_block(perm, &cfg, index / args.interleave, &mut scratch, &mut emit);

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
        index = index.wrapping_add(1);
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
