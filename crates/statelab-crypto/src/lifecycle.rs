//! Lifecycle harness — proposal §9.4, the fork/snapshot half.
//!
//! `PHASE_I_SEEDING_GAP_ANALYSIS.md` audited the existing battery against the
//! two failures §3.7 names and found it would have reported clean on both. The
//! four failure classes collapse to two modes:
//!
//!   1. **the seed carries too little entropy** — Debian CVE-2008-0166, boot
//!      starvation. Nothing in this crate can represent it. Not addressed here.
//!   2. **the same seed reaches two places it should not** — Android
//!      SecureRandom 2013, `fork(2)`, VM snapshot restore. **This module.**
//!
//! The audit's finding for mode 2 was specific and is what this module is
//! built on: **the detector already exists and is validated; the trigger does
//! not.** N1's identical-seed control and N2's eight-identical-streams positive
//! control both fire decisively, so the crate can already tell that two streams
//! are the same. What it has never had is anything that decides *which* streams
//! a real deployment would create and then compares those. That is the gap
//! here.
//!
//! ## What "fork" means in this module, stated before any result
//!
//! **`fork(2)` is modelled, not called.** [`LifecycleRng::snapshot`] copies the
//! generator's state exactly, which is what duplicating a process image does to
//! its memory. That models the *mechanism* faithfully — but it means this
//! harness tests whether a **construction** is fork-safe, and **not** whether a
//! given **implementation** correctly wires up its reseed hook on a real fork.
//! Answering the second needs actual process spawning, which a portable
//! zero-dependency crate cannot do, and this module must never be cited as
//! having answered it.
//!
//! Three copy paths are modelled separately, because conflating them is what
//! made the first version of this module wrong:
//!
//! | | memory copied | platform wipe | entropy delivered |
//! |---|---|---|---|
//! | [`LifecycleRng::snapshot`] — VM restore | yes | **no** | **no** |
//! | [`LifecycleRng::fork_child`] — `fork(2)` | yes | if supported | — |
//! | [`LifecycleRng::on_fork`] — `pthread_atfork` | — | — | yes |
//!
//! ## What a pass means
//!
//! Only that no two instances emitted a common 32-byte window. It says nothing
//! about output quality — that is what the rest of the batteries are for — and
//! nothing whatever about security. Phase 7 is untouched by anything here.

use crate::generator::{FastKeyErasureRng, ForwardSecureRng, NaiveRekeyRng};
use std::collections::HashMap;

/// Window size for collision detection. At 32 bytes a chance collision has
/// probability about 2^-256, so any hit is structural rather than statistical
/// and no significance machinery is needed.
const WINDOW: usize = 32;

// ---------------------------------------------------------------------------
// The lifecycle abstraction
// ---------------------------------------------------------------------------

/// A generator that can be put through a process lifecycle.
pub trait LifecycleRng {
    fn name(&self) -> &'static str;

    fn fill(&mut self, out: &mut [u8]);

    /// A raw copy of the process image — a **VM snapshot**.
    ///
    /// Nothing is wiped and nothing is notified, because from inside the guest
    /// nothing happened. Implementations must copy faithfully: a construction
    /// that "passes" by refusing to copy itself models a snapshot that does not
    /// exist.
    fn snapshot(&self) -> Box<dyn LifecycleRng>;

    /// A copy produced by `fork(2)`, which the **platform may annotate**.
    ///
    /// Defaults to [`LifecycleRng::snapshot`], because that is what almost every
    /// construction gets. Overriding it means claiming platform support such as
    /// Linux's `MADV_WIPEONFORK`, and the override should be as visible as the
    /// claim.
    fn fork_child(&self) -> Box<dyn LifecycleRng> {
        self.snapshot()
    }

