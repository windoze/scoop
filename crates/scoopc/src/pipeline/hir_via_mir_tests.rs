use std::collections::HashSet;

use crate::hir::{Block, CallArg, Expr, ExprKind, Item, LoweredHir, StmtKind, ValDecl, ValueRef};
use crate::session::Session;
use crate::source::SourceFile;

fn lower_typed_single_source_file_via_mir_instance_collection(
    sess: &Session,
    source: &SourceFile,
) -> LoweredHir {
    let context =
        crate::frontend::prepare_virtual_cone_context_with_options(source.clone(), sess.options())
            .unwrap();
    let front = crate::frontend::run_project_frontend(sess, context).unwrap();
    crate::frontend::lower_hir_for_codegen_with_request_root_mode(
        sess,
        &front,
        crate::opt::OptLevel::O0,
        crate::frontend::MirRequestRootMode::RequestSources,
    )
    .unwrap()
    .lowered_hir
}

fn collect_top_level_call_fqns_in_expr(expr: &Expr, out: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind {
                out.push(fqn.clone());
            }
            collect_top_level_call_fqns_in_expr(callee, out);
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                    CallArg::Named { value, .. } => collect_top_level_call_fqns_in_expr(value, out),
                }
            }
        }
        ExprKind::MemberAccess { receiver, .. } => {
            collect_top_level_call_fqns_in_expr(receiver, out);
        }
        ExprKind::When { subject, arms } => {
            collect_top_level_call_fqns_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_top_level_call_fqns_in_expr(guard, out);
                }
                collect_top_level_call_fqns_in_expr(&arm.body, out);
            }
        }
        ExprKind::Block(block) => collect_top_level_call_fqns_in_block(block, out),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_top_level_call_fqns_in_expr(cond, out);
            collect_top_level_call_fqns_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_top_level_call_fqns_in_expr(else_branch, out);
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => collect_top_level_call_fqns_in_expr(expr, out),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_top_level_call_fqns_in_expr(lhs, out);
            collect_top_level_call_fqns_in_expr(rhs, out);
        }
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_top_level_call_fqns_in_expr(&field.value, out);
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_top_level_call_fqns_in_expr(element, out);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_top_level_call_fqns_in_expr(expr, out);
                }
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => collect_top_level_call_fqns_in_expr(expr, out),
                    CallArg::Named { value, .. } => collect_top_level_call_fqns_in_expr(value, out),
                }
            }
        }
        ExprKind::Handle(handle) => {
            collect_top_level_call_fqns_in_block(&handle.body, out);
            for arm in &handle.arms {
                collect_top_level_call_fqns_in_expr(&arm.body, out);
            }
            if let Some(finally) = handle.finally.as_ref() {
                collect_top_level_call_fqns_in_block(finally, out);
            }
        }
        ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Closure(_)
        | ExprKind::Missing
        | ExprKind::Todo(_) => {}
    }
}

fn collect_top_level_call_fqns_in_block(block: &Block, out: &mut Vec<String>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Expr(expr) => collect_top_level_call_fqns_in_expr(expr, out),
            StmtKind::Val(val) => {
                if let Some(init) = val.init.as_ref() {
                    collect_top_level_call_fqns_in_expr(init, out);
                }
            }
            StmtKind::Assign { lhs, rhs, .. } => {
                collect_top_level_call_fqns_in_expr(lhs, out);
                collect_top_level_call_fqns_in_expr(rhs, out);
            }
            StmtKind::While { cond, body } => {
                collect_top_level_call_fqns_in_expr(cond, out);
                collect_top_level_call_fqns_in_block(body, out);
            }
            StmtKind::Return { value } => {
                if let Some(value) = value.as_ref() {
                    collect_top_level_call_fqns_in_expr(value, out);
                }
            }
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
        }
    }
}

