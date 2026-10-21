# K4.10 — Final Specialized Effects Gate

**Verdict: PASS for the supported compiler-authoritative static surface.**

This ledger covers the implemented K4 specialized-effects track through K4.9E,
on `feature/k3-effects-runtime-parity`, with compiler trust `semantics-7` and
bytecode format 2. The normative authorities are
[Capability Contracts v0.1](specs/effects-v0.1.md) and
[Specialized Effects v0.2](specs/specialized-effects-v0.2.md). The
[v0.1 specialized specification](specs/specialized-effects-v0.1.md) remains a
historical checkpoint. This is a same-build, same-host/target static compiler
gate, not a sandbox, an all-backend proof, or a cross-machine guarantee.

## 1. Guarantee and production coverage classes

Fresh supported compilation verifies TIR, analyzes it once, binds contracts to
concrete function identities, enforces broad denials first and then specialized
predicates, and only then lowers/verifies bytecode. Both definite and potential
specialized matches reject compilation. The guarantee covers every emitted
function and its statically reachable facts, including non-entry functions and
emitted generic specializations. Unsupported lowering fails; it is not proof
of contract satisfaction. Comptime-pruned operations absent from TIR are not
analyzed. No second HIR analyzer or runtime policy is added.

Production coverage classes mean:

- **A:** classified and reachable through an existing production native binding.
- **B:** classified/enforceable through an actual TIR producer, but without a
  production native binding.
- **C:** recognized taxonomy/contract vocabulary without a current executable
  source-to-classified-TIR producer on the capability-aware path.

**Current result: 3 A operations, 0 B operations, 10 C operations.** There are
four specialized native bindings because both `print` and `println` produce
`IoWrite`. Synthetic facts prove the closed matcher matrix for all 13 operations;
they do not establish a production TIR producer and do not upgrade C to B.
Class C does not mean the underlying action is impossible everywhere: for
example HIR command execution, input and exit exist, but annotated HIR execution
is rejected and those actions have no stable classified TIR origin here.

The authoritative metadata/producer boundary is
[operations.rs](../crates/tempra-effects/src/operations.rs),
[facts.rs](../crates/tempra-effects/src/facts.rs),
[externs.rs](../crates/tempra-effects/src/externs.rs), and the separate production
[native registry](../crates/tempra-native/src/registry.rs).

## 2. Complete operation ledger

`D/T` means direct/transitive coverage through production source compilation.
`Synthetic only` means identity/membership/resource matching is tested on
controlled facts; no production execution claim follows. Parameter lists below
are the canonical schema, not a claim that a native can supply those values.

| Operation | Parent | Family | Categories | Resource parameters | Native binding | Class / static enforcement | D/T and limitations |
|---|---|---|---|---|---|---|---|
| `fs.read` | Io | fs | io.read | source | `std.fs.read_text` | A; definite/potential enforcement | D/T tested; literal source exact, dynamic/missing source unknown; opaque path spelling |
| `fs.write` | Io | fs | io.write | destination | `std.fs.write_text` | A; definite/potential enforcement | D/T tested, including recursion/diamond/generics; content is not the destination |
| `fs.remove` | Io | fs | none | destination | none | C; matcher ready, no production origin | Synthetic only; removal is not io.write; no native deletion binding |
| `net.connect` | Io | net | none | host, port, protocol (reserved) | none | C; matcher ready, no production origin | Synthetic only; not io.read/io.write; no endpoint tracking |
| `net.listen` | Io | net | none | host, port, protocol (reserved) | none | C; matcher ready, no production origin | Synthetic only; listener creation, not accept or receive |
| `net.send` | Io | net | io.write | none | none | C; matcher ready, no production origin | Synthetic only; resource parameters rejected; no connection identity |
| `net.receive` | Io | net | io.read | none | none | C; matcher ready, no production origin | Synthetic only; resource parameters rejected; no connection identity |
| `io.read` | Io | none | io.read | stream | none | C; matcher ready, no production origin | Synthetic only; HIR read_line is not a classified TIR native |
| `io.write` | Io | none | io.write | stream | `std.io.print`, `std.io.println` | A; definite/potential enforcement | D/T tested; stream is unknown, never inferred from printed content |
| `process.exec` | Process | none | none | executable | none | C; matcher ready, no production origin | Synthetic only; HIR commands do not establish VM/TIR effect coverage |
| `process.exit` | Process | none | none | none | none | C; matcher ready, no production origin | Synthetic only; HIR exit has no stable TIR origin; ordinary return is not exit |
| `spawn.task` | Spawn | none | none | scope (reserved) | none | C; matcher ready, no production origin | Synthetic only; no task runtime/creation native, no scope guarantee |
| `spawn.actor` | Spawn | none | none | scope (reserved) | none | C; matcher ready, no production origin | Synthetic only; no actor runtime/creation native, no scope guarantee |

