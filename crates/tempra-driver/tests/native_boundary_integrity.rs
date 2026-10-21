//! Cross-crate regression tests for the production native boundary.
//!
//! The native registry and the effects classifier are separate explicit tables
//! by design. These tests derive their names from those production tables (never
//! from a second copy) and compare them with the declared `std/*.tmpr` signatures
//! and with the actual native handlers.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use tempra_driver::{compile_to_bytecode_with_analysis, DriverError};
use tempra_effects::{
    known_externs, Capability, CapabilityAnalysisError, CapabilitySet, EffectCategory,
    EffectFamily, EffectOperation, EffectParameter, KnownExtern,
};
use tempra_native::registry::{registered_natives, resolve};
use tempra_vm::value::VmValue;

#[derive(Debug, Clone)]
struct DeclaredParameter {
    name: String,
    ty: String,
}

#[derive(Debug, Clone)]
struct DeclaredSignature {
    parameters: Vec<DeclaredParameter>,
    return_type: String,
}

fn std_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std")
}

fn declared_signatures() -> BTreeMap<String, DeclaredSignature> {
    let mut signatures = BTreeMap::new();

    for entry in fs::read_dir(std_directory()).expect("std directory must be readable") {
        let entry = entry.expect("std entry must be readable");
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("tmpr") {
            continue;
        }

        let module = format!(
            "std.{}",
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .expect("std module must have a UTF-8 stem")
        );
        let source = fs::read_to_string(&path).expect("std module must be readable");

        for line in source.lines() {
            let line = line.trim();
            let Some(declaration) = line.strip_prefix("pub extern fn ") else {
                continue;
            };

            let open = declaration
                .find('(')
                .unwrap_or_else(|| panic!("missing parameter list in {line}"));
            let close = declaration[open..]
                .find(')')
                .map(|offset| open + offset)
                .unwrap_or_else(|| panic!("missing closing parenthesis in {line}"));
            let function = declaration[..open].trim();
            let parameters = parse_parameters(&declaration[open + 1..close]);
            let tail = declaration[close + 1..].trim();
            let return_type = tail
                .strip_prefix("->")
                .map(str::trim)
                .unwrap_or("Unit")
                .to_string();
            let name = format!("{module}.{function}");
            let signature = DeclaredSignature {
                parameters,
                return_type,
            };

            assert!(
                signatures.insert(name.clone(), signature).is_none(),
                "duplicate std signature for {name}"
            );
        }
    }

    signatures
}

fn parse_parameters(parameters: &str) -> Vec<DeclaredParameter> {
    if parameters.trim().is_empty() {
        return Vec::new();
    }

    parameters
        .split(',')
        .map(|parameter| {
            let (name, ty) = parameter
                .split_once(':')
                .unwrap_or_else(|| panic!("missing parameter type in `{parameter}`"));
            DeclaredParameter {
                name: name.trim().to_string(),
                ty: ty.trim().to_string(),
            }
        })
        .collect()
}

fn registered_native_names() -> BTreeMap<String, (&'static str, &'static str)> {
    let mut names = BTreeMap::new();
    for (module, function) in registered_natives() {
        let name = format!("{module}.{function}");
        assert!(
            names.insert(name.clone(), (module, function)).is_none(),
            "duplicate production native registration for {name}"
        );
    }
    names
}

fn known_extern_names() -> BTreeMap<String, KnownExtern> {
    let mut names = BTreeMap::new();
    for extern_definition in known_externs() {
        let name = extern_definition.name.to_string();
        assert!(
            names.insert(name.clone(), extern_definition).is_none(),
            "duplicate compiler extern classification for {name}"
        );
    }
    names
}

#[test]
fn registry_classifier_and_declared_signatures_agree_exactly() {
    let signatures = declared_signatures();
    let registered = registered_native_names();
    let classified = known_extern_names();

    let registered_names: BTreeSet<_> = registered.keys().collect();
    let classified_names: BTreeSet<_> = classified.keys().collect();
    let declared_names: BTreeSet<_> = signatures.keys().collect();

    assert_eq!(
        registered_names, classified_names,
        "every production native must have exactly one explicit compiler classification"
    );
    assert_eq!(
        registered_names, declared_names,
        "the production native registry and std signatures must stay in exact agreement"
    );

    for (name, (module, function)) in &registered {
        assert!(
            resolve(module, function).is_some(),
            "{name} must resolve through the production native registry"
        );

        let classification = classified
            .get(name)
            .unwrap_or_else(|| panic!("{name} must have an explicit classification"));

        // `capabilities` is a value, including the empty set, so the entry is an
        // explicit classification even when no broad capability is required.
        if let Some(operation) = classification.operation {
            assert!(
                classification.capabilities.contains(operation.parent()),
                "{name} must require the broad parent capability for {operation:?}"
            );
        }
    }

    // Capability-free natives are classified explicitly, not omitted.
    for name in [
        "std.strings.contains",
        "std.strings.starts_with",
        "std.strings.ends_with",
    ] {
        assert_eq!(
            classified[name].capabilities,
            CapabilitySet::empty(),
            "{name} must remain explicitly capability-free"
        );
    }

    // `exists` deliberately keeps broad Io without a specialized operation.
    let exists = &classified["std.fs.exists"];
    assert_eq!(
        exists.capabilities,
        CapabilitySet::singleton(Capability::Io)
    );
    assert_eq!(exists.operation, None);
    assert!(exists.resources.is_empty());
}

