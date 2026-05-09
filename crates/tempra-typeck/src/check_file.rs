//! Entry point for type-checking a Tempra HIR file.

use crate::check_item::{check_item, register_items};
use crate::context::TypeContext;
use crate::error::TypeError;
use tempra_hir::HirFile;

/// Type-check a complete HIR file.
///
/// Returns `Ok(())` if the file is well-typed, or a list of type errors.
pub fn check_file(file: &mut HirFile) -> Result<(), Vec<TypeError>> {
    let mut ctx = TypeContext::new();
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