    /// Explicit notification that a fork happened, carrying entropy drawn from
    /// **outside the process image** — `getrandom(2)`, modelled.
    ///
    /// **The default is to do nothing, because that is what almost every
    /// deployed construction actually does.**
    ///
    /// The entropy must come from outside: two siblings have byte-identical
    /// memory, so *any* function of their own state gives them the same answer.
    /// That is not a detail of this harness, it is the reason fork-safety
    /// cannot be solved in userspace alone.
    fn on_fork(&mut self, _fresh: &[u8; 32]) {}
}

// ---------------------------------------------------------------------------
// Constructions under test
// ---------------------------------------------------------------------------

impl LifecycleRng for NaiveRekeyRng {
    fn name(&self) -> &'static str {
        "naive-rekey"
    }
    fn fill(&mut self, out: &mut [u8]) {
        ForwardSecureRng::fill(self, out)
    }
    fn snapshot(&self) -> Box<dyn LifecycleRng> {
        Box::new(self.clone())
    }
}

impl LifecycleRng for FastKeyErasureRng {
    fn name(&self) -> &'static str {
        "fast-key-erasure"
    }
    fn fill(&mut self, out: &mut [u8]) {
        ForwardSecureRng::fill(self, out)
    }
    fn snapshot(&self) -> Box<dyn LifecycleRng> {
        Box::new(self.clone())
    }
}

/// Fast key erasure plus a fork marker the platform destroys on fork.
///
/// This models Linux's `MADV_WIPEONFORK`: a page that is zeroed in the child
/// but not the parent. Before emitting, a zeroed marker means "I am a child
/// that has not reseeded yet", and the generator reseeds from fresh entropy.
///
/// **The safety comes from the platform, not from the construction.** There is
/// no pure-userspace state a child can inspect to learn it was forked — its
/// memory is by definition identical to its parent's. That is why §3.7 calls
/// fork reuse "genuinely unsolved at the architecture level", and modelling it
/// here should not be mistaken for solving it.
#[derive(Clone)]
pub struct WipeOnForkRng {
    inner: FastKeyErasureRng,
    /// Non-zero in a live instance; zeroed by the platform in a forked child.
    /// A VM snapshot does **not** zero it — that asymmetry is the whole point.
    marker: [u8; 8],
    /// Entropy delivered from outside the process image by [`Self::on_fork`].
    /// `None` means the child knows it was forked but has nothing to reseed
    /// *from*, which is a real state and is not silently treated as safe.
    fresh: Option<[u8; 32]>,
    reseeds: u64,
    wiped_without_entropy: bool,
}

impl WipeOnForkRng {
    pub fn new(key: [u8; 32], buffer_bytes: usize) -> Self {
        Self {
            inner: FastKeyErasureRng::new(key, buffer_bytes),
            marker: [1u8; 8],
            fresh: None,
            reseeds: 0,
            wiped_without_entropy: false,
        }
    }
    pub fn reseeds(&self) -> u64 {
        self.reseeds
    }
    /// True if this instance discovered it was forked but had no entropy to
    /// reseed from. The platform wipe is **necessary but not sufficient**.
    pub fn wiped_without_entropy(&self) -> bool {
        self.wiped_without_entropy
    }
    fn reseed_if_wiped(&mut self) {
        if self.marker != [0u8; 8] {
            return;
        }
        match self.fresh.take() {
            Some(fresh) => {
                self.inner = FastKeyErasureRng::new(fresh, 256);
                self.marker = [1u8; 8];
                self.reseeds += 1;
            }
            // Nothing to reseed from. A real implementation blocks on
            // getrandom(2); modelling that as "carry on" is the pessimistic
            // choice and keeps the resulting collision visible.
            None => self.wiped_without_entropy = true,
        }
    }
}

impl LifecycleRng for WipeOnForkRng {
    fn name(&self) -> &'static str {
        "fast-key-erasure + wipe-on-fork"
    }
    fn fill(&mut self, out: &mut [u8]) {
        self.reseed_if_wiped();
        ForwardSecureRng::fill(&mut self.inner, out)
    }
    fn snapshot(&self) -> Box<dyn LifecycleRng> {
        // A VM snapshot copies the marker intact. The guest cannot tell.
        Box::new(self.clone())
    }
    fn fork_child(&self) -> Box<dyn LifecycleRng> {
        let mut copy = self.clone();
        copy.marker = [0u8; 8];
        copy.fresh = None;
        Box::new(copy)
    }
    fn on_fork(&mut self, fresh: &[u8; 32]) {
        self.fresh = Some(*fresh);
    }
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

