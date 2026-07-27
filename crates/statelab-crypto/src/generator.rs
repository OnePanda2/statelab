//! Forward-secure PRNG constructions, for the H1-narrow measurement.
//!
//! Phase 0 falsified H1 as originally stated. The claim was that batching
//! output trades away forward secrecy, and that nobody had designed out the
//! tradeoff. Bernstein's fast-key-erasure construction (2017) did exactly that,
//! and the Linux kernel adopted it in 2022.
//!
//! What survives is a quantitative question this module answers: **how much
//! did fast key erasure already recover?** If the answer is "nearly all of it",
//! the slack H1 hoped to exploit is gone. That is measurable on any CPU, needs
//! no hardware acceleration, and does not depend on the blocked half of
//! Phase 2.
//!
//! Both constructions here are built on the same ChaCha20 block function, so
//! the comparison isolates the *mode* rather than the primitive.

/// Produces random bytes with forward secrecy.
pub trait ForwardSecureRng {
    fn name(&self) -> &'static str;
    /// Fills `out`, after which no earlier output is recoverable from the
    /// generator's state.
    fn fill(&mut self, out: &mut [u8]);
    /// ChaCha20 block invocations so far — the work metric, independent of the
    /// machine this happens to run on.
    fn blocks_used(&self) -> u64;
}

// ---------------------------------------------------------------------------
// ChaCha20 block function
// ---------------------------------------------------------------------------

const CHACHA_CONSTANTS: [u32; 4] = [0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574];

#[inline]
fn quarter_round(s: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(16);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(12);
    s[a] = s[a].wrapping_add(s[b]);
    s[d] = (s[d] ^ s[a]).rotate_left(8);
    s[c] = s[c].wrapping_add(s[d]);
    s[b] = (s[b] ^ s[c]).rotate_left(7);
}

