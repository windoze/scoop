use std::collections::BTreeMap;

use miette::Diagnostic;

use crate::comptime::{ConstBinding, eval_const_bindings_in_file};
use crate::comptime::{
    ConstEnum, ConstEvalCtx, ConstFloat, ConstInt, ConstIntTy, ConstStruct, ConstValue,
    eval_const_expr,
};
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

fn mk_float64(value: f64) -> ConstValue {
    ConstValue::Float(ConstFloat::from_f64(value))
}

fn mk_float32(value: f32) -> ConstValue {
    ConstValue::Float(ConstFloat::from_f32(value))
}

fn mk_float_hash(int_ty: ConstIntTy, bits: u64) -> ConstValue {
    let mut mixed = bits;
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^= mixed >> 31;
    ConstValue::Int(ConstInt::new(int_ty, mixed as i64 as u128))
}

fn mk_type_kind(variant: &str) -> ConstValue {
    ConstValue::Enum(ConstEnum {
        ty: Some("TypeKind".to_string()),
        variant: variant.to_string(),
        payload: Vec::new(),
    })
}

fn mk_type_meta(name: &str, kind: &str) -> ConstValue {
    ConstValue::Struct(ConstStruct {
        ty: "TypeMeta".to_string(),
        fields: BTreeMap::from([
            ("annotations".to_string(), ConstValue::Tuple(Vec::new())),
            ("kind".to_string(), mk_type_kind(kind)),
            ("name".to_string(), ConstValue::String(name.to_string())),
        ]),
    })
}

fn mk_field_meta(int_ty: ConstIntTy, name: &str, ty_name: &str, index: u128) -> ConstValue {
    ConstValue::Struct(ConstStruct {
        ty: "FieldMeta".to_string(),
        fields: BTreeMap::from([
            ("annotations".to_string(), ConstValue::Tuple(Vec::new())),
            ("index".to_string(), mk_int(int_ty, index)),
            ("name".to_string(), ConstValue::String(name.to_string())),
            ("type".to_string(), mk_type_meta(ty_name, "Primitive")),
        ]),
    })
}

fn mk_param_meta(int_ty: ConstIntTy, name: &str, ty_name: &str, index: u128) -> ConstValue {
    ConstValue::Struct(ConstStruct {
        ty: "ParamMeta".to_string(),
        fields: BTreeMap::from([
            ("annotations".to_string(), ConstValue::Tuple(Vec::new())),
            ("index".to_string(), mk_int(int_ty, index)),
            ("name".to_string(), ConstValue::String(name.to_string())),
            ("type".to_string(), mk_type_meta(ty_name, "Primitive")),
        ]),
    })
}

fn mk_variant_meta(
    int_ty: ConstIntTy,
    name: &str,
    fields: Vec<ConstValue>,
    index: u128,
) -> ConstValue {
    ConstValue::Struct(ConstStruct {
        ty: "VariantMeta".to_string(),
        fields: BTreeMap::from([
            ("annotations".to_string(), ConstValue::Tuple(Vec::new())),
            ("fields".to_string(), ConstValue::Tuple(fields)),
            ("index".to_string(), mk_int(int_ty, index)),
            ("name".to_string(), ConstValue::String(name.to_string())),
        ]),
    })
}

fn mk_annotation_arg_meta(name: &str, value: ConstValue) -> ConstValue {
    ConstValue::Struct(ConstStruct {
        ty: "AnnotationArgMeta".to_string(),
        fields: BTreeMap::from([
            ("name".to_string(), ConstValue::String(name.to_string())),
            ("value".to_string(), value),
        ]),
    })
}

fn mk_annotation_meta(name: &str, args: Vec<ConstValue>) -> ConstValue {
    ConstValue::Struct(ConstStruct {
        ty: "AnnotationMeta".to_string(),
        fields: BTreeMap::from([
            ("args".to_string(), ConstValue::Tuple(args)),
            ("name".to_string(), ConstValue::String(name.to_string())),
        ]),
    })
}

