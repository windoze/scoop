//! Parser 测试：
//!
//! - 移植自 legacy `scoopc_ast::parser::tests`（34 个，按新 API/AST 调整）；
//! - 随机输入 fuzz（不 panic）；
//! - sysroot 全量 smoke（零错误诊断，完备性 gate）；
//! - grammar §14 changelog 逐条 targeted 测试；
//! - grammar §10 移除语法诊断码测试。

use scoop2_base::{Interner, SourceFile};

use super::{ParseResult, parse_file};
use crate::ast::decl::*;
use crate::ast::expr::*;
use crate::ast::pattern::*;
use crate::ast::types::*;
use crate::ast::{File, FloatSuffix, ItemKind, ModifierKind};
use crate::dump::dump_file;

// ------------------------------------------------------------------
// 辅助
// ------------------------------------------------------------------

fn parse(text: &str) -> ParseResult {
    let src = SourceFile::new_virtual("<mem>", text);
    parse_file(&src)
}

/// 解析并断言零诊断。
fn ok(text: &str) -> (File, Interner) {
    let result = parse(text);
    assert!(
        !result.diagnostics.has_errors(),
        "期望零错误诊断，实际为：{:?}\n源：{text}",
        result.diagnostics.into_vec()
    );
    (result.file, result.interner)
}

/// 解析并返回全部错误诊断码。
fn err_codes(text: &str) -> Vec<&'static str> {
    let result = parse(text);
    result
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| d.code)
        .collect()
}

fn expect_err(text: &str, code: &str) {
    let codes = err_codes(text);
    assert!(
        codes.contains(&code),
        "期望诊断码 {code}，实际为 {codes:?}\n源：{text}"
    );
}

fn dump(text: &str) -> String {
    let (file, interner) = ok(text);
    dump_file(&file, &interner)
}

fn first_fun_body_block(file: &File, idx: usize) -> &Block {
    let ItemKind::Fun(f) = &file.items[idx].kind else {
        panic!("期望第 {idx} 个 item 为 fun 声明");
    };
    let Some(FunBody::Block(b)) = &f.body else {
        panic!("期望函数体为 block");
    };
    b
}

// ------------------------------------------------------------------
// 移植：legacy parser tests（34）
// ------------------------------------------------------------------

#[test]
fn parse_minimal_file() {
    let (file, _i) = ok("package a.b\n\nfun main() { val x = 1 }");
    assert!(file.package.is_some());
    assert_eq!(file.imports.len(), 0);
    assert_eq!(file.items.len(), 1);
}

