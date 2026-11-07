# Scripting comparison benchmark suite

**Target competitors:** CPython, Node.js JavaScript, Bun JavaScript,
Bun TypeScript, and Node.js TypeScript (with build/transpile measured
separately). Compare with Tempra Ergal **VM**, not experimental JIT.

Initial workloads: empty/minimal CLI; integer/text processing; input
file transform; JSON parsing/writing when Tempra supports it; process
invocation; filesystem paths; module imports; larger data collections.
Each scenario needs equivalent behavior, fixture input hashes, expected
stdout/result hashes and an explicit failure-path test.

Metrics: process wall duration, compiler phase timings, peak RSS and
CPU (added in K5.2–K5.3), output correctness and version/environment.
Also evaluate source size, error-handling explicitness, compiler
diagnostics and agent iteration counts separately from raw speed.

`scripts/k5_bench.py` is the K5.1 **generic wall-time** runner. It
spawns a new process for every sample but cannot enforce OS cold cache.
Never publish cross-language performance claims without validated
equivalence, pinned versions and raw output.
