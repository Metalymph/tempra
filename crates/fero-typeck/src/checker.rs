use crate::context::TyCtx;
use fero_hir::item::Item;
use anyhow::Result;

/// Type-check a complete HIR program.
///
/// Returns the items unchanged on success (they will carry type annotations
/// once that system is built), or an error if a type violation is found.
pub fn check_program(_ctx: &mut TyCtx, items: &[Item]) -> Result<()> {
    for _item in items {
        // TODO: walk each item and check expression types
    }
    Ok(())
}
