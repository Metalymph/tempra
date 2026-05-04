//! Language Server Protocol implementation for Tempra.
//!
//! Provides IDE features (diagnostics, go-to-definition, completion) by
//! running the Tempra frontend in an incremental, document-at-a-time mode
//! and communicating over the LSP JSON-RPC protocol.

pub mod server;
