//! HIR lowering integration tests.

#![allow(dead_code)]

use super::*;

use crate::hir::LiteralKind;
use crate::hir::{CallArg, CallSite, ClassInitStep, WhenPat};
use crate::resolve::Index;
use crate::session::SessionOptions;
use crate::ty::{RefTypeKind, TypeKind, TypeStore, ValueTypeKind};
use crate::typecheck;
use std::collections::HashSet;
use std::path::PathBuf;

fn collect_unresolved_member_names_in_expr(expr: &Expr, out: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::MemberAccess { receiver, member } => {
            if member.resolved.is_none() {
                out.push(member.name.clone());
            }
            collect_unresolved_member_names_in_expr(receiver, out);
        }
        ExprKind::Call { callee, args } => {
            collect_unresolved_member_names_in_expr(callee, out);
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => collect_unresolved_member_names_in_expr(expr, out),
                    CallArg::Named { value, .. } => {
                        collect_unresolved_member_names_in_expr(value, out)
                    }
                }
            }
        }
        ExprKind::When { subject, arms } => {
            collect_unresolved_member_names_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_unresolved_member_names_in_expr(guard, out);
                }
                collect_unresolved_member_names_in_expr(&arm.body, out);
            }
        }
        ExprKind::Block(block) => collect_unresolved_member_names_in_block(block, out),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_unresolved_member_names_in_expr(cond, out);
            collect_unresolved_member_names_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_unresolved_member_names_in_expr(else_branch, out);
            }
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => collect_unresolved_member_names_in_expr(expr, out),
        ExprKind::Binary { lhs, rhs, .. } => {
            collect_unresolved_member_names_in_expr(lhs, out);
            collect_unresolved_member_names_in_expr(rhs, out);
        }
        ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_unresolved_member_names_in_expr(&field.value, out);
            }
        }
        ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_unresolved_member_names_in_expr(element, out);
            }
        }
        ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_unresolved_member_names_in_expr(expr, out);
                }
            }
        }
        ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => collect_unresolved_member_names_in_expr(expr, out),
                    CallArg::Named { value, .. } => {
                        collect_unresolved_member_names_in_expr(value, out)
                    }
                }
            }
        }
        ExprKind::Handle(handle) => {
            collect_unresolved_member_names_in_block(&handle.body, out);
            for arm in &handle.arms {
                collect_unresolved_member_names_in_expr(&arm.body, out);
            }
            if let Some(finally) = handle.finally.as_ref() {
                collect_unresolved_member_names_in_block(finally, out);
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

fn collect_unresolved_member_names_in_block(block: &Block, out: &mut Vec<String>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            StmtKind::Expr(expr) => collect_unresolved_member_names_in_expr(expr, out),
            StmtKind::Val(val) => {
                if let Some(init) = val.init.as_ref() {
                    collect_unresolved_member_names_in_expr(init, out);
                }
            }
            StmtKind::Assign { lhs, rhs, .. } => {
                collect_unresolved_member_names_in_expr(lhs, out);
                collect_unresolved_member_names_in_expr(rhs, out);
            }
            StmtKind::While { cond, body } => {
                collect_unresolved_member_names_in_expr(cond, out);
                collect_unresolved_member_names_in_block(body, out);
            }
            StmtKind::Return { value } => {
                if let Some(value) = value.as_ref() {
                    collect_unresolved_member_names_in_expr(value, out);
                }
            }
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
        }
    }
}

fn expr_contains_todo_kind(expr: &Expr, kind: &str) -> bool {
    match &expr.kind {
        ExprKind::Todo(found) => *found == kind,
        ExprKind::MemberAccess { receiver, .. } => expr_contains_todo_kind(receiver, kind),
        ExprKind::Call { callee, args } => {
            expr_contains_todo_kind(callee, kind)
                || args.iter().any(|arg| match arg {
                    CallArg::Positional(expr) => expr_contains_todo_kind(expr, kind),
                    CallArg::Named { value, .. } => expr_contains_todo_kind(value, kind),
                })
        }
        ExprKind::StructLit { fields, .. } => fields
            .iter()
            .any(|field| expr_contains_todo_kind(&field.value, kind)),
        ExprKind::TupleLit { elements } => elements
            .iter()
            .any(|element| expr_contains_todo_kind(element, kind)),
        ExprKind::When { subject, arms } => {
            expr_contains_todo_kind(subject, kind)
                || arms
                    .iter()
                    .any(|arm| expr_contains_todo_kind(&arm.body, kind))
        }
        ExprKind::Block(block) => block.stmts.iter().any(|stmt| match &stmt.kind {
            StmtKind::Expr(expr) => expr_contains_todo_kind(expr, kind),
            StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .is_some_and(|expr| expr_contains_todo_kind(expr, kind)),
            StmtKind::Assign { lhs, rhs, .. } => {
                expr_contains_todo_kind(lhs, kind) || expr_contains_todo_kind(rhs, kind)
            }
            StmtKind::While { cond, body } => {
                expr_contains_todo_kind(cond, kind)
                    || body.stmts.iter().any(|body_stmt| match &body_stmt.kind {
                        StmtKind::Expr(expr) => expr_contains_todo_kind(expr, kind),
                        StmtKind::Val(decl) => decl
                            .init
                            .as_ref()
                            .is_some_and(|expr| expr_contains_todo_kind(expr, kind)),
                        _ => false,
                    })
            }
            StmtKind::Return { value } => value
                .as_ref()
                .is_some_and(|expr| expr_contains_todo_kind(expr, kind)),
            _ => false,
        }),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_contains_todo_kind(cond, kind)
                || expr_contains_todo_kind(then_branch, kind)
                || else_branch
                    .as_deref()
                    .is_some_and(|expr| expr_contains_todo_kind(expr, kind))
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => expr_contains_todo_kind(expr, kind),
        ExprKind::Binary { lhs, rhs, .. } => {
            expr_contains_todo_kind(lhs, kind) || expr_contains_todo_kind(rhs, kind)
        }
        _ => false,
    }
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

fn lower_typed_single_source_file(sess: &Session, source: &SourceFile) -> LoweredHir {
    let mut ast = parse_file(source).unwrap();
    {
        let sources = [source];
        let mut files = [&mut ast];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            sess.sysroot(),
            &sources,
            &mut files,
        )
        .unwrap();
    }

    typecheck::check_file_headers(source, &ast).unwrap();
    typecheck::check_file_struct_decls(source, &ast).unwrap();

    let index = {
        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in sess.sysroot().index_files() {
            unit.push((&file.source, &file.ast));
        }
        unit.push((source, &ast));
        Index::build(&unit).unwrap()
    };
    let headers = crate::resolve::check_file_headers(source, &ast, &index).unwrap();
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers).unwrap();

    let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
    env.extend_from_file(source, &ast, &index).unwrap();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    typecheck::check_file_annotations(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_properties(source, &ast, &index, &env).unwrap();
    typecheck::check_file_inheritance(source, &ast, &index).unwrap();
    typecheck::check_file_interfaces(source, &ast, &index, &env).unwrap();
    typecheck::check_file_override_effects(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_where_clauses(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_overload_conflicts(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in sess.sysroot().index_files() {
        unit.push((&file.source, &file.ast));
    }
    unit.push((source, &ast));

    lower_for_compilation_unit_multi_files(source, &index, &unit, &[(source, &ast)], &[], &types)
        .unwrap()
}

fn session() -> Session {
    Session::with_options(SessionOptions::new()).unwrap()
}

fn find_fun<'a>(lowered: &'a LoweredHir, fqn: &str) -> &'a FunDecl {
    lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == fqn => Some(fun),
            _ => None,
        })
        .unwrap_or_else(|| panic!("expected HIR fun {fqn}"))
}

fn top_level_call_fqns_in_fun(fun: &FunDecl) -> Vec<String> {
    let mut call_fqns = Vec::new();
    collect_top_level_call_fqns_in_block(
        fun.body.as_ref().expect("fun should have body"),
        &mut call_fqns,
    );
    call_fqns
}

fn add_call_synth_string_arg(stmt: &Stmt) -> Option<&str> {
    let StmtKind::Expr(Expr {
        kind: ExprKind::Call { callee, args },
        ..
    }) = &stmt.kind
    else {
        return None;
    };
    let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
        return None;
    };
    if fqn != "scoop.lang.string.StringBuilder.add" {
        return None;
    }
    let Some(CallArg::Positional(Expr {
        kind: ExprKind::Literal(LiteralKind::SynthString(value)),
        ..
    })) = args.get(1)
    else {
        return None;
    };
    Some(value.as_str())
}

fn add_call_arg_is_to_string_member_call(stmt: &Stmt) -> bool {
    let StmtKind::Expr(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    }) = &stmt.kind
    else {
        return false;
    };
    let Some(CallArg::Positional(Expr {
        kind: ExprKind::Call { callee, args },
        ..
    })) = args.get(1)
    else {
        return false;
    };
    if !args.is_empty() {
        return false;
    }
    let ExprKind::MemberAccess { member, .. } = &callee.kind else {
        return false;
    };
    matches!(
        member.resolved.as_ref(),
        Some(crate::hir::MemberRef::Fun { fqn, .. })
            if fqn == "scoop.core.ToString.toString"
    )
}

#[test]
fn fstring_desugar_lowers_to_string_builder_chain() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/fstring_desugar_string_builder.scoop",
        r#"
package fixtures.fstring_desugar

fun format(x: Int): String {
    val s = f"a{x}b"
    return s
}

