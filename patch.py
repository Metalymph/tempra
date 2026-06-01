import re

with open("crates/tempra-driver/src/pipeline.rs", "r") as f:
    content = f.read()

# Add missing arguments to lower_function calls
content = re.sub(r'lower_function\(([^,]+),\s*([^,]+),\s*([^,]+),\s*None,\s*\)', r'lower_function(\1, \2, \3, None, None)', content)
content = re.sub(r'lower_function\(([^,]+),\s*([^,]+),\s*([^,]+),\s*Some\(([^)]+)\),\s*\)', r'lower_function(\1, \2, \3, Some(\4), None)', content)
content = content.replace("                    &mut module_builder,\n                    None,\n                ) {", "                    &mut module_builder,\n                    None,\n                    None,\n                ) {")
content = content.replace("                        &mut module_builder,\n                        Some(target_ty.clone()),\n                    ) {", "                        &mut module_builder,\n                        Some(target_ty.clone()),\n                        None,\n                    ) {")
content = content.replace("                &mut module_builder,\n                None,\n            ) {", "                &mut module_builder,\n                None,\n                None,\n            ) {")

new_funcs = """
pub fn run_file_vm_with_options(
    path: &std::path::Path,
    args: &[String],
    options: tempra_vm::profile::VmOptions,
) -> Result<tempra_vm::value::VmValue, DriverError> {
    let hash = crate::cache::compute_file_hash(path).unwrap_or_else(|_| [0; 32]);
    let cache_path = crate::cache::get_cache_path(&hash);
    let fingerprint = format!("tempra-{}", env!("CARGO_PKG_VERSION"));
    
    let (module, type_ctx) = compile_and_cache_module_graph(path, &hash, &cache_path, &fingerprint)?;
    
    let has_main = module.functions.iter().any(|f| f.name == "main");
    if !has_main {
        return Err(DriverError::UnsupportedExperimentalVm(
            "experimental VM currently requires main()".into(),
        ));
    }
    
    tempra_vm::interpreter::execute(
        &module,
        "main",
        vec![],
        options,
        Some(tempra_native::registry::resolve),
    )
    .map_err(|e| DriverError::VmError(format!("{:?}", e)))
}

pub fn compile_and_cache_module_graph(
    entry_path: &std::path::Path,
    hash: &[u8; 32],
    cache_path: &std::path::Path,
    fingerprint: &str,
) -> Result<(tempra_bytecode::bytecode::BytecodeModule, tempra_typeck::TypeContext), DriverError> {
    use crate::module_graph::{load_module_graph, DiskDocumentLoader};
    use tempra_typeck::{check_modules_always, ModuleToCheck};
    use std::collections::HashMap;

    let loader = DiskDocumentLoader;
    let mut diagnostics = Vec::new();
    let graph = load_module_graph(entry_path, &loader, &mut diagnostics)
        .map_err(|_| DriverError::ModuleLoadError(format!("{:?}", diagnostics)))?;

    let mut cloned_hirs = Vec::new();
    for module_id in &graph.order {
        let module = &graph.modules[module_id];
        cloned_hirs.push((module.name.clone(), module.imports.clone(), module.hir.clone()));
    }

    let mut typeck_modules = Vec::new();
    for (name, imports, hir) in &mut cloned_hirs {
        typeck_modules.push(ModuleToCheck {
            name: name.clone(),
            imports: imports.clone(),
            file: hir,
        });
    }

    let source_texts = HashMap::new();
    let type_ctx = check_modules_always(&mut typeck_modules, &source_texts, false);
    if !type_ctx.errors.is_empty() {
        return Err(DriverError::TypeError(format!("{:?}", type_ctx.errors)));
    }

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();
    let mut errors = Vec::new();

    for (name, _imports, hir) in &cloned_hirs {
        for item in &hir.items {
            if let tempra_hir::HirItem::Function(func) = item {
                let prefix = if name.is_empty() { None } else { Some(name.as_str()) };
                match tempra_tir_lower::lower::lower_function(
                    func,
                    &type_ctx,
                    &mut module_builder,
                    None,
                    prefix,
                ) {
                    Ok(tir_func) => {
                        module_builder.add_function(tir_func);
                    }
                    Err(e) => {
                        errors.push(format!("emit-tir unsupported in fn '{}': {}", func.name, e));
                    }
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(DriverError::TirLowerError(errors.join("\\n")));
    }

    let tir_module = module_builder.module;

    if let Err(verifier_errors) = tempra_tir::verifier::verify_module(&tir_module) {
        let msgs: Vec<String> = verifier_errors
            .iter()
            .map(|e| format!("internal TIR verifier error: {:?}", e))
            .collect();
        return Err(DriverError::TirLowerError(msgs.join("\\n")));
    }

    let bytecode_module = tempra_bytecode_lower::lower::lower_module(&tir_module)
        .map_err(|e| DriverError::BytecodeLowerError(format!("{:?}", e)))?;
        
    let optimized_module = tempra_bytecode_lower::peephole::optimize_module(bytecode_module);

    let artifact = crate::cache::CacheArtifact {
        header: crate::cache::CacheHeader {
            magic: crate::cache::CACHE_MAGIC,
            version: crate::cache::CACHE_VERSION,
            compiler_fingerprint: fingerprint.to_string(),
            source_hash: *hash,
        },
        module: optimized_module.clone(),
    };
    let _ = crate::cache::write_cache_artifact(cache_path, &artifact);

    Ok((optimized_module, type_ctx))
}
"""

content = content.replace("pub fn run_source_vm_with_options(", new_funcs + "\npub fn run_source_vm_with_options(")

with open("crates/tempra-driver/src/pipeline.rs", "w") as f:
    f.write(content)
