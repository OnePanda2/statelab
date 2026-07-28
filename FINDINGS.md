# A blind spot in the 2016 ChaCha/MCC rotation-constant search

**Status:** independently found, mechanism verified, author (Dr. Rajeev Sobti) contacted directly and awaiting response before this is treated as final. If you're relying on the metric described below, please read this before you cite it.

## TL;DR

Sobti & Ganesan (2016) searched all 32⁴ possible rotation-constant combinations for the ChaCha quarter round and reported over 58,000 combinations that score better than ChaCha's own [16, 12, 8, 7], proposing an alternative design (MCC) with constants [4, 17, 8, 0]. While reproducing that search, we found that the diffusion metric they used cannot actually see the fourth rotation constant at all — rotating a value doesn't change the statistic being measured, so of the 32⁴ points supposedly searched, only 32³ are distinguishable to the metric. This doesn't mean the paper's numbers are wrong; it means the search space was smaller than reported, and it plausibly explains why MCC's own proposed constant lands on the one value (`l = 0`) a blind metric has no reason to avoid.

## Background

The paper: Sobti, R. & Ganesan, G., *"Analysis of Quarter Rounds of Salsa and ChaCha Core and Proposal of an Alternative Design to Maximize Diffusion,"* Indian Journal of Science and Technology, Vol 9(3), Jan 2016. DOI: 10.17485/ijst/2016/v9i3/80087.

ChaCha's quarter round takes four 32-bit words and mixes them through a sequence of add/xor/rotate operations, ending with:

```
x1 = (x1 XOR x2) <<< l
```

where `l` is the fourth of four rotation constants (ChaCha's own values are 16, 12, 8, 7). The 2016 paper generates a diffusion matrix for each candidate set of four constants — measuring, across many single-bit input flips, how many output bits change — and scores each matrix by its mean and standard deviation. Lower/tighter values are treated as "better diffusion." Searching all 32⁴ = 1,048,576 combinations of the four constants, the paper reports 58,000+ combinations beating ChaCha's own, and proposes MCC with constants [4, 17, 8, 0].

## Reproduction

We rebuilt this experiment independently as part of a broader PRNG measurement instrument (StateLab / statelab-crypto). Three of the paper's published diffusion values reproduced to within 0.32%, and the reported "58,000+ better combinations" figure reproduced closely (we found 90,112 in our own run of the full search, likely differing due to tie-handling or floating-point details we haven't pinned down yet — flagged, not glossed over).

## The finding

While sweeping the full 32⁴ space ourselves, we noticed something odd: varying the fourth rotation constant `l` alone, holding the other three fixed, never changed the diffusion matrix at all.

The reason is structural, not a bug in our reproduction: `l` is applied as the very last operation on `x1` in the quarter round, and nothing downstream reads or recombines `x1` again within that round. A rotation only permutes *which position* each bit sits in — it cannot change *how many* bits differ between two inputs (Hamming weight is rotation-invariant). Since the diffusion metric is computed from exactly that quantity (bit-flip counts across the output matrix), rotating the last word by any of the 32 possible amounts produces a **byte-identical** diffusion matrix, and therefore an identical score, every time.

We verified this directly: fixing the first three rotation constants and sweeping `l` across all 32 values gives 32 byte-for-byte identical matrices. Not similar — identical.

## What this means

The paper's search reports scoring 32⁴ combinations, but as far as this specific metric can tell, there are only 32³ *distinguishable* outcomes — the fourth dimension of the search doesn't move the score at all. Two consequences follow:

1. **The "58,000+ better combinations" figure is real, but overcounts** — each distinguishable result is being counted 32 times over (once per value of `l`), since all 32 look identical to the metric.
2. **This plausibly explains MCC's choice of `l = 0`.** A metric that can't see `l` has no basis to prefer any value of it, so setting `l = 0` "costs" nothing on this particular measure — even though `l` is not inert everywhere. In our own testing, fixing the other three constants at MCC's values ([15, 24, 19, *]) and varying `l` under a bit-level avalanche test (a different measurement than the paper's diffusion matrix) changed rounds-to-full-avalanche for 14 of the 32 values — a property this diffusion metric cannot detect at all.

## What we're not claiming

- We are not claiming the paper's other conclusions (that many rotation-constant sets outperform ChaCha's on this metric, or that MCC's quarter round achieves more diffusion in fewer operations) are wrong — those are separate from this specific blind spot.
- We haven't ruled out that the paper's own implementation constructed or scored the diffusion matrix slightly differently than our reproduction, in a way that would restore sensitivity to `l`. We think this is unlikely given the mechanism above, but we're not certain, and we reached out to Dr. Sobti directly for exactly this reason before writing this up publicly.
- This says nothing about whether any of these rotation constants are good or bad for actual cryptographic security — diffusion-matrix scoring and cryptanalytic security are different questions.

## Reproducing this yourself

The relevant code lives in `crates/statelab-crypto` (`qr_diffusion.rs` for the 2016 metric implementation, `avalanche.rs` for the independent bit-level avalanche test used as a cross-check). Fix any three rotation constants, sweep the fourth across 0–31, and diff the resulting matrices — they will match exactly.

---
*If you're a maintainer of code that relies on this metric, or you're Dr. Sobti or Dr. Ganesan and think we've made an error somewhere, please open an issue — we'd genuinely like to be corrected if that's the case.*
