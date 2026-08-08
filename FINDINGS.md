# A blind spot in the 2016 ChaCha/MCC rotation-constant search

**Status:** independently found, mechanism verified, and **confirmed by the paper's author, Dr. Rajeev Sobti**, who checked it against the published formula and supplied downstream context that corrected one of the conclusions (see the correction banner). His double-round correction has since been **independently reproduced on this instrument**. If you're relying on the metric described below, please read this before you cite it.

## TL;DR

> ## ⚠ CORRECTION — 2026-08-08. THE AUTHOR HAS RESPONDED.
>
> **Dr. Rajeev Sobti has confirmed the mechanism** and supplied context that
> **corrects consequence 3 below**. Two changes, both material:
>
> 1. **`l = 0` was NOT an unexamined artefact.** This document originally framed
>    it as a value "a blind metric has no reason to avoid". Per Dr. Sobti, the
>    top candidate sets were statistically tied (within ~0.5%, plausibly noise
>    from averaging only 1,000 trials per set), and `l = 0` was chosen
>    **deliberately from among the tied group because it lets the fourth rotation
>    be skipped entirely** — a costed efficiency decision the blind spot made
>    available, not an oversight it let slip through. Consequence 3 is rewritten.
> 2. **The blind spot is a property of the quarter round IN ISOLATION.** At the
>    **double round** — column then diagonal, how ChaCha actually runs — `l`
>    becomes visible again. **We have now independently verified this on our own
>    instrument; see "The double round" below.**
>
> The measurements in this document stand. The inference in consequence 3 did
> not, and has been replaced.
>
> **Dr. Sobti's positions below are our paraphrase of private correspondence,
> summarised in our own words and not yet checked back with him.** Any
> mischaracterisation is ours. His *mechanism* confirmation is the part we are
> most confident of; the surrounding context is reported as his account.

Sobti & Ganesan (2016) searched all 32⁴ possible rotation-constant combinations for the ChaCha quarter round and reported over 58,000 combinations that score better than ChaCha's own [16, 12, 8, 7], proposing an alternative design (MCC) with constants [4, 17, 8, 0]. While reproducing that search, we found that the diffusion metric they used cannot actually see the fourth rotation constant at all — rotating a value doesn't change the statistic being measured, so of the 32⁴ points supposedly searched, only 32³ are distinguishable to the metric. This doesn't mean the paper's numbers are wrong; it means the search space was smaller than reported **within a single quarter round**, and it is what made MCC's `l = 0` available as a free efficiency choice — a deliberate trade-off, per the author, not an oversight. At the **double round**, where ChaCha actually runs, `l` becomes visible again, and we have verified that directly.

## Background

The paper: Sobti, R. & Ganesan, G., *"Analysis of Quarter Rounds of Salsa and ChaCha Core and Proposal of an Alternative Design to Maximize Diffusion,"* Indian Journal of Science and Technology, Vol 9(3), Jan 2016. DOI: 10.17485/ijst/2016/v9i3/80087.

ChaCha's quarter round takes four 32-bit words and mixes them through a sequence of add/xor/rotate operations, ending with:

```
x1 = (x1 XOR x2) <<< l
```

where `l` is the fourth of four rotation constants (ChaCha's own values are 16, 12, 8, 7). The 2016 paper generates a diffusion matrix for each candidate set of four constants — measuring, across many single-bit input flips, how many output bits change — and scores each matrix by its mean and standard deviation. A *higher* mean with a *lower* standard deviation is treated as "better diffusion" — the paper's own wording is that a better set gives a mean that "is higher and standard deviation is lower than" ChaCha's. Searching all 32⁴ = 1,048,576 combinations of the four constants, the paper reports 58,000+ combinations beating ChaCha's own, and proposes MCC with constants [4, 17, 8, 0].

## Reproduction

We rebuilt this experiment independently as part of a broader PRNG measurement instrument (StateLab / statelab-crypto). Three of the paper's published diffusion values reproduced to within 0.32%, and the reported "58,000+ better combinations" figure reproduced closely (we found 90,112 in our own run of the full search, likely differing due to tie-handling or floating-point details we haven't pinned down yet — flagged, not glossed over).

## The finding

While sweeping the full 32⁴ space ourselves, we noticed something odd: varying the fourth rotation constant `l` alone, holding the other three fixed, never changed the diffusion matrix at all.

The reason is structural, not a bug in our reproduction: `l` is applied as the very last operation on `x1` in the quarter round, and nothing downstream reads or recombines `x1` again within that round. A rotation only permutes *which position* each bit sits in — it cannot change *how many* bits differ between two inputs (Hamming weight is rotation-invariant). Since the diffusion metric is computed from exactly that quantity (bit-flip counts across the output matrix), rotating the last word by any of the 32 possible amounts produces a **byte-identical** diffusion matrix, and therefore an identical score, every time.