fun main(): Int {
    println(format(1))
    return 0
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &src);
    let format = find_fun(&lowered, "fixtures.fstring_desugar.format");
    let body = format.body.as_ref().expect("format should have body");
    let StmtKind::Val(val_decl) = &body.stmts[0].kind else {
        panic!("first statement should be val s, got {:?}", body.stmts[0]);
    };
    let Some(init) = val_decl.init.as_ref() else {
        panic!("val s should have initializer");
    };
    let ExprKind::Block(block) = &init.kind else {
        panic!("f-string should lower to a StringBuilder block, got {init:?}");
    };

    let call_fqns = top_level_call_fqns_in_fun(format);
    assert_eq!(
        call_fqns
            .iter()
            .filter(|fqn| fqn.as_str() == "scoop.lang.string.StringBuilder.add")
            .count(),
        3,
        "f-string text/expression/text parts should each call StringBuilder.add: {call_fqns:#?}"
    );
    assert_eq!(
        call_fqns
            .iter()
            .filter(|fqn| fqn.as_str() == "scoop.lang.string.StringBuilder.toString")
            .count(),
        1,
        "f-string block should finish with StringBuilder.toString: {call_fqns:#?}"
    );
    assert_eq!(add_call_synth_string_arg(&block.stmts[1]), Some("a"));
    assert!(
        add_call_arg_is_to_string_member_call(&block.stmts[2]),
        "expression part should call ToString.toString through ordinary member dispatch"
    );
    assert_eq!(add_call_synth_string_arg(&block.stmts[3]), Some("b"));
    assert!(
        lowered
            .dispatch_call_sites
            .values()
            .any(|kind| *kind == crate::hir::DispatchCallKind::Interface),
        "ToString.toString should be published as an interface dispatch call"
    );
}

#[test]
fn array_literal_desugar_array_uses_mutable_array_freeze() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/array_literal_desugar_array.scoop",
        r#"
package fixtures.array_literal_desugar

import scoop.core.*

fun main(): Int {
    val xs: Array<Int> = [1, 2, 3]
    return xs.size()
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &src);
    let main = find_fun(&lowered, "fixtures.array_literal_desugar.main");
    let call_fqns = top_level_call_fqns_in_fun(main);

    assert_eq!(
        call_fqns
            .iter()
            .filter(|fqn| fqn.as_str() == "scoop.core.mutableArrayNew")
            .count(),
        1,
        "array literal should allocate through mutableArrayNew: {call_fqns:#?}"
    );
    assert_eq!(
        call_fqns
            .iter()
            .filter(|fqn| fqn.as_str() == "scoop.core.push")
            .count(),
        3,
        "array literal should push each element through MutableArray.push: {call_fqns:#?}"
    );
    assert_eq!(
        call_fqns
            .iter()
            .filter(|fqn| fqn.as_str() == "scoop.core.freeze")
            .count(),
        1,
        "Array<T> literal should finish with MutableArray.freeze: {call_fqns:#?}"
    );
}

#[test]
fn array_literal_desugar_mutable_array_skips_freeze() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/array_literal_desugar_mutable_array.scoop",
        r#"
package fixtures.array_literal_desugar

import scoop.core.*

fun main(): Int {
    val xs: MutableArray<Int> = [1, 2, 3]
    return xs.size()
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &src);
    let main = find_fun(&lowered, "fixtures.array_literal_desugar.main");
    let call_fqns = top_level_call_fqns_in_fun(main);

    assert_eq!(
        call_fqns
            .iter()
            .filter(|fqn| fqn.as_str() == "scoop.core.mutableArrayNew")
            .count(),
        1,
        "MutableArray<T> literal should allocate through mutableArrayNew: {call_fqns:#?}"
    );
    assert_eq!(
        call_fqns
            .iter()
            .filter(|fqn| fqn.as_str() == "scoop.core.push")
            .count(),
        3,
        "MutableArray<T> literal should push each element: {call_fqns:#?}"
    );
    assert!(
        !call_fqns.iter().any(|fqn| fqn == "scoop.core.freeze"),
        "MutableArray<T> literal must not freeze: {call_fqns:#?}"
    );
}

#[test]
fn array_literal_desugar_capacity_matches_element_count() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/array_literal_desugar_capacity.scoop",
        r#"
package fixtures.array_literal_desugar

import scoop.core.*

fun main(): Int {
    val xs: Array<Int> = [1, 2, 3]
    return xs.size()
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &src);
    let main = find_fun(&lowered, "fixtures.array_literal_desugar.main");
    let main_body = main.body.as_ref().expect("main should have body");
    let new_call = find_top_level_call_in_block(main_body, "scoop.core.mutableArrayNew")
        .expect("array literal should call mutableArrayNew");
    let ExprKind::Call { args, .. } = &new_call.kind else {
        panic!("mutableArrayNew should lower to a call, got {new_call:?}");
    };
    let Some(CallArg::Positional(capacity)) = args.first() else {
        panic!("mutableArrayNew should receive a positional capacity arg: {args:?}");
    };
    assert!(
        matches!(capacity.kind, ExprKind::Literal(LiteralKind::SynthInt(3))),
        "capacity hint should match element count, got {:?}",
        capacity.kind
    );
}

#[test]
fn array_literal_desugar_empty_array_uses_zero_capacity_and_freeze() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/array_literal_desugar_empty.scoop",
        r#"
package fixtures.array_literal_desugar

import scoop.core.*

fun main(): Int {
    val xs: Array<Int> = []
    return xs.size()
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &src);
    let main = find_fun(&lowered, "fixtures.array_literal_desugar.main");
    let main_body = main.body.as_ref().expect("main should have body");
    let call_fqns = top_level_call_fqns_in_fun(main);
    assert!(
        call_fqns.iter().any(|fqn| fqn == "scoop.core.freeze"),
        "empty Array<T> literal should still freeze: {call_fqns:#?}"
    );
    let new_call = find_top_level_call_in_block(main_body, "scoop.core.mutableArrayNew")
        .expect("empty array literal should call mutableArrayNew");
    let ExprKind::Call { args, .. } = &new_call.kind else {
        panic!("mutableArrayNew should lower to a call, got {new_call:?}");
    };
    let Some(CallArg::Positional(capacity)) = args.first() else {
        panic!("mutableArrayNew should receive a positional capacity arg: {args:?}");
    };
    assert!(
        matches!(capacity.kind, ExprKind::Literal(LiteralKind::SynthInt(0))),
        "empty array literal should pass zero capacity, got {:?}",
        capacity.kind
    );
}

#[test]
fn hir_collects_scoop_extern_abi_metadata() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/hir_collects_scoop_extern_abi_metadata.scoop",
        r#"
package fixtures.hir

import scoop.core.*

@Extern("managed_echo", abi = "scoop")
fun managedEcho(value: String): String
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &src);
    let extern_fun = lowered
        .extern_funs
        .get("fixtures.hir.managedEcho")
        .expect("expected extern fun side-table entry");

    assert_eq!(extern_fun.abi, crate::hir::ExternAbi::Scoop);
    assert_eq!(extern_fun.symbol, "managed_echo");
    assert_eq!(
        extern_fun.callable_abi_identity(),
        crate::hir::CallableAbiIdentity::ManagedExtern
    );
}

#[test]
fn hir_comptime_expands_block_if_for_and_package_if() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/hir_comptime.scoop",
        r#"
package fixtures.hir_comptime

const val ENABLED: Bool = false

comptime if (true) {
    fun packageSelected(): Int { return 3 }
} else {
    fun packageSelected(): Int { return 4 }
}

fun fromBlock(): Int {
    comptime {
        return 7
    }
}

fun selectedBranch(): Int {
    comptime if (ENABLED) {
        return 1
    } else {
        return 2
    }
}

fun unrolled(): Int {
    var acc: Int = 0
    comptime for (i in 1..3) {
        acc = acc + 1
    }
    return acc
}

fun returnsIterationValue(): Int {
    comptime for (i in 1..3) {
        return i
    }
    return 0
}
"#,
    );

    let lowered = crate::pipeline::lower_typed_hir_for_dump(&sess, &src)
        .expect("HIR stage should accept expanded comptime control flow");
    let dump = format!("{:#?}", lowered.file);
    assert!(
        !dump.contains("Todo"),
        "expanded HIR must not contain Todo: {dump}"
    );
    assert!(
        !dump.contains("comptime_"),
        "expanded HIR must not retain comptime placeholder reasons: {dump}"
    );

    let selected = find_fun(&lowered, "fixtures.hir_comptime.selectedBranch");
    let selected_body = selected.body.as_ref().expect("selectedBranch has body");
    let [selected_stmt] = selected_body.stmts.as_slice() else {
        panic!(
            "selectedBranch should contain exactly the chosen return, got {:?}",
            selected_body.stmts
        );
    };
    let StmtKind::Return { value: Some(expr) } = &selected_stmt.kind else {
        panic!("selectedBranch should lower to a return, got {selected_stmt:?}");
    };
    assert_eq!(src.slice(expr.span), "2");

    let package_selected = find_fun(&lowered, "fixtures.hir_comptime.packageSelected");
    let package_body = package_selected
        .body
        .as_ref()
        .expect("packageSelected has body");
    let [package_stmt] = package_body.stmts.as_slice() else {
        panic!(
            "packageSelected should contain one selected return, got {:?}",
            package_body.stmts
        );
    };
    let StmtKind::Return { value: Some(expr) } = &package_stmt.kind else {
        panic!("packageSelected should lower to a return, got {package_stmt:?}");
    };
    assert_eq!(src.slice(expr.span), "3");

    let from_block = find_fun(&lowered, "fixtures.hir_comptime.fromBlock");
    let from_block_body = from_block.body.as_ref().expect("fromBlock has body");
    assert!(
        matches!(
            from_block_body.stmts.as_slice(),
            [Stmt {
                kind: StmtKind::Return { .. },
                ..
            }]
        ),
        "comptime block should inline its generated return: {:?}",
        from_block_body.stmts
    );

    let unrolled = find_fun(&lowered, "fixtures.hir_comptime.unrolled");
    let unrolled_body = unrolled.body.as_ref().expect("unrolled has body");
    let assign_count = unrolled_body
        .stmts
        .iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::Assign { .. }))
        .count();
    assert_eq!(assign_count, 3, "comptime for should unroll three copies");

    let returns_iteration_value = find_fun(&lowered, "fixtures.hir_comptime.returnsIterationValue");
    let returns_body = returns_iteration_value
        .body
        .as_ref()
        .expect("returnsIterationValue has body");
    let first_return = returns_body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Return { value: Some(expr) } => Some(expr),
            _ => None,
        })
        .expect("unrolled comptime for should emit returns");
    assert!(
        matches!(
            first_return.kind,
            ExprKind::Literal(LiteralKind::SynthInt(1))
        ),
        "comptime for binder should lower to a synthesized literal, got {:?}",
        first_return.kind
    );
}

