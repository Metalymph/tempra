#![deny(missing_docs)]

//! Pure capability analysis for verified Tempra TIR modules.
//!
//! This crate is internal to the Tempra toolchain. It does not define public
//! language syntax or integrate the analysis into the compiler driver.

mod analysis;
mod capability;
mod externs;
mod facts;
mod operations;

pub use analysis::{
    analyze, CallEdge, CapabilityAnalysis, CapabilityAnalysisError, CapabilityWitness,
    DirectCapabilityOrigin, DirectCapabilitySource, FunctionCapabilitySummary,
    FunctionSpecializedEffectSummary, SpecializedEffectWitness, TirLocation,
};
pub use capability::{Capability, CapabilitySet};
pub use facts::{
    LocalEffectFact, ResourceFact, ResourceKnowledge, ResourceValue, SpecializedOperationFact,
    UnclassifiedOperationFact,
};
pub use operations::{EffectOperation, EffectParameter};

#[cfg(test)]
mod tests;