fn find_top_level_call_in_block<'a>(block: &'a Block, fqn: &str) -> Option<&'a Expr> {
    block.stmts.iter().find_map(|stmt| match &stmt.kind {
        StmtKind::Expr(expr) => find_top_level_call_in_expr(expr, fqn),
        StmtKind::Val(val) => val
            .init
            .as_ref()
            .and_then(|expr| find_top_level_call_in_expr(expr, fqn)),
        StmtKind::Assign { lhs, rhs, .. } => {
            find_top_level_call_in_expr(lhs, fqn).or_else(|| find_top_level_call_in_expr(rhs, fqn))
        }
        StmtKind::While { cond, body } => find_top_level_call_in_expr(cond, fqn)
            .or_else(|| find_top_level_call_in_block(body, fqn)),
        StmtKind::Return { value } => value
            .as_ref()
            .and_then(|expr| find_top_level_call_in_expr(expr, fqn)),
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => None,
    })
}

fn find_top_level_call_in_expr<'a>(expr: &'a Expr, fqn: &str) -> Option<&'a Expr> {
    match &expr.kind {
        ExprKind::Call { callee, args } => {
            if let ExprKind::VarRef(ValueRef::TopLevel {
                fqn: callee_fqn, ..
            }) = &callee.kind
                && callee_fqn == fqn
            {
                return Some(expr);
            }
            find_top_level_call_in_expr(callee, fqn).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    CallArg::Positional(expr) => find_top_level_call_in_expr(expr, fqn),
                    CallArg::Named { value, .. } => find_top_level_call_in_expr(value, fqn),
                })
            })
        }
        ExprKind::MemberAccess { receiver, .. } => find_top_level_call_in_expr(receiver, fqn),
        ExprKind::When { subject, arms } => {
            find_top_level_call_in_expr(subject, fqn).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(|guard| find_top_level_call_in_expr(guard, fqn))
                        .or_else(|| find_top_level_call_in_expr(&arm.body, fqn))
                })
            })
        }
        ExprKind::Block(block) => find_top_level_call_in_block(block, fqn),
        ExprKind::Closure(closure) => find_top_level_call_in_expr(&closure.body, fqn),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => find_top_level_call_in_expr(cond, fqn)
            .or_else(|| find_top_level_call_in_expr(then_branch, fqn))
            .or_else(|| {
                else_branch
                    .as_deref()
                    .and_then(|expr| find_top_level_call_in_expr(expr, fqn))
            }),
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => find_top_level_call_in_expr(expr, fqn),
        ExprKind::Binary { lhs, rhs, .. } => {
            find_top_level_call_in_expr(lhs, fqn).or_else(|| find_top_level_call_in_expr(rhs, fqn))
        }
        ExprKind::StructLit { fields, .. } => fields
            .iter()
            .find_map(|field| find_top_level_call_in_expr(&field.value, fqn)),
        ExprKind::TupleLit { elements } => elements
            .iter()
            .find_map(|element| find_top_level_call_in_expr(element, fqn)),
        ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| match part {
            crate::hir::InterpolatedStringPart::Expr { expr } => {
                find_top_level_call_in_expr(expr, fqn)
            }
            crate::hir::InterpolatedStringPart::Text { .. } => None,
        }),
        ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
            CallArg::Positional(expr) => find_top_level_call_in_expr(expr, fqn),
            CallArg::Named { value, .. } => find_top_level_call_in_expr(value, fqn),
        }),
        ExprKind::Handle(handle) => find_top_level_call_in_block(&handle.body, fqn)
            .or_else(|| {
                handle
                    .arms
                    .iter()
                    .find_map(|arm| find_top_level_call_in_expr(&arm.body, fqn))
            })
            .or_else(|| {
                handle
                    .finally
                    .as_ref()
                    .and_then(|block| find_top_level_call_in_block(block, fqn))
            }),
        ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Missing
        | ExprKind::Todo(_) => None,
    }
}

#[test]
fn compilation_unit_via_mir_instances_materializes_non_intrinsic_direct_call_targets() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t5000e3d_via_mir_instances>",
        r#"
package fixtures.t5000e3d

import scoop.core.*

fun <T> id(x: T): T { return x }

object Box {
    fun <T> memberId(x: T): T { return id(x) }
}