#[test]
fn hir_tail_if_uses_declared_return_type_hint() {
    let sess = session();
    let src = SourceFile::new_virtual(
        "<mem>/hir_tail_if_return_expected.scoop",
        r#"
fun main(): Int {
    if (true) { 3 } else { 1 }
}
"#,
    );

    let lowered = crate::pipeline::lower_typed_hir_for_dump(&sess, &src)
        .expect("HIR stage should lower a tail if with declared return type");
    let main = find_fun(&lowered, "main");
    let body = main.body.as_ref().expect("main has body");
    assert_eq!(
        body.ty, main.return_ty,
        "body type should match function return type"
    );

    let [
        Stmt {
            kind: StmtKind::Expr(expr),
            ..
        },
    ] = body.stmts.as_slice()
    else {
        panic!(
            "main should contain a single tail expr stmt, got {:?}",
            body.stmts
        );
    };
    let ExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &expr.kind
    else {
        panic!("main tail should remain an if expr, got {expr:?}");
    };

    assert_eq!(
        expr.ty, main.return_ty,
        "if expr should inherit function return type"
    );
    assert_eq!(
        then_branch.ty, main.return_ty,
        "then branch should stay Int"
    );
    assert_eq!(
        else_branch.as_deref().expect("if has else branch").ty,
        main.return_ty,
        "else branch should stay Int"
    );
}

fn lower_typed_single_source_file_via_mir_instance_collection(
    sess: &Session,
    source: &SourceFile,
) -> LoweredHir {
    let mut ast = parse_file(source).unwrap();
    {
        let sources = [source];
        let mut files = [&mut ast];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            sess.sysroot(),
            &sources,
            &mut files,
        )
        .unwrap();
    }

    typecheck::check_file_headers(source, &ast).unwrap();
    typecheck::check_file_struct_decls(source, &ast).unwrap();

    let index = {
        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in sess.sysroot().index_files() {
            unit.push((&file.source, &file.ast));
        }
        unit.push((source, &ast));
        Index::build(&unit).unwrap()
    };
    let headers = crate::resolve::check_file_headers(source, &ast, &index).unwrap();
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers).unwrap();

    let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
    env.extend_from_file(source, &ast, &index).unwrap();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    typecheck::check_file_annotations(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_properties(source, &ast, &index, &env).unwrap();
    typecheck::check_file_inheritance(source, &ast, &index).unwrap();
    typecheck::check_file_interfaces(source, &ast, &index, &env).unwrap();
    typecheck::check_file_override_effects(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_where_clauses(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_overload_conflicts(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in sess.sysroot().index_files() {
        unit.push((&file.source, &file.ast));
    }
    unit.push((source, &ast));

    lower_for_compilation_unit_multi_files_via_mir_instance_collection(
        &index,
        &unit,
        &[(source, &ast)],
        &[],
        Some(&env),
        &types,
    )
    .unwrap()
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
    assert_eq!(
        top_level_pick_fqns.len(),
        2,
        "via-mir lowering 应为同名 generic overload 的相同 Int 实例保留两个不同的顶层实例名"
    );
    let unary_pick_fqn = top_level_pick_instances
        .iter()
        .find_map(|(fqn, param_count)| (*param_count == 1).then_some(fqn.clone()))
        .expect("应收集到 unary pick::<Int> 实例");
    let binary_pick_fqn = top_level_pick_instances
        .iter()
        .find_map(|(fqn, param_count)| (*param_count == 2).then_some(fqn.clone()))
        .expect("应收集到 binary pick::<Int> 实例");
    assert_ne!(
        unary_pick_fqn, binary_pick_fqn,
        "两个 overload 的实例名必须保持 distinct identity"
    );

    let member_pick_fqns = lowered
        .member_funs
        .iter()
        .filter(|fun| fun.fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
        .map(|fun| fun.fqn.clone())
        .collect::<HashSet<_>>();
    assert_eq!(
        member_pick_fqns.len(),
        2,
        "via-mir lowering 应为同名 generic member overload 的相同 Int 实例保留两个不同的成员实例名"
    );

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
    assert_eq!(
        direct_top_level_calls.len(),
        2,
        "main 里的 direct-call target 应区分两个 overloaded top-level generic 实例"
    );
    let direct_member_calls = main_call_fqns
        .iter()
        .filter(|fqn| fqn.starts_with("fixtures.t5000e3d.Box.pick::<Int>"))
        .cloned()
        .collect::<HashSet<_>>();
    assert_eq!(
        direct_member_calls.len(),
        2,
        "main 里的 direct-call target 应区分两个 overloaded generic member 实例"
    );

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
    assert!(
        closure_call_fqns.contains(&unary_pick_fqn),
        "函数值 closure 体应调用 unary overload 的 overload-aware 实例 target"
    );
    assert!(
        !closure_call_fqns.contains(&binary_pick_fqn),
        "函数值 closure 体不应误调用 binary overload 的实例 target"
    );
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
        top_level_instances(&lowered_b),
        "不同源码根路径下的 top-level generic overload 实例名应保持一致"
    );
    assert_eq!(
        member_instances(&lowered_a),
        member_instances(&lowered_b),
        "不同源码根路径下的 generic member overload 实例名应保持一致"
    );
    assert_eq!(
        main_direct_targets(&lowered_a),
        main_direct_targets(&lowered_b),
        "不同源码根路径下的 overload-aware direct-call target 应保持一致"
    );
}

#[test]
fn typed_hir_dump_keeps_generic_direct_calls_as_template_targets() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t5000e3d_typed_dump>",
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

    let lowered = lower_typed_for_dump(&sess, &source).unwrap();

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
        find_top_level_call_in_block(main_body, "fixtures.t5000e3d.id").is_some(),
        "typed dump 仍应保留 standalone generic direct-call 的 template target"
    );
    assert!(
        find_top_level_call_in_block(main_body, "fixtures.t5000e3d.id::<Int>").is_none(),
        "typed dump 不应提前把 standalone generic direct-call 物化为实例目标"
    );

    let member_fun = lowered
        .member_funs
        .iter()
        .find(|fun| fun.fqn == "fixtures.t5000e3d.Box.memberId")
        .expect("typed dump 应保留 generic member template");
    let member_body = member_fun
        .body
        .as_ref()
        .expect("memberId template 应有 body");
    assert!(
        find_top_level_call_in_block(member_body, "fixtures.t5000e3d.id").is_some(),
        "typed dump 中 generic member template 体内的 direct-call 仍应指向 template target"
    );
    assert!(
        find_top_level_call_in_block(member_body, "fixtures.t5000e3d.id::<Int>").is_none(),
        "typed dump 不应提前把 generic member template 体内的 direct-call 物化为实例目标"
    );
}

#[test]
fn lower_typed_single_source_file_preserves_inferred_fun_return_types() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t5000e3c_inferred_return_ty>",
        r#"
package fixtures.t5000e3c

import scoop.core.*

class Box {
    fun value() { 1 }
}

fun helper() { 1 }

fun main() {}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);

    let helper = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t5000e3c.helper" => Some(fun),
            _ => None,
        })
        .expect("应收集到 helper");
    assert_eq!(lowered.types.display(helper.return_ty).to_string(), "Int");

    let main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t5000e3c.main" => Some(fun),
            _ => None,
        })
        .expect("应收集到 main");
    assert_eq!(lowered.types.display(main.return_ty).to_string(), "Unit");

    let value_method = lowered
        .member_funs
        .iter()
        .find(|fun| fun.fqn == "fixtures.t5000e3c.Box.value")
        .expect("应收集到 Box.value");
    assert_eq!(
        lowered.types.display(value_method.return_ty).to_string(),
        "Int"
    );
}

#[test]
fn lower_typed_single_source_file_expands_irrefutable_top_level_pattern_into_hidden_subject() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t4004b>",
        r#"
package fixtures.t4004b

import scoop.core.*

val (left, right) = (7, 9)

