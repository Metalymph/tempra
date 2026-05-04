//! Filesystem utilities for loading Tempra source files.
//!
//! This crate isolates all direct filesystem access so that the rest of the
//! compiler pipeline operates on in-memory [`SourceFile`] values. This makes
//! it straightforward to test compiler stages without touching disk.

use std::path::Path;

use anyhow::{Context, Result};
use tempra_span::SourceFile;

/// Read a source file from disk into memory.
///
/// Returns an error if the file does not exist or is not valid UTF-8.
pub fn read_source(path: &Path) -> Result<SourceFile> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read `{}`", path.display()))?;
    Ok(SourceFile::new(path.to_path_buf(), contents))
}
