use crate::comptime::{ConstEvalCtx, ConstIntTy, ConstValue, eval_const_expr};
use crate::parser;
use crate::source::SourceFile;

fn eval_expr(expr_src: &str, default_int_ty: ConstIntTy) -> ConstValue {
    // 通过最小文件包装，复用 parser 生成 span，避免手写 AST/span。
    let text = format!("fun main() {{ val x = {expr_src}; }}");
    let source = SourceFile::new_virtual("<mem>", text);
    let file = parser::parse_file(&source).expect("parse");

    let fun = match file.items.first() {
        Some(crate::ast::Item::Fun(f)) => f,
        other => panic!("expected one fun item, got: {other:?}"),
    };
    let body = match &fun.body {
        crate::ast::FunBody::Block(b) => b,
        crate::ast::FunBody::Missing => panic!("expected fun body block"),
    };
    let first_stmt = body.stmts.first().expect("one stmt");
    let decl = match &first_stmt.kind {
        crate::ast::StmtKind::Val(v) => v,
        other => panic!("expected val stmt, got: {other:?}"),
    };
    let init = decl.init.as_ref().expect("val init");

    let mut ctx = ConstEvalCtx::new(&source);
    ctx.default_int_ty = default_int_ty;
    eval_const_expr(ctx, init).expect("eval")
}

#[test]
fn const_eval_int_arithmetic_and_bitwise() {
    let v = eval_expr("1 + 2 * 3", ConstIntTy::host_word(true));
    assert_eq!(v, ConstValue::Int(crate::comptime::ConstInt::new(ConstIntTy::host_word(true), 7)));

    let v = eval_expr("~0", ConstIntTy { bits: 8, signed: true });
    // 8-bit ~0 == 0xff
    assert_eq!(v, ConstValue::Int(crate::comptime::ConstInt::new(ConstIntTy { bits: 8, signed: true }, 0xff)));
}

#[test]
fn const_eval_bool_and_short_circuit() {
    let v = eval_expr("true && false", ConstIntTy::host_word(true));
    assert_eq!(v, ConstValue::Bool(false));

    // short-circuit：rhs 不应被求值，因此不会触发除以 0 的错误。
    let v = eval_expr("false && (1 / 0 == 0)", ConstIntTy::host_word(true));
    assert_eq!(v, ConstValue::Bool(false));
}

#[test]
fn const_eval_shift_respects_signedness() {
    // 8-bit unsigned: -1 == 0xff; 0xff >> 1 == 0x7f
    let v = eval_expr("-1 >> 1", ConstIntTy { bits: 8, signed: false });
    assert_eq!(
        v,
        ConstValue::Int(crate::comptime::ConstInt::new(ConstIntTy { bits: 8, signed: false }, 0x7f))
    );

    // 8-bit signed arithmetic shift: -1 >> 1 == -1 (0xff)
    let v = eval_expr("-1 >> 1", ConstIntTy { bits: 8, signed: true });
    assert_eq!(
        v,
        ConstValue::Int(crate::comptime::ConstInt::new(ConstIntTy { bits: 8, signed: true }, 0xff))
    );
}
