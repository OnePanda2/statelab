//! Philox and Threefry — the counter-based baselines (proposal §6.5).
//!
//! These are the natural comparators for **splittability**: both are stateless
//! keyed bijections `(counter, key) → output`, so independent streams come from
//! disjoint counter or key ranges rather than from a shared evolving state.
//! That is the property the N1–N4 battery was built to measure, and until now
//! there was nothing counter-based to point it at.
//!
//! Salmon, Moraes, Dror & Shaw, *Parallel Random Numbers: As Easy as 1, 2, 3*,
//! SC11. Reference implementation: `DEShawResearch/random123`.
//!
//! ## Everything here comes from the reference sources, not from memory
//!
//! Implementing a published primitive from recollection and then "verifying" it
//! against recalled test vectors is circular — it checks memory against itself.
//! So the round structures, the multipliers, the Weyl constants, the Threefry
//! rotation table and the key-schedule parity were all read out of
//! `include/Random123/philox.h` and `include/Random123/threefry.h`, and the
//! known-answer vectors come from `tests/kat_vectors` in the same repository.
//! The KAT is an independent check precisely because it originated outside this
//! codebase.
//!
//! ## A note on the `Permutation` trait
//!
//! These are deliberately **not** wired into [`crate::Permutation`] here. That
//! trait models a round-decomposable permutation where
//! `permute(s, n) == n × round(s, ·)`, and neither design satisfies it: Philox
//! bumps its key *between* rounds, and Threefry adds the key before round 0 and
//! re-injects it every four rounds. Forcing them in would make `round()` and
//! `permute()` disagree, which is exactly the sort of quiet inconsistency the
//! rest of this crate exists to catch. Registry entry is a separate decision.

/// Philox multipliers and Weyl increments, from `philox.h`.
const PHILOX_M4X32_0: u32 = 0xD251_1F53;
const PHILOX_M4X32_1: u32 = 0xCD9E_8D57;
const PHILOX_W32_0: u32 = 0x9E37_79B9; // golden ratio
const PHILOX_W32_1: u32 = 0xBB67_AE85; // sqrt(3) - 1
const PHILOX_M4X64_0: u64 = 0xD2E7_470E_E14C_6C93;
const PHILOX_M4X64_1: u64 = 0xCA5A_8263_9512_1157;
const PHILOX_W64_0: u64 = 0x9E37_79B9_7F4A_7C15;
const PHILOX_W64_1: u64 = 0xBB67_AE85_84CA_A73B;

/// Threefry key-schedule parity, `SKEIN_KS_PARITY64` from `threefry.h`.
const SKEIN_KS_PARITY64: u64 = 0x1BD1_1BDA_A9FC_1A22;

/// Threefry-4×64 rotation table, `R_64x4_*` from `threefry.h`. These are the
/// `R_256` constants of the Threefish reference sources.
const R_64X4: [(u32, u32); 8] = [
    (14, 16),
    (52, 57),
    (23, 40),
    (5, 37),
    (25, 33),
    (46, 12),
    (58, 22),
    (32, 32),
];

/// Philox-4×32 at `rounds` rounds. `rounds <= 16`.
///
/// The key is bumped *before* every round after the first, matching
/// `philox4x32_R`: round 1 uses the raw key, and each later round uses a key
/// advanced by the Weyl constants.
pub fn philox4x32(ctr: [u32; 4], key: [u32; 2], rounds: usize) -> [u32; 4] {
    assert!(rounds <= 16, "Philox is defined for at most 16 rounds");
    let (mut c, mut k) = (ctr, key);
    for r in 0..rounds {
        if r > 0 {
            k[0] = k[0].wrapping_add(PHILOX_W32_0);
            k[1] = k[1].wrapping_add(PHILOX_W32_1);
        }
        let p0 = u64::from(PHILOX_M4X32_0) * u64::from(c[0]);
        let p1 = u64::from(PHILOX_M4X32_1) * u64::from(c[2]);
        let (hi0, lo0) = ((p0 >> 32) as u32, p0 as u32);
        let (hi1, lo1) = ((p1 >> 32) as u32, p1 as u32);
        c = [hi1 ^ c[1] ^ k[0], lo1, hi0 ^ c[3] ^ k[1], lo0];
    }
    c
}