We verified this directly: fixing the first three rotation constants and sweeping `l` across all 32 values gives 32 byte-for-byte identical matrices. Not similar — identical.

## What this means

The paper's search reports scoring 32⁴ combinations, but as far as this specific metric can tell, there are only 32³ *distinguishable* outcomes — the fourth dimension of the search doesn't move the score at all. Two consequences follow:

1. **The "58,000+ better combinations" figure is real, but each distinguishable result appears 32 times in it** — once per value of `l`, since all 32 look identical to the metric. Our own count is exactly divisible by 32 (90,112 = 2,816 × 32), which is what the invariance predicts and is independent arithmetic confirmation of it.
2. **The paper's own headline result names a constant the metric cannot determine.** Section 5.2 reports that "the rotation constants that generate the diffusion matrix with the highest mean is **[14, 24, 19, 11]**", with mean 7.0599 and standard deviation 3.5353. But `l = 11` is one of 32 tied values — under our implementation of their metric, all 32 of [14, 24, 19, `l`] produce an identical mean. The fourth constant of the paper's *maximum-diffusion* result is therefore not selected by the measurement that reports it. (Our own sweep independently landed on [15, 24, 19, \*] at mean 6.9698, against [14, 24, 19, \*] at 6.9667 — 0.04% apart, i.e. tied within noise, which is why the two searches differ in the first constant.)

