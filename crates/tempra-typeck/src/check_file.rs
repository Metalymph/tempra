//! Entry point for type-checking a Tempra HIR file.

use crate::check_item::{check_item, register_items};
use crate::context::TypeContext;
use crate::error::TypeError;
use std::collections::HashMap;
use tempra_hir::HirFile;
use tempra_interpreter::interpreter::{Interpreter, InterpreterModule};

pub struct ModuleToCheck<'a> {
    pub name: String,
    pub imports: HashMap<String, String>,
    pub file: &'a mut HirFile,
}

fn install_module_interpreter(ctx: &mut TypeContext, modules: &mut [ModuleToCheck<'_>]) {
    ctx.interpreter = Some(Interpreter::new_for_modules(modules.iter_mut().map(
        |module| InterpreterModule {
            name: &module.name,
            imports: &module.imports,
            file: &*module.file,
        },
    )));
}

fn select_module(ctx: &mut TypeContext, module: &ModuleToCheck<'_>) {
    ctx.current_module = module.name.clone();
    ctx.imports = module.imports.clone();
    let selected = ctx
        .interpreter
        .as_mut()
        .is_some_and(|interpreter| interpreter.select_module(&module.name));
    assert!(selected, "comptime interpreter is missing module identity");
}

pub fn check_modules(
    modules: &mut [ModuleToCheck],
    is_single_file_fallback: bool,
) -> Result<TypeContext, Vec<TypeError>> {
    let mut ctx = TypeContext::new();
    ctx.is_single_file_fallback = is_single_file_fallback;
    ctx.modules = modules.iter().map(|m| m.name.clone()).collect();
    install_module_interpreter(&mut ctx, modules);

    // Pass 1: register all items in all modules
    for m in modules.iter_mut() {
        select_module(&mut ctx, m);
        register_items(&mut ctx, &m.file.items);
    }

    // Pass 2: check item bodies in all modules
    for m in modules.iter_mut() {
        select_module(&mut ctx, m);
        for item in &mut m.file.items {
            check_item(&mut ctx, item);
        }
    }

    // Pass 3: check top-level statements in all modules
    for m in modules.iter_mut() {
        select_module(&mut ctx, m);
        if !m.file.top_level_statements.is_empty() {
            ctx.current_is_fallible = true;
            ctx.current_return_type = None;
            ctx.scopes.push_scope();
            crate::check_stmt::check_statement_list(
                &mut ctx,
                &mut m.file.top_level_statements,
                false,
            );
            ctx.scopes.pop_scope();
            ctx.current_is_fallible = false;
        }
    }

    if ctx.errors.is_empty() {
        Ok(ctx)
    } else {
        Err(ctx.errors)
    }
}

pub fn check_modules_always(
    modules: &mut [ModuleToCheck],
    source_texts: &HashMap<String, String>,
    is_single_file_fallback: bool,
) -> TypeContext {
    check_modules_always_impl(modules, source_texts, is_single_file_fallback, false)
}

/// Type-checks modules while retaining errors and enabling canonical comptime evaluation.
///
/// This is the fact-producing counterpart used by ordinary file/compiler paths.
/// Incremental analysis and REPL consumers intentionally continue to use their
/// existing authority boundaries.
pub fn check_modules_always_with_comptime(
    modules: &mut [ModuleToCheck],
    source_texts: &HashMap<String, String>,
    is_single_file_fallback: bool,
) -> TypeContext {
    check_modules_always_impl(modules, source_texts, is_single_file_fallback, true)
}

fn check_modules_always_impl(
    modules: &mut [ModuleToCheck],
    source_texts: &HashMap<String, String>,
    is_single_file_fallback: bool,
    comptime_enabled: bool,
) -> TypeContext {
    let mut ctx = TypeContext::new();
    ctx.is_single_file_fallback = is_single_file_fallback;
    ctx.modules = modules.iter().map(|m| m.name.clone()).collect();
    if comptime_enabled {
        install_module_interpreter(&mut ctx, modules);
    }

    // Pass 1: register all items
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
        if comptime_enabled {
            let selected = ctx
                .interpreter
                .as_mut()
                .is_some_and(|interpreter| interpreter.select_module(&m.name));
            assert!(selected, "comptime interpreter is missing module identity");
        }
        register_items(&mut ctx, &m.file.items);
    }

    // Pass 2: check items
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
        if comptime_enabled {
            let selected = ctx
                .interpreter
                .as_mut()
                .is_some_and(|interpreter| interpreter.select_module(&m.name));
            assert!(selected, "comptime interpreter is missing module identity");
        }
        ctx.facts = crate::facts::TypecheckFacts::default();
        if let Some(source) = source_texts.get(&m.name) {
            ctx.source_text = Some(source.clone());
        } else {
            ctx.source_text = None;
        }
        for item in &mut m.file.items {
            check_item(&mut ctx, item);
        }
        ctx.module_facts.insert(m.name.clone(), ctx.facts.clone());
    }

    // Pass 3: check top-level statements
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
        ctx.facts = ctx.module_facts.remove(&m.name).unwrap_or_default();
        if let Some(source) = source_texts.get(&m.name) {
            ctx.source_text = Some(source.clone());
        } else {
            ctx.source_text = None;
        }
        if !m.file.top_level_statements.is_empty() {
            ctx.current_is_fallible = true;
            ctx.current_return_type = None;
            ctx.scopes.push_scope();
            crate::check_stmt::check_statement_list(
                &mut ctx,
                &mut m.file.top_level_statements,
                false,
            );
            ctx.scopes.pop_scope();
            ctx.current_is_fallible = false;
        }
        ctx.module_facts.insert(m.name.clone(), ctx.facts.clone());
    }

    ctx
}

