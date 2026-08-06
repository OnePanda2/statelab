//! PRIOR ART — Nager 2024 (IACR ePrint 2024/103), measured.
//!
//! H4 (64-bit word width) was retired on this paper. Only the ABSTRACT had ever
//! been read; the PDF was 403'd. The full text is now in hand, and it publishes
//! a complete C listing of the permutation, which makes the design directly
//! measurable on this instrument.
//!
//! *** VERIFICATION STATUS — READ BEFORE CITING ANY NUMBER BELOW. ***
//! The paper publishes NO test vectors. None. So this transcription cannot be
//! KAT-verified, and the project's standing rule (implementing from a source
//! and verifying against the same source is circular) cannot be satisfied here
//! by any amount of care. What IS true: the round function, rotation constants
//! (43 and 17), state layout, quarter-round call order and round count (24) are
//! transcribed character by character from the paper's own §2 listing, not from
//! recollection. Every number this driver prints is therefore conditional on
//! the transcription being right, and it is NOT independently confirmed.
//!
//! One transcription note, flagged rather than silently resolved: the paper's
//! final feed-forward reads
//!     a^=a0; b^=b0; c^=c0; d^=d0;
//!     e^=e0; f^=f0; h^=h0;
//! — seven XORs for eight words. `g^=g0` is absent. That may be a typo in the
//! paper or a real asymmetry. It does not affect the permutation measured here
//! (the feed-forward is outside the round function), but it is recorded because
//! anyone implementing from this paper will hit it.
//!
//! ```text
//! cargo run -p statelab-crypto --release --example prior_art_nager
//! ```

use statelab_crypto::avalanche::{recommended_samples, rounds_to_avalanche};
use statelab_crypto::permutation_by_name;
use statelab_crypto::Permutation;

/// Nager 2024 §2, verbatim structure: eight 64-bit words, four quarter rounds
/// per round, rotation constants 43 and 17, 24 rounds.
///
/// ```c
/// #define QR(a,b,c,d){ a+=b; d^=c; rot(43,a); c+=a; b^=d; rot(17,c); }
/// for (i=0;i<ROUNDS;i++){
///     QR(a,b,c,d); QR(e,f,g,h); QR(a,b,e,f); QR(c,d,g,h);
/// }
/// ```
struct Nager64;

#[inline]
fn qr(w: &mut [u64; 8], ia: usize, ib: usize, ic: usize, id: usize) {
    // a += b
    w[ia] = w[ia].wrapping_add(w[ib]);
    // d ^= c
    w[id] ^= w[ic];
    // rot(43, a)
    w[ia] = w[ia].rotate_left(43);
    // c += a
    w[ic] = w[ic].wrapping_add(w[ia]);
    // b ^= d
    w[ib] ^= w[id];
    // rot(17, c)
    w[ic] = w[ic].rotate_left(17);
}

impl Permutation for Nager64 {
    fn name(&self) -> &'static str {
        "nager64"
    }
    fn state_bytes(&self) -> usize {
        64 // eight 64-bit words = 512 bits, same state size as ChaCha
    }
    fn default_rounds(&self) -> usize {
        24
    }
    fn round(&self, state: &mut [u8], _round_index: usize) {
        let mut w = [0u64; 8];
        for (i, wi) in w.iter_mut().enumerate() {
            *wi = u64::from_le_bytes(state[i * 8..i * 8 + 8].try_into().unwrap());
        }
        // a b c d e f g h  =  0 1 2 3 4 5 6 7
        qr(&mut w, 0, 1, 2, 3); // QR(a,b,c,d)
        qr(&mut w, 4, 5, 6, 7); // QR(e,f,g,h)
        qr(&mut w, 0, 1, 4, 5); // QR(a,b,e,f)
        qr(&mut w, 2, 3, 6, 7); // QR(c,d,g,h)
        for (i, wi) in w.iter().enumerate() {
            state[i * 8..i * 8 + 8].copy_from_slice(&wi.to_le_bytes());
        }
    }
}

