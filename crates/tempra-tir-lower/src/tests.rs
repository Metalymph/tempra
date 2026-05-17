use crate::lower::lower_function;
use tempra_hir::*;
use tempra_span::Span;
use tempra_tir::builder::ModuleBuilder;
use tempra_tir::ir::*;
use tempra_tir::pretty::format_tir_module;
use tempra_tir::verifier::verify_module;
use tempra_typeck::context::{FunctionParam, FunctionSignature, TypeContext};
use tempra_typeck::Type;

fn empty_span() -> Span {
    Span::new(0, 0)
}

fn create_test_ctx() -> TypeContext {
    TypeContext::new()
}

fn create_function(name: &str, ret_ty: HirType, stmts: Vec<HirStatement>) -> HirFunction {
    HirFunction {
        name: name.to_string(),
        span: empty_span(),
        visibility: tempra_ast::Visibility::Private,
        generic_parameters: vec![],
        parameters: vec![],
        return_type: Some(ret_ty),
        body: HirBlock {
            span: empty_span(),
            statements: stmts,
        },
        is_fallible: false,
    }
}

#[test]
fn lower_const_return_i64() {
    let func = create_function(
        "test",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![HirStatement::Return(Some(HirExpression::Integer(
            empty_span(),
            "42".to_string(),
        )))],
    );

    let ctx = create_test_ctx();
    let mut builder = ModuleBuilder::new();
    let tir_func = lower_function(&func, &ctx, &mut builder, None).unwrap();
    builder.add_function(tir_func);
    let module = builder.module;

    assert!(verify_module(&module).is_ok());
    assert_eq!(module.functions.len(), 1);
    let f = &module.functions[0];
    assert!(matches!(
        f.blocks[0].terminator,
        Some(tempra_tir::ir::TirTerminator::Ret(Some(_)))
    ));
}

#[test]
fn lower_add_return_i64() {
    let expr = HirExpression::Binary(HirBinary {
        span: empty_span(),
        left: Box::new(HirExpression::Integer(empty_span(), "10".to_string())),
        op: HirBinaryOp::Add,
        right: Box::new(HirExpression::Integer(empty_span(), "20".to_string())),
    });

    let func = create_function(
        "test_add",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![HirStatement::Return(Some(expr))],
    );

    let ctx = create_test_ctx();
    let mut builder = ModuleBuilder::new();
    let tir_func = lower_function(&func, &ctx, &mut builder, None).unwrap();
    builder.add_function(tir_func);

    assert!(verify_module(&builder.module).is_ok());

    // Golden test
    let out = format_tir_module(&builder.module);
    assert!(out.contains("%0 = const 10i64"));
    assert!(out.contains("%1 = const 20i64"));
    assert!(out.contains("%2 = add %0, %1"));
    assert!(out.contains("ret %2"));
}

#[test]
fn lower_param_return() {
    let mut func = create_function(
        "id",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![HirStatement::Return(Some(HirExpression::Name(
            empty_span(),
            "x".to_string(),
        )))],
    );
    func.parameters.push(HirParameter {
        name: "x".to_string(),
        ty: HirType::Named(empty_span(), "Int64".to_string()),
        span: empty_span(),
        is_inout: false,
        kind: tempra_ast::ParameterKind::Runtime,
    });

    let ctx = create_test_ctx();
    let mut builder = ModuleBuilder::new();
    let tir_func = lower_function(&func, &ctx, &mut builder, None).unwrap();
    builder.add_function(tir_func);

    assert!(verify_module(&builder.module).is_ok());
    let out = format_tir_module(&builder.module);
    assert!(out.contains("fn id (%0: i64) -> i64"));
}

#[test]
fn var_assign_return_ok() {
    let stmt1 = HirStatement::Var(HirVar {
        span: empty_span(),
        name: "mut_x".to_string(),
        ty: None,
        value: HirExpression::Integer(empty_span(), "100".to_string()),
    });

    let assign = HirExpression::Binary(HirBinary {
        span: empty_span(),
        left: Box::new(HirExpression::Name(empty_span(), "mut_x".to_string())),
        op: HirBinaryOp::Assign,
        right: Box::new(HirExpression::Integer(empty_span(), "200".to_string())),
    });

    let stmt2 = HirStatement::Expr(assign);
    let stmt3 = HirStatement::Return(Some(HirExpression::Name(empty_span(), "mut_x".to_string())));

    let func = create_function(
        "test_var",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![stmt1, stmt2, stmt3],
    );

    let ctx = create_test_ctx();
    let mut builder = ModuleBuilder::new();
    let tir_func = lower_function(&func, &ctx, &mut builder, None).unwrap();
    builder.add_function(tir_func);

    assert!(verify_module(&builder.module).is_ok());
}

