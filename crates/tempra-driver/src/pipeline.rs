use crate::compilation::{
    analyze_tir_module, compile_tir_module, enforce_analyzed_tir, lower_analyzed_tir, AnalyzedTir,
    VmCompilation,
};
use crate::contracts::{ContractBindings, ContractSource};
use crate::error::DriverError;
use crate::CompilationConfig;
use std::fs;
use std::path::Path;
use tempra_hir::lower_file;
use tempra_hir::HirItem;
use tempra_interpreter::run;
use tempra_interpreter::value::{Value, ValueResult};
use tempra_parser::parse_source;
use tempra_typeck::check_file as typeck_check_file;
use tempra_vm::value::VmValue;

/// Portable process exit codes supported by a `main` returning `Int`.
const PORTABLE_EXIT_CODE_MIN: i64 = 0;
const PORTABLE_EXIT_CODE_MAX: i64 = 255;

fn is_generic_template(func: &tempra_hir::HirFunction) -> bool {
    !func.generic_parameters.is_empty()
        || func
            .parameters
            .iter()
            .any(|p| p.kind == tempra_ast::ParameterKind::Type)
}

fn entry_exit_code(code: i64) -> Result<(), DriverError> {
    if (PORTABLE_EXIT_CODE_MIN..=PORTABLE_EXIT_CODE_MAX).contains(&code) {
        Err(DriverError::Exit(code as i32))
    } else {
        Err(DriverError::RuntimeError(format!(
            "main returned exit code {code}; portable exit codes are {PORTABLE_EXIT_CODE_MIN}..={PORTABLE_EXIT_CODE_MAX}"
        )))
    }
}

fn hir_entry_value_type(value: &Value) -> &'static str {
    match value {
        Value::Unit => "Unit",
        Value::Bool(_) => "Bool",
        Value::Int64(_) => "Int64",
        Value::UInt8(_) => "UInt8",
        Value::Float64(_) => "Float64",
        Value::String(_) => "String",
        Value::Array(_) => "Array",
        Value::Slice(_) => "Slice",
        Value::Tuple(_) => "Tuple",
        Value::Struct(_) => "Struct",
        Value::Command(_) => "Command",
        Value::CommandResult(_) => "CommandResult",
        Value::Result(_) => "Result",
        Value::Option(_) => "Option",
        Value::Error(_) => "Error",
        Value::Regex(_) => "Regex",
        Value::Enum(_) => "Enum",
        Value::Closure(_) => "Closure",
        Value::Cell(_) => "Cell",
        Value::Namespace(_) => "Namespace",
        Value::Type(_) => "Type",
        Value::ArrayIter(_) => "ArrayIter",
        Value::SliceIter(_) => "SliceIter",
    }
}

fn normalize_hir_entry_value(value: Value) -> Result<(), DriverError> {
    match value {
        Value::Unit => Ok(()),
        Value::Int64(code) => entry_exit_code(code),
        Value::Result(result) => match *result {
            ValueResult::Ok(_) => Ok(()),
            ValueResult::Err(error) => {
                let message = match error {
                    Value::String(message) => message,
                    Value::Error(message) => message,
                    other => other.to_string(),
                };
                Err(DriverError::RuntimeError(format!(
                    "Uncaught error: {message}"
                )))
            }
        },
        other => Err(DriverError::RuntimeError(format!(
            "main returned unsupported entry-point value of type {}; expected Unit, Int, or Result",
            hir_entry_value_type(&other)
        ))),
    }
}

fn normalize_vm_entry_value(value: VmValue) -> Result<(), DriverError> {
    match value {
        VmValue::Unit => Ok(()),
        VmValue::Int64(code) => entry_exit_code(code),
        VmValue::Enum(enum_name, variant, payloads) if enum_name == "Result" => {
            match variant.as_str() {
                "Ok" => Ok(()),
                "Err" => {
                    let message = match payloads.first() {
                        Some(VmValue::String(message)) => message.clone(),
                        Some(payload) => payload.to_string(),
                        None => "()".to_string(),
                    };
                    Err(DriverError::RuntimeError(format!(
                        "Uncaught error: {message}"
                    )))
                }
                _ => Err(DriverError::RuntimeError(format!(
                    "main returned unsupported Result variant '{variant}'"
                ))),
            }
        }
        other => Err(DriverError::RuntimeError(format!(
            "main returned unsupported entry-point value of type {}; expected Unit, Int, or Result",
            other.type_name()
        ))),
    }
}

