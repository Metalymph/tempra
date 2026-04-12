//! IR optimization passes.
//!
//! All semantic-preserving transformations on the Fero IR happen here,
//! *before* the IR reaches any backend. Passes are grouped into a pipeline
//! and applied in order.

pub mod pass;
