//! IR-to-Cranelift lowering.
//!
//! Walks the Fero IR and emits Cranelift IR (`cranelift_codegen::ir`)
//! instructions that will be compiled to native machine code.
