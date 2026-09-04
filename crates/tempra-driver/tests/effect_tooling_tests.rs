use tempra_driver::{
    inspect_effects_file, ContractSource, EffectContractOrigin, EffectQueryError,
    CAPABILITY_DENIED_CODE,
};
use tempra_effects::Capability;

fn write_app(source: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.tmpr");
    std::fs::write(&path, source).unwrap();
    (dir, path)
}

fn diagnostics(source: &str) -> Vec<tempra_driver::EffectDiagnostic> {
    let (_dir, path) = write_app(source);
    let EffectQueryError::Diagnostics(diagnostics) = inspect_effects_file(&path).unwrap_err()
    else {
        panic!("expected projected capability diagnostics")
    };
    diagnostics
}

#[test]
fn explicit_and_pure_intent_are_derived_per_capability() {
    let explicit = diagnostics(
        "import std.io; @deny(io) pub fn f() { std.io.println(\"x\") } pub fn main() {}",
    );
    assert_eq!(explicit.len(), 1);
    let diagnostic = &explicit[0];
    assert_eq!(diagnostic.code, CAPABILITY_DENIED_CODE);
    assert_eq!(diagnostic.capability, Capability::Io);
    assert_eq!(diagnostic.function.display_name, "app.f");
    assert_eq!(diagnostic.contract_origins, [EffectContractOrigin::Deny]);
    assert!(diagnostic.contract.explicit_denied.contains(Capability::Io));
    assert!(diagnostic.contract.pure_span.is_none());
    assert!(matches!(
        diagnostic.contract.source,
        ContractSource::Module { .. }
    ));
    assert_eq!(diagnostic.witness.calls.len(), 0);
    assert_eq!(
        diagnostic
            .witness
            .origin
            .external
            .as_ref()
            .unwrap()
            .display_name,
        "std.io.println"
    );
    assert_eq!(
        diagnostic.to_diagnostic().code.as_deref(),
        Some(CAPABILITY_DENIED_CODE)
    );

    let pure =
        diagnostics("import std.io; @pure pub fn f() { std.io.println(\"x\") } pub fn main() {}");
    assert_eq!(pure[0].contract_origins, [EffectContractOrigin::Pure]);
    assert!(!pure[0].contract.explicit_denied.contains(Capability::Io));
    assert!(pure[0].contract.pure_span.is_some());
    assert!(pure[0].to_diagnostic().message.starts_with("pure function"));

    let both = diagnostics(
        "import std.io; @pure @deny(io) pub fn f() { std.io.println(\"x\") } pub fn main() {}",
    );
    assert_eq!(
        both[0].contract_origins,
        [EffectContractOrigin::Deny, EffectContractOrigin::Pure]
    );
}

#[test]
fn transitive_projection_reuses_the_named_e1_witness() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("inner.tmpr"),
        "import std.io; pub fn work() { std.io.println(\"x\") }",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("middle.tmpr"),
        "import inner; pub fn work() { inner.work() }",
    )
    .unwrap();
    let path = dir.path().join("outer.tmpr");
    std::fs::write(
        &path,
        "import middle; @deny(io) pub fn outer() { middle.work() } pub fn main() {}",
    )
    .unwrap();
    let EffectQueryError::Diagnostics(first) = inspect_effects_file(&path).unwrap_err() else {
        panic!()
    };
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].witness.calls.len(), 2);
    assert_eq!(
        first[0].witness_path(),
        "outer.outer -> middle.work -> inner.work -> std.io.println -> io"
    );
    for _ in 0..4 {
        let EffectQueryError::Diagnostics(next) = inspect_effects_file(&path).unwrap_err() else {
            panic!()
        };
        assert_eq!(next, first);
    }
}

