#!/usr/bin/env python3
"""Compare two independently executed discrete models; fail on the first difference."""
from pathlib import Path
import random
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
COMMANDS = ["inject", "ramp", "squeeze", "collide", "dump", "pause", "resume",
            "reset", "auto-on", "auto-off"]


def corpus():
    # Exact manual entry paths for every Phase, in repr(u64) order.
    probe = ["inject"]
    physics = probe + ["step 40"]
    ready = physics + ["step 45"]
    prepare = ready + ["ramp"]
    ramp = prepare + ["step 60"]
    flat = ramp + ["step 1200"]
    squeeze = flat + ["squeeze"]
    collide = squeeze + ["step 600"]
    adjust = collide + ["collide"]
    stable = adjust + ["step 120"]
    dump = stable + ["dump"]
    paths = [[], probe, physics, ready, prepare, ramp, flat, squeeze, collide,
             adjust, stable, dump, dump + ["step 1"]]
    lines = []
    for path in paths:
        for auto in ("auto-off", "auto-on"):
            for pause in ([], ["pause"]):
                for cmd in COMMANDS:
                    lines += ["new 1 1000", *path, auto, *pause, cmd, "step 1"]
    # Before/at/after every timed edge, including complete automatic restarts.
    durations = [40000, 45000, 60000, 1200000, 60000, 600000,
                 120000, 21600000, 1000, 1200000]
    for dt in (1, 333, 1000):
        lines += [f"new 1 {dt}", "auto-on", "inject"]
        current = 0
        boundary = 0
        for duration in durations:
            boundary += duration
            target = (boundary - 1) // dt
            count = target - current
            while count > 0:
                part = min(count, 100000)
                lines.append(f"step {part}")
                count -= part
            current = target
            lines += ["step 1"]
            current += 1
        lines += ["step 1", "pause", "step 10", "resume", "step 1", "reset"]
    # Long manual holds become overdue as soon as automatic mode is enabled.
    for path, hold in ((flat, 61), (stable, 21601)):
        lines += ["new 1 1000", *path, f"step {hold}", "auto-on", "step 1"]
    # Configuration extremes and deterministic control mixtures.
    rng = random.Random(10421)
    for n in (1, 12, 3564):
        lines += [f"new {n} 333", "inject", "step 256"]
        for _ in range(100):
            lines.append(rng.choice(COMMANDS))
            lines.append(f"step {rng.randrange(0, 50)}")
    return lines


def run(args, cwd, data):
    result = subprocess.run(args, cwd=cwd, input=data, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            timeout=300, check=True)
    if result.stderr:
        print(result.stderr, file=sys.stderr, end="")
    return result.stdout.splitlines()


def main():
    lines = corpus()
    data = "\n".join(lines) + "\n"
    rust = run(["cargo", "run", "--quiet", "--release", "--locked", "--example",
                "formal_conformance"], ROOT, data)
    lean = run(["lake", "exe", "conformance"], ROOT / "formal", data)
    if len(rust) != len(lines) or len(lean) != len(lines):
        raise SystemExit(f"row count mismatch: input={len(lines)} Rust={len(rust)} Lean={len(lean)}")
    for i, (r, l) in enumerate(zip(rust, lean)):
        if r != l:
            raise SystemExit(f"row {i + 1}, input={lines[i]!r}\nRust: {r}\nLean: {l}")
        if lines[i].startswith("step ") and not r.startswith("1,"):
            raise SystemExit(f"step failed at row {i + 1}")
    print(f"PASS: {len(lines)} matching rows; 13 phases × 10 commands × 2 auto × 2 pause modes;")
    print("dt=1/333/1000 boundaries and full cycles; overdue holds; seeded controls; bunch bounds.")


if __name__ == "__main__":
    main()