/// One labelled output stream produced during a scenario.
pub struct Stream {
    pub label: String,
    pub bytes: Vec<u8>,
}

/// A pair of instances that emitted identical output.
#[derive(Debug, Clone)]
pub struct Collision {
    pub a: String,
    pub b: String,
    /// Distinct 32-byte windows the two share.
    pub shared_windows: usize,
}

/// Outcome of running one scenario against one construction.
#[derive(Debug, Clone)]
pub struct LifecycleReport {
    pub scenario: &'static str,
    pub construction: &'static str,
    pub instances: usize,
    pub total_bytes: usize,
    pub collisions: Vec<Collision>,
}

impl LifecycleReport {
    /// No two instances emitted a common window.
    ///
    /// This is the *only* thing a pass asserts. See the module header.
    pub fn is_safe(&self) -> bool {
        self.collisions.is_empty()
    }
}

/// Every 32-byte window at every offset, so an unaligned overlap is caught too.
///
/// Aligned blocks would be enough for the fork case, where a child resumes at
/// exactly the parent's position — but assuming alignment is assuming the
/// answer, and a snapshot restored mid-buffer need not be aligned.
fn windows_of(bytes: &[u8]) -> Vec<[u8; WINDOW]> {
    if bytes.len() < WINDOW {
        return Vec::new();
    }
    (0..=bytes.len() - WINDOW)
        .map(|i| {
            let mut w = [0u8; WINDOW];
            w.copy_from_slice(&bytes[i..i + WINDOW]);
            w
        })
        .collect()
}