fun main(): Int {
    return left + right
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);
    let left_fqn = "fixtures.t4004b.left";
    let top_level_value_names = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Val(val) => val.name.as_deref(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(top_level_value_names.contains(&"left"));
    assert!(top_level_value_names.contains(&"right"));

    let hidden_subjects = lowered
        .top_level_immutable_values
        .keys()
        .filter(|fqn| fqn.contains("__top_level_pattern_") && fqn.ends_with("__subject"))
        .cloned()
        .collect::<Vec<_>>();
    let hidden_checks = lowered
        .top_level_immutable_values
        .keys()
        .filter(|fqn| fqn.contains("__top_level_pattern_") && fqn.ends_with("__check"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(hidden_subjects.len(), 1);
    assert_eq!(hidden_checks.len(), 0);
    let hidden_subject = &hidden_subjects[0];

    let left = lowered
        .top_level_immutable_values
        .get(left_fqn)
        .expect("应收集到可见 binder left 的顶层 immutable value 记录");
    let init = left.init.as_ref().expect("binder 应带 initializer");
    let ExprKind::MemberAccess { receiver, member } = &init.kind else {
        panic!("irrefutable tuple binder initializer 应直接做成员访问提取");
    };
    assert!(
        matches!(
            &receiver.kind,
            ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) if fqn == hidden_subject
        ),
        "tuple binder 提取应复用隐藏 subject 顶层值"
    );
    assert_eq!(member.name, "_0");
}

fn assert_raise_runtime_error_effect_ty(types: &TypeStore, effect_ty: TypeId) {
    let TypeKind::Ref(RefTypeKind::Nominal(effect_nominal)) = types.kind(effect_ty) else {
        panic!(
            "effect_ty 应为 effect nominal，实际为 {:?}",
            types.kind(effect_ty)
        );
    };
    assert_eq!(effect_nominal.fqn, "scoop.core.Raise");
    assert_eq!(effect_nominal.args.len(), 1);

    match types.kind(effect_nominal.args[0]) {
        TypeKind::Ref(RefTypeKind::Nominal(arg_nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(arg_nominal)) => {
            assert_eq!(arg_nominal.fqn, "scoop.core.RuntimeError");
        }
        other => panic!("Raise 的类型实参应为 RuntimeError，实际为 {:?}", other),
    }
}

fn assert_raise_int_effect_ty(types: &TypeStore, effect_ty: TypeId) {
    let TypeKind::Ref(RefTypeKind::Nominal(effect_nominal)) = types.kind(effect_ty) else {
        panic!(
            "effect_ty 应为 effect nominal，实际为 {:?}",
            types.kind(effect_ty)
        );
    };
    assert_eq!(effect_nominal.fqn, "scoop.core.Raise");
    assert_eq!(effect_nominal.args.len(), 1);
    assert!(
        matches!(
            types.kind(effect_nominal.args[0]),
            TypeKind::Value(ValueTypeKind::Int)
        ),
        "Raise 的类型实参应为 Int，实际为 {:?}",
        types.kind(effect_nominal.args[0])
    );
}

fn find_raise_perform_effect_ty_in_block(block: &Block) -> Option<TypeId> {
    block
        .stmts
        .iter()
        .find_map(find_raise_perform_effect_ty_in_stmt)
}

fn find_raise_perform_effect_ty_in_stmt(stmt: &Stmt) -> Option<TypeId> {
    match &stmt.kind {
        StmtKind::Expr(expr) => find_raise_perform_effect_ty_in_expr(expr),
        StmtKind::Val(val) => val
            .init
            .as_ref()
            .and_then(find_raise_perform_effect_ty_in_expr),
        StmtKind::Assign { lhs, rhs, .. } => find_raise_perform_effect_ty_in_expr(lhs)
            .or_else(|| find_raise_perform_effect_ty_in_expr(rhs)),
        StmtKind::While { cond, body } => find_raise_perform_effect_ty_in_expr(cond)
            .or_else(|| find_raise_perform_effect_ty_in_block(body)),
        StmtKind::Return { value } => value
            .as_ref()
            .and_then(find_raise_perform_effect_ty_in_expr),
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => None,
    }
}

fn find_raise_perform_effect_ty_in_expr(expr: &Expr) -> Option<TypeId> {
    match &expr.kind {
        ExprKind::Perform { effect_ty, op, .. } if op.fqn == "scoop.core.Raise.raise" => {
            Some(*effect_ty)
        }
        ExprKind::Call { callee, args } => {
            find_raise_perform_effect_ty_in_expr(callee).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    CallArg::Positional(expr) => find_raise_perform_effect_ty_in_expr(expr),
                    CallArg::Named { value, .. } => find_raise_perform_effect_ty_in_expr(value),
                })
            })
        }
        ExprKind::MemberAccess { receiver, .. } => find_raise_perform_effect_ty_in_expr(receiver),
        ExprKind::When { subject, arms } => {
            find_raise_perform_effect_ty_in_expr(subject).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(find_raise_perform_effect_ty_in_expr)
                        .or_else(|| find_raise_perform_effect_ty_in_expr(&arm.body))
                })
            })
        }
        ExprKind::Block(block) => find_raise_perform_effect_ty_in_block(block),
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => find_raise_perform_effect_ty_in_expr(cond)
            .or_else(|| find_raise_perform_effect_ty_in_expr(then_branch))
            .or_else(|| {
                else_branch
                    .as_deref()
                    .and_then(find_raise_perform_effect_ty_in_expr)
            }),
        ExprKind::Unary { expr, .. }
        | ExprKind::TypeCheck { expr, .. }
        | ExprKind::Cast { expr, .. } => find_raise_perform_effect_ty_in_expr(expr),
        ExprKind::Binary { lhs, rhs, .. } => find_raise_perform_effect_ty_in_expr(lhs)
            .or_else(|| find_raise_perform_effect_ty_in_expr(rhs)),
        ExprKind::StructLit { fields, .. } => fields
            .iter()
            .find_map(|field| find_raise_perform_effect_ty_in_expr(&field.value)),
        ExprKind::TupleLit { elements } => elements
            .iter()
            .find_map(find_raise_perform_effect_ty_in_expr),
        ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| match part {
            crate::hir::InterpolatedStringPart::Expr { expr } => {
                find_raise_perform_effect_ty_in_expr(expr)
            }
            crate::hir::InterpolatedStringPart::Text { .. } => None,
        }),
        ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
            CallArg::Positional(expr) => find_raise_perform_effect_ty_in_expr(expr),
            CallArg::Named { value, .. } => find_raise_perform_effect_ty_in_expr(value),
        }),
        ExprKind::Handle(handle) => find_raise_perform_effect_ty_in_block(&handle.body)
            .or_else(|| {
                handle
                    .arms
                    .iter()
                    .find_map(|arm| find_raise_perform_effect_ty_in_expr(&arm.body))
            })
            .or_else(|| {
                handle
                    .finally
                    .as_ref()
                    .and_then(find_raise_perform_effect_ty_in_block)
            }),
        ExprKind::Literal(_)
        | ExprKind::VarRef(_)
        | ExprKind::UnresolvedIdent { .. }
        | ExprKind::ClassLiteral(_)
        | ExprKind::Closure(_)
        | ExprKind::Missing
        | ExprKind::Todo(_) => None,
    }
}

#[test]
fn lower_minimal_file_smoke() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual("<mem>", "package a\nfun main() { val x: Int = 1; x }");

    let lowered = lower_for_dump(&sess, &src).unwrap();
    assert!(!lowered.file.items.is_empty());
}

#[test]
fn hir_fixture_minimal_golden() {
    let sess = Session::new().unwrap();

    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hir/minimal.scoop");
    let file = SourceFile::load(&fixture_path).unwrap();

    let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
    let actual = output.stable_dump();

    let golden_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hir/minimal.hir");
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn hir_fixture_handle_perform_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/handle_perform.scoop");
    let file = SourceFile::load(&fixture_path).unwrap();

    let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
    let actual = output.stable_dump();

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/handle_perform.hir");
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn hir_fixture_control_flow_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/control_flow.scoop");
    let file = SourceFile::load(&fixture_path).unwrap();

    let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
    let actual = output.stable_dump();

    let golden_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hir/control_flow.hir");
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn hir_fixture_member_access_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/member_access.scoop");
    let file = SourceFile::load(&fixture_path).unwrap();

    let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
    let actual = output.stable_dump();

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/member_access.hir");
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn hir_fixture_closure_non_capture_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/closure_non_capture.scoop");
    let file = SourceFile::load(&fixture_path).unwrap();

    let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
    let actual = output.stable_dump();

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/closure_non_capture.hir");
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn hir_fixture_closure_capture_val_golden() {
    let sess = Session::new().unwrap();

    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/closure_capture_val.scoop");
    let file = SourceFile::load(&fixture_path).unwrap();

    let output = crate::pipeline::load_typed_hir_stage_output_for_dump(&sess, &file).unwrap();
    let actual = output.stable_dump();

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/hir/closure_capture_val.hir");
    let expected = std::fs::read_to_string(&golden_path).unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn lower_float_literals_to_typed_hir_literals() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>",
        "package a\nfun main() { val a = 2.75; val b = 0.5f; val c = 1e3 }\n",
    );

    let lowered = lower_for_dump(&sess, &src).unwrap();
    let Item::Fun(fun) = &lowered.file.items[0] else {
        panic!("期望第一个 item 为函数");
    };
    let body = fun.body.as_ref().expect("main 应有函数体");
    let [stmt_a, stmt_b, stmt_c] = body.stmts.as_slice() else {
        panic!("期望 main 中包含三个 val 语句");
    };

    let StmtKind::Val(val_a) = &stmt_a.kind else {
        panic!("第一个语句应为 val");
    };
    let init_a = val_a.init.as_ref().expect("a 应有 initializer");
    assert!(matches!(
        init_a.kind,
        ExprKind::Literal(LiteralKind::Float64(value)) if value == 2.75
    ));
    assert_eq!(lowered.types.display(init_a.ty).to_string(), "Float64");

    let StmtKind::Val(val_b) = &stmt_b.kind else {
        panic!("第二个语句应为 val");
    };
    let init_b = val_b.init.as_ref().expect("b 应有 initializer");
    assert!(matches!(
        init_b.kind,
        ExprKind::Literal(LiteralKind::Float32(value)) if value == 0.5
    ));
    assert_eq!(lowered.types.display(init_b.ty).to_string(), "Float32");

    let StmtKind::Val(val_c) = &stmt_c.kind else {
        panic!("第三个语句应为 val");
    };
    let init_c = val_c.init.as_ref().expect("c 应有 initializer");
    assert!(matches!(
        init_c.kind,
        ExprKind::Literal(LiteralKind::Float64(value)) if value == 1000.0
    ));
    assert_eq!(lowered.types.display(init_c.ty).to_string(), "Float64");
}

