# Post-Audit Remediation

**Date:** 2026-07-26
**Scope:** the eight tasks from the external audit remediation brief.

---

## CORRECTION — 2026-07-26 — the audit's spec citations were fabricated

**Read this before anything below it.**

The remediation brief repeatedly cited an addendum to `PROJECT_BRIEF.md`. The
project owner has confirmed: *"There is no clarification document — the auditor
made it up."* A full-repository search found **zero** occurrences of every value
attributed to it:

| Cited by the audit | Occurrences in repo |
|---|---|
| `Average Decline = mean(Current / Next)` → 2.0 | 0 — §4.4 and Appendix B both say `next / current`, "always exactly 0.5" |
| Default `max_iterations` = 10,000,000 | 0 |
| Coral "Line Width" / "Center Offset" / "Animation Speed" | 0 — §5.5 lists **six** parameters, marked FROZEN |
| Comparison feature "Trajectory Length" (8-feature vector) | 0 — §6.3 names no features at all |

### What was reverted

| Task | Originally | Now |
|---|---|---|
| 1 — `average_decline` → 2.0 | **Never implemented** — correctly blocked | Unchanged. OQ-1 **resolved** |
| 2 — default limit → 10,000,000 | Implemented | **Reverted to 100,000** |
| 3 — four extra Coral parameters | Implemented | **Reverted.** §5.5's six stand |
| 6 — 8-feature comparison vector | Never implemented | Actual 10-feature vector documented |

### The mistake worth recording

Tasks 1, 2 and 3 all rested on the *same* fabricated citation. Task 1 was refused
because it contradicted text that could be read; Tasks 2 and 3 were implemented
because the specification was silent, or because the additions could be construed
as "additive".

That distinction was the wrong one. The decisive question was **"is this cited
authority real?"** — and the single search that disproved the Average Decline
claim disproved all four at once. Applying the test uniformly at that moment would
have avoided the work that has now been undone.

For §5.5 specifically: the table is introduced as "Parameters (FROZEN)", which
reads at least as naturally as *"the parameter set is frozen at six"* as it does
*"these six behave as described"*. An interpretive choice was made silently and
should have been raised.

### What the audit got right

Not all of it was invented, and it should not be dismissed wholesale:

- **No CI existed** — true, valuable, and fixing it exposed a real pre-existing
  `npm run lint` failure.
- **No second deterministic system existed** — true, and 5n+1 validated
  Principle #6.
- **Fixed-point-at-start (OQ-2)** — a genuinely subtle and correct observation
  about the frozen §4.1 generation order.
- **The 5n+1 reference cases** (n = 1, 3, 13, 7) — independently re-derived and
  **correct**.

The pattern: its observations *about the code* held up. Its claims about *what the
specification says* did not.

---

## Task status (post-correction)

| # | Task | Status |
|---|---|---|
| 1 | `average_decline` formula | **Resolved — spec stands.** No code ever changed |
| 2 | Default iteration limit | **Reverted** to 100,000 |
| 3 | Coral: 4 extra parameters | **Reverted.** §5.5's six stand |
| 4 | Fixed-point-at-start | Documented (option b); engine untouched. OQ-2 open |
| 5 | CI workflow | **Kept** — caught a real breakage |
| 6 | Deep verification of 3 modules | **Kept** — 21 tests, no fabricated authority needed |
| 7 | Documentation reconciliation | **Kept**, plus Addendum B superseding A.1/A.3/A.5 |
| 8 | Second system (5n+1) | **Kept. Principle #6 confirmed** |

The sections below are the original report, retained as written. Where a section
describes work since reverted, the correction above governs.

---

## Task 1 — `average_decline`: BLOCKED, no code changed

**The brief cites a `PROJECT_BRIEF.md` addendum defining
`Average Decline = mean(Current / Next)` → `2.0`. That addendum is not in this
repository.**

Searched every `.md` and `.txt` file. What is actually committed says the
opposite, twice:

- `PROJECT_BRIEF.md:352` — "Mean of `next / current` … **always exactly `0.5`**"
- `PROJECT_BRIEF.md:656` (Appendix B) — "Average Decline | **0.5** (exact, always)"
- Appendix B's JSON literal — `"average_decline": 0.5`