/// Which of the eight words ever receives an addition, a rotation, an XOR.
///
/// Derived from the call order, not from the measurement, so it is a claim
/// about the published listing that the avalanche numbers can then corroborate
/// or contradict.
fn operation_exposure() {
    // Slot roles inside QR(a,b,c,d):
    //   slot a: += and rotate      slot b: ^= only
    //   slot c: += and rotate      slot d: ^= only
    let names = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let calls = [(0, 1, 2, 3), (4, 5, 6, 7), (0, 1, 4, 5), (2, 3, 6, 7)];
    let mut add = [false; 8];
    let mut rot = [false; 8];
    let mut xor = [false; 8];
    for (a, b, c, d) in calls {
        add[a] = true;
        rot[a] = true;
        add[c] = true;
        rot[c] = true;
        xor[b] = true;
        xor[d] = true;
    }
    println!("-- Operation exposure per state word, from the published call order --");
    println!("   (MCC's stated principle, and the one this project re-derived as");
    println!("   `wide-cross`: every word should see Addition, XOR and Rotation.)\n");
    println!("   word   add   rot   xor");
    for i in 0..8 {
        println!(
            "   {:<6} {:<5} {:<5} {}",
            names[i],
            if add[i] { "yes" } else { "NO" },
            if rot[i] { "yes" } else { "NO" },
            if xor[i] { "yes" } else { "no" }
        );
    }
    let never_rotated: Vec<&str> = (0..8).filter(|&i| !rot[i]).map(|i| names[i]).collect();
    println!(
        "\n   Never rotated, never added, in any round: {:?}",
        never_rotated
    );
    println!("   Note `h` is the block counter (h = ++h0 in the listing).\n");
}

fn main() {
    println!("PRIOR ART — Nager 2024, ePrint 2024/103, measured on the instrument");
    println!("Transcribed from the paper's §2 C listing. NO published test vectors");
    println!("exist, so this is UNVERIFIED against any independent answer.\n");

    operation_exposure();

    // Sanity: the round must actually depend on the state and be invertible-ish
    // in the weak sense that distinct inputs give distinct outputs.
    let mut s1 = [0u8; 64];
    let mut s2 = [0u8; 64];
    s2[0] = 1;
    Nager64.permute(&mut s1, 24);
    Nager64.permute(&mut s2, 24);
    assert_ne!(s1, s2, "distinct inputs collapsed");
    let mut s3 = [7u8; 64];
    let mut s4 = [7u8; 64];
    Nager64.permute(&mut s3, 4);
    Nager64.permute(&mut s4, 8);
    assert_ne!(s3, s4, "round count ignored");
    println!("   structural sanity: distinct inputs differ, round count is honoured.\n");

    let tolerance = 0.12;
    let bits = 512;
    let samples = recommended_samples(bits, tolerance);
    println!("-- Rounds to avalanche, tolerance {tolerance}, {samples} samples --");
    println!("   Samples from recommended_samples() — methodological item (1).");
    println!("   An avalanche number without an adequacy check is suspect.\n");

    for name in ["chacha", "chacha64", "blake2b"] {
        let p = permutation_by_name(name).expect("registered");
        let sweep = rounds_to_avalanche(p.as_ref(), 12, samples, tolerance, 1);
        println!(
            "   {:<10} first full avalanche: {:?}",
            name, sweep.rounds_to_avalanche
        );
        for (r, maxd, _mean, dead) in sweep.per_round.iter().take(8) {
            println!("      r{r:<3} max_dev {maxd:.4}  dead {dead:.4}");
        }
    }

    let sweep = rounds_to_avalanche(&Nager64, 12, samples, tolerance, 1);
    println!(
        "\n   {:<10} first full avalanche: {:?}",
        "nager64", sweep.rounds_to_avalanche
    );
    for (r, maxd, _mean, dead) in sweep.per_round.iter().take(8) {
        println!("      r{r:<3} max_dev {maxd:.4}  dead {dead:.4}");
    }

    println!("\n-- Multi-seed check, methodological item (10) --");
    println!("   Single seed is a default violation, not a hardening step.\n");
    for seed in [1u64, 2, 3, 4, 5] {
        let s = rounds_to_avalanche(&Nager64, 12, samples, tolerance, seed);
        let c = rounds_to_avalanche(
            permutation_by_name("chacha").unwrap().as_ref(),
            12,
            samples,
            tolerance,
            seed,
        );
        println!(
            "   seed {seed}:  nager64 -> {:?}   chacha -> {:?}",
            s.rounds_to_avalanche, c.rounds_to_avalanche
        );
    }
}
