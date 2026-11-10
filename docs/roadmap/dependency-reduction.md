# Cross-cutting dependency reduction — measured audit

**Target:** lower maintenance, cold build, footprint and release costs without
replacing mature OS/Unicode/crypto/LLVM-grade functionality with fragile custom
implementations. Internal `tempra-*` crates are **not** the audit target.

## Inventory inputs

Run from workspace root after recording Rust toolchain and `Cargo.lock`:

```bash
cargo metadata --locked --format-version 1 > dependency-metadata.json
cargo tree --locked --workspace > dependency-tree.txt
cargo tree --locked --workspace -e features > dependency-features.txt
cargo tree --locked --workspace -d > dependency-duplicates.txt
```

Store raw measurements outside the release binaries; decide separately whether
to commit them. Check each external crate's direct users, active features,
transitive versions, build scripts, native libraries, OS/target scope,
security/maintenance and cost of replacement.

## Decision matrix

| Action | Threshold |
| --- | --- |
| Keep | Complex, maintained and correct dependency; no measured benefit to replace |
| Narrow features | Needed crate includes unused or expensive functionality |
| Isolate optional | Backend/platform/experimental component absent from normal Ergal |
| Replace with std/local code | Small, stable feature, trivial to test and maintain |
| Remove | No remaining dependency or execution requirement, verified via graph/tests |

Audit JIT removal early but do not assume removing a Rust workspace member is
sufficient to drop its transitive dependencies. Separate optional LLVM/Wasm and
platform integration from Ergal's shipping dependency graph. Cargo feature
unification means target-specific dep auditing requires the actual target and
feature configuration, not only a text search of `Cargo.toml`.

## Comparison evidence

Before/after: `Cargo.lock` dependency graph, compiler/target hashes, cold and
warm build time, installed binary size, **new-process** start, peak RSS,
platform support, workspace tests and reproducibility. Use K5 methodology;
report both gains and losses. Do not label a smaller dependency count a win
without a measured technical benefit.

## Self-hosting relationship

Reduced external coupling helps later bootstrap, but does **not** mean the
non-AOT VM is self-hosted. LLVM and other externally implemented components
may remain important after the compiler is ported to Tempra.
