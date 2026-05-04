use std::path::PathBuf;

/// An in-memory representation of a single source file.
///
/// The compiler reads each `.tempra` file into a `SourceFile` before handing it
/// to the lexer. Keeping the full source text around allows later stages
/// (diagnostics, IDE tooling) to produce human-readable error messages without
/// re-reading from disk.
#[derive(Debug)]
pub struct SourceFile {
    /// The filesystem path this source was loaded from.
    pub path: PathBuf,
    /// The complete UTF-8 contents of the file.
    pub contents: String,
}

impl SourceFile {
    pub fn new(path: PathBuf, contents: String) -> Self {
        Self { path, contents }
    }
}