/// The full ChaCha20 block function: 20 rounds plus the feed-forward addition.
///
/// Unlike [`crate::systems::ChaCha`], which is the bare permutation the
/// diffusion batteries measure, this is the real keystream generator.
pub fn chacha20_block(key: &[u8; 32], counter: u32, nonce: &[u8; 12], out: &mut [u8; 64]) {
    let mut state = [0u32; 16];
    state[..4].copy_from_slice(&CHACHA_CONSTANTS);
    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes(key[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
    }
    state[12] = counter;
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes(nonce[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
    }

    let mut w = state;
    for _ in 0..10 {
        quarter_round(&mut w, 0, 4, 8, 12);
        quarter_round(&mut w, 1, 5, 9, 13);
        quarter_round(&mut w, 2, 6, 10, 14);
        quarter_round(&mut w, 3, 7, 11, 15);
        quarter_round(&mut w, 0, 5, 10, 15);
        quarter_round(&mut w, 1, 6, 11, 12);
        quarter_round(&mut w, 2, 7, 8, 13);
        quarter_round(&mut w, 3, 4, 9, 14);
    }
    for i in 0..16 {
        out[i * 4..i * 4 + 4].copy_from_slice(&w[i].wrapping_add(state[i]).to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Naive rekey-per-request — the NIST DRBG shape
// ---------------------------------------------------------------------------

/// Generates output, then performs an extra block invocation to rekey.
///
/// This is the shape NIST's DRBGs use for backtracking resistance, and the
/// construction Bernstein argues is unnecessary. Its cost floor is two block
/// calls per request regardless of how few bytes were asked for — one for the
/// output, one for the ratchet.
pub struct NaiveRekeyRng {
    key: [u8; 32],
    blocks: u64,
}

impl NaiveRekeyRng {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key, blocks: 0 }
    }
}

impl ForwardSecureRng for NaiveRekeyRng {
    fn name(&self) -> &'static str {
        "naive-rekey (NIST DRBG shape)"
    }

    fn fill(&mut self, out: &mut [u8]) {
        let nonce = [0u8; 12];
        let mut block = [0u8; 64];
        let mut counter: u32 = 1;

        for chunk in out.chunks_mut(64) {
            chacha20_block(&self.key, counter, &nonce, &mut block);
            self.blocks += 1;
            counter = counter.wrapping_add(1);
            let n = chunk.len();
            chunk.copy_from_slice(&block[..n]);
        }

        // The extra step: ratchet the key so earlier output is unrecoverable.
        chacha20_block(&self.key, 0, &nonce, &mut block);
        self.blocks += 1;
        self.key.copy_from_slice(&block[..32]);
        block.fill(0);
    }

    fn blocks_used(&self) -> u64 {
        self.blocks
    }
}

// ---------------------------------------------------------------------------
// Fast key erasure — Bernstein 2017, in the Linux kernel since 2022
// ---------------------------------------------------------------------------

/// Refills a buffer whose first 32 bytes become the next key and are erased
/// immediately; the rest is output, erased as it is consumed.
///
/// Forward secrecy is structural rather than an extra step, so a small request
/// costs a fraction of a block call once the refill is amortised. This is the
/// construction that removed the tradeoff H1 was built on.
pub struct FastKeyErasureRng {
    key: [u8; 32],
    buffer: Vec<u8>,
    /// Next unconsumed byte. Starts at `buffer.len()` so the first call refills.
    pos: usize,
    blocks: u64,
}

impl FastKeyErasureRng {
    /// `buffer_bytes` is rounded up to a multiple of 64. Larger buffers amortise
    /// harder; the Linux implementation uses a per-CPU buffer of this shape.
    pub fn new(key: [u8; 32], buffer_bytes: usize) -> Self {
        let n = buffer_bytes.div_ceil(64) * 64;
        assert!(n >= 64, "buffer must hold at least one block");
        Self {
            key,
            buffer: vec![0u8; n],
            pos: n,
            blocks: 0,
        }
    }

    fn refill(&mut self) {
        let nonce = [0u8; 12];
        let mut block = [0u8; 64];
        for (i, out) in self.buffer.chunks_mut(64).enumerate() {
            chacha20_block(&self.key, i as u32, &nonce, &mut block);
            self.blocks += 1;
            out.copy_from_slice(&block);
        }
        block.fill(0);

        // The leading 32 bytes become the next key and are erased from the
        // buffer, so they can never be handed out as output.
        self.key.copy_from_slice(&self.buffer[..32]);
        self.buffer[..32].fill(0);
        self.pos = 32;
    }
}

impl ForwardSecureRng for FastKeyErasureRng {
    fn name(&self) -> &'static str {
        "fast-key-erasure (DJB 2017 / Linux 2022)"
    }

    fn fill(&mut self, out: &mut [u8]) {
        let mut written = 0;
        while written < out.len() {
            if self.pos == self.buffer.len() {
                self.refill();
            }
            let take = (out.len() - written).min(self.buffer.len() - self.pos);
            out[written..written + take].copy_from_slice(&self.buffer[self.pos..self.pos + take]);
            // Erase on consumption: this is what makes the secrecy forward.
            self.buffer[self.pos..self.pos + take].fill(0);
            self.pos += take;
            written += take;
        }
    }

    fn blocks_used(&self) -> u64 {
        self.blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 8439 §2.3.2. Without this the benchmark could be comparing two
    /// modes built on something that is not ChaCha20.
    #[test]
    fn chacha20_block_matches_rfc8439() {
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = i as u8;
        }
        let nonce = [
            0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x4a, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut out = [0u8; 64];
        chacha20_block(&key, 1, &nonce, &mut out);

        let expected: [u8; 64] = [
            0x10, 0xf1, 0xe7, 0xe4, 0xd1, 0x3b, 0x59, 0x15, 0x50, 0x0f, 0xdd, 0x1f, 0xa3, 0x20,
            0x71, 0xc4, 0xc7, 0xd1, 0xf4, 0xc7, 0x33, 0xc0, 0x68, 0x03, 0x04, 0x22, 0xaa, 0x9a,
            0xc3, 0xd4, 0x6c, 0x4e, 0xd2, 0x82, 0x64, 0x46, 0x07, 0x9f, 0xaa, 0x09, 0x14, 0xc2,
            0xd7, 0x05, 0xd9, 0x8b, 0x02, 0xa2, 0xb5, 0x12, 0x9c, 0xd1, 0xde, 0x16, 0x4e, 0xb9,
            0xcb, 0xd0, 0x83, 0xe8, 0xa2, 0x50, 0x3c, 0x4e,
        ];
        assert_eq!(out, expected);
    }

    #[test]
    fn both_generators_produce_the_requested_length() {
        for len in [1usize, 16, 32, 64, 100, 1000] {
            let mut a = NaiveRekeyRng::new([7u8; 32]);
            let mut b = FastKeyErasureRng::new([7u8; 32], 1024);
            let mut xa = vec![0u8; len];
            let mut xb = vec![0u8; len];
            a.fill(&mut xa);
            b.fill(&mut xb);
            assert_eq!(xa.len(), len);
            assert_eq!(xb.len(), len);
        }
    }

    /// Successive requests must not repeat. A generator that returns the same
    /// bytes twice would benchmark beautifully and be worthless.
    #[test]
    fn successive_requests_differ() {
        let mut a = NaiveRekeyRng::new([1u8; 32]);
        let (mut p, mut q) = ([0u8; 32], [0u8; 32]);
        a.fill(&mut p);
        a.fill(&mut q);
        assert_ne!(p, q);

        let mut b = FastKeyErasureRng::new([1u8; 32], 1024);
        b.fill(&mut p);
        b.fill(&mut q);
        assert_ne!(p, q);
    }

    /// Output must never include the bytes reserved as the next key.
    #[test]
    fn fast_key_erasure_never_emits_its_own_next_key() {
        let mut g = FastKeyErasureRng::new([3u8; 32], 256);
        let mut first = [0u8; 32];
        g.fill(&mut first);
        // After refill the key came from buffer[..32], which is zeroed and
        // never handed out; the first output starts at offset 32.
        assert_ne!(first, g.key);
    }

    /// The work metric, in block calls, which is what the mode comparison is
    /// really about.
    #[test]
    fn block_accounting_shows_the_amortisation() {
        // Naive: one block for a 32-byte request, plus one to rekey.
        let mut naive = NaiveRekeyRng::new([0u8; 32]);
        let mut out = [0u8; 32];
        naive.fill(&mut out);
        assert_eq!(naive.blocks_used(), 2);

        // Fast key erasure with a 1024-byte buffer: 16 blocks per refill,
        // then 31 requests of 32 bytes served from it (992 usable bytes).
        let mut fke = FastKeyErasureRng::new([0u8; 32], 1024);
        for _ in 0..31 {
            fke.fill(&mut out);
        }
        assert_eq!(fke.blocks_used(), 16, "one refill should serve 31 requests");

        // So over 31 small requests: 62 block calls versus 16.
        let mut naive31 = NaiveRekeyRng::new([0u8; 32]);
        for _ in 0..31 {
            naive31.fill(&mut out);
        }
        assert_eq!(naive31.blocks_used(), 62);
    }
}
