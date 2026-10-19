#!/usr/bin/env python3
"""Fail closed when the source reviewed for this formal model changes."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
manifest = json.loads((ROOT / "formal/source-manifest.json").read_text())
for name, expected in manifest["sha256"].items():
    actual = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit(f"Source drift: {name}. Review the Lean correspondence before updating its digest.")
print(f"PASS: {len(manifest['sha256'])} source bindings ({manifest['baseline_commit']}).")