pub fn run_source(source: &str) -> Result<(), DriverError> {
    run_source_with_args(source, &[])
}

pub fn run_source_with_experimental_vm(source: &str, args: &[String]) -> Result<(), DriverError> {
    let value = run_source_vm(source, args)?;
    normalize_vm_entry_value(value)
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
    let (value, is_script) = run_source_hir_with_mode(source, args)?;
    if is_script {
        Ok(())
    } else {
        normalize_hir_entry_value(value)
    }
}

pub fn run_source_hir(source: &str, args: &[String]) -> Result<Value, DriverError> {
    run_source_hir_with_mode(source, args).map(|(value, _)| value)
}

fn run_source_hir_with_mode(source: &str, args: &[String]) -> Result<(Value, bool), DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    tempra_interpreter::ensure_capability_contracts_supported(&hir)
        .map_err(|message| DriverError::RuntimeError(message.to_owned()))?;

    typeck_check_file(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let is_script = !hir.top_level_statements.is_empty();
    let value = tempra_interpreter::run_with_args(&hir, args.to_vec()).map_err(|e| match e {
        tempra_interpreter::RuntimeError::Exit(code) => DriverError::Exit(code),
        tempra_interpreter::RuntimeError::Crash(msg) => {
            DriverError::RuntimeError(format!("crash: {}", msg))
        }
        tempra_interpreter::RuntimeError::Message(s) => DriverError::RuntimeError(s),
    })?;

    Ok((value, is_script))
}

pub fn run_file(path: &Path) -> Result<(), DriverError> {
    run_file_with_args(path, &[])
}

pub fn run_file_with_experimental_vm(path: &Path, args: &[String]) -> Result<(), DriverError> {
    run_file_with_experimental_vm_jit_entry(path, args, false)
}

/// Executes a file entry point through the experimental VM/JIT and applies
/// entry-point result handling. The raw VM value remains available through
/// [`run_file_with_experimental_vm_jit`].
pub fn run_file_with_experimental_vm_jit_entry(
    path: &Path,
    args: &[String],
    jit_enabled: bool,
) -> Result<(), DriverError> {
    let value = run_file_with_experimental_vm_jit(path, args, jit_enabled)?;
    normalize_vm_entry_value(value)
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
    compile_file_to_bytecode_with_analysis_with_config(path, &CompilationConfig::default())
}

/// Compiles a file and its module graph fresh using explicit compilation inputs.
///
/// This path retains TIR and capability facts and does not use the bytecode cache.
pub fn compile_file_to_bytecode_with_analysis_with_config(
    path: &Path,
    config: &CompilationConfig,
) -> Result<VmCompilation, DriverError> {
    let analyzed = analyze_file_to_tir(path, config)?;
    enforce_analyzed_tir(&analyzed)?;
    lower_analyzed_tir(analyzed)
}

