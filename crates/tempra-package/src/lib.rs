//! Package manifest and build-target definitions.
//!
//! A Tempra package is described by a `tempra.toml` manifest file. This crate
//! owns the data model for that manifest as well as the [`BuildTarget`] and
//! [`BuildMode`] enums used by the driver and CLI.

pub mod manifest;
pub mod target;

pub use target::{BuildMode, BuildTarget};