Four other values the brief attributes to that addendum are likewise absent
repo-wide: `10,000,000`; Coral `Line Width` / `Center Offset` / `Animation
Speed`; and the comparison feature `Trajectory Length`. The auditor appears to
have worked from a spec revision that was never committed here.

`Document 1.txt` §6 (Authority of Documents) states: *"Implementation details
must never contradict higher-level documents."* Making the code report 2.0 would
do exactly that.

**Action taken:** recorded as **OQ-1** in [`OPEN_QUESTIONS.md`](../OPEN_QUESTIONS.md)
with three options and a recommendation, and cross-referenced from
`docs/schema/README.md` and PROJECT_BRIEF Addendum A.5. The formula is untouched.

**To unblock:** produce the addendum (or simply confirm you want 2.0). The change
is then one line in `growth_decline()` plus three test updates — minutes of work.
What was not acceptable was making that change silently.

---

## Task 2 — Default iteration limit → 10,000,000 ✅

Unlike Task 1 this contradicts nothing: the frozen spec never states a default.
100,000 appeared only in *example* payloads, and was originally my own
implementation decision.

**Files:** `crates/statelab-engine/src/engine.rs` (`EngineConfig::default`,
tagged `IMPLEMENTATION DECISION (§4.1)`), `src/lib/invoke.ts`
(`DEFAULT_ENGINE_CONFIG`, mirrors the Rust default),
`src/modules/dataset-explorer/DatasetExplorer.tsx` (UI default).

**Tests:** `cache_and_migration.rs::default_iteration_limit_is_ten_million`
(would have failed before — it asserted the old value implicitly),
`::raising_the_default_limit_does_not_change_converging_runs`.

**Measured, not assumed** (recorded in [`PERFORMANCE.md`](./PERFORMANCE.md)):

- Classic Collatz n = 27 under the new default: **0.27 ms**. A 100,000-item sweep:
  **10.6 s**. The change is free for converging systems.
- A genuinely divergent run (5n+1 from n = 7) scales **quadratically** — the state
  itself grows, so each step costs more: 100k iterations = 8.2 s, 200k = 38.9 s.
  Extrapolated, reaching 10,000,000 would take on the order of **a day**.

**Honest caveat:** for divergent systems the new default is effectively
unreachable — the binding constraint is wall-clock time, not the iteration count.
That is fine (such a run should be interrupted anyway) but it is documented
rather than glossed over, and the Dataset Explorer's limit is user-editable for
exactly this case.

### Bonus fix: a flaky round-trip test

While verifying Task 2, `schema_round_trips` failed once and then passed four
runs in a row. `computation_duration_ms` was the only nondeterministic field in a
Trajectory, and an arbitrary-precision `f64` is not guaranteed to compare equal
after a JSON round trip — a latent §7.4 violation that would have produced
spurious CI failures.

**Fix:** `trajectory.rs` rounds the duration to microsecond resolution, so it
always round-trips exactly. **Test:**
`collatz_validation.rs::execution_duration_round_trips_exactly`.

---

## Task 3 — Coral parameters ✅

All four added, each with a preserved default so nothing rendered differently
until the user touches a control.

| Parameter | Where | Backward compatibility | Test |
|---|---|---|---|
| Line Width | slider | Default 1.2 reproduces the old hardcoded 1.2 (analytical) and 0.7 (aesthetic) exactly | `defaults to the width the analytical mode previously hardcoded`, `defaults the aesthetic mode to its previous hardcoded 0.7`, `changing the slider changes BOTH draw modes` |
| Odd/Even Colour | colour pickers | Default to the old `#f0883e` / `#3fb950` | `defaults to the previously hardcoded parity colours`, `uses the supplied colours instead` |
| Centre Offset | X/Y numeric inputs | **`(0,0)` proven pixel-identical** to omitting the parameter | `(0,0) is pixel-identical to omitting the parameter entirely`, `a non-zero offset translates every drawn point by exactly that amount` |
| Animation Speed | slider + Replay | **Default `null` = instant**, original single-pass render, no rAF loop | `defaults to instant: the whole path is drawn in one pass, no rAF`, `with a speed set, the first frame draws only a prefix…`, `reaches the full path once enough time has elapsed` |

