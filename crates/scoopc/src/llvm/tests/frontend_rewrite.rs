//! Frontend rewrite tests: string compare/concat to direct calls, builtin string members, top-level generic / callable-value named args, direct-hir reachability, mir inlining.

#![allow(dead_code, clippy::too_many_lines)]

use super::*;

#[test]
pub(super) fn frontend_codegen_rewrites_string_literal_compare_to_and_concat_to_member_direct_calls()
 {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered main body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1c_string_literal_direct_calls.scoop",
        r#"
package fixtures.t5000j1c

import scoop.core.*

fun main(): Int {
    val strCmp = "ab".compareTo("ac") < 0
    val strEq = "hi".concat("!") == "hi!"
    return if (strCmp && strEq) { 0 } else { 1 }
}
"#,
    );

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let main = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == "fixtures.t5000j1c.main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");

    let hir::ExprKind::Call {
        callee: cmp_callee,
        args: cmp_args,
    } = &find_local_init(body, "strCmp").kind
    else {
        panic!("strCmp should lower to an Int.lt method call");
    };
    let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &cmp_callee.kind else {
        panic!(
            "expected Int.lt top-level callee, actual: {:?}",
            cmp_callee.kind
        );
    };
    assert_eq!(fqn, "scoop.core.Int.lt");
    let Some(hir::CallArg::Positional(cmp_lhs)) = cmp_args.first() else {
        panic!("Int.lt should receive compareTo result as first arg: {cmp_args:?}");
    };
    let Some(hir::CallArg::Positional(cmp_rhs)) = cmp_args.get(1) else {
        panic!("Int.lt should receive zero as second arg: {cmp_args:?}");
    };
    assert_top_level_call(cmp_lhs, "scoop.core.String.compareTo", 2);
    assert!(matches!(
        cmp_rhs.kind,
        hir::ExprKind::Literal(hir::LiteralKind::Int | hir::LiteralKind::SynthInt(0))
    ));

    let hir::ExprKind::Binary {
        lhs: concat_lhs,
        op: concat_op,
        rhs: concat_rhs,
        ..
    } = &find_local_init(body, "strEq").kind
    else {
        panic!("strEq should lower to a binary equality expression");
    };
    assert_eq!(*concat_op, ast::BinaryOp::Eq);
    assert_top_level_call(concat_lhs, "scoop.core.String.concat", 2);
    assert!(matches!(
        concat_rhs.kind,
        hir::ExprKind::Literal(hir::LiteralKind::String)
    ));
}

#[test]
pub(super) fn builtin_string_intrinsic_member_calls_lower_to_direct_calls() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered function body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    fn mir_fun_contains_direct_call(fun: &crate::mir::FunDecl, expected_fqn: &str) -> bool {
        let Some(body) = &fun.body else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return false;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    return false;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return false;
                };
                callee_fqn == expected_fqn
            })
        })
    }

    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1d_string_intrinsic_member_calls.scoop",
        r#"
package fixtures.t5000j1d

import scoop.core.*

fun inspect(s: String, idx: Int): Int {
    val len = s.byteLength()
    val byte = s.getByte(idx)
    val slice = @Unsafe do { s.unsafeSliceBytes(0, len) }
    return byte + slice.byteLength()
}

fun main(): Int {
    return inspect("hello", 1)
}
"#,
    );
    let inspect_fqn = "fixtures.t5000j1d.inspect";

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let inspect = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == inspect_fqn => Some(fun),
            _ => None,
        })
        .expect("expected lowered inspect helper");
    let body = inspect.body.as_ref().expect("inspect should have a body");

    assert_top_level_call(find_local_init(body, "len"), "scoop.core.byteLength", 1);
    assert_top_level_call(find_local_init(body, "byte"), "scoop.core.getByte", 2);

    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let inspect_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == inspect_fqn)
        .expect("inspect helper should enter caller-side pass candidates");
    assert!(
        !mir_fun_contains_fun_value_call(inspect_mir),
        "String intrinsic member calls should lower to direct contracts, not FunValue calls"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.byteLength"),
        "materialized MIR should contain a direct call to scoop.core.byteLength"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.getByte"),
        "materialized MIR should contain a direct call to scoop.core.getByte"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.String.unsafeSliceBytes"),
        "materialized MIR should contain a direct call to the String.unsafeSliceBytes body"
    );
}

