use fero_ast::item::Item as AstItem;
use crate::item::Item as HirItem;

/// Lower an entire AST program into HIR.
///
/// This pass desugars syntactic constructs, resolves names, and prepares the
/// program for type checking.
pub fn lower_program(_items: Vec<AstItem>) -> Vec<HirItem> {
    // TODO: walk each AST item and produce the corresponding HIR item
    Vec::new()
}
