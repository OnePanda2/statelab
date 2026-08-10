//! Entropy accounting — the reachable seed set as an object you can measure.
//!
//! # Why this module exists
//!
//! `PHASE_I` split real-world PRNG failures into exactly two modes:
//!
//! > **either the seed carries too little entropy, or the same seed reaches two
//! > places it should not.**
//!
//! Mode two got a lifecycle harness (`crate::lifecycle`) and a nine-generator
//! survey. **Mode one had no detector at all**, and `PHASE_I` §7 named it "the
//! half that does not exist". This is that half.
//!
//! ## The thing that makes it a category difference, not a resolution problem
//!
//! Debian's CVE-2008-0166 reduced OpenSSL's seed to the PID range — roughly 15
//! bits, 32,767 reachable keys. **Every battery in this crate reports that
//! clean**, and correctly so: N1 compares `stream(seedA)` against
//! `stream(seedB)` and finds them genuinely uncorrelated, because the mixing
//! function was never the problem. SAC, BIC, GF(2) rank and PractRand all
//! consume output from a seed they were handed.
//!
//! > The defect is the **cardinality of the set of reachable streams**, and no
//! > test in the crate takes the reachable seed set as an object it can
//! > measure. You cannot detect a 15-bit seed space by examining streams one
//! > seed at a time, and one seed at a time is the only mode the battery has.
//!
//! A bigger version of the same test does not help. The unit of analysis has to
//! change, and that is all this module does: it makes the **seeding path** the
//! object rather than the stream.
//!
//! # *** WHAT THIS IS NOT ***
//!
//! **This is an accounting surface, not a detector.** It computes the
//! consequences of a seeding path *you declare*. It cannot discover what your
//! seeding path actually is, and it will faithfully report 256 bits for a path
//! you have described wrongly.
//!
//! That limit is not incidental — **it is the exact shape of the original
//! failure.** Nobody at Debian believed the seed was 15 bits. The patch removed
//! an entropy source and no artefact anywhere in the system recorded that the
//! reachable set had collapsed. A tool that makes the declared path explicit
//! and checkable is worth having for precisely that reason, but declaring it
//! correctly remains a human act and this module cannot perform it.
//!
//! Stated plainly so it cannot be mistaken for more: **passing this is not
//! evidence a deployment is well seeded. Failing it is evidence one is not.**

use crate::lifecycle::{detect_collisions, Stream};

/// One entropy source contributing to a seed.
#[derive(Debug, Clone)]
pub enum Source {
    /// Values drawn from a known finite range — a PID, a timestamp at a known
    /// granularity, a boot counter, a sequence number. **The cardinality is the
    /// entropy**, regardless of how wide the field holding it is.
    Enumerable { name: String, cardinality: u128 },

    /// A source believed to deliver full entropy: `getrandom(2)`, `getentropy`,
    /// `RDRAND`, a hardware TRNG.
    Full { name: String, bits: u32 },

    /// Fixed for the lifetime of a deployment: a MAC address, a value baked
    /// into a container image, anything captured in a golden VM snapshot.
    ///
    /// **Contributes ZERO to the reachable set across instances of that
    /// deployment, however many bits wide it looks.** This is the variant that
    /// makes the module more than addition — a 48-bit MAC address reads as 48
    /// bits of seed material in every code review and as 0 bits across the
    /// fleet booted from one image.
    Fixed { name: String, apparent_bits: u32 },
}

impl Source {
    /// Entropy this source contributes **across instances of one deployment**.
    pub fn effective_bits(&self) -> f64 {
        match self {
            Source::Enumerable { cardinality, .. } => {
                if *cardinality <= 1 {
                    0.0
                } else {
                    (*cardinality as f64).log2()
                }
            }
            Source::Full { bits, .. } => f64::from(*bits),
            Source::Fixed { .. } => 0.0,
        }
    }

    /// What a reader would plausibly *assume* the source contributes, from its
    /// width. The gap between this and [`Self::effective_bits`] is the finding.
    pub fn apparent_bits(&self) -> f64 {
        match self {
            Source::Enumerable { cardinality, .. } => {
                if *cardinality <= 1 {
                    0.0
                } else {
                    (*cardinality as f64).log2()
                }
            }
            Source::Full { bits, .. } => f64::from(*bits),
            Source::Fixed { apparent_bits, .. } => f64::from(*apparent_bits),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Source::Enumerable { name, .. }
            | Source::Full { name, .. }
            | Source::Fixed { name, .. } => name,
        }
    }
}