#[test]
pub(super) fn builtin_string_member_calls_lower_to_direct_calls() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered function body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    fn mir_fun_contains_direct_call(fun: &crate::mir::FunDecl, expected_fqn: &str) -> bool {
        let Some(body) = &fun.body else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return false;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    return false;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return false;
                };
                callee_fqn == expected_fqn
            })
        })
    }

    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000j1e_string_member_calls.scoop",
        r#"
package fixtures.t5000j1e

import scoop.core.*

fun inspect(s: String): Int {
    val empty = s.isEmpty()
    val replaced = s.replace("a", "b")
    val code = s.charAt(1)
    val repeated = s.repeat(2)
    val emptyScore = if (empty) { 0 } else { 1 }
    return code + replaced.byteLength() + repeated.byteLength() + emptyScore
}

fun main(): Int {
    return inspect("ab")
}
"#,
    );
    let inspect_fqn = "fixtures.t5000j1e.inspect";

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let inspect = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.fqn == inspect_fqn => Some(fun),
            _ => None,
        })
        .expect("expected lowered inspect helper");
    let body = inspect.body.as_ref().expect("inspect should have a body");

    assert_top_level_call(
        find_local_init(body, "empty"),
        "scoop.core.String.isEmpty",
        1,
    );
    assert_top_level_call(
        find_local_init(body, "replaced"),
        "scoop.core.String.replace",
        3,
    );
    assert_top_level_call(find_local_init(body, "code"), "scoop.core.String.charAt", 2);
    assert_top_level_call(
        find_local_init(body, "repeated"),
        "scoop.core.String.repeat",
        2,
    );

    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let inspect_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == inspect_fqn)
        .expect("inspect helper should enter caller-side pass candidates");
    assert!(
        !mir_fun_contains_fun_value_call(inspect_mir),
        "String builtin member calls should lower to direct contracts, not FunValue calls"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.String.isEmpty"),
        "materialized MIR should contain a direct call to scoop.core.String.isEmpty"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.String.replace"),
        "materialized MIR should contain a direct call to scoop.core.String.replace"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.String.charAt"),
        "materialized MIR should contain a direct call to scoop.core.String.charAt"
    );
    assert!(
        mir_fun_contains_direct_call(inspect_mir, "scoop.core.String.repeat"),
        "materialized MIR should contain a direct call to scoop.core.String.repeat"
    );
}

#[test]
pub(super) fn builtin_string_trim_indent_member_calls_lower_to_direct_calls() {
    fn find_local_init<'a>(body: &'a hir::Block, name: &str) -> &'a hir::Expr {
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                hir::StmtKind::Val(val) if val.name.as_deref() == Some(name) => val.init.as_ref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected local `{name}` in lowered function body"))
    }

    fn assert_top_level_call(expr: &hir::Expr, expected_fqn: &str, expected_args: usize) {
        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            panic!("expected direct call expr, actual: {:?}", expr.kind);
        };
        let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            panic!("expected top-level callee, actual: {:?}", callee.kind);
        };
        assert_eq!(fqn, expected_fqn);
        assert_eq!(args.len(), expected_args);
    }

    fn mir_fun_direct_call_count(fun: &crate::mir::FunDecl, expected_fqn: &str) -> usize {
        let Some(body) = &fun.body else {
            return 0;
        };
        body.blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .filter(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return false;
                };
                let crate::mir::Rvalue::Call { kind, .. } = value else {
                    return false;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return false;
                };
                callee_fqn == expected_fqn
            })
            .count()
    }

    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/string_trim_indent_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let main = codegen_unit
        .lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            hir::Item::Fun(fun) if fun.name == "main" => Some(fun),
            _ => None,
        })
        .expect("expected lowered main");
    let body = main.body.as_ref().expect("main should have a body");

    assert_top_level_call(
        find_local_init(body, "s"),
        "scoop.core.String.trimIndent",
        1,
    );
    assert_top_level_call(
        find_local_init(body, "again"),
        "scoop.core.String.trimIndent",
        1,
    );

    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 materialized MIR");
    let main_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.name == "main")
        .expect("main should enter caller-side pass candidates");
    assert!(
        !mir_fun_contains_fun_value_call(main_mir),
        "sysroot String.trimIndent() member calls should lower to direct contracts, not FunValue calls"
    );
    assert_eq!(
        mir_fun_direct_call_count(main_mir, "scoop.core.String.trimIndent"),
        2,
        "materialized MIR should contain exactly two direct calls to scoop.core.String.trimIndent"
    );
}

