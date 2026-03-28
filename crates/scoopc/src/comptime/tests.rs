use std::collections::BTreeMap;

use miette::Diagnostic;

use crate::comptime::{ConstEnum, ConstEvalCtx, ConstInt, ConstIntTy, ConstStruct, ConstValue, eval_const_expr};
use crate::comptime::{ConstBinding, eval_const_bindings_in_file};
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

fn mk_int(ty: ConstIntTy, raw: u128) -> ConstValue {
    ConstValue::Int(ConstInt::new(ty, raw))
}

fn eval_file_consts(file_src: &str) -> Vec<ConstBinding> {
    let source = SourceFile::new_virtual("<mem>", file_src.to_string());
    let file = parser::parse_file(&source).expect("parse");
    eval_const_bindings_in_file(&source, &file).expect("eval file consts")
}

#[test]
fn const_eval_int_arithmetic_and_bitwise() {
    let v = eval_expr("1 + 2 * 3", ConstIntTy::host_word(true));
    assert_eq!(v, mk_int(ConstIntTy::host_word(true), 7));

    let v = eval_expr("~0", ConstIntTy { bits: 8, signed: true });
    // 8-bit ~0 == 0xff
    assert_eq!(v, mk_int(ConstIntTy { bits: 8, signed: true }, 0xff));
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
        mk_int(ConstIntTy { bits: 8, signed: false }, 0x7f)
    );

    // 8-bit signed arithmetic shift: -1 >> 1 == -1 (0xff)
    let v = eval_expr("-1 >> 1", ConstIntTy { bits: 8, signed: true });
    assert_eq!(
        v,
        mk_int(ConstIntTy { bits: 8, signed: true }, 0xff)
    );
}

#[test]
fn const_eval_tuple_construct_and_access() {
    let ty = ConstIntTy::host_word(true);

    let v = eval_expr("(1, 2)", ty);
    assert_eq!(v, ConstValue::Tuple(vec![mk_int(ty, 1), mk_int(ty, 2)]));

    let v = eval_expr("(1, 2)._0", ty);
    assert_eq!(v, mk_int(ty, 1));

    let v = eval_expr("(1, 2)._1", ty);
    assert_eq!(v, mk_int(ty, 2));
}

#[test]
fn const_eval_struct_construct_and_access() {
    let ty = ConstIntTy::host_word(true);

    let v = eval_expr("Point { x: 1, y: 2 }", ty);
    let fields: BTreeMap<String, ConstValue> =
        BTreeMap::from([("x".to_string(), mk_int(ty, 1)), ("y".to_string(), mk_int(ty, 2))]);
    assert_eq!(
        v,
        ConstValue::Struct(ConstStruct {
            ty: "Point".to_string(),
            fields
        })
    );

    let v = eval_expr("Point { x: 1, y: 2 }.x", ty);
    assert_eq!(v, mk_int(ty, 1));
}

#[test]
fn const_eval_enum_construct_and_access() {
    let ty = ConstIntTy::host_word(true);

    let v = eval_expr("Opt.None", ty);
    assert_eq!(
        v,
        ConstValue::Enum(ConstEnum {
            ty: Some("Opt".to_string()),
            variant: "None".to_string(),
            payload: Vec::new()
        })
    );

    let v = eval_expr("Opt.Some(42)", ty);
    assert_eq!(
        v,
        ConstValue::Enum(ConstEnum {
            ty: Some("Opt".to_string()),
            variant: "Some".to_string(),
            payload: vec![mk_int(ty, 42)]
        })
    );

    let v = eval_expr("Opt.Some(42)._0", ty);
    assert_eq!(v, mk_int(ty, 42));
}

#[test]
fn const_eval_const_fun_call_and_const_val_fold() {
    let ty = ConstIntTy::host_word(true);

    let consts = eval_file_consts(
        r#"
const fun add(a: Int, b: Int): Int {
    val c = a + b
    c
}

const fun add3(x: Int): Int {
    val y = x + 2
    y + 1
}

const val A: Int = add(1, 2)
const val B: Int = add3(10)
"#,
    );

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "A".to_string(),
                value: mk_int(ty, 3),
            },
            ConstBinding {
                name: "B".to_string(),
                value: mk_int(ty, 13),
            }
        ]
    );
}

#[test]
fn const_eval_calling_non_const_fun_is_error() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
fun add(a: Int, b: Int): Int { return a + b }
const val X: Int = add(1, 2)
"#
        .to_string(),
    );
    let file = parser::parse_file(&source).expect("parse");
    let err = eval_const_bindings_in_file(&source, &file).unwrap_err();
    assert_eq!(
        err.code().unwrap().to_string(),
        "scoop::comptime::callee_not_const_fun"
    );
}

#[test]
fn const_eval_recursion_limit_has_stable_code() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
const fun loop(): Int { loop() }
const val X: Int = loop()
"#
        .to_string(),
    );
    let file = parser::parse_file(&source).expect("parse");
    let err = eval_const_bindings_in_file(&source, &file).unwrap_err();
    assert_eq!(
        err.code().unwrap().to_string(),
        "scoop::comptime::recursion_limit_exceeded"
    );
}