/// A declared seeding path: how one deployment produces the seed it starts from.
#[derive(Debug, Clone, Default)]
pub struct SeedingPath {
    pub label: String,
    pub sources: Vec<Source>,
}

impl SeedingPath {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            sources: Vec::new(),
        }
    }

    pub fn with(mut self, s: Source) -> Self {
        self.sources.push(s);
        self
    }

    /// Entropy of the reachable seed set, in bits.
    ///
    /// Sources are summed, which assumes independence. That assumption is
    /// **stated rather than checked** — a PID and a start timestamp are not
    /// independent for processes launched in sequence, and this returns an
    /// upper bound in that case. An upper bound is the safe direction here: it
    /// can only make a path look *better* than it is, so a failure reported by
    /// this function is a real failure.
    pub fn reachable_bits(&self) -> f64 {
        self.sources.iter().map(Source::effective_bits).sum()
    }

    /// What the path looks like it delivers if `Fixed` sources are read at face
    /// value — i.e. what a code review counts.
    pub fn apparent_bits(&self) -> f64 {
        self.sources.iter().map(Source::apparent_bits).sum()
    }

    /// Entropy visible in review but absent across the fleet.
    pub fn phantom_bits(&self) -> f64 {
        self.apparent_bits() - self.reachable_bits()
    }

    /// Exact size of the reachable set, when it is small enough to name.
    pub fn reachable_count(&self) -> Option<u128> {
        let b = self.reachable_bits();
        if b >= 127.0 {
            return None;
        }
        Some(2u128.saturating_pow(b.round() as u32))
    }

    /// Instances at which two share a seed with probability ~50%.
    ///
    /// The birthday bound, `1.1774 * sqrt(N)`. This is the number that makes a
    /// bit-count concrete: 15 bits is not "small", it is **a coin flip at a few
    /// hundred hosts**.
    pub fn birthday_instances(&self) -> f64 {
        let n = self.reachable_bits().exp2();
        1.177_410_02_f64 * n.sqrt()
    }

    /// Probability at least two of `instances` deployments share a seed.
    ///
    /// Uses the standard `1 - exp(-k(k-1)/2N)` approximation.
    pub fn collision_probability(&self, instances: u64) -> f64 {
        let n = self.reachable_bits().exp2();
        if n <= 1.0 {
            return 1.0;
        }
        let k = instances as f64;
        1.0 - (-(k * (k - 1.0)) / (2.0 * n)).exp()
    }

    /// Verdict against a required strength.
    pub fn meets(&self, required_bits: f64) -> bool {
        self.reachable_bits() >= required_bits
    }
}

/// A path small enough to enumerate, checked **empirically** rather than
/// arithmetically: derive every reachable seed, generate a stream from each,
/// and run the validated collision oracle over the lot.
///
/// This is the bridge between the accounting above and the instrument that
/// already exists. `detect_collisions` is documented as independent of how the
/// streams were produced, which is exactly what lets it be reused here.
///
/// `derive` maps a seed index to a 32-byte key, modelling the deployment's
/// key-derivation step. `emit` produces stream bytes from that key.
pub fn enumerate_and_check<D, E>(
    path: &SeedingPath,
    limit: u128,
    derive: D,
    emit: E,
) -> EnumerationResult
where
    D: Fn(u128) -> [u8; 32],
    E: Fn(&[u8; 32]) -> Vec<u8>,
{
    let count = path.reachable_count().unwrap_or(u128::MAX);
    let n = count.min(limit);
    let streams: Vec<Stream> = (0..n)
        .map(|i| Stream {
            label: format!("seed#{i}"),
            bytes: emit(&derive(i)),
        })
        .collect();
    let collisions = detect_collisions(&streams);
    EnumerationResult {
        enumerated: n,
        reachable: count,
        distinct_streams: n as usize - collisions.len(),
        collisions: collisions.len(),
    }
}

#[derive(Debug, Clone)]
pub struct EnumerationResult {
    pub enumerated: u128,
    pub reachable: u128,
    pub distinct_streams: usize,
    pub collisions: usize,
}

// ---------------------------------------------------------------------------
// Known-answer paths — the positive controls
// ---------------------------------------------------------------------------