fn analyze_file_to_tir(
    path: &Path,
    config: &CompilationConfig,
) -> Result<AnalyzedTir, DriverError> {
    use crate::module_graph::{load_module_graph, DiskDocumentLoader};
    use std::collections::HashMap;
    use tempra_typeck::{check_modules_always_with_comptime, ModuleToCheck};

    let mut diagnostics = Vec::new();
    let mut graph = match load_module_graph(
        path,
        &DiskDocumentLoader,
        config.stdlib_root(),
        &mut diagnostics,
    ) {
        Ok(g) => g,
        Err(_) => {
            // Diagnostics are rendered with `Display`: the derived `Debug` form escapes
            // path separators, which makes Windows paths unreadable and un-copyable.
            let err_msgs: Vec<String> = diagnostics.iter().map(|d| d.to_string()).collect();
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
            // This frontend does not lower extend methods. Never discard a
            // source contract together with an unlowered method, even if unused.
            if let tempra_hir::HirItem::Extend(extend) = item {
                if extend
                    .methods
                    .iter()
                    .any(|method| method.deny_contract.is_some() || method.pure_span.is_some())
                {
                    return Err(DriverError::UnsupportedExperimentalVm(
                        "capability-contract enforcement for extend methods is not supported in module/file compilation".into(),
                    ));
                }
            }
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
    inspect_effects_file_with_config(path, &CompilationConfig::default())
}

/// Inspects capability facts using explicit file/module compilation inputs.
pub fn inspect_effects_file_with_config(
    path: &Path,
    config: &CompilationConfig,
) -> Result<crate::EffectInspection, crate::EffectQueryError> {
    let analyzed = analyze_file_to_tir(path, config).map_err(crate::EffectQueryError::Compiler)?;
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
    let options = tempra_vm::profile::VmOptions {
        jit_enabled,
        ..Default::default()
    };
    let value = run_source_vm_with_options(source, args, options)?;
    normalize_vm_entry_value(value)
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
    check_file_with_config(path, &CompilationConfig::default())
}

/// Type-checks a file and its module graph using explicit compilation inputs.
pub fn check_file_with_config(path: &Path, config: &CompilationConfig) -> Result<(), DriverError> {
    use crate::module_graph::{load_module_graph, DiskDocumentLoader};
    use tempra_typeck::{check_modules, ModuleToCheck};

    let mut diagnostics = Vec::new();
    let mut graph = match load_module_graph(
        path,
        &DiskDocumentLoader,
        config.stdlib_root(),
        &mut diagnostics,
    ) {
        Ok(g) => g,
        Err(_) => {
            let err_msgs: Vec<String> = diagnostics.iter().map(|d| d.to_string()).collect();
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
    diagnose_file_with_config(path, &CompilationConfig::default())
}

/// Diagnoses a file and its module graph using explicit compilation inputs.
pub fn diagnose_file_with_config(
    path: &Path,
    config: &CompilationConfig,
) -> Vec<tempra_diagnostics::Diagnostic> {
    use crate::module_graph::{load_module_graph, DiskDocumentLoader};
    use std::collections::HashMap;
    use tempra_typeck::{check_modules_always_with_comptime, ModuleToCheck};

    let mut diagnostics = Vec::new();
    let mut graph = match load_module_graph(
        path,
        &DiskDocumentLoader,
        config.stdlib_root(),
        &mut diagnostics,
    ) {
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
    fn pure_uses_semantics_seven_and_source_identity_without_cache_metadata() {
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
            format!("tempra-{}-semantics-7", env!("CARGO_PKG_VERSION"))
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

    #[test]
    fn enforced_family_target_can_publish_a_current_cache_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let source = "// k4.9d enforced family target\n@deny(fs) fn main() -> Int64 { return 42 }";
        let path = dir.path().join("family.tbc");
        let module = load_or_compile_cached_module(source, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&module, "main", vec![]).unwrap(),
            VmValue::Int64(42)
        );
        assert!(path.exists());
        assert!(crate::cache::read_compatible_module(
            &path,
            &crate::cache::compute_source_hash(source)
        )
        .is_some());
    }

    #[test]
    fn semantics_six_cannot_certify_v02_contracts_or_promote_failed_compilation() {
        let dir = tempfile::tempdir().unwrap();
        let seed = compile_to_bytecode_with_analysis(
            "fn main() -> Int64 { return 9 }",
            &dir.path().join("seed.tbc"),
        )
        .unwrap();
        let accepted = "@deny(fs, net, io.read, io.write, net.send, net.receive) fn main() -> Int64 { return 42 }";
        let rejected = "@deny(fs, panic) fn main() -> Int64 { return 10 / 2 }";
        let unknown = "pub extern fn k49_unknown() -> Int64\n@deny(io.write) fn main() -> Int64 { return k49_unknown() }";
        for (index, source) in [accepted, rejected, unknown].into_iter().enumerate() {
            let path = dir.path().join(format!("stale-{index}.tbc"));
            let hash = crate::cache::compute_source_hash(source);
            let stale = tempra_bytecode::BytecodeCacheArtifact {
                header: tempra_bytecode::BytecodeCacheHeader {
                    compiler_fingerprint: format!(
                        "tempra-{}-semantics-6",
                        env!("CARGO_PKG_VERSION")
                    ),
                    source_hash: hash,
                    ..Default::default()
                },
                module: seed.bytecode().clone(),
            };
            crate::cache::write_cache_artifact(&path, &stale).unwrap();
            let verified = crate::cache::read_cache_artifact(&path).unwrap();
            assert_eq!(verified.header.source_hash, hash);
            assert_eq!(
                tempra_vm::run_function(&verified.module, "main", vec![]).unwrap(),
                VmValue::Int64(9)
            );
            let bytes = fs::read(&path).unwrap();
            assert!(crate::cache::read_compatible_module(&path, &hash).is_none());
            if source == accepted {
                let fresh = load_or_compile_cached_module(source, &path).unwrap();
                assert_eq!(
                    tempra_vm::run_function(&fresh, "main", vec![]).unwrap(),
                    VmValue::Int64(42)
                );
                assert_eq!(
                    crate::cache::read_cache_artifact(&path)
                        .unwrap()
                        .header
                        .compiler_fingerprint,
                    crate::cache::compiler_fingerprint()
                );
                let bytes = fs::read(&path).unwrap();
                let modified = fs::metadata(&path).unwrap().modified().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(50));
                let hit = load_or_compile_cached_module(source, &path).unwrap();
                assert_eq!(
                    tempra_bytecode::serialize::encode_module(&fresh).unwrap(),
                    tempra_bytecode::serialize::encode_module(&hit).unwrap()
                );
                assert_eq!(fs::read(&path).unwrap(), bytes);
                assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
            } else {
                let error = load_or_compile_cached_module(source, &path).unwrap_err();
                if source == rejected {
                    assert!(matches!(
                        error,
                        DriverError::CapabilityContractViolations(_)
                    ));
                } else {
                    assert!(matches!(
                        error,
                        DriverError::CapabilityAnalysisError(
                            tempra_effects::CapabilityAnalysisError::UnclassifiedExtern { .. }
                        )
                    ));
                }
                assert_eq!(fs::read(&path).unwrap(), bytes);
                assert!(crate::cache::read_compatible_module(&path, &hash).is_none());
                assert_eq!(
                    crate::cache::read_cache_artifact(&path).unwrap().header,
                    stale.header
                );
            }
        }
    }

    #[test]
    fn matching_semantics_seven_header_cannot_bypass_bytecode_verification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid-module.tbc");
        let source = "@deny(fs, io.read, io.write()) fn main() -> Int64 { return 42 }";
        let hash = crate::cache::compute_source_hash(source);
        let fresh = compile_to_bytecode_with_analysis(source, &path).unwrap();
        let canonical_bytes = fs::read(&path).unwrap();
        for invalid_call in [true, false] {
            let mut invalid = crate::cache::read_cache_artifact(&path).unwrap();
            if invalid_call {
                invalid.module.functions[0].code = vec![
                    tempra_bytecode::opcode::BytecodeInstr::Call(
                        tempra_bytecode::function::FunctionId(999),
                        0,
                    ),
                    tempra_bytecode::opcode::BytecodeInstr::Return,
                ];
            } else {
                invalid.module.functions[0]
                    .params
                    .push(tempra_bytecode::function::BytecodeType::Int64);
                invalid.module.functions[0].locals.clear();
            }
            crate::cache::write_cache_artifact(&path, &invalid).unwrap();
            let decoded =
                tempra_bytecode::serialize::decode_artifact(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(decoded.header.source_hash, hash);
            assert_eq!(
                decoded.header.compiler_fingerprint,
                crate::cache::compiler_fingerprint()
            );
            assert!(tempra_bytecode::verify::verify_module(&decoded.module).is_err());
            assert!(crate::cache::read_compatible_module(&path, &hash).is_none());
            let rebuilt = load_or_compile_cached_module(source, &path).unwrap();
            assert_eq!(
                tempra_vm::run_function(&rebuilt, "main", vec![]).unwrap(),
                VmValue::Int64(42)
            );
            assert_eq!(
                tempra_bytecode::serialize::encode_module(&rebuilt).unwrap(),
                tempra_bytecode::serialize::encode_module(fresh.bytecode()).unwrap()
            );
            assert_eq!(fs::read(&path).unwrap(), canonical_bytes);
        }
    }

    const VALID: &str = "@deny(io) fn main() -> Int64 { return 10 / 2 }";
    const VIOLATING: &str = "@deny(panic) fn main() -> Int64 { return 10 / 2 }";
    const K2_SPECIALIZATION_SOURCE: &str = r#"
fn choose(type T, first: T, second: T) -> T { return first }
fn choose_second(type T, first: T, second: T) -> T { return second }
fn choose_again(type T, first: T, second: T) -> T { return first }
fn main() -> Int64 {
    let values: Array(Int) = [41, 0]
    let chosen = values.reduce(choose)
    let ignored_second = values.reduce(choose_second)
    let ignored_again = values.reduce(choose_again)
    if chosen is Some(value) { return value }
    return 99
}
"#;
    const K2_INTERFACE_B_FIRST_SOURCE: &str = r#"
interface B {
    type Item
    fn b(self) -> Item
}
interface A {
    type Item
    fn a(self) -> Item
}
struct Both {}
extend Both {
    fn a(self) -> Int { 41 }
    fn b(self) -> String { "forty-one" }
}
fn select(type T: A, value: T) -> T.Item { value.a() }
fn main() -> Int64 { return select[Both](Both {}) }
"#;

    const K1_STALE_SOURCES: [(&str, &str, &str); 4] = [
        (
            "arithmetic",
            "fn main() -> Int64 { return comptime 9223372036854775807 + 1 }",
            "integer overflow",
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

    fn semantics_two_artifact(
        source: &str,
        module: tempra_bytecode::BytecodeModule,
    ) -> tempra_bytecode::BytecodeCacheArtifact {
        tempra_bytecode::BytecodeCacheArtifact {
            header: tempra_bytecode::BytecodeCacheHeader {
                compiler_fingerprint: format!("tempra-{}-semantics-2", env!("CARGO_PKG_VERSION")),
                source_hash: crate::cache::compute_source_hash(source),
                ..Default::default()
            },
            module,
        }
    }

    fn semantics_three_artifact(
        source: &str,
        module: tempra_bytecode::BytecodeModule,
    ) -> tempra_bytecode::BytecodeCacheArtifact {
        tempra_bytecode::BytecodeCacheArtifact {
            header: tempra_bytecode::BytecodeCacheHeader {
                compiler_fingerprint: format!("tempra-{}-semantics-3", env!("CARGO_PKG_VERSION")),
                source_hash: crate::cache::compute_source_hash(source),
                ..Default::default()
            },
            module,
        }
    }

    fn semantics_four_artifact(
        source: &str,
        module: tempra_bytecode::BytecodeModule,
    ) -> tempra_bytecode::BytecodeCacheArtifact {
        tempra_bytecode::BytecodeCacheArtifact {
            header: tempra_bytecode::BytecodeCacheHeader {
                compiler_fingerprint: format!("tempra-{}-semantics-4", env!("CARGO_PKG_VERSION")),
                source_hash: crate::cache::compute_source_hash(source),
                ..Default::default()
            },
            module,
        }
    }

    fn semantics_five_artifact(
        source: &str,
        module: tempra_bytecode::BytecodeModule,
    ) -> tempra_bytecode::BytecodeCacheArtifact {
        tempra_bytecode::BytecodeCacheArtifact {
            header: tempra_bytecode::BytecodeCacheHeader {
                compiler_fingerprint: format!("tempra-{}-semantics-5", env!("CARGO_PKG_VERSION")),
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
    fn semantics_two_cannot_reuse_a_pre_k2_specialization_result() {
        let dir = tempfile::tempdir().unwrap();
        let seed_path = dir.path().join("stale-seed.tbc");
        let stale =
            compile_to_bytecode_with_analysis("fn main() -> Int64 { return 0 }", &seed_path)
                .unwrap()
                .into_bytecode();
        assert_eq!(
            tempra_vm::run_function(&stale, "main", vec![]).unwrap(),
            VmValue::Int64(0)
        );

        let path = dir.path().join("specialization.tbc");
        crate::cache::write_cache_artifact(
            &path,
            &semantics_two_artifact(K2_SPECIALIZATION_SOURCE, stale),
        )
        .unwrap();
        let source_hash = crate::cache::compute_source_hash(K2_SPECIALIZATION_SOURCE);
        let stale_bytes = fs::read(&path).unwrap();
        let stale_artifact = crate::cache::read_cache_artifact(&path).unwrap();
        assert_eq!(stale_artifact.header.source_hash, source_hash);
        assert_eq!(
            stale_artifact.header.compiler_fingerprint,
            format!("tempra-{}-semantics-2", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(
            tempra_vm::run_function(&stale_artifact.module, "main", vec![]).unwrap(),
            VmValue::Int64(0)
        );
        assert!(crate::cache::read_compatible_module(&path, &source_hash).is_none());

        let corrected = load_or_compile_cached_module(K2_SPECIALIZATION_SOURCE, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&corrected, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        let current_artifact = crate::cache::read_cache_artifact(&path).unwrap();
        assert_eq!(
            current_artifact.header.compiler_fingerprint,
            format!("tempra-{}-semantics-7", env!("CARGO_PKG_VERSION"))
        );
        let current_bytes = fs::read(&path).unwrap();
        assert_ne!(current_bytes, stale_bytes);
        let modified = fs::metadata(&path).unwrap().modified().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));
        let hit = load_or_compile_cached_module(K2_SPECIALIZATION_SOURCE, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&hit, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        assert_eq!(fs::read(&path).unwrap(), current_bytes);
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn semantics_three_cannot_bypass_canonical_interface_selection() {
        let dir = tempfile::tempdir().unwrap();
        let seed_path = dir.path().join("interface-seed.tbc");
        let seed =
            compile_to_bytecode_with_analysis("fn main() -> Int64 { return 41 }", &seed_path)
                .unwrap()
                .into_bytecode();
        assert_eq!(
            tempra_vm::run_function(&seed, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );

        let stale_path = dir.path().join("interface-stale.tbc");
        crate::cache::write_cache_artifact(
            &stale_path,
            &semantics_three_artifact(K2_INTERFACE_B_FIRST_SOURCE, seed),
        )
        .unwrap();
        let source_hash = crate::cache::compute_source_hash(K2_INTERFACE_B_FIRST_SOURCE);
        let stale_bytes = fs::read(&stale_path).unwrap();
        let stale = crate::cache::read_cache_artifact(&stale_path).unwrap();
        assert_eq!(stale.header.source_hash, source_hash);
        assert_eq!(
            stale.header.compiler_fingerprint,
            format!("tempra-{}-semantics-3", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(
            tempra_vm::run_function(&stale.module, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        assert!(crate::cache::read_compatible_module(&stale_path, &source_hash).is_none());

        let error = load_or_compile_cached_module(K2_INTERFACE_B_FIRST_SOURCE, &stale_path)
            .expect_err("fresh typechecking must apply canonical interface order");
        assert!(matches!(error, DriverError::TypeError(_)));
        let message = error.to_string();
        assert!(
            message.contains("expected Int64, found String"),
            "unexpected canonical type error: {message}"
        );
        assert_eq!(fs::read(&stale_path).unwrap(), stale_bytes);
        assert!(crate::cache::read_compatible_module(&stale_path, &source_hash).is_none());

        let current_source = "fn main() -> Int64 { return 41 }";
        let current_path = dir.path().join("interface-current.tbc");
        let current = load_or_compile_cached_module(current_source, &current_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&current, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        let current_bytes = fs::read(&current_path).unwrap();
        let modified = fs::metadata(&current_path).unwrap().modified().unwrap();
        let current_artifact = crate::cache::read_cache_artifact(&current_path).unwrap();
        assert_eq!(
            current_artifact.header.compiler_fingerprint,
            format!("tempra-{}-semantics-7", env!("CARGO_PKG_VERSION"))
        );

        std::thread::sleep(std::time::Duration::from_millis(100));
        let hit = load_or_compile_cached_module(current_source, &current_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&hit, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        assert_eq!(fs::read(&current_path).unwrap(), current_bytes);
        assert_eq!(
            fs::metadata(&current_path).unwrap().modified().unwrap(),
            modified
        );
    }

    #[test]
    fn semantics_four_cannot_bypass_checked_int64_panic_enforcement() {
        const DENIED: &str = "@deny(panic) fn main() -> Int64 { return 1 + 2 }";
        const ALLOWED: &str = "fn main() -> Int64 { return 1 + 2 }";

        let dir = tempfile::tempdir().unwrap();
        let seed_path = dir.path().join("arithmetic-seed.tbc");
        let stale_module =
            compile_to_bytecode_with_analysis("fn main() -> Int64 { return 3 }", &seed_path)
                .unwrap()
                .into_bytecode();
        assert_eq!(
            tempra_vm::run_function(&stale_module, "main", vec![]).unwrap(),
            VmValue::Int64(3)
        );

        let stale_path = dir.path().join("arithmetic-stale.tbc");
        crate::cache::write_cache_artifact(
            &stale_path,
            &semantics_four_artifact(DENIED, stale_module),
        )
        .unwrap();
        let source_hash = crate::cache::compute_source_hash(DENIED);
        let stale_bytes = fs::read(&stale_path).unwrap();
        let stale = crate::cache::read_cache_artifact(&stale_path).unwrap();
        assert_eq!(stale.header.source_hash, source_hash);
        assert_eq!(
            stale.header.compiler_fingerprint,
            format!("tempra-{}-semantics-4", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(
            tempra_vm::run_function(&stale.module, "main", vec![]).unwrap(),
            VmValue::Int64(3)
        );
        assert!(crate::cache::read_compatible_module(&stale_path, &source_hash).is_none());

        let DriverError::CapabilityContractViolations(violations) =
            load_or_compile_cached_module(DENIED, &stale_path).unwrap_err()
        else {
            panic!("fresh compilation must enforce the new Int64 panic origin")
        };
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].capability, tempra_effects::Capability::Panic);
        assert_eq!(
            violations[0].witness.origin.source,
            tempra_effects::DirectCapabilitySource::IntegerAddition
        );
        assert_eq!(fs::read(&stale_path).unwrap(), stale_bytes);
        assert!(crate::cache::read_compatible_module(&stale_path, &source_hash).is_none());

        let current_path = dir.path().join("arithmetic-current.tbc");
        let current = load_or_compile_cached_module(ALLOWED, &current_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&current, "main", vec![]).unwrap(),
            VmValue::Int64(3)
        );
        let current_bytes = fs::read(&current_path).unwrap();
        let modified = fs::metadata(&current_path).unwrap().modified().unwrap();
        assert_eq!(
            crate::cache::read_cache_artifact(&current_path)
                .unwrap()
                .header
                .compiler_fingerprint,
            format!("tempra-{}-semantics-7", env!("CARGO_PKG_VERSION"))
        );

        std::thread::sleep(std::time::Duration::from_millis(100));
        let hit = load_or_compile_cached_module(ALLOWED, &current_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&hit, "main", vec![]).unwrap(),
            VmValue::Int64(3)
        );
        assert_eq!(fs::read(&current_path).unwrap(), current_bytes);
        assert_eq!(
            fs::metadata(&current_path).unwrap().modified().unwrap(),
            modified
        );
    }

    #[test]
    fn semantics_five_artifacts_are_not_reused_after_specialized_enforcement() {
        // K4.5A stamped artifacts that retained specialized clauses but did not
        // enforce them. They must never be treated as current compiler products.
        const SOURCE: &str =
            "@deny(fs.write(destination: \"blocked.txt\"))\nfn main() -> Int64 { return 0 }";

        let dir = tempfile::tempdir().unwrap();
        let seed_path = dir.path().join("specialized-seed.tbc");
        let stale_module =
            compile_to_bytecode_with_analysis("fn main() -> Int64 { return 7 }", &seed_path)
                .unwrap()
                .into_bytecode();
        assert_eq!(
            tempra_vm::run_function(&stale_module, "main", vec![]).unwrap(),
            VmValue::Int64(7)
        );

        let path = dir.path().join("specialized-stale.tbc");
        crate::cache::write_cache_artifact(&path, &semantics_five_artifact(SOURCE, stale_module))
            .unwrap();
        let source_hash = crate::cache::compute_source_hash(SOURCE);
        let stale = crate::cache::read_cache_artifact(&path).unwrap();
        assert_eq!(stale.header.source_hash, source_hash);
        assert_eq!(
            stale.header.compiler_fingerprint,
            format!("tempra-{}-semantics-5", env!("CARGO_PKG_VERSION"))
        );
        assert!(crate::cache::read_compatible_module(&path, &source_hash).is_none());

        let fresh = load_or_compile_cached_module(SOURCE, &path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&fresh, "main", vec![]).unwrap(),
            VmValue::Int64(0)
        );
        let current = crate::cache::read_cache_artifact(&path).unwrap();
        assert_eq!(
            current.header.compiler_fingerprint,
            format!("tempra-{}-semantics-7", env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(current.header.source_hash, source_hash);
    }

    #[test]
    fn valid_semantics_hit_is_verified_bytecode_only_and_contract_edits_miss() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.tbc");
        let fresh = compile_to_bytecode_with_analysis(VALID, &path).unwrap();
        let artifact = crate::cache::read_cache_artifact(&path).unwrap();
        assert_eq!(
            artifact.header.compiler_fingerprint,
            format!("tempra-{}-semantics-7", env!("CARGO_PKG_VERSION"))
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

    #[test]
    fn cache_state_does_not_change_current_semantics_artifact_bytes() {
        const SOURCE: &str =
            "fn helper() -> Int64 { return 40 } fn main() -> Int64 { return helper() + 1 }";

        let dir = tempfile::tempdir().unwrap();
        let cold_path = dir.path().join("cold.tbc");
        let cold = load_or_compile_cached_module(SOURCE, &cold_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&cold, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        let canonical_bytes = fs::read(&cold_path).unwrap();
        let canonical_artifact = crate::cache::read_cache_artifact(&cold_path).unwrap();
        assert_eq!(
            canonical_artifact.header.compiler_fingerprint,
            format!("tempra-{}-semantics-7", env!("CARGO_PKG_VERSION"))
        );

        let modified = fs::metadata(&cold_path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let warm = load_or_compile_cached_module(SOURCE, &cold_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&warm, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        assert_eq!(fs::read(&cold_path).unwrap(), canonical_bytes);
        assert_eq!(
            fs::metadata(&cold_path).unwrap().modified().unwrap(),
            modified
        );

        let stale_path = dir.path().join("stale.tbc");
        crate::cache::write_cache_artifact(
            &stale_path,
            &semantics_three_artifact(SOURCE, canonical_artifact.module.clone()),
        )
        .unwrap();
        assert!(crate::cache::read_compatible_module(
            &stale_path,
            &crate::cache::compute_source_hash(SOURCE)
        )
        .is_none());
        let stale_rebuilt = load_or_compile_cached_module(SOURCE, &stale_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&stale_rebuilt, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        assert_eq!(fs::read(&stale_path).unwrap(), canonical_bytes);

        let malformed_path = dir.path().join("malformed.tbc");
        fs::write(&malformed_path, b"not a Tempra bytecode artifact").unwrap();
        let malformed_rebuilt = load_or_compile_cached_module(SOURCE, &malformed_path).unwrap();
        assert_eq!(
            tempra_vm::run_function(&malformed_rebuilt, "main", vec![]).unwrap(),
            VmValue::Int64(41)
        );
        assert_eq!(fs::read(&malformed_path).unwrap(), canonical_bytes);
    }
}