#[test]
fn fstring_parses_as_interpolated_string() {
    let (file, _i) = ok("package scoop.core\nval x = f\"hello\"\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("val 应有 initializer");
    assert!(matches!(init.kind, ExprKind::InterpolatedString { .. }));
}

#[test]
fn parse_import_alias_decl() {
    let (file, interner) = ok("package a\nimport foo.bar.Baz as Qux\nfun main() {}");
    assert_eq!(file.imports.len(), 1);
    let import = &file.imports[0];
    assert_eq!(import.path.segments.len(), 3);
    assert!(import.wildcard.is_none());
    let alias = import.alias.expect("alias import 应当记录 alias 名");
    assert_eq!(interner.resolve(alias.symbol), "Qux");
}

#[test]
fn parse_type_decls() {
    let (file, _i) =
        ok("open class A(x: Int) : B(x)\nstruct P(val x: Int)\nenum E { A, B }\nfun main() {}");
    assert_eq!(file.items.len(), 4);
}

#[test]
fn parse_annotation_class_decl() {
    let (file, interner) =
        ok("package a\nannotation class Deprecated(val message: String = \"\")\nclass C {}\n");
    assert_eq!(file.items.len(), 2);
    let ItemKind::Type(t) = &file.items[0].kind else {
        panic!("期望第一个 item 为类型声明");
    };
    assert_eq!(t.kind, TypeKind::Class);
    assert!(
        t.modifiers
            .iter()
            .any(|m| m.kind == ModifierKind::Annotation)
    );
    let _ = interner;
}

#[test]
fn parse_fun_decl_with_annotations() {
    let (file, interner) = ok("package a\n@Unsafe\n@Extern(\"c_name\")\nfun f() {}\n");
    let ItemKind::Fun(f) = &file.items[0].kind else {
        panic!("期望 fun 声明");
    };
    assert_eq!(interner.resolve(f.name.symbol), "f");
    assert_eq!(f.annotations.len(), 2);
    assert_eq!(
        interner.resolve(f.annotations[0].path.segments[0].symbol),
        "Unsafe"
    );
    assert!(f.annotations[0].args.is_empty());
    assert_eq!(
        interner.resolve(f.annotations[1].path.segments[0].symbol),
        "Extern"
    );
    assert_eq!(f.annotations[1].args.len(), 1);
    assert!(f.annotations[1].args[0].name.is_none());
    let ExprKind::StringLit(lit) = &f.annotations[1].args[0].value.kind else {
        panic!("期望字符串字面量实参");
    };
    assert_eq!(lit.value, "c_name");
}

#[test]
fn parse_target_annotation_with_enum_values() {
    let (file, interner) =
        ok("package a\n@Target(AnnotationTarget.Field)\nannotation class Column\n");
    let ItemKind::Type(t) = &file.items[0].kind else {
        panic!("期望类型声明");
    };
    assert!(
        t.modifiers
            .iter()
            .any(|m| m.kind == ModifierKind::Annotation)
    );
    assert_eq!(interner.resolve(t.name.symbol), "Column");
    assert_eq!(t.annotations.len(), 1);
    let ann = &t.annotations[0];
    assert_eq!(interner.resolve(ann.path.segments[0].symbol), "Target");
    assert_eq!(ann.args.len(), 1);
    assert!(matches!(
        ann.args[0].value.kind,
        ExprKind::MemberAccess { .. }
    ));
}

#[test]
fn parse_unsafe_block_expr() {
    let (file, _i) = ok("package a\nfun f() { @Unsafe do { 1 } }\n");
    let b = first_fun_body_block(&file, 0);
    let StmtKind::Expr(e) = &b.stmts[0].kind else {
        panic!("期望表达式语句");
    };
    let ExprKind::UnsafeBlock(body) = &e.kind else {
        panic!("期望 unsafe block");
    };
    assert_eq!(body.stmts.len(), 1);
}

#[test]
fn parse_safe_annotated_closure_expr() {
    let (file, _i) = ok("package a\nval f: () -> Int = @Safe { 1 }\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("val 应有 initializer");
    let ExprKind::Lambda(lam) = &init.kind else {
        panic!("期望 annotated lambda，实际为 {:?}", init.kind);
    };
    assert!(lam.is_safe, "@Safe closure 应记录 is_safe");
    assert!(lam.params.is_empty());
    let LambdaBody::Expr(body) = &lam.body else {
        panic!("单表达式 lambda body 应解包");
    };
    assert!(matches!(body.kind, ExprKind::IntLit(_)));
}

#[test]
fn parse_unsafe_block_requires_do() {
    expect_err(
        "package a\nfun f() { @Unsafe { 1 } }\n",
        "scoop::parse::unsafe_block_requires_do",
    );
}

#[test]
fn parse_char_literal_expr_and_when_pattern() {
    let (file, _i) =
        ok("package a\nval plain = 'A'\nval choice = when (c) { 'x' -> 1 else -> 2 }\n");
    assert_eq!(file.items.len(), 2);
    let ItemKind::Val(plain) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let plain_init = plain.init.as_ref().expect("plain 应有 initializer");
    let ExprKind::CharLit(lit) = &plain_init.kind else {
        panic!("期望 char 字面量");
    };
    assert_eq!(lit.value, 'A');

    let ItemKind::Val(choice) = &file.items[1].kind else {
        panic!("期望 val");
    };
    let choice_init = choice.init.as_ref().expect("choice 应有 initializer");
    let ExprKind::When { arms, .. } = &choice_init.kind else {
        panic!("期望 when 表达式");
    };
    assert!(matches!(
        arms[0].pat.kind,
        PatternKind::Literal(PatternLiteral::Char(_))
    ));
}

#[test]
fn parse_when_variant_payload_or_pattern() {
    let (file, interner) =
        ok("package a\nval choice = when (x) { Hit(0) | Miss() -> 1 else -> 2 }\n");
    let ItemKind::Val(choice) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = choice.init.as_ref().expect("应有 initializer");
    let ExprKind::When { arms, .. } = &init.kind else {
        panic!("期望 when");
    };
    let PatternKind::Or(pats) = &arms[0].pat.kind else {
        panic!("期望 or-pattern");
    };
    assert_eq!(pats.len(), 2);
    let PatternKind::Variant { path, args } = &pats[0].kind else {
        panic!("期望 variant pattern");
    };
    assert_eq!(path.segments.len(), 1);
    assert_eq!(interner.resolve(path.segments[0].symbol), "Hit");
    let args = args.as_ref().expect("Hit(0) 应有括号参数");
    assert_eq!(args.len(), 1);
    assert!(matches!(
        args[0].kind,
        PatternKind::Literal(PatternLiteral::Int(_))
    ));

    let PatternKind::Variant { path, args } = &pats[1].kind else {
        panic!("期望 variant pattern");
    };
    assert_eq!(interner.resolve(path.segments[0].symbol), "Miss");
    assert_eq!(args.as_ref().map(Vec::len), Some(0));
}

#[test]
fn parse_when_bare_variant_or_pattern_keeps_zero_arity_variants() {
    let (file, interner) = ok("package a\nval choice = when (x) { Hit | Miss -> 1 else -> 2 }\n");
    let ItemKind::Val(choice) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = choice.init.as_ref().expect("应有 initializer");
    let ExprKind::When { arms, .. } = &init.kind else {
        panic!("期望 when");
    };
    let PatternKind::Or(pats) = &arms[0].pat.kind else {
        panic!("期望 or-pattern");
    };
    assert_eq!(pats.len(), 2);
    for (pat, name) in pats.iter().zip(["Hit", "Miss"]) {
        let PatternKind::Variant { path, args } = &pat.kind else {
            panic!("期望 bare variant pattern");
        };
        assert_eq!(interner.resolve(path.segments[0].symbol), name);
        assert!(args.is_none(), "bare variant 不应有括号参数");
    }
}

#[test]
fn parse_when_qualified_variant_patterns() {
    let (file, interner) =
        ok("package a\nval choice = when (x) { State.Ready(1) | State.Pending -> 1 else -> 2 }\n");
    let ItemKind::Val(choice) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = choice.init.as_ref().expect("应有 initializer");
    let ExprKind::When { arms, .. } = &init.kind else {
        panic!("期望 when");
    };
    let PatternKind::Or(pats) = &arms[0].pat.kind else {
        panic!("期望 or-pattern");
    };
    let PatternKind::Variant { path, args } = &pats[0].kind else {
        panic!("期望 qualified variant pattern");
    };
    assert_eq!(path.segments.len(), 2);
    assert_eq!(interner.resolve(path.segments[0].symbol), "State");
    assert_eq!(interner.resolve(path.segments[1].symbol), "Ready");
    assert_eq!(args.as_ref().map(Vec::len), Some(1));

    let PatternKind::Variant { path, args } = &pats[1].kind else {
        panic!("期望 qualified bare variant pattern");
    };
    assert_eq!(interner.resolve(path.segments[1].symbol), "Pending");
    assert!(args.is_none());
}

#[test]
fn do_block_basic() {
    let (file, _i) = ok("fun f() { val x = do { 1 }; return x }");
    let b = first_fun_body_block(&file, 0);
    let StmtKind::LocalVal(val) = &b.stmts[0].kind else {
        panic!("期望局部 val");
    };
    let init = val.init.as_ref().expect("val 应有 initializer");
    let ExprKind::DoBlock(body) = &init.kind else {
        panic!("期望 DoBlock，实际为 {:?}", init.kind);
    };
    assert_eq!(body.stmts.len(), 1);
}

#[test]
fn bare_brace_is_lambda_not_do_block() {
    let (file, _i) = ok("val f = { 1 }");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    assert!(
        matches!(init.kind, ExprKind::Lambda(_)),
        "期望裸 {{}} 解析为 lambda，实际为 {:?}",
        init.kind
    );
}

#[test]
fn safe_do_block() {
    let (file, _i) = ok("fun f() { @Safe do { 1 } }");
    let b = first_fun_body_block(&file, 0);
    let StmtKind::Expr(e) = &b.stmts[0].kind else {
        panic!("期望表达式语句");
    };
    assert!(
        matches!(e.kind, ExprKind::SafeBlock(_)),
        "期望 SafeBlock，实际为 {:?}",
        e.kind
    );
}

#[test]
fn unsafe_do_block() {
    let (file, _i) = ok("fun f() { @Unsafe do { 1 } }");
    let b = first_fun_body_block(&file, 0);
    let StmtKind::Expr(e) = &b.stmts[0].kind else {
        panic!("期望表达式语句");
    };
    assert!(
        matches!(e.kind, ExprKind::UnsafeBlock(_)),
        "期望 UnsafeBlock，实际为 {:?}",
        e.kind
    );
}

#[test]
fn annotated_local_val_decl_basic() {
    let (file, interner) = ok("fun f() { @Suppress(\"deprecated\") val x = oldAdd() }");
    let b = first_fun_body_block(&file, 0);
    let StmtKind::LocalVal(val) = &b.stmts[0].kind else {
        panic!("期望局部 val");
    };
    assert_eq!(val.annotations.len(), 1);
    assert_eq!(
        interner.resolve(val.annotations[0].path.segments[0].symbol),
        "Suppress"
    );
}

#[test]
fn parse_top_level_val_var() {
    let (file, _i) = ok("package a\nval x: Int = 1\nvar y = x\nfun main() {}");
    assert_eq!(file.items.len(), 3);
}

#[test]
fn parse_top_level_typealias() {
    let (file, interner) = ok("package a\ntypealias Byte = UInt8\n");
    assert_eq!(file.items.len(), 1);
    let ItemKind::TypeAlias(ta) = &file.items[0].kind else {
        panic!("期望 typealias");
    };
    assert_eq!(interner.resolve(ta.name.symbol), "Byte");
    assert!(ta.modifiers.is_empty());
    assert!(ta.type_params.is_none());
    let TypeRefKind::Path { path, .. } = &ta.ty.kind else {
        panic!("期望路径类型");
    };
    assert_eq!(interner.resolve(path.segments[0].symbol), "UInt8");
}

#[test]
fn parse_top_level_generic_typealias() {
    let (file, interner) = ok("package a\ntypealias Handler<T> = (T) -> Unit\n");
    let ItemKind::TypeAlias(ta) = &file.items[0].kind else {
        panic!("期望 typealias");
    };
    assert_eq!(interner.resolve(ta.name.symbol), "Handler");
    let type_params = ta.type_params.as_ref().expect("应有类型参数");
    assert_eq!(type_params.params.len(), 1);
    assert_eq!(interner.resolve(type_params.params[0].name.symbol), "T");
    let TypeRefKind::Function { params, .. } = &ta.ty.kind else {
        panic!("期望函数类型");
    };
    assert_eq!(params.len(), 1);
}

#[test]
fn parse_call_named_args() {
    let (file, interner) = ok("package a\nval v = f(x = 1, y = 2)\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { args, .. } = &init.kind else {
        panic!("期望调用表达式");
    };
    assert_eq!(args.len(), 2);
    for (arg, name) in args.iter().zip(["x", "y"]) {
        let arg_name = arg.name.expect("期望命名实参");
        assert_eq!(interner.resolve(arg_name.symbol), name);
        assert!(!arg.is_spread);
        assert!(matches!(arg.value.kind, ExprKind::IntLit(_)));
    }
}

#[test]
fn parser_hir_surface_gate() {
    expect_err(
        "fun f(box: Box) { val y = (box.value = 1) }",
        "scoop::parse::assignment_expression_not_allowed",
    );
    expect_err(
        "fun f(xs: Array<Int>) { val y = [*xs] }",
        "scoop::parse::spread_arg_outside_call",
    );
    expect_err(
        "fun f() { val y = [x = 1] }",
        "scoop::parse::named_arg_outside_call",
    );
    let (_file, _i) = ok("fun f(xs: Array<Int>) { var x = 0; x = 1; val y = call(a = 1, *xs) }");
}

#[test]
fn parse_resume_member_call_as_plain_call_shape() {
    let (file, interner) = ok("package a\nval resumed = k.resume(x)\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { callee, args } = &init.kind else {
        panic!("期望调用表达式");
    };
    assert_eq!(args.len(), 1, "`k.resume(x)` 不应改写 arity");
    assert!(matches!(args[0].value.kind, ExprKind::Ident(_)));
    let ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        panic!("期望 member access callee");
    };
    assert!(matches!(receiver.kind, ExprKind::Ident(_)));
    let MemberName::Named(member) = member else {
        panic!("期望命名成员");
    };
    assert_eq!(interner.resolve(member.symbol), "resume");
}

#[test]
fn parse_zero_arg_resume_member_call_without_unit_desugar() {
    let (file, _i) = ok("package a\nval resumed = k.resume()\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { args, .. } = &init.kind else {
        panic!("期望调用表达式");
    };
    assert!(args.is_empty(), "`k.resume()` 不应自动补 `UnitLit`");
}

#[test]
fn parse_zero_arg_and_explicit_unit_calls_as_distinct_shapes() {
    let (file, _i) = ok("package a\nval zero = f()\nval explicit = f(())\n");
    let ItemKind::Val(zero) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let zero_init = zero.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { args, .. } = &zero_init.kind else {
        panic!("期望调用表达式");
    };
    assert!(args.is_empty(), "`f()` 必须保持零参数形状");

    let ItemKind::Val(explicit) = &file.items[1].kind else {
        panic!("期望 val");
    };
    let explicit_init = explicit.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { args, .. } = &explicit_init.kind else {
        panic!("期望调用表达式");
    };
    assert_eq!(args.len(), 1, "`f(())` 必须保留显式单参数形状");
    assert!(matches!(args[0].value.kind, ExprKind::UnitLit));
}

#[test]
fn parse_prefixed_int_literals_as_single_tokens() {
    let (file, _i) = ok("package a\nval hex = 0xFF\nval bin = 0b1010\n");
    let ItemKind::Val(hex) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let hex_init = hex.init.as_ref().expect("应有 initializer");
    let ExprKind::IntLit(lit) = &hex_init.kind else {
        panic!("期望 int 字面量");
    };
    assert_eq!(lit.value, 0xFF);

    let ItemKind::Val(bin) = &file.items[1].kind else {
        panic!("期望 val");
    };
    let bin_init = bin.init.as_ref().expect("应有 initializer");
    let ExprKind::IntLit(lit) = &bin_init.kind else {
        panic!("期望 int 字面量");
    };
    assert_eq!(lit.value, 0b1010);
}

#[test]
fn parse_float_literals_and_int_member_call() {
    let (file, interner) = ok(
        "package a\nval plain = 2.75\nval sci = 1.5e3\nval f32v = 0.5f\nval call = 1.toString()\n",
    );
    let ItemKind::Val(plain) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let plain_init = plain.init.as_ref().expect("应有 initializer");
    let ExprKind::FloatLit(lit) = &plain_init.kind else {
        panic!("期望 float 字面量");
    };
    assert_eq!(lit.value, 2.75);
    assert!(lit.suffix.is_none());

    let ItemKind::Val(sci) = &file.items[1].kind else {
        panic!("期望 val");
    };
    let sci_init = sci.init.as_ref().expect("应有 initializer");
    let ExprKind::FloatLit(lit) = &sci_init.kind else {
        panic!("期望 float 字面量");
    };
    assert_eq!(lit.value, 1500.0);

    let ItemKind::Val(f32v) = &file.items[2].kind else {
        panic!("期望 val");
    };
    let f32_init = f32v.init.as_ref().expect("应有 initializer");
    let ExprKind::FloatLit(lit) = &f32_init.kind else {
        panic!("期望 float 字面量");
    };
    assert_eq!(lit.value, 0.5);
    assert_eq!(lit.suffix, Some(FloatSuffix::F32));

    let ItemKind::Val(call) = &file.items[3].kind else {
        panic!("期望 val");
    };
    let call_init = call.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { callee, .. } = &call_init.kind else {
        panic!("期望 `1.toString()` 为调用表达式");
    };
    let ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        panic!("期望 member access callee");
    };
    assert!(matches!(receiver.kind, ExprKind::IntLit(_)));
    let MemberName::Named(member) = member else {
        panic!("期望命名成员");
    };
    assert_eq!(interner.resolve(member.symbol), "toString");
}

