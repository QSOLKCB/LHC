# Initial release notes (2026-10-19)

## Browser Beam Lab and CPU-Only Rust Runtime

Release notes prepared 19 October 2026. **No tag or GitHub Release has been published.** The runtime package already declares version `0.1.0`. GitHub releases and tag refs were empty when checked at 12:40 UTC.

The features below are merged into `main` at [`e466aabccf0772d5f951bf7bcdd7fbd43ab8981a`](https://github.com/QSOLKCB/LHC/commit/e466aabccf0772d5f951bf7bcdd7fbd43ab8981a). These notes describe that implementation baseline; later documentation-only updates do not change its behavior.

## Overview

LHC now offers an interactive browser beam lab and a separate, reproducible CPU-only Rust library and command-line runner. Both illustrate simplified beam-cycle behavior for education and software experimentation. Neither is a validated LHC digital twin, accelerator control system, collision generator, or source of measured CERN results.

[Open the browser lab](https://qsolkcb.github.io/LHC/) · [Repository](https://github.com/QSOLKCB/LHC)

## Browser beam lab

- Preserves the supplied Fable control-room design in one HTML file with embedded CSS and JavaScript. No build step or application runtime dependencies are needed. Optional Google Fonts requests have system-font fallbacks.
- Displays two counter-rotating beams, four filling schemes, schematic injection and dump paths, beam readouts, and an explicitly synthetic event display.
- Supports automatic sequencing through injection, ramp, squeeze, adjustment, stable beams, dump, and ramp-down, plus manual stage controls.
- Adds pause/resume, reset, and polite machine-status announcements. Machine-clock speed and visible bunch-motion speed remain separate; active simulation time can lag wall time in background tabs or at low frame rates.
- Fixes sequencer resumption after manual filling and clarifies injection, collision-start, and session-event-count wording.
- Adds eight dependency-free Node regression tests and a GitHub Pages workflow. Pull requests test without deploying; successful main-branch site checks publish only `index.html` and `.nojekyll`.

These changes arrived in [PR #1](https://github.com/QSOLKCB/LHC/pull/1), with the browser-launch link added in [`13104f1`](https://github.com/QSOLKCB/LHC/commit/13104f1062b607fd6415f4abbfd3ad140612746a).

## Native CPU-only runtime

[PR #2](https://github.com/QSOLKCB/LHC/pull/2) adds a fresh Rust implementation using only the standard library and the pinned Rust 1.90.0 toolchain.

- A library-owned phase state machine and fixed-step simulation clock, independent of rendering and wall time. Steps split at phase boundaries instead of discarding leftover time.
- Configurable bunch count, timestep, and beta-star target, with input bounds and transition validation. State includes per-beam bunch intensities, energy, luminosity, integrated luminosity, and synthetic collision counts.
- Scriptable `inject`, `ramp`, `squeeze`, `collide`, `dump`, `pause`, `resume`, `reset`, `auto-on`, and `auto-off` commands.
- `run`, `replay`, and `bench` CLI operations. Versioned receipts capture configuration, ordered controls, execution policy, and the complete canonical final state, including each bunch and hidden state.
- Replay recomputes the run and compares all canonical state words. The displayed fingerprint is only a non-cryptographic diagnostic, not a signature or proof of authenticity.
- Scalar execution remains the default and reference. Optional bounded scoped CPU workers update independent bunch intensities while preserving the ordered reduction and dependent timestep sequence. There is no GPU/CUDA backend, unsafe code, manual SIMD, or fast-math path.
- Source-bound verification records, a replay fixture, and both original and independently rerun benchmark results are retained under `tests/fixtures/cpu-v1/`.

The browser still runs JavaScript. This release does not connect the native runtime to the browser or claim whole-cycle numerical parity between the two implementations.

## Getting started

Open `index.html` directly, or serve the checkout with `python3 -m http.server 8000` and visit `http://localhost:8000`.

From the repository root, with the pinned Rust toolchain installed:

```sh
# Generate and verify a deterministic native receipt.
cargo run --release --locked -- run --bunches 2808 --steps 5000 > fill.receipt
cargo run --release --locked -- replay fill.receipt

# Explicitly opt into workers, or measure both execution strategies.
cargo run --release --locked -- run --bunches 2808 --steps 5000 --workers 4 > parallel.receipt
cargo run --release --locked -- bench
```

Use `--script PATH` with `run` to replace the default auto-on/inject commands. Script indices count runner calls, including paused calls. See the [runtime contract](https://github.com/QSOLKCB/LHC/blob/e466aabccf0772d5f951bf7bcdd7fbd43ab8981a/docs/CPU-RUNTIME.md) for command ordering, bounds, receipt semantics, and model simplifications.

## Verification

The checked-in [verification record](https://github.com/QSOLKCB/LHC/blob/e466aabccf0772d5f951bf7bcdd7fbd43ab8981a/tests/fixtures/cpu-v1/verification.json) and [executed checks](https://github.com/QSOLKCB/LHC/blob/e466aabccf0772d5f951bf7bcdd7fbd43ab8981a/tests/fixtures/cpu-v1/checks.txt) report:

- 22 passing Rust tests: 18 library tests and four CLI integration tests.
- Eight passing Node browser-logic regression tests.
- Passing rustfmt and Clippy with warnings denied.
- Passing release-mode full-state receipt replay, scalar/worker parity, and independent-scenario parity.

The merged implementation also passed [CPU runtime CI](https://github.com/QSOLKCB/LHC/actions/runs/39253581233) and [site checks and Pages deployment](https://github.com/QSOLKCB/LHC/actions/runs/39253581156).

To rerun the checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
node --test tests/*.test.cjs
```

Use Node.js 22 or later. The Node tests use DOM/canvas stubs; they do not establish real-browser rendering or accessibility correctness. Initial browser/screenshot validation was blocked by the execution environment. None of these checks establishes scientific accuracy.

## Performance findings

Measurements on the shared Linux x86-64 executor support keeping scalar as the default:

- Per-tick workers were slower in both recorded runs. The first measured about 121.83 ms parallel versus 4.29 ms scalar for 128 steps; the rerun measured about 227.53 ms versus 1.31 ms.
- A separate experiment with 16 independent scenarios initially measured about 11.12 ms parallel versus 46.89 ms serial. The independent rerun measured 23.65 ms versus 23.80 ms, effectively tied.
- Both experiments passed their full-state equality gates. The inconsistent timings do not establish a reliable speedup. The independent-scenario experiment is a benchmark, not a batch receipt API or automatic execution policy.

Recorded evidence: [original benchmark](https://github.com/QSOLKCB/LHC/blob/e466aabccf0772d5f951bf7bcdd7fbd43ab8981a/tests/fixtures/cpu-v1/benchmark.txt) and [independent rerun](https://github.com/QSOLKCB/LHC/blob/e466aabccf0772d5f951bf7bcdd7fbd43ab8981a/tests/fixtures/cpu-v1/benchmark-review-rerun.txt).

## Compatibility and limitations

- Exact repeatability is scoped to the same source, toolchain, target, and floating-point environment. Cross-platform bit identity is not guaranteed; replay rejects differing final bits rather than applying a tolerance.
- Native injection and dump scheduling are intentionally simplified. Transfer-line motion, SPS batches, filling-slot geometry, and browser event categories are not reproduced.
- Timestep changes can change numerical results. Synthetic event frequencies, including Higgs/W/Z candidates in the browser, are not physics predictions.
- There is no telemetry, account requirement, live machine-data connection, or operational accelerator control.
- Receipts are self-consistency evidence. Inputs can be changed and receipts recomputed; source provenance must be checked separately.

## Provenance and license

The browser design came from the supplied `lhc-simulation.zip`, with verified SHA-256 `790112340a00bc13eebbd866e250ade3d62de5f6bf96b9b352410cba2a422908`.

OPT, GALAXY, and GLUBALL informed general CPU partitioning, contiguous storage, repeatability checks, and benchmark methodology. No donor source files or third-party Rust crates were imported, and no donor physics or throughput claims are inherited. Pinned references and licensing distinctions are recorded in [CPU technique provenance](https://github.com/QSOLKCB/LHC/blob/e466aabccf0772d5f951bf7bcdd7fbd43ab8981a/docs/DONOR-NOTES.md). The repository's existing Apache-2.0 license is retained.