#[test]
fn lower_for_compilation_unit_multi_files_includes_non_entry_top_level_funs() {
    let sess = Session::new().unwrap();

    let src_lib = SourceFile::new_virtual(
        "<lib>",
        r#"
package fixtures.t1315a

import scoop.core.*

fun id(x: Int): Int { return x }
"#,
    );
    let src_main = SourceFile::new_virtual(
        "<main>",
        r#"
package fixtures.t1315a

import scoop.core.*

fun main(): Int { return id(1) }
"#,
    );

    let mut ast_lib = parse_file(&src_lib).unwrap();
    let mut ast_main = parse_file(&src_main).unwrap();

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((&src_lib, &ast_lib));
        pairs.push((&src_main, &ast_main));
        Index::build(&pairs).unwrap()
    };

    let h_lib = crate::resolve::check_file_headers(&src_lib, &ast_lib, &index).unwrap();
    crate::resolve::check_file_bodies(&src_lib, &mut ast_lib, &index, &h_lib).unwrap();

    let h_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
    crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &h_main).unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in sess.sysroot().index_files() {
        unit.push((&f.source, &f.ast));
    }
    unit.push((&src_lib, &ast_lib));
    unit.push((&src_main, &ast_main));

    let files_to_lower = vec![(&src_lib, &ast_lib), (&src_main, &ast_main)];
    let empty_types = TypeStore::new();
    let lowered = lower_for_compilation_unit_multi_files(
        &src_main,
        &index,
        &unit,
        &files_to_lower,
        &[],
        &empty_types,
    )
    .unwrap();

    let fun_fqns: HashSet<&str> = lowered
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect();

    assert!(fun_fqns.contains("fixtures.t1315a.id"));
    assert!(fun_fqns.contains("fixtures.t1315a.main"));
}

#[test]
fn lower_for_compilation_unit_multi_files_preserves_non_entry_call_site_side_tables() {
    let sess = Session::new().unwrap();

    let src_helper = SourceFile::new_virtual(
        "<t4006r_helper>",
        r#"
package fixtures.t4006r

import scoop.core.*

class Box(val x: Int = 10, val y: Int = 20)

public fun helper_sum(): Int {
    val box = Box(y = 32)
    return box.x + box.y
}

public fun resume_once(k: Continuation<Int, Unit, eff Pure>): Unit / Raise<RuntimeError> {
    k.resume(1)
}
"#,
    );
    let src_main = SourceFile::new_virtual(
        "<t4006r_main>",
        r#"
package fixtures.t4006r

import scoop.core.*

fun main(): Int {
    return helper_sum()
}
"#,
    );

    let mut ast_helper = parse_file(&src_helper).unwrap();
    let mut ast_main = parse_file(&src_main).unwrap();

    typecheck::check_file_headers(&src_helper, &ast_helper).unwrap();
    typecheck::check_file_struct_decls(&src_helper, &ast_helper).unwrap();
    typecheck::check_file_headers(&src_main, &ast_main).unwrap();
    typecheck::check_file_struct_decls(&src_main, &ast_main).unwrap();

    let index = {
        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&src_helper, &ast_helper));
        unit.push((&src_main, &ast_main));
        Index::build(&unit).unwrap()
    };

    let headers_helper =
        crate::resolve::check_file_headers(&src_helper, &ast_helper, &index).unwrap();
    crate::resolve::check_file_bodies(&src_helper, &mut ast_helper, &index, &headers_helper)
        .unwrap();

    let headers_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
    crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

    let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
    env.extend_from_file(&src_helper, &ast_helper, &index)
        .unwrap();
    env.extend_from_file(&src_main, &ast_main, &index).unwrap();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    for (source, ast, headers) in [
        (&src_helper, &ast_helper, &headers_helper),
        (&src_main, &ast_main, &headers_main),
    ] {
        typecheck::check_file_annotations(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_properties(source, ast, &index, &env).unwrap();
        typecheck::check_file_inheritance(source, ast, &index).unwrap();
        typecheck::check_file_interfaces(source, ast, &index, &env).unwrap();
        typecheck::check_file_override_effects(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_where_clauses(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_overload_conflicts(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
    }

    typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in sess.sysroot().index_files() {
        unit.push((&f.source, &f.ast));
    }
    unit.push((&src_helper, &ast_helper));
    unit.push((&src_main, &ast_main));

    let lowered = lower_for_compilation_unit_multi_files(
        &src_main,
        &index,
        &unit,
        &[(&src_helper, &ast_helper), (&src_main, &ast_main)],
        &[],
        &types,
    )
    .unwrap();

    let helper_sum = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t4006r.helper_sum" => Some(fun),
            _ => None,
        })
        .expect("应收集到 helper_sum");
    let helper_sum_body = helper_sum.body.as_ref().expect("helper_sum 应有 body");
    fn find_call_span_in_expr(expr: &Expr) -> Option<Span> {
        if let ExprKind::Call { .. } = &expr.kind {
            return Some(expr.span);
        }
        if let ExprKind::Block(block) = &expr.kind {
            return block.stmts.iter().find_map(find_call_span_in_stmt);
        }
        None
    }

    fn find_call_span_in_stmt(stmt: &Stmt) -> Option<Span> {
        match &stmt.kind {
            StmtKind::Expr(expr) => find_call_span_in_expr(expr),
            StmtKind::Val(val_decl) => val_decl.init.as_ref().and_then(find_call_span_in_expr),
            StmtKind::Return { value } => value.as_ref().and_then(find_call_span_in_expr),
            _ => None,
        }
    }

    let ctor_call_span = helper_sum_body
        .stmts
        .iter()
        .find_map(find_call_span_in_stmt)
        .expect("helper_sum 应包含 ctor call initializer");
    assert!(
        lowered.ctor_call_sites.contains_key(&CallSite::new(
            src_helper.path().to_path_buf(),
            ctor_call_span,
        )),
        "非入口文件中的 ctor 调用点应保留在 lowering side table 中"
    );

    let resume_once = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t4006r.resume_once" => Some(fun),
            _ => None,
        })
        .expect("应收集到 resume_once");
    assert_eq!(
        lowered.types.display(resume_once.params[0].ty).to_string(),
        "scoop.core.Continuation<Int, Unit, eff Pure>"
    );
    let resume_body = resume_once.body.as_ref().expect("resume_once 应有 body");
    let resume_span = match resume_body.stmts.as_slice() {
        [
            Stmt {
                kind: StmtKind::Expr(expr),
                ..
            },
        ] => match &expr.kind {
            ExprKind::Call { .. } => expr.span,
            other => panic!("resume_once 的唯一语句应为 call expr，实际为 {:?}", other),
        },
        stmts => panic!("resume_once body 形态不符合预期: {:?}", stmts),
    };
    assert!(
        lowered
            .continuation_resume_call_sites
            .contains(&CallSite::new(src_helper.path().to_path_buf(), resume_span,)),
        "非入口文件中的 Continuation.resume 调用点应保留在 lowering side table 中"
    );
}

#[test]
fn lower_for_compilation_unit_multi_files_preserves_safe_member_access_resolution() {
    let sess = Session::new().unwrap();
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop");
    let source = SourceFile::load(&fixture_path).unwrap();
    let mut ast = parse_file(&source).unwrap();
    {
        let sources = [&source];
        let mut files = [&mut ast];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            sess.sysroot(),
            &sources,
            &mut files,
        )
        .unwrap();
    }

    typecheck::check_file_headers(&source, &ast).unwrap();
    typecheck::check_file_struct_decls(&source, &ast).unwrap();

    let index = {
        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&source, &ast));
        Index::build(&unit).unwrap()
    };
    let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
    crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

    let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
    env.extend_from_file(&source, &ast, &index).unwrap();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    typecheck::check_file_annotations(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_properties(&source, &ast, &index, &env).unwrap();
    typecheck::check_file_inheritance(&source, &ast, &index).unwrap();
    typecheck::check_file_interfaces(&source, &ast, &index, &env).unwrap();
    typecheck::check_file_override_effects(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_refs(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_where_clauses(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_overload_conflicts(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_exprs(
        &source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

    let safe_debug = format!("{:?}", ast.safe_member_access_resolved.borrow());
    assert!(safe_debug.contains("User.score"), "{safe_debug}");
    assert!(safe_debug.contains("Config.port"), "{safe_debug}");
    assert!(safe_debug.contains("doubleScore"), "{safe_debug}");

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in sess.sysroot().index_files() {
        unit.push((&f.source, &f.ast));
    }
    unit.push((&source, &ast));

    let lowered = lower_for_compilation_unit_multi_files(
        &source,
        &index,
        &unit,
        &[(&source, &ast)],
        &[],
        &types,
    )
    .unwrap();

    let mut unresolved_member_names = Vec::new();
    let mut top_level_call_fqns = Vec::new();
    for item in &lowered.file.items {
        if let Item::Fun(fun) = item
            && let Some(body) = fun.body.as_ref()
        {
            collect_unresolved_member_names_in_block(body, &mut unresolved_member_names);
            collect_top_level_call_fqns_in_block(body, &mut top_level_call_fqns);
        }
    }

    assert!(!unresolved_member_names.iter().any(|name| name == "score"));
    assert!(!unresolved_member_names.iter().any(|name| name == "port"));
    assert!(top_level_call_fqns.iter().any(|fqn| fqn == "doubleScore"));
}

#[test]
fn lower_typed_single_source_file_preserves_chained_member_access_resolution() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t4006v>",
        r#"
package fixtures.t4006v

import scoop.core.*

struct Tag(val label: String, val score: Int)

class Node(val name: String, val tag: Tag, val value: Int)

class Holder(val node: Node)

fun main(): Int {
    val holder: Holder = Holder(Node("root", Tag { label: "alpha", score: 7 }, 42))
    val label: String = holder.node.tag.label
    println(label)
    return holder.node.tag.score
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);
    let mut unresolved_member_names = Vec::new();
    for item in &lowered.file.items {
        if let Item::Fun(fun) = item
            && let Some(body) = fun.body.as_ref()
        {
            collect_unresolved_member_names_in_block(body, &mut unresolved_member_names);
        }
    }

    assert!(
        !unresolved_member_names.iter().any(|name| name == "label"),
        "{unresolved_member_names:?}"
    );
    assert!(
        !unresolved_member_names.iter().any(|name| name == "score"),
        "{unresolved_member_names:?}"
    );
}

#[test]
fn lower_typed_single_source_file_expands_with_update_over_tuple_nested_paths() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t4010a1>",
        r#"
package fixtures.t4010a1

import scoop.core.*

struct Point(val x: Int, val y: Int)

fun use(pair: (Point, (Int, Int))) {
    val updated: (Point, (Int, Int)) = pair with { _0.x: 10, _1._0: 30 }
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t4010a1.use" => Some(fun),
            _ => None,
        })
        .expect("expected lowered function");
    let body = fun.body.as_ref().expect("expected function body");
    let updated_init = body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Val(decl) if decl.name.as_deref() == Some("updated") => decl.init.as_ref(),
            _ => None,
        })
        .expect("expected updated init");

    assert!(
        !expr_contains_todo_kind(updated_init, "with_update"),
        "with-update lowering should not fall back to Todo: {updated_init:#?}"
    );

    let ExprKind::Block(block) = &updated_init.kind else {
        panic!("with-update init should lower to block: {updated_init:#?}");
    };
    assert_eq!(block.stmts.len(), 4, "{block:#?}");

    let StmtKind::Val(base_decl) = &block.stmts[0].kind else {
        panic!(
            "first with-update stmt should bind synthetic base: {:#?}",
            block.stmts[0]
        );
    };
    assert_eq!(base_decl.name.as_deref(), Some("$with_base"));

    let StmtKind::Expr(rebuilt_expr) = &block.stmts[3].kind else {
        panic!(
            "second with-update stmt should be rebuilt value: {:#?}",
            block.stmts[3]
        );
    };

    let ExprKind::TupleLit { elements } = &rebuilt_expr.kind else {
        panic!("with-update over tuple should rebuild tuple literal: {rebuilt_expr:#?}");
    };
    assert_eq!(elements.len(), 2);

    let ExprKind::StructLit { fields, .. } = &elements[0].kind else {
        panic!(
            "first tuple element should rebuild nested struct: {:#?}",
            elements[0]
        );
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "x");
    assert!(matches!(
        &fields[0].value.kind,
        ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
    ));
    assert_eq!(fields[1].name, "y");
    assert!(matches!(
        fields[1].value.kind,
        ExprKind::MemberAccess { .. }
    ));

    let ExprKind::TupleLit {
        elements: nested_tuple,
    } = &elements[1].kind
    else {
        panic!(
            "second tuple element should rebuild nested tuple: {:#?}",
            elements[1]
        );
    };
    assert_eq!(nested_tuple.len(), 2);
    assert!(matches!(
        &nested_tuple[0].kind,
        ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
    ));
    assert!(matches!(
        nested_tuple[1].kind,
        ExprKind::MemberAccess { .. }
    ));
}