#[test]
pub(super) fn top_level_generic_named_args_keep_canonical_param_order_in_pass_mir() {
    fn direct_call_arg_local_names(fun: &crate::mir::FunDecl, expected_fqn: &str) -> Vec<String> {
        let body = fun.body.as_ref().expect("expected MIR body");
        let args = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let crate::mir::Rvalue::Call { kind, args, .. } = value else {
                    return None;
                };
                let crate::mir::CallKind::Direct { callee_fqn } = kind else {
                    return None;
                };
                (callee_fqn == expected_fqn).then_some(args)
            })
            .unwrap_or_else(|| panic!("expected direct call to `{expected_fqn}`"));
        args.iter()
            .map(|arg| match &arg.value {
                crate::mir::Operand::Local(local) => body.locals[local.as_u32() as usize]
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("l{}", local.as_u32())),
                other => panic!("expected local call arg for `{expected_fqn}`, actual: {other:?}"),
            })
            .collect()
    }

    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend should keep materialized MIR");
    let main_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.name == "main")
        .expect("main should enter caller-side pass candidates");

    assert_eq!(
        direct_call_arg_local_names(main_mir, "pick::<Int>"),
        vec!["__call_arg_1".to_string(), "__call_arg_0".to_string()],
        "materialized MIR should preserve the HIR canonical param order for named args"
    );
}

#[test]
pub(super) fn callable_value_and_top_level_funptr_named_args_keep_binding_order_in_mir() {
    fn call_arg_spans_at_stmt(
        fun: &crate::mir::FunDecl,
        stmt_span: crate::span::Span,
    ) -> Vec<crate::span::Span> {
        let body = fun.body.as_ref().expect("expected MIR body");
        let args = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let crate::mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let crate::mir::Rvalue::Call { args, .. } = value else {
                    return None;
                };
                (stmt.span == stmt_span).then_some(args)
            })
            .unwrap_or_else(|| panic!("expected call statement at span {stmt_span:?}"));
        args.iter().map(|arg| arg.span).collect()
    }

    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();

    let codegen_unit = frontend::prepare_single_file_codegen_unit(&session, &source).unwrap();
    let materialized = codegen_unit
        .lowered
        .materialized_mir()
        .expect("production frontend should keep materialized MIR");
    let main_mir = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.name == "main")
        .expect("main should enter caller-side pass candidates");

    assert_eq!(
        call_arg_spans_at_stmt(main_mir, crate::span::Span::new(1442, 1469)),
        vec![
            crate::span::Span::new(1463, 1468),
            crate::span::Span::new(1449, 1450)
        ],
        "named receiver callable-value call should reorder args to receiver-then-a0 in MIR"
    );
    assert_eq!(
        call_arg_spans_at_stmt(main_mir, crate::span::Span::new(1770, 1797)),
        vec![
            crate::span::Span::new(1795, 1796),
            crate::span::Span::new(1781, 1782)
        ],
        "top-level FunPtr named direct call should reorder args to receiver-then-a0 in MIR"
    );
}

#[test]
pub(super) fn callable_value_pattern_binder_receiver_named_args_fixture_codegen_succeeds() {
    let session = Session::new().unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop")
        .canonicalize()
        .unwrap();
    let source = SourceFile::load(&fixture).unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();

    assert!(
        ir.contains("top_level_val_init") && ir.contains("__scoop_priv0__closure_body__h"),
        "top-level callable-value initializers should lower pure closure carriers without requiring a published dynamic entry\n{ir}"
    );
}