#[test]
fn final_gate_distinguishes_production_operations_from_taxonomy_only_targets() {
    let known: Vec<_> = known_externs().collect();
    let reachable: Vec<_> = EffectOperation::ALL
        .into_iter()
        .filter(|operation| {
            known
                .iter()
                .any(|external| external.operation == Some(*operation))
        })
        .collect();
    assert_eq!(
        reachable,
        vec![
            EffectOperation::FsRead,
            EffectOperation::FsWrite,
            EffectOperation::IoWrite
        ]
    );
    assert_eq!(
        known
            .iter()
            .filter(|external| external.operation.is_some())
            .count(),
        4
    );
    for (module, _) in registered_natives() {
        assert!(!matches!(
            module,
            "std.net" | "std.process" | "std.spawn" | "std.task" | "std.actor"
        ));
    }
    for (module, functions) in [
        ("std.net", &["connect", "listen", "send", "receive"][..]),
        ("std.process", &["exec", "exit"][..]),
        ("std.spawn", &["task", "actor"][..]),
    ] {
        for function in functions {
            assert!(resolve(module, function).is_none());
        }
    }
}

#[test]
fn native_classifier_exposes_canonical_family_and_category_metadata() {
    let classified = known_extern_names();
    let expected = [
        (
            "std.fs.read_text",
            EffectOperation::FsRead,
            Some(EffectFamily::Fs),
            &[EffectCategory::IoRead][..],
        ),
        (
            "std.fs.write_text",
            EffectOperation::FsWrite,
            Some(EffectFamily::Fs),
            &[EffectCategory::IoWrite][..],
        ),
        (
            "std.io.print",
            EffectOperation::IoWrite,
            None,
            &[EffectCategory::IoWrite][..],
        ),
        (
            "std.io.println",
            EffectOperation::IoWrite,
            None,
            &[EffectCategory::IoWrite][..],
        ),
    ];

    for (name, operation, family, categories) in expected {
        let classification = classified
            .get(name)
            .unwrap_or_else(|| panic!("missing classification for {name}"));
        assert_eq!(classification.operation, Some(operation), "{name}");
        assert!(
            classification.capabilities.contains(operation.parent()),
            "{name} must require the operation's broad parent capability"
        );
        assert_eq!(operation.family(), family, "{name}");
        assert_eq!(operation.categories(), categories, "{name}");
    }

    // A known native without a specialized operation retains its broad Io
    // classification and never receives a guessed fs/read categorization.
    let exists = &classified["std.fs.exists"];
    assert_eq!(
        exists.capabilities,
        CapabilitySet::singleton(Capability::Io)
    );
    assert_eq!(exists.operation, None);
    assert!(exists.resources.is_empty());

    // `net.send` and `net.receive` are taxonomy-only and must not acquire a
    // native classifier mapping implicitly.
    assert!(classified.values().all(|classification| {
        classification.operation != Some(EffectOperation::NetSend)
            && classification.operation != Some(EffectOperation::NetReceive)
    }));
    assert!(!classified.keys().any(|name| name.starts_with("std.net.")));
}

#[test]
fn specialized_bindings_follow_declared_signatures_and_native_positions() {
    let signatures = declared_signatures();
    let classified = known_extern_names();

    let expected_signatures = [
        ("std.fs.read_text", &[("path", "String")][..], "String!"),
        (
            "std.fs.write_text",
            &[("path", "String"), ("text", "String")][..],
            "Unit!",
        ),
        ("std.io.print", &[("value", "String")][..], "Unit"),
        ("std.io.println", &[("value", "String")][..], "Unit"),
    ];

    for (name, expected_parameters, expected_return) in expected_signatures {
        let signature = signatures
            .get(name)
            .unwrap_or_else(|| panic!("missing declared signature for {name}"));
        let actual_parameters: Vec<_> = signature
            .parameters
            .iter()
            .map(|parameter| (parameter.name.as_str(), parameter.ty.as_str()))
            .collect();

        assert_eq!(actual_parameters, expected_parameters, "{name}");
        assert_eq!(signature.return_type, expected_return, "{name}");
    }

    let expected_bindings = [
        (
            "std.fs.read_text",
            EffectOperation::FsRead,
            vec![(EffectParameter::Source, Some(0))],
        ),
        (
            "std.fs.write_text",
            EffectOperation::FsWrite,
            vec![(EffectParameter::Destination, Some(0))],
        ),
        (
            "std.io.print",
            EffectOperation::IoWrite,
            vec![(EffectParameter::Stream, None)],
        ),
        (
            "std.io.println",
            EffectOperation::IoWrite,
            vec![(EffectParameter::Stream, None)],
        ),
    ];

    for (name, operation, expected_resources) in expected_bindings {
        let classification = classified
            .get(name)
            .unwrap_or_else(|| panic!("missing classification for {name}"));
        assert_eq!(classification.operation, Some(operation), "{name}");

        let actual_resources: Vec<_> = classification
            .resources
            .iter()
            .map(|resource| (resource.parameter, resource.argument_index))
            .collect();
        assert_eq!(actual_resources, expected_resources, "{name}");

        let signature = &signatures[name];
        for resource in &classification.resources {
            if let Some(argument_index) = resource.argument_index {
                assert!(
                    argument_index < signature.parameters.len(),
                    "{name} binds {resource:?} beyond its declared arguments"
                );
                assert_eq!(
                    signature.parameters[argument_index].ty, "String",
                    "{name} resource position must remain a string path/value"
                );
            }
        }
    }

    // `write_text` argument 1 is content, never a second resource.
    assert_eq!(signatures["std.fs.write_text"].parameters[1].name, "text");
    assert_eq!(classified["std.fs.write_text"].resources.len(), 1);

    // `print`/`println` have no stream argument; the stream stays unknown.
    for name in ["std.io.print", "std.io.println"] {
        assert_eq!(signatures[name].parameters.len(), 1);
        assert_eq!(
            classified[name].resources,
            vec![tempra_effects::KnownExternResource {
                parameter: EffectParameter::Stream,
                argument_index: None,
            }]
        );
    }
}

