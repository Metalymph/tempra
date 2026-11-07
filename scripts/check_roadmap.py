#!/usr/bin/env python3
"""Minimal dependency-free coherence checks for roadmap assets."""
from __future__ import annotations

from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]
required = (
    "docs/roadmap.md",
    "docs/roadmap/migration.md",
    "docs/roadmap/status.md",
    "docs/roadmap/ergal.md",
    "docs/roadmap/release-gates.md",
    "docs/roadmap/formal-verification.md",
    "docs/roadmap/k5.md",
    "formal/manifest.toml",
    "formal/obligations/result.md",
    "AGENTS.md",
    "benchmarks/scripting/README.md",
)
for relative in required:
    path = ROOT / relative
    assert path.is_file() and path.stat().st_size > 0, relative

with (ROOT / "formal/manifest.toml").open("rb") as stream:
    manifest = tomllib.load(stream)

assert manifest["schema_version"] == 1
items = manifest["obligations"]
ids = [item["id"] for item in items]
assert len(ids) == len(set(ids)), "duplicate proof IDs"
assert "R.PROOF" in ids
states = {"DRAFT", "SPECIFIED", "MODEL_PROVEN", "IMPLEMENTATION_LINKED", "COMPLETE"}
for item in items:
    assert item["status"] in states, item
    for evidence in item["evidence"]:
        assert (ROOT / evidence).is_file(), (item["id"], evidence)
    if item["status"] in {"MODEL_PROVEN", "IMPLEMENTATION_LINKED", "COMPLETE"}:
        assert item["verifier"] != "UNDECIDED", item["id"]
        assert item["evidence"], item["id"]

roadmap = (ROOT / "docs/roadmap.md").read_text()
assert roadmap.count("## Scripting-first execution order (decision overlay, 2026-11-07)") == 1
assert "K5" in roadmap and "S1" in roadmap and "R.PROOF" in roadmap
assert "JavaScript" in roadmap and "TypeScript" in roadmap and "Python" in roadmap
print("PASS: roadmap and initial formal registry are coherent")