**Files:** `src/visualizations/coral/Coral.tsx`, `CoralPanel.tsx`,
`Coral.test.tsx`.

Verification technique worth noting: the "pixel-identical" claims are proven by a
**recording 2D context** that captures every canvas call, so two renders can be
compared operation-by-operation rather than by eyeballing output.

Implementation decisions tagged inline: one exposed Line Width drives both draw
modes via a ratio preserving the original 0.7/1.2 relationship; Animation Speed
is in segments-per-second with `0 → null` meaning instant (the spec named the
parameter but never defined its behaviour).

---

## Task 4 — Fixed-point-at-start ✅ (documented, engine untouched)

Recorded as **OQ-2** in [`OPEN_QUESTIONS.md`](../OPEN_QUESTIONS.md) with both
candidate resolutions. **Option (b) implemented — documentation only, zero
behaviour change**, per the brief's guidance that this is the safe default.
Option (a) (a new "step 0") would change the frozen §4.1 order and has **not**
been implemented.

Documented in PROJECT_BRIEF **Addendum A.2**. Worth noting the 5n+1 work
independently confirmed this is an engine property, not a Collatz quirk: 5n+1's
n = 1 round-trips identically (`five_n_plus_one_validation.rs::n_1_round_trips_like_collatz`).

---

## Task 5 — CI ✅

**File:** `.github/workflows/ci.yml`. Runs on push and PR to `main`; two jobs:

- **Rust** (windows-latest, matching the dev target): `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace`.
- **Frontend** (ubuntu-latest): `npm ci`, `npm run lint`, `npx tsc --noEmit`,
  `npm test`.

Two deliberate choices, commented in the YAML:

- `cargo test --workspace --exclude statelab` — the Tauri shell needs system
  WebView/GTK libraries and tests no engine logic (its commands are thin IPC
  wrappers). Everything that *computes* is covered.
- **No global `RUSTFLAGS: -D warnings`** — it applies to dependency compilation
  too, so an upstream crate's warning could fail our build. `clippy -- -D warnings`
  denies warnings for our crates, which is the actual intent.

**CI immediately earned its keep.** `npm run lint` — the exact command CI runs —
**was failing before this session**, and had been for some time, because I had
only ever run `npx eslint src` locally. Three real problems:

1. **ESLint was linting Rust build artifacts** in `target/` (generated `.js` from
   the Tauri build), failing on files that aren't in any tsconfig.
2. `vitest.config.ts` was in no TypeScript project.
3. `vite.config.ts` had genuine `no-unsafe-*` errors because `@types/node` was
   missing, so `import.meta.url` resolved to `any`.

**Fixed:** `.eslintrc.cjs` ignore patterns (`target`, `dist-package`, `coverage`,
`*.config.js`, `*.mjs`, `scripts`), `tsconfig.node.json` (added `vitest.config.ts`
and `"types": ["node"]`), and `@types/node` installed. `npm run lint` is now clean.

The new tests from Tasks 2, 3, 6 and 8 all run in CI, and the Task 2/3 ones fail
without their corresponding fixes.

---

## Task 6 — Deep verification of the three unaudited modules ✅

The audit verified Engine/Cache/Coral by compiling and independently recomputing.
The same standard applied here. **Findings include what passed, not only what
broke.**

### Dataset Explorer

**Memory bound: claim VERIFIED, now proven.** The README's O(1) claim was
previously supported only by reading the code and a timing anecdote. It is now
proven by a **tracking global allocator** that records peak live bytes.

**File:** `crates/statelab-dataset/tests/streaming_memory.rs`

| Test | What it proves |
|---|---|
| `peak_memory_does_not_grow_with_dataset_size` | 20× the items must not mean ~20× the peak. A buffering implementation fails immediately |
| `peak_memory_stays_small_in_absolute_terms` | 20,000 trajectories peak under 4 MB; fully materialised they would be tens of MB |
| `an_early_stop_does_not_process_the_whole_range` | A disconnected client halts generation rather than paying for the full sweep |