#[test]
fn lower_typed_single_source_file_expands_with_update_over_enum_payload_paths() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t4010a2b>",
        r#"
package fixtures.t4010a2b

import scoop.core.*

struct Point(val x: Int, val y: Int)

enum Result {
    Ok(val point: Point),
    Err(val code: Int),
}

fun use(r: Result) {
    val updated: Result = r with { Ok.point.x: 10, Err.code: 20 }
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);
    let fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t4010a2b.use" => Some(fun),
            _ => None,
        })
        .expect("expected lowered function");
    let body = fun.body.as_ref().expect("expected function body");
    let updated_init = body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Val(decl) if decl.name.as_deref() == Some("updated") => decl.init.as_ref(),
            _ => None,
        })
        .expect("expected updated init");

    assert!(
        !expr_contains_todo_kind(updated_init, "with_update"),
        "enum with-update lowering should not fall back to Todo: {updated_init:#?}"
    );

    let ExprKind::Block(block) = &updated_init.kind else {
        panic!("enum with-update init should lower to block: {updated_init:#?}");
    };
    assert_eq!(block.stmts.len(), 4, "{block:#?}");

    let StmtKind::Expr(rebuilt_expr) = &block.stmts[3].kind else {
        panic!(
            "second with-update stmt should be rebuilt value: {:#?}",
            block.stmts[3]
        );
    };

    let ExprKind::When { subject, arms } = &rebuilt_expr.kind else {
        panic!("enum with-update should rebuild through when: {rebuilt_expr:#?}");
    };
    assert!(matches!(
        &subject.kind,
        ExprKind::VarRef(ValueRef::Local { name, .. }) if name == "$with_base"
    ));
    assert_eq!(arms.len(), 2);

    let ok_arm = arms
        .iter()
        .find(|arm| matches!(&arm.pat, WhenPat::Variant { name, .. } if name == "Ok"))
        .expect("expected Ok arm");
    let ExprKind::Call { callee, args } = &ok_arm.body.kind else {
        panic!("Ok arm should rebuild variant ctor: {:#?}", ok_arm.body);
    };
    assert!(matches!(
        &callee.kind,
        ExprKind::UnresolvedIdent { name } if name == "Ok"
    ));
    assert_eq!(args.len(), 1);
    let CallArg::Positional(ok_payload) = &args[0] else {
        panic!("Ok arm payload should be positional: {:#?}", args[0]);
    };
    let ExprKind::StructLit { fields, .. } = &ok_payload.kind else {
        panic!("Ok payload should rebuild nested Point: {ok_payload:#?}");
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "x");
    assert!(matches!(
        &fields[0].value.kind,
        ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
    ));
    assert_eq!(fields[1].name, "y");
    assert!(matches!(
        fields[1].value.kind,
        ExprKind::MemberAccess { .. }
    ));

    let err_arm = arms
        .iter()
        .find(|arm| matches!(&arm.pat, WhenPat::Variant { name, .. } if name == "Err"))
        .expect("expected Err arm");
    let ExprKind::Call { callee, args } = &err_arm.body.kind else {
        panic!("Err arm should rebuild variant ctor: {:#?}", err_arm.body);
    };
    assert!(matches!(
        &callee.kind,
        ExprKind::UnresolvedIdent { name } if name == "Err"
    ));
    assert_eq!(args.len(), 1);
    let CallArg::Positional(err_payload) = &args[0] else {
        panic!("Err arm payload should be positional: {:#?}", args[0]);
    };
    assert!(matches!(
        &err_payload.kind,
        ExprKind::VarRef(ValueRef::Local { name, .. }) if name.starts_with("__with_update_value")
    ));
}

#[test]
fn lower_typed_single_source_file_rewrites_value_computed_property_access_to_getter_call() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t4010b1>",
        r#"
package fixtures.t4010b1

import scoop.core.*

struct Point(val x: Int) {
    val doubled: Int
        get() = this.x * 2
}

fun use() {
    val result: Int = Point(3).doubled
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);

    let find_result_init = |fun_fqn: &str| {
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == fun_fqn => Some(fun),
                _ => None,
            })
            .expect("expected lowered function");
        let body = fun.body.as_ref().expect("expected function body");
        body.stmts
            .iter()
            .find_map(|stmt| match &stmt.kind {
                StmtKind::Val(decl) if decl.name.as_deref() == Some("result") => decl.init.as_ref(),
                _ => None,
            })
            .expect("expected result initializer")
    };

    let point_result_init = find_result_init("fixtures.t4010b1.use");
    let ExprKind::Call {
        callee: point_callee,
        args: point_args,
    } = &point_result_init.kind
    else {
        panic!(
            "value computed property access should lower to getter call: {point_result_init:#?}"
        );
    };
    let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &point_callee.kind else {
        panic!("getter callee should be top-level value ref: {point_callee:#?}");
    };
    assert_eq!(fqn, "fixtures.t4010b1.Point.doubled");
    assert_eq!(point_args.len(), 1);

    assert!(
        lowered
            .member_funs
            .iter()
            .any(|fun| fun.fqn == "fixtures.t4010b1.Point.doubled"),
        "expected non-generic computed property getter to be collected as member callable"
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

#[test]
fn lower_for_compilation_unit_multi_files_preserves_effect_ty_in_class_init_side_tables() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t3014b>",
        r#"
package fixtures.t3014b

import scoop.core.*

class BoomClass() {
    val x: Int = Raise.raise(RuntimeError.NullAssertionFailed)
}

fun main(): Int { return 0 }
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);

    let class_init = lowered
        .class_inits
        .get("fixtures.t3014b.BoomClass")
        .expect("应收集到 BoomClass 的 class init");
    let class_property_init = class_init
        .steps
        .iter()
        .find_map(|step| match step {
            ClassInitStep::PropertyInit { field_fqn, init }
                if field_fqn == "fixtures.t3014b.BoomClass.x" =>
            {
                Some(init)
            }
            _ => None,
        })
        .expect("BoomClass.x 应存在 property initializer");
    let class_effect_ty = match &class_property_init.kind {
        ExprKind::Perform { effect_ty, op, .. } => {
            assert_eq!(op.fqn, "scoop.core.Raise.raise");
            *effect_ty
        }
        other => panic!(
            "BoomClass.x initializer 应 lower 为 Perform，实际为 {:?}",
            other
        ),
    };
    assert_raise_runtime_error_effect_ty(&lowered.types, class_effect_ty);
}

#[test]
fn lower_typed_single_source_file_preserves_effect_ty_in_observable_delegate_callback() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t3014c>",
        r#"
package fixtures.t3014c

import scoop.core.*
import scoop.delegates.*

class Counter() {
    var x: Int by observable(0) { old, new ->
        if (new == 1) {
            Raise.raise(7)
        }
        println(old)
    }
}

fun main(): Int {
    val counter: Counter = Counter()
    try {
        counter.x = 1
    } catch (e: Int) {
        println(e)
    }
    return 0
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);
    let main_fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t3014c.main" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.t3014c.main");
    let main_body = main_fun.body.as_ref().expect("main 应有 body");
    let effect_ty = find_raise_perform_effect_ty_in_block(main_body)
        .expect("observable callback 内的 Raise.raise 应被 lower 为带 effect_ty 的 Perform");
    assert_raise_int_effect_ty(&lowered.types, effect_ty);
}

