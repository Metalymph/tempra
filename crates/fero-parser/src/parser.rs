use crate::grammar::fallback_module;
use fero_ast::Module;
use fero_lexer::Lexer;

pub fn parse_module(source: &str) -> Module {
    let _tokens = Lexer::new(source).tokenize();
    fallback_module()
}

#[cfg(test)]
mod tests {
    use super::parse_module;

    #[test]
    fn builds_a_fallback_module() {
        let module = parse_module("io.println(\"hello\")");
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].name, "main");
    }
}