/// The collision oracle. Independent of how the streams were produced.
pub fn detect_collisions(streams: &[Stream]) -> Vec<Collision> {
    let mut index: HashMap<[u8; WINDOW], Vec<usize>> = HashMap::new();
    for (i, s) in streams.iter().enumerate() {
        for w in windows_of(&s.bytes) {
            let entry = index.entry(w).or_default();
            if entry.last() != Some(&i) {
                entry.push(i);
            }
        }
    }

    let mut pairs: HashMap<(usize, usize), usize> = HashMap::new();
    for owners in index.values() {
        for (x, &a) in owners.iter().enumerate() {
            for &b in owners.iter().skip(x + 1) {
                *pairs.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
    }

    let mut out: Vec<Collision> = pairs
        .into_iter()
        .map(|((a, b), n)| Collision {
            a: streams[a].label.clone(),
            b: streams[b].label.clone(),
            shared_windows: n,
        })
        .collect();
    out.sort_by(|x, y| (&x.a, &x.b).cmp(&(&y.a, &y.b)));
    out
}

/// Builds a construction from a 32-byte key.
pub type Build = fn([u8; 32]) -> Box<dyn LifecycleRng>;

pub fn build_naive(key: [u8; 32]) -> Box<dyn LifecycleRng> {
    Box::new(NaiveRekeyRng::new(key))
}
pub fn build_fke(key: [u8; 32]) -> Box<dyn LifecycleRng> {
    Box::new(FastKeyErasureRng::new(key, 256))
}
pub fn build_wipe_on_fork(key: [u8; 32]) -> Box<dyn LifecycleRng> {
    Box::new(WipeOnForkRng::new(key, 256))
}

fn key_from(seed: u64) -> [u8; 32] {
    let mut probe = crate::avalanche::Probe::new(seed);
    let mut k = [0u8; 32];
    probe.fill(&mut k);
    k
}

const EMIT: usize = 256;

/// The scenarios a deployment actually produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scenario {
    /// **Negative control.** Two processes, two distinct full-entropy seeds.
    /// Must not collide, or the harness is broken.
    DistinctSeeds,
    /// **Positive control.** Two processes handed the same seed — the Android
    /// SecureRandom shape. Must collide, or the detector is not firing.
    SameSeed,
    /// Parent emits, then forks. Both continue emitting.
    ForkAfterEmit,
    /// Parent forks immediately after seeding, before any output.
    ForkBeforeEmit,
    /// Parent forks twice: the sibling case, not just parent/child.
    ///
    /// Siblings have byte-identical memory, so any construction that derives
    /// its "fresh" entropy from its own state gives both children the same
    /// answer. This scenario exists because that bug was written and caught
    /// here — see `PHASE_J_LIFECYCLE.md`.
    ForkTwice,
    /// `fork(2)` where the platform wipe fires but **no entropy is delivered**
    /// — a raw `fork` syscall with no `pthread_atfork` handler registered.
    /// Isolates whether the platform mechanism alone is sufficient.
    ForkWithoutEntropy,
    /// State captured, parent continues, snapshot later resumed — a VM restore.
    ///
    /// **No fork occurs**, so no platform wipe and no notification. From inside
    /// the guest, nothing happened.
    SnapshotRestore,
}

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Scenario::DistinctSeeds => "distinct-seeds (negative control)",
            Scenario::SameSeed => "same-seed (positive control)",
            Scenario::ForkAfterEmit => "fork after emit",
            Scenario::ForkBeforeEmit => "fork before emit",
            Scenario::ForkTwice => "fork twice (siblings)",
            Scenario::ForkWithoutEntropy => "fork, platform wipe, no entropy",
            Scenario::SnapshotRestore => "snapshot restore (VM)",
        }
    }

    /// Whether a *correct* construction should collide here. Both controls have
    /// a known answer; the real scenarios are the open questions.
    pub fn expected_collision(self) -> Option<bool> {
        match self {
            Scenario::DistinctSeeds => Some(false),
            Scenario::SameSeed => Some(true),
            _ => None,
        }
    }
}

/// Runs one scenario against one construction.
pub fn run(scenario: Scenario, construction: Build, seed: u64) -> LifecycleReport {
    let mut streams: Vec<Stream> = Vec::new();
    let mut emit = |rng: &mut Box<dyn LifecycleRng>, label: &str| {
        let mut buf = vec![0u8; EMIT];
        rng.fill(&mut buf);
        streams.push(Stream {
            label: label.to_string(),
            bytes: buf,
        });
    };

    // The kernel, modelled: an entropy source OUTSIDE every process image, so
    // two siblings drawing from it get different answers. Seeded separately
    // from the construction's key so a scenario stays reproducible.
    let mut kernel = crate::avalanche::Probe::new(seed ^ 0x5EED_0000_0000_0000);
    let mut fresh_entropy = move || {
        let mut e = [0u8; 32];
        kernel.fill(&mut e);
        e
    };

    /// `fork(2)`: a platform-annotated copy, then the atfork handler delivers
    /// entropy the child could not have derived from its own memory.
    fn forked(parent: &dyn LifecycleRng, fresh: [u8; 32]) -> Box<dyn LifecycleRng> {
        let mut child = parent.fork_child();
        child.on_fork(&fresh);
        child
    }

    let name = construction(key_from(seed)).name();

    match scenario {
        Scenario::DistinctSeeds => {
            let mut a = construction(key_from(seed));
            let mut b = construction(key_from(seed ^ 0xFFFF_FFFF));
            emit(&mut a, "process A");
            emit(&mut b, "process B");
        }
        Scenario::SameSeed => {
            let mut a = construction(key_from(seed));
            let mut b = construction(key_from(seed));
            emit(&mut a, "process A");
            emit(&mut b, "process B");
        }
        Scenario::ForkAfterEmit => {
            let mut parent = construction(key_from(seed));
            emit(&mut parent, "parent (pre-fork)");
            let mut child = forked(parent.as_ref(), fresh_entropy());
            emit(&mut parent, "parent (post-fork)");
            emit(&mut child, "child");
        }
        Scenario::ForkBeforeEmit => {
            let mut parent = construction(key_from(seed));
            let mut child = forked(parent.as_ref(), fresh_entropy());
            emit(&mut parent, "parent");
            emit(&mut child, "child");
        }
        Scenario::ForkTwice => {
            let mut parent = construction(key_from(seed));
            let mut c1 = forked(parent.as_ref(), fresh_entropy());
            let mut c2 = forked(parent.as_ref(), fresh_entropy());
            emit(&mut parent, "parent");
            emit(&mut c1, "child 1");
            emit(&mut c2, "child 2");
        }
        Scenario::ForkWithoutEntropy => {
            // fork_child() fires the platform wipe; on_fork is never called.
            let mut parent = construction(key_from(seed));
            let mut c1 = parent.fork_child();
            let mut c2 = parent.fork_child();
            emit(&mut parent, "parent");
            emit(&mut c1, "child 1");
            emit(&mut c2, "child 2");
        }
        Scenario::SnapshotRestore => {
            // snapshot(), NOT fork_child(): no wipe, no notification.
            let mut vm = construction(key_from(seed));
            emit(&mut vm, "vm (pre-snapshot)");
            let mut restored = vm.snapshot();
            emit(&mut vm, "vm (continued)");
            emit(&mut restored, "vm (restored from snapshot)");
        }
    }

    let total_bytes = streams.iter().map(|s| s.bytes.len()).sum();
    LifecycleReport {
        scenario: scenario.name(),
        construction: name,
        instances: streams.len(),
        total_bytes,
        collisions: detect_collisions(&streams),
    }
}

