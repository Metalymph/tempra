#!/usr/bin/env python3
"""Read-only VM parity inventory for Tempra. No external dependencies.

Usage:
    python3 scripts/vm_parity_audit.py
    python3 scripts/vm_parity_audit.py --run-tests
    python3 scripts/vm_parity_audit.py --run-tests --out-dir target/k5/vm-parity

The audit finds review leads; passing test suites are NOT global soundness or
complete VM coverage claims. It does not execute other languages.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCAN_FILES = (
    "crates/tempra-driver/src/pipeline.rs",
    "crates/tempra-driver/src/compilation.rs",
    "crates/tempra-driver/src/module_graph.rs",
    "crates/tempra-tir-lower/src/lower.rs",
    "crates/tempra-bytecode-lower/src/lower.rs",
    "crates/tempra-tir/src/verifier.rs",
    "crates/tempra-bytecode/src/verify.rs",
    "crates/tempra-vm/src/interpreter.rs",
    "crates/tempra-repl/src/repl.rs",
    "crates/tempra-cli/src/app.rs",
)
SUITES = (
    ("hir_vm_differential", ("cargo", "test", "-p", "tempra-driver", "--test", "differential_tests")),
    ("hir_tir_differential", ("cargo", "test", "-p", "tempra-driver", "--test", "tir_differential_tests")),
    ("tir_correctness", ("cargo", "test", "-p", "tempra-driver", "--test", "tir_correctness_tests")),
    ("multi_module_vm", ("cargo", "test", "-p", "tempra-driver", "--test", "multi_module_vm_tests")),
    ("comptime", ("cargo", "test", "-p", "tempra-driver", "--test", "comptime_tests")),
    ("deny_enforcement", ("cargo", "test", "-p", "tempra-driver", "--test", "deny_enforcement_tests")),
    ("cache", ("cargo", "test", "-p", "tempra-driver", "--test", "cache_tests")),
    ("vm_pipeline", ("cargo", "test", "-p", "tempra-vm", "--test", "pipeline_tests")),
    ("tir_lower", ("cargo", "test", "-p", "tempra-tir-lower")),
    ("bytecode_lower", ("cargo", "test", "-p", "tempra-bytecode-lower")),
)
SEARCH = re.compile(r"\b(?:TODO|FIXME|unsupported|UnsupportedExperimentalVm)\b|todo!\s*\(|unimplemented!\s*\(", re.IGNORECASE)
TEST_SUMMARY = re.compile(r"test result:\s+(ok|FAILED)\.\s+(\d+) passed;\s+(\d+) failed")


def git(*args: str) -> str:
    result = subprocess.run(("git", *args), cwd=ROOT, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    return result.stdout.strip() if result.returncode == 0 else "UNKNOWN"


def inventory() -> list[dict]:
    entries = []
    for relative in SCAN_FILES:
        path = ROOT / relative
        if not path.is_file():
            entries.append({"path": relative, "found": False})
            continue
        content = path.read_bytes()
        lines = content.decode("utf-8", errors="replace").splitlines()
        hints = [{"line": i, "text": line.strip()[:150]}
                 for i, line in enumerate(lines, 1) if SEARCH.search(line)]
        entries.append({"path": relative, "found": True,
                        "sha256": hashlib.sha256(content).hexdigest(),
                        "review_hint_count": len(hints),
                        "review_hints": hints[:30]})
    return entries


def facts() -> dict[str, bool]:
    def read(relative: str) -> str:
        path = ROOT / relative
        return path.read_text() if path.exists() else ""
    p = read("crates/tempra-driver/src/pipeline.rs")
    repl = read("crates/tempra-repl/src/repl.rs")
    tests = read("crates/tempra-driver/tests/experimental_vm_tests.rs")
    return {
        "hir_source_executor_present": "run_source_hir_with_mode(" in p,
        "vm_source_executor_present": "run_source_vm(" in p,
        "vm_main_requirement_marker": "requires main()" in p,
        "vm_cli_args_rejection_marker": "CLI arguments are not yet supported" in p,
        "repl_interpreter_reference": "tempra_interpreter" in repl,
        "tir_interpreter_crate_present": (ROOT / "crates/tempra-tir-interp").exists(),
        "weak_vm_accepts_unsupported_test": "may succeed" in tests and "UnsupportedExperimentalVm" in tests,
    }


def run_tests(outdir: Path) -> list[dict]:
    results = []
    for name, cmd in SUITES:
        started = time.perf_counter()
        print("RUN", name, flush=True)
        try:
            proc = subprocess.run(cmd, cwd=ROOT, text=True, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, timeout=600, check=False)
            output, code = proc.stdout, proc.returncode
            status = "pass" if code == 0 else "fail"
        except subprocess.TimeoutExpired as error:
            output = (error.stdout or b"")
            if isinstance(output, bytes):
                output = output.decode("utf-8", errors="replace")
            code, status = None, "timeout"
        elapsed = round(time.perf_counter() - started, 3)
        logpath = outdir / f"{name}.log"
        logpath.write_text(output)
        matches = TEST_SUMMARY.findall(output)
        result = {"name": name, "command": list(cmd), "status": status,
                  "exit_code": code, "duration_s": elapsed,
                  "passed": sum(int(m[1]) for m in matches),
                  "failed": sum(int(m[2]) for m in matches),
                  "log": str(logpath.relative_to(ROOT))}
        print(" ", status.upper(), name, f"({elapsed:.1f}s)", flush=True)
        results.append(result)
    return results


def to_markdown(data: dict) -> str:
    rows = ["# VM parity baseline — audit, not a parity certificate", "",
            f"- Commit: `{data['git']['head']}`",
            f"- Branch: `{data['git']['branch']}`",
            f"- Tracked modifications at run time: `{data['git']['tracked_dirty']}`",
            f"- Timestamp (UTC): `{data['utc']}`", "",
            "## Execution-path markers (source inspection only)", ""]
    rows += [f"- `{key}`: **{value}**" for key, value in data["execution_markers"].items()]
    rows += ["", "## Existing test suites", "",
             "Passing tests are evidence only for their selected inputs; some cases",
             "explicitly allow `UnsupportedExperimentalVm` and do not prove parity.", ""]
    if data["tests"]:
        rows += ["| Suite | Status | Pass | Fail | Runtime (s) |", "|---|---|---:|---:|---:|"]
        for item in data["tests"]:
            rows.append(f"| {item['name']} | {item['status']} | {item['passed']} | {item['failed']} | {item['duration_s']:.3f} |")
    else:
        rows.append("Tests not executed; rerun with `--run-tests`.")
    rows += ["", "## Lowering, verifier and runtime review leads", "",
             "Static text matches are *not* confirmed defects and are not exhaustive.", ""]
    for item in data["source_inventory"]:
        if not item["found"]:
            rows.append(f"- `{item['path']}`: missing in this checkout")
            continue
        rows.append(f"- `{item['path']}`: {item['review_hint_count']} review hints")
        for hint in item["review_hints"][:8]:
            snippet = hint["text"].replace("`", "'").replace("|", "/")
            rows.append(f"  - L{hint['line']}: `{snippet}`")
    rows += ["", "## Next required engineering actions", "",
             "1. Turn every declared Tempra Script feature into runnable VM positive/negative tests.",
             "2. Audit every HIR -> TIR and TIR -> bytecode unsupported arm and verify all side effects.",
             "3. Add explicit diagnostics/exit/observable-output parity; no permissive ‘or unsupported’ tests.",
             "4. Move comptime and REPL only after differential gates; remove old engines last.",
             "5. Profile and optimize against internal K5 baselines; cross-language benchmark gate remains closed.", ""]
    return "\n".join(rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-tests", action="store_true")
    parser.add_argument("--out-dir", default="target/k5/vm-parity")
    args = parser.parse_args()
    if not (ROOT / "Cargo.toml").is_file():
        parser.error("Tempra repository root (Cargo.toml) not found")
    outdir = (ROOT / args.out_dir).resolve()
    outdir.mkdir(parents=True, exist_ok=True)
    data = {"schema_version": 1, "audit_type": "vm_parity_inventory_not_proof",
            "utc": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
            "git": {"head": git("rev-parse", "HEAD"),
                    "branch": git("branch", "--show-current"),
                    "tracked_dirty": git("status", "--porcelain", "--untracked-files=no") != ""},
            "execution_markers": facts(), "source_inventory": inventory(),
            "tests": run_tests(outdir) if args.run_tests else []}
    (outdir / "audit.json").write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    (outdir / "audit.md").write_text(to_markdown(data))
    print("WROTE", outdir / "audit.md")
    print("WROTE", outdir / "audit.json")
    if not args.run_tests:
        print("INFO: static inventory only; use --run-tests for selected test evidence")
    failures = [s["name"] for s in data["tests"] if s["status"] != "pass"]
    if failures:
        print("TEST FAILURES:", ", ".join(failures), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
