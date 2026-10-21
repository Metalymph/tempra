use std::fs;
use tempra_native::registry::{registered_natives, resolve};

#[test]
fn test_registry_coverage() {
    let std_dir = format!("{}/../../std", env!("CARGO_MANIFEST_DIR"));
    let mut missing = vec![];

    // Get all public extern functions from std/*.tmpr
    for entry in fs::read_dir(std_dir).expect("failed to read std dir") {
        let entry = entry.expect("failed to read entry");
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("tmpr") {
            let file_stem = path.file_stem().unwrap().to_str().unwrap();
            let mod_name = format!("std.{}", file_stem);
            let content = fs::read_to_string(&path).expect("failed to read file");

            for line in content.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("pub extern fn ") {
                    let func_name = rest.split('(').next().unwrap().trim();
                    if resolve(&mod_name, func_name).is_none() {
                        missing.push(format!("{}::{}", mod_name, func_name));
                    }
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "Missing native registry targets for: {:?}",
        missing
    );
}

#[test]
fn net_transfer_operations_have_no_native_bindings() {
    // `net.send` and `net.receive` are taxonomy-only in v0.2. The native
    // registry is a separate compiler-owned table; no network implementation or
    // registration may appear without an explicit classifier operation.
    assert!(registered_natives().all(|(module, _)| module != "std.net"));
    for function in ["connect", "listen", "send", "receive"] {
        assert!(
            resolve("std.net", function).is_none(),
            "std.net.{function} must not gain an implicit native binding"
        );
    }
}

#[test]
fn test_api_consistency() {
    let registry_file = format!("{}/src/registry.rs", env!("CARGO_MANIFEST_DIR"));
    let content = fs::read_to_string(&registry_file).expect("failed to read registry.rs");

    // Quick heuristic: find all ("std.xxx", "yyy") strings in registry.rs
    // and ensure they exist in the corresponding std.tmpr file
    let mut declared_in_registry = vec![];
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("(\"std.") {
            // Parse ("std.mod", "func")
            let parts: Vec<&str> = line.split('"').collect();
            if parts.len() >= 4 {
                let mod_name = parts[1]; // e.g. std.fs
                let func_name = parts[3]; // e.g. exists
                declared_in_registry.push((mod_name.to_string(), func_name.to_string()));
            }
        }
    }

    let std_dir = format!("{}/../../std", env!("CARGO_MANIFEST_DIR"));
    let mut missing_in_std = vec![];

    for (mod_name, func_name) in declared_in_registry {
        let module_part = mod_name.strip_prefix("std.").unwrap();
        let tmpr_path = format!("{}/{}.tmpr", std_dir, module_part);
        let content = fs::read_to_string(&tmpr_path)
            .unwrap_or_else(|_| panic!("Missing module file: {}", tmpr_path));

        let expected_decl = format!("pub extern fn {}(", func_name);
        if !content.lines().any(|l| l.contains(&expected_decl)) {
            missing_in_std.push(format!("{}::{}", mod_name, func_name));
        }
    }

    assert!(
        missing_in_std.is_empty(),
        "Functions in registry not declared in std/*.tmpr: {:?}",
        missing_in_std
    );
}
