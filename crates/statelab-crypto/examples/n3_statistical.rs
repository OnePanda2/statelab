//! `N3-STATISTICAL` — the seed → first-output map, tested statistically.
//!
//! §6.4's entire text on N3 is "the seed-to-first-output map tested as a
//! function in its own right". That admits two readings and they are not
//! redundant:
//!
//! - **`N3-DIFFUSION`** (in `correlation.rs`) — *does the map diffuse?* Flip one
//!   seed bit, watch the output bits move. An avalanche matrix over seed bits,
//!   averaged over **random** base seeds.
//! - **`N3-STATISTICAL`** (here) — *is the map's image structured?* Walk the
//!   seed through a **sequence**, take output[0] from each, and hand the result
//!   to an external battery.
//!
//! The second catches what the first is structurally blind to, and this is
//! measured rather than argued: at 3 rounds `N3-DIFFUSION` reads 0.0928 against
//! a 0.12 tolerance and passes, while `N3-STATISTICAL` on the same design at the
//! same round count fails. Avalanche is a statement about *pairs* averaged over
//! random seeds; clustering is a property of the image of a *structured
//! sequence*, and no pairwise statistic over random inputs can see it.
//!
//! ```text
//! n3_statistical [--system S] [--rounds N] [--zero-frac F]
//!                [--stride N] [--start N] [--mode concat|xor] | RNG_test stdin64
//! ```
//!
//! ## Two constructions
//!
//! `concat` emits `out(s)`, `out(s+k)`, `out(s+2k)`, … — the sequence itself,
//! which an external battery scans for structure across rows.
//!
//! `xor` emits `out(s) ⊕ out(s+k)` for successive `s` — adjacent differences.
//! An independently-coded route to the same question: if outputs from
//! neighbouring seeds are linearly related, differencing exposes it directly
//! rather than relying on the battery to find rank deficiency across rows. The
//! XOR construction has already served as a cross-check twice in this project,
//! for N1 and for the N2 masking result.
//!
//! ## Seed sequence
//!
//! `stride` defaults to 1: `start, start+1, start+2, …`. That is both the
//! proposal's own "seeds 1, 2, 3 … N" and the deployment-realistic case, since
//! process IDs, timestamps and loop counters produce small strides. `start`
//! exists because a single starting point has produced a wrong answer four times
//! in this project.
//!
//! ## The input-side trap applies here too
//!
//! `N3-DIFFUSION` had to be run zero-filled: the keyed construction expands the
//! seed through SplitMix64 on its way to the state tail, so a keyed run scores
//! that expansion rather than the permutation. **The same applies here, and it
//! is confirmed rather than assumed** — one-round ChaCha, which cannot have
//! diffused anything, reads 333 failures isolated and 1 keyed. So `zero_frac`
//! defaults to **1.0** here, the opposite of everything else in the suite.

use statelab_crypto::stream::{emit_block, StreamConfig};
use statelab_crypto::{permutation_by_name, PERMUTATIONS};
use std::io::{self, BufWriter, Write};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Emit the sequence of first outputs.
    Concat,
    /// Emit adjacent differences of the sequence.
    Xor,
}

struct Args {
    system: String,
    rounds: usize,
    zero_frac: f64,
    stride: u64,
    start: u64,
    mode: Mode,
}

fn usage() -> String {
    format!(
        "n3_statistical — seed -> output[0] over a seed sequence, for N3-STATISTICAL\n\
         \n\
         USAGE:\n    n3_statistical [--system S] [--rounds N] [--zero-frac F]\n\
         \x20                  [--stride N] [--start N] [--mode concat|xor]\n\
         \n\
         --system     default chacha; one of:\n    {}\n\
         --rounds     default 0 = the design's own round count\n\
         --zero-frac  default 1.0 = isolate the permutation from the key schedule\n\
         --stride     default 1 = consecutive seeds, the deployment-realistic case\n\
         --start      default 0 = first seed in the sequence\n\
         --mode       concat (default) emits the sequence; xor emits adjacent differences\n",
        PERMUTATIONS.join(", ")
    )
}

fn parse() -> Result<Args, String> {
    let mut a = Args {
        system: "chacha".into(),
        rounds: 0,
        // NOT StreamConfig's default. See the module note: keyed input scores
        // the key schedule, not the permutation.
        zero_frac: 1.0,
        stride: 1,
        start: 0,
        mode: Mode::Concat,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "-h" | "--help" => return Err(usage()),
            "--system" => a.system = value()?,
            "--rounds" => a.rounds = value()?.parse().map_err(|_| "--rounds must be a number")?,
            "--zero-frac" => {
                a.zero_frac = value()?
                    .parse()
                    .map_err(|_| "--zero-frac must be a number in [0,1]")?;
                if !(0.0..=1.0).contains(&a.zero_frac) {
                    return Err("--zero-frac must be in [0,1]".into());
                }
            }
            "--stride" => {
                a.stride = value()?.parse().map_err(|_| "--stride must be a number")?;
                if a.stride == 0 {
                    return Err("--stride must be nonzero".into());
                }
            }
            "--start" => a.start = value()?.parse().map_err(|_| "--start must be a number")?,
            "--mode" => {
                a.mode = match value()?.as_str() {
                    "concat" => Mode::Concat,
                    "xor" => Mode::Xor,
                    other => return Err(format!("unknown --mode: {other}")),
                }
            }
            other => return Err(format!("unknown flag: {other}\n\n{}", usage())),
        }
    }
    Ok(a)
}

fn main() {
    let args = match parse() {
        Ok(a) => a,
        Err(m) => {
            eprintln!("{m}");
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
    let perm = perm.as_ref();

    let stdout = io::stdout();
    let mut out = BufWriter::with_capacity(1 << 16, stdout.lock());
    let mut scratch = vec![0u8; perm.state_bytes()];
    let mut cur = Vec::with_capacity(perm.state_bytes());
    let mut next = Vec::with_capacity(perm.state_bytes());

    let block_for = |seed: u64| StreamConfig {
        seed,
        rounds: args.rounds,
        zero_frac: args.zero_frac,
        ..StreamConfig::default()
    };

    let mut seed = args.start;
    loop {
        // Block 0 only: the *first* output a freshly seeded generator hands out,
        // which is what a caller who reseeds actually receives, and the object
        // §6.4 names.
        emit_block(perm, &block_for(seed), 0, &mut scratch, &mut cur);

        if args.mode == Mode::Xor {
            let partner = seed.wrapping_add(args.stride);
            emit_block(perm, &block_for(partner), 0, &mut scratch, &mut next);
            for (a, b) in cur.iter_mut().zip(&next) {
                *a ^= *b;
            }
        }

        // A closed pipe is the normal end: the battery hit its limit.
        if out.write_all(&cur).is_err() {
            return;
        }
        seed = seed.wrapping_add(args.stride);
    }
}