/// Debian OpenSSL, CVE-2008-0166. PID as the sole entropy source.
///
/// The proposal's verified Appendix A.13 records **32,767 possible keys**.
/// Reproducing ~15 bits from a path description is the known-answer test for
/// this module: if it cannot recover a documented historical failure, it has no
/// business being pointed at anything current.
pub fn debian_2008() -> SeedingPath {
    SeedingPath::new("Debian OpenSSL CVE-2008-0166").with(Source::Enumerable {
        name: "pid".into(),
        cardinality: 32_768,
    })
}

/// A healthy modern path: 32 bytes straight from the kernel CSPRNG.
pub fn healthy_getrandom() -> SeedingPath {
    SeedingPath::new("getrandom(2), 32 bytes").with(Source::Full {
        name: "getrandom".into(),
        bits: 256,
    })
}

/// The shape that motivates the `Fixed` variant: a container image that bakes a
/// seed at build time and adds a PID at run time. Reads as 304 bits in review.
pub fn baked_image() -> SeedingPath {
    SeedingPath::new("seed baked into image + pid")
        .with(Source::Fixed {
            name: "seed captured at image build".into(),
            apparent_bits: 256,
        })
        .with(Source::Enumerable {
            name: "pid".into(),
            cardinality: 32_768,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// *** THE KNOWN-ANSWER TEST. ***
    ///
    /// Debian's reachable key space was 32,767 keys. If this module cannot
    /// recover ~15 bits from the path description, nothing else it reports is
    /// worth reading.
    #[test]
    fn debian_reproduces_fifteen_bits() {
        let p = debian_2008();
        let bits = p.reachable_bits();
        assert!((bits - 15.0).abs() < 0.01, "expected ~15 bits, got {bits}");
        assert_eq!(p.reachable_count(), Some(32_768));
    }

    /// 15 bits is a coin flip at a few hundred hosts, not at millions. The
    /// birthday bound is what turns a bit-count into a deployment-scale fact.
    #[test]
    fn debian_collides_at_a_few_hundred_hosts() {
        let p = debian_2008();
        let k = p.birthday_instances();
        assert!((200.0..230.0).contains(&k), "birthday instances {k}");
        assert!(p.collision_probability(213) > 0.49);
        assert!(p.collision_probability(1000) > 0.99);
    }

    #[test]
    fn healthy_path_is_not_flagged() {
        let p = healthy_getrandom();
        assert_eq!(p.reachable_bits(), 256.0);
        assert!(p.meets(128.0));
        assert_eq!(p.reachable_count(), None);
        assert!(p.collision_probability(1_000_000_000) < 1e-50);
    }

    /// The `Fixed` variant is the whole point: 304 apparent bits, 15 real ones.
    #[test]
    fn baked_image_hides_256_phantom_bits() {
        let p = baked_image();
        assert_eq!(p.apparent_bits(), 271.0);
        assert!((p.reachable_bits() - 15.0).abs() < 0.01);
        assert!((p.phantom_bits() - 256.0).abs() < 0.01);
        assert!(!p.meets(128.0));
    }

    /// A path that passes review by summing widths, and fails on cardinality.
    #[test]
    fn summing_widths_is_the_error_this_catches() {
        let p = baked_image();
        assert!(p.apparent_bits() > 128.0, "review would pass this");
        assert!(!p.meets(128.0), "cardinality says no");
    }

    /// The empirical bridge: a tiny reachable set really does produce a small
    /// number of distinct streams, checked with the validated oracle rather
    /// than asserted from arithmetic.
    #[test]
    fn enumeration_agrees_with_the_arithmetic() {
        let p = SeedingPath::new("toy").with(Source::Enumerable {
            name: "counter".into(),
            cardinality: 64,
        });
        let r = enumerate_and_check(
            &p,
            64,
            |i| {
                let mut k = [0u8; 32];
                k[..16].copy_from_slice(&i.to_le_bytes());
                k
            },
            |k| {
                let mut block = [0u8; 64];
                crate::generator::chacha20_block(k, 0, &[0u8; 12], &mut block);
                block.to_vec()
            },
        );
        assert_eq!(r.reachable, 64);
        assert_eq!(r.enumerated, 64);
        // Distinct seeds, distinct streams — the mixing function is fine. That
        // is exactly why the battery reports Debian clean.
        assert_eq!(r.collisions, 0);
    }
}
