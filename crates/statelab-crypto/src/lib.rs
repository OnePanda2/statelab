//! # statelab-crypto — the cryptographic measurement instrument
//!
//! Phase 1 of `RESEARCH_PROPOSAL_v2.1`. This crate does **not** propose a
//! cryptographic primitive. It measures them, so that claims about diffusion
//! and structure can be settled by numbers instead of by confidence.
//!
//! ## Relationship to `statelab-engine`
//!
//! None, deliberately. The engine models terminating trajectories over
//! arbitrary-precision integers; a cryptographic permutation is fixed-width,
//! non-terminating and bijective. The proposal anticipated needing an engine
//! trait extension for this; building a separate crate turned out to be the
//! better answer, because it leaves the engine's invariants and its test suite
//! completely untouched.
//!
//! ## The one rule that matters
//!
//! Every battery here measures the **raw permutation, with no output
//! extractor**. A counter plus SHA-256 passes every statistical test ever
//! written, so a battery fed extracted output measures the extractor and
//! nothing else. See `avalanche` and the protocol in proposal §6.1.
//!
//! ## Validating the instrument before trusting it
//!
//! The batteries are bracketed by controls whose answers are known in advance:
//! [`systems::Counter`] must never reach avalanche, [`systems::ChaCha`] must,
//! and [`systems::KlimovShamir`] must show the triangular structure that killed
//! the T-function stream ciphers. These run as ordinary unit tests.

pub mod arx64;
pub mod avalanche;
pub mod bench;
pub mod generator;
pub mod permutation;
pub mod render;
pub mod structural;
pub mod systems;

pub use permutation::{Permutation, SmallMap};

/// Resolves a permutation by name, for binaries and report drivers.
///
/// Returns `None` for an unknown name so callers can list what is available
/// rather than panicking on a typo.
pub fn permutation_by_name(name: &str) -> Option<Box<dyn Permutation>> {
    match name {
        "counter" => Some(Box::new(systems::Counter::default())),
        "chacha" => Some(Box::new(systems::ChaCha)),
        "chacha64" => Some(Box::new(arx64::CHACHA64)),
        "blake2b" => Some(Box::new(arx64::BLAKE2B)),
        "ascon" => Some(Box::new(systems::Ascon)),
        "xoshiro256++" => Some(Box::new(systems::Xoshiro256pp)),
        "lcg" => Some(Box::new(systems::Lcg::default())),
        "splitmix-lanes" => Some(Box::new(systems::SplitMixLanes::default())),
        "klimov-shamir" => Some(Box::new(systems::KlimovShamir::default())),
        "klimov-shamir-transposed" => Some(Box::new(systems::KlimovShamirTransposed::default())),
        _ => None,
    }
}

/// Every permutation name [`permutation_by_name`] accepts.
pub const PERMUTATIONS: &[&str] = &[
    "counter",
    "chacha",
    "chacha64",
    "blake2b",
    "ascon",
    "xoshiro256++",
    "lcg",
    "splitmix-lanes",
    "klimov-shamir",
    "klimov-shamir-transposed",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_advertised_permutation_resolves() {
        for name in PERMUTATIONS {
            let p = permutation_by_name(name)
                .unwrap_or_else(|| panic!("advertised permutation {name} did not resolve"));
            assert_eq!(&p.name(), name);
            assert!(p.state_bytes() > 0);
            assert!(p.default_rounds() > 0);
        }
    }

    #[test]
    fn unknown_names_return_none_rather_than_panicking() {
        assert!(permutation_by_name("no-such-permutation").is_none());
    }
}
