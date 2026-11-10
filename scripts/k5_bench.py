#!/usr/bin/env python3
"""K5.1 reproducible fresh-process wall-time harness (NOT RSS or compiler stages)."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
from pathlib import Path
import statistics
import subprocess
import sys
import time


def digest(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def once(command: list[str], *, timeout: float) -> dict:
    began = time.perf_counter_ns()
    completed = subprocess.run(command, capture_output=True, timeout=timeout, check=False)
    elapsed = (time.perf_counter_ns() - began) / 1_000_000
    return {
        "wall_ms": elapsed,
        "exit_code": completed.returncode,
        "stdout_sha256": digest(completed.stdout),
        "stderr_sha256": digest(completed.stderr),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label", default="unlabeled")
    parser.add_argument("--runs", type=int, default=15)
    parser.add_argument("--warmup", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--input", type=Path, action="append", default=[], help="fixture file hashed in report")
    parser.add_argument("--tool-version", action="append", default=[], help="record exact runtime/tool version")
    parser.add_argument("--build-profile", default="unspecified")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("cmd", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    cmd = args.cmd[1:] if args.cmd and args.cmd[0] == "--" else args.cmd
    if args.self_test:
        cmd = [sys.executable, "-c", "print('k5-smoke')"]
        args.runs, args.warmup, args.out, args.label = 3, 1, None, "harness-self-test"
    if not cmd or args.runs < 1 or args.warmup < 0 or args.timeout <= 0:
        parser.error("provide a command after -- and positive sample/timeout counts")

    results = []
    try:
        for i in range(args.warmup + args.runs):
            sample = once(cmd, timeout=args.timeout)
            if sample["exit_code"] != 0:
                raise RuntimeError(f"command exited {sample['exit_code']} on iteration {i+1}")
            if i >= args.warmup:
                results.append(sample)
    except (OSError, subprocess.TimeoutExpired, RuntimeError) as error:
        parser.exit(1, f"FAIL: {error}\n")

    outputs = {(x["stdout_sha256"], x["stderr_sha256"]) for x in results}
    if len(outputs) != 1:
        parser.exit(1, "FAIL: stdout or stderr changed across samples\n")

    times = [x["wall_ms"] for x in results]
    try:
        git_revision = subprocess.run(
            ["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=False
        )
        git_status = subprocess.run(
            ["git", "status", "--porcelain"], capture_output=True, text=True, check=False
        )
        revision = git_revision.stdout.strip() if git_revision.returncode == 0 else None
        dirty = bool(git_status.stdout.strip()) if git_status.returncode == 0 else None
    except FileNotFoundError:
        revision, dirty = None, None
    try:
        input_hashes = {str(p): digest(p.read_bytes()) for p in args.input}
    except OSError as error:
        parser.exit(1, f"FAIL: fixture hashing failed: {error}\n")

    record = {
        "schema_version": 1,
        "label": args.label,
        "measurement": "fresh_process_wall_ms",
        "note": "Not OS cold-cache, peak RSS, CPU or compiler phase time",
        "timestamp_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu": platform.processor(),
        "git_revision": revision,
        "git_dirty": dirty,
        "tool_versions": args.tool_version,
        "build_profile": args.build_profile,
        "input_sha256": input_hashes,
        "python_harness": platform.python_version(),
        "cwd": os.getcwd(),
        "command": cmd,
        "warmups": args.warmup,
        "runs": len(times),
        "median_ms": statistics.median(times),
        "min_ms": min(times),
        "max_ms": max(times),
        "samples": results,
    }
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n")
    print(f"{args.label}: {record['median_ms']:.3f} ms median ({len(times)} fresh processes)")
    if args.self_test:
        assert len(times) == 3
        print("PASS: K5.1 harness self-test")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