#[test]
fn let_assign_error() {
    let stmt1 = HirStatement::Let(HirLet {
        span: empty_span(),
        name: "x".to_string(),
        ty: None,
        value: HirExpression::Integer(empty_span(), "100".to_string()),
    });

    let assign = HirExpression::Binary(HirBinary {
        span: empty_span(),
        left: Box::new(HirExpression::Name(empty_span(), "x".to_string())),
        op: HirBinaryOp::Assign,
        right: Box::new(HirExpression::Integer(empty_span(), "200".to_string())),
    });

    let func = create_function(
        "test_let_err",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![stmt1, HirStatement::Expr(assign)],
    );

    let ctx = create_test_ctx();
    let mut builder = ModuleBuilder::new();
    let res = lower_function(&func, &ctx, &mut builder, None);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("cannot assign to let binding"));
}

#[test]
fn call_known_function_ok() {
    let mut ctx = create_test_ctx();
    ctx.functions.insert(
        "other".to_string(),
        FunctionSignature {
            visibility: tempra_ast::Visibility::Public,
            generic_params: vec![],
            params: vec![FunctionParam {
                name: "a".to_string(),
                ty: Type::Int64,
                is_inout: false,
                kind: tempra_ast::ParameterKind::Runtime,
            }],
            return_type: Type::Int64,
            is_fallible: false,
        },
    );

    let call = HirExpression::Call(HirCall {
        span: empty_span(),
        callee: Box::new(HirExpression::Name(empty_span(), "other".to_string())),
        type_args: None,
        args: vec![HirCallArgument::Positional(HirExpression::Integer(
            empty_span(),
            "5".to_string(),
        ))],
    });

    let func = create_function(
        "main",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![HirStatement::Return(Some(call))],
    );

    let mut builder = ModuleBuilder::new();
    builder.add_declaration("other", vec![TirType::Int64], TirType::Int64);
    let tir_func = lower_function(&func, &ctx, &mut builder, None).unwrap();
    builder.add_function(tir_func);

    if let Err(e) = verify_module(&builder.module) {
        panic!("Verification failed: {:?}", e);
    }
    let out = format_tir_module(&builder.module);
    assert!(out.contains("call @0 (%0)"));
}

#[test]
fn if_both_branches_return_ok() {
    let cond = HirExpression::Bool(empty_span(), true);
    let if_expr = HirExpression::If(HirIf {
        span: empty_span(),
        condition: Box::new(cond),
        then_block: HirBlock {
            span: empty_span(),
            statements: vec![HirStatement::Return(Some(HirExpression::Integer(
                empty_span(),
                "1".to_string(),
            )))],
        },
        else_branch: Some(Box::new(HirExpression::Block(HirBlock {
            span: empty_span(),
            statements: vec![HirStatement::Return(Some(HirExpression::Integer(
                empty_span(),
                "2".to_string(),
            )))],
        }))),
        is_comptime: false,
    });

    let func = create_function(
        "test_if",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![HirStatement::Expr(if_expr)],
    );

    let ctx = create_test_ctx();
    let mut builder = ModuleBuilder::new();
    let tir_func = lower_function(&func, &ctx, &mut builder, None).unwrap();
    builder.add_function(tir_func);

    assert!(verify_module(&builder.module).is_ok());
}

#[test]
fn if_missing_return_error() {
    let cond = HirExpression::Bool(empty_span(), true);
    let if_expr = HirExpression::If(HirIf {
        span: empty_span(),
        condition: Box::new(cond),
        then_block: HirBlock {
            span: empty_span(),
            statements: vec![HirStatement::Expr(HirExpression::Integer(
                empty_span(),
                "1".to_string(),
            ))],
        },
        else_branch: Some(Box::new(HirExpression::Block(HirBlock {
            span: empty_span(),
            statements: vec![HirStatement::Return(Some(HirExpression::Integer(
                empty_span(),
                "2".to_string(),
            )))],
        }))),
        is_comptime: false,
    });

    let func = create_function(
        "test_if_err",
        HirType::Named(empty_span(), "Int64".to_string()),
        vec![HirStatement::Expr(if_expr)],
    );

    let ctx = create_test_ctx();
    let mut builder = ModuleBuilder::new();
    let res = lower_function(&func, &ctx, &mut builder, None);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("missing return"));
}