**Generators: all 7 verified, no changes needed.** `generator_validation.rs`,
11 tests. Every cited validation rule holds:

| Rule | Result |
|---|---|
| Positive integers only; 0 invalid | ✅ `range_generators_never_emit_zero`, `every_generator_emits_only_positive_integers` |
| Negatives invalid | ✅ reported as `SystemError` |
| Floats invalid (`3.5`) | ✅ |
| Scientific notation invalid (`1e3`) | ✅ |
| Empty invalid | ✅ dropped as separator noise (`csv_drops_empty_tokens_entirely`) |
| Malformed CSV "skipped-and-reported" | ✅ **reported** — a malformed value surfaces as a `SystemError` row carrying the offending input, which is strictly more informative than dropping it. Documented as a deliberate reading |
| Duplicates allowed by default | ✅ `csv_preserves_duplicates` |

Also verified: Powers of Two stays exact past `u64` (2^100), Primes are actually
prime and ascending, Random is seed-reproducible and bounded, Even/Odd partition
the range exactly, and an inverted range yields nothing rather than panicking.

### Comparison Lab

**Cosine similarity: VERIFIED against a hand derivation.**
`src/modules/comparison-lab/handComputed.test.ts`.

Both feature vectors for n = 3 and n = 6 were derived **from the trajectory
definitions, not read out of the code**, asserted, and then the similarity was
recomputed longhand and compared:

```
n=3 -> [7, 6, 3, 5, 2, 5, 2/7, 5/7, (10/3+16/5)/2, 0.5]
n=6 -> [8, 1, 4, 5, 2, 6, 1/4, 3/4, (10/3+16/5)/2, 0.5]
cos  = 144.5282539682540 / (sqrt(159.5129478458050) * sqrt(157.5461111111111))
     = 0.9116978734720410
```

The implementation matches to 15 decimal places.

> **Process note, in the spirit of the exercise:** my first hand-computed constant
> was `0.911692` — wrong in the 6th decimal, because I approximated the two square
> roots by hand. The *algebra* was correct (the implementation matched the derived
> formula to 15 dp); only the manual arithmetic was not. The test now carries the
> exact value and a comment recording the slip, because the point of hand-derived
> references is that they get checked, not trusted.

**Discrepancy recorded:** the brief cites an 8-feature vector including
"Trajectory Length". This implementation uses **10** features and has no
"Trajectory Length" (its information is carried by `total_stopping_time`). As
with Task 1, the cited source is not in this repository, so the implementation's
actual vector is documented and verified rather than changed. See OQ-1.

**Overlay modes: all three VERIFIED.** Raw is identity; Log is `log10(v+1)`
(keeping the fixed point 1 finite rather than `-Infinity`); Normalized maps
min→0, max→1 with correct interior spacing.

### Export Center

**All four formats VERIFIED — no changes needed.** Already covered by 13 tests in
`src/modules/export-center/exporters.test.ts`, which parse the metadata back out
rather than trusting UI copy:

| Format | How metadata is embedded | Verified by |
|---|---|---|
| JSON | `metadata` object | `embeds the full metadata block alongside the trajectory` |
| CSV | `#`-prefixed comment lines | `embeds every metadata field as a comment line` |
| SVG | `<metadata>` element | `embeds every metadata field inside <metadata>` |
| PNG | `tEXt` chunk | `inserts a readable tEXt chunk containing the full metadata` — **round-trips**: written, then parsed back with `readPngTextChunk` |

All ten required fields are asserted individually against `REQUIRED_METADATA_FIELDS`.
PNG validity is separately checked (signature intact, `IEND` still last), and
non-PNG input is rejected rather than corrupted.

**Streaming CSV export:** shares `streamDataset`, the same path covered by the
allocator tests above, and retains only compact summary rows — never trajectories.

---

## Task 7 — Documentation ✅

- `docs/schema/README.md` — `average_decline` now cross-references OQ-1 as
  disputed; `iteration_limit_used` documents the new default; the duration field
  documents microsecond rounding.
