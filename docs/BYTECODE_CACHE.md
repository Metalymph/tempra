# Tempra Bytecode Cache Specification v0.2

**Status:** revised specification of the documented v0 cache; implementation snapshot: GitHub `main` (not the local `feature/k5-compiler-timings` branch).
**Authority:** [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) for memory semantics; [BYTECODE_VM.md](BYTECODE_VM.md) and [BYTECODE_ISA.md](BYTECODE_ISA.md) for the execution and instruction contracts.
**Scope:** persistent Tempra Script bytecode artifacts, invalidation, trust, verification, reproducibility, and the migration to ownership-aware execution.

## 1. Contract and non-goals

The bytecode cache is an **optimization**, never an independent language authority. Reusing cached bytes MUST be observationally equivalent to compiling the same supported source with the same compiler semantics and declared inputs. A successful hit MUST NOT skip a new safety or capability obligation merely because a prior compiler accepted the file.

The format is compiler-private, not a published ABI, not an authenticated distribution format, and not a cache for arbitrary hostile third-party bytecode. The cache alone does not establish memory safety or formal soundness.

The planned execution architecture is Tempra Script's stack VM. JIT and Cranelift are legacy implementations designated for removal; neither is a cache target. Tempra Native LLVM AOT may have a distinct artifact/cache model and is not presumed to reuse VM bytecode artifacts.

## 2. Existing v0 container

A serialized artifact contains a header and a bytecode module. The documented implementation uses manual, little-endian encoding; strings use a `u32` little-endian byte length followed by UTF-8; enum encodings have a `u8` discriminant and payload. There is no compression or variable-length integer encoding in the documented v0 format.