#[test]
fn when_float_pattern_reports_single_parse_error() {
    let codes = err_codes(
        "fun main() {\n    val x: Float64 = 1.5\n    val y = when (x) {\n        1.5 -> 1\n        else -> 2\n    }\n    println(y)\n}\n",
    );
    assert_eq!(
        codes.len(),
        1,
        "Float when pattern 应恰好产生一个错误，实际为 {codes:?}"
    );
}

#[test]
fn parse_top_level_val_destructuring() {
    // 新 grammar（§3.3）：解构绑定不允许 `:` 类型标注。
    let (file, _i) = ok(
        "package a\nstruct Point(val x: Int, val y: Int)\nenum MaybeInt {\n    Some(val value: Int),\n    None,\n}\nval (a, b) = (1, 2)\nval Point { x, y } = Point { x: 3, y: 4 }\nval Some(total) = Some(5)\n",
    );
    for idx in [2, 3, 4] {
        let ItemKind::Val(decl) = &file.items[idx].kind else {
            panic!("期望第 {idx} 个 item 为解构 val");
        };
        assert!(
            matches!(decl.binding, ValBinding::Pattern(_)),
            "第 {idx} 个 item 应为模式绑定"
        );
        assert!(decl.ty.is_none(), "解构绑定不允许类型标注");
        assert!(decl.init.is_some());
    }
}

