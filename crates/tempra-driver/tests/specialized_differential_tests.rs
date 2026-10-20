use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use tempra_driver::{
    check_file, compile_file_to_bytecode_with_analysis,
    compile_file_to_bytecode_with_analysis_with_config, compile_to_bytecode_with_analysis,
    inspect_effects_file, CapabilityContractViolation, CompilationConfig, DriverError,
    EffectQueryError, FunctionCapabilityContract, SpecializedContractViolation, SpecializedMatch,
    VmCompilation,
};
use tempra_effects::{
    Capability, CapabilityAnalysis, CapabilitySet, EffectOperation, EffectParameter,
    LocalEffectFact, ResourceKnowledge, ResourceValue,
};
use tempra_span::Span;
use tempra_tir::ir::TirFunctionId;
use tempra_vm::profile::VmOptions;
use tempra_vm::VmValue;

const BROAD_SOURCE: &str = r#"
@deny(fs.write(destination: "never.txt"))
fn helper(n: Int64) -> Int64 {
    let text = "allocated";
    return n / 2
}

fn main() -> Int64 { return helper(10) }
"#;

const ACCEPTED_SOURCE: &str = r#"
import std.fs;

@deny(fs.write(destination: "allowed.txt"))
pub fn main() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}
"#;

const DEFINITE_SOURCE: &str = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}
"#;

const POTENTIAL_SOURCE: &str = r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 { return helper("blocked.txt") }

