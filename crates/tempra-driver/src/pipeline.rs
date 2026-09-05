use crate::compilation::{
    analyze_tir_module, compile_tir_module, enforce_analyzed_tir, lower_analyzed_tir, AnalyzedTir,
    VmCompilation,
};
use crate::contracts::{ContractBindings, ContractSource};
use crate::error::DriverError;
use std::fs;
use std::path::Path;
use tempra_hir::lower_file;
use tempra_hir::HirItem;
use tempra_interpreter::run;
use tempra_interpreter::value::Value;
use tempra_parser::parse_source;
use tempra_typeck::check_file as typeck_check_file;
use tempra_vm::value::VmValue;

fn is_generic_template(func: &tempra_hir::HirFunction) -> bool {
    !func.generic_parameters.is_empty()
        || func
            .parameters
            .iter()
            .any(|p| p.kind == tempra_ast::ParameterKind::Type)
}

pub fn run_source(source: &str) -> Result<(), DriverError> {
    run_source_with_args(source, &[])
}

pub fn run_source_with_experimental_vm(source: &str, args: &[String]) -> Result<(), DriverError> {
    match run_source_vm(source, args) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn run_source_vm(source: &str, args: &[String]) -> Result<VmValue, DriverError> {
    run_source_vm_with_options(source, args, tempra_vm::profile::VmOptions::default())
}

pub fn run_source_vm_with_options(
    source: &str,
    args: &[String],
    options: tempra_vm::profile::VmOptions,
) -> Result<VmValue, DriverError> {
    if !args.is_empty() {
        return Err(DriverError::UnsupportedExperimentalVm(
            "CLI arguments are not yet supported in experimental VM".into(),
        ));
    }

    let hash = crate::cache::compute_source_hash(source);
    let cache_path = crate::cache::get_cache_path(&hash);
    let module = load_or_compile_cached_module(source, &cache_path)?;

    let has_main = module.functions.iter().any(|f| f.name == "main");
    if !has_main {
        return Err(DriverError::UnsupportedExperimentalVm(
            "experimental VM currently requires main()".into(),
        ));
    }

    tempra_vm::run_function_with_resolver(
        &module,
        "main",
        vec![],
        options,
        Some(tempra_native::registry::resolve),
    )
    .map_err(|e| DriverError::VmError(format!("{:?}", e)))
}

pub fn run_source_vm_profiled(
    source: &str,
    options: tempra_vm::VmOptions,
) -> Result<(tempra_vm::VmValue, tempra_vm::VmProfile), DriverError> {
    let hash = crate::cache::compute_source_hash(source);
    let cache_path = crate::cache::get_cache_path(&hash);
    let module = load_or_compile_cached_module(source, &cache_path)?;

    let has_main = module.functions.iter().any(|f| f.name == "main");
    if !has_main {
        return Err(DriverError::UnsupportedExperimentalVm(
            "experimental VM currently requires main()".into(),
        ));
    }

    tempra_vm::Interpreter::run_function_profiled_with_resolver(
        &module,
        "main",
        vec![],
        options,
        Some(tempra_native::registry::resolve),
    )
    .map_err(|e| DriverError::VmError(format!("{:?}", e)))
}

fn load_or_compile_cached_module(
    source: &str,
    cache_path: &Path,
) -> Result<tempra_bytecode::BytecodeModule, DriverError> {
    let hash = crate::cache::compute_source_hash(source);
    if let Some(module) = crate::cache::read_compatible_module(cache_path, &hash) {
        return Ok(module);
    }
    compile_and_cache_module(source, &hash, cache_path).map(VmCompilation::into_bytecode)
}

fn compile_and_cache_module(
    source: &str,
    hash: &[u8; 32],
    cache_path: &std::path::Path,
) -> Result<VmCompilation, DriverError> {
    let ast = tempra_parser::parse_source(source)
        .map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;
    let mut hir = tempra_hir::lower_file(source, &ast);
    let type_ctx = tempra_typeck::check_file_with_context(&mut hir)
        .map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();
    let mut bindings = ContractBindings::default();
    let source_identity = ContractSource::InMemory;
    let mut errors = Vec::new();
    for item in &hir.items {
        match item {
            tempra_hir::HirItem::Function(func) => {
                if is_generic_template(func) {
                    continue;
                }
                match tempra_tir_lower::lower::lower_function(
                    func,
                    &type_ctx,
                    &mut module_builder,
                    None,
                    None,
                ) {
                    Ok(tir_func) => {
                        bindings.register(tir_func.id, func, &source_identity)?;
                        module_builder.add_function(tir_func);
                    }
                    Err(e) => {
                        errors.push(format!("unsupported in fn '{}': {}", func.name, e));
                    }
                }
            }
            tempra_hir::HirItem::Extend(extend) => {
                let (target_name, target_ty) = match &extend.target {
                    tempra_hir::HirType::Named(_, name) => {
                        let ty = tempra_typeck::types::resolve_named_type(name);
                        if ty == tempra_hir::Type::Unknown {
                            if type_ctx.structs.contains_key(name) {
                                (name.clone(), tempra_hir::Type::Struct(name.clone(), vec![]))
                            } else if type_ctx.enums.contains_key(name) {
                                (name.clone(), tempra_hir::Type::Enum(name.clone(), vec![]))
                            } else {
                                errors.push(format!("unknown extend target: {}", name));
                                continue;
                            }
                        } else {
                            (name.clone(), ty)
                        }
                    }
                    _ => {
                        errors.push(format!("unsupported extend target: {:?}", extend.target));
                        continue;
                    }
                };
                for method in &extend.methods {
                    let mut method_clone = method.clone();
                    method_clone.name = format!("{}.{}", target_name, method.name);
                    match tempra_tir_lower::lower::lower_function(
                        &method_clone,
                        &type_ctx,
                        &mut module_builder,
                        Some(target_ty.clone()),
                        None,
                    ) {
                        Ok(tir_func) => {
                            bindings.register(tir_func.id, method, &source_identity)?;
                            module_builder.add_function(tir_func);
                        }
                        Err(e) => {
                            errors.push(format!(
                                "unsupported in method '{}': {}",
                                method_clone.name, e
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    lower_requested_function_specializations(
        &hir.items,
        &type_ctx,
        &mut module_builder,
        &mut errors,
        None,
        &mut bindings,
        &source_identity,
    )?;
    if !errors.is_empty() {
        return Err(DriverError::UnsupportedExperimentalVm(errors.join("\n")));
    }

    let compilation = compile_tir_module(module_builder.module, bindings)?;
    let header = tempra_bytecode::BytecodeCacheHeader {
        compiler_fingerprint: crate::cache::compiler_fingerprint(),
        source_hash: *hash,
        ..Default::default()
    };

    let artifact = tempra_bytecode::BytecodeCacheArtifact {
        header,
        module: compilation.bytecode().clone(),
    };

    if let Err(e) = crate::cache::write_cache_artifact(cache_path, &artifact) {
        eprintln!("Warning: failed to write cache artifact: {}", e);
    }

    Ok(compilation)
}

fn lower_requested_function_specializations(
    items: &[tempra_hir::HirItem],
    type_ctx: &tempra_typeck::context::TypeContext,
    module_builder: &mut tempra_tir::builder::ModuleBuilder,
    errors: &mut Vec<String>,
    module_prefix: Option<&str>,
    bindings: &mut ContractBindings,
    source_identity: &ContractSource,
) -> Result<(), DriverError> {
    for key in &type_ctx.specialization_log {
        let Some(func) = items.iter().find_map(|item| match item {
            tempra_hir::HirItem::Function(func) => {
                let qualified = match module_prefix.filter(|prefix| !prefix.is_empty()) {
                    Some(prefix) => format!("{prefix}.{}", func.name),
                    None => func.name.clone(),
                };
                (qualified == key.function_name).then_some(func)
            }
            _ => None,
        }) else {
            continue;
        };
        if !is_generic_template(func) {
            continue;
        }

        match tempra_tir_lower::lower::lower_function_specialization(
            func,
            type_ctx,
            module_builder,
            None,
            module_prefix,
            &key.type_args,
        ) {
            Ok(tir_func) => {
                bindings.register(tir_func.id, func, source_identity)?;
                module_builder.add_function(tir_func);
            }
            Err(e) => errors.push(format!(
                "unsupported in specialization '{}': {}",
                key.function_name, e
            )),
        }
    }
    Ok(())
}

/// Internal helper for benchmarking and testing the experimental VM pipeline.
/// This is not a stable API and may change or be removed in future versions.
pub fn compile_to_bytecode(
    source: &str,
    cache_path: &Path,
) -> Result<tempra_bytecode::BytecodeModule, DriverError> {
    compile_to_bytecode_with_analysis(source, cache_path).map(VmCompilation::into_bytecode)
}

/// Compiles source fresh, retaining verified TIR and capability facts.
///
/// Never reads the cache. Writes the same bytecode-only cache artifact as
/// [`compile_to_bytecode`]. Explicit deny contracts are enforced before bytecode.
pub fn compile_to_bytecode_with_analysis(
    source: &str,
    cache_path: &Path,
) -> Result<VmCompilation, DriverError> {
    let hash = crate::cache::compute_source_hash(source);
    compile_and_cache_module(source, &hash, cache_path)
}

pub fn run_source_with_args(source: &str, args: &[String]) -> Result<(), DriverError> {
    match run_source_hir(source, args) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn run_source_hir(source: &str, args: &[String]) -> Result<Value, DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    tempra_interpreter::ensure_capability_contracts_supported(&hir)
        .map_err(|message| DriverError::RuntimeError(message.to_owned()))?;

    typeck_check_file(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let value = tempra_interpreter::run_with_args(&hir, args.to_vec()).map_err(|e| match e {
        tempra_interpreter::RuntimeError::Exit(code) => DriverError::Exit(code),
        tempra_interpreter::RuntimeError::Crash(msg) => {
            DriverError::RuntimeError(format!("crash: {}", msg))
        }
        tempra_interpreter::RuntimeError::Message(s) => DriverError::RuntimeError(s),
    })?;

    Ok(value)
}

pub fn run_file(path: &Path) -> Result<(), DriverError> {
    run_file_with_args(path, &[])
}

pub fn run_file_with_experimental_vm(path: &Path, args: &[String]) -> Result<(), DriverError> {
    run_file_with_experimental_vm_jit(path, args, false).map(|_| ())
}

pub fn run_file_with_experimental_vm_jit(
    path: &Path,
    args: &[String],
    jit_enabled: bool,
) -> Result<tempra_vm::value::VmValue, DriverError> {
    if !args.is_empty() {
        return Err(DriverError::UnsupportedExperimentalVm(
            "CLI arguments are not yet supported in experimental VM".into(),
        ));
    }

    let compilation = compile_file_to_bytecode_with_analysis(path)?;
    let bc_module = compilation.bytecode();
    let main_name = bc_module
        .functions
        .iter()
        .find(|f| f.name == "main" || f.name.ends_with(".main"))
        .map(|f| f.name.as_str())
        .ok_or_else(|| {
            DriverError::UnsupportedExperimentalVm(
                "experimental VM currently requires main()".into(),
            )
        })?;
    let options = tempra_vm::profile::VmOptions {
        jit_enabled,
        ..Default::default()
    };
    tempra_vm::run_function_with_resolver(
        bc_module,
        main_name,
        vec![],
        options,
        Some(tempra_native::registry::resolve),
    )
    .map_err(|e| DriverError::VmError(format!("{:?}", e)))
}

/// Compiles a file and its module graph fresh, retaining TIR and capability facts.
/// This path does not read or write a bytecode cache.
pub fn compile_file_to_bytecode_with_analysis(path: &Path) -> Result<VmCompilation, DriverError> {
    let analyzed = analyze_file_to_tir(path)?;
    enforce_analyzed_tir(&analyzed)?;
    lower_analyzed_tir(analyzed)
}

fn analyze_file_to_tir(path: &Path) -> Result<AnalyzedTir, DriverError> {
    use crate::module_graph::{load_module_graph, DiskDocumentLoader};
    use std::collections::HashMap;
    use tempra_typeck::{check_modules_always_with_comptime, ModuleToCheck};

    let mut diagnostics = Vec::new();
    let mut graph = match load_module_graph(path, &DiskDocumentLoader, &mut diagnostics) {
        Ok(g) => g,
        Err(_) => {
            let err_msgs: Vec<String> = diagnostics.iter().map(|d| format!("{:?}", d)).collect();
            return Err(DriverError::TypeError(err_msgs.join("\n")));
        }
    };

    let mut files = Vec::new();
    for name in &graph.order {
        if let Some(node) = graph.modules.get_mut(name) {
            if let Some(hir) = node.hir.take() {
                files.push((
                    node.name.clone(),
                    node.path.clone(),
                    node.imports.clone(),
                    hir,
                ));
            }
        }
    }

    let mut modules_to_check = Vec::new();
    for (name, _path, imports, hir) in &mut files {
        modules_to_check.push(ModuleToCheck {
            name: name.clone(),
            imports: imports.clone(),
            file: hir,
        });
    }

    let source_texts = HashMap::new();
    let type_ctx = check_modules_always_with_comptime(&mut modules_to_check, &source_texts, false);
    if !type_ctx.errors.is_empty() {
        let err_msgs: Vec<String> = type_ctx.errors.iter().map(|e| format!("{:?}", e)).collect();
        return Err(DriverError::TypeError(err_msgs.join("\n")));
    }

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();
    let mut errors = Vec::new();

    let mut bindings = ContractBindings::default();
    for (name, path, _imports, hir) in &files {
        let source_identity = ContractSource::Module {
            name: name.clone(),
            path: path.clone(),
        };
        for item in &hir.items {
            if let tempra_hir::HirItem::Function(func) = item {
                if is_generic_template(func) {
                    continue;
                }
                let prefix = if name.is_empty() {
                    None
                } else {
                    Some(name.as_str())
                };
                match tempra_tir_lower::lower::lower_function(
                    func,
                    &type_ctx,
                    &mut module_builder,
                    None,
                    prefix,
                ) {
                    Ok(tir_func) => {
                        bindings.register(tir_func.id, func, &source_identity)?;
                        module_builder.add_function(tir_func);
                    }
                    Err(e) => {
                        errors.push(format!("unsupported in fn '{}': {}", func.name, e));
                    }
                }
            }
        }
        let prefix = if name.is_empty() {
            None
        } else {
            Some(name.as_str())
        };
        lower_requested_function_specializations(
            &hir.items,
            &type_ctx,
            &mut module_builder,
            &mut errors,
            prefix,
            &mut bindings,
            &source_identity,
        )?;
    }

    if !errors.is_empty() {
        return Err(DriverError::UnsupportedExperimentalVm(errors.join("\n")));
    }

    analyze_tir_module(module_builder.module, bindings)
}

/// Compiles a file fresh and projects its retained E1–E4 capability facts.
///
/// This operation never reads or writes the bytecode cache. A valid program is
/// fully lowered and bytecode-verified before its inspection result is returned.
pub fn inspect_effects_file(
    path: &Path,
) -> Result<crate::EffectInspection, crate::EffectQueryError> {
    let analyzed = analyze_file_to_tir(path).map_err(crate::EffectQueryError::Compiler)?;
    if let Err(error) = enforce_analyzed_tir(&analyzed) {
        return match error {
            DriverError::CapabilityContractViolations(violations) => {
                let diagnostics = crate::project_effect_diagnostics(&analyzed.module, &violations)
                    .map_err(crate::EffectQueryError::Projection)?;
                Err(crate::EffectQueryError::Diagnostics(diagnostics))
            }
            other => Err(crate::EffectQueryError::Compiler(other)),
        };
    }
    let compilation = lower_analyzed_tir(analyzed).map_err(crate::EffectQueryError::Compiler)?;
    compilation
        .effect_inspection()
        .map_err(crate::EffectQueryError::Projection)
}

pub fn run_source_with_experimental_vm_jit(
    source: &str,
    args: &[String],
    jit_enabled: bool,
) -> Result<(), DriverError> {
    let mut options = tempra_vm::profile::VmOptions::default();
    options.jit_enabled = jit_enabled;
    match run_source_vm_with_options(source, args, options) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn run_file_with_args(path: &Path, args: &[String]) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    run_source_with_args(&source, args)
}

pub fn check_source(source: &str) -> Result<(), DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    typeck_check_file(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    Ok(())
}

pub fn check_file(path: &Path) -> Result<(), DriverError> {
    use crate::module_graph::{load_module_graph, DiskDocumentLoader};
    use tempra_typeck::{check_modules, ModuleToCheck};

    let mut diagnostics = Vec::new();
    let mut graph = match load_module_graph(path, &DiskDocumentLoader, &mut diagnostics) {
        Ok(g) => g,
        Err(_) => {
            let err_msgs: Vec<String> = diagnostics.iter().map(|d| format!("{:?}", d)).collect();
            return Err(DriverError::TypeError(err_msgs.join("\n")));
        }
    };

    let mut files = Vec::new();
    for name in &graph.order {
        if let Some(node) = graph.modules.get_mut(name) {
            if let Some(hir) = node.hir.take() {
                files.push((node.name.clone(), node.imports.clone(), hir));
            }
        }
    }

    let mut modules_to_check = Vec::new();
    for (name, imports, hir) in &mut files {
        modules_to_check.push(ModuleToCheck {
            name: name.clone(),
            imports: imports.clone(),
            file: hir,
        });
    }

    match check_modules(&mut modules_to_check, false) {
        Ok(_) => Ok(()),
        Err(errs) => {
            let err_msgs: Vec<String> = errs.iter().map(|e| format!("{:?}", e)).collect();
            Err(DriverError::TypeError(err_msgs.join("\n")))
        }
    }
}

pub fn emit_tir_source(source: &str) -> Result<String, DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    let type_ctx = tempra_typeck::check_file_with_context(&mut hir)
        .map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();

    let mut errors = Vec::new();

    for item in &hir.items {
        if let HirItem::Function(func) = item {
            match tempra_tir_lower::lower::lower_function(
                func,
                &type_ctx,
                &mut module_builder,
                None,
                None,
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

    if !errors.is_empty() {
        return Err(DriverError::TirLowerError(errors.join("\n")));
    }

    let module = module_builder.module;

    if let Err(verifier_errors) = tempra_tir::verifier::verify_module(&module) {
        let msgs: Vec<String> = verifier_errors
            .iter()
            .map(|e| format!("internal TIR verifier error: {:?}", e))
            .collect();
        return Err(DriverError::TirLowerError(msgs.join("\n")));
    }

    Ok(tempra_tir::pretty::format_tir_module(&module))
}

pub fn emit_tir_file(path: &Path) -> Result<String, DriverError> {
    let source = fs::read_to_string(path)?;
    emit_tir_source(&source)
}

pub fn interpret_tir_source(
    source: &str,
    function_name: &str,
    args: Vec<tempra_tir_interp::TirRuntimeValue>,
) -> Result<tempra_tir_interp::TirRuntimeValue, DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;
    let mut hir = lower_file(source, &ast);
    tempra_interpreter::ensure_capability_contracts_supported(&hir)
        .map_err(|message| DriverError::RuntimeError(message.to_owned()))?;
    let type_ctx = tempra_typeck::check_file_with_context(&mut hir)
        .map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();
    let mut errors = Vec::new();

    for item in &hir.items {
        if let HirItem::Function(func) = item {
            match tempra_tir_lower::lower::lower_function(
                func,
                &type_ctx,
                &mut module_builder,
                None,
                None,
            ) {
                Ok(tir_func) => {
                    module_builder.add_function(tir_func);
                }
                Err(e) => {
                    errors.push(format!("lower error in fn '{}': {}", func.name, e));
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(DriverError::TirLowerError(errors.join("\n")));
    }

    let module = module_builder.module;
    tempra_tir_interp::interpret_function(&module, function_name, args)
        .map_err(DriverError::TirInterpError)
}

pub fn test_source(source: &str) -> Result<(), DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    tempra_interpreter::ensure_capability_contracts_supported(&hir)
        .map_err(|message| DriverError::RuntimeError(message.to_owned()))?;

    typeck_check_file(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let value = run(&hir).map_err(|e| match e {
        tempra_interpreter::RuntimeError::Exit(code) => DriverError::Exit(code),
        tempra_interpreter::RuntimeError::Crash(msg) => {
            DriverError::RuntimeError(format!("test failed: {}", msg))
        }
        tempra_interpreter::RuntimeError::Message(s) => DriverError::RuntimeError(s),
    })?;

    if let tempra_interpreter::value::Value::Result(res) = value {
        if let tempra_interpreter::value::ValueResult::Err(err_val) = *res {
            return Err(DriverError::RuntimeError(format!(
                "Uncaught error: {}",
                err_val
            )));
        }
    }

    Ok(())
}

pub fn test_file(path: &Path) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    test_source(&source)
}

fn format_hir_type(ty: &tempra_hir::HirType) -> String {
    match ty {
        tempra_hir::HirType::Named(_, name) => name.clone(),
        tempra_hir::HirType::Array(_, inner) => format!("Array({})", format_hir_type(inner)),
        tempra_hir::HirType::Tuple(_, elements) => {
            let el_strs: Vec<String> = elements.iter().map(format_hir_type).collect();
            format!("({})", el_strs.join(", "))
        }
        tempra_hir::HirType::Apply(_, name, args) => {
            let arg_strs: Vec<String> = args.iter().map(format_hir_type).collect();
            format!("{}<{}>", format_hir_type(name), arg_strs.join(", "))
        }
        tempra_hir::HirType::Optional(_, inner) => format!("Option({})", format_hir_type(inner)),
        tempra_hir::HirType::Result(_, ok, err) => {
            format!("Result({}, {})", format_hir_type(ok), format_hir_type(err))
        }
        tempra_hir::HirType::Function(_, params, ret) => {
            let param_strs: Vec<String> = params.iter().map(format_hir_type).collect();
            format!("fn({}) -> {}", param_strs.join(", "), format_hir_type(ret))
        }
        tempra_hir::HirType::SelfType(_) => "Self".to_string(),
        tempra_hir::HirType::Projection(_, base, name) => {
            format!("{}.{}", format_hir_type(base), name)
        }
    }
}

fn format_function_signature(func: &tempra_hir::HirFunction) -> String {
    let pub_prefix = if func.visibility == tempra_ast::Visibility::Public {
        "pub "
    } else {
        ""
    };
    let params: Vec<String> = func
        .parameters
        .iter()
        .map(|p| {
            let inout_str = if p.is_inout { "inout " } else { "" };
            format!("{}{}: {}", inout_str, p.name, format_hir_type(&p.ty))
        })
        .collect();
    let ret_str = if let Some(ret) = &func.return_type {
        format!(" -> {}", format_hir_type(ret))
    } else {
        "".to_string()
    };
    let fallible_str = if func.is_fallible { "!" } else { "" };
    format!(
        "{}fn {}{}({}){}",
        pub_prefix,
        func.name,
        fallible_str,
        params.join(", "),
        ret_str
    )
}

pub fn diagnose_source(source: &str) -> Vec<tempra_diagnostics::Diagnostic> {
    let mut diagnostics = Vec::new();
    let ast = match tempra_parser::parse_source(source) {
        Ok(ast) => ast,
        Err(e) => {
            diagnostics.push(tempra_diagnostics::Diagnostic::error(e.span, e.message));
            return diagnostics;
        }
    };

    let mut hir = tempra_hir::lower_file(source, &ast);

    if let Err(type_errors) = tempra_typeck::check_file_with_context(&mut hir) {
        for err in type_errors {
            diagnostics.push(tempra_diagnostics::Diagnostic::error(err.span, err.message));
        }
    }

    diagnostics
}

#[derive(Debug, Clone)]
pub struct AnalysisFactsResult {
    pub diagnostics: Vec<tempra_diagnostics::Diagnostic>,
    pub facts: Option<tempra_typeck::TypecheckFacts>,
}

pub fn diagnose_source_with_facts(source: &str) -> AnalysisFactsResult {
    let mut diagnostics = Vec::new();
    let ast = match tempra_parser::parse_source(source) {
        Ok(ast) => ast,
        Err(e) => {
            diagnostics.push(tempra_diagnostics::Diagnostic::error(e.span, e.message));
            return AnalysisFactsResult {
                diagnostics,
                facts: None,
            };
        }
    };

    let mut hir = tempra_hir::lower_file(source, &ast);

    let ctx = tempra_typeck::check_file_with_context_always(&mut hir, Some(source.to_string()));

    for err in ctx.errors {
        diagnostics.push(tempra_diagnostics::Diagnostic::error(err.span, err.message));
    }

    AnalysisFactsResult {
        diagnostics,
        facts: Some(ctx.facts),
    }
}

pub fn diagnose_file(path: &Path) -> Vec<tempra_diagnostics::Diagnostic> {
    use crate::module_graph::{load_module_graph, DiskDocumentLoader};
    use std::collections::HashMap;
    use tempra_typeck::{check_modules_always_with_comptime, ModuleToCheck};

    let mut diagnostics = Vec::new();
    let mut graph = match load_module_graph(path, &DiskDocumentLoader, &mut diagnostics) {
        Ok(g) => g,
        Err(_) => return diagnostics, // load_module_graph already populated diagnostics
    };

    let mut files = Vec::new();
    let mut source_texts = HashMap::new();
    for name in &graph.order {
        if let Some(node) = graph.modules.get_mut(name) {
            source_texts.insert(node.name.clone(), node.source.clone());
            if let Some(hir) = node.hir.take() {
                files.push((node.name.clone(), node.imports.clone(), hir));
            }
        }
    }

    let mut modules_to_check = Vec::new();
    for (name, imports, hir) in &mut files {
        modules_to_check.push(ModuleToCheck {
            name: name.clone(),
            imports: imports.clone(),
            file: hir,
        });
    }

    let ctx = check_modules_always_with_comptime(&mut modules_to_check, &source_texts, false);
    for err in ctx.errors {
        let mut d = tempra_diagnostics::Diagnostic::error(err.span, err.message);
        if let Some(ref m_name) = err.module_name {
            if let Some(node) = graph.modules.get(m_name) {
                d = d.with_file_path(node.path.to_string_lossy().to_string());
            }
        }
        diagnostics.push(d);
    }

    diagnostics
}

pub fn extract_symbols_from_source(
    source: &str,
) -> Result<Vec<tempra_diagnostics::Symbol>, DriverError> {
    let ast =
        tempra_parser::parse_source(source).map_err(|e| DriverError::ParseError(e.message))?;
    let hir = tempra_hir::lower_file(source, &ast);

    let mut symbols = Vec::new();
    for item in &hir.items {
        match item {
            tempra_hir::HirItem::Function(func) => {
                let signature = format_function_signature(func);
                symbols.push(tempra_diagnostics::Symbol {
                    name: func.name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Function,
                    span: func.span,
                    signature: Some(signature),
                });
            }
            tempra_hir::HirItem::Struct(s) => {
                let pub_prefix = if s.visibility == tempra_ast::Visibility::Public {
                    "pub "
                } else {
                    ""
                };
                symbols.push(tempra_diagnostics::Symbol {
                    name: s.name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Struct,
                    span: s.span,
                    signature: Some(format!("{}struct {}", pub_prefix, s.name)),
                });
                for field in &s.fields {
                    let ty_str = format_hir_type(&field.ty);
                    symbols.push(tempra_diagnostics::Symbol {
                        name: format!("{}.{}", s.name, field.name),
                        kind: tempra_diagnostics::SymbolKind::Field,
                        span: field.span,
                        signature: Some(format!("{}: {}", field.name, ty_str)),
                    });
                }
            }
            tempra_hir::HirItem::Enum(e) => {
                let pub_prefix = if e.visibility == tempra_ast::Visibility::Public {
                    "pub "
                } else {
                    ""
                };
                symbols.push(tempra_diagnostics::Symbol {
                    name: e.name.clone(),
                    kind: tempra_diagnostics::SymbolKind::Enum,
                    span: e.span,
                    signature: Some(format!("{}enum {}", pub_prefix, e.name)),
                });
            }
            tempra_hir::HirItem::Extend(ext) => {
                let target_name = format_hir_type(&ext.target);
                symbols.push(tempra_diagnostics::Symbol {
                    name: format!("extend {}", target_name),
                    kind: tempra_diagnostics::SymbolKind::Extension,
                    span: ext.span,
                    signature: Some(format!("extend {}", target_name)),
                });

                for method in &ext.methods {
                    let signature = format_function_signature(method);
                    symbols.push(tempra_diagnostics::Symbol {
                        name: format!("{}.{}", target_name, method.name),
                        kind: tempra_diagnostics::SymbolKind::Method,
                        span: method.span,
                        signature: Some(signature),
                    });
                }
            }
            _ => {}
        }
    }
    Ok(symbols)
}

pub fn extract_symbols(path: &Path) -> Result<Vec<tempra_diagnostics::Symbol>, DriverError> {
    let source = fs::read_to_string(path)?;
    extract_symbols_from_source(&source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_source_basic() {
        let source = "
        fn main() {
            let x = 1;
            print(x)
        }
        ";
        let result = run_source(source);
        assert!(
            result.is_ok(),
            "Expected success, got {:?}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_check_source_success() {
        let source = "
        fn main() {
            let x = 1;
            print(x)
        }
        ";
        let result = check_source(source);
        assert!(
            result.is_ok(),
            "Expected success, got {:?}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_check_source_type_error() {
        let source = "
        fn main() {
            let x = fs.read(123);
        }
        ";
        let result = check_source(source);
        assert!(matches!(result, Err(DriverError::TypeError(_))));
    }

    #[test]
    fn test_check_source_parse_error() {
        let source = "
        fn main( {
            let x = 1;
        }
        ";
        let result = check_source(source);
        assert!(matches!(result, Err(DriverError::ParseError(_))));
    }

    #[test]
    fn test_test_source_passing() {
        let source = "
        fn main() {
            expect(2 + 2 == 4)
            expect(fmt(\"a\", 1) == \"a1\")
        }
        ";
        let result = test_source(source);
        assert!(
            result.is_ok(),
            "Expected test to pass, got {:?}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_test_source_failing() {
        let source = "
        fn main() {
            expect(false, \"boom\")
        }
        ";
        let result = test_source(source);
        assert!(matches!(result, Err(DriverError::RuntimeError(msg)) if msg.contains("boom")));
    }

    #[test]
    fn test_emit_tir_simple_add() {
        let source = "
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        ";
        let result = emit_tir_source(source);
        assert!(
            result.is_ok(),
            "Expected TIR output, got {:?}",
            result.err().unwrap()
        );
        let tir = result.unwrap();
        assert!(tir.contains("fn add"));
        assert!(tir.contains("add %"));
        assert!(tir.contains("ret %"));
    }

    #[test]
    fn test_emit_tir_unsupported_for_error() {
        let source = "
        fn main() {
            for i in \"string_not_supported\" {
                print(i)
            }
        }
        ";
        let result = emit_tir_source(source);
        assert!(matches!(result, Err(DriverError::TirLowerError(_))));
    }

    #[test]
    fn test_tir_interp_source_const_return() {
        let source = "fn main() -> Int64 { return 42 }";
        let result = interpret_tir_source(source, "main", vec![]);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            tempra_tir_interp::TirRuntimeValue::Int64(42)
        );
    }

    #[test]
    fn test_tir_interp_source_add_function() {
        let source = "
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn main() -> Int64 {
            return add(10, 32)
        }
        ";
        let result = interpret_tir_source(source, "main", vec![]);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            tempra_tir_interp::TirRuntimeValue::Int64(42)
        );
    }

    #[test]
    fn test_tir_interp_source_param_identity() {
        let source = "fn id(x: Int64) -> Int64 { return x }";
        let result = interpret_tir_source(
            source,
            "id",
            vec![tempra_tir_interp::TirRuntimeValue::Int64(99)],
        );
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            tempra_tir_interp::TirRuntimeValue::Int64(99)
        );
    }

    #[test]
    fn test_tir_interp_source_if_return() {
        let source = "
        fn test(cond: Bool) -> Int64 {
            if cond {
                return 1
            } else {
                return 0
            }
        }
        ";
        let res_t = interpret_tir_source(
            source,
            "test",
            vec![tempra_tir_interp::TirRuntimeValue::Bool(true)],
        );
        assert_eq!(res_t.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(1));

        let res_f = interpret_tir_source(
            source,
            "test",
            vec![tempra_tir_interp::TirRuntimeValue::Bool(false)],
        );
        assert_eq!(res_f.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(0));
    }

    #[test]
    fn test_tir_interp_source_var_assignment() {
        let source = "
        fn test() -> Int64 {
            var x = 10
            x = 20
            return x
        }
        ";
        let result = interpret_tir_source(source, "test", vec![]);
        assert_eq!(
            result.unwrap(),
            tempra_tir_interp::TirRuntimeValue::Int64(20)
        );
    }

    #[test]
    fn test_tir_interp_source_call_simple() {
        let source = "
        fn double(x: Int64) -> Int64 { return x * 2 }
        fn test() -> Int64 { return double(21) }
        ";
        let result = interpret_tir_source(source, "test", vec![]);
        assert_eq!(
            result.unwrap(),
            tempra_tir_interp::TirRuntimeValue::Int64(42)
        );
    }

    #[test]
    fn test_tir_interp_source_unsupported_for_error() {
        let source = "
        fn main() {
            for i in 123 {}
        }
        ";
        let result = interpret_tir_source(source, "main", vec![]);
        assert!(matches!(result, Err(DriverError::TypeError(_))));
    }

    #[test]
    fn test_tir_interp_unknown_function_error() {
        let source = "fn main() {}";
        let result = interpret_tir_source(source, "ghost", vec![]);
        assert!(matches!(
            result,
            Err(DriverError::TirInterpError(
                tempra_tir_interp::TirInterpError::UnknownFunction(_)
            ))
        ));
    }
}

#[cfg(test)]
mod enforcement_cache_tests {
    use super::*;

    #[test]
    fn pure_uses_semantics_two_and_source_identity_without_cache_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pure.tbc");
        let pure = "@pure fn main() -> Int64 { return 10 / 2 }";
        let plain = pure.replace("@pure", "");
        // Adding pure must miss an existing same-path unannotated artifact.
        compile_to_bytecode_with_analysis(&plain, &path).unwrap();
        let pure_hash = crate::cache::compute_source_hash(pure);
        assert!(crate::cache::read_compatible_module(&path, &pure_hash).is_none());
        let fresh = load_or_compile_cached_module(pure, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&fresh, "main", vec![]).unwrap(),
            VmValue::Int64(5)
        );
        let header = crate::cache::read_cache_artifact(&path).unwrap().header;
        assert_eq!(
            header.compiler_fingerprint,
            format!("tempra-{}-semantics-2", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(header.source_hash, pure_hash);
        let bytes = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        let hit = load_or_compile_cached_module(pure, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&hit, "main", vec![]).unwrap(),
            VmValue::Int64(5)
        );
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert_eq!(fs::read(&path).unwrap(), bytes);

        let violating = pure.replace("@pure", "@pure @deny(panic)");
        // Source hashing distinguishes removal, equivalent explicit spelling,
        // and composition introducing an observed denied requirement.
        for changed in [&plain, &pure.replace("@pure", "@deny(io)"), &violating] {
            let hash = crate::cache::compute_source_hash(changed);
            assert_ne!(hash, pure_hash);
            assert!(crate::cache::read_compatible_module(&path, &hash).is_none());
        }
        let DriverError::CapabilityContractViolations(errors) =
            load_or_compile_cached_module(&violating, &path).unwrap_err()
        else {
            panic!()
        };
        assert!(errors[0].contract.pure_span.is_some());
        assert_eq!(errors[0].capability, tempra_effects::Capability::Panic);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        let absent = dir.path().join("violating.tbc");
        assert!(matches!(
            compile_to_bytecode_with_analysis(&violating, &absent),
            Err(DriverError::CapabilityContractViolations(_))
        ));
        assert!(!absent.exists());

        load_or_compile_cached_module(&plain, &path).unwrap();
        assert_eq!(
            crate::cache::read_cache_artifact(&path)
                .unwrap()
                .header
                .source_hash,
            crate::cache::compute_source_hash(&plain)
        );
        // Facts-producing API still compiles fresh and retains original pure intent.
        let compilation = compile_to_bytecode_with_analysis(pure, &path).unwrap();
        assert!(compilation
            .contracts()
            .next()
            .unwrap()
            .1
            .pure_span
            .is_some());
    }

    const VALID: &str = "@deny(io) fn main() -> Int64 { return 10 / 2 }";
    const VIOLATING: &str = "@deny(panic) fn main() -> Int64 { return 10 / 2 }";

    const K1_STALE_SOURCES: [(&str, &str, &str); 4] = [
        (
            "arithmetic",
            "fn main() -> Int64 { return comptime 9223372036854775807 + 1 }",
            "integer addition overflow",
        ),
        (
            "bodyless extern",
            "pub extern fn missing() -> Unit\nfn main() { comptime missing() }",
            "function 'missing' has no executable HIR body",
        ),
        (
            "forbidden effect",
            "fn hidden() { dbg(\"hidden\") }\nfn main() { comptime hidden() }",
            "operation 'dbg' is not allowed during comptime evaluation",
        ),
        (
            "termination",
            "fn recurse() -> Int64 { return recurse() }\nfn main() -> Int64 { return comptime recurse() }",
            "comptime call depth exceeded",
        ),
    ];

    fn semantics_one_artifact(
        source: &str,
        module: tempra_bytecode::BytecodeModule,
    ) -> tempra_bytecode::BytecodeCacheArtifact {
        tempra_bytecode::BytecodeCacheArtifact {
            header: tempra_bytecode::BytecodeCacheHeader {
                compiler_fingerprint: format!("tempra-{}-semantics-1", env!("CARGO_PKG_VERSION")),
                source_hash: crate::cache::compute_source_hash(source),
                ..Default::default()
            },
            module,
        }
    }

    fn legacy_artifact(source: &str) -> tempra_bytecode::BytecodeCacheArtifact {
        // Reproduce the real pre-E3.2 compiler: preserve @deny in HIR, but lower
        // without enforcement. Never manufacture an enforcement-aware stamp.
        let ast = parse_source(source).unwrap();
        let mut hir = lower_file(source, &ast);
        let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
        let mut builder = tempra_tir::builder::ModuleBuilder::new();
        for item in &hir.items {
            if let HirItem::Function(function) = item {
                let lowered = tempra_tir_lower::lower::lower_function(
                    function,
                    &context,
                    &mut builder,
                    None,
                    None,
                )
                .unwrap();
                builder.add_function(lowered);
            }
        }
        tempra_tir::verifier::verify_module(&builder.module).unwrap();
        let module = tempra_bytecode_lower::lower_tir_module(&builder.module).unwrap();
        tempra_bytecode::verify::verify_module(&module).unwrap();
        tempra_bytecode::BytecodeCacheArtifact {
            header: tempra_bytecode::BytecodeCacheHeader {
                compiler_fingerprint: format!("tempra-{}", env!("CARGO_PKG_VERSION")),
                source_hash: crate::cache::compute_source_hash(source),
                ..Default::default()
            },
            module,
        }
    }

    #[test]
    fn old_semantics_cannot_bypass_enforcement_or_be_promoted_after_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.tbc");
        crate::cache::write_cache_artifact(&path, &legacy_artifact(VIOLATING)).unwrap();
        let before = fs::read(&path).unwrap();
        assert!(crate::cache::read_compatible_module(
            &path,
            &crate::cache::compute_source_hash(VIOLATING)
        )
        .is_none());
        assert!(matches!(
            load_or_compile_cached_module(VIOLATING, &path),
            Err(DriverError::CapabilityContractViolations(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        // The obsolete artifact remains an ignored miss, not a newly trusted entry.
        assert!(crate::cache::read_compatible_module(
            &path,
            &crate::cache::compute_source_hash(VIOLATING)
        )
        .is_none());
        let fresh_path = dir.path().join("must-not-exist.tbc");
        assert!(matches!(
            compile_to_bytecode_with_analysis(VIOLATING, &fresh_path),
            Err(DriverError::CapabilityContractViolations(_))
        ));
        assert!(!fresh_path.exists());
        assert!(matches!(
            compile_to_bytecode(VIOLATING, &fresh_path),
            Err(DriverError::CapabilityContractViolations(_))
        ));
        assert!(!fresh_path.exists());
    }

    #[test]
    fn semantics_one_artifacts_cannot_bypass_k1_comptime_failures() {
        let dir = tempfile::tempdir().unwrap();
        let seed_path = dir.path().join("seed.tbc");
        let seed = compile_to_bytecode_with_analysis("fn main() -> Int64 { return 7 }", &seed_path)
            .unwrap();
        let stale_module = seed.bytecode().clone();
        assert_eq!(
            tempra_vm::run_function(&stale_module, "main", vec![]).unwrap(),
            VmValue::Int64(7)
        );

        for (index, (family, source, expected_error)) in K1_STALE_SOURCES.iter().enumerate() {
            let path = dir.path().join(format!("stale-{index}.tbc"));
            let artifact = semantics_one_artifact(source, stale_module.clone());
            crate::cache::write_cache_artifact(&path, &artifact).unwrap();
            let before = fs::read(&path).unwrap();
            let source_hash = crate::cache::compute_source_hash(source);
            let decoded = crate::cache::read_cache_artifact(&path)
                .expect("stale artifact must be structurally valid and verified");

            assert_eq!(decoded.header.source_hash, source_hash);
            assert_eq!(
                decoded.header.compiler_fingerprint,
                format!("tempra-{}-semantics-1", env!("CARGO_PKG_VERSION"))
            );
            assert_eq!(
                tempra_vm::run_function(&decoded.module, "main", vec![]).unwrap(),
                VmValue::Int64(7),
                "stale {family} bytecode must otherwise be executable"
            );

            assert!(
                crate::cache::read_compatible_module(&path, &source_hash).is_none(),
                "semantics-1 {family} artifact must be incompatible"
            );

            let error = load_or_compile_cached_module(source, &path)
                .expect_err("fresh K1 compilation must reject stale source semantics");
            let message = error.to_string();
            assert!(
                matches!(error, DriverError::TypeError(_)),
                "unexpected {family} error: {message}"
            );
            assert!(
                message.contains(expected_error),
                "fresh {family} compilation produced the wrong error: {message}"
            );
            assert_eq!(
                fs::read(&path).unwrap(),
                before,
                "failed {family} compilation must not replace the stale artifact"
            );
            let unchanged = crate::cache::read_cache_artifact(&path).unwrap();
            assert_eq!(unchanged.header.source_hash, source_hash);
            assert_eq!(
                unchanged.header.compiler_fingerprint,
                format!("tempra-{}-semantics-1", env!("CARGO_PKG_VERSION")),
                "failed {family} compilation must not promote the stale artifact"
            );
            assert!(
                crate::cache::read_compatible_module(&path, &source_hash).is_none(),
                "stale {family} artifact must remain an incompatible miss"
            );
        }
    }

    #[test]
    fn valid_semantics_hit_is_verified_bytecode_only_and_contract_edits_miss() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.tbc");
        let fresh = compile_to_bytecode_with_analysis(VALID, &path).unwrap();
        let artifact = crate::cache::read_cache_artifact(&path).unwrap();
        assert_eq!(
            artifact.header.compiler_fingerprint,
            format!("tempra-{}-semantics-2", env!("CARGO_PKG_VERSION"))
        );
        let before = fs::read(&path).unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        // The same production hit predicate returns BytecodeModule, not TIR/facts.
        let cached =
            crate::cache::read_compatible_module(&path, &crate::cache::compute_source_hash(VALID))
                .unwrap();
        assert_eq!(
            tempra_bytecode::serialize::encode_module(&cached).unwrap(),
            tempra_bytecode::serialize::encode_module(fresh.bytecode()).unwrap()
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
        let hit = load_or_compile_cached_module(VALID, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&hit, "main", vec![]).unwrap(),
            VmValue::Int64(5)
        );
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert_eq!(fs::read(&path).unwrap(), before);
        // Even at the same path, changing only the contract rejects the old identity.
        let changed_hash = crate::cache::compute_source_hash(VIOLATING);
        assert_ne!(
            crate::cache::get_cache_path(&changed_hash),
            crate::cache::get_cache_path(&crate::cache::compute_source_hash(VALID))
        );
        assert!(crate::cache::read_compatible_module(&path, &changed_hash).is_none());
        assert!(matches!(
            load_or_compile_cached_module(VIOLATING, &path),
            Err(DriverError::CapabilityContractViolations(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        // Source-body changes also miss and successfully replace compatible data.
        let changed = VALID.replace("10 / 2", "12 / 2");
        let changed_module = load_or_compile_cached_module(&changed, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&changed_module, "main", vec![]).unwrap(),
            VmValue::Int64(6)
        );
    }
}