#[test]
fn destructuring_with_type_annotation_is_rejected() {
    // §3.3：模式绑定后直接走 `=`，`:` 类型标注不再解析。
    let codes = err_codes("val (a, b): (Int, Int) = (1, 2)\n");
    assert!(!codes.is_empty(), "解构 + `:` 类型标注应报错");
}

#[test]
fn top_level_var_destructuring_is_rejected() {
    let codes = err_codes("package a\nvar (a, b) = (1, 2)\n");
    assert!(!codes.is_empty(), "顶层 `var` 解构应报错");
}

#[test]
fn parse_comptime_as_plain_identifier_after_surface_removal() {
    let (_file, _i) = ok("package a\n\nfun f() {\n    val comptime = 1\n    val y = comptime\n}\n");
}

// ------------------------------------------------------------------
// 移植：随机输入 fuzz（不 panic）
// ------------------------------------------------------------------

/// 极简可复现 PRNG（避免引入 `rand` 依赖）。
#[derive(Clone)]
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        let seed = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn gen_usize(&mut self, upper_exclusive: usize) -> usize {
        if upper_exclusive == 0 {
            return 0;
        }
        (self.next_u64() as usize) % upper_exclusive
    }
}

fn gen_source(rng: &mut XorShift64, max_len: usize) -> String {
    const CHARS: &[char] = &[
        ' ', '\t', '\n', '\r', '_', '@', '.', ',', ':', ';', '(', ')', '{', '}', '[', ']', '+',
        '-', '*', '/', '%', '=', '<', '>', '!', '?', '&', '|', '"', '\'', '\\', 'a', 'b', 'c', 'x',
        'y', 'z', 'A', 'B', 'C', '0', '1', '2', '3', '9', '中', 'é',
    ];
    let len = rng.gen_usize(max_len + 1);
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let ch = CHARS[rng.gen_usize(CHARS.len())];
        s.push(ch);
    }
    s
}

#[test]
fn parser_random_inputs_do_not_panic() {
    const CORPUS: &[&str] = &[
        "",
        " ",
        "\n\n\n",
        "/*",
        "*/",
        "//",
        "\"",
        "\"\\",
        "\"\n",
        "\"\"\"",
        "package",
        "package a.b\nimport x.y.*\nfun f() {}",
        "fun f( {",
        "open class A(x: Int) : B(x)\nfun main() {",
        "f\"${",
        "handle { } on { Raise.raise(",
        "val x: A<B<C",
    ];

    for (idx, &text) in CORPUS.iter().enumerate() {
        let text_owned = text.to_string();
        let res = std::panic::catch_unwind(move || {
            let src = SourceFile::new_virtual("<corpus>", &text_owned);
            let _ = parse_file(&src);
        });
        assert!(res.is_ok(), "parser panic（corpus#{idx}）: {text:?}");
    }

    let mut rng = XorShift64::new(0xD15E_A5E0_1234_5678);
    for i in 0..1_000usize {
        let text = gen_source(&mut rng, 512);
        let text_clone = text.clone();
        let res = std::panic::catch_unwind(move || {
            let src = SourceFile::new_virtual("<fuzz>", &text_clone);
            let _ = parse_file(&src);
        });
        assert!(res.is_ok(), "parser panic（iter={i}）: {text:?}");
    }
}

// ------------------------------------------------------------------
// sysroot smoke：完备性 gate（sysroot 使用的所有构造都必须解析成功）
// ------------------------------------------------------------------

#[test]
fn sysroot_sources_parse_without_errors() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/scoop2_syntax 应在 workspace 根下两级")
        .to_path_buf();
    let sysroot_lib = root.join("sysroot/lib");

    let mut files = Vec::new();
    collect_scoop_files(&sysroot_lib, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "未找到 sysroot 源文件：{}",
        sysroot_lib.display()
    );

    let mut failures = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("无法读取 {}: {err}", path.display()));
        let src = SourceFile::new_virtual(path.clone(), text);
        let result = parse_file(&src);
        if result.diagnostics.has_errors() {
            let mut sink = result.diagnostics;
            sink.sort_by_offset();
            for d in sink.iter() {
                failures.push(format!("{}: {}: {}", path.display(), d.code, d.message));
            }
        }
        // NodeId 空间应与 AST 节点数一致（>0）。
        assert!(
            result.node_count > 0,
            "{}: node_count 应大于 0",
            path.display()
        );
    }

    assert!(
        failures.is_empty(),
        "sysroot 解析失败（{} 个诊断）：\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn collect_scoop_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_scoop_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "scoop") {
            out.push(path);
        }
    }
}

// ------------------------------------------------------------------
// grammar §14 changelog targeted tests
// ------------------------------------------------------------------

