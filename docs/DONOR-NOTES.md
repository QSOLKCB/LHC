# CPU technique provenance

This PR is a fresh implementation for LHC's educational model. It imports no
third-party source files or Rust crates. The LHC repository's Apache-2.0 LICENSE
is retained. Pinned donors informed the execution and verification design:

- **OPT**, commit [`7556dd36acd53ed3ed538fd03b1db44e5c1ec869`](https://github.com/QSOLKCB/OPT/tree/7556dd36acd53ed3ed538fd03b1db44e5c1ec869):
  OPT-PAR-001 bounded CPU partitioning and OPT-SOA-001 contiguous hot-field layout.
  Applied as separate contiguous beam-intensity arrays and a scalar oracle.
  Full per-element/canonical output comparison is required; checksum equality
  alone is insufficient. Worker counts 1/2/4/7 and uneven tails are tested.
- **GALAXY**, commit [`f219bb427b3a9b0c93900169d903f48f410ae67a`](https://github.com/QSOLKCB/GALAXY/tree/f219bb427b3a9b0c93900169d903f48f410ae67a),
  [`docs/CPU-RUNTIME.md`](https://github.com/QSOLKCB/GALAXY/blob/f219bb427b3a9b0c93900169d903f48f410ae67a/docs/CPU-RUNTIME.md)
  and `cpu-runtime/src/main.rs` (Apache-2.0): warm all paths, alternate/interleave
  timing order, record medians and validate outputs. Applied to bounded per-tick
  and independent-scenario experiments, with host/load-scoped results. Its
  astrophysical equations and BAM/LUT approximations are not applicable here.
- **GLUBALL**, commit [`d25da2a1ade877e2fe048fd4dabf019728cb25e7`](https://github.com/QSOLKCB/GLUBALL/tree/d25da2a1ade877e2fe048fd4dabf019728cb25e7),
  [`runtime/rust/lib_v2.rs`](https://github.com/QSOLKCB/GLUBALL/blob/d25da2a1ade877e2fe048fd4dabf019728cb25e7/runtime/rust/lib_v2.rs)
  (MPL-2.0 file/crate): bounded contiguous partition ranges and worker-independent
  repeatability checks. Only general techniques were independently implemented;
  no MPL source was copied into Apache-licensed files.

The LHC equations originate in [`index.html` at the base commit](https://github.com/QSOLKCB/LHC/blob/13104f1062b607fd6415f4abbfd3ad140612746a/index.html).
The runtime splits phase-boundary time and simplifies injection/dump geometry;
this is not whole-frontend numerical parity. No donor throughput number, public
fixture result, physical claim, license assumption, or hardware claim is inherited.

Coupled timesteps and luminosity reductions stay sequential. Only independent
per-bunch updates or independent full scenario copies are eligible for parallelism.
The measured negative per-tick result and variable scenario result are preserved
in `tests/fixtures/cpu-v1/`; neither establishes a universal speedup.
