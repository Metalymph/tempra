//! Language Server Protocol adapter for Tempra.
//!
//! This crate is a **thin JSON-RPC adapter** that forwards LSP requests to the
//! in-memory IDE brain ([`tempra_analysis`]).  It contains **no compiler or
//! typechecker logic** — all semantic intelligence lives inside
//! `tempra-analysis`.
//!
//! ## Architecture
//!
//! ```text
//! Editor --JSON-RPC--> tempra-lsp --> tempra-analysis --> tempra-driver
//!                       (adapter)       (IDE brain)        (compiler)
//! ```
//!
//! ## Supported LSP capabilities (Phase 18.1E)
//! - `textDocument/didOpen` / `didChange` (full sync) / `didClose`
//! - `textDocument/hover`
//! - `textDocument/definition`
//! - `textDocument/documentSymbol`
//!
//! **Not** implemented: completion, rename, references, semantic tokens,
//! incremental text sync.

pub mod server;

use tower_lsp::{LspService, Server};

/// Set up and run the LSP server, reading from stdin and writing to stdout.
///
/// This is a long-running async function; it returns only after the client
/// sends a `shutdown` / `exit` sequence.
pub async fn run_lsp_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(server::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
