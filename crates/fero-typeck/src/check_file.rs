use fero_hir::HirFile;
use crate::error::TypeError;
use crate::context::TypeContext;
use crate::check_item::{register_items, check_item};

/// Type-check a complete HIR file.
///
/// Returns `Ok(())` if the file is well-typed, or a list of type errors.
pub fn check_file(file: &HirFile) -> Result<(), Vec<TypeError>> {
    let mut ctx = TypeContext::new();

    // Pass 1: register all top-level items (functions, structs, enums)
    register_items(&mut ctx, &file.items);

    // Pass 2: check item bodies
    for item in &file.items {
        check_item(&mut ctx, item);
    }

    if ctx.errors.is_empty() {
        Ok(())
    } else {
        Err(ctx.errors)
    }
}
