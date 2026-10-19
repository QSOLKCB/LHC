# LHC Beam Simulator

**[Launch the browser lab](https://qsolkcb.github.io/LHC/)** ·
[Release notes](RELEASE-NOTES.md) · [CPU runtime contract](docs/CPU-RUNTIME.md) ·
[Lean formalization](docs/FORMALIZATION.md)

An educational collider lab with two independent ways to explore a simplified
beam cycle:

- **Browser lab:** the original Fable control-room design, rendered in one HTML
  file with embedded CSS and JavaScript. No build step or JavaScript dependencies.
- **CPU-only Rust runtime:** a dependency-free library and command-line runner
  for deterministic, scripted beam cycles, full-state receipts and replay.
  No CUDA or GPU backend. It does not drive the browser UI.

## Browser quickstart

Open the [live lab](https://qsolkcb.github.io/LHC/), open `index.html` locally, or run:

```sh
python3 -m http.server 8000
```

Then visit `http://localhost:8000`. Optional Google Fonts requests are made when
online; system fonts work offline. There is no telemetry, account, or live CERN
data connection.

Press **Inject** to start. The sequencer runs injection, ramp, squeeze, adjustment,
stable beams, dump and ramp-down automatically. Disable it for manual stage
controls. **Pause simulation** freezes the clock and animation; **Reset** clears
the current fill. Machine-clock speed and visible bunch-motion speed are separate.
The clock follows active simulation time, so background tabs and low frame rates
can slow it relative to wall time.

## Rust quickstart

With Rust/rustup installed, the checked-in toolchain file selects **Rust 1.90.0**.
Run from the repository root:

```sh
# Run 5000 fixed one-second steps; save the full-state receipt.
cargo run --release --locked -- run --bunches 2808 --steps 5000 > fill.receipt

# Recompute the recorded run and compare every canonical state word.
cargo run --release --locked -- replay fill.receipt

# Run bounded correctness-checked CPU benchmarks.
cargo run --release --locked -- bench
```

The default run enables sequencing and injects at step zero. Options include
`--bunches`, `--dt-ms`, `--beta`, `--steps`, `--workers` and `--script`.
A script replaces the default controls; each line is `STEP COMMAND`, for example:

```text
0 auto-on
0 inject
100 pause
200 resume
300 dump
```

Save this as `controls.txt` and run:

```sh
cargo run --release --locked -- run --steps 1500 --script controls.txt > scripted.receipt
```

The library exposes validated configuration, machine controls, fixed-tick
stepping, observable snapshots and complete canonical state. Inputs are bounded
at 1–3564 bunches per beam, 1–1000 ms per tick and beta-star 0.3–1.0 m; the CLI
also bounds total steps, command count and input-file size. See the
[runtime contract](docs/CPU-RUNTIME.md) for control timing and receipt semantics.

### CPU techniques and performance

Scalar execution is the default/reference. `--workers 4` explicitly opts into
bounded scoped threads for independent per-bunch updates; luminosity reductions
and dependent timesteps retain their sequential order. The benchmark also tests
coarse parallel execution of independent scenarios.

Contiguous intensity arrays, bounded partitions and full-output conformance checks
are fresh adaptations of techniques from pinned **OPT, GLUBALL and GALAXY**
revisions. [Donor notes](docs/DONOR-NOTES.md) explain their provenance and licenses.

Measured per-tick threading was slower than scalar. Coarse scenario results varied
from a gain to effectively tied on rerun. These shared-host measurements are not
portable speedup claims. Both paths must pass full-state parity before benchmark
results are accepted. See the [recorded evidence](tests/fixtures/cpu-v1/verification.json).

## Model and limits

- The browser shows two counter-rotating beams, four filling schemes, injection
  and dump paths, and a schematic injector complex. Geometry is illustrative.
- Beam energy, field/current, beta-star, luminosity, stored energy, pile-up,
  collision accumulation and intensity loss are simplified educational readouts.
- Browser collision events use synthetic categories, momenta, directions and
  track shapes. They are not a conservation-constrained event generator; displayed
  Higgs/W/Z candidate frequencies are not predictions.
- The native runtime reuses the educational ramp/luminosity/burn-off equations
  with explicitly simplified aggregate injection and dump timing. It omits
  display geometry and random visual events. Whole-cycle browser/native parity
  is not claimed.
- Exact replay is scoped to the same source, toolchain, target and floating-point
  environment. Receipts are reproducibility checks, not signatures or authenticity
  proofs. Changing timestep can change numerical results.

**This is not a validated accelerator simulation, operational control system,
physical LHC digital twin, or source of measured physics results.**

## Tests and CI

An additive [Lean 4 specification](docs/FORMALIZATION.md) proves selected native
control, clock, CPU-partition and canonical-word contracts. It includes a pinned,
dependency-free proof build, axiom audit, source-drift gate and executable
Lean/Rust conformance checks. Its guarantees apply to the documented model;
floating-point physics, browser behaviour and complete Rust refinement remain
outside the proof boundary.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
node --test tests/*.test.cjs
```

Use Node.js 22 or later. The initial runtime includes 18 core and 4 CLI tests;
the browser has 8 dependency-free tests in a minimal DOM/canvas shell. Tests cover
controls, bounds, phase transitions, pause/reset, replay/tamper rejection, formula
conformance and full-state scalar/worker parity. They do not replace scientific
validation or real-browser visual/accessibility checks.

GitHub Actions checks the Rust runtime and browser separately on pull requests.
The runtime workflow also runs release-mode replay and bounded benchmark parity.
[Source-bound fixtures](tests/fixtures/cpu-v1/) retain local verification results,
including negative and variable timing results.

## GitHub Pages

The site is live at **https://qsolkcb.github.io/LHC/**. The Pages workflow checks
pull requests without deploying. Main-branch runs publish only `index.html` and
`.nojekyll`; the Rust runner is not deployed as a service. Inline assets support
the `/LHC/` project path without rewriting.

For a fork, a repository administrator must select **Settings → Pages → Build and
deployment → Source: GitHub Actions**. The workflow does not change repository
security settings.

## Source and license

Imported from the user-supplied `lhc-simulation.zip` on 2026-10-19. Its SHA-256 was:

`790112340a00bc13eebbd866e250ade3d62de5f6bf96b9b352410cba2a422908`

The original control-room artwork and single-file structure are preserved.
Browser integration fixes sequencer resumption, clarifies illustrative event/clock
semantics, adds pause/status accessibility, and supplies tests/deployment. The
separate Rust runtime was added in [PR #2](https://github.com/QSOLKCB/LHC/pull/2).
The repository's [Apache-2.0 license](LICENSE) is retained; no donor source files
or third-party Rust crates are imported.
