# StateLab

A deterministic **state-evolution research platform**. Classic Collatz is the
first built-in system — not the point of the application. The engine is generic:
adding a new deterministic system requires **no engine changes** (Principle #6).

The full, frozen design lives in [`PROJECT_BRIEF.md`](./PROJECT_BRIEF.md).

## Cryptography research

This repository also contains an independent finding on a 2016 ChaCha/Salsa
diffusion-metric paper — a structural blind spot in the metric, confirmed by
the original author (Dr. Rajeev Sobti), plus follow-up verification of a
correction he supplied. Full writeup: [`FINDINGS.md`](./FINDINGS.md).
The code lives in [`crates/statelab-crypto`](./crates/statelab-crypto).

## Status

**All 11 roadmap phases (0–10) are complete**, including the Tauri bundler
producing installable desktop artifacts. StateLab ships as a native desktop app
(MSI / NSIS installers) plus an alternative single-file browser host — see
[`docs/PACKAGING.md`](./docs/PACKAGING.md).

- **Phase 1** — generic engine, `DeterministicSystem` interface, immutable
  Trajectory Object, Classic Collatz + full metric set. Tested.
- **Phase 2** — generic cycle detection, LRU memoization cache (§4.8),
  schema-migration registry (§4.9). Tested.
- **Phase 3** — the **React + TS + Tailwind** frontend (`src/`) with the
  **Research Controller** (§5.1) and a minimal UI shell: enter a number → the
  correct Trajectory JSON renders. The frontend reaches the engine over an IPC
  boundary ([`src/lib/invoke.ts`](./src/lib/invoke.ts)) shaped exactly like the
  Tauri `invoke()` call, so swapping to the real Tauri shell later touches only
  that one file.
- **Phase 4** — **Value Chart** (§5.3) and **Logarithmic Chart** (§5.4) as
  Canvas 2D React components ([`src/visualizations/`](./src/visualizations)).
  Pure consumers of the immutable Trajectory (the ESLint boundary rule forbids
  them from importing engine paths); the BigInt→f64 conversion is confined to the
  render call (§4.5). Tested with Vitest (jsdom).
- **Phase 5** — **Coral / Branch Visualization** (§5.5): a turtle-graphics
  renderer ([`src/visualizations/coral/`](./src/visualizations/coral)) with all
  six FROZEN parameters (odd/even angle, line length, opacity, scale, rotation)
  and all five direction rules (Relative default, Absolute, Rotate Before/After,
  Alternating). It draws from the trajectory's **pre-computed parity sequence** —
  never recomputing parity (Principle #3) — and shows "Metric Not Supported" when
  that metric is absent. The pure path engine is unit-verified against the
  Appendix B (n = 3) parity for every rule.
- **Phase 6** — **Feature Analysis** module (§6.1):
  [`src/modules/feature-analysis/`](./src/modules/feature-analysis) presents every
  System-Specific Metric already embedded in the Trajectory, grouped and
  formatted. It computes nothing — absent keys render the FROZEN "Metric Not
  Supported" fallback, present-but-null values render "N/A". The fallback path has
  dedicated tests (§7.5).
- **Phase 7** — **Dataset Explorer** (§6.2): all 7 generators (Range, Random Set,
  Primes, Even, Odd, Powers of Two, CSV import) with **mandatory streaming**. The
  host ([`crates/statelab-app/src/dataset.rs`](./crates/statelab-app/src/dataset.rs))
  generates states as lazy iterators, runs each, writes one NDJSON summary row, and
  drops the trajectory — O(1) backend memory. The frontend
  ([`src/modules/dataset-explorer/`](./src/modules/dataset-explorer)) folds rows
  into a fixed-size aggregate and keeps only a bounded row window, so no full set
  is ever held. Verified: the first rows of a 200 000-item range stream back in
  ~0.2 s (not buffered).
- **Phase 8** — **Comparison Lab** (§6.3):
  [`src/modules/comparison-lab/`](./src/modules/comparison-lab) compares multiple
  trajectories with an overlay chart (**Raw / Log / Normalized** modes), a feature
  table (raw ↔ min-max normalized), and a **cosine-similarity matrix** over the raw
  feature vectors derived from the metrics. All presentation math is at the
  consumer boundary; trajectories are produced through the Research Controller.
- **Phase 9** — **Export Center** (§6.4):
  [`src/modules/export-center/`](./src/modules/export-center) exports the current
  trajectory as **PNG, SVG, CSV, or JSON**, each embedding the full FROZEN
  metadata block (app / engine / schema / visualization versions, iteration limit,
  cycle detection, dataset definition, rendering parameters, timestamp, platform).
  PNG carries it in a `tEXt` chunk; CSV as `#` comment lines; SVG inside
  `<metadata>`. **SVG is generated only at export time** (§5.6). The Dataset
  Explorer also has a streaming CSV export that retains only summary rows.
- **Phase 10** — polish, performance pass, docs, packaging: a `criterion`
  benchmark harness for the engine loop and cycle detection (§7.7, baseline in
  [`docs/PERFORMANCE.md`](./docs/PERFORMANCE.md)), full schema documentation
  ([`docs/schema/`](./docs/schema)), an architecture map
  ([`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md)), and packaging — the **Tauri
  bundler now produces MSI and NSIS installers**
  ([`docs/PACKAGING.md`](./docs/PACKAGING.md)).

## Documentation

| Document | Contents |
|---|---|
| [`PROJECT_BRIEF.md`](./PROJECT_BRIEF.md) | The frozen design document (v1.3.1) |
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | Frozen layers mapped to real files; how boundaries are enforced |
| [`docs/schema/README.md`](./docs/schema/README.md) | Trajectory schema v1.0.0, metrics, migration guide |
| [`docs/PERFORMANCE.md`](./docs/PERFORMANCE.md) | Benchmark harness + recorded baseline |
| [`docs/PACKAGING.md`](./docs/PACKAGING.md) | Both hosts, installers, and toolchain requirements |
| [`docs/AUDIT_REMEDIATION.md`](./docs/AUDIT_REMEDIATION.md) | Post-audit pass: what changed, what was verified, which tests cover it |
| [`OPEN_QUESTIONS.md`](./OPEN_QUESTIONS.md) | Decisions that need a human, with options and recommendations |

**Two hosts, one UI build.** The desktop app (`src-tauri`, Tauri 2 + WebView2) is
primary; [`crates/statelab-app`](./crates/statelab-app) is an alternative std-only
host that serves the same UI over loopback and opens your browser, for
environments without WebView2. The frontend detects its host at runtime, so a
single build runs under either — the whole difference is confined to
[`src/lib/invoke.ts`](./src/lib/invoke.ts).

## Layout

```
crates/statelab-engine/   Pure Rust engine — ZERO host dependency (§3.3)
crates/statelab-dataset/  Dataset generators + streaming, shared by both hosts
crates/statelab-app/      Browser host (StateLabServer.exe)
src-tauri/                Tauri desktop shell — IPC commands only, no mathematics
src/                      React + TS + Tailwind frontend
docs/schema/              Versioned Trajectory schema docs (§4.9)
tests/                    Cross-cutting integration tests
```

## Run it

### Desktop app (recommended)

Install from either artifact produced by `npm run tauri build`:

- `target/release/bundle/msi/StateLab_0.1.0_x64_en-US.msi`
- `target/release/bundle/nsis/StateLab_0.1.0_x64-setup.exe`

Or run the built executable directly: `target/release/statelab.exe`. It opens a
native window — enter a number, click **Run trajectory**, and you get the status,
value and log charts, all 15 metrics, the Coral visualization, comparison, dataset
streaming, and exports, all computed by the Rust engine over IPC.

### Browser host (no WebView2 required)

```bash
bash scripts/package.sh                 # produces dist-package/StateLabServer.exe
```

Double-click `StateLabServer.exe`: a console window shows a local address and your
browser opens with the same UI. Self-contained, ~750 KB, no installer.

## Building & testing the engine

The engine is the source of truth and the Phase-1 Definition of Done:

```bash
cargo test          # unit, validation-dataset, property, and schema tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Benchmarks (§7.7) live under `cargo bench -p statelab-engine --bench engine` —
see [`docs/PERFORMANCE.md`](./docs/PERFORMANCE.md).

### Toolchain (Windows / MSVC)

The project targets **`x86_64-pc-windows-msvc`**, which Tauri requires. Setup
instructions and the exact Build Tools components are in
[`docs/PACKAGING.md`](./docs/PACKAGING.md).

The engine crate itself has no platform-specific dependencies and builds on any
host — only the desktop shell needs MSVC.

## Frontend

The React + TS + Tailwind app lives in `src/`. It is built with **npm** (corepack
is blocked from writing to `Program Files` on this machine, so the `pnpm` row was
swapped for `npm` — a tooling detail per Appendix C).

```bash
npm install          # one-time
npm run tauri dev    # desktop app with hot reload
npm run dev          # Vite dev server only (talks to a running StateLabServer.exe for /api)
npm run lint         # eslint, strict, no warnings
npm test             # vitest (jsdom)
npm run tauri build  # desktop app + MSI/NSIS installers
npm run sync-ui      # embed the production build into the browser-host crate
```

`npm run sync-ui` regenerates `crates/statelab-app/src/embedded_ui.html` (the
single-file build the browser host embeds). After running it, rebuild that host:

```bash
cargo build --release -p statelab-app
```
