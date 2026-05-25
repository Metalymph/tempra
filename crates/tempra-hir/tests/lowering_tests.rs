use tempra_hir::lower_file;
use tempra_hir::*;
use tempra_parser::parse_source;

fn assert_lowers(source: &str) -> HirFile {
    let ast = parse_source(source).expect("Failed to parse source");
    lower_file(source, &ast)
}

#[test]
fn test_basic_lowering() {
    let source = "fn main() { let x = 1; }";
    let hir = assert_lowers(source);

    assert_eq!(hir.items.len(), 1);
    if let HirItem::Function(f) = &hir.items[0] {
        assert_eq!(f.name, "main");
        assert_eq!(f.body.as_ref().unwrap().statements.len(), 1);
        if let HirStatement::Let(l) = &f.body.as_ref().unwrap().statements[0] {
            assert_eq!(l.name, "x");
            if let HirExpression::Integer(_, val) = &l.value {
                assert_eq!(val, "1");
            } else {
                panic!("Expected Integer, found {:?}", l.value);
            }
        } else {
            panic!(
                "Expected Let, found {:?}",
                f.body.as_ref().unwrap().statements[0]
            );
        }
    } else {
        panic!("Expected Function, found {:?}", hir.items[0]);
    }
}

#[test]
fn test_binary_op_lowering() {
    let source = "fn foo() { 1 + 2 * 3; }";
    let hir = assert_lowers(source);

    if let HirItem::Function(f) = &hir.items[0] {
        if let HirStatement::Expr(HirExpression::Binary(b)) =
            &f.body.as_ref().unwrap().statements[0]
        {
            assert_eq!(b.op, HirBinaryOp::Add);
            if let HirExpression::Binary(rhs) = &*b.right {
                assert_eq!(rhs.op, HirBinaryOp::Mul);
            } else {
                panic!("Expected Binary RHS, found {:?}", b.right);
            }
        } else {
            panic!(
                "Expected Binary, found {:?}",
                f.body.as_ref().unwrap().statements[0]
            );
        }
    }
}

#[test]
fn test_struct_literal_lowering() {
    let source = "fn foo() { User { name: \"a\" }; }";
    let hir = assert_lowers(source);

    if let HirItem::Function(f) = &hir.items[0] {
        if let HirStatement::Expr(HirExpression::StructLiteral(sl)) =
            &f.body.as_ref().unwrap().statements[0]
        {
            if let HirType::Named(_, name) = &sl.ty {
                assert_eq!(name, "User");
            } else {
                panic!("Expected Named type, found {:?}", sl.ty);
            }
            assert_eq!(sl.fields.len(), 1);
            assert_eq!(sl.fields[0].0, "name");
        } else {
            panic!(
                "Expected StructLiteral, found {:?}",
                f.body.as_ref().unwrap().statements[0]
            );
        }
    }
}

#[test]
fn test_if_lowering() {
    let source = "fn foo() { if true { 1 } else { 2 }; }";
    let hir = assert_lowers(source);

    if let HirItem::Function(f) = &hir.items[0] {
        if let HirStatement::Expr(HirExpression::If(i)) = &f.body.as_ref().unwrap().statements[0] {
            assert!(matches!(*i.condition, HirExpression::Bool(_, true)));
            assert_eq!(i.then_block.statements.len(), 1);
            assert!(i.else_branch.is_some());
        }
    }
}