#[test]
fn lower_typed_single_source_file_records_statement_position_continuation_resume_call_site() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t3016c0_stmt_resume>",
        r#"
package fixtures.t3016c0

import scoop.core.*

fun run(k: Continuation<Int, Unit, eff Pure>): Unit / Raise<RuntimeError> {
    k.resume(1)
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);
    let run_fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t3016c0.run" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.t3016c0.run");
    assert_eq!(
        lowered.types.display(run_fun.params[0].ty).to_string(),
        "scoop.core.Continuation<Int, Unit, eff Pure>"
    );
    let body = run_fun.body.as_ref().expect("run 应有 body");
    let resume_span = match body.stmts.as_slice() {
        [
            Stmt {
                kind: StmtKind::Expr(expr),
                ..
            },
        ] => match &expr.kind {
            ExprKind::Call { .. } => expr.span,
            other => panic!("run 的唯一语句应为 call expr，实际为 {:?}", other),
        },
        stmts => panic!("run body 语句数不符合预期: {:?}", stmts),
    };

    assert_eq!(lowered.continuation_resume_call_sites.len(), 1);
    assert!(
        lowered
            .continuation_resume_call_sites
            .contains(&CallSite::new(source.path().to_path_buf(), resume_span)),
        "statement-position `Continuation.resume(...)` 应写入 continuation_resume_call_sites"
    );
}

#[test]
fn lower_typed_single_source_file_does_not_record_effect_op_named_resume_as_builtin_call_site() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<t3016c0r_effect_op_resume>",
        r#"
package fixtures.t3016c0r

import scoop.core.*

effect Echo {
    fun resume(value: Int): Unit
}

fun run(): Unit / Echo {
    Echo.resume(1)
}
"#,
    );

    let lowered = lower_typed_single_source_file(&sess, &source);
    let run_fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t3016c0r.run" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.t3016c0r.run");
    let body = run_fun.body.as_ref().expect("run 应有 body");
    match body.stmts.as_slice() {
        [
            Stmt {
                kind: StmtKind::Expr(expr),
                ..
            },
        ] => match &expr.kind {
            ExprKind::Perform { .. } => {}
            other => panic!("effect op `resume` 应 lower 为 Perform，实际为 {:?}", other),
        },
        stmts => panic!("run body 语句数不符合预期: {:?}", stmts),
    }

    assert!(
        lowered.continuation_resume_call_sites.is_empty(),
        "effect op `resume` 不应污染 continuation_resume_call_sites"
    );
}

#[test]
fn lower_for_compilation_unit_multi_files_preserves_effect_ty_in_cross_file_observable_delegate_callback()
 {
    let sess = Session::new().unwrap();
    let src_model = SourceFile::new_virtual(
        "<t3014cr_model>",
        r#"
package fixtures.t3014cr

import scoop.core.*
import scoop.delegates.*

class Counter() {
    var x: Int by observable(0) { old, new ->
        if (new == 1) {
            Raise.raise(7)
        }
        println(old)
    }
}
"#,
    );
    let src_main = SourceFile::new_virtual(
        "<t3014cr_main>",
        r#"
package fixtures.t3014cr

import scoop.core.*

fun main(): Int {
    val counter: Counter = Counter()
    try {
        counter.x = 1
    } catch (e: Int) {
        println(e)
    }
    return 0
}
"#,
    );

    let mut ast_model = parse_file(&src_model).unwrap();
    let mut ast_main = parse_file(&src_main).unwrap();

    let index = {
        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in sess.sysroot().index_files() {
            unit.push((&f.source, &f.ast));
        }
        unit.push((&src_model, &ast_model));
        unit.push((&src_main, &ast_main));
        Index::build(&unit).unwrap()
    };

    let headers_model = crate::resolve::check_file_headers(&src_model, &ast_model, &index).unwrap();
    crate::resolve::check_file_bodies(&src_model, &mut ast_model, &index, &headers_model).unwrap();
    let headers_main = crate::resolve::check_file_headers(&src_main, &ast_main, &index).unwrap();
    crate::resolve::check_file_bodies(&src_main, &mut ast_main, &index, &headers_main).unwrap();

    let mut env = typecheck::TypeEnv::from_sysroot(sess.sysroot(), &index).unwrap();
    env.extend_from_file(&src_model, &ast_model, &index)
        .unwrap();
    env.extend_from_file(&src_main, &ast_main, &index).unwrap();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    typecheck::check_file_annotations(
        &src_model,
        &ast_model,
        &index,
        &headers_model.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_properties(&src_model, &ast_model, &index, &env).unwrap();
    typecheck::check_file_inheritance(&src_model, &ast_model, &index).unwrap();
    typecheck::check_file_interfaces(&src_model, &ast_model, &index, &env).unwrap();
    typecheck::check_file_override_effects(
        &src_model,
        &ast_model,
        &index,
        &headers_model.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_refs(
        &src_model,
        &ast_model,
        &index,
        &headers_model.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_where_clauses(
        &src_model,
        &ast_model,
        &index,
        &headers_model.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_overload_conflicts(
        &src_model,
        &ast_model,
        &index,
        &headers_model.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_exprs(
        &src_model,
        &ast_model,
        &index,
        &headers_model.imports,
        &env,
        &mut types,
        builtins,
    )
    .unwrap();
    typecheck::check_file_type_layouts(&index, &env, &mut types, builtins).unwrap();

    let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in sess.sysroot().index_files() {
        unit.push((&f.source, &f.ast));
    }
    unit.push((&src_model, &ast_model));
    unit.push((&src_main, &ast_main));

    let lowered = lower_for_compilation_unit_multi_files(
        &src_main,
        &index,
        &unit,
        &[(&src_main, &ast_main)],
        &[],
        &types,
    )
    .unwrap();

    let main_fun = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.t3014cr.main" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.t3014cr.main");
    let main_body = main_fun.body.as_ref().expect("main 应有 body");
    let effect_ty = find_raise_perform_effect_ty_in_block(main_body)
        .expect("跨文件 observable callback 内的 Raise.raise 应被 lower 为带 effect_ty 的 Perform");
    assert_raise_int_effect_ty(&lowered.types, effect_ty);
}

#[test]
fn typed_hir_keeps_effect_generic_member_type_apply_on_direct_call_path() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>/hir_member_effect_type_apply.scoop",
        r#"
package fixtures.hirreview

class Box {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}
"#,
    );

    let lowered = lower_typed_for_dump(&sess, &src).unwrap();
    let wrap = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.hirreview.wrap" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.hirreview.wrap");
    let body = wrap.body.as_ref().expect("wrap 应有函数体");
    let call_expr = body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Return { value: Some(expr) } => Some(expr),
            _ => None,
        })
        .expect("wrap 应包含返回调用");
    let ExprKind::Call { callee, args } = &call_expr.kind else {
        panic!(
            "期望 wrap 返回值被 lower 为 direct call，实际为 {:?}",
            call_expr.kind
        );
    };
    assert_eq!(args.len(), 1, "成员 direct-call 应携带隐式 receiver 实参");
    match &callee.kind {
        ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
            assert_eq!(fqn, "fixtures.hirreview.Box.forward");
        }
        other => panic!("期望 callee 已被降糖为顶层函数引用，实际为 {other:?}"),
    }
}

#[test]
fn typed_hir_lowers_safe_member_type_apply_as_safe_direct_call() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>/hir_safe_member_effect_type_apply.scoop",
        r#"
package fixtures.hirreview

class Box {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box?): Int? / E {
    return box?.forward<eff E>()
}
"#,
    );

    let lowered = lower_typed_for_dump(&sess, &src)
        .expect("safe member direct-call + TypeApply 应能通过 typed HIR lowering");
    let wrap = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.hirreview.wrap" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.hirreview.wrap");
    let body = wrap.body.as_ref().expect("wrap 应有函数体");
    let ret_expr = body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Return { value: Some(expr) } => Some(expr),
            _ => None,
        })
        .expect("wrap 应包含返回表达式");
    let ExprKind::When { arms, .. } = &ret_expr.kind else {
        panic!(
            "safe member call 应被 lower 为 when desugar，实际为 {:?}",
            ret_expr.kind
        );
    };
    let some_arm = arms
        .iter()
        .find(|arm| matches!(&arm.pat, crate::hir::WhenPat::Variant { name, .. } if name == "Some"))
        .expect("safe call desugar 应包含 Some 分支");
    let ExprKind::Call {
        callee: some_callee,
        args: some_args,
    } = &some_arm.body.kind
    else {
        panic!(
            "Some 分支应包装 Some(inner_call)，实际为 {:?}",
            some_arm.body.kind
        );
    };
    match &some_callee.kind {
        ExprKind::UnresolvedIdent { name } => assert_eq!(name, "Some"),
        other => panic!("Some 分支外层应调用 Some(...)，实际为 {other:?}"),
    }
    let [CallArg::Positional(inner_call)] = some_args.as_slice() else {
        panic!("Some 分支应只包装一个 inner_call，实际为 {:?}", some_args);
    };
    let ExprKind::Call { callee, args } = &inner_call.kind else {
        panic!(
            "safe call 的 inner_call 应为 direct call，实际为 {:?}",
            inner_call.kind
        );
    };
    assert_eq!(
        args.len(),
        1,
        "safe member direct-call 仍应携带隐式 receiver 实参"
    );
    assert_eq!(
        some_arm.body.ty, ret_expr.ty,
        "Some 分支包装后的表达式应保留 `Int?` 结果类型"
    );
    match &callee.kind {
        ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
            assert_eq!(fqn, "fixtures.hirreview.Box.forward");
        }
        other => panic!("inner_call 应已降糖为顶层函数引用，实际为 {other:?}"),
    }
    let none_arm = arms
        .iter()
        .find(|arm| matches!(&arm.pat, crate::hir::WhenPat::Variant { name, .. } if name == "None"))
        .expect("safe call desugar 应包含 None 分支");
    let ExprKind::Call {
        callee: none_callee,
        args: none_args,
    } = &none_arm.body.kind
    else {
        panic!(
            "None 分支应重建为 0 参 variant ctor 调用，实际为 {:?}",
            none_arm.body.kind
        );
    };
    assert!(
        none_args.is_empty(),
        "None 分支应重建为 `None()` 而不是保留 unresolved value"
    );
    assert_eq!(
        none_arm.body.ty, ret_expr.ty,
        "None 分支应保留与 safe call 一致的 `Int?` 结果类型"
    );
    match &none_callee.kind {
        ExprKind::UnresolvedIdent { name } => assert_eq!(name, "None"),
        other => panic!("None 分支应调用 None()，实际为 {other:?}"),
    }
}

