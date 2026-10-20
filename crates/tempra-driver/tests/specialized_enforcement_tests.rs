use std::fs;
use tempra_driver::{
    ContractSource, DriverError, EffectQueryError, SpecializedContractViolation, SpecializedMatch,
    VmCompilation,
};
use tempra_effects::{
    Capability, EffectOperation, EffectParameter, LocalEffectFact, ResourceKnowledge, ResourceValue,
};

fn compile(source: &str) -> Result<VmCompilation, DriverError> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.tmpr");
    fs::write(&path, source).unwrap();
    tempra_driver::compile_file_to_bytecode_with_analysis(&path)
}

fn violations(source: &str) -> Vec<SpecializedContractViolation> {
    match compile(source).unwrap_err() {
        DriverError::SpecializedContractViolations(violations) => violations,
        error => panic!("expected specialized contract violations, got {error:?}"),
    }
}

fn specialized_fact(violation: &SpecializedContractViolation) -> &LocalEffectFact {
    &violation.fact
}

fn write_source(contract: &str, destination: &str) -> String {
    format!(
        r#"
import std.fs;

{contract}
pub fn main() -> Int64 {{
    let result = std.fs.write_text("{destination}", "data");
    return match result {{
        Ok(_) => 0
        Err(_) => 1
    }};
}}
"#
    )
}

#[test]
fn direct_exact_specialized_violation_retains_all_provenance() {
    let source = write_source(
        r#"@deny(fs.write(destination: "blocked.txt"))"#,
        "blocked.txt",
    );
    let violations = violations(&source);
    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.classification, SpecializedMatch::DefiniteMatch);
    assert_eq!(violation.clause.operation, EffectOperation::FsWrite);
    assert_eq!(
        &source[violation.clause.span.start as usize..violation.clause.span.end as usize],
        "fs.write(destination: \"blocked.txt\")"
    );
    assert_eq!(
        &source[violation.clause.parameters[0].span.start as usize
            ..violation.clause.parameters[0].span.end as usize],
        "destination: \"blocked.txt\""
    );
    assert_eq!(
        &source[violation.contract.declaration_span.start as usize
            ..violation.contract.declaration_span.end as usize],
        "fn main() -> Int64 {\n    let result = std.fs.write_text(\"blocked.txt\", \"data\");\n    return match result {\n        Ok(_) => 0\n        Err(_) => 1\n    };\n}"
    );
    assert_eq!(violation.contract.attribute_spans.len(), 1);
    assert_eq!(
        &source[violation.contract.attribute_spans[0].start as usize
            ..violation.contract.attribute_spans[0].end as usize],
        "@deny(fs.write(destination: \"blocked.txt\"))"
    );
    assert!(matches!(
        &violation.contract.source,
        ContractSource::Module { path, .. } if path.ends_with("main.tmpr")
    ));

    let LocalEffectFact::Specialized(fact) = specialized_fact(violation) else {
        panic!("expected a specialized fact")
    };
    assert_eq!(fact.operation, EffectOperation::FsWrite);
    assert_eq!(fact.resources.len(), 1);
    assert_eq!(fact.resources[0].parameter, EffectParameter::Destination);
    assert_eq!(
        fact.resources[0].knowledge,
        ResourceKnowledge::Exact(ResourceValue::Text("blocked.txt".to_owned()))
    );
    assert_eq!(fact.extern_name, "std.fs.write_text");
    assert_eq!(violation.function, fact.location.function);
    assert!(violation.witness.calls.is_empty());
    assert_eq!(violation.witness.origin, violation.fact);
    assert_eq!(
        violation.contract.denied,
        tempra_effects::CapabilitySet::empty()
    );
    assert_eq!(
        violation.contract.explicit_denied,
        tempra_effects::CapabilitySet::empty()
    );
}

#[test]
fn exact_disjoint_resource_is_not_a_violation() {
    let source = write_source(
        r#"@deny(fs.write(destination: "allowed.txt"))"#,
        "blocked.txt",
    );
    let compilation = compile(&source).unwrap();
    let main = compilation
        .tir()
        .functions
        .iter()
        .find(|function| function.name == "main" || function.name.ends_with(".main"))
        .unwrap();
    let contract = compilation.contract(main.id).unwrap();
    assert_eq!(contract.specialized_denied.len(), 1);
    assert_eq!(
        contract.specialized_denied[0].operation,
        EffectOperation::FsWrite
    );
}

#[test]
fn wildcard_destination_matches_an_exact_observed_resource() {
    let source = write_source(r#"@deny(fs.write(destination: *))"#, "blocked.txt");
    let violations = violations(&source);
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0].classification,
        SpecializedMatch::DefiniteMatch
    );
}

#[test]
fn unknown_resource_is_potential_without_interprocedural_substitution() {
    let source = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
fn check(path: String) -> Int64 {
    let result = std.fs.write_text(path, "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}

pub fn main() -> Int64 { return check("blocked.txt") }
"#;
    let violations = violations(source);
    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.classification, SpecializedMatch::PotentialMatch);
    let LocalEffectFact::Specialized(fact) = &violation.fact else {
        panic!("expected a specialized fact")
    };
    assert_eq!(
        fact.resources[0].knowledge,
        ResourceKnowledge::Unknown,
        "the caller's literal must not be substituted into the callee"
    );
    assert!(violation.witness.calls.is_empty());
}

#[test]
fn unclassified_operation_matches_the_same_parent_capability() {
    let source = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 {
    let exists = std.fs.exists("blocked.txt");
    return 0;
}
"#;
    let violations = violations(source);
    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.classification, SpecializedMatch::PotentialMatch);
    let LocalEffectFact::UnclassifiedOperation(fact) = &violation.fact else {
        panic!("expected an unclassified operation fact")
    };
    assert_eq!(fact.capability, Capability::Io);
    assert_eq!(fact.extern_name, "std.fs.exists");
}