#[test]
fn changelog_expression_body_fun() {
    // §14.1：`fun f(): T = expr`（顶层与成员）。
    let (file, _i) =
        ok("fun add(a: Int, b: Int): Int = a + b\nclass C {\n    fun one(): Int = 1\n}\n");
    let ItemKind::Fun(f) = &file.items[0].kind else {
        panic!("期望 fun");
    };
    let Some(FunBody::Expr(body)) = &f.body else {
        panic!("期望表达式体");
    };
    assert!(matches!(body.kind, ExprKind::Binary { .. }));

    let ItemKind::Type(t) = &file.items[1].kind else {
        panic!("期望 class");
    };
    let body = t.body.as_ref().expect("class 应有 body");
    let TypeMemberKind::Fun(m) = &body.members[0].kind else {
        panic!("期望成员 fun");
    };
    assert!(matches!(m.body, Some(FunBody::Expr(_))));
}

#[test]
fn changelog_expression_body_fun_trailing_junk_is_hard_error() {
    // §3.2/§12.6：表达式体后的多余 token 是硬错误（不是静默降级）。
    expect_err("fun f(): Int = 1 2\n", "scoop::parse::trailing_tokens");
}

#[test]
fn changelog_index_read_and_assign() {
    // §14.2：`a[i]` / `a[i, j] = v`。
    let (file, _i) =
        ok("fun f() {\n    val x = a[i]\n    val y = a[i, j]\n    a[i] = v\n    a[i, j] = w\n}\n");
    let b = first_fun_body_block(&file, 0);

    let StmtKind::LocalVal(x) = &b.stmts[0].kind else {
        panic!("期望 val");
    };
    let init = x.init.as_ref().expect("应有 initializer");
    let ExprKind::Index { indices, .. } = &init.kind else {
        panic!("期望 Index，实际为 {:?}", init.kind);
    };
    assert_eq!(indices.len(), 1);

    let StmtKind::LocalVal(y) = &b.stmts[1].kind else {
        panic!("期望 val");
    };
    let init = y.init.as_ref().expect("应有 initializer");
    let ExprKind::Index { indices, .. } = &init.kind else {
        panic!("期望多下标 Index");
    };
    assert_eq!(indices.len(), 2);

    let StmtKind::Assign { target, .. } = &b.stmts[2].kind else {
        panic!("期望赋值语句");
    };
    let AssignTargetKind::Index { indices, .. } = &target.kind else {
        panic!("期望 IndexAssign 目标");
    };
    assert_eq!(indices.len(), 1);

    let StmtKind::Assign { target, .. } = &b.stmts[3].kind else {
        panic!("期望赋值语句");
    };
    let AssignTargetKind::Index { indices, .. } = &target.kind else {
        panic!("期望多下标 IndexAssign 目标");
    };
    assert_eq!(indices.len(), 2);
}

#[test]
fn changelog_contextual_infix_until_downto_step() {
    // §14.3：`until` / `downTo` / `step` 上下文中缀（与 `..` 同级、左结合）。
    let (file, interner) =
        ok("fun f() {\n    val a = 1 until 10\n    val b = 5 downTo 1 step 2\n}\n");
    let b = first_fun_body_block(&file, 0);

    let StmtKind::LocalVal(a) = &b.stmts[0].kind else {
        panic!("期望 val");
    };
    let init = a.init.as_ref().expect("应有 initializer");
    let ExprKind::InfixCall { name, .. } = &init.kind else {
        panic!("期望 InfixCall，实际为 {:?}", init.kind);
    };
    assert_eq!(interner.resolve(name.symbol), "until");

    // `5 downTo 1 step 2` 左结合：`(5 downTo 1) step 2`。
    let StmtKind::LocalVal(vb) = &b.stmts[1].kind else {
        panic!("期望 val");
    };
    let init = vb.init.as_ref().expect("应有 initializer");
    let ExprKind::InfixCall {
        receiver,
        name,
        arg,
    } = &init.kind
    else {
        panic!("期望 InfixCall，实际为 {:?}", init.kind);
    };
    assert_eq!(interner.resolve(name.symbol), "step");
    assert!(matches!(arg.kind, ExprKind::IntLit(_)));
    let ExprKind::InfixCall { name, .. } = &receiver.kind else {
        panic!("期望左结合的内层 InfixCall");
    };
    assert_eq!(interner.resolve(name.symbol), "downTo");
}