| Header field | Documented v0 meaning |
|---|---|
| Magic | Four bytes `TMBC`, **not** `TMPR` (the historical ISA sketch's module magic) |
| Format version | `u32`; documented cache serialization version `2` |
| Compiler fingerprint | `tempra-{CARGO_PKG_VERSION}-semantics-8` on the documented `main` baseline |
| Source hash | 32 bytes formed by repeating a noncryptographic `DefaultHasher` result; identity hint only |
| Language version | Minimum relevant Tempra language version |
| Stdlib ABI version | Declared standard-library ABI compatibility identifier |

The cache file is documented under `.tempra/cache/` relative to the working directory on the experimental VM path. Actual CLI options and paths must be rechecked on the active branch before release.

### 2.1 Decoding is not verifying

`decode_artifact` permits decoding without automatically verifying code. This is useful for inspection, **not** permission to execute. Execution requires `decode_and_verify_artifact` or equivalent explicit `verify_module` before dispatch. Invalid instructions, stack effects, call targets, branch destinations and local indexes must fail with controlled diagnostics rather than host crashes.

## 3. Trust and hit/miss rules

**A hit is valid only if** format/header compatibility, compiler semantic trust, source identity and required declared inputs match, and the decoded module passes the current bytecode verifier.

For the *existing documented v0 source cache*, the trust boundary is deliberately narrow: compatible hits contain verified bytecode only; no TIR, capability witnesses, source spans, ownership summaries or analysis products are reconstructed. This is safe only to the extent that the semantic fingerprint proves those validations were already performed when the artifact was written.

A cache miss, corrupt artifact, obsolete fingerprint or verifier rejection triggers fresh compilation. New bytes are published **only after** compilation, contract enforcement and bytecode verification succeed. A fresh compilation failure MUST NOT overwrite a prior artifact with uncertified bytes. A valid hit should not rewrite bytes or metadata merely because it was read.

### 3.1 Analysis-free hits require an explicit trust proof

The existing cache does not serialize capability/ownership facts. Therefore:

1. Fresh compilation enforces all contracts supported by that compiler before writing.
2. A hit is allowed to omit rebuilding TIR and analyses **only if** its fingerprint and all input identities establish the same semantic obligations.
3. When an ownership checker, borrow summary, destructor rule, closure escape rule, FFI restriction or `@deny(alloc)` semantics changes, older artifacts cannot be considered trustworthy by convention; invalidate them or establish a mechanically checkable equivalent.
4. Do not promote v0.4 merely by updating this Markdown file. **No new semantic revision number is assigned here.** The implementing change chooses, tests and records its actual revision.

## 4. Historical semantic revision ledger

The bytecode **format** version and compiler **semantic** revision are independent. The following records the documented `main` history, not an assertion about the active local branch.

| Revision | Milestone | Why older compatible-looking artifacts cannot be trusted |
|---|---|---|
| `semantics-1` | E3.2 | Enforced broad `@deny` before bytecode publication |
| `semantics-2` | K1.7 | Canonical comptime execution: checked arithmetic, bodyless-call rejection, effect restrictions and deterministic termination |
| `semantics-3` | K2.2 | Correct specialization request order and distinction between TIR function identity and physical bytecode index |
| `semantics-4` | K2.4 | Deterministic interface selection and canonical declaration/diagnostic iteration |
| `semantics-5` | K3.2 | Checked `Int64` and conservative `Panic` origins |
| `semantics-6` | K4.5B | Actual enforcement of retained specialized `@deny` clauses |
| `semantics-7` | K4.9D | Families/categories and expanded specialized operation matching; K4.9E compatibility review |
| `semantics-8` | K3.3 | Correct typed `UInt64`/`Float64` TIR→bytecode selection and numeric runtime parity |

E4 `@pure` did not independently require an E3-era bump: existing enforcement handled its normalized restrictions and the new attribute altered source identity. K2.3 stdlib discovery did not bump the single-source cache since that cache did not compile imported module graphs. K2.5 reproducibility checked existing guarantees without changing language semantics. These are scoped historical decisions, not blanket precedents for future migrations.

## 5. Source, module, stdlib and environmental inputs

The documented v0 persistent cache applies to a narrow **single-source VM compilation** path. Ordinary multi-module compilation is fresh and is **not** claimed to have a dependency-aware persistent cache. Stdlib lookup for file/module compilation uses an explicit or executable-relative root, not an implicit cwd search.

A future multi-module cache must identify the entire resolved module graph, imported source and specialization identities, compiler options, semantic version, declared stdlib inputs, target/runtime mode and relevant ABI definitions. It must be invalidated on changes that affect semantic meaning, not just the top-level source bytes. Environment and host I/O must not be silently used as hidden comptime inputs.

**Security:** the repeated `DefaultHasher` bytes are not SHA-256, not a cryptographic integrity proof and not an authenticity mechanism. Do not claim protection against deliberate cache tampering. The compiler should treat malformed or hostile local bytes as potentially invalid and fail safely.

## 6. Reproducibility: K2.5 Level 3 bounded contract

The reported K2.5 gate requires byte-for-byte identical artifacts for supported input with identical source/module order, compiler build and semantic revision, target/host, options and resolved stdlib content. Process IDs, incidental temporary names, hash seeds, inode/mtime, test order, and cwd after explicit path resolution must not change output.

The documented suite includes single/multi-function, multi-module, specialization, comptime, capability and stdlib fixtures where the current compilation surface permits it. It compares raw bytes between independent compiler processes, not merely hashes. Interface tests outside bytecode coverage use frontend diagnostic/order checks instead.

This is **not** a Level 4 cross-machine/target guarantee. Recheck the exact corpus and commit before publishing benchmarks or release notes.

## 7. Known format gap

`BytecodeFunction::is_extern` was not serialized by the documented v0 container. Cached source paths did not exercise executable extern round-trips, so the known omission did not demonstrate a divergence on that bounded path. It remains a format-correctness gap, **not** a claim that extern cache round-tripping works. Resolve it before any expansion of the cacheable surface to extern-bearing modules.

## 8. Mandatory v0.4 migration gates

The [Memory Specification v0.4](Tempra_Memory_Ownership_v0.4.md) makes the following invariants normative for **both** Tempra Script and future Tempra Native execution:

- Duplicable by-value arguments preserve independent value semantics. Backend-level moves are allowed only when unobservable.
- `consume` invalidates the source semantically; nonduplicable resources cannot be cloned by an incidental bytecode `Dup`/load.
- Borrow provenance, `ref` and `inout` exclusivity, temporary borrowed aggregates, slice owner lifetime and closure captures are enforced.
- Cleanup occurs exactly once on normal/fallible exits; actor handles remain deferred after NO-CYCLE, and dynamic borrows cannot cross suspension.
- `@deny(alloc)` uses conservative semantic allocation accounting independent of optimization success.

These are **not** certified as implemented by v0 cache format 2 or semantic revision 8. An ownership-aware compiler must either (a) enforce before emission and version its trust boundary or (b) serialize enough verified data to recheck on cache hits. Merely decoding bytecode with the existing structural verifier is insufficient.

A cache migration must include positive/negative regressions: valid hit, stale old version, corrupt body with matching header, denied effect on cold compile, value-consume violations, borrow escapes, resource double-use and malformed function targets. A changed compiler cannot reuse a proven-too-old artifact just because its bytecode happens to decode.

## 9. Comptime transition

The documented K1 compile-time evaluator is HIR-based; migration to Tempra Script VM is a **planned parity task**, not an accomplished cache feature. A future compile-time evaluator must preserve deterministic evaluation, effect restrictions, compiler-owned resource boundaries, specialization and pruning semantics. A bytecode-only cache hit cannot bypass new comptime semantics. Where fuel/call-depth accounting differs, an explicitly reviewed semantic revision is necessary.

## 10. Cache improvements, not commitments

Dependency tracking for module builds, stable metadata and extern serialization, stronger content identification, artifact-atomic writing and optional compression are candidates. None is declared implemented here. Compression is not a substitute for semantic validity or cache integrity.

## 11. Verification before the documentation PR

Verify implementation paths and exact test commands against `feature/k5-compiler-timings`, then update measured status. Do not modify the recorded `semantics-8` baseline unless an actual compiler change has been merged. The current file consolidates documented requirements, **not** an assertion that the VM already satisfies v0.4.

**Invariant:** cache reuse may skip work, never required proof.
