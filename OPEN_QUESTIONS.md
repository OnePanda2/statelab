# Open Questions

Questions that require a human decision because the frozen architecture
(`PROJECT_BRIEF.md`) does not answer them, or because answering them would
contradict it. Per the Ambiguity Protocol (§2.3) these are reported, never
silently resolved.

Status legend: **OPEN** (awaiting sign-off) · **RESOLVED** (decision recorded).

---

## OQ-1 — `average_decline`: the cited addendum is not in this repository — **OPEN**

**Raised:** 2026-07-26, during the post-audit remediation pass.
**Blocks:** the audit's Task 1.
**Nothing was changed.** The formula is untouched pending a decision.

### The request

The remediation brief states that `PROJECT_BRIEF.md`'s addendum defines:

> Average Decline = mean(Current / Next), computed over every transition where
> Next Value < Current Value.

and that `average_decline` must therefore be **2.0** for n = 3, not the 0.5 the
code currently produces.

### Why this could not be actioned

**The cited addendum does not exist in this repository.** Every `.md` and `.txt`
file was searched. What `PROJECT_BRIEF.md` actually says is the opposite, in two
independent places:

- **§4.4, metric table (line 352):**
  > `Average Decline` | Mean of `next / current` taken only over even (decline)
  > transitions — **always exactly `0.5`** for classic Collatz, since even
  > transitions are always `n / 2`.

- **Appendix B, worked n = 3 example (line 656):**
  > `Average Decline` | **0.5** (exact, always, for classic Collatz)

- Appendix B's full JSON literal also carries `"average_decline": 0.5`.

The current implementation matches both. Changing it to `current / next` would
make the implementation **contradict the frozen architecture document**, which
`Document 1.txt` §6 ("Authority of Documents") forbids outright:

> Implementation details must never contradict higher-level documents.

Both definitions are internally coherent — for an exact halving, `next/current`
is 0.5 and `current/next` is 2.0. This is purely a question of *which one the
spec mandates*, and the only spec text available in this repo mandates 0.5.

Related searches, all returning **zero** hits repo-wide, suggesting the auditor
worked from a spec revision that was never committed here:

| Cited by the audit | Occurrences in repo |
|---|---|
| `Average Decline = mean(Current / Next)` | 0 (repo says `next / current`) |
| default `max_iterations` = 10,000,000 | 0 |
| Coral "Line Width" / "Center Offset" / "Animation Speed" | 0 (§5.5 lists six parameters) |
| Comparison feature "Trajectory Length" | 0 |

### Options

**(a) Commit the addendum, then apply the change.** If a spec revision defining
`Average Decline = Current / Next` exists outside this repo, add it to
`PROJECT_BRIEF.md` as a dated addendum section. The code change is then a
one-line edit in `growth_decline()` plus three test updates, and is
*mathematically trivially verifiable*: every Classic Collatz even step is an
exact halving, so `average_decline` becomes exactly `2.0` for any trajectory
with at least one even transition. **Recommended if the addendum is real** —
say the word and this takes minutes.

**(b) Keep `next / current` (current behaviour).** The ratio direction is then
uniform across both metrics (`average_growth` and `average_decline` both read
"next relative to current"), which is arguably the more consistent reading and
is what the committed spec says. No code change; close this question.

**(c) Report both.** Add a second metric (e.g. `average_decline_inverse`) so
both conventions are available. Additive and non-breaking under §4.9, but adds
a metric the frozen spec never named.

### Recommendation

**(a) if the addendum can be produced, otherwise (b).** The blocker is purely
evidentiary — the change itself is small and safe. What is *not* safe is
silently making the code disagree with the only specification present.

---

## OQ-2 — Fixed-point-at-start: a system already satisfying its own termination rule — **OPEN**

**Raised:** 2026-07-26, during the post-audit remediation pass.
**Blocks:** the audit's Task 4.
**Nothing was changed.** `engine.rs`'s loop order is untouched.

### The issue

The FROZEN Trajectory Generation Order (§4.1) is:

```
1. Generate next state           (system.transition)
2. Check convergence             (system.is_terminated)
3. Check cycle detection         (engine-generic)
4. Check iteration limit         (engine-generic)
5. Continue
```

Step 1 always precedes step 2. So a system whose **initial** state already
satisfies its own termination rule cannot report that immediately — it is forced
to transition at least once and travel all the way back before it is allowed to
notice.

For Classic Collatz this means `n = 1` produces a forced three-transition round
trip `1 → 4 → 2 → 1` with `iteration_count = 3`, rather than an immediate
"already converged, 0 transitions" result.

This is **not** Collatz-specific. Any future `DeterministicSystem` whose initial
state can be a fixed point, or can already satisfy its termination condition,
hits the same behaviour. It has never been explicitly decided whether that is
correct — it is currently a *consequence* of the frozen order rather than a
*decision*.

### Options

**(a) Add a Step 0 — check `is_terminated()` on the initial state before any
transition.** If satisfied, the trajectory is a single-state, zero-iteration
`Converged` result. This changes Classic Collatz `n = 1` from
`iteration_count = 3` to `iteration_count = 0`, and changes `state_sequence`
from `["1","4","2","1"]` to `["1"]`.

This is a **genuine change to the frozen §4.1 generation order** and requires a
spec addendum plus human sign-off, not merely a code change. It would also
invalidate the currently-passing `hand_verified_small_cases` assertion for n = 1
and the `validation_dataset()` entry that documents the round trip.

**(b) Keep the current behaviour permanently and document it explicitly.**
"Every trajectory applies at least one transition" becomes a stated, intentional
rule rather than an accidental consequence. Zero behaviour change; the existing
tests and validation dataset stay valid.

### Status of this question

**Option (b) has been implemented as documentation only** — see the dated
addendum appended to `PROJECT_BRIEF.md` and the note in `docs/schema/README.md`.
This is the safe, reversible default: it changes no behaviour and does not
foreclose option (a).

**Option (a) has NOT been implemented and must not be without explicit sign-off
recorded here.** If you want it, say so and this document will record the
decision before any engine change is made.

### Recommendation

**(b)**, on these grounds: the "always transition first" rule is what makes the
ordering guarantee (§7.2 — convergence is checked before cycle detection) clean
and uniform, and a zero-iteration trajectory is a genuine edge case for
downstream consumers (an empty parity sequence, a null stopping time, a chart
with a single point). If the zero-iteration result is wanted later, option (a)
remains available as an additive, versioned change.