fun main(): Int {
    val a: Int = id(1)
    val b: Int = Box.memberId(2)
    return a + b
}
"#,
    );

    let lowered = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source);

    let main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.t5000e3d.main");
    let main_body = main.body.as_ref().expect("main 应有 body");
    assert!(
        find_top_level_call_in_block(main_body, "fixtures.t5000e3d.id::<Int>").is_some(),
        "via-mir compilation-unit lowering 应把 standalone generic direct-call 物化为实例目标"
    );
    assert!(
        find_top_level_call_in_block(main_body, "fixtures.t5000e3d.Box.memberId::<Int>").is_some(),
        "via-mir compilation-unit lowering 应把 generic member direct-call 物化为实例目标"
    );

    let member_fun = lowered
        .member_funs
        .iter()
        .find(|fun| fun.fqn == "fixtures.t5000e3d.Box.memberId::<Int>")
        .expect("应收集到 Box.memberId::<Int> 单态成员实例");
    let member_body = member_fun.body.as_ref().expect("memberId::<Int> 应有 body");
    assert!(
        find_top_level_call_in_block(member_body, "fixtures.t5000e3d.id::<Int>").is_some(),
        "单态成员实例体内的 generic direct-call 应直接指向已实例化 target"
    );
}

#[test]
fn compilation_unit_via_mir_instances_keeps_overloaded_generic_identity_distinct() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t5000e3d_overload_identity>",
        r#"
package fixtures.t5000e3d

import scoop.core.*

fun <T> pick(x: T): T { return x }
fun <T> pick(x: T, y: T): T { return y }

object Box {
    fun <T> pick(x: T): T { return x }
    fun <T> pick(x: T, y: T): T { return y }
}

fun main(): Int {
    val a: Int = pick(1)
    val b: Int = pick(1, 2)
    val c: Int = Box.pick(3)
    val d: Int = Box.pick(3, 4)
    val f: (Int) -> Int = pick<Int>
    val e: Int = f(5)
    return a + b + c + d + e
}
"#,
    );

    let lowered = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source);

    let top_level_pick_instances = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun)
                if fun.fqn.starts_with("fixtures.t5000e3d.pick::<Int>")
                    && fun.fqn != "fixtures.t5000e3d.main" =>
            {
                Some((fun.fqn.clone(), fun.params.len()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let top_level_pick_fqns = top_level_pick_instances
        .iter()
        .map(|(fqn, _)| fqn.clone())
        .collect::<HashSet<_>>();
    assert_eq!(top_level_pick_fqns.len(), 2);
    let unary_pick_fqn = top_level_pick_instances
        .iter()
        .find_map(|(fqn, param_count)| (*param_count == 1).then_some(fqn.clone()))
        .expect("应收集到 unary pick::<Int> 实例");
    let binary_pick_fqn = top_level_pick_instances
        .iter()
        .find_map(|(fqn, param_count)| (*param_count == 2).then_some(fqn.clone()))
        .expect("应收集到 binary pick::<Int> 实例");
    assert_ne!(unary_pick_fqn, binary_pick_fqn);

    let member_pick_fqns = lowered
        .member_funs
        .iter()
        .filter(|fun| fun.fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
        .map(|fun| fun.fqn.clone())
        .collect::<HashSet<_>>();
    assert_eq!(member_pick_fqns.len(), 2);

    let main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.t5000e3d.main");
    let main_body = main.body.as_ref().expect("main 应有 body");
    let mut main_call_fqns = Vec::new();
    collect_top_level_call_fqns_in_block(main_body, &mut main_call_fqns);
    let direct_top_level_calls = main_call_fqns
        .iter()
        .filter(|fqn| fqn.starts_with("fixtures.t5000e3d.pick::<Int>"))
        .cloned()
        .collect::<HashSet<_>>();
    assert_eq!(direct_top_level_calls.len(), 2);
    let direct_member_calls = main_call_fqns
        .iter()
        .filter(|fqn| fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
        .cloned()
        .collect::<HashSet<_>>();
    assert_eq!(direct_member_calls.len(), 2);

    let fun_value_init = main_body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Val(ValDecl {
                name: Some(name),
                init: Some(init),
                ..
            }) if name == "f" => Some(init),
            _ => None,
        })
        .expect("main 应包含函数值绑定 f");
    let ExprKind::Closure(closure) = &fun_value_init.kind else {
        panic!(
            "generic top-level function value 应被 lower 为 closure，实际为 {:?}",
            fun_value_init.kind
        );
    };
    let mut closure_call_fqns = Vec::new();
    collect_top_level_call_fqns_in_expr(&closure.body, &mut closure_call_fqns);
    assert!(closure_call_fqns.contains(&unary_pick_fqn));
    assert!(!closure_call_fqns.contains(&binary_pick_fqn));
}

#[test]
fn compilation_unit_via_mir_instances_keeps_overloaded_generic_identity_path_stable() {
    let sess = Session::new().unwrap();
    let program = r#"
package fixtures.t5000e3d

import scoop.core.*

fun <T> pick(x: T): T { return x }
fun <T> pick(x: T, y: T): T { return y }

object Box {
    fun <T> pick(x: T): T { return x }
    fun <T> pick(x: T, y: T): T { return y }
}

fun main(): Int {
    val a: Int = pick(1)
    val b: Int = pick(1, 2)
    val c: Int = Box.pick(3)
    val d: Int = Box.pick(3, 4)
    return a + b + c + d
}
"#;
    let source_a = SourceFile::new_virtual(
        "/tmp/root-a/fixtures/t5000e3d_overload_identity.scoop",
        program,
    );
    let source_b = SourceFile::new_virtual(
        "/tmp/root-b/fixtures/t5000e3d_overload_identity.scoop",
        program,
    );

    let lowered_a = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source_a);
    let lowered_b = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source_b);

    let top_level_instances = |lowered: &LoweredHir| {
        lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun)
                    if fun.fqn.starts_with("fixtures.t5000e3d.pick::<Int>")
                        && fun.fqn != "fixtures.t5000e3d.main" =>
                {
                    Some(fun.fqn.clone())
                }
                _ => None,
            })
            .collect::<HashSet<_>>()
    };
    let member_instances = |lowered: &LoweredHir| {
        lowered
            .member_funs
            .iter()
            .filter(|fun| fun.fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
            .map(|fun| fun.fqn.clone())
            .collect::<HashSet<_>>()
    };
    let main_direct_targets = |lowered: &LoweredHir| {
        let main = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.t5000e3d.main" => Some(fun),
                _ => None,
            })
            .expect("应收集到 fixtures.t5000e3d.main");
        let main_body = main.body.as_ref().expect("main 应有 body");
        let mut call_fqns = Vec::new();
        collect_top_level_call_fqns_in_block(main_body, &mut call_fqns);
        call_fqns
            .into_iter()
            .filter(|fqn| {
                fqn.starts_with("fixtures.t5000e3d.pick::<Int>")
                    || fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>")
            })
            .collect::<HashSet<_>>()
    };

    assert_eq!(
        top_level_instances(&lowered_a),
        top_level_instances(&lowered_b)
    );
    assert_eq!(member_instances(&lowered_a), member_instances(&lowered_b));
    assert_eq!(
        main_direct_targets(&lowered_a),
        main_direct_targets(&lowered_b)
    );
}

