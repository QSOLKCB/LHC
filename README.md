# LHC Beam Simulator

[![Release v0.2.0](https://img.shields.io/badge/release-v0.2.0-blue)](https://github.com/QSOLKCB/LHC/releases/tag/v0.2.0)
[![CPU runtime](https://github.com/QSOLKCB/LHC/actions/workflows/runtime.yml/badge.svg?branch=main)](https://github.com/QSOLKCB/LHC/actions/workflows/runtime.yml)
[![Lean formalization](https://github.com/QSOLKCB/LHC/actions/workflows/formal.yml/badge.svg?branch=main)](https://github.com/QSOLKCB/LHC/actions/workflows/formal.yml)
[![Site checks](https://github.com/QSOLKCB/LHC/actions/workflows/pages.yml/badge.svg?branch=main)](https://github.com/QSOLKCB/LHC/actions/workflows/pages.yml)
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.23476232.svg)](https://doi.org/10.5281/zenodo.23476232)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-lightgrey)](LICENSE)

**[Launch the browser lab](https://qsolkcb.github.io/LHC/)** ·
[v0.2.0 release](https://github.com/QSOLKCB/LHC/releases/tag/v0.2.0) ·
[Runtime contract](docs/CPU-RUNTIME.md) · [Formalization guide](docs/FORMALIZATION.md)

An educational beam-cycle simulator with three complementary components:

- **Browser lab:** the original Fable control-room design in one HTML file with
  embedded CSS and JavaScript. Interactive filling, ramp, squeeze, collision,
  dump, pause/reset, and synthetic event displays.
- **CPU-only Rust runtime:** a dependency-free library and CLI for fixed-step,
  scripted beam cycles, complete-state receipts and exact replay. Scalar
  execution is the reference; bounded CPU workers are optional.
- **Lean verification layer:** a separately executable specification of selected
  native control, clock, partition and canonical-word contracts, connected to
  Rust by source bindings and conformance tests.

The browser and native runtime are separate implementations. There is no GPU
backend, telemetry, account requirement, live CERN data or operational control.

## v0.2.0: Lean-checked contracts

- **47 named theorems** with pinned Lean **4.19.0**, bundled `Std`, and no external
  Lean packages or Mathlib dependency.
- **160 audited theorem declarations**, including generated support; no admitted
  proofs or `native_decide` proofs.
- **6,422 matching Lean/Rust rows** in the verification corpus, including all
  13 phases, 10 commands, both auto/pause settings, and 1/333/1000 ms timesteps.
- Seven SHA-256 source bindings, an axiom audit, and dedicated CI using the
  checksum-pinned official Lean distribution.

The release adds verification infrastructure. Rust and Lean package versions
remain `0.1.0`; runtime behaviour and the v0.1.0 evidence fixtures are preserved.
See the [release notes](https://github.com/QSOLKCB/LHC/releases/tag/v0.2.0) and
[retained verification record](formal/verification.json).

## Browser quickstart

Open the [live lab](https://qsolkcb.github.io/LHC/) or `index.html` directly.
To serve a local checkout:

```sh
python3 -m http.server 8000
```

Visit `http://localhost:8000`, then press **Inject**. Disable the sequencer for
manual stages. Machine-clock speed and visible bunch-motion speed are separate;
background tabs and low frame rates can slow active simulation time. Optional
Google Fonts requests have system-font fallbacks.

## Native runtime quickstart

Install Rust/rustup; `rust-toolchain.toml` selects **Rust 1.90.0**. From the root:

```sh
cargo run --release --locked -- run --bunches 2808 --steps 5000 > fill.receipt
cargo run --release --locked -- replay fill.receipt
cargo run --release --locked -- bench
```

Options include `--bunches`, `--dt-ms`, `--beta`, `--steps`, `--workers`
and `--script`. Configurations allow 1-3564 bunches, 1-1000 ms ticks, and
beta-star 0.3-1.0 m. Scripts support inject, ramp, squeeze, collide, dump,
pause/resume, reset and auto-on/off. See the [runtime contract](docs/CPU-RUNTIME.md)
for command ordering, receipt semantics and bounds.

Scalar is the default. Recorded per-tick threading was slower; independent-scenario
timings varied. These measurements establish no portable speedup.
[Donor notes](docs/DONOR-NOTES.md) record the OPT, GALAXY and GLUBALL references;
[historical fixtures](tests/fixtures/cpu-v1/) preserve the measured results.

## Check the proofs and implementation

With Lean/elan, Rust/rustup, Python 3, and Node.js 22 or later installed:

```sh
python3 formal/check_source.py
cd formal
lake build
lake env lean -DwarningAsError=true Audit.lean
cd ..
python3 formal/check_conformance.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
node --test tests/*.test.cjs
```

CI checks the proofs, source bindings, Rust comparison, 18 library tests,
4 CLI tests, and 8 browser regressions. The runtime job also checks release
replay and benchmark parity. Browser tests use DOM/canvas stubs; visual rendering
and accessibility need separate validation.

## Scope and limits

**The Lean proofs apply to the documented mathematical model.** Finite Rust
conformance tests and source hashes support correspondence; they do not establish
complete Rust refinement, verified compilation, floating-point physics correctness,
browser correctness, receipt-parser correctness, or cross-platform bit identity.
The [formalization guide](docs/FORMALIZATION.md) states the exact assumptions.

Beam dynamics and event displays are educational approximations. Synthetic
Higgs/W/Z candidates are not predicted physical event rates. Replay checks
self-consistency on a fixed source/toolchain/platform; receipts and fingerprints
are not signatures or authenticity proofs. Timestep changes can change results.

**This is not a validated accelerator model, physical LHC digital twin,
operational control system, or source of measured CERN results.**

## Deployment, provenance and license

The Pages workflow tests PRs without deploying. Successful main runs publish
only `index.html` and `.nojekyll`; the Rust runtime is not deployed as a service.
For forks, select GitHub Actions as the Pages build source.

The browser design originated in the supplied `lhc-simulation.zip`, with SHA-256
`790112340a00bc13eebbd866e250ade3d62de5f6bf96b9b352410cba2a422908`.
The native runtime arrived in [PR #2](https://github.com/QSOLKCB/LHC/pull/2);
Lean formalization arrived in [PR #4](https://github.com/QSOLKCB/LHC/pull/4).
The repository retains its [Apache-2.0 license](LICENSE). No donor source files
or third-party Rust crates were imported.

## Citation

For the v0.2.0 archival record, use DOI
[10.5281/zenodo.23476232](https://doi.org/10.5281/zenodo.23476232).

> Slade, T. (2026). *LHC: An Educational Beam-Cycle Simulator with a Reproducible
> CPU Runtime and Lean-Checked Contracts* (Version 0.2.0) [Computer software].
> Zenodo. https://doi.org/10.5281/zenodo.23476232
