//! Interactive Read-Eval-Print Loop.
//!
//! Provides a line-by-line interactive Fero session, useful for
//! experimentation and debugging. Internally drives the same pipeline
//! as `fero run`, but in incremental, expression-at-a-time mode.

pub mod repl;