#[test]
fn via_mir_instance_collection_materializes_generic_value_property_getter_target() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t4010b1a>",
        r#"
package fixtures.t4010b1a

import scoop.core.*

struct Box<T>(val value: T) {
    val readBack: T
        get() = this.value
}

fun main(): Int {
    val result: Int = Box(2).readBack
    return result
}
"#,
    );

    let lowered = lower_typed_single_source_file_via_mir_instance_collection(&sess, &source);

    let main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t4010b1a.main" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.t4010b1a.main");
    let body = main.body.as_ref().expect("main 应有 body");
    let result_init = body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Val(decl) if decl.name.as_deref() == Some("result") => decl.init.as_ref(),
            _ => None,
        })
        .expect("main 应包含 result 初始化");
    let ExprKind::Call { callee, args } = &result_init.kind else {
        panic!("generic getter access 应 lowering 成 direct call: {result_init:#?}");
    };
    let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
        panic!("generic getter callee 应为 top-level value ref: {callee:#?}");
    };
    assert_eq!(fqn, "fixtures.t4010b1a.Box.readBack::<Int>");
    assert_eq!(args.len(), 1);

    let getter = lowered
        .member_funs
        .iter()
        .find(|fun| fun.fqn == "fixtures.t4010b1a.Box.readBack::<Int>")
        .expect("应收集到具体化后的 getter 实例");
    assert!(matches!(
        lowered.types.kind(getter.return_ty),
        crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Int)
    ));
}