3. **It made MCC's `l = 0` available as a free efficiency choice.** *(Rewritten 2026-08-08 following Dr. Sobti's correction — see the banner above.)* Because the metric cannot distinguish the 32 values of `l`, the top candidate sets were tied on it, and `l = 0` could be selected from that tied group at no measured cost while **removing the fourth rotation from the quarter round entirely**. Per Dr. Sobti that is exactly what happened, and it was deliberate. The blind spot did not cause a bad choice; **it made a specific efficiency trade-off invisible to the metric that was supposed to be arbitrating it.** That is a narrower claim than this document originally made, and it is the correct one.

   `l` is not inert everywhere, which is what makes the trade-off a trade-off: fixing the other three constants at the top-scoring set our own sweep found ([15, 24, 19, \*] — *not* MCC's) and varying `l` under a bit-level avalanche test — a different measurement entirely — changed rounds-to-full-avalanche for 14 of the 32 values.

## The double round — where `l` becomes visible again

Dr. Sobti's central correction is that the invariance belongs to the quarter round *in isolation*. At a **double round** (column round then diagonal round), the word that no downstream operation reads within its own quarter round becomes an input to a *different* quarter round in the second half — and **addition, unlike XOR, is sensitive to bit position**, so once carries propagate, `l`'s effect returns.

**We tested this rather than taking it on trust** (`examples/double_round_l.rs`), with both outcomes recorded before running:

| | matrices identical to `l = 0` | mean diffusion across `l` |
|---|---|---|
| **1 round** (isolated QRs), MCC [4,17,8,\*] | **32 / 32** | 1.7048 — spread **0.0000** |
| **1 round**, ChaCha [16,12,8,\*] | **32 / 32** | 1.6559 — spread **0.0000** |
| **2 rounds** (column→diagonal), MCC [4,17,8,\*] | **1 / 32** | 12.1052 – 12.1188 — spread **0.0137** |
| **2 rounds**, ChaCha [16,12,8,\*] | **1 / 32** | 11.3746 – 11.3918 — spread **0.0172** |

The single-round rows are the positive control and reproduce the original finding exactly. **The double-round rows confirm Dr. Sobti's correction: 31 of 32 matrices differ.** The same seed is used across each sweep, so these are exact paired differences, not sampling noise.

**One qualification the correspondence did not cover, and it cuts both ways.** The effect is real but *very small*: a spread of ~0.0137 on a mean of ~12.11 is **≈0.1%**. Dr. Sobti characterises the ~0.5% spread among the original paper's top candidates as *"probably just noise from only averaging 1000 random trials per set"*. **The double-round `l` effect we measure is roughly five times smaller than the spread he identifies as noise.** So `l` is not invisible at the double round — the mechanism is confirmed — but on this metric it is only *barely* visible, and selecting a value of `l` by double-round diffusion score means choosing on differences of about a tenth of a percent. Whether a search at that scale resolves signal depends entirely on its trial count, which we do not know for the follow-up work.

Best `l` by double-round mean in our run is 17 for MCC's other three constants and 26 for ChaCha's — neither is 0, which is consistent with Dr. Sobti's report that the ten best double-round sets all carry different nonzero `l`.

**Not verified here:** his specific figures — that ~1.7% of a million rotation sets beat [4, 17, 8, 0] on double-round diffusion, versus 24–28% for Salsa's and ChaCha's prescribed constants. Those come from the follow-up paper's own search and are cited to it, not to this run:

> Sobti, R. & Geetha, G., *"A Comparison of Diffusion Properties of Salsa, ChaCha, and MCC Core,"* in **Security in Computing and Communications (SSCC 2016)**, Communications in Computer and Information Science, Springer, pp. 87–98. DOI: [10.1007/978-981-10-2738-3_8](https://doi.org/10.1007/978-981-10-2738-3_8)

(Same second author as the 2016 quarter-round paper — published there as *Geetha Ganesan*, here as *G. Geetha*. One person, two name orderings.)

### The blind spot is width-free

The invariance argument never mentions word width: Hamming weight is
rotation-invariant at 64 bits exactly as at 32, and nothing reads `x1` again
inside the round either way. So the blind spot should carry to any word width a
design chooses. That is close to a prediction from proof rather than from
measurement, which makes it worth checking rather than assuming — the MCC core
was subsequently used at 64-bit width in *Cocktail* (Sobti, Bagga & Kaur, 2021,
DOI: [10.1109/ICRITO51393.2021.9596510](https://doi.org/10.1109/ICRITO51393.2021.9596510)),
whose abstract states it works on both 32- and 64-bit words.

Repeating the whole experiment at 64 bits (`examples/cocktail_64_l.rs`), with
BLAKE2b's 64-bit constants [32, 24, 16, 63] as a control — a deployed 64-bit ARX
design whose fourth constant is deliberately nonzero. **The set [52, 41, 16, 0] was
described to us in correspondence as Cocktail's; we have not checked it against the
2021 paper, and nothing below depends on it — the width-free result holds for any
constants, and the second set is BLAKE2b's, which is public.**

| | identical to `l = 0` | spread as % of mean |
|---|---|---|
| **1 round**, 64-bit, [52, 41, 16, \*] | **64 / 64** | **0.0000%** |
| **1 round**, 64-bit, BLAKE2b [32, 24, 16, \*] | **64 / 64** | **0.0000%** |
| **2 rounds**, 64-bit, [52, 41, 16, \*] | 1 / 64 | 0.2343% |
| **2 rounds**, 64-bit, BLAKE2b [32, 24, 16, \*] | 1 / 64 | 0.2104% |

**The blind spot is width-free, confirmed.** All 64 matrices identical at one
round, on both constant sets.

**And a prediction of ours failed, which is the more useful half.** We predicted
the double-round effect would stay at or below the 32-bit case's ~0.11%,
reasoning that a wider state diffuses further per round. It came in at
**0.21–0.23% — about twice as large, not smaller.** The qualification above
survives but is materially weaker at 64 bits: instead of being ~5× below the
~0.5% spread Dr. Sobti attributes to noise, the effect is only ~2× below it.
Same side of the line, much less margin. We were wrong about the direction, and
the reasoning behind the prediction was wrong too.

Neither design's own fourth constant is the double-round optimum on this metric
(best `l` was 61 and 41 respectively, against 0 and 63 as shipped) — consistent
with the ~0.1–0.2% effect sizes, at which "optimum" is not a meaningful
selection.

## What we're not claiming

- We are not claiming the paper's other conclusions (that many rotation-constant sets outperform ChaCha's on this metric, or that MCC's quarter round achieves more diffusion in fewer operations) are wrong — those are separate from this specific blind spot.
- ~~We haven't ruled out that the paper's own implementation constructed or scored the diffusion matrix slightly differently than our reproduction.~~ **Resolved 2026-08-08:** Dr. Sobti checked the mechanism against the published formula and confirmed it. This is no longer an open question.
- We are **not** claiming MCC's `l = 0` was a mistake. Per the correspondence above it was a deliberate, informed choice among statistically tied candidates, and the follow-up double-round work suggests it held up.
- This says nothing about whether any of these rotation constants are good or bad for actual cryptographic security — diffusion-matrix scoring and cryptanalytic security are different questions.

## Reproducing this yourself

The relevant code lives in `crates/statelab-crypto` (`qr_diffusion.rs` for the 2016 metric implementation, `avalanche.rs` for the independent bit-level avalanche test used as a cross-check). Fix any three rotation constants, sweep the fourth across 0–31, and diff the resulting matrices — they will match exactly.

For the double-round result, run `cargo run -p statelab-crypto --release --example double_round_l`. It prints the single-round control and the double-round test together, so the two regimes can be compared in one output. For the 64-bit replication, `--example cocktail_64_l`.

---
*If you're a maintainer of code that relies on this metric, or you're Dr. Sobti or Dr. Ganesan and think we've made an error somewhere, please open an issue — we'd genuinely like to be corrected if that's the case.*
