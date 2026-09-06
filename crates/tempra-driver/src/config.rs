//! Explicit inputs owned by a file/module compilation.

use std::path::{Path, PathBuf};

/// File/module compilation inputs that are independent of source contents.
///
/// The standard-library root is the `std/` directory itself. A missing root is
/// valid until a source file imports a standard-library module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationConfig {
    stdlib_root: Option<PathBuf>,
}

impl CompilationConfig {
    /// Creates a configuration with an explicitly selected standard-library root.
    ///
    /// Relative paths are made absolute once, at this configuration boundary.
    /// The path is not canonicalized or required to exist until an import uses it.
    pub fn with_stdlib_root(root: impl AsRef<Path>) -> std::io::Result<Self> {
        Ok(Self {
            stdlib_root: Some(std::path::absolute(root)?),
        })
    }

    /// Creates a configuration without a standard-library root.
    ///
    /// Sources without standard-library imports remain valid with this config.
    pub const fn without_stdlib() -> Self {
        Self { stdlib_root: None }
    }

    /// Returns the resolved standard-library root, if one is available.
    pub fn stdlib_root(&self) -> Option<&Path> {
        self.stdlib_root.as_deref()
    }
}

impl Default for CompilationConfig {
    fn default() -> Self {
        let stdlib_root = std::env::current_exe()
            .ok()
            .and_then(|executable| discover_stdlib_from_executable(&executable));
        Self { stdlib_root }
    }
}

/// Finds the nearest `std/` directory owned by an executable's ancestor tree.
///
/// This helper is intentionally independent of the process working directory.
/// The executable path supplied by the caller is expected to be absolute, as
/// paths returned by [`std::env::current_exe`] are.
pub fn discover_stdlib_from_executable(executable: &Path) -> Option<PathBuf> {
    executable
        .parent()?
        .ancestors()
        .map(|ancestor| ancestor.join("std"))
        .find(|candidate| candidate.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_is_anchored_to_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let toolchain = dir.path().join("toolchain");
        let executable = toolchain.join("target/debug/deps/tempra-test");
        let stdlib = toolchain.join("std");
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&stdlib).unwrap();

        assert_eq!(discover_stdlib_from_executable(&executable), Some(stdlib));
    }

    #[test]
    fn nearest_executable_ancestor_wins() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path().join("std");
        let inner_root = dir.path().join("nested/toolchain");
        let inner = inner_root.join("std");
        let executable = inner_root.join("bin/tempra");
        std::fs::create_dir_all(&outer).unwrap();
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();

        assert_eq!(discover_stdlib_from_executable(&executable), Some(inner));
    }

    #[test]
    fn explicit_relative_root_is_resolved_once() {
        let config = CompilationConfig::with_stdlib_root(".").unwrap();
        assert!(config.stdlib_root().unwrap().is_absolute());
        assert_eq!(
            config.stdlib_root().unwrap(),
            std::path::absolute(".").unwrap()
        );
    }
}
