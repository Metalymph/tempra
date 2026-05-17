use crate::ir::TirBlockId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TirBuildError {
    #[error("cannot append instructions to block {0:?} because it is already terminated")]
    BlockAlreadyTerminated(TirBlockId),

    #[error("no current block selected for builder")]
    NoCurrentBlock,
}
