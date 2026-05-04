/// A single instruction in the Tempra IR.
///
/// The instruction set is intentionally minimal at this stage — only the
/// operations that the interpreter and backends need to agree on.
#[derive(Debug, Clone)]
pub enum Instr {
    /// Do nothing (useful as a placeholder during lowering).
    Nop,
    /// Push a constant integer onto the operand stack.
    PushInt(i64),
    /// Add the top two stack values.
    Add,
    /// Return from the current function.
    Ret,
}
