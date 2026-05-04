//! Test fixture loading and path resolution.
//!
//! Fixtures are `.tempra` source files stored under `tests/` that serve as
//! golden inputs for parser, type checker, and interpreter tests.

use std::path::{Path, PathBuf};

/// Returns the absolute path to the workspace `tests/` directory.
pub fn test_fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests")
}
