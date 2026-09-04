#![deny(missing_docs)]

//! Pure capability analysis for verified Tempra TIR modules.
//!
//! This crate is internal to the Tempra toolchain. It does not define public
//! language syntax or integrate the analysis into the compiler driver.

mod analysis;
mod capability;
mod externs;

pub use analysis::{
    analyze, CallEdge, CapabilityAnalysis, CapabilityAnalysisError, CapabilityWitness,
    DirectCapabilityOrigin, DirectCapabilitySource, FunctionCapabilitySummary, TirLocation,
};
pub use capability::{Capability, CapabilitySet};

#[cfg(test)]
mod tests;