`protocol`/`scope` remain declared/reserved metadata. Their existence is not a
claim of concrete protocol/actor-resource semantics. The current model has no
connection handle, accept operation, actor messaging attribution or runtime
resource inference. New operations must supply total canonical metadata before
admission; family/category contracts remain predicates and are never expanded
to this table's current operation list.

`std.fs.exists` is an additional reachable **unclassified Io** fact, not
`FsRead` and not a fourteenth operation. Its unknown membership yields potential
violations against fs/net/read/write restrictions. Alloc-only and explicitly
capability-free natives do not gain specialized Io facts.

## 3. Contract coverage

| Contract/case | Verified behavior | Evidence |
|---|---|---|
| Broad `@deny` | `denied ∩ required` remains authoritative; broad violations precede specialized results | driver enforcement and mixed broad/specialized tests |
| `@pure` | denies Io/Process/Spawn/Nondeterminism; permits Alloc/Panic unless separately denied | pure and specialized enforcement regressions |
| Family Fs / Net | canonical family membership; no expansion and no new capability bit | normative v0.2 matcher matrix, real fs natives |
| Category IoRead / IoWrite | cross-family ingress/egress membership | all 13 matrix rows; real read/write/console source tests |
| All concrete targets | exact operation identity; every other operation disjoint | 13-by-13 synthetic identity matrix |
| Bare `io.read` / `io.write` | category; previously invalid bare v0.1 spellings now accepted | parser, canonical contract and source regressions |
| `io.read()` / `io.write()` | concrete generic I/O operation | parser, canonical and matching regressions |
| Resource-qualified v0.1 I/O | remains operation-scoped, never reinterpreted as category | K4.9E exact/mismatch/unknown stream and source tests |
| Exact resources | equal known = definite; unequal known = disjoint; missing/unknown = potential | matcher and source provenance tests |
| Wildcard resources | definite for the identified operation, even unknown resource | existing matcher and native-console tests |
| Unknown resources | preserve operation identity and remain conservative; no caller literal substitution | extraction/propagation and source tests |
| Unclassified operation | relevant parent = potential; different parent = disjoint | real exists origin; synthetic Io/Process/Spawn facts |
| Invalid target/parameter pair | rejected at parsing or checked canonical binding | parser and binding regressions |
| Generic inheritance | each emitted ID receives the original source contract/spans and is enforced | independent Int/String specialization regressions |
| Transitive/recursive/diamond calls | same fixed point, canonical shortest witness and origin deduplication | analysis and differential suites |

Deliberate exclusions are preserved: FsRemove is not IoWrite; NetConnect and
NetListen are neither IoRead nor IoWrite; FsRead is not IoWrite; FsWrite is not
IoRead; generic I/O is not automatically filesystem I/O; Process/Spawn are not Io.
Operations without production origins have synthetic matching coverage only.

## 4. Execution-boundary matrix

Labels describe actual supported execution, not just accepted syntax.

