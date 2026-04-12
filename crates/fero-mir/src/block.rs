/// Identifies a basic block within a function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub usize);

/// A straight-line sequence of statements followed by a terminator.
///
/// Basic blocks never contain internal control flow — branches are expressed
/// by the terminator that ends each block.
#[derive(Debug)]
pub struct BasicBlock {
    pub id: BlockId,
    // TODO: statements: Vec<Statement>
    // TODO: terminator: Terminator
}