#[test]
fn native_handlers_honor_verified_resource_positions() {
    let directory = tempfile::tempdir().unwrap();

    let input = directory.path().join("input.txt");
    fs::write(&input, "native-boundary").unwrap();
    let read_text = resolve("std.fs", "read_text").expect("read_text must resolve");
    let read_result = read_text(&[VmValue::String(input.to_string_lossy().into_owned())])
        .expect("read_text must accept one string path");
    assert_eq!(
        read_result,
        VmValue::Enum(
            "Result".to_string(),
            "Ok".to_string(),
            vec![VmValue::String("native-boundary".to_string())],
        )
    );

    let output = directory.path().join("output.txt");
    let write_text = resolve("std.fs", "write_text").expect("write_text must resolve");
    let write_result = write_text(&[
        VmValue::String(output.to_string_lossy().into_owned()),
        VmValue::String("written-by-argument-one".to_string()),
    ])
    .expect("write_text must accept a path followed by content");
    assert_eq!(
        write_result,
        VmValue::Enum("Result".to_string(), "Ok".to_string(), vec![VmValue::Unit],)
    );
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        "written-by-argument-one",
        "write_text must use argument 0 as the path and argument 1 as content"
    );

    for function in ["print", "println"] {
        let print = resolve("std.io", function).expect("io handler must resolve");
        assert_eq!(
            print(&[VmValue::String("native boundary".to_string())])
                .expect("io handler must accept one string value"),
            VmValue::Unit,
            "{function} must not require a hidden stream argument"
        );
    }
}

#[test]
fn every_registered_native_rejects_wrong_argument_count_and_types() {
    let signatures = declared_signatures();
    let registered = registered_native_names();

    for (name, (module, function)) in registered {
        let signature = signatures
            .get(&name)
            .unwrap_or_else(|| panic!("missing declared signature for {name}"));
        let handler = resolve(module, function).expect("registered native must resolve");
        let arity = signature.parameters.len();
        assert!(arity > 0, "{name} unexpectedly declares no arguments");

        let valid_arguments: Vec<_> = signature
            .parameters
            .iter()
            .map(|parameter| {
                assert_eq!(
                    parameter.ty, "String",
                    "{name} changed its native boundary type; update the test data"
                );
                VmValue::String("x".to_string())
            })
            .collect();

        let too_few = &valid_arguments[..arity - 1];
        assert!(
            handler(too_few).is_err(),
            "{name} accepted too few arguments"
        );

        let mut too_many = valid_arguments.clone();
        too_many.push(VmValue::String("x".to_string()));
        assert!(
            handler(&too_many).is_err(),
            "{name} accepted too many arguments"
        );

        for index in 0..arity {
            let mut wrong_types = valid_arguments.clone();
            wrong_types[index] = VmValue::Int64(0);
            assert!(
                handler(&wrong_types).is_err(),
                "{name} accepted a non-String argument at position {index}"
            );
        }
    }
}

#[test]
fn unknown_natives_and_externs_remain_fail_closed() {
    assert!(resolve("std.fs", "read").is_none());
    assert!(resolve("vendor", "unknown").is_none());
    assert!(!known_extern_names().contains_key("std.fs.read"));

    let dir = tempfile::tempdir().unwrap();
    let error = compile_to_bytecode_with_analysis(
        r#"
pub extern fn vendor_unknown() -> Int64
fn main() -> Int64 { return vendor_unknown() }
"#,
        &dir.path().join("fresh.tbc"),
    )
    .unwrap_err();

    match error {
        DriverError::CapabilityAnalysisError(CapabilityAnalysisError::UnclassifiedExtern {
            name,
            ..
        }) => assert_eq!(name, "vendor_unknown"),
        error => panic!("unexpected error: {error:?}"),
    }
}