| Boundary | Classification | Actual guarantee |
|---|---|---|
| Fresh normal source VM compilation | ENFORCED | supported lowering → verify → analyze → contracts → enforce → bytecode verify; failures publish no artifact |
| Cranelift native JIT | LIMITED | consumes already-enforced bytecode for eligible subset; current std effect natives are not JIT-eligible |
| Interpreter fallback from JIT | ENFORCED | consumes the same previously enforced bytecode; trusted native resolver executes supported calls; no fallback around compilation rejection |
| Profiling | ENFORCED | uses the shared compiled/cache validity path before VM profiling |
| Multi-file/module compilation | LIMITED | emitted supported functions/specializations enforced fresh; no module cache; annotated extend methods rejected |
| Generic specializations | ENFORCED | supported emitted concrete specializations retain distinct IDs and shared declaration intent; no claim of full specialization support |
| Supported single-source extend methods | ENFORCED | actual lowered method ID bound to original method declaration |
| Annotated module/file extend methods | REJECTED | explicit unsupported-enforcement error, including unused methods; no silent contract loss |
| HIR interpreter/test runner | REJECTED | any source capability contract rejected before execution, including unused declarations |
| REPL | REJECTED | annotation rejection before state commit; earlier session state preserved |
| Source-TIR execution | REJECTED | annotation detected before lowering loses source contract metadata |
| Raw TIR and bytecode APIs | NOT APPLICABLE | no source-level contracts carried; not an independent certification path |
| Trusted bytecode-cache miss | ENFORCED | fresh supported source compilation and enforcement before writing/execution |
| Trusted compatible cache hit | LIMITED | verified bytecode from matching source hash/fingerprint; relies on prior successful enforcement; no TIR/facts or runtime re-enforcement |
| `check` / `emit-tir` | NOT APPLICABLE | non-executing tooling; successful typecheck/IR emission does not certify contracts |

Native calls, FFI and raw-IR consumers remain trust boundaries. The default HIR
runtime being more complete does not make rejected annotated execution supported.
Missing network/task APIs are explicit feature limits rather than a claimed
passing native execution path. The static guarantee ends at the trusted native
implementation and compiler-owned cache assumptions.

## 5. Cache and failure invariants

The current fingerprint is `tempra-{CARGO_PKG_VERSION}-semantics-7`; bytecode
format remains 2. These revisions are independent. Hits require exact source
hash and fingerprint plus successful decoding and bytecode verification.

Existing permanent regressions prove:

- a valid, executable, source-hash-matching semantics-6 artifact is rejected;
- fingerprint/source changes force fresh compilation;
- malformed bytes and decodable invalid call IDs rebuild safely;
- fresh failures neither publish a new artifact nor overwrite existing bytes;
- successful fresh compilation writes current trust; valid hits preserve byte
  content and modification time, including ordinary/profiled source VM paths;
- hits return BytecodeModule only, never reconstructed TIR or effect facts.

K4.10 found and closes one additional malformed-cache failure: a nonempty
function with more parameters than local slots previously indexed outside the
verifier's local-state vector. The existing InvalidLocalId error now rejects it
before indexing; the existing cache test also verifies fresh rebuild for this
case. K4.9E's invalid-call-ID fix remains covered. Neither correction changes
valid-program effects, artifact schema, cache paths or `semantics-7`.

The malformed-input verdict is bounded by these fixtures and code review. This
is not an exhaustive adversarial decoder/resource-exhaustion proof. The cache
header is unauthenticated, source hashing is non-cryptographic, and a deliberately
rewritten matching header is outside the compiler-owned trusted-cache model.
No cache migration, hostile-native sandbox or runtime contract guard is claimed.

Called unknown externs fail closed even in non-entry functions; entirely unused
extern declarations are not effects. Known unclassified externs remain potential
under the relevant parent. Missing summaries/witnesses and conflicting declaration
bindings are compiler invariant errors, not empty successful results. Known operation family/category
metadata is total through exhaustive matches: explicit no-family/empty-category
values mean known non-membership. Unclassified facts have unknown membership.
Invalid compiler-owned resource bindings fail their constructor/invariant check;
duplicate/wrong-kind observed resource metadata never proves exact safety.

## 6. Determinism, native integrity and diagnostic evidence

The existing differential subprocess gate runs **8 independent processes** by
default, alternating different working directories and temporary roots. Every
process produces the same canonical v0.2 report with 22 violations, preserving
family/category/operation identity, definite/potential classifications, original
origins and callsite witnesses. Original v0.1 reports also remain identical.
No new subprocess infrastructure or unordered diagnostic aggregation is added.
The broader K2 reproducibility regression remains part of workspace validation.

The native integrity tests derive production registrations and classifier names
from their real tables and compare them with declared std signatures and handler
argument/resource positions. FsRead/FsWrite/IoWrite have the exact bindings in
section 2. No native networking, process or spawn registration is fabricated.
The final-gate regression freezes the distinction between those three production
operation identities and the remaining taxonomy-only targets.

