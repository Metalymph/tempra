//! Entry point for type-checking a Tempra HIR file.

use crate::check_item::{check_item, register_items};
use crate::context::TypeContext;
use crate::error::TypeError;
use std::collections::HashMap;
use tempra_hir::HirFile;

pub struct ModuleToCheck<'a> {
    pub name: String,
    pub imports: HashMap<String, String>,
    pub file: &'a mut HirFile,
}

pub fn check_modules(
    modules: &mut [ModuleToCheck],
    is_single_file_fallback: bool,
) -> Result<TypeContext, Vec<TypeError>> {
    let mut ctx = TypeContext::new();
    ctx.is_single_file_fallback = is_single_file_fallback;
    ctx.modules = modules.iter().map(|m| m.name.clone()).collect();

    // Pass 1: register all items in all modules
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
        register_items(&mut ctx, &m.file.items);
    }

    // Pass 2: check item bodies in all modules
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
        for item in &mut m.file.items {
            check_item(&mut ctx, item);
        }
    }

    // Pass 3: check top-level statements in all modules
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
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
    let mut ctx = TypeContext::new();
    ctx.is_single_file_fallback = is_single_file_fallback;
    ctx.modules = modules.iter().map(|m| m.name.clone()).collect();

    // Pass 1: register all items
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
        register_items(&mut ctx, &m.file.items);
    }

    // Pass 2: check items
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
        if let Some(source) = source_texts.get(&m.name) {
            ctx.source_text = Some(source.clone());
        } else {
            ctx.source_text = None;
        }
        for item in &mut m.file.items {
            check_item(&mut ctx, item);
        }
    }

    // Pass 3: check top-level statements
    for m in modules.iter_mut() {
        ctx.current_module = m.name.clone();
        ctx.imports = m.imports.clone();
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
