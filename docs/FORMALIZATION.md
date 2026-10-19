# Lean formalization of LHC's native discrete contracts

This is a machine-checked **specification of selected native-runtime contracts**,
with executable correspondence tests against Rust. It is not a proof of the
entire repository, a verified Rust compilation, or a proof of accelerator physics.
The browser and native numerical kernels remain their existing implementations.

The reviewed baseline is `v0.1.0`, commit
`aa69d32bea81e3bdac088ab34670b5dc1a1c589a`. The additions do not alter that tag,
the runtime version, runtime behaviour, browser, or existing evidence fixtures.

## Build and check

Install [Lean's official toolchain manager](https://lean-lang.org/install/manual/),
Rust/rustup, and Python 3. From the checkout root:

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

`formal/lean-toolchain` pins Lean **4.19.0**. The Lake manifest has no external
packages: proofs use Lean's bundled `Std`, without Mathlib. `lake build` builds
the proof library and a separate executable model. The existing root toolchain
continues to select Rust **1.90.0**. Node 22 or later runs the browser regressions.

The `Lean formalization` workflow downloads the official Linux Lean distribution,
checks its retained SHA-256, builds the proofs with warnings treated as errors,
audits theorem axioms, checks source bindings, and runs the Lean/Rust comparison.
It runs on every PR and main push, with read-only repository permissions.

## Proof coverage and source correspondence

| Lean module | Runtime correspondence | Kernel-checked statements |
| --- | --- | --- |
| `LHC/Control.lean` | `Phase`, `Command`, `command`, `phase_duration`, `transition_if_due` | Command success exactly matches guards; rejected commands retain the projected state; pause/resume idempotence; reset; dump clears the active count; injection/collision/dump eligibility; positive phase durations; manual ready holds; fresh auto mode does not inject; transition clocks and active-count bounds; two transition passes reach a fixed point before any timed deadline. |
| `LHC/Clock.lean` | `step` preflight guards and integer segment accounting | Paused steps are exact no-ops; rejection is atomic; successful active steps increment tick and elapsed time exactly and remain within u64 bounds; segment conservation and strict progress; the executable segment loop has sufficient fuel; success is equivalent to pause or passing the three clock guards. |
| `LHC/Partition.lean` | worker selection and `update_intensities` partitioning | Positive bounded worker count; exactly one chunk per worker; chunk sizes sum to the input length; no empty chunks when workers do not exceed elements; ordered chunk mapping equals scalar mapping; the subsequent left fold is identical without assuming associativity of addition. |
| `LHC/Canonical.lean` | `canonical_state_bits` and replay's full-state comparison boundary | A 19-word prefix and two n-word beams have length `19 + 2*n`; concatenation is injective when prefix/first-beam lengths agree; word-list equality accepts exactly equal lists and rejects a differing word. |

The control projection retains phase, phase elapsed milliseconds, tick, total
elapsed milliseconds, fill number, pause/auto flags and active bunch count.
Configuration retains bunch count and timestep. Both beam counts in the runtime
snapshot are compared to the projected active count. Floating values, arrays,
worker policy, and other numerical fields are not projected into this state.
Accordingly, a projected reset/atomicity theorem is not itself a full-bit Rust
state theorem; existing Rust tests cover full canonical-state behaviour.

`normalize` executes two iterations of the discrete transition relation. The
fixed-point theorem establishes that this suffices for the runtime's unbounded
`transition_if_due` loop: an elapsed phase can be followed by one automatic ready
edge, and new timed phases start at zero. The segment loop carries fuel equal to
`dtMs + 1`; `consume_total` proves that neither its zero-interval guard nor fuel
failure can occur with sufficient starting fuel. Each positive segment consumes
at least one integer millisecond. Fuel is a proof/model device, not a Rust limit.

Naturals represent integer fields. The three u64 step-overflow guards are explicit.
The model does not separately prove a finite-machine bound for the fill counter
or every intermediate Rust arithmetic operation. `Config.Valid` describes only
the discrete bounds; it does not formalize beta-star validation or NaN handling.
The partition theorem assumes the actual slices implement the specified ordered
chunks, and uses an arbitrary fixed per-element function. It does not verify
Rust's thread scheduler, allocation, memory model or floating-point environment.

The canonical model starts with already-encoded `UInt64` words. The 19-word
prefix is the actual emitted length (the Rust vector reserves capacity 22, which
is not its length). Injectivity requires fixed field boundaries. No theorem here
establishes the completeness of the Rust field list, `f64::to_bits`, parsing,
text formatting, fingerprint collision resistance, or receipt authenticity.

## How the implementation is connected to the specification

There are two distinct checks, neither of which is a Rust-to-Lean refinement proof:

1. `formal/source-manifest.json` binds the reviewed runtime sources, toolchains,
   package files and browser baseline by SHA-256. `check_source.py` fails on any
   change. A future source change requires reviewing the correspondence and
   updating the model, tests and digests together. The browser digest records its
   baseline; it does not give it proof coverage. A hash is a drift detector, not
   a semantic equivalence proof or signed provenance record.
2. `check_conformance.py` generates a deterministic action stream and compares
   the compiled Lean model with `runtime/examples/formal_conformance.rs`, which
   calls the real public Rust API. Neither executable reads expected results
   from the other. Every emitted row includes success, phase and clocks, fill,
   flags and both active beam counts. Mismatches and failed steps fail the check.

The corpus exercises all **13 phases × 10 commands × 2 auto settings × 2 pause
settings**, with a subsequent step. It also checks times immediately before and
after each automatic phase boundary, complete automatic restarts at **1, 333 and
1000 ms**, overdue manual holds, pause/reset, bunch-count extremes, and seeded
command mixtures. Rust additionally asserts full-state atomicity on command
rejection and checks the canonical layout length at every output row. These are
finite conformance tests, not universal implementation verification. Command
guard coverage is exhaustive over the finite phase/command/flag combinations;
numeric state-space coverage is not exhaustive.

## Trust boundary and next work

`Audit.lean` discovers all theorems under `LHC`, including generated support
theorems, and rejects dependencies outside the standard logical axioms
`propext`, `Quot.sound`, and `Classical.choice`. It also rejects project axiom
declarations. There are no admitted proofs, `sorry`, or `native_decide` proofs.
The trusted base includes Lean's kernel and bundled libraries, the installed
toolchain, and the host. The executable comparison additionally trusts the
compilers and comparison harness; it does not enlarge the mathematical claim.

Not formalized here: the browser DOM/canvas and sequencer, synthetic events,
luminosity and burn-off, floating-point rounding/transcendentals, scientific
validity, file/receipt parsing, CLI resource bounds, scheduling, and cross-platform
bit identity. Existing tests remain necessary. No CERN validation, digital-twin
claim, performance gain or new physical result follows from these proofs.

Further work can add an IEEE-754 numerical specification and error bounds, a
proved relationship between Rust state and the Lean projection, receipt-parser
correctness, and a separately specified browser control model. Each requires its
own evidence and explicit assumptions.
