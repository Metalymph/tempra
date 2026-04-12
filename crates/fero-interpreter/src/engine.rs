use fero_ast::Module;
use fero_runtime::Value;
use fero_stdlib::standard_library_symbols;

#[derive(Debug, Default)]
pub struct Interpreter;

impl Interpreter {
    pub fn run(&self, _module: &Module) -> Value {
        let _symbols = standard_library_symbols();
        Value::Unit
    }
}