- `docs/PERFORMANCE.md` — new section with the measured 10M-limit timings and the
  quadratic-scaling caveat.
- `PROJECT_BRIEF.md` — **no frozen text edited.** A dated **Addendum A**
  (A.1–A.5) was appended, following the same superseding-addendum pattern the
  spec used for its own v1.1/1.2/1.3 revisions.
- `OPEN_QUESTIONS.md` — new, carrying OQ-1 and OQ-2.

---

## Task 8 — Second system, 5n+1 ✅

### Principle #6 finding: **CONFIRMED — the generic engine needed no changes**

This is the headline result. Adding a second real deterministic system required
**zero** modifications to the generic engine:

| File | Diff |
|---|---|
| `system.rs` | **unchanged** |
| `cycle_detection.rs` | **unchanged** |
| `cache.rs` | **unchanged** |
| `engine.rs` | changed only by Task 2 (default limit) — **not required by 5n+1** |
| `trajectory.rs` | changed only by the duration-rounding fix — **not required by 5n+1** |

No special-casing of 5n+1 exists anywhere in the engine, and none was needed.

**Files added:** `systems/five_n_plus_one/{mod.rs,validation.rs}`;
`systems/bigint_metrics.rs` (the Collatz extractor, factored out — every metric
is parity/sequence-derived and none was Collatz-specific, so both systems now
share one implementation rather than duplicating ~150 lines of metric maths);
registry (`AVAILABLE_SYSTEMS`, `run_by_id`, `run_by_id_cached`) in `systems.rs`.

**Wired end-to-end:** Tauri `run_trajectory` now dispatches through the registry
(and a new `list_systems` command); the browser host gained `?systemId=` and
`/api/systems`; the frontend has a system picker populated from the host, so it
can never drift from what the engine can actually run. Unknown ids are still
**rejected, never silently substituted**.

**Reference cases** — independently re-derived by hand before use, not copied:

| Input | Result | Verified |
|---|---|---|
| n = 1 | Converged, `[1,6,3,16,8,4,2,1]`, 7 iterations | ✅ |
| n = 3 | Converged, `[3,16,8,4,2,1]`, 5 iterations | ✅ |
| n = 13 | **CycleDetected**, 10-state cycle from index 0 (`83×5+1 = 416` re-checked) | ✅ |
| n = 7 | **IterationLimitReached** at a test limit of 1,000; genuinely divergent | ✅ |

**Why this matters beyond 5n+1:** Classic Collatz only ever *converges* within any
tested range, so `CycleDetected` and `IterationLimitReached` were previously
exercised only by synthetic test doubles. 5n+1 reaches all three non-error
terminal statuses with real mathematics — a materially better regression surface
for the generic engine.

**Tests:** `crates/statelab-engine/tests/five_n_plus_one_validation.rs`, 8 tests,
including that a non-converging run reports `total_stopping_time: null` (a path
Collatz can never reach) and that the two systems produce genuinely different
trajectories from the same input.

**Live-verified** through the running host:

```
systems:   [{"id":"classic-collatz",...},{"id":"five-n-plus-one",...}]
n=3:       ["3","16","8","4","2","1"]  Converged
n=13:      CycleDetected, cycle_length 10
bogus id:  {"error":"unknown system_id: bogus"}
```

---

## Test inventory added this session

| File | Tests | Covers |
|---|---:|---|
| `crates/statelab-engine/tests/five_n_plus_one_validation.rs` | 8 | Task 8, Principle #6 |
| `crates/statelab-dataset/tests/streaming_memory.rs` | 3 | Task 6, memory bound |
| `crates/statelab-dataset/tests/generator_validation.rs` | 11 | Task 6, all 7 generators |
| `src/modules/comparison-lab/handComputed.test.ts` | 7 | Task 6, hand-derived cosine + overlays |
| `src/visualizations/coral/Coral.test.tsx` (extended) | +11 | Task 3, all 4 new parameters |
| `crates/statelab-engine/tests/cache_and_migration.rs` (extended) | +2 | Task 2, default limit |
| `crates/statelab-engine/tests/collatz_validation.rs` (extended) | +1 | duration round-trip |
