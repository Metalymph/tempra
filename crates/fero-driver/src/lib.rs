//! Compilation pipeline orchestration.
//!
//! The driver is the central coordinator: it reads source files, feeds them
//! through the pipeline (lexer → parser → HIR → typeck → MIR → IR → backend),
//! and returns the result. Neither the CLI nor the LSP contain compilation
//! logic directly — they delegate to the driver.

pub mod build;
pub mod install;
pub mod pipeline;

use std::path::Path;
use anyhow::Result;

/// The top-level compilation driver.
pub struct Driver;

impl Driver {
    pub fn new() -> Self {
        Self
    }

    /// Run a script: compile to IR and evaluate with the interpreter.
    pub fn run_file(&self, path: &Path) -> Result<()> {
        let source = fero_fs::read_source(path)?;
        pipeline::compile(source.contents)?;
        // TODO: hand the resulting IR module to the interpreter
        Ok(())
    }

    /// Check a source file for errors without executing it.
    pub fn check_file(&self, path: &Path) -> Result<()> {
        let source = fero_fs::read_source(path)?;
        pipeline::compile(source.contents)?;
        Ok(())
    }

    /// Compile a source file to a binary or Wasm artifact.
    pub fn build_file(&self, path: &Path) -> Result<()> {
        let source = fero_fs::read_source(path)?;
        pipeline::compile(source.contents)?;
        // TODO: route to the appropriate backend
        Ok(())
    }

    /// Build and install a Fero tool into the local bin directory.
    pub fn install_file(&self, path: &Path) -> Result<()> {
        self.build_file(path)?;
        // TODO: copy the output binary to ~/.fero/bin
        Ok(())
    }
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}