#[test]
fn valid_inspection_separates_facts_contracts_and_pure_intent() {
    let (_dir, path) = write_app(
        r#"
import std.io;
@deny(process)
pub fn logging() { std.io.println("x") }
@pure
fn local() -> String { let n = 10 / 2; return "allocated" }
pub fn main() {}
"#,
    );
    let inspection = inspect_effects_file(&path).unwrap();
    assert!(inspection
        .functions
        .windows(2)
        .all(|pair| pair[0].function.id.0 < pair[1].function.id.0));

    let logging = inspection
        .functions
        .iter()
        .find(|function| function.function.display_name == "app.logging")
        .unwrap();
    assert!(logging.direct.contains(Capability::Io));
    assert!(logging.required.contains(Capability::Io));
    let contract = logging.contract.as_ref().unwrap();
    assert!(contract.explicit_denied.contains(Capability::Process));
    assert!(!contract.denied.contains(Capability::Io));

    let local = inspection
        .functions
        .iter()
        .find(|function| function.function.display_name == "app.local")
        .unwrap();
    assert!(local.required.contains(Capability::Alloc));
    assert!(local.required.contains(Capability::Panic));
    let pure = local.contract.as_ref().unwrap();
    assert!(pure.explicit_denied.iter().next().is_none());
    assert!(pure.pure_span.is_some());
    for capability in [
        Capability::Io,
        Capability::Process,
        Capability::Spawn,
        Capability::Nondeterminism,
    ] {
        assert!(pure.denied.contains(capability));
    }
    assert!(!pure.denied.contains(Capability::Alloc));
    assert!(!pure.denied.contains(Capability::Panic));

    let main = inspection
        .functions
        .iter()
        .find(|function| function.function.display_name == "app.main")
        .unwrap();
    assert!(main.contract.is_none());
}

#[test]
fn valid_inspection_keeps_direct_and_transitive_requirements_distinct() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("inner.tmpr"),
        "import std.io; pub fn work() { std.io.println(\"x\") }",
    )
    .unwrap();
    let path = dir.path().join("outer.tmpr");
    std::fs::write(
        &path,
        "import inner; @deny(process) pub fn outer() { inner.work() } pub fn main() {}",
    )
    .unwrap();

    let inspection = inspect_effects_file(&path).unwrap();
    let outer = inspection
        .functions
        .iter()
        .find(|function| function.function.display_name == "outer.outer")
        .unwrap();
    assert!(outer.direct.iter().next().is_none());
    assert_eq!(
        outer.required.iter().collect::<Vec<_>>(),
        vec![Capability::Alloc, Capability::Io]
    );
    assert_eq!(outer.requirements.len(), 2);
    let io = outer
        .requirements
        .iter()
        .find(|requirement| requirement.capability == Capability::Io)
        .unwrap();
    assert_eq!(io.witness.calls.len(), 1);
    assert_eq!(
        io.witness.origin.external.as_ref().unwrap().display_name,
        "std.io.println"
    );
}

#[test]
fn inspection_is_fresh_and_does_not_create_or_replace_vm_cache() {
    struct RestoreCache {
        path: std::path::PathBuf,
        previous: Option<Vec<u8>>,
    }
    impl Drop for RestoreCache {
        fn drop(&mut self) {
            if let Some(bytes) = &self.previous {
                let _ = std::fs::write(&self.path, bytes);
            } else {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    let source = "// e6 fresh inspection cache gate\npub fn main() -> Int64 { return 42 }";
    let (_dir, path) = write_app(source);
    let cache_path =
        tempra_driver::cache::get_cache_path(&tempra_driver::cache::compute_source_hash(source));
    let previous = std::fs::read(&cache_path).ok();
    let _restore = RestoreCache {
        path: cache_path.clone(),
        previous: previous.clone(),
    };
    inspect_effects_file(&path).unwrap();
    assert_eq!(std::fs::read(&cache_path).ok(), previous);
}

#[test]
fn multiple_diagnostics_preserve_enforcement_order() {
    let source = r#"
import std.io;
@pure @deny(alloc, panic)
fn first() { std.io.println("x"); let n = 10 / 2 }
@pure
fn second() { std.io.println("y") }
pub fn main() {}
"#;
    let (_dir, path) = write_app(source);
    let EffectQueryError::Diagnostics(first) = inspect_effects_file(&path).unwrap_err() else {
        panic!()
    };
    assert_eq!(
        first.iter().map(|item| item.capability).collect::<Vec<_>>(),
        vec![
            Capability::Alloc,
            Capability::Panic,
            Capability::Io,
            Capability::Io
        ]
    );
    let keys = first
        .iter()
        .map(|item| (item.function.id.0, item.capability as u8))
        .collect::<Vec<_>>();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    let EffectQueryError::Diagnostics(second) = inspect_effects_file(&path).unwrap_err() else {
        panic!()
    };
    assert_eq!(second, first);
}
