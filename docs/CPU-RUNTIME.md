# CPU-only runtime contract (v1)

This is a headless educational beam-cycle model, not a physical LHC digital twin,
accelerator control system, collision event generator or CERN data product. The
single-file browser lab remains independent and unchanged. No browser/native
whole-cycle bit parity is claimed. There is no GPU/CUDA backend or dependency.
Rust 1.90.0 is pinned; the workspace uses only the Rust standard library.

## Scope and state

The library owns integer tick/time counters, a phase state machine, controls,
energy, beta-star, luminosity, integrated luminosity, synthetic collision count,
and two contiguous per-beam bunch-intensity arrays. `Runtime::step` advances one
configured fixed timestep, independent of wall clock or display cadence. Phase
boundaries split a step so elapsed time is not discarded. Inputs: 1–3564 bunches
per beam, 1–1000 ms timestep, beta-star target 0.3–1.0 m. Non-finite/out-of-range
values and invalid transitions fail. No hidden random state exists.

The source browser's 450→6800 GeV ramp, geometric luminosity formula, crossing
angle factor, 40-hour residual lifetime, simple burn-off and main phase durations
are the basis. Native injection intentionally inserts two probe bunches immediately,
holds 40 s, holds another 45 s for aggregate physics injection, then populates the
requested bunch count. It does not reproduce transfer-line motion, individual SPS
batches, filling slot geometry, or visual event categories. Dump immediately clears
beam populations, followed by 1 s bookkeeping and 1200 s ramp-down. These choices
remove rendering-clock coupling rather than claiming physical improvements.

All populated bunches are treated as paired at collision points. Intensity loss
and synthetic collision integrals use explicit finite timestep updates; changing
timestep can change numerical results. Conformance and replay test software
contracts only, not scientific validity. No conservation-law or precision-physics
claim is made.

## CLI and control replay

```sh
cargo run --release --locked -- run --bunches 12 --steps 3000 > fill.receipt
cargo run --release --locked -- replay fill.receipt
cargo run --release --locked -- run --bunches 2808 --steps 5000 --workers 4 > parallel.receipt
```

Defaults come from `Config::default`; CLI runs 5000 steps and applies `auto-on`
then `inject` before step zero. `--script PATH` replaces these commands entirely.
Each nonblank script line is `STEP COMMAND`. Commands: `inject`, `ramp`, `squeeze`,
`collide`, `dump`, `pause`, `resume`, `reset`, `auto-on`, `auto-off`. Steps must be
nondecreasing; same-step commands execute in file order before that step. Script
indices count runner invocations, including paused steps, rather than simulation
ticks. Reset resets the model and disables auto sequencing; it does not rewind
the script. Commands after/equal to the run length are rejected. Example:

```text
0 auto-on
0 inject
100 pause
200 resume
300 dump
```

There are at most 100000 CLI steps, 1024 commands and 256 KiB input per file.
Receipts go to stdout; run diagnostics go to stderr. Redirect output yourself;
the CLI never overwrites arbitrary paths. Replay reads only the named file and
recomputes the full run; no network, shell execution or embedded file loading.

`lhc-receipt-v1` stores model/package version, configuration (beta as f64 bits),
requested execution policy, step count, ordered controls and the full canonical
state as fixed-width hexadecimal words. The state includes hidden fields and each
bunch, excludes the kernel policy, and is compared in full. The displayed FNV-style
fingerprint is a non-cryptographic diagnostic, never an authenticity/signature or
standalone correctness check. A receipt is self-consistency evidence; someone can
edit inputs and recompute it. Pin/check the source separately for provenance.

Exact repeatability is promised for the same source, toolchain, target and floating
point environment. Libm operations (`exp`, `ln`, `sqrt`) are not specified here as
cross-platform bit-identical. Replay fails closed on different final bits; it does
not silently apply tolerances. Hardware thread availability can differ: it only
changes independent update scheduling, not reduction order or receipt state.

## CPU execution and benchmark gate

Scalar is the default and reference. Opt-in `--workers N` updates independent
bunch intensities using scoped threads, after a single ordered luminosity/reduction
pass computes immutable update inputs. Dependent timesteps are never parallelized.
The selected worker count is capped by requested workers, bunch count, available
parallelism and 256; actual count is printed. The per-beam arrays avoid scattering
hot intensity fields through display objects. There is no unsafe code, manual SIMD,
fast-math, alternate reduction order or approximate math lookup table.

```sh
cargo run --release --locked -- bench
```

The bounded benchmark prepares the same stable-beam state, warms both paths, then
runs seven alternating-order paired samples of 128 steps with 2808 bunches per
beam. Each sample clones the start before timing. Timings include scoped-thread
creation/join cost; initialization, cloning and full-state validation are excluded.
Every pair must match every canonical word before timing results are accepted.
It reports host OS/architecture, requested/actual workers and median nanoseconds.
A second experiment measures 16 independent copies of that frozen scenario,
128 steps each, with scalar updates inside each job. It compares serial execution
against up to four coarse scoped workers using the same warmup/alternating seven
pair protocol and full ordered per-job state checks. This experiments with job
granularity; it is not an automatic execution policy or a new batch receipt API.

Timing is host-specific evidence, not a performance pass/fail threshold. Parallel
can be slower at this small scale. CI gates correctness/replay, not a speed ratio.

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
node --test tests/*.test.cjs
```

Core tests cover controls, bounds, deterministic replay, phase boundaries, pause,
reset, burn-off/luminosity invariants and full per-bunch scalar/worker parity.
CLI tests cover receipt tampering, repeat runs, malformed/oversized inputs and
script ordering. Existing Node tests continue exercising the browser lab. Neither
suite is scientific validation. See donor notes for pinned inspirations and the
source-bound verification record for actual executed checks.

## Measured example (2026-10-19)

The [captured local release benchmark](../tests/fixtures/cpu-v1/benchmark.txt)
passed both full-state parity gates. On this shared Linux x86-64 executor, the
per-tick worker path was slower (121.83 ms vs 4.29 ms median for 128 steps).
The coarse 16-scenario experiment was faster (11.12 ms vs 46.89 ms median).
An [independent rerun](../tests/fixtures/cpu-v1/benchmark-review-rerun.txt)
did not reproduce the coarse-scenario gain: 23.80 ms serial vs 23.65 ms parallel,
effectively tied. Per-tick workers were slower again. These measurements vary
substantially under shared load and establish no reliable speedup.
The candidate technique is to amortize CPU-worker overhead across independent jobs;
scalar remains the normal single-fill runtime.
