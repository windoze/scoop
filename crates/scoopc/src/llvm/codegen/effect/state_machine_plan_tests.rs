mod tests {
    use std::collections::HashMap;

    use crate::ast;
    use crate::hir;
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore};
    use crate::typecheck;

    use super::{HandlePlanContext, HandleStateMachinePlan};

    #[test]
    fn plan_dump_covers_direct_branch_loop_and_finally() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() -> resume {
            resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=direct-perform"));
        assert!(dump.contains("branch cond=if-cond"));
        assert!(dump.contains("loop re-entry"));
        assert!(dump.contains("cleanup0 kind=finally"));
        assert!(dump.contains("mode=immediate-resume"));
    }

    #[test]
    fn plan_dump_distinguishes_state_machine_callee_and_indirect_call_sites() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(thunk: () -> Int / (Ask)): Int {
    val result: Int = handle {
        val a: Int = fetch(1)
        val b: Int = thunk()
        a + b
    } with {
        Ask.ask(seed) -> resume {
            resume(seed + 10)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-state-machine-callee"));
        assert!(dump.contains("detail=a.fetch"));
        assert!(dump.contains("kind=indirect-call-may-suspend"));
    }

    #[test]
    fn plan_dump_covers_nested_handle_and_multiple_arms() {
        let dump = build_plan_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Nothing
}

fun demo(mode: Int): Int {
    val result: Int = handle {
        val inner: Int = handle {
            val x: Int = Yield.next()
            x + mode
        } with {
            Yield.next() -> resume {
                resume(10)
            }
        }
        if (mode == 0) {
            val y: Int = Ask.current()
            inner + y
        } else {
            Boom.boom(mode)
            0
        }
    } with {
        Ask.current(), k -> 7
        Boom.boom(code: Int) -> 0
    }
    result
}
"#,
        );

        assert!(dump.contains("nested-handles:\n  nested#0"));
        assert!(dump.contains("mode=escape-continuation"));
        assert!(dump.contains("mode=never-resume"));
        assert!(dump.contains("dispatch:\n  a.Ask.current => [arm0]\n  a.Boom.boom => [arm1]"));
    }

    fn build_plan_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
            .pretty_dump(&lowered.types)
    }

    fn lower_typed_single_source(source_text: &str) -> hir::LoweredHir {
        let session = Session::new().unwrap();
        let source = SourceFile::new_virtual("<mem>", source_text);
        let mut ast = parse_file(&source).unwrap();

        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).unwrap()
        };

        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));

        hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &typecheck_types,
        )
        .unwrap()
    }

    fn first_handle_in_file(file: &hir::File) -> Option<(&hir::FunDecl, &hir::HandleExpr)> {
        for item in &file.items {
            if let hir::Item::Fun(fun) = item
                && let Some(body) = &fun.body
                && let Some(handle) = first_handle_in_block(body)
            {
                return Some((fun, handle));
            }
        }
        None
    }

    fn first_handle_in_block(block: &hir::Block) -> Option<&hir::HandleExpr> {
        for stmt in &block.stmts {
            if let Some(handle) = first_handle_in_stmt(stmt) {
                return Some(handle);
            }
        }
        None
    }

    fn first_handle_in_stmt(stmt: &hir::Stmt) -> Option<&hir::HandleExpr> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => first_handle_in_expr(expr),
            hir::StmtKind::Val(decl) => decl.init.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::StmtKind::While { cond, body } => {
                first_handle_in_expr(cond).or_else(|| first_handle_in_block(body))
            }
            hir::StmtKind::Return { value } => value.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
        }
    }

    fn first_handle_in_expr(expr: &hir::Expr) -> Option<&hir::HandleExpr> {
        match &expr.kind {
            hir::ExprKind::Handle(handle) => Some(handle),
            hir::ExprKind::Block(block) => first_handle_in_block(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => first_handle_in_expr(cond)
                .or_else(|| first_handle_in_expr(then_branch))
                .or_else(|| else_branch.as_deref().and_then(first_handle_in_expr)),
            hir::ExprKind::Call { callee, args } => first_handle_in_expr(callee).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                    hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
                })
            }),
            hir::ExprKind::StructLit { fields, .. } => {
                fields.iter().find_map(|field| first_handle_in_expr(&field.value))
            }
            hir::ExprKind::TupleLit { elements } => elements.iter().find_map(first_handle_in_expr),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    first_handle_in_expr(expr)
                } else {
                    None
                }
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => first_handle_in_expr(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::ExprKind::When { subject, arms } => first_handle_in_expr(subject).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(first_handle_in_expr)
                        .or_else(|| first_handle_in_expr(&arm.body))
                })
            }),
            hir::ExprKind::Closure(closure) => first_handle_in_expr(&closure.body),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
            }),
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => None,
        }
    }

    fn collect_plan_context(lowered: &hir::LoweredHir, owner_fun: &hir::FunDecl) -> HandlePlanContext {
        let mut known_fun_effects = HashMap::new();
        for item in &lowered.file.items {
            if let hir::Item::Fun(fun) = item {
                known_fun_effects.insert(
                    fun.fqn.clone(),
                    fun_effects_are_non_pure(&lowered.types, fun.ty),
                );
            }
        }
        for fun in &lowered.member_funs {
            known_fun_effects.insert(
                fun.fqn.clone(),
                fun_effects_are_non_pure(&lowered.types, fun.ty),
            );
        }

        let mut known_local_fun_effects = HashMap::new();
        for param in &owner_fun.params {
            known_local_fun_effects.insert(
                param.id,
                fun_effects_are_non_pure(&lowered.types, param.ty),
            );
        }
        if let Some(body) = &owner_fun.body {
            collect_local_fun_effects_in_block(body, &lowered.types, &mut known_local_fun_effects);
        }

        HandlePlanContext {
            known_fun_effects,
            known_local_fun_effects,
        }
    }

    fn fun_effects_are_non_pure(types: &TypeStore, ty: TypeId) -> bool {
        match types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => !fun_ty.effects.is_pure(),
            _ => false,
        }
    }

    fn collect_local_fun_effects_in_block(
        block: &hir::Block,
        types: &TypeStore,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        for stmt in &block.stmts {
            collect_local_fun_effects_in_stmt(stmt, types, out);
        }
    }

    fn collect_local_fun_effects_in_stmt(
        stmt: &hir::Stmt,
        types: &TypeStore,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &stmt.kind {
            hir::StmtKind::Val(decl) => {
                if let Some(id) = decl.id {
                    out.insert(id, fun_effects_are_non_pure(types, decl.ty));
                }
                if let Some(init) = decl.init.as_ref() {
                    collect_local_fun_effects_in_expr(init, types, out);
                }
            }
            hir::StmtKind::Expr(expr) => collect_local_fun_effects_in_expr(expr, types, out),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                collect_local_fun_effects_in_expr(lhs, types, out);
                collect_local_fun_effects_in_expr(rhs, types, out);
            }
            hir::StmtKind::While { cond, body } => {
                collect_local_fun_effects_in_expr(cond, types, out);
                collect_local_fun_effects_in_block(body, types, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    collect_local_fun_effects_in_expr(expr, types, out);
                }
            }
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
        }
    }

    fn collect_local_fun_effects_in_expr(
        expr: &hir::Expr,
        types: &TypeStore,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &expr.kind {
            hir::ExprKind::Block(block) => collect_local_fun_effects_in_block(block, types, out),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                collect_local_fun_effects_in_expr(cond, types, out);
                collect_local_fun_effects_in_expr(then_branch, types, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    collect_local_fun_effects_in_expr(else_branch, types, out);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                collect_local_fun_effects_in_expr(subject, types, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        collect_local_fun_effects_in_expr(guard, types, out);
                    }
                    collect_local_fun_effects_in_expr(&arm.body, types, out);
                }
            }
            hir::ExprKind::Call { callee, args } => {
                collect_local_fun_effects_in_expr(callee, types, out);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            collect_local_fun_effects_in_expr(expr, types, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            collect_local_fun_effects_in_expr(value, types, out)
                        }
                    }
                }
            }
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    collect_local_fun_effects_in_expr(&field.value, types, out);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    collect_local_fun_effects_in_expr(element, types, out);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        collect_local_fun_effects_in_expr(expr, types, out);
                    }
                }
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => collect_local_fun_effects_in_expr(inner, types, out),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                collect_local_fun_effects_in_expr(lhs, types, out);
                collect_local_fun_effects_in_expr(rhs, types, out);
            }
            hir::ExprKind::Closure(closure) => {
                collect_local_fun_effects_in_expr(&closure.body, types, out);
            }
            hir::ExprKind::Handle(handle) => {
                collect_local_fun_effects_in_block(&handle.body, types, out);
                for arm in &handle.arms {
                    for binder in &arm.op.binders {
                        out.insert(binder.id, fun_effects_are_non_pure(types, binder.ty));
                    }
                    collect_local_fun_effects_in_expr(&arm.body, types, out);
                }
                if let Some(finally_block) = &handle.finally {
                    collect_local_fun_effects_in_block(finally_block, types, out);
                }
            }
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            collect_local_fun_effects_in_expr(expr, types, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            collect_local_fun_effects_in_expr(value, types, out)
                        }
                    }
                }
            }
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => {}
        }
    }
}