/// Shared structural typechecking pipeline for a single HIR file.
///
/// All public single-file entry points use this implementation to ensure
/// identical semantic checks and diagnostic ordering.
fn check_file_internal(file: &mut HirFile, source: Option<String>) -> TypeContext {
    let mut ctx = TypeContext::new();

    ctx.is_single_file_fallback = true;
    ctx.source_text = source;
    ctx.interpreter = Some(tempra_interpreter::interpreter::Interpreter::new(file));

    // Pass 1: register top-level declarations.
    register_items(&mut ctx, &file.items);

    // Pass 2: check item bodies.
    for item in &mut file.items {
        check_item(&mut ctx, item);
    }

    // Pass 3: check script statements.
    if !file.top_level_statements.is_empty() {
        ctx.current_is_fallible = true;
        ctx.current_return_type = None;

        ctx.scopes.push_scope();

        crate::check_stmt::check_statement_list(&mut ctx, &mut file.top_level_statements, false);

        ctx.scopes.pop_scope();
        ctx.current_is_fallible = false;
    }

    // R2.2d: Result provenance analysis after successful typechecking.
    if ctx.errors.is_empty() {
        let diagnostics = crate::result_obligations::analyze_linear_result_obligations(
            file,
            &ctx.facts,
            &ctx.function_spans,
            &ctx.current_module,
        );
        ctx.errors.extend(diagnostics);
    }

    ctx
}

/// Type-check a complete HIR file.
///
/// Returns Ok(()) if the file is well-typed.
pub fn check_file(file: &mut HirFile) -> Result<(), Vec<TypeError>> {
    let ctx = check_file_internal(file, None);

    if ctx.errors.is_empty() {
        Ok(())
    } else {
        Err(ctx.errors)
    }
}

/// Type-check a complete HIR file and return the populated TypeContext.
pub fn check_file_with_context(file: &mut HirFile) -> Result<TypeContext, Vec<TypeError>> {
    let ctx = check_file_internal(file, None);

    if ctx.errors.is_empty() {
        Ok(ctx)
    } else {
        Err(ctx.errors)
    }
}

/// Type-check a complete HIR file and return the context,
/// including any diagnostics.
pub fn check_file_with_context_always(file: &mut HirFile, source: Option<String>) -> TypeContext {
    check_file_internal(file, source)
}

#[cfg(test)]
mod declaration_order_tests {
    use super::{check_modules_always, ModuleToCheck};
    use std::collections::HashMap;

    fn lower(source: &str) -> tempra_hir::HirFile {
        let ast = tempra_parser::parse_source(source).unwrap();
        tempra_hir::lower_file(source, &ast)
    }

    #[test]
    fn module_input_order_flows_directly_into_interface_registration_order() {
        let mut alpha = lower("interface Alpha { fn alpha(self) -> Int }");
        let mut beta = lower("interface Beta { fn beta(self) -> Int }");
        let mut entry = lower("interface Entry { fn entry(self) -> Int }");
        let mut modules = [
            ModuleToCheck {
                name: "alpha".to_string(),
                imports: HashMap::new(),
                file: &mut alpha,
            },
            ModuleToCheck {
                name: "beta".to_string(),
                imports: HashMap::new(),
                file: &mut beta,
            },
            ModuleToCheck {
                name: "main".to_string(),
                imports: HashMap::new(),
                file: &mut entry,
            },
        ];

        let context = check_modules_always(&mut modules, &HashMap::new(), false);
        assert_eq!(
            context
                .interface_names_in_declaration_order()
                .collect::<Vec<_>>(),
            vec![
                "Collection",
                "Iterator",
                "Iterable",
                "alpha.Alpha",
                "beta.Beta",
                "main.Entry",
            ]
        );
    }
}

#[cfg(test)]
mod r2_api_parity_tests {
    use super::{check_file, check_file_with_context, check_file_with_context_always};

    fn lower(source: &str) -> tempra_hir::HirFile {
        let ast = tempra_parser::parse_source(source).expect("test source must parse");

        tempra_hir::lower_file(source, &ast)
    }

    fn assert_api_parity(source: &str) {
        let mut first = lower(source);
        let mut second = lower(source);
        let mut third = lower(source);

        let basic = check_file(&mut first);
        let with_context = check_file_with_context(&mut second);
        let always = check_file_with_context_always(&mut third, Some(source.to_string()));

        let basic_errors = basic.err().unwrap_or_default();

        let context_errors = match with_context {
            Ok(ctx) => ctx.errors,
            Err(errors) => errors,
        };

        let always_errors = always.errors;

        let basic_debug = format!("{basic_errors:?}");
        let context_debug = format!("{context_errors:?}");
        let always_debug = format!("{always_errors:?}");

        assert_eq!(
            basic_debug, context_debug,
            "check_file and check_file_with_context diverged"
        );

        assert_eq!(
            basic_debug, always_debug,
            "check_file and check_file_with_context_always diverged"
        );
    }

    #[test]
    fn parity_valid_program() {
        assert_api_parity("fn main() { let value = 42 }");
    }

    #[test]
    fn parity_direct_result_discard() {
        assert_api_parity(
            r#"
            fn load() -> String! {
                Ok("data")
            }

            fn main() {
                load()
                let value = 42
            }
            "#,
        );
    }

    #[test]
    fn parity_multiple_errors() {
        assert_api_parity(
            r#"
            fn load() -> String! {
                Ok("data")
            }

            fn main() {
                load()
                let first: Int = "wrong"
                load()
                let second: Int = "wrong"
            }
            "#,
        );
    }
}
