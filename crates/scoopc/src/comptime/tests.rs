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
fn const_eval_splice_field_access() {
    let ty = ConstIntTy::host_word(true);

    let v = eval_expr("Point { x: 1, y: 2 }.[\"x\"]", ty);
    assert_eq!(v, mk_int(ty, 1));

    // 为后续 FieldMeta 兼容预留：允许 `{ name: \"y\" }` 形态提供字段名。
    let v = eval_expr("Point { x: 1, y: 2 }.[FieldMeta { name: \"y\" }]", ty);
    assert_eq!(v, mk_int(ty, 2));
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

#[test]
fn const_eval_comptime_block_and_if_executes_selected_branch_only() {
    let ty = ConstIntTy::host_word(true);

    // 未选中分支包含除以 0：只要 comptime if 正确裁剪分支，就不应报错。
    let consts = eval_file_consts(
        r#"
const fun choose(flag: Bool): Int {
    comptime {
        comptime if (flag) {
            10 + 1
        } else {
            1 / 0
        }
    }
}

const val A: Int = choose(true)
"#,
    );

    assert_eq!(
        consts,
        vec![ConstBinding {
            name: "A".to_string(),
            value: mk_int(ty, 11),
        }]
    );
}

#[test]
fn const_eval_comptime_if_supports_else_if_chain() {
    let ty = ConstIntTy::host_word(true);

    let consts = eval_file_consts(
        r#"
const fun pick(x: Int): Int {
    comptime if (x == 0) {
        0
    } else comptime if (x == 1) {
        10
    } else {
        20
    }
}

const val A: Int = pick(0)
const val B: Int = pick(1)
const val C: Int = pick(2)
"#,
    );

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "A".to_string(),
                value: mk_int(ty, 0),
            },
            ConstBinding {
                name: "B".to_string(),
                value: mk_int(ty, 10),
            },
            ConstBinding {
                name: "C".to_string(),
                value: mk_int(ty, 20),
            },
        ]
    );
}

#[test]
fn const_eval_comptime_if_condition_must_be_bool() {
    let source = SourceFile::new_virtual(
        "<mem>",
        r#"
const fun bad(): Int {
    comptime if (1) { 1 } else { 2 }
}

const val X: Int = bad()
"#
        .to_string(),
    );
    let file = parser::parse_file(&source).expect("parse");
    let err = eval_const_bindings_in_file(&source, &file).unwrap_err();
    assert_eq!(
        err.code().unwrap().to_string(),
        "scoop::comptime::operand_type_mismatch"
    );
}

#[test]
fn const_eval_reflection_intrinsics_v0_basic() {
    let ty = ConstIntTy::host_word(true);

    let consts = eval_file_consts(
        r#"
struct Point(val x: Int, val y: Int) {
    val tag: String
    // 计算属性：v0 的 fieldsOf 不应把它当作字段。
    val computed: Int get() = 0
}

const val N: String = nameOf<Point>()
const val S: Int = sizeOf<Int64>()
const val F = fieldsOf<Point>()
"#,
    );

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "N".to_string(),
                value: ConstValue::String("Point".to_string()),
            },
            ConstBinding {
                name: "S".to_string(),
                value: mk_int(ty, 8),
            },
            ConstBinding {
                name: "F".to_string(),
                value: ConstValue::Tuple(vec![
                    ConstValue::String("x".to_string()),
                    ConstValue::String("y".to_string()),
                    ConstValue::String("tag".to_string()),
                ]),
            },
        ]
    );
}
