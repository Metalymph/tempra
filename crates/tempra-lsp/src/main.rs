//! `tempra-lsp` binary — Tempra Language Server Protocol adapter.
//!
//! Communicates via JSON-RPC over stdin/stdout (standard LSP transport).
//!
//! Usage (editor configuration):
//!   command: tempra-lsp

#[tokio::main]
async fn main() {
    tempra_lsp::run_lsp_server().await;
}
