# What a round of ChaCha actually costs, and why we stopped

**Status:** complete and closed. This is a negative result about our own project
goal, published because the measurements underneath it are useful to other
people and the negative result is useful to us.

## TL;DR

We spent seven months trying to build a PRNG that beats ChaCha20 on ordinary
hardware — no AES-NI, no AVX-512, nothing newer than SSE2. **That goal was
already met before we started.** ChaCha8 is published, shipping in
`rand_chacha` today, and measures **2.22× faster than ChaCha20** on our
reference machine. There is no design work between "we want this" and "we have
this."

Three things we measured on the way are worth having anyway:

1. **The commonly quoted "ChaCha8 is 2.5× faster" figure is the round ratio
   20 ÷ 8, not a measurement.** It assumes a round is the only thing a block
   costs. It isn't — there is a fixed per-block cost that doesn't scale with
   rounds, and it eats about 11% of the claimed speedup.
2. **Byte-aligned rotation constants buy nothing in scalar code** — under 1%
   across constant sets that a SIMD instruction-count model separates by 25% in
   *both* directions. The SIMD cost model is not a portable cost model.
3. **Our own measurement harness was inflating every cost figure we had by
   2.35×**, and we didn't notice for months, because every design went through
   the same harness so every *relative* number stayed self-consistent.

Nothing here is a security claim. See "What this is not" at the bottom — it
matters more than usual for this document.

## The measurements

Intel Core i5-750 (Lynnfield, 2009), 2.67 GHz, SSE2 + SSSE3 only. Scalar
implementation, no intrinsics, rotations as compile-time constants. Cost figures
are cycles/byte from a paired TSC and wall-clock harness, timed in rotated
batteries with per-condition run-to-run spread reported as an explicit noise
floor.

### Round count against quality and cost

Three independent statistical batteries and cost, ChaCha, 11 round counts:

| rounds | avalanche max_dev | BIC max\|r\| (floor 0.0848) | GF(2) rank z | cyc/B |
|---|---|---|---|---|
| 2 | 0.5000 | 1.0000 | −6.37 | 1.654 |
| 3 | 0.1886 | 0.9584 | −3.72 | 2.082 |
| **4** | 0.0836 | 0.0863 | +0.69 | 2.434 |
| 6 | 0.0802 | 0.0899 | −0.19 | 3.148 |
| **8** | 0.0755 | 0.0859 | −0.86 | 3.945 |
| **12** | 0.0802 | 0.0821 | +0.91 | 5.580 |
| **20** | 0.0744 | 0.0815 | +0.69 | 8.756 |

**All three batteries saturate together at 4 rounds.** The transition is the
3→4 jump — BIC falls 0.9584 → 0.0863, rank moves −3.72 → +0.69, avalanche
0.1886 → 0.0836. Below 4, ChaCha is visibly broken on every measure we have.
At 4 and above, all three sit at their noise floors and stay there through 20.

So **the entire gap from 4 rounds to 20 is cryptanalytic margin with no
statistical component at all.** Statistical test batteries have nothing to say
about ChaCha's round count above 4. We say this as the people who built the
batteries.

### The fixed cost nobody subtracts

Least squares over all eleven points:

```
cost(rounds) = 0.395 × rounds + 0.828   cycles/byte
R² = 0.9996, max residual 0.098, strictly monotonic
```

That **0.828 cyc/B intercept** is state setup and the feed-forward addition —
work every block does once regardless of round count. It is **9.5% of ChaCha20
but 21.0% of ChaCha8**, so the fewer rounds you run, the more it dominates.

Which is why the quoted speedups don't hold:

| | round ratio (quoted) | measured | shortfall |
|---|---|---|---|
| ChaCha12 vs ChaCha20 | 1.67× | **1.57×** | 6% of the claimed gain |
| ChaCha8 vs ChaCha20 | 2.5× | **2.22×** | 11% of the claimed gain |

**The quoted numbers are 20/12 and 20/8.** They are exactly correct if a block
costs nothing but rounds. Ours is the same cipher, same machine, same compiler,
one variable changed — so the gap is the intercept and nothing else.

**Read the mechanism, not our number.** The 2.22× is one machine's figure and
will move with architecture, compiler and implementation. What generalises is
that *there is an intercept and it is not small at low round counts*. Divide
your own two measurements; don't divide round counts. (We learned this the hard
way in the other direction — we once predicted an effect size would transfer
across word widths because its mechanism did. It came back twice as large.)

**On precision.** Two independent harnesses in this repo measure 20-round
ChaCha at 8.756 and 8.323 cyc/B — a **5% disagreement between our own runs**,
and per-condition run-to-run spread reaches 5.8%. So treat the absolute figures
as ≈2 significant figures. The *ratios* above are tighter than that because
every one comes from a single run in which all conditions were measured in
rotated order — **within-run comparisons are paired, between-run comparisons
carry the noise.** That is also why we do not quote a speedup by dividing
numbers from two different tables.

### Byte alignment is a SIMD property, not a cost property

A rotation by a multiple of 8 is one `vpshufb`; anything else is shift + shift +
or. That model says an all-byte-aligned quarter round costs 12 instructions
against ChaCha's 16 — a 25% cut. We searched for such quarter rounds and found
them.

In scalar code, `ROL r32, imm8` on x86 and `ROR` with an immediate on ARM are
**one instruction at any rotation amount**. There is no penalty to avoid, so
there is no bonus to collect:

| constants | byte-aligned | cyc/B | vs ChaCha | SIMD model predicts |
|---|---|---|---|---|
| ChaCha [16,12,8,7] | 2 of 4 | 8.683 | — | — |
| all-aligned [16,24,8,16] | 4 of 4 | 8.751 | +0.79% | **−25%** |
| none-aligned [15,13,11,7] | 0 of 4 | 8.671 | −0.13% | **+25%** |
| MCC [4,17,8,0] *(control)* | 1 of 4 | **7.984** | **−8.05%** | −6% |

The three non-control conditions span 0.92%, against a model that separates
them by 25% in both directions — and that span is well *below* this machine's
own 5.80% run-to-run noise. The difference isn't small; it isn't resolvable.

**MCC is the control and it's what makes that null readable.** Its `l = 0`
doesn't make a rotation cheaper, it *removes the instruction*, and that shows
up at −8.05%, far outside that noise. So the harness resolves
one instruction per quarter round fine. It just has nothing to resolve on the
other three.

The transferable version: **eliminating an operation is portable; making one
cheaper on one instruction set is not.**

### Our harness was wrong by 2.35×

Our measurement trait takes state as `&mut [u8]` so the batteries can address
individual bits without knowing a design's word size. The consequence is that
each design **loads its entire state from bytes and stores it back every
round** — 40 extra state round-trips per ChaCha block versus a real
implementation, which loads once, runs 20 rounds in registers, and stores once.

| path | cyc/B | |
|---|---|---|
| real ChaCha20 block function | **8.323** | |
| same 20 rounds through our trait | 19.543 | 2.35× |
| load/store only, **no arithmetic at all** | 12.289 | 1.48× |

**62.9% of the trait path is memory traffic with no arithmetic in it.** The
control — which does nothing but copy 16 words back and forth — costs 1.5× the
entire real block function.

Every published cost figure this project had was that inflated number. We
didn't catch it for months, and the reason is worth stating: **every design went
through the same harness, so every relative comparison stayed self-consistent,
and self-consistency is exactly what a uniform overhead cannot disturb.** What
exposed it was a number from outside the project — what a scalar ChaCha20 is
known to cost. A closed measurement system cannot audit its own units.

The diffusion results were unaffected — rounds-to-avalanche is a property of a
permutation, not of how often you copy the state to measure it. The cost figures
were not.

## Why we stopped

Our goal was "a PRNG that performs better than ChaCha20 on almost any device,
with no special hardware required." ChaCha8 satisfies every clause of that
sentence: 2.22× faster as measured here, pure scalar ARX, runs on anything with
a 32-bit adder, published since 2008, and shipping in `rand_chacha` as
`ChaCha8Rng`. `rand`'s own `StdRng` is ChaCha12. Google's Adiantum is ChaCha12.
BLAKE3 cut to 7 rounds.

We also searched the structural alternatives and found nothing:

| we tried | result |
|---|---|
| rotation constants | exhaustively searched by others in 2016; no round reduction |
| wiring / topology | 3 searches. ChaCha is diameter-optimal, top 0.33%. **Zero rounds bought** |
| quarter-round structure | 4 candidates found; all 4 killed by bit-independence testing |
| eliminating an operation | **−6.8%.** Real, portable, and an order of magnitude short |

Three independent structural searches returning nothing is itself evidence about
how well-optimised ChaCha's parameters already are.

The honest summary is that we were trying to make a *round* cheaper, and the
field's answer to "make ChaCha faster" was never to change the round — it was to
run fewer of them. That answer was published in 2019 and shipped by 2020.

## What this is not

**This is not a recommendation to use ChaCha8.** We measured speed. Round-count
selection is a cryptanalytic question and we do no cryptanalysis. For reference,
the published attack frontier on ChaCha256 runs 6 rounds at 2⁹⁹·⁴⁸, 7 rounds at
2²¹⁸·⁹², and nothing at 20 — **and that frontier moves.** The 6-round figure
came down substantially over the preceding years. Any round count justified by
"current attacks don't reach here" is justified by a quantity that has been
observed to change.

**Statistical saturation is a lower bound on rounds and never becomes anything
else.** The sharpest demonstration is in our own table: ChaCha at 7 rounds is
broken in the literature and passes every battery here comfortably. Our results
can rule round counts *out*. They cannot rule any *in*. Anyone reading the
4-round saturation as permission has read it backwards.

## Reproducing this

```
cargo run -p statelab-crypto --release --example round_budget
cargo run -p statelab-crypto --release --example harness_tax
cargo run -p statelab-crypto --release --example scalar_rotation_cost
```

Each prints its predictions — including what a *failed* prediction would mean —
before its results. Two of the predictions behind this document failed, and both
failures are in the output rather than edited out of it.

## References

- Aumasson, J.-P. *Too Much Crypto.* [ePrint 2019/1492](https://eprint.iacr.org/2019/1492)
- Dey, S., Garai, H.K. & Maitra, S. *Cryptanalysis of Reduced Round ChaCha — New
  Attack and Deeper Analysis.* ToSC 2023. [ePrint 2023/134](https://eprint.iacr.org/2023/134)
- `rand` issue [#932](https://github.com/rust-random/rand/issues/932) — `StdRng` → ChaCha12
- Sobti, R. & Ganesan, G. IJST 9(3), 2016. DOI [10.17485/ijst/2016/v9i3/80087](https://doi.org/10.17485/ijst/2016/v9i3/80087) — see also [`FINDINGS.md`](./FINDINGS.md)

---
*If you think we've made an error here, please open an issue. Two of the results
above exist because a control caught us making one.*
