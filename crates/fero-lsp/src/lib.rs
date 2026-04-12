//! Language Server Protocol implementation for Fero.
//!
//! Provides IDE features (diagnostics, go-to-definition, completion) by
//! running the Fero frontend in an incremental, document-at-a-time mode
//! and communicating over the LSP JSON-RPC protocol.

pub mod server;
