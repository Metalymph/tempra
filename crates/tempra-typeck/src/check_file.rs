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
            for stmt in &mut m.file.top_level_statements {
                crate::check_stmt::check_statement(&mut ctx, stmt, None);
            }
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
            for stmt in &mut m.file.top_level_statements {
                crate::check_stmt::check_statement(&mut ctx, stmt, None);
            }
            ctx.scopes.pop_scope();
            ctx.current_is_fallible = false;
        }
        ctx.module_facts.insert(m.name.clone(), ctx.facts.clone());
    }

    ctx
}

/// Type-check a complete HIR file.
///
/// Returns `Ok(())` if the file is well-typed, or a list of type errors.
pub fn check_file(file: &mut HirFile) -> Result<(), Vec<TypeError>> {
    let mut ctx = TypeContext::new();
    ctx.is_single_file_fallback = true;
    ctx.interpreter = Some(tempra_interpreter::interpreter::Interpreter::new(file));

    // Pass 1: register all top-level items (functions, structs, enums)
    register_items(&mut ctx, &file.items);

    // Pass 2: check item bodies
    for item in &mut file.items {
        check_item(&mut ctx, item);
    }

    // Pass 3: check top-level statements (Script mode)
    if !file.top_level_statements.is_empty() {
        ctx.current_is_fallible = true;
        ctx.current_return_type = None; // Returns unit, but let's just leave it None to prevent explicit return
        ctx.scopes.push_scope();
        for stmt in &mut file.top_level_statements {
            crate::check_stmt::check_statement(&mut ctx, stmt, None);
        }
        ctx.scopes.pop_scope();
        ctx.current_is_fallible = false;
    }

    if ctx.errors.is_empty() {
        Ok(())
    } else {
        Err(ctx.errors)
    }
}

/// Type-check a complete HIR file and return the populated TypeContext.
///
/// This is used by downstream passes (e.g. TIR lowering) that need
/// the type information gathered during checking.
pub fn check_file_with_context(file: &mut HirFile) -> Result<TypeContext, Vec<TypeError>> {
    let mut ctx = TypeContext::new();
    ctx.is_single_file_fallback = true;
    ctx.interpreter = Some(tempra_interpreter::interpreter::Interpreter::new(file));

    register_items(&mut ctx, &file.items);

    for item in &mut file.items {
        check_item(&mut ctx, item);
    }

    if !file.top_level_statements.is_empty() {
        ctx.current_is_fallible = true;
        ctx.current_return_type = None;
        ctx.scopes.push_scope();
        for stmt in &mut file.top_level_statements {
            crate::check_stmt::check_statement(&mut ctx, stmt, None);
        }
        ctx.scopes.pop_scope();
        ctx.current_is_fallible = false;
    }

    if ctx.errors.is_empty() {
        Ok(ctx)
    } else {
        Err(ctx.errors)
    }
}

/// Type-check a complete HIR file and return the populated TypeContext, even if there are type errors.
pub fn check_file_with_context_always(file: &mut HirFile, source: Option<String>) -> TypeContext {
    let mut ctx = TypeContext::new();
    ctx.is_single_file_fallback = true;
    ctx.source_text = source;
    ctx.interpreter = Some(tempra_interpreter::interpreter::Interpreter::new(file));

    register_items(&mut ctx, &file.items);

    for item in &mut file.items {
        check_item(&mut ctx, item);
    }

    if !file.top_level_statements.is_empty() {
        ctx.current_is_fallible = true;
        ctx.current_return_type = None;
        ctx.scopes.push_scope();
        for stmt in &mut file.top_level_statements {
            crate::check_stmt::check_statement(&mut ctx, stmt, None);
        }
        ctx.scopes.pop_scope();
        ctx.current_is_fallible = false;
    }

    ctx
}