pub const SCENARIOS: [Scenario; 7] = [
    Scenario::DistinctSeeds,
    Scenario::SameSeed,
    Scenario::ForkAfterEmit,
    Scenario::ForkBeforeEmit,
    Scenario::ForkTwice,
    Scenario::ForkWithoutEntropy,
    Scenario::SnapshotRestore,
];

pub const CONSTRUCTIONS: [(&str, Build); 3] = [
    ("naive-rekey", build_naive as Build),
    ("fast-key-erasure", build_fke as Build),
    ("fke + wipe-on-fork", build_wipe_on_fork as Build),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oracle_finds_a_planted_identical_stream() {
        let streams = vec![
            Stream {
                label: "a".into(),
                bytes: vec![7u8; 128],
            },
            Stream {
                label: "b".into(),
                bytes: vec![7u8; 128],
            },
        ];
        let c = detect_collisions(&streams);
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].a.as_str(), c[0].b.as_str()), ("a", "b"));
    }

    #[test]
    fn the_oracle_is_silent_on_distinct_streams() {
        let a: Vec<u8> = (0..128u16).map(|i| i as u8).collect();
        let b: Vec<u8> = (0..128u16).map(|i| (i as u8) ^ 0xFF).collect();
        let streams = vec![
            Stream {
                label: "a".into(),
                bytes: a,
            },
            Stream {
                label: "b".into(),
                bytes: b,
            },
        ];
        assert!(detect_collisions(&streams).is_empty());
    }

    #[test]
    fn the_oracle_catches_an_unaligned_overlap() {
        // A shared run that starts at a non-multiple-of-32 offset in one stream.
        // Aligned block comparison would miss this; every-offset windows do not.
        let shared: Vec<u8> = (0..64u16).map(|i| (i as u8).wrapping_mul(37)).collect();
        let mut a = vec![0xAAu8; 7];
        a.extend_from_slice(&shared);
        let mut b = vec![0xBBu8; 19];
        b.extend_from_slice(&shared);
        let streams = vec![
            Stream {
                label: "a".into(),
                bytes: a,
            },
            Stream {
                label: "b".into(),
                bytes: b,
            },
        ];
        assert_eq!(detect_collisions(&streams).len(), 1);
    }

    #[test]
    fn both_controls_behave_for_every_construction() {
        // If either control is wrong, no verdict below it means anything.
        for (label, build) in CONSTRUCTIONS {
            let neg = run(Scenario::DistinctSeeds, build, 1);
            assert!(
                neg.is_safe(),
                "{label}: distinct seeds must not collide, got {:?}",
                neg.collisions
            );
            let pos = run(Scenario::SameSeed, build, 1);
            assert!(
                !pos.is_safe(),
                "{label}: identical seeds MUST collide — detector is not firing"
            );
        }
    }

    #[test]
    fn forward_secrecy_does_not_imply_fork_safety() {
        // The substantive claim this module was built to test. Fast key erasure
        // gives forward secrecy; a forked child inherits the same key and the
        // same unconsumed buffer, so it re-emits its parent's output exactly.
        for scenario in [
            Scenario::ForkAfterEmit,
            Scenario::ForkBeforeEmit,
            Scenario::ForkTwice,
            Scenario::SnapshotRestore,
        ] {
            let r = run(scenario, build_fke, 42);
            assert!(
                !r.is_safe(),
                "fast-key-erasure unexpectedly survived {}",
                scenario.name()
            );
            let r = run(scenario, build_naive, 42);
            assert!(
                !r.is_safe(),
                "naive-rekey unexpectedly survived {}",
                scenario.name()
            );
        }
    }

    #[test]
    fn platform_support_plus_outside_entropy_fixes_forking() {
        // Both halves are required. Note ForkTwice: siblings only diverge
        // because the entropy came from outside their (identical) memory.
        for scenario in [
            Scenario::ForkAfterEmit,
            Scenario::ForkBeforeEmit,
            Scenario::ForkTwice,
        ] {
            let r = run(scenario, build_wipe_on_fork, 42);
            assert!(
                r.is_safe(),
                "wipe-on-fork failed {}: {:?}",
                scenario.name(),
                r.collisions
            );
        }
    }

    #[test]
    fn the_platform_wipe_alone_is_not_sufficient() {
        // The wipe fires, but no atfork handler delivers entropy. The child
        // knows it forked and has nothing to reseed from. Siblings collide.
        let r = run(Scenario::ForkWithoutEntropy, build_wipe_on_fork, 42);
        assert!(
            !r.is_safe(),
            "a platform wipe with no entropy source must NOT read as safe"
        );
    }

    #[test]
    fn nothing_survives_a_vm_snapshot() {
        // No fork occurs, so no wipe and no notification: from inside the guest
        // nothing happened. This is §3.7's "genuinely unsolved at the
        // architecture level", reproduced rather than asserted.
        for (label, build) in CONSTRUCTIONS {
            let r = run(Scenario::SnapshotRestore, build, 42);
            assert!(
                !r.is_safe(),
                "{label} claimed to survive a VM snapshot — check the model, \
                 not the construction"
            );
        }
    }

    #[test]
    fn multi_seed_the_headline_claim() {
        // Item (10): single seed is a default violation, not a hardening step.
        for seed in [1u64, 2, 3, 12345, 0xDEAD_BEEF] {
            assert!(!run(Scenario::ForkAfterEmit, build_fke, seed).is_safe());
            assert!(run(Scenario::ForkAfterEmit, build_wipe_on_fork, seed).is_safe());
        }
    }

    #[test]
    fn snapshot_really_snapshots() {
        // Guards against a construction "passing" by refusing to copy itself,
        // which would model a snapshot that does not exist. Checked for every
        // construction, including the one that passes the fork scenarios.
        for (label, build) in CONSTRUCTIONS {
            let mut a = build(key_from(9));
            let mut b = a.snapshot();
            let (mut x, mut y) = ([0u8; 64], [0u8; 64]);
            a.fill(&mut x);
            b.fill(&mut y);
            assert_eq!(x, y, "{label}: snapshot() must copy state exactly");
        }
    }
}
