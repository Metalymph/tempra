use anyhow::Result;
use fero_lexer::Lexer;

/// Run the full compilation pipeline on raw source text.
///
/// Currently stops after lexing — each stage will be wired in as it becomes
/// available:
///   lexer → parser → HIR lowering → type check → MIR → Fero IR
pub fn compile(source: String) -> Result<()> {
    let mut lexer = Lexer::new(&source);
    // Drain all tokens (placeholder until the parser is wired in).
    loop {
        let tok = lexer.next_token();
        if tok.kind == fero_lexer::TokenKind::EndOfFile {
            break;
        }
    }
    Ok(())
}