Typed SpecializedContractViolations preserve function identity, canonical target
kind and resources, clause/declaration/attribute byte spans and source identity,
fact operation or unclassified parent, classification, structural TIR origin and
deterministic witness. Broad violations retain precedence; identical reports are
deduplicated without collapsing independent origins. Intermediate instructions
still lack reliable source coordinates. The future bridge remains a lowering
side table `TirLocation → module/source Span`, not invented instruction spans.

The current CLI/E6 boundary loses specialized details: a rejected inspection
renders only `Specialized capability contract violation(s): N`. JSON emits one
generic error (`code: null`, count-only message, empty labels/notes/functions),
not individual target/classification/resource/span/witness objects. Typed Rust
errors retain that information. Successful inspection also omits specialized
clauses/requirements; human output can say `contract: none` for specialized-only
intent. The existing schema is preserved and is not a complete specialized query
surface. Tests prove one JSON document on stdout, empty stderr, stable output
and nonzero exit on rejection; broad structured diagnostics remain unchanged.

## 7. Evidence map and validation

| Area | Permanent evidence |
|---|---|
| Taxonomy, classification, extraction, fixed point and witnesses | [effects tests](../crates/tempra-effects/src/tests.rs) |
| Complete target/resource matching matrix | [specialized matching](../crates/tempra-driver/src/specialized_matching.rs) |
| Canonical targets, syntax distinction, spans, binding collision | [contract tests](../crates/tempra-driver/tests/specialized_deny_contract_tests.rs) |
| Direct/transitive native origins, broad-first, unknowns, inheritance | [enforcement tests](../crates/tempra-driver/tests/specialized_enforcement_tests.rs) |
| Independent-process/cwd parity and actual JIT versus fallback | [differential tests](../crates/tempra-driver/tests/specialized_differential_tests.rs) |
| VM/profile/JIT/module/cache execution boundaries | [compilation boundaries](../crates/tempra-driver/tests/compilation_boundary_tests.rs) |
| Cache trust, stale/malformed fallback, preservation and stable hits | [pipeline cache tests](../crates/tempra-driver/src/pipeline.rs), [cache lifecycle](../crates/tempra-driver/tests/cache_tests.rs) |
| Native registry/classifier/signature/resource integrity | [native boundary tests](../crates/tempra-driver/tests/native_boundary_integrity.rs), [registry coverage](../crates/tempra-native/tests/registry_coverage.rs) |
| Invalid call and parameter/local layout without host panic | [bytecode tests](../crates/tempra-bytecode/src/tests.rs) |
| CLI/E6 projection limits and JSON output contract | [CLI tests](../crates/tempra-cli/tests/effects_cli_tests.rs) |
| REPL transactional rejection | [REPL tests](../crates/tempra-repl/src/repl.rs) |

All final required commands passed in this K4.10 run:

```text
cargo test -p tempra-bytecode
cargo test -p tempra-effects
cargo test -p tempra-native
cargo test -p tempra-driver
cargo test --workspace
cargo test --workspace --features jit-cranelift
cargo fmt --all -- --check
git diff --check
```

Local Markdown links and tracked artifact/status diffs are checked separately.
A dirty worktree is expected: the preexisting K4.9 delta is preserved, and this
gate does not stage, commit or push.

## 8. Remaining risks and completion decision

Deferred work is explicit:

- richer E6 specialized diagnostics and successful inspection projection;
- module/file extend lowering (current annotated path explicitly rejected);
- production native networking and other missing TIR/native origins;
- endpoint/connection identity, aliasing, accepts and transfer attribution;
- runtime guards, if separately specified; none exists today;
- filesystem path normalization/confinement, links, aliases and TOCTOU;
- broader FFI enforcement, native resolver authentication and native sandboxing;
- actor runtime integration and scope semantics;
- reliable instruction-level source provenance and existing broader TIR limits.

The inherited bytecode `is_extern` serialization limitation and general hostile
artifact/resource-exhaustion hardening remain separate format/runtime debts;
file/module native execution is fresh and does not use that source cache.

**K4.10 DONE; the K4 specialized-effects track is complete for this bounded
scope.** All required test/format/diff commands passed. That verdict does not make C operations executable,
promote rejected interpreters to supported paths, complete K3 runtime parity,
or assert filesystem/network/FFI sandbox guarantees.