#[test]
pub(super) fn direct_hir_reachability_emits_object_init_helper_dependency_for_hir_top_level_ref() {
    let source = SourceFile::new_virtual(
        "<mem>/t5000j3ar_direct_hir_object_init_helper_dep.scoop",
        r#"
package a

object BoomObject {
    init {
        helper()
    }
}

fun helper() {}

fun main(): Int {
    val _obj = BoomObject
    return 0
}
"#,
    );

    let session = Session::new().unwrap();
    let ir = emit_minimal_main_ir(&session, &source).unwrap();
    let helper_ir = function_ir_matching(
        &ir,
        "user helper referenced only by object init body",
        |header, function| {
            !header.contains("@main(")
                && stable_id_symbol_is_user_callable(llvm_function_symbol_name(function))
                && !stable_id_ir_contains_hidden_init_call(function)
                && !function.contains(" call ")
        },
    );
    let helper_symbol = llvm_function_symbol_name(helper_ir);
    let object_init_ir = function_ir_matching(
        &ir,
        "compiler-private object init helper for BoomObject",
        |header, function| {
            !header.contains("@main(")
                && llvm_function_symbol_name(function) != helper_symbol
                && function_ir_calls_symbol(function, helper_symbol)
        },
    );
    let object_init_symbol = llvm_function_symbol_name(object_init_ir);

    assert!(
        function_ir_calls_symbol(object_init_ir, helper_symbol),
        "direct-HIR reachability 也必须保留 object init body 对 helper 的调用:\n{object_init_ir}"
    );
    assert!(
        stable_id_symbol_has_private_role(object_init_symbol, "object_init"),
        "object init helper 应收口到 object_init private role，实际符号: {object_init_symbol}"
    );
    assert!(
        ir.lines()
            .any(|line| line.starts_with("define ")
                && llvm_line_mentions_symbol(line, helper_symbol)),
        "仅由 object init body 触达的 helper 仍必须在模块中拥有定义:\n{ir}"
    );
}

#[test]
pub(super) fn production_codegen_respects_mir_inlining_opt_level_gate() {
    let session = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/t5000hr_mir_inline_opt_gate.scoop",
        r#"
package fixtures.t5000hr

fun <T> id(x: T): T {
    return x
}

fun <T> wrap(x: T): T {
    return id<T>(x)
}

fun main(): Int {
    return wrap<Int>(1)
}
"#,
    );
    let wrap_fqn = "fixtures.t5000hr.wrap::<Int>";
    let id_fqn = "fixtures.t5000hr.id::<Int>";

    let o0_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O0)
            .unwrap();
    let o0_materialized = o0_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 O0 materialized MIR");
    assert!(
        !o0_materialized
            .pass_view()
            .callable_body_is_overridden(wrap_fqn),
        "O0 不应运行 summary-driven MIR inlining 并覆盖 pass body"
    );
    let o0_wrap = o0_materialized
        .pass_view()
        .callable(wrap_fqn)
        .expect("O0 pass view 仍应可读取 raw materialized body");
    assert!(
        mir_fun_contains_direct_call(o0_wrap, id_fqn),
        "O0 pass view 不应消除 wrap -> id direct call"
    );

    let o2_unit =
        frontend::prepare_single_file_codegen_unit_with_opt_level(&session, &source, OptLevel::O2)
            .unwrap();
    let o2_materialized = o2_unit
        .lowered
        .materialized_mir()
        .expect("production frontend 应保留 O2 materialized MIR");
    assert!(
        o2_materialized
            .pass_view()
            .callable_body_is_overridden(wrap_fqn),
        "O2 应运行 summary-driven MIR inlining 并覆盖 pass body"
    );
    let o2_wrap = o2_materialized
        .pass_view()
        .callable(wrap_fqn)
        .expect("O2 pass view 应保留 rewritten wrap body");
    assert!(
        !mir_fun_contains_direct_call(o2_wrap, id_fqn),
        "O2 pass view 应消除 wrap -> id direct call"
    );
}
