use crate::block::BasicBlock;
use tempra_hir::HirItem;

/// Lower a type-checked HIR program into MIR.
///
/// Each function body becomes a control-flow graph of [`BasicBlock`]s.
pub fn lower_to_mir(_items: &[HirItem]) -> Vec<BasicBlock> {
    // TODO: build CFG from HIR control flow
    Vec::new()
}
