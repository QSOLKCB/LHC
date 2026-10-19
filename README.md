# LHC Beam Simulator

**[Launch the browser lab](https://qsolkcb.github.io/LHC/)**

An interactive, educational illustration of the Large Hadron Collider beam cycle.
The supplied Fable design is kept as a single HTML file with embedded CSS and
JavaScript: no build step or runtime dependencies. A separate, CPU-only Rust
library and CLI now provide reproducible headless educational beam cycles.

## Run

Open `index.html` in a modern browser, or serve this directory with
`python3 -m http.server 8000` and visit `http://localhost:8000`.
Optional Google Fonts requests are made when online; system fonts work offline.
There is no telemetry, account, or live CERN data connection.

Press **Inject** to begin. The sequencer runs injection, ramp, squeeze, adjustment,
stable beams, dump, and ramp-down automatically. Disable it for manual stage
controls. **Pause simulation** freezes the clock and animation; **Reset** clears
the current fill. Machine-clock speed and visible bunch-motion speed are separate.
The clock follows active simulation time, so background tabs and low frame rates
can slow it relative to wall time.

## Model and limits

- Two counter-rotating beams, four filling schemes, injection/dump paths, and a
  schematic injector complex. Ring circumferences are scaled; placement is illustrative.
- Simplified beam readouts for energy, field/current, beta-star, luminosity,
  stored energy, pile-up, collision accumulation, and intensity loss.
- Synthetic event display: category probabilities, momenta, directions, and track
  shapes are illustrative, not a conservation-constrained collision generator.
  In particular, the displayed Higgs/W/Z candidate frequency is not a prediction.
- This is not a validated accelerator simulation, an operational control system,
  or a source of measured physics results. The native runtime has the same
  educational scope, with explicitly simplified injection/dump scheduling.

## CPU-only Rust runtime

See [the runtime contract, replay format, limits and benchmark procedure](docs/CPU-RUNTIME.md).
No CUDA, GPU, network service, or third-party Rust dependencies are used.
The native runner does not replace or drive the browser UI in this PR.

```sh
cargo run --release --locked -- run --bunches 2808 --steps 5000 > fill.receipt
cargo run --release --locked -- replay fill.receipt
cargo run --release --locked -- bench
```

Scalar is the default. `--workers 4` explicitly enables bounded scoped CPU
workers for independent per-bunch burn-off. Measure locally: thread overhead can
make this small workload slower. No universal speedup is claimed.

## Tests

With Node.js 22 or later, run:

```sh
node --test tests/*.test.cjs
```

The dependency-free tests execute the inline application in a minimal DOM/canvas
shell. They cover filling schemes, automatic/manual cycles, interrupted injection,
sequencer resumption, and luminosity invariants. They do not replace real-browser
visual/accessibility checks or scientific validation.

## GitHub Pages

The workflow checks pull requests without deploying. After merging to `main`, it
publishes only `index.html` and `.nojekyll` to Pages. All application assets are
inline, so it works under the project path `/LHC/` without base-path rewriting.

A repository administrator must first select **Settings → Pages → Build and
deployment → Source: GitHub Actions**. The workflow does not create or change
repository security settings. If Pages is not configured, configure it and rerun
the main-branch workflow. The intended URL is `https://qsolkcb.github.io/LHC/`;
it is not live until a successful Pages deployment.

## Source

Imported from the user-supplied `lhc-simulation.zip` on 2026-10-19. The accompanying
SHA-256 matched:

`790112340a00bc13eebbd866e250ade3d62de5f6bf96b9b352410cba2a422908`

The original control-room artwork and single-file structure are preserved.
Integration changes fix sequencer resumption, clarify illustrative event/clock
semantics, add a pause control and polite machine-status announcements, and add
repository tests/deployment. The repository's existing LICENSE is retained.