#[test]
fn changelog_nested_nullable_types() {
    // §14.4：`T??` 每层 `?` 包一层 Option，不拍平。
    let (file, _i) = ok("val x: Int?? = v\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let ty = v.ty.as_ref().expect("应有类型标注");
    let TypeRefKind::Nullable(outer) = &ty.kind else {
        panic!("期望外层 Nullable，实际为 {:?}", ty.kind);
    };
    let TypeRefKind::Nullable(inner) = &outer.kind else {
        panic!("期望内层 Nullable，实际为 {:?}", outer.kind);
    };
    assert!(matches!(inner.kind, TypeRefKind::Path { .. }));
}

#[test]
fn changelog_property_type_optional_with_initializer() {
    // §14.6：有 `= init` 时 `: T` 可省略；两者皆无 → targeted 错误。
    let (file, _i) = ok("class C {\n    val x = 1\n    val y: Int = 2\n    val z: Int\n}\n");
    let ItemKind::Type(t) = &file.items[0].kind else {
        panic!("期望 class");
    };
    let body = t.body.as_ref().expect("应有 body");
    assert_eq!(body.members.len(), 3);
    let TypeMemberKind::Property(p) = &body.members[0].kind else {
        panic!("期望属性");
    };
    assert!(p.ty.is_none(), "有 init 时类型标注可省略");
    assert!(p.init.is_some());

    expect_err("class C {\n    val x\n}\n", "scoop::parse::expected");
}

#[test]
fn changelog_gteq_split_after_nested_generics() {
    // §14.8：`A<B<C>> >= x` 解析为 `(A<B<C>>) >= x`。
    let (file, _i) = ok("val y = A<B<C>> >= x\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::Binary { lhs, op, .. } = &init.kind else {
        panic!("期望 `>=` 比较，实际为 {:?}", init.kind);
    };
    assert_eq!(*op, BinaryOp::Ge);
    let ExprKind::TypeApply { callee, args } = &lhs.kind else {
        panic!("期望左侧为类型应用，实际为 {:?}", lhs.kind);
    };
    assert_eq!(args.len(), 1);
    assert!(matches!(callee.kind, ExprKind::Ident(_)));
    let TypeArgKind::Type(inner) = &args[0].kind else {
        panic!("期望类型实参");
    };
    let TypeRefKind::Path {
        args: inner_args, ..
    } = &inner.kind
    else {
        panic!("期望路径类型");
    };
    assert_eq!(inner_args.len(), 1);
}

#[test]
fn changelog_gtgt_split_in_nested_generic_types() {
    // §5.2：类型位置的 `>>` 拆分。
    let (file, _i) = ok("val x: Continuation<Continuation<Int, Unit>> = v\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let ty = v.ty.as_ref().expect("应有类型标注");
    let TypeRefKind::Path { args, .. } = &ty.kind else {
        panic!("期望路径类型");
    };
    assert_eq!(args.len(), 1);
}

#[test]
fn changelog_anonymous_object_is_dedicated_error() {
    // §14.5：表达式位置的 `object` 是专用错误。
    expect_err(
        "val o = object : Foo { }\n",
        "scoop::parse::anonymous_object_unsupported",
    );
}

#[test]
fn changelog_top_level_initializer_trailing_junk_is_hard_error() {
    // §14.7：顶层 initializer 后的多余 token 是硬错误（legacy 静默降级已被否决）。
    expect_err("val x = 1 ???\n", "scoop::parse::trailing_tokens");
    expect_err("val x = f() g()\n", "scoop::parse::trailing_tokens");
}

#[test]
fn changelog_range_precedence_with_arithmetic() {
    // §14.9：`a + b .. c` 解析为 `(a + b) .. c`（`..` 在比较级）。
    let (file, _i) = ok("val r = a + b .. c\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::Binary { lhs, op, .. } = &init.kind else {
        panic!("期望 Range 二元表达式");
    };
    assert_eq!(*op, BinaryOp::Range);
    let ExprKind::Binary { op, .. } = &lhs.kind else {
        panic!("期望左侧为 `a + b`");
    };
    assert_eq!(*op, BinaryOp::Add);
}

// ------------------------------------------------------------------
// grammar §10 移除语法诊断码
// ------------------------------------------------------------------

#[test]
fn removed_perform_keyword() {
    expect_err(
        "fun f() { val x = perform Raise.raise(1) }\n",
        "scoop::parse::perform_keyword_removed",
    );
}

#[test]
fn removed_inline_modifier() {
    expect_err(
        "inline fun f() {}\n",
        "scoop::parse::inline_modifier_removed",
    );
    // 错误记录后解析继续：fun 仍应被保留。
    let result = parse("inline fun f() {}\n");
    assert_eq!(result.file.items.len(), 1);
}

#[test]
fn removed_handler_with_keyword() {
    expect_err(
        "fun f() { val x = handle { g() } with { Raise.raise(e) -> h() } }\n",
        "scoop::parse::handler_with_keyword_removed",
    );
}

#[test]
fn removed_handle_immediate_resume() {
    expect_err(
        "fun f() { val x = handle { g() } on { Raise.raise(e) -> resume { h() } } }\n",
        "scoop::parse::handle_immediate_resume_removed",
    );
}

#[test]
fn removed_bound_keyword_in_type_position() {
    expect_err(
        "val x: ref = 1\n",
        "scoop::parse::bound_keyword_type_position",
    );
    expect_err(
        "fun f(v: value) {}\n",
        "scoop::parse::bound_keyword_type_position",
    );
}

#[test]
fn removed_assignment_expression() {
    expect_err(
        "fun f() { val y = (x = 1) }\n",
        "scoop::parse::assignment_expression_not_allowed",
    );
}

#[test]
fn removed_spread_arg_outside_call() {
    expect_err(
        "fun f() { val y = *xs }\n",
        "scoop::parse::spread_arg_outside_call",
    );
}

#[test]
fn removed_named_arg_outside_call() {
    expect_err(
        "fun f() { val y = [x = 1] }\n",
        "scoop::parse::named_arg_outside_call",
    );
}

#[test]
fn removed_unsafe_block_without_do() {
    expect_err(
        "fun f() { @Unsafe { g() } }\n",
        "scoop::parse::unsafe_block_requires_do",
    );
}

#[test]
fn removed_class_literal_receiver_invalid() {
    expect_err(
        "val k = f()::class\n",
        "scoop::parse::class_literal_receiver_invalid",
    );
    expect_err(
        "val k = (1 + 2)::class\n",
        "scoop::parse::class_literal_receiver_invalid",
    );
}

#[test]
fn removed_anonymous_object_expression() {
    expect_err(
        "fun f() { val o = object { } }\n",
        "scoop::parse::anonymous_object_unsupported",
    );
}

// ------------------------------------------------------------------
// 结构性覆盖测试（更多 grammar 构造）
// ------------------------------------------------------------------

#[test]
fn parse_handle_expr_with_escape_continuation() {
    let (file, interner) = ok(
        "fun f() {\n    val x = handle { g() } on {\n        Raise<IOError>.raise(e), k -> k.resume(1)\n        Query.ask<Int>() -> 42\n    } finally { cleanup() }\n}\n",
    );
    let b = first_fun_body_block(&file, 0);
    let StmtKind::LocalVal(v) = &b.stmts[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::Handle { arms, finally, .. } = &init.kind else {
        panic!("期望 handle 表达式");
    };
    assert_eq!(arms.len(), 2);

    // `Raise<IOError>.raise(e), k ->`：effect args + escape continuation。
    let arm0 = &arms[0];
    assert_eq!(arm0.op.effect_args.len(), 1);
    let k = arm0.escape_continuation.expect("应有 escape continuation");
    assert_eq!(interner.resolve(k.symbol), "k");
    assert_eq!(arm0.op.binders.len(), 1);

    // `Query.ask<Int>()`：op 自己的类型实参。
    let arm1 = &arms[1];
    assert_eq!(arm1.op.op_type_args.len(), 1);
    assert_eq!(interner.resolve(arm1.op.op.symbol), "ask");
    assert!(arm1.escape_continuation.is_none());

    assert!(finally.is_some());
}

#[test]
fn parse_try_catch_desugars_to_handle() {
    let (file, interner) =
        ok("fun f() {\n    try { g() } catch (e: IOError) { h() } finally { cleanup() }\n}\n");
    let b = first_fun_body_block(&file, 0);
    let StmtKind::Expr(e) = &b.stmts[0].kind else {
        panic!("期望表达式语句");
    };
    let ExprKind::Handle { arms, finally, .. } = &e.kind else {
        panic!("try/catch 应脱糖为 handle，实际为 {:?}", e.kind);
    };
    assert_eq!(arms.len(), 1);
    let op = &arms[0].op;
    let segments: Vec<&str> = op
        .effect_path
        .segments
        .iter()
        .map(|s| interner.resolve(s.symbol))
        .collect();
    assert_eq!(segments, ["scoop", "core", "Raise"]);
    assert_eq!(interner.resolve(op.op.symbol), "raise");
    assert_eq!(op.binders.len(), 1);
    assert_eq!(interner.resolve(op.binders[0].name.symbol), "e");
    assert!(op.binders[0].ty.is_some());
    assert!(finally.is_some());
}

#[test]
fn parse_extension_fun_and_property() {
    let (file, interner) = ok(
        "fun <T> List<T>.firstOrNull(): T? = this.getOrNull(0)\nval String.size: Int\n    get() = this.length()\n",
    );
    let ItemKind::Fun(f) = &file.items[0].kind else {
        panic!("期望 fun");
    };
    assert!(f.receiver.is_some(), "扩展函数应有 receiver");
    assert_eq!(interner.resolve(f.name.symbol), "firstOrNull");
    assert!(f.type_params.is_some());
    assert!(matches!(f.body, Some(FunBody::Expr(_))));

    let ItemKind::ExtensionProperty(p) = &file.items[1].kind else {
        panic!("期望扩展属性，实际为 {:?}", file.items[1].kind);
    };
    assert_eq!(interner.resolve(p.name.symbol), "size");
    assert_eq!(p.accessors.len(), 1);
    assert_eq!(p.accessors[0].kind, AccessorKind::Get);
}

#[test]
fn parse_secondary_ctor_init_companion() {
    let (file, _i) = ok(
        "class C(val x: Int) {\n    constructor() : this(0) {\n        println(0)\n    }\n    init {\n        println(x)\n    }\n    companion object {\n        fun make(): C = C(1)\n    }\n}\n",
    );
    let ItemKind::Type(t) = &file.items[0].kind else {
        panic!("期望 class");
    };
    let body = t.body.as_ref().expect("应有 body");
    assert_eq!(body.members.len(), 3);
    let TypeMemberKind::SecondaryCtor(ctor) = &body.members[0].kind else {
        panic!("期望次构造");
    };
    let delegation = ctor.delegation.as_ref().expect("应有委托调用");
    assert_eq!(delegation.kind, CtorDelegationKind::This);
    assert!(matches!(body.members[1].kind, TypeMemberKind::InitBlock(_)));
    let TypeMemberKind::Object(obj) = &body.members[2].kind else {
        panic!("期望 companion object");
    };
    assert!(obj.companion);
}

#[test]
fn parse_enum_with_fields_and_discriminant() {
    let (file, interner) =
        ok("enum E : Int {\n    A(val x: Int) = 1,\n    B,\n    C(val y: Int, val z: Int),\n}\n");
    let ItemKind::Type(t) = &file.items[0].kind else {
        panic!("期望 enum");
    };
    assert_eq!(t.kind, TypeKind::Enum);
    assert_eq!(t.supertypes.len(), 1, "enum 底层类型走超类型列表");
    let body = t.body.as_ref().expect("应有 body");
    assert_eq!(body.members.len(), 3);
    let TypeMemberKind::EnumVariant(a) = &body.members[0].kind else {
        panic!("期望 variant");
    };
    assert_eq!(interner.resolve(a.name.symbol), "A");
    assert_eq!(a.fields.len(), 1);
    assert!(a.discriminant.is_some());
    let TypeMemberKind::EnumVariant(b) = &body.members[1].kind else {
        panic!("期望 variant");
    };
    assert!(b.fields.is_empty());
    assert!(b.discriminant.is_none());
}

#[test]
fn parse_effect_decl_and_op() {
    let (file, interner) =
        ok("public effect Raise<in E> {\n    public fun raise(error: E): Nothing\n}\n");
    let ItemKind::Type(t) = &file.items[0].kind else {
        panic!("期望 effect");
    };
    assert_eq!(t.kind, TypeKind::Effect);
    let body = t.body.as_ref().expect("应有 body");
    let TypeMemberKind::Fun(op) = &body.members[0].kind else {
        panic!("期望 effect operation");
    };
    assert_eq!(interner.resolve(op.name.symbol), "raise");
    assert!(op.body.is_none(), "effect operation 无函数体");
}

#[test]
fn parse_delegated_property_and_accessors() {
    let (file, _i) = ok(
        "class C {\n    val x: Int by lazy({ 1 })\n    var y: Int = 0\n        get() = field\n        set(v) { field = v }\n}\n",
    );
    let ItemKind::Type(t) = &file.items[0].kind else {
        panic!("期望 class");
    };
    let body = t.body.as_ref().expect("应有 body");
    let TypeMemberKind::Property(p) = &body.members[0].kind else {
        panic!("期望委托属性");
    };
    assert!(p.delegate.is_some());
    assert!(p.init.is_none());
    assert!(p.accessors.is_empty());

    let TypeMemberKind::Property(p) = &body.members[1].kind else {
        panic!("期望属性");
    };
    assert_eq!(p.accessors.len(), 2);
    assert_eq!(p.accessors[0].kind, AccessorKind::Get);
    assert_eq!(p.accessors[1].kind, AccessorKind::Set);
}

#[test]
fn parse_where_clause_and_eff_row_param() {
    let (file, _i) = ok(
        "fun <T, eff E = Pure> run(block: () -> Unit / E): Unit / E where T: ToString {\n    block()\n}\n",
    );
    let ItemKind::Fun(f) = &file.items[0].kind else {
        panic!("期望 fun");
    };
    let tp = f.type_params.as_ref().expect("应有类型参数");
    assert_eq!(tp.params.len(), 1);
    let eff = tp.effect_row.as_ref().expect("应有 eff 参数");
    assert!(eff.default.is_some(), "`<eff E = Pure>` 应记录默认行");
    assert!(f.effect.is_some(), "`/ E` effect 注解");
    let wc = f.where_clause.as_ref().expect("应有 where 子句");
    assert_eq!(wc.constraints.len(), 1);
}

#[test]
fn parse_when_is_arm_boundary_without_newlines() {
    // §8.5：非块 arm body 中 `is TypeRef ->` 是下一个 arm 的起始。
    let (file, _i) = ok("val y = when (x) { 0 -> f() is Int -> 1 else -> 2 }\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::When { arms, .. } = &init.kind else {
        panic!("期望 when");
    };
    assert_eq!(arms.len(), 3, "arms={:?}", arms.len());
    assert!(matches!(arms[1].pat.kind, PatternKind::Is(_)));
}

#[test]
fn parse_type_apply_follower_rules() {
    // §8.4：`f<T>(x)` 是类型应用调用；`a < b` 是比较。
    let (file, _i) = ok("val a = f<Int>(x)\nval b = a < b\n");
    let ItemKind::Val(a) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = a.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { callee, .. } = &init.kind else {
        panic!("期望调用，实际为 {:?}", init.kind);
    };
    assert!(matches!(callee.kind, ExprKind::TypeApply { .. }));

    let ItemKind::Val(b) = &file.items[1].kind else {
        panic!("期望 val");
    };
    let init = b.init.as_ref().expect("应有 initializer");
    let ExprKind::Binary { op, .. } = &init.kind else {
        panic!("期望 `<` 比较，实际为 {:?}", init.kind);
    };
    assert_eq!(*op, BinaryOp::Lt);
}

#[test]
fn parse_class_lit_splice_tuple_index() {
    let (file, _i) =
        ok("val k = String::class\nval q = a.b.C::class\nval s = rec.[field]\nval t = pair.0\n");
    assert_eq!(file.items.len(), 4);
    let ItemKind::Val(k) = &file.items[0].kind else {
        panic!("期望 val");
    };
    assert!(matches!(
        k.init.as_ref().map(|e| &e.kind),
        Some(ExprKind::ClassLit { .. })
    ));
    let ItemKind::Val(s) = &file.items[2].kind else {
        panic!("期望 val");
    };
    assert!(matches!(
        s.init.as_ref().map(|e| &e.kind),
        Some(ExprKind::SpliceField { .. })
    ));
    let ItemKind::Val(t) = &file.items[3].kind else {
        panic!("期望 val");
    };
    let init = t.init.as_ref().expect("应有 initializer");
    let ExprKind::MemberAccess { member, .. } = &init.kind else {
        panic!("期望 member access");
    };
    assert!(matches!(member, MemberName::TupleIndex { value: 0, .. }));
}

#[test]
fn parse_fstring_with_holes() {
    let (file, _i) = ok("val s = f\"a${x + 1}b${if (c) { 1 } else { 2 }}\"\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::InterpolatedString { raw, parts } = &init.kind else {
        panic!("期望插值字符串");
    };
    assert!(!raw);
    // text / hole / text / hole（尾部无 text 时 4 段）。
    let exprs = parts
        .iter()
        .filter(|p| matches!(p, StringPart::Expr(_)))
        .count();
    assert_eq!(exprs, 2, "应有两个 hole：{parts:?}");
}

#[test]
fn parse_lambda_forms_and_trailing_lambda() {
    let (file, _i) = ok(
        "val a = { 1 }\nval b = { -> 1 }\nval c = { x: Int -> x + 1 }\nval d = combine(1) { x -> x } { y -> y }\n",
    );
    let ItemKind::Val(a) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = a.init.as_ref().expect("应有 initializer");
    let ExprKind::Lambda(lam) = &init.kind else {
        panic!("期望 lambda");
    };
    assert!(matches!(lam.body, LambdaBody::Expr(_)), "单表达式应解包");

    let ItemKind::Val(d) = &file.items[3].kind else {
        panic!("期望 val");
    };
    let init = d.init.as_ref().expect("应有 initializer");
    let ExprKind::Call { args, .. } = &init.kind else {
        panic!(
            "多个 trailing lambda 应折叠为一个 Call，实际为 {:?}",
            init.kind
        );
    };
    assert_eq!(args.len(), 3, "combine(1) + 两个 trailing lambda");
}

#[test]
fn parse_with_update_and_float_field_path() {
    let (file, _i) = ok("val q = p with { pos.x: 1, 0.1: v }\n");
    let ItemKind::Val(v) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let init = v.init.as_ref().expect("应有 initializer");
    let ExprKind::WithUpdate { updates, .. } = &init.kind else {
        panic!("期望 WithUpdate");
    };
    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].path.segments.len(), 2);
    // `0.1` 拆成两个整数段。
    assert_eq!(updates[1].path.segments.len(), 2);
    assert!(matches!(
        updates[1].path.segments[0],
        MemberName::TupleIndex { value: 0, .. }
    ));
    assert!(matches!(
        updates[1].path.segments[1],
        MemberName::TupleIndex { value: 1, .. }
    ));
}

#[test]
fn parse_receiver_function_type_and_nullable_fn_type() {
    let (file, _i) = ok("val f: T.(A, B) -> R = g\nval h: (() -> Unit / Pure!)? = k\n");
    let ItemKind::Val(f) = &file.items[0].kind else {
        panic!("期望 val");
    };
    let ty = f.ty.as_ref().expect("应有类型标注");
    let TypeRefKind::ReceiverFunction { params, .. } = &ty.kind else {
        panic!("期望 receiver function type，实际为 {:?}", ty.kind);
    };
    assert_eq!(params.len(), 2);

    let ItemKind::Val(h) = &file.items[1].kind else {
        panic!("期望 val");
    };
    let ty = h.ty.as_ref().expect("应有类型标注");
    let TypeRefKind::Nullable(inner) = &ty.kind else {
        panic!("期望 nullable function type");
    };
    let TypeRefKind::Function { effect, .. } = &inner.kind else {
        panic!("期望函数类型（分组透明）");
    };
    let row = effect.as_ref().expect("应有 effect 行");
    assert!(row.closed.is_some(), "`Pure!` 应记录闭合标记");
}

#[test]
fn parse_file_annotations() {
    let (file, interner) = ok("@file:JvmName(\"x\")\npackage a\nfun f() {}\n");
    assert_eq!(file.file_annotations.len(), 1);
    let ann = &file.file_annotations[0];
    let target = ann.target.expect("应有 use-site target");
    assert_eq!(interner.resolve(target.symbol), "file");
}

#[test]
fn parse_operator_fun_and_modifiers_sorted() {
    let (file, _i) =
        ok("public operator fun plus(a: Int, b: Int): Int = a + b\noverride open class C\n");
    let ItemKind::Fun(f) = &file.items[0].kind else {
        panic!("期望 fun");
    };
    let kinds: Vec<ModifierKind> = f.modifiers.iter().map(|m| m.kind).collect();
    assert_eq!(kinds, vec![ModifierKind::Public, ModifierKind::Operator]);

    // 修饰符排序去重（源码顺序无关）。
    let ItemKind::Type(t) = &file.items[1].kind else {
        panic!("期望 class");
    };
    let kinds: Vec<ModifierKind> = t.modifiers.iter().map(|m| m.kind).collect();
    assert_eq!(kinds, vec![ModifierKind::Open, ModifierKind::Override]);
}

#[test]
fn dump_ast_snapshot_smoke() {
    let text = dump("package a\nfun main() {\n    val x = 1 + 2\n}\n");
    assert!(text.contains("File 0..43"), "{text}");
    assert!(text.contains("FunDecl"), "{text}");
    assert!(text.contains("name=main"), "{text}");
    assert!(text.contains("Binary"), "{text}");
    // 确定性：同一输入渲染两次一致。
    let again = dump("package a\nfun main() {\n    val x = 1 + 2\n}\n");
    assert_eq!(text, again);
}

#[test]
fn node_ids_cover_all_nodes() {
    // NodeId 空间：每个节点都有唯一 id，node_count = 分配数量。
    let result = parse("package a\nfun f() { val x = [1, 2] }\n");
    assert!(result.node_count > 5);
}
