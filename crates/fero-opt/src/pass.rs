//! Individual optimization passes and the pipeline that chains them.

use fero_ir::module::Module;

/// Trait implemented by each optimisation pass.
pub trait Pass {
    /// A human-readable name for this pass (used in diagnostics and logging).
    fn name(&self) -> &'static str;
    /// Apply the pass to `module`, mutating it in place.
    fn run(&self, module: &mut Module);
}

/// Dead-code elimination: removes unreachable functions and instructions.
pub struct DeadCodeElimination;

impl Pass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dce"
    }
    fn run(&self, _module: &mut Module) {
        // TODO: walk the call graph and prune unreachable functions
    }
}

/// Constant folding and algebraic simplification.
pub struct Simplify;

impl Pass for Simplify {
    fn name(&self) -> &'static str {
        "simplify"
    }
    fn run(&self, _module: &mut Module) {
        // TODO: fold constant expressions, simplify identity operations
    }
}

/// ARC (Automatic Reference Counting) insertion and elision.
///
/// Part of the `fast` build mode — uses Perceus-style analysis to eliminate
/// redundant reference-count operations.
pub struct ArcOptimisation;

impl Pass for ArcOptimisation {
    fn name(&self) -> &'static str {
        "arc"
    }
    fn run(&self, _module: &mut Module) {
        // TODO: insert/elide ARC operations based on ownership analysis
    }
}

/// Escape analysis for stack promotion.
///
/// Values that do not escape their defining scope can be allocated on the
/// stack instead of the heap, avoiding ARC overhead entirely.
pub struct EscapeAnalysis;

impl Pass for EscapeAnalysis {
    fn name(&self) -> &'static str {
        "escape"
    }
    fn run(&self, _module: &mut Module) {
        // TODO: mark non-escaping allocations for stack promotion
    }
}

/// Run the default optimisation pipeline on a module.
pub fn run_default_pipeline(module: &mut Module) {
    let passes: Vec<Box<dyn Pass>> = vec![
        Box::new(DeadCodeElimination),
        Box::new(Simplify),
        Box::new(ArcOptimisation),
        Box::new(EscapeAnalysis),
    ];
    for pass in &passes {
        pass.run(module);
    }
}
