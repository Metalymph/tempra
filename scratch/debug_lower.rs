use tempra_hir::*;
use tempra_tir_lower::lower_function;
use tempra_typeck::context::TypeContext;
use tempra_tir::ir::*;

fn main() {
    let mut ctx = TypeContext::new();
    // Setup context if needed
    
    let source = r#"
        fn test() -> [Int64] {
            let xs = [1, 2, 3]
            let ys = for x in xs {
                x + 10
            }
            return ys
        }
    "#;
    
    // This is hard to run without full pipeline setup.
    // I'll just check the code again.
}