fn helper(path: String) -> Int64 {
    let result = std.fs.write_text(path, "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}
"#;

const TRANSITIVE_SOURCE: &str = r#"
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

const BROAD_AND_SPECIALIZED_SOURCE: &str = r#"
import std.fs;

@deny(io, fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result {
        Ok(_) => 0
        Err(_) => 1
    };
}
"#;

const PURE_SOURCE: &str = r#"
import std.io;

@pure
@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 {
    std.io.println("pure output");
    return 0
}
"#;

#[cfg(feature = "jit-cranelift")]
const JIT_SOURCE: &str = r#"
import std.fs;

fn add(a: Int64, b: Int64) -> Int64 { return a + b }

@deny(fs.write(destination: "jit-never.txt"))
pub fn main() -> Int64 {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    var sum = 0
    for i in xs {
        for j in xs {
            sum = add(sum, j)
        }
    }
    return sum - 550
}
"#;

fn rich_source(annotation: &str) -> String {
    format!(
        r#"
import std.fs;

{annotation}
pub fn main() -> Int64 {{
    let direct = direct_origin();
    let transitive = transitive();
    let left = diamond_left();
    let right = diamond_right();
    let recursive = recurse(1);
    let unknown = unknown_caller();
    let exists = std.fs.exists("blocked.txt");
    let result = std.fs.write_text("blocked.txt", "direct");
    return direct + transitive + left + right + recursive + unknown;
}}

fn direct_origin() -> Int64 {{
    let result = std.fs.write_text("blocked.txt", "direct");
    return match result {{
        Ok(_) => 0
        Err(_) => 1
    }};
}}

fn transitive() -> Int64 {{ return transitive_tail() }}

fn transitive_tail() -> Int64 {{
    let result = std.fs.write_text("blocked.txt", "transitive");
    return match result {{
        Ok(_) => 0
        Err(_) => 1
    }};
}}

fn diamond_left() -> Int64 {{ return diamond_leaf() }}
fn diamond_right() -> Int64 {{ return diamond_leaf() }}

fn diamond_leaf() -> Int64 {{
    let result = std.fs.write_text("blocked.txt", "diamond");
    return match result {{
        Ok(_) => 0
        Err(_) => 1
    }};
}}

fn recurse(n: Int64) -> Int64 {{
    if n == 0 {{
        let result = std.fs.write_text("blocked.txt", "recursive");
        return match result {{
            Ok(_) => 0
            Err(_) => 1
        }};
    }}
    return recurse(n - 1);
}}

fn unknown_origin(path: String) -> Int64 {{
    let result = std.fs.write_text(path, "unknown");
    return match result {{
        Ok(_) => 0
        Err(_) => 1
    }};
}}

fn unknown_caller() -> Int64 {{ return unknown_origin("blocked.txt") }}
"#
    )
}

#[derive(Debug, PartialEq, Eq)]
struct ContractObservation {
    explicit_denied: CapabilitySet,
    denied: CapabilitySet,
    specialized_denied: Vec<tempra_driver::SpecializedDenyClause>,
    declaration_span: Span,
    attribute_spans: Vec<Span>,
    pure_span: Option<Span>,
}

impl From<&FunctionCapabilityContract> for ContractObservation {
    fn from(contract: &FunctionCapabilityContract) -> Self {
        Self {
            explicit_denied: contract.explicit_denied,
            denied: contract.denied,
            specialized_denied: contract.specialized_denied.clone(),
            declaration_span: contract.declaration_span,
            attribute_spans: contract.attribute_spans.clone(),
            pure_span: contract.pure_span,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CompilationOutcome {
    Accepted {
        capabilities: CapabilityAnalysis,
        contracts: Vec<(TirFunctionId, FunctionCapabilityContract)>,
        bytecode: Vec<u8>,
    },
    Broad(Vec<CapabilityContractViolation>),
    Specialized(Vec<SpecializedContractViolation>),
    Other(String),
}

#[derive(Clone, Copy)]
enum ExpectedOutcome {
    Accepted,
    Broad,
    Specialized {
        classification: SpecializedMatch,
        witness_len: usize,
        unknown_destination: bool,
    },
}

fn repo_stdlib() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../std")
}

fn copied_stdlib(root: &Path) -> PathBuf {
    let target = root.join("copied-stdlib");
    fs::create_dir_all(&target).unwrap();
    for entry in fs::read_dir(repo_stdlib()).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("tmpr") {
            fs::copy(&path, target.join(entry.file_name())).unwrap();
        }
    }
    target
}

fn write_source(dir: &Path, source: &str) -> PathBuf {
    let path = dir.join("main.tmpr");
    fs::write(&path, source).unwrap();
    path
}

fn compile_outcome(path: &Path, config: &CompilationConfig) -> CompilationOutcome {
    match compile_file_to_bytecode_with_analysis_with_config(path, config) {
        Ok(compilation) => CompilationOutcome::Accepted {
            capabilities: compilation.capabilities().clone(),
            contracts: compilation
                .contracts()
                .map(|(id, contract)| (id, contract.clone()))
                .collect(),
            bytecode: tempra_bytecode::serialize::encode_module(compilation.bytecode()).unwrap(),
        },
        Err(DriverError::CapabilityContractViolations(violations)) => {
            CompilationOutcome::Broad(violations)
        }
        Err(DriverError::SpecializedContractViolations(violations)) => {
            CompilationOutcome::Specialized(violations)
        }
        Err(error) => CompilationOutcome::Other(format!("{error:?}")),
    }
}

fn normalized_function_names(compilation: &VmCompilation) -> Vec<(TirFunctionId, String)> {
    compilation
        .tir()
        .functions
        .iter()
        .map(|function| {
            let name = function
                .name
                .rsplit_once('.')
                .map(|(_, name)| name)
                .unwrap_or(&function.name);
            (function.id, name.to_owned())
        })
        .collect()
}

fn normalized_contracts(compilation: &VmCompilation) -> Vec<(String, ContractObservation)> {
    let names = normalized_function_names(compilation);
    let mut contracts: Vec<_> = compilation
        .contracts()
        .map(|(function, contract)| {
            let name = names
                .iter()
                .find(|(id, _)| *id == function)
                .map(|(_, name)| name.clone())
                .unwrap();
            (name, ContractObservation::from(contract))
        })
        .collect();
    contracts.sort_by(|left, right| left.0.cmp(&right.0));
    contracts
}

fn entry_name(compilation: &VmCompilation) -> &str {
    compilation
        .bytecode()
        .functions
        .iter()
        .map(|function| function.name.as_str())
        .find(|name| *name == "main" || name.ends_with(".main"))
        .unwrap()
}

fn run_entry(compilation: &VmCompilation) -> VmValue {
    tempra_vm::run_function_with_options(
        compilation.bytecode(),
        entry_name(compilation),
        vec![],
        VmOptions::default(),
    )
    .unwrap()
}

#[test]
fn source_and_file_paths_agree_on_broad_facts_witnesses_and_contracts() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = write_source(dir.path(), BROAD_SOURCE);
    let source_compilation =
        compile_to_bytecode_with_analysis(BROAD_SOURCE, &dir.path().join("broad.tbc")).unwrap();
    let file_compilation = compile_file_to_bytecode_with_analysis(&file_path).unwrap();

    assert_eq!(
        source_compilation.capabilities(),
        file_compilation.capabilities(),
        "equivalent source and file paths must retain the same broad analysis"
    );
    let source_contracts = normalized_contracts(&source_compilation);
    assert_eq!(source_contracts, normalized_contracts(&file_compilation));
    assert_eq!(source_contracts.len(), 1);
    assert_eq!(source_contracts[0].0, "helper");
    assert_eq!(
        source_contracts[0].1.specialized_denied[0].operation,
        EffectOperation::FsWrite
    );
    for summary in source_compilation.capabilities().summaries() {
        for capability in summary.required.iter() {
            assert_eq!(
                source_compilation
                    .capabilities()
                    .witness(summary.function, capability),
                file_compilation
                    .capabilities()
                    .witness(summary.function, capability)
            );
        }
    }
    assert_eq!(run_entry(&source_compilation), VmValue::Int64(5));
    assert_eq!(run_entry(&file_compilation), VmValue::Int64(5));
}

#[test]
fn equivalent_file_paths_agree_on_specialized_facts_resources_and_provenance() {
    let source = rich_source("@deny(process.exec(executable: \"/never\"))");
    let dir = tempfile::tempdir().unwrap();
    let path = write_source(dir.path(), &source);
    let explicit_root = copied_stdlib(dir.path());
    let explicit_config = CompilationConfig::with_stdlib_root(&explicit_root).unwrap();

    let default = compile_outcome(&path, &CompilationConfig::default());
    let explicit = compile_outcome(&path, &explicit_config);
    assert_eq!(default, explicit);

    let CompilationOutcome::Accepted {
        capabilities,
        contracts,
        bytecode,
    } = default
    else {
        panic!("valid disjoint specialized contract must be accepted");
    };
    assert!(!bytecode.is_empty());

    let exact_destinations = capabilities
        .local_effect_facts()
        .iter()
        .filter(|fact| match fact {
            LocalEffectFact::Specialized(observed) => observed.resources.iter().any(|resource| {
                resource.parameter == EffectParameter::Destination
                    && resource.knowledge
                        == ResourceKnowledge::Exact(ResourceValue::Text("blocked.txt".into()))
            }),
            LocalEffectFact::UnclassifiedOperation(_) => false,
        })
        .count();
    assert!(
        exact_destinations >= 2,
        "exact direct and transitive facts must retain exact resource knowledge"
    );

    let unknown_destination = capabilities
        .local_effect_facts()
        .iter()
        .find_map(|fact| match fact {
            LocalEffectFact::Specialized(observed) => observed
                .resources
                .iter()
                .find(|resource| {
                    resource.parameter == EffectParameter::Destination
                        && resource.knowledge == ResourceKnowledge::Unknown
                })
                .map(|_| observed),
            LocalEffectFact::UnclassifiedOperation(_) => None,
        })
        .expect("parameter-derived destination must remain unknown");
    assert_eq!(
        unknown_destination.extern_name, "std.fs.write_text",
        "caller literals must not rewrite callee-origin resource knowledge"
    );

    let (_, contract) = contracts
        .iter()
        .find(|(_, contract)| !contract.specialized_denied.is_empty())
        .expect("valid specialized contract must retain provenance");
    assert_eq!(
        contract.specialized_denied[0].operation,
        EffectOperation::ProcessExec
    );
    assert!(contract.specialized_denied[0].span.end > contract.specialized_denied[0].span.start);
}

#[test]
fn equivalent_file_paths_agree_on_accepted_and_rejected_enforcement_outcomes() {
    let cases = [
        ("accepted", ACCEPTED_SOURCE, ExpectedOutcome::Accepted),
        (
            "definite",
            DEFINITE_SOURCE,
            ExpectedOutcome::Specialized {
                classification: SpecializedMatch::DefiniteMatch,
                witness_len: 0,
                unknown_destination: false,
            },
        ),
        (
            "potential",
            POTENTIAL_SOURCE,
            ExpectedOutcome::Specialized {
                classification: SpecializedMatch::PotentialMatch,
                witness_len: 1,
                unknown_destination: true,
            },
        ),
        (
            "transitive",
            TRANSITIVE_SOURCE,
            ExpectedOutcome::Specialized {
                classification: SpecializedMatch::DefiniteMatch,
                witness_len: 1,
                unknown_destination: false,
            },
        ),
        (
            "broad and specialized",
            BROAD_AND_SPECIALIZED_SOURCE,
            ExpectedOutcome::Broad,
        ),
        ("pure compatibility", PURE_SOURCE, ExpectedOutcome::Broad),
    ];

    for (name, source, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        let path = write_source(dir.path(), source);
        let explicit_root = copied_stdlib(dir.path());
        let explicit_config = CompilationConfig::with_stdlib_root(&explicit_root).unwrap();

        let default = compile_outcome(&path, &CompilationConfig::default());
        let explicit = compile_outcome(&path, &explicit_config);
        assert_eq!(default, explicit, "outcome mismatch for {name}");

        match (expected, default) {
            (ExpectedOutcome::Accepted, CompilationOutcome::Accepted { .. }) => {}
            (ExpectedOutcome::Broad, CompilationOutcome::Broad(violations)) => {
                assert_eq!(violations.len(), 1, "broad case {name}");
                assert_eq!(violations[0].capability, Capability::Io);
            }
            (
                ExpectedOutcome::Specialized {
                    classification,
                    witness_len,
                    unknown_destination,
                },
                CompilationOutcome::Specialized(violations),
            ) => {
                assert_eq!(violations.len(), 1, "specialized case {name}");
                let violation = &violations[0];
                assert_eq!(violation.classification, classification, "{name}");
                assert_eq!(violation.witness.calls.len(), witness_len, "{name}");
                assert_eq!(violation.witness.origin, violation.fact, "{name}");
                assert_eq!(
                    violation.fact.location(),
                    violation.witness.origin.location()
                );
                if unknown_destination {
                    let LocalEffectFact::Specialized(observed) = &violation.fact else {
                        panic!("{name} must retain a specialized fact");
                    };
                    assert!(observed.resources.iter().any(|resource| {
                        resource.parameter == EffectParameter::Destination
                            && resource.knowledge == ResourceKnowledge::Unknown
                    }));
                }
            }
            (_, CompilationOutcome::Other(error)) => {
                panic!("unexpected compilation error for {name}: {error}")
            }
            (_, outcome) => panic!("unexpected outcome for {name}: {outcome:?}"),
        }
    }
}

#[test]
fn specialized_violations_are_deterministic_across_fresh_compilations() {
    let source = rich_source("@deny(fs.write(destination: \"blocked.txt\"))");
    let dir = tempfile::tempdir().unwrap();
    let path = write_source(dir.path(), &source);
    let explicit_root = copied_stdlib(dir.path());
    let explicit_config = CompilationConfig::with_stdlib_root(&explicit_root).unwrap();
    let default_config = CompilationConfig::default();

    let mut baseline: Option<Vec<SpecializedContractViolation>> = None;
    for run in 0..6 {
        let config = if run % 2 == 0 {
            &default_config
        } else {
            &explicit_config
        };
        let outcome = compile_outcome(&path, config);
        let CompilationOutcome::Specialized(violations) = outcome else {
            panic!("rich fixture must be rejected, got {outcome:?}");
        };
        if let Some(expected) = &baseline {
            assert_eq!(
                expected, &violations,
                "fresh run {run} changed observations"
            );
        } else {
            baseline = Some(violations);
        }
    }

    let violations = baseline.unwrap();
    assert_eq!(violations.len(), 7);
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.classification == SpecializedMatch::DefiniteMatch)
            .count(),
        5
    );
    assert_eq!(
        violations
            .iter()
            .filter(|violation| violation.classification == SpecializedMatch::PotentialMatch)
            .count(),
        2
    );

    let mut witness_lengths: Vec<_> = violations
        .iter()
        .map(|violation| violation.witness.calls.len())
        .collect();
    witness_lengths.sort_unstable();
    assert_eq!(witness_lengths, vec![0, 0, 1, 1, 2, 2, 2]);

    let unique_origins: HashSet<_> = violations
        .iter()
        .map(|violation| violation.fact.location())
        .collect();
    assert_eq!(
        unique_origins.len(),
        violations.len(),
        "diamond and recursive propagation must not duplicate facts"
    );

    let unknown = violations
        .iter()
        .find(|violation| match &violation.fact {
            LocalEffectFact::Specialized(observed) => observed.resources.iter().any(|resource| {
                resource.parameter == EffectParameter::Destination
                    && resource.knowledge == ResourceKnowledge::Unknown
            }),
            LocalEffectFact::UnclassifiedOperation(_) => false,
        })
        .expect("unknown resource origin must remain visible");
    assert_eq!(unknown.classification, SpecializedMatch::PotentialMatch);
    assert_eq!(unknown.witness.calls.len(), 2);
    assert!(unknown.witness.origin == unknown.fact);

    let unclassified = violations
        .iter()
        .find(|violation| match &violation.fact {
            LocalEffectFact::UnclassifiedOperation(observed) => {
                observed.capability == Capability::Io && observed.extern_name == "std.fs.exists"
            }
            LocalEffectFact::Specialized(_) => false,
        })
        .expect("unclassified fs operation must remain fail-closed");
    assert_eq!(
        unclassified.classification,
        SpecializedMatch::PotentialMatch
    );
    assert!(unclassified.witness.calls.is_empty());

    for violation in &violations {
        assert_eq!(violation.witness.origin, violation.fact);
        assert_eq!(
            violation.fact.location(),
            violation.witness.origin.location()
        );
    }
}