fn mk_platform(triple: &str, arch: &str, vendor: &str, os: &str, env: &str) -> ConstValue {
    ConstValue::Struct(ConstStruct {
        ty: "Platform".to_string(),
        fields: BTreeMap::from([
            ("arch".to_string(), ConstValue::String(arch.to_string())),
            ("env".to_string(), ConstValue::String(env.to_string())),
            ("os".to_string(), ConstValue::String(os.to_string())),
            ("triple".to_string(), ConstValue::String(triple.to_string())),
            ("vendor".to_string(), ConstValue::String(vendor.to_string())),
        ]),
    })
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

    let v = eval_expr(
        "~0",
        ConstIntTy {
            bits: 8,
            signed: true,
        },
    );
    // 8-bit ~0 == 0xff
    assert_eq!(
        v,
        mk_int(
            ConstIntTy {
                bits: 8,
                signed: true
            },
            0xff
        )
    );
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
fn const_eval_string_trim_indent_folds() {
    let ty = ConstIntTy::host_word(true);
    let v = eval_expr(
        r#""""
    a
      b
    c
""".trimIndent()"#,
        ty,
    );
    assert_eq!(v, ConstValue::String("a\n  b\nc".to_string()));
}

#[test]
fn const_eval_float_literals_and_arithmetic() {
    let ty = ConstIntTy::host_word(true);

    assert_eq!(eval_expr("1.5 + 0.5", ty), mk_float64(2.0));
    assert_eq!(eval_expr("1.5f + 0.5", ty), mk_float32(2.0));
    assert_eq!(eval_expr("-0.5f", ty), mk_float32(-0.5));
    assert_eq!(eval_expr("3.0 > 2.0", ty), ConstValue::Bool(true));

    // 与运行期/LLVM 路径保持一致：NaN == NaN 为 false，NaN != NaN 为 true。
    assert_eq!(
        eval_expr("(0.0 / 0.0) == (0.0 / 0.0)", ty),
        ConstValue::Bool(false)
    );
    assert_eq!(
        eval_expr("(0.0 / 0.0) != (0.0 / 0.0)", ty),
        ConstValue::Bool(true)
    );
}

#[test]
fn const_eval_float32_annotations_and_const_fun_preserve_precision() {
    let consts = eval_file_consts(
        r#"
const fun bump(x: Float32): Float32 {
    val mid: Float32 = x + 0.25
    mid
}

const val BASE: Float32 = 1.5
const val SUM: Float32 = BASE + 0.5f
const val FROM_FUN: Float32 = bump(1.75)
const val CMP: Bool = SUM == 2.0
"#,
    );

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "BASE".to_string(),
                value: mk_float32(1.5),
            },
            ConstBinding {
                name: "SUM".to_string(),
                value: mk_float32(2.0),
            },
            ConstBinding {
                name: "FROM_FUN".to_string(),
                value: mk_float32(2.0),
            },
            ConstBinding {
                name: "CMP".to_string(),
                value: ConstValue::Bool(true),
            },
        ]
    );
}

#[test]
fn const_eval_float_builtin_methods() {
    let ty = ConstIntTy::host_word(true);

    assert_eq!(eval_expr("3.75.toInt()", ty), mk_int(ty, 3));
    assert_eq!(eval_expr("(0.0 / 0.0).toInt()", ty), mk_int(ty, 0));
    assert_eq!(
        eval_expr("125.0.toString()", ty),
        ConstValue::String("125.0".to_string())
    );
    assert_eq!(
        eval_expr("0.5f.toString()", ty),
        ConstValue::String("0.5".to_string())
    );
    assert_eq!(eval_expr("(-2.5).abs()", ty), mk_float64(2.5));
    assert_eq!(eval_expr("(-0.5f).abs()", ty), mk_float32(0.5));
    assert_eq!(eval_expr("(0.0 / 0.0).isNaN()", ty), ConstValue::Bool(true));
    assert_eq!(
        eval_expr("(1.0 / 0.0).isInfinite()", ty),
        ConstValue::Bool(true)
    );
    assert_eq!(
        eval_expr("1.5.hash()", ty),
        mk_float_hash(ty, 1.5f64.to_bits())
    );
    assert_eq!(
        eval_expr("1.5f.hash()", ty),
        mk_float_hash(ty, u64::from((1.5f32).to_bits()))
    );
}

#[test]
fn const_eval_shift_respects_signedness() {
    // 8-bit unsigned: -1 == 0xff; 0xff >> 1 == 0x7f
    let v = eval_expr(
        "-1 >> 1",
        ConstIntTy {
            bits: 8,
            signed: false,
        },
    );
    assert_eq!(
        v,
        mk_int(
            ConstIntTy {
                bits: 8,
                signed: false
            },
            0x7f
        )
    );

    // 8-bit signed arithmetic shift: -1 >> 1 == -1 (0xff)
    let v = eval_expr(
        "-1 >> 1",
        ConstIntTy {
            bits: 8,
            signed: true,
        },
    );
    assert_eq!(
        v,
        mk_int(
            ConstIntTy {
                bits: 8,
                signed: true
            },
            0xff
        )
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
    let fields: BTreeMap<String, ConstValue> = BTreeMap::from([
        ("x".to_string(), mk_int(ty, 1)),
        ("y".to_string(), mk_int(ty, 2)),
    ]);
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
fn const_eval_comptime_for_supports_range_and_array_and_tuple() {
    let ty = ConstIntTy::host_word(true);

    let consts = eval_file_consts(
        r#"
const fun lastRange(): Int {
    comptime for (i in 1..3) { i }
}

const fun lastArray(): Int {
    comptime for (x in [10, 20, 30]) { x }
}

const fun lastTuple(): Int {
    comptime for (x in (7, 8, 9)) { x }
}

const val A: Int = lastRange()
const val B: Int = lastArray()
const val C: Int = lastTuple()
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
                value: mk_int(ty, 30),
            },
            ConstBinding {
                name: "C".to_string(),
                value: mk_int(ty, 9),
            },
        ]
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
const val F0N: String = fieldsOf<Point>()._0.name
const val F0T: String = fieldsOf<Point>()._0.type.name
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
                    mk_field_meta(ty, "x", "Int", 0),
                    mk_field_meta(ty, "y", "Int", 1),
                    mk_field_meta(ty, "tag", "String", 2),
                ]),
            },
            ConstBinding {
                name: "F0N".to_string(),
                value: ConstValue::String("x".to_string()),
            },
            ConstBinding {
                name: "F0T".to_string(),
                value: ConstValue::String("Int".to_string()),
            },
        ]
    );
}

#[test]
fn const_eval_fields_of_supports_class_fields_v0() {
    let ty = ConstIntTy::host_word(true);

    let consts = eval_file_consts(
        r#"
class C(val x: Int) {
    val y: String
    // 计算属性：fieldsOf 不应把它当作字段。
    val computed: Int get() = 0
}

const val F = fieldsOf<C>()
"#,
    );

    assert_eq!(
        consts,
        vec![ConstBinding {
            name: "F".to_string(),
            value: ConstValue::Tuple(vec![
                mk_field_meta(ty, "x", "Int", 0),
                mk_field_meta(ty, "y", "String", 1),
            ]),
        }]
    );
}

#[test]
fn const_eval_reflection_annotations_of_v0_basic() {
    let consts = eval_file_consts(
        r#"
annotation class Deprecated(val msg: String)

@Deprecated("x")
@Deprecated(msg: "y")
struct Foo(val x: Int)

const val A = annotationsOf<Foo>()
const val A0N: String = annotationsOf<Foo>()._0.name
const val A0AN: String = annotationsOf<Foo>()._0.args._0.name
const val A0AV: String = annotationsOf<Foo>()._0.args._0.value
const val A1AV: String = annotationsOf<Foo>()._1.args._0.value
"#,
    );

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "A".to_string(),
                value: ConstValue::Tuple(vec![
                    mk_annotation_meta(
                        "Deprecated",
                        vec![mk_annotation_arg_meta(
                            "msg",
                            ConstValue::String("x".to_string()),
                        )],
                    ),
                    mk_annotation_meta(
                        "Deprecated",
                        vec![mk_annotation_arg_meta(
                            "msg",
                            ConstValue::String("y".to_string()),
                        )],
                    ),
                ]),
            },
            ConstBinding {
                name: "A0N".to_string(),
                value: ConstValue::String("Deprecated".to_string()),
            },
            ConstBinding {
                name: "A0AN".to_string(),
                value: ConstValue::String("msg".to_string()),
            },
            ConstBinding {
                name: "A0AV".to_string(),
                value: ConstValue::String("x".to_string()),
            },
            ConstBinding {
                name: "A1AV".to_string(),
                value: ConstValue::String("y".to_string()),
            },
        ]
    );
}

#[test]
fn const_eval_reflection_annotations_of_v0_complex_args() {
    let ty = ConstIntTy::host_word(true);

    let consts = eval_file_consts(
        r#"
annotation class Anno(val a: Int, val colors: Array<Color>, val cls: String)

enum Color { Red, Blue }

@Anno(1 + 2, [Color.Red], String::class)
struct Foo(val x: Int)

const val A = annotationsOf<Foo>()
const val V0: Int = annotationsOf<Foo>()._0.args._0.value
const val V1 = annotationsOf<Foo>()._0.args._1.value
const val V2: String = annotationsOf<Foo>()._0.args._2.value
"#,
    );

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "A".to_string(),
                value: ConstValue::Tuple(vec![mk_annotation_meta(
                    "Anno",
                    vec![
                        mk_annotation_arg_meta("a", mk_int(ty, 3)),
                        mk_annotation_arg_meta(
                            "colors",
                            ConstValue::Tuple(vec![ConstValue::Enum(ConstEnum {
                                ty: Some("Color".to_string()),
                                variant: "Red".to_string(),
                                payload: Vec::new(),
                            })]),
                        ),
                        mk_annotation_arg_meta("cls", ConstValue::String("String".to_string())),
                    ],
                )]),
            },
            ConstBinding {
                name: "V0".to_string(),
                value: mk_int(ty, 3),
            },
            ConstBinding {
                name: "V1".to_string(),
                value: ConstValue::Tuple(vec![ConstValue::Enum(ConstEnum {
                    ty: Some("Color".to_string()),
                    variant: "Red".to_string(),
                    payload: Vec::new(),
                })]),
            },
            ConstBinding {
                name: "V2".to_string(),
                value: ConstValue::String("String".to_string()),
            },
        ]
    );
}

#[test]
fn const_eval_reflection_intrinsics_v0_more() {
    let ty = ConstIntTy::host_word(true);

    let consts = eval_file_consts(
        r#"
interface I {}

class C() : I {}

enum Color { Red, Blue }

enum E { A(val x: Int, val y: String), B }

fun add(a: Int, b: String): Int { return 0 }

const val A: Int = alignOf<Int32>()
const val ST = superTypesOf<C>()
const val ST0: String = superTypesOf<C>()._0.name
const val VS = variantsOf<Color>()
const val V0: String = variantsOf<Color>()._0.name
const val EV = variantsOf<E>()
const val EV0F0: String = variantsOf<E>()._0.fields._0.name
const val P = paramsOf(FunctionMeta { name: "add" })
const val P0N: String = paramsOf(FunctionMeta { name: "add" })._0.name
const val P1T: String = paramsOf(FunctionMeta { name: "add" })._1.type.name
"#,
    );

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "A".to_string(),
                value: mk_int(ty, std::mem::align_of::<u32>() as u128),
            },
            ConstBinding {
                name: "ST".to_string(),
                value: ConstValue::Tuple(vec![mk_type_meta("I", "Interface")]),
            },
            ConstBinding {
                name: "ST0".to_string(),
                value: ConstValue::String("I".to_string()),
            },
            ConstBinding {
                name: "VS".to_string(),
                value: ConstValue::Tuple(vec![
                    mk_variant_meta(ty, "Red", Vec::new(), 0),
                    mk_variant_meta(ty, "Blue", Vec::new(), 1),
                ]),
            },
            ConstBinding {
                name: "V0".to_string(),
                value: ConstValue::String("Red".to_string()),
            },
            ConstBinding {
                name: "EV".to_string(),
                value: ConstValue::Tuple(vec![
                    mk_variant_meta(
                        ty,
                        "A",
                        vec![
                            mk_field_meta(ty, "x", "Int", 0),
                            mk_field_meta(ty, "y", "String", 1)
                        ],
                        0,
                    ),
                    mk_variant_meta(ty, "B", Vec::new(), 1),
                ]),
            },
            ConstBinding {
                name: "EV0F0".to_string(),
                value: ConstValue::String("x".to_string()),
            },
            ConstBinding {
                name: "P".to_string(),
                value: ConstValue::Tuple(vec![
                    mk_param_meta(ty, "a", "Int", 0),
                    mk_param_meta(ty, "b", "String", 1),
                ]),
            },
            ConstBinding {
                name: "P0N".to_string(),
                value: ConstValue::String("a".to_string()),
            },
            ConstBinding {
                name: "P1T".to_string(),
                value: ConstValue::String("String".to_string()),
            },
        ]
    );
}

#[test]
fn const_eval_get_platform_intrinsic_v0() {
    let consts = eval_file_consts(
        r#"
const val P = getPlatform()
const val T: String = getPlatform().triple
const val A: String = getPlatform().arch
const val V: String = getPlatform().vendor
const val O: String = getPlatform().os
const val E: String = getPlatform().env
"#,
    );

    // 注意：这里的期望值来自 Cargo 编译期 cfg（与解释器的实现保持一致）。
    let arch = option_env!("CARGO_CFG_TARGET_ARCH").unwrap_or("unknown");
    let vendor = option_env!("CARGO_CFG_TARGET_VENDOR").unwrap_or("unknown");
    let os_cfg = option_env!("CARGO_CFG_TARGET_OS").unwrap_or("unknown");
    let os = if os_cfg == "macos" { "darwin" } else { os_cfg };
    let env = option_env!("CARGO_CFG_TARGET_ENV").unwrap_or("");
    let triple = if env.is_empty() {
        format!("{arch}-{vendor}-{os}")
    } else {
        format!("{arch}-{vendor}-{os}-{env}")
    };

    assert_eq!(
        consts,
        vec![
            ConstBinding {
                name: "P".to_string(),
                value: mk_platform(&triple, arch, vendor, os, env),
            },
            ConstBinding {
                name: "T".to_string(),
                value: ConstValue::String(triple.clone()),
            },
            ConstBinding {
                name: "A".to_string(),
                value: ConstValue::String(arch.to_string()),
            },
            ConstBinding {
                name: "V".to_string(),
                value: ConstValue::String(vendor.to_string()),
            },
            ConstBinding {
                name: "O".to_string(),
                value: ConstValue::String(os.to_string()),
            },
            ConstBinding {
                name: "E".to_string(),
                value: ConstValue::String(env.to_string()),
            },
        ]
    );
}