/// Philox-4×64 at `rounds` rounds. `rounds <= 16`.
pub fn philox4x64(ctr: [u64; 4], key: [u64; 2], rounds: usize) -> [u64; 4] {
    assert!(rounds <= 16, "Philox is defined for at most 16 rounds");
    let (mut c, mut k) = (ctr, key);
    for r in 0..rounds {
        if r > 0 {
            k[0] = k[0].wrapping_add(PHILOX_W64_0);
            k[1] = k[1].wrapping_add(PHILOX_W64_1);
        }
        let p0 = u128::from(PHILOX_M4X64_0) * u128::from(c[0]);
        let p1 = u128::from(PHILOX_M4X64_1) * u128::from(c[2]);
        let (hi0, lo0) = ((p0 >> 64) as u64, p0 as u64);
        let (hi1, lo1) = ((p1 >> 64) as u64, p1 as u64);
        c = [hi1 ^ c[1] ^ k[0], lo1, hi0 ^ c[3] ^ k[1], lo0];
    }
    c
}

/// Threefry-4×64 at `rounds` rounds. `rounds <= 72`.
///
/// Key schedule: `ks[4] = parity ⊕ k0 ⊕ k1 ⊕ k2 ⊕ k3`, the state is seeded with
/// `X[i] = ctr[i] + ks[i]`, and after every fourth round the key is re-injected
/// with a rotating offset and a round-group counter added to `X3`.
pub fn threefry4x64(ctr: [u64; 4], key: [u64; 4], rounds: usize) -> [u64; 4] {
    assert!(rounds <= 72, "Threefry-4x64 is defined for at most 72 rounds");
    let mut ks = [key[0], key[1], key[2], key[3], SKEIN_KS_PARITY64];
    for k in &key {
        ks[4] ^= k;
    }
    let mut x = [
        ctr[0].wrapping_add(ks[0]),
        ctr[1].wrapping_add(ks[1]),
        ctr[2].wrapping_add(ks[2]),
        ctr[3].wrapping_add(ks[3]),
    ];

    for r in 0..rounds {
        let (ra, rb) = R_64X4[r % 8];
        if r % 2 == 0 {
            x[0] = x[0].wrapping_add(x[1]);
            x[1] = x[1].rotate_left(ra);
            x[1] ^= x[0];
            x[2] = x[2].wrapping_add(x[3]);
            x[3] = x[3].rotate_left(rb);
            x[3] ^= x[2];
        } else {
            x[0] = x[0].wrapping_add(x[3]);
            x[3] = x[3].rotate_left(ra);
            x[3] ^= x[0];
            x[2] = x[2].wrapping_add(x[1]);
            x[1] = x[1].rotate_left(rb);
            x[1] ^= x[2];
        }
        // InjectKey after every fourth round, with the group number j.
        if r % 4 == 3 {
            let j = r / 4 + 1;
            for i in 0..4 {
                x[i] = x[i].wrapping_add(ks[(j + i) % 5]);
            }
            x[3] = x[3].wrapping_add(j as u64);
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    // Known-answer vectors, transcribed from `tests/kat_vectors` in
    // DEShawResearch/random123 (fetched 2026-08-01, 8885 bytes). The file's own
    // header notes the third case is the leading hex digits of pi.

    #[test]
    fn philox4x32_matches_published_vectors() {
        // R = 7
        assert_eq!(
            philox4x32([0, 0, 0, 0], [0, 0], 7),
            [0x5f6f_b709, 0x0d89_3f64, 0x4f12_1f81, 0x4f73_0a48]
        );
        assert_eq!(
            philox4x32([u32::MAX; 4], [u32::MAX; 2], 7),
            [0x5207_ddc2, 0x4516_5e59, 0x4d8e_e751, 0x8c52_f662]
        );
        assert_eq!(
            philox4x32(
                [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
                [0xa409_3822, 0x299f_31d0],
                7
            ),
            [0x4dfc_caba, 0x190a_87f0, 0xc473_62ba, 0xb6b5_242a]
        );
        // R = 10, the shipped configuration
        assert_eq!(
            philox4x32([0, 0, 0, 0], [0, 0], 10),
            [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8]
        );
        assert_eq!(
            philox4x32([u32::MAX; 4], [u32::MAX; 2], 10),
            [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd]
        );
        assert_eq!(
            philox4x32(
                [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
                [0xa409_3822, 0x299f_31d0],
                10
            ),
            [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1]
        );
    }

    #[test]
    fn philox4x64_matches_published_vectors() {
        assert_eq!(
            philox4x64([0; 4], [0; 2], 7),
            [
                0x5dc8_ee62_68ec_62cd,
                0x139b_c570_b6c1_25a0,
                0x84d6_deb4_fb65_f49e,
                0xaff7_5833_76d3_78c2
            ]
        );
        assert_eq!(
            philox4x64([0; 4], [0; 2], 10),
            [
                0x1655_4d9e_ca36_314c,
                0xdb20_fe9d_672d_0fdc,
                0xd7e7_72ce_e186_176b,
                0x7e68_b68a_ec7b_a23b
            ]
        );
        assert_eq!(
            philox4x64([u64::MAX; 4], [u64::MAX; 2], 10),
            [
                0x87b0_92c3_013f_e90b,
                0x438c_3c67_be8d_0224,
                0x9cc7_d7c6_9cd7_77b6,
                0xa09c_aebf_594f_0ba0
            ]
        );
        assert_eq!(
            philox4x64(
                [
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89
                ],
                [0x4528_21e6_38d0_1377, 0xbe54_66cf_34e9_0c6c],
                10
            ),
            [
                0xa528_f454_03e6_1d95,
                0x38c7_2dbd_566e_9788,
                0xa5a1_610e_72fd_18b5,
                0x57bd_43b5_e52b_7fe6
            ]
        );
    }

    #[test]
    fn threefry4x64_matches_published_vectors() {
        // R = 13
        assert_eq!(
            threefry4x64([0; 4], [0; 4], 13),
            [
                0x4071_fabe_e1dc_8e05,
                0x02ed_3113_695c_9c62,
                0x3973_11b5_b89f_9d49,
                0xe212_92c3_2580_24bc
            ]
        );
        // R = 20, the shipped configuration
        assert_eq!(
            threefry4x64([0; 4], [0; 4], 20),
            [
                0x0921_8ebd_e6c8_5537,
                0x5594_1f52_66d8_6105,
                0x4bd2_5e16_2824_34dc,
                0xee29_ec84_6bd2_e40b
            ]
        );
        assert_eq!(
            threefry4x64(
                [
                    0x243f_6a88_85a3_08d3,
                    0x1319_8a2e_0370_7344,
                    0xa409_3822_299f_31d0,
                    0x082e_fa98_ec4e_6c89
                ],
                [
                    0x4528_21e6_38d0_1377,
                    0xbe54_66cf_34e9_0c6c,
                    0xbe54_66cf_34e9_0c6c,
                    0xc0ac_29b7_c97c_50dd
                ],
                20
            ),
            [
                0xa7e8_fde5_9165_1bd9,
                0xbaaf_d0c3_0138_319b,
                0x84a5_c1a7_29e6_85b9,
                0x901d_406c_cebc_1ba4
            ]
        );
        // R = 72, the maximum. Exercises the key-injection rotation well past
        // one full cycle of the 5-element schedule.
        assert_eq!(
            threefry4x64([0; 4], [0; 4], 72),
            [
                0x94ee_ea8b_1f2a_da84,
                0xadf1_0331_3eae_6670,
                0x9524_19a1_f4b1_6d53,
                0xd83f_13e6_3c9f_6b11
            ]
        );
        assert_eq!(
            threefry4x64([u64::MAX; 4], [u64::MAX; 4], 72),
            [
                0x1151_8c03_4bc1_ff4c,
                0x193f_10b8_bcdc_c9f7,
                0xd024_229c_b58f_20d8,
                0x563e_d6e4_8e05_183f
            ]
        );
    }

    /// Both are bijections in the counter for a fixed key — the property that
    /// makes them counter-based, and the reason disjoint counter ranges give
    /// independent streams. Checked by injectivity over a sample rather than
    /// asserted.
    #[test]
    fn distinct_counters_give_distinct_outputs() {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for i in 0..4096u64 {
            assert!(seen.insert(threefry4x64([i, 0, 0, 0], [1, 2, 3, 4], 20)));
        }
        let mut seen = HashSet::new();
        for i in 0..4096u64 {
            assert!(seen.insert(philox4x64([i, 0, 0, 0], [1, 2], 10)));
        }
    }

    /// Round count must actually matter — guards against a loop that silently
    /// runs a fixed number of rounds regardless of the argument.
    #[test]
    fn round_count_changes_the_output() {
        assert_ne!(
            philox4x32([1, 2, 3, 4], [5, 6], 7),
            philox4x32([1, 2, 3, 4], [5, 6], 10)
        );
        assert_ne!(
            threefry4x64([1, 2, 3, 4], [5, 6, 7, 8], 13),
            threefry4x64([1, 2, 3, 4], [5, 6, 7, 8], 20)
        );
    }
}