#[test]
fn adding_specialized_classification_does_not_change_broad_facts_or_witnesses() {
    let annotated_source = rich_source("@deny(process.exec(executable: \"/never\"))");
    let plain_source = rich_source("");
    let dir = tempfile::tempdir().unwrap();
    let path = write_source(dir.path(), &annotated_source);
    let annotated = compile_file_to_bytecode_with_analysis(&path).unwrap();
    fs::write(&path, plain_source).unwrap();
    let plain = compile_file_to_bytecode_with_analysis(&path).unwrap();

    assert_eq!(annotated.capabilities(), plain.capabilities());
    assert_eq!(
        tempra_bytecode::serialize::encode_module(annotated.bytecode()).unwrap(),
        tempra_bytecode::serialize::encode_module(plain.bytecode()).unwrap()
    );
    assert_eq!(annotated.contracts().count(), 1);
    assert_eq!(plain.contracts().count(), 0);

    for summary in annotated.capabilities().summaries() {
        for capability in summary.required.iter() {
            assert_eq!(
                annotated
                    .capabilities()
                    .witness(summary.function, capability),
                plain.capabilities().witness(summary.function, capability)
            );
        }
    }
}

#[test]
fn non_executing_check_file_does_not_certify_specialized_enforcement() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_source(dir.path(), DEFINITE_SOURCE);

    check_file(&path).expect("type checking alone must not enforce specialized contracts");
    assert!(matches!(
        compile_file_to_bytecode_with_analysis(&path).unwrap_err(),
        DriverError::SpecializedContractViolations(ref violations) if violations.len() == 1
    ));
    assert!(matches!(
        inspect_effects_file(&path),
        Err(EffectQueryError::Compiler(
            DriverError::SpecializedContractViolations(ref violations)
        )) if violations.len() == 1
    ));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn valid_specialized_program_has_equivalent_vm_and_jit_results() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_source(dir.path(), JIT_SOURCE);
    let compilation = compile_file_to_bytecode_with_analysis(&path).unwrap();
    let entry = entry_name(&compilation).to_owned();

    let mut vm_options = VmOptions::default();
    vm_options.profiling = true;
    let (vm_value, _) = tempra_vm::Interpreter::run_function_profiled_with_resolver(
        compilation.bytecode(),
        &entry,
        vec![],
        vm_options,
        None,
    )
    .unwrap();

    let mut jit_options = VmOptions::default();
    jit_options.profiling = true;
    jit_options.jit_enabled = true;
    jit_options.hot_call_threshold = 10;
    jit_options.hot_backedge_threshold = 10;
    let (jit_value, profile) = tempra_vm::Interpreter::run_function_profiled_with_resolver(
        compilation.bytecode(),
        &entry,
        vec![],
        jit_options,
        None,
    )
    .unwrap();

    assert_eq!(vm_value, VmValue::Int64(0));
    assert_eq!(jit_value, vm_value);
    assert!(
        profile
            .function_stats
            .iter()
            .any(|stats| stats.jit_compile_successes > 0),
        "the JIT-enabled path must actually compile at least one function"
    );
}
