/// The type-checking environment.
///
/// Holds the mapping from names to their inferred or declared types during a
/// single pass over the HIR. Will grow to include scope stacks, trait tables,
/// and structural interface resolution state.
pub struct TyCtx;

impl TyCtx {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TyCtx {
    fn default() -> Self {
        Self::new()
    }
}