#[test]
fn typed_hir_lowers_companion_member_type_apply_as_direct_call() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>/hir_companion_member_effect_type_apply.scoop",
        r#"
package fixtures.hirreview

class Box {
    companion object {
        fun <eff E = Pure> forward(): Int / E {
            return 1
        }
    }
}

fun <eff E = Pure> wrap(): Int / E {
    return Box.forward<eff E>()
}
"#,
    );

    let lowered = lower_typed_for_dump(&sess, &src)
        .expect("companion member direct-call + TypeApply 应能通过 typed HIR lowering");
    let wrap = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.hirreview.wrap" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.hirreview.wrap");
    let body = wrap.body.as_ref().expect("wrap 应有函数体");
    let call_expr = body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Return { value: Some(expr) } => Some(expr),
            _ => None,
        })
        .expect("wrap 应包含返回调用");
    let ExprKind::Call { callee, args } = &call_expr.kind else {
        panic!(
            "期望 companion member 调用被 lower 为 direct call，实际为 {:?}",
            call_expr.kind
        );
    };
    assert_eq!(
        args.len(),
        1,
        "companion direct-call 应注入 companion receiver 实参"
    );
    match &callee.kind {
        ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
            assert_eq!(fqn, "fixtures.hirreview.Box.Companion.forward");
        }
        other => panic!("期望 callee 已被降糖为 companion 顶层函数引用，实际为 {other:?}"),
    }
    let [CallArg::Positional(receiver)] = args.as_slice() else {
        panic!(
            "companion direct-call 应只注入一个 receiver，实际为 {:?}",
            args
        );
    };
    match &receiver.kind {
        ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
            assert_eq!(fqn, "fixtures.hirreview.Box.Companion");
        }
        other => panic!("receiver 应为 companion object 单例值，实际为 {other:?}"),
    }
}

#[test]
fn typed_hir_preserves_function_typed_nested_call_callee() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>/hir_nested_callable_call.scoop",
        r#"
package fixtures.hirreview

fun make(x: Int): () -> Int {
    return { x }
}

fun main(): Int {
    return make(1)()
}
"#,
    );

    let lowered =
        lower_typed_for_dump(&sess, &src).expect("nested callable call should lower to typed HIR");
    let main = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.hirreview.main" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.hirreview.main");
    let body = main.body.as_ref().expect("main 应有函数体");
    let call_expr = body
        .stmts
        .iter()
        .find_map(|stmt| match &stmt.kind {
            StmtKind::Return { value: Some(expr) } => Some(expr),
            _ => None,
        })
        .expect("main 应包含返回调用");
    let ExprKind::Call { callee, .. } = &call_expr.kind else {
        panic!("期望外层表达式被 lower 为调用，实际为 {:?}", call_expr.kind);
    };
    assert!(
        matches!(
            lowered.types.kind(callee.ty),
            TypeKind::Ref(RefTypeKind::Function(_))
        ),
        "调用返回的 callable 在 typed HIR 中应保留函数类型，实际为 {}",
        lowered.types.display(callee.ty)
    );
}

#[test]
fn typed_hir_top_level_immutable_receiver_closure_keeps_length_as_call_in_side_table() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>/hir_top_level_receiver_closure_side_table.scoop",
        r#"
import scoop.core.*

val topNamed: String.(Int) -> Int = { n: Int -> this.length() + n }
"#,
    );

    let lowered = lower_typed_for_dump(&sess, &src)
        .expect("top-level receiver closure should lower to typed HIR");
    let value = lowered
        .top_level_immutable_values
        .get("topNamed")
        .expect("topNamed 应进入 top_level_immutable_values side table");
    let init = value.init.as_ref().expect("topNamed 应有 initializer");
    let ExprKind::Closure(closure) = &init.kind else {
        panic!("topNamed initializer 应为 closure，实际为 {:?}", init.kind);
    };
    let ExprKind::Binary { lhs, .. } = &closure.body.kind else {
        panic!(
            "receiver closure body 应为 binary，实际为 {:?}",
            closure.body.kind
        );
    };
    let ExprKind::Call { callee, args } = &lhs.kind else {
        panic!("length 调用应保留为 Call，实际为 {:?}", lhs.kind);
    };
    assert_eq!(
        args.len(),
        1,
        "String.length() 作为 body method 应携带 receiver 实参: {args:?}"
    );
    let ExprKind::VarRef(crate::hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
        panic!(
            "length 调用 callee 应为 direct body method，实际为 {:?}",
            callee.kind
        );
    };
    assert_eq!(fqn, "scoop.core.String.length");
    assert_eq!(lowered.types.display(lhs.ty).to_string(), "Int");
}

#[test]
fn typed_hir_continuation_resume_unit_sugar_canonicalizes_zero_arg_and_explicit_unit() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>/hir_continuation_resume_unit_sugar.scoop",
        r#"
package fixtures.hirreview

import scoop.core.*

fun takesUnit(value: Unit): Unit {}

fun resumeZero(k: Continuation<Unit, Unit, eff Pure>): Unit / Raise<RuntimeError> {
    k.resume()
    k.resume(())
    takesUnit()
    takesUnit(())
}
"#,
    );

    let lowered = lower_typed_for_dump(&sess, &src).unwrap();
    let resume_zero = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.hirreview.resumeZero" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.hirreview.resumeZero");
    let body = resume_zero.body.as_ref().expect("resumeZero 应有函数体");

    let call_exprs: Vec<&Expr> = body
        .stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StmtKind::Expr(expr) => Some(expr),
            _ => None,
        })
        .collect();
    assert_eq!(call_exprs.len(), 4, "resumeZero 应包含 4 个调用表达式语句");

    for (index, call_expr) in call_exprs.iter().enumerate() {
        let ExprKind::Call { callee, args } = &call_expr.kind else {
            panic!("期望调用表达式，实际为 {:?}", call_expr.kind);
        };
        match index {
            0 | 1 => {
                match &callee.kind {
                    ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
                        assert_eq!(fqn, "scoop.core.Continuation.resume");
                    }
                    other => panic!("resume call 应 lower 为 direct call，实际为 {other:?}"),
                }
                let [CallArg::Positional(receiver), CallArg::Positional(arg)] = args.as_slice()
                else {
                    panic!(
                        "typed HIR 应把 continuation.resume canonicalize 为 receiver + Unit 实参: {:?}",
                        args
                    );
                };
                assert!(
                    matches!(receiver.kind, ExprKind::VarRef(ValueRef::Local { .. })),
                    "resume direct-call 的第 0 个实参应为 receiver，实际为 {:?}",
                    receiver.kind
                );
                assert!(
                    matches!(arg.kind, ExprKind::Literal(LiteralKind::Unit)),
                    "canonicalized resume payload 应为显式 Unit literal，实际为 {:?}",
                    arg.kind
                );
            }
            _ => {
                let [CallArg::Positional(arg)] = args.as_slice() else {
                    panic!(
                        "typed HIR 应把 zero-arg Unit sugar canonicalize 为单个实参: {:?}",
                        args
                    );
                };
                assert!(
                    matches!(arg.kind, ExprKind::Literal(LiteralKind::Unit)),
                    "canonicalized Unit sugar 实参应为显式 Unit literal，实际为 {:?}",
                    arg.kind
                );
            }
        }
    }
}

#[test]
fn typed_hir_unit_single_param_zero_arg_call_canonicalizes_to_unit_literal() {
    let sess = Session::new().unwrap();
    let src = SourceFile::new_virtual(
        "<mem>/hir_unit_single_param_zero_arg_call.scoop",
        r#"
package fixtures.hirreview

fun takesUnit(value: Unit): Unit {}

fun run(): Unit {
    takesUnit()
    takesUnit(())
}
"#,
    );

    let lowered = lower_typed_for_dump(&sess, &src).unwrap();
    let run = lowered
        .file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.hirreview.run" => Some(fun),
            _ => None,
        })
        .expect("应收集到 fixtures.hirreview.run");
    let body = run.body.as_ref().expect("run 应有函数体");

    let call_exprs: Vec<&Expr> = body
        .stmts
        .iter()
        .filter_map(|stmt| match &stmt.kind {
            StmtKind::Expr(expr) => Some(expr),
            _ => None,
        })
        .collect();
    assert_eq!(call_exprs.len(), 2, "run 应包含 2 个调用表达式语句");

    for call_expr in call_exprs {
        let ExprKind::Call { args, .. } = &call_expr.kind else {
            panic!("期望调用表达式，实际为 {:?}", call_expr.kind);
        };
        let [CallArg::Positional(arg)] = args.as_slice() else {
            panic!(
                "typed HIR 应把 `takesUnit()` canonicalize 为单个 Unit 实参: {:?}",
                args
            );
        };
        assert!(
            matches!(arg.kind, ExprKind::Literal(LiteralKind::Unit)),
            "canonicalized Unit 实参应为显式 Unit literal，实际为 {:?}",
            arg.kind
        );
    }
}