#[test]
fn unclassified_operation_with_a_different_parent_is_disjoint() {
    let source = r#"
import std.fs;

@deny(process.exec(executable: "/bin/rm"))
pub fn main() -> Int64 {
    let exists = std.fs.exists("blocked.txt");
    return 0;
}
"#;
    compile(source).unwrap();
}

#[test]
fn transitive_specialized_violation_uses_the_existing_shortest_witness() {
    let source = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 { return helper() }

fn helper() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}
"#;
    let violations = violations(source);
    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.classification, SpecializedMatch::DefiniteMatch);
    assert_eq!(violation.witness.calls.len(), 1);
    assert_eq!(
        violation.witness.calls[0].callee,
        violation.fact.location().function
    );
    assert_eq!(violation.witness.origin, violation.fact);
}

#[test]
fn recursive_effects_are_reported_once_with_their_original_origin() {
    let source = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 { return recurse(0) }

fn recurse(n: Int64) -> Int64 {
    if n == 0 {
        let result = std.fs.write_text("blocked.txt", "data");
        return match result {
            Ok(_) => 0
            Err(_) => 1
        };
    }
    return recurse(n - 1);
}
"#;
    let violations = violations(source);
    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.witness.calls.len(), 1);
    let origin_function = violation.fact.location().function;
    assert_eq!(violation.witness.calls[0].callee, origin_function);
    assert_eq!(violation.witness.origin, violation.fact);
}

#[test]
fn shortest_witness_wins_when_the_same_origin_has_multiple_paths() {
    let source = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 {
    let first = via();
    return leaf();
}

fn via() -> Int64 { return leaf() }

fn leaf() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}
"#;
    let violations = violations(source);
    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(
        violation.witness.calls.len(),
        1,
        "main calls leaf directly as well as through via"
    );
    assert_eq!(
        violation.witness.calls[0].callee,
        violation.fact.location().function
    );
}

#[test]
fn distinct_matching_origins_are_reported_independently() {
    let source = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 { return first() + second() }

fn first() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}

fn second() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result {
        Ok(_) => 1
        Err(_) => 0
    };
}
"#;
    let violations = violations(source);
    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0].clause, violations[1].clause);
    assert_ne!(violations[0].fact.location(), violations[1].fact.location());
    let first = violations[0].fact.location();
    let second = violations[1].fact.location();
    assert!(
        (first.function.0, first.block.0, first.instruction_index)
            < (second.function.0, second.block.0, second.instruction_index)
    );
    for violation in &violations {
        assert_eq!(violation.witness.calls.len(), 1);
        assert_eq!(
            violation.witness.calls[0].callee,
            violation.fact.location().function
        );
    }
}

#[test]
fn mixed_broad_denial_remains_authoritative() {
    let source = write_source(
        r#"@deny(io, fs.write(destination: "blocked.txt"))"#,
        "blocked.txt",
    );
    let DriverError::CapabilityContractViolations(violations) = compile(&source).unwrap_err()
    else {
        panic!("expected the unchanged broad violation variant")
    };
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].capability, Capability::Io);
    assert_eq!(violations[0].contract.specialized_denied.len(), 1);
}

#[test]
fn an_unrelated_broad_denial_does_not_mask_specialized_enforcement() {
    let source = write_source(
        r#"@deny(process, fs.write(destination: "blocked.txt"))"#,
        "blocked.txt",
    );
    let violations = violations(&source);
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0].contract.explicit_denied,
        tempra_effects::CapabilitySet::singleton(Capability::Process)
    );
}

#[test]
fn pure_enforcement_is_unchanged_by_specialized_clauses() {
    let source = r#"
import std.io;

@pure @deny(fs.write(destination: "blocked.txt"))
pub fn main() { std.io.println("blocked") }
"#;
    let DriverError::CapabilityContractViolations(violations) = compile(source).unwrap_err() else {
        panic!("expected the unchanged @pure Io violation")
    };
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].capability, Capability::Io);
    assert_eq!(violations[0].contract.specialized_denied.len(), 1);
}

#[test]
fn generic_specializations_enforce_their_inherited_contract_independently() {
    let source = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn work(type T, first: T, second: T) -> T {
    let result = std.fs.write_text("blocked.txt", "data");
    return first;
}

pub fn main() -> Int64 {
    let ints: Array(Int) = [1, 2];
    let strings: Array(String) = ["a", "b"];
    let int_result = ints.reduce(work);
    let string_result = strings.reduce(work);
    return 42;
}
"#;
    let violations = violations(source);
    assert_eq!(violations.len(), 2);
    assert_ne!(violations[0].function, violations[1].function);
    assert_eq!(violations[0].clause, violations[1].clause);
    assert_eq!(violations[0].contract, violations[1].contract);
    assert_eq!(
        violations[0].contract.declaration_span,
        violations[1].contract.declaration_span
    );
    for violation in &violations {
        assert_eq!(
            violation.contract.specialized_denied[0].span,
            violation.clause.span
        );
        assert_eq!(violation.classification, SpecializedMatch::DefiniteMatch);
    }
}

#[test]
fn effect_inspection_path_enforces_specialized_contracts() {
    let source = write_source(
        r#"@deny(fs.write(destination: "blocked.txt"))"#,
        "blocked.txt",
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.tmpr");
    fs::write(&path, source).unwrap();
    match tempra_driver::inspect_effects_file(&path).unwrap_err() {
        EffectQueryError::Compiler(DriverError::SpecializedContractViolations(violations)) => {
            assert_eq!(violations.len(), 1)
        }
        other => panic!("unexpected inspection error: {other:?}"),
    }
}
