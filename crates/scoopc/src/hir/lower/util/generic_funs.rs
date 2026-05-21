//! Generic-fun call collection, generic-fun instantiations, stable template suffixes, explicit instance helpers.

#![allow(dead_code)]

use super::*;

pub(in crate::hir::lower) fn generic_fun_dispatch_fqn(fqn: &str) -> &str {
    if let Some((base, _)) = fqn.rsplit_once("::<") {
        return base;
    }
    fqn.split_once("$overload$")
        .map(|(base, _)| base)
        .unwrap_or(fqn)
}

pub(in crate::hir::lower) fn generic_fun_callee_fqn(expr: &super::super::Expr) -> Option<&str> {
    match &expr.kind {
        super::super::ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) => {
            Some(generic_fun_dispatch_fqn(fqn.as_str()))
        }
        super::super::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
            MemberRef::Fun { fqn, .. } | MemberRef::ExtensionFun { fqn, .. } => {
                Some(generic_fun_dispatch_fqn(fqn.as_str()))
            }
            _ => None,
        },
        _ => None,
    }
}

pub(in crate::hir::lower) fn generic_fun_call_receiver_expr(
    expr: &super::super::Expr,
) -> Option<&super::super::Expr> {
    let super::super::ExprKind::MemberAccess { receiver, member } = &expr.kind else {
        return None;
    };
    match member.resolved.as_ref()? {
        MemberRef::Fun { .. } | MemberRef::ExtensionFun { .. } => Some(receiver),
        _ => None,
    }
}

pub(in crate::hir::lower) fn infer_generic_fun_call_type_args(
    types: &TypeStore,
    sig_fun: &super::super::FunDecl,
    declared_type_param_names: &[String],
    receiver_expr: Option<&super::super::Expr>,
    args: &[CallArg],
    result_ty: crate::ty::TypeId,
) -> Option<Vec<crate::ty::TypeId>> {
    let receiver_param_offset = usize::from(
        receiver_expr.is_some()
            && sig_fun
                .params
                .first()
                .is_some_and(|param| param.name == "this"),
    );

    let decl_param_names: Vec<String> = sig_fun
        .params
        .iter()
        .skip(receiver_param_offset)
        .map(|param| param.name.clone())
        .collect();
    let arg_to_param = map_hir_call_args_to_params_by_name(&decl_param_names, args)?;
    let mut param_to_arg: Vec<Option<usize>> = vec![None; sig_fun.params.len()];
    for (arg_idx, param_idx) in arg_to_param.iter().copied().enumerate() {
        *param_to_arg.get_mut(param_idx + receiver_param_offset)? = Some(arg_idx);
    }

    let mut referenced_type_param_names: Vec<String> = Vec::new();
    for param in &sig_fun.params {
        collect_hir_type_param_names(types, param.ty, &mut referenced_type_param_names);
    }
    collect_hir_type_param_names(types, sig_fun.return_ty, &mut referenced_type_param_names);
    if referenced_type_param_names.is_empty() {
        return None;
    }

    let mut bindings: HashMap<String, crate::ty::TypeId> = HashMap::new();
    for (idx, param) in sig_fun.params.iter().enumerate() {
        if !type_contains_param(types, param.ty) {
            continue;
        }
        let concrete_ty = if receiver_param_offset == 1 && idx == 0 {
            let receiver_expr = receiver_expr?;
            extract_concrete_hir_expr_ty(types, receiver_expr)?
        } else {
            let arg_idx = param_to_arg.get(idx).copied().flatten()?;
            let arg_expr = match args.get(arg_idx)? {
                CallArg::Positional(expr) => expr,
                CallArg::Named { value, .. } => value,
            };
            extract_concrete_hir_expr_ty(types, arg_expr)?
        };
        collect_hir_type_param_bindings(types, param.ty, concrete_ty, &mut bindings);
    }
    if type_contains_param(types, sig_fun.return_ty) && !type_contains_param(types, result_ty) {
        let param_type_param_names = sig_fun
            .params
            .iter()
            .flat_map(|param| {
                let mut names = Vec::new();
                collect_hir_type_param_names(types, param.ty, &mut names);
                names
            })
            .collect::<HashSet<_>>();
        let mut result_bindings = HashMap::new();
        collect_hir_type_param_bindings(types, sig_fun.return_ty, result_ty, &mut result_bindings);
        for (name, ty) in result_bindings {
            if !param_type_param_names.contains(&name) || bindings.contains_key(&name) {
                bindings.entry(name).or_insert(ty);
            }
        }
    }

    let mut ordered_args = Vec::with_capacity(declared_type_param_names.len());
    for name in declared_type_param_names {
        let ty = bindings.get(name).copied()?;
        if type_contains_param(types, ty) {
            return None;
        }
        ordered_args.push(ty);
    }
    Some(ordered_args)
}

pub(in crate::hir::lower) fn collect_generic_fun_calls_in_block(
    block: &Block,
    generic_fun_candidates_by_fqn: &HashMap<String, Vec<(String, Span)>>,
    generic_fun_type_param_names: &HashMap<(String, Span), Vec<String>>,
    generic_fun_signatures: &HashMap<(String, Span), super::super::FunDecl>,
    types: &TypeStore,
    out: &mut Vec<((String, Span), Vec<crate::ty::TypeId>)>,
) {
    for stmt in &block.stmts {
        collect_generic_fun_calls_in_stmt(
            stmt,
            generic_fun_candidates_by_fqn,
            generic_fun_type_param_names,
            generic_fun_signatures,
            types,
            out,
        );
    }
}

pub(in crate::hir::lower) fn collect_generic_fun_calls_in_stmt(
    stmt: &super::super::Stmt,
    generic_fun_candidates_by_fqn: &HashMap<String, Vec<(String, Span)>>,
    generic_fun_type_param_names: &HashMap<(String, Span), Vec<String>>,
    generic_fun_signatures: &HashMap<(String, Span), super::super::FunDecl>,
    types: &TypeStore,
    out: &mut Vec<((String, Span), Vec<crate::ty::TypeId>)>,
) {
    match &stmt.kind {
        StmtKind::Empty
        | StmtKind::Break { .. }
        | StmtKind::Continue { .. }
        | StmtKind::Todo(_) => {}
        StmtKind::Expr(expr) => collect_generic_fun_calls_in_expr(
            expr,
            generic_fun_candidates_by_fqn,
            generic_fun_type_param_names,
            generic_fun_signatures,
            types,
            out,
        ),
        StmtKind::Val(decl) => {
            if let Some(init) = decl.init.as_ref() {
                collect_generic_fun_calls_in_expr(
                    init,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
        }
        StmtKind::Assign { lhs, rhs, .. } => {
            collect_generic_fun_calls_in_expr(
                lhs,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            collect_generic_fun_calls_in_expr(
                rhs,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
        }
        StmtKind::Return { value } => {
            if let Some(value) = value.as_ref() {
                collect_generic_fun_calls_in_expr(
                    value,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
        }
        StmtKind::While { cond, body } => {
            collect_generic_fun_calls_in_expr(
                cond,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            collect_generic_fun_calls_in_block(
                body,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
        }
    }
}

pub(in crate::hir::lower) fn collect_generic_fun_calls_in_expr(
    expr: &super::super::Expr,
    generic_fun_candidates_by_fqn: &HashMap<String, Vec<(String, Span)>>,
    generic_fun_type_param_names: &HashMap<(String, Span), Vec<String>>,
    generic_fun_signatures: &HashMap<(String, Span), super::super::FunDecl>,
    types: &TypeStore,
    out: &mut Vec<((String, Span), Vec<crate::ty::TypeId>)>,
) {
    match &expr.kind {
        super::super::ExprKind::Missing
        | super::super::ExprKind::Literal(_)
        | super::super::ExprKind::VarRef(_)
        | super::super::ExprKind::UnresolvedIdent { .. }
        | super::super::ExprKind::ClassLiteral(_)
        | super::super::ExprKind::Todo(_) => {}
        super::super::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_generic_fun_calls_in_expr(
                    &field.value,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
        }
        super::super::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_generic_fun_calls_in_expr(
                    element,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
        }
        super::super::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let InterpolatedStringPart::Expr { expr } = part {
                    collect_generic_fun_calls_in_expr(
                        expr,
                        generic_fun_candidates_by_fqn,
                        generic_fun_type_param_names,
                        generic_fun_signatures,
                        types,
                        out,
                    );
                }
            }
        }
        super::super::ExprKind::Unary { expr: inner, .. }
        | super::super::ExprKind::TypeCheck { expr: inner, .. }
        | super::super::ExprKind::Cast { expr: inner, .. } => collect_generic_fun_calls_in_expr(
            inner,
            generic_fun_candidates_by_fqn,
            generic_fun_type_param_names,
            generic_fun_signatures,
            types,
            out,
        ),
        super::super::ExprKind::Binary { lhs, rhs, .. } => {
            collect_generic_fun_calls_in_expr(
                lhs,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            collect_generic_fun_calls_in_expr(
                rhs,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
        }
        super::super::ExprKind::Block(block) => collect_generic_fun_calls_in_block(
            block,
            generic_fun_candidates_by_fqn,
            generic_fun_type_param_names,
            generic_fun_signatures,
            types,
            out,
        ),
        super::super::ExprKind::Call { callee, args } => {
            if let Some(callee_fqn) = generic_fun_callee_fqn(callee)
                && let Some(candidates) = generic_fun_candidates_by_fqn.get(callee_fqn)
                && candidates.len() == 1
            {
                let lookup_key = candidates[0].clone();
                if let (Some(sig_fun), Some(type_param_names)) = (
                    generic_fun_signatures.get(&lookup_key),
                    generic_fun_type_param_names.get(&lookup_key),
                ) && let Some(type_args) = infer_generic_fun_call_type_args(
                    types,
                    sig_fun,
                    type_param_names,
                    generic_fun_call_receiver_expr(callee),
                    args,
                    expr.ty,
                ) {
                    out.push((lookup_key, type_args));
                }
            }

            collect_generic_fun_calls_in_expr(
                callee,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => collect_generic_fun_calls_in_expr(
                        expr,
                        generic_fun_candidates_by_fqn,
                        generic_fun_type_param_names,
                        generic_fun_signatures,
                        types,
                        out,
                    ),
                    CallArg::Named { value, .. } => collect_generic_fun_calls_in_expr(
                        value,
                        generic_fun_candidates_by_fqn,
                        generic_fun_type_param_names,
                        generic_fun_signatures,
                        types,
                        out,
                    ),
                }
            }
        }
        super::super::ExprKind::Closure(closure) => collect_generic_fun_calls_in_expr(
            &closure.body,
            generic_fun_candidates_by_fqn,
            generic_fun_type_param_names,
            generic_fun_signatures,
            types,
            out,
        ),
        super::super::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_generic_fun_calls_in_expr(
                cond,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            collect_generic_fun_calls_in_expr(
                then_branch,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            if let Some(else_branch) = else_branch.as_ref() {
                collect_generic_fun_calls_in_expr(
                    else_branch,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
        }
        super::super::ExprKind::When { subject, arms } => {
            collect_generic_fun_calls_in_expr(
                subject,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_generic_fun_calls_in_expr(
                        guard,
                        generic_fun_candidates_by_fqn,
                        generic_fun_type_param_names,
                        generic_fun_signatures,
                        types,
                        out,
                    );
                }
                collect_generic_fun_calls_in_expr(
                    &arm.body,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
        }
        super::super::ExprKind::MemberAccess { receiver, .. } => collect_generic_fun_calls_in_expr(
            receiver,
            generic_fun_candidates_by_fqn,
            generic_fun_type_param_names,
            generic_fun_signatures,
            types,
            out,
        ),
        super::super::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    CallArg::Positional(expr) => collect_generic_fun_calls_in_expr(
                        expr,
                        generic_fun_candidates_by_fqn,
                        generic_fun_type_param_names,
                        generic_fun_signatures,
                        types,
                        out,
                    ),
                    CallArg::Named { value, .. } => collect_generic_fun_calls_in_expr(
                        value,
                        generic_fun_candidates_by_fqn,
                        generic_fun_type_param_names,
                        generic_fun_signatures,
                        types,
                        out,
                    ),
                }
            }
        }
        super::super::ExprKind::Handle(handle) => {
            collect_generic_fun_calls_in_block(
                &handle.body,
                generic_fun_candidates_by_fqn,
                generic_fun_type_param_names,
                generic_fun_signatures,
                types,
                out,
            );
            for arm in &handle.arms {
                collect_generic_fun_calls_in_expr(
                    &arm.body,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
            if let Some(finally) = handle.finally.as_ref() {
                collect_generic_fun_calls_in_block(
                    finally,
                    generic_fun_candidates_by_fqn,
                    generic_fun_type_param_names,
                    generic_fun_signatures,
                    types,
                    out,
                );
            }
        }
    }
}

/// T0127: 从 monomorph keys 收集泛型独立函数的具体实例化，生成单态化的 HIR FunDecl。
///
/// 工作原理：
/// 1. 从 AST 中索引所有泛型顶层函数声明（有 type_params 的 `ast::Item::Fun`）。
/// 2. 遍历 monomorph keys，对每个 key 找到对应的函数声明。
/// 3. 调用 `lower_fun_with_type_bindings` 生成具体实例的 HIR FunDecl。
/// 4. 重命名 FQN 为 mangled 形式（例如 `pkg.id::<Int>`）。
pub(in crate::hir::lower) struct GenericFunInstantiationInputs<'a> {
    pub compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    pub monomorph_keys: &'a [crate::monomorph::MonomorphKey],
    pub index: &'a Index,
    pub type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub types: &'a mut TypeStore,
    pub builtins: BuiltinTypes,
    pub typecheck_types: &'a TypeStore,
    pub initial_items: &'a [super::super::Item],
    pub initial_member_funs: &'a [super::super::FunDecl],
    pub stable_cone_key: &'a StableConeKey,
    pub source_cones: &'a HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
}

pub(in crate::hir::lower) fn collect_generic_fun_instantiations(
    inputs: GenericFunInstantiationInputs<'_>,
) -> Vec<super::super::FunDecl> {
    let GenericFunInstantiationInputs {
        compilation_unit,
        monomorph_keys,
        index,
        type_kinds,
        types,
        builtins,
        typecheck_types,
        initial_items,
        initial_member_funs,
        stable_cone_key,
        source_cones,
    } = inputs;

    if monomorph_keys.is_empty() && initial_items.is_empty() && initial_member_funs.is_empty() {
        return Vec::new();
    }

    // 1) 索引泛型顶层函数：(fqn, decl_span) → (source, file, fun_decl)
    let mut generic_funs: HashMap<
        (String, crate::span::Span),
        (&SourceFile, &ast::File, &ast::FunDecl),
    > = HashMap::new();
    for (source, file) in compilation_unit {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };
            if fun.type_params.is_empty() {
                continue;
            }
            if matches!(fun.body, ast::FunBody::Missing) {
                // T4016T2: `sysroot/lib/scoop.core/src/core.scoop` 会保留 declaration-only surface，而真实实现体位于
                // `sysroot/lib/scoop.task/src/task.scoop` 等可编译源。generic fixed-point 发现器只应索引“可实例化的实现体”，
                // 否则像 `scoop.core.step` 这类符号会因为 declaration + implementation 双候选而被误判成 overload。
                continue;
            }
            let local_name = source.slice(fun.name.span);
            let fqn = if pkg_prefix.is_empty() {
                local_name.to_string()
            } else {
                format!("{pkg_prefix}.{local_name}")
            };
            generic_funs.insert((fqn, fun.name.span), (source, file, fun));
        }
    }

    if generic_funs.is_empty() {
        return Vec::new();
    }
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes_with_source_cones(
            stable_cone_key,
            index,
            compilation_unit,
            source_cones,
        );
    let mut generic_fun_candidates_by_fqn: HashMap<String, Vec<(String, crate::span::Span)>> =
        HashMap::new();
    let mut generic_fun_type_param_names: HashMap<(String, crate::span::Span), Vec<String>> =
        HashMap::new();
    let mut generic_fun_signatures: HashMap<(String, crate::span::Span), super::super::FunDecl> =
        HashMap::new();
    for (lookup_key, (source, file, fun_decl)) in &generic_funs {
        generic_fun_candidates_by_fqn
            .entry(lookup_key.0.clone())
            .or_default()
            .push(lookup_key.clone());
        generic_fun_type_param_names.insert(
            lookup_key.clone(),
            fun_decl
                .type_params
                .iter()
                .map(|param| param.name.text(source).to_string())
                .collect(),
        );

        let param_bindings = fun_decl
            .type_params
            .iter()
            .map(|param| {
                let name = param.name.text(source).to_string();
                let ty = types.ty_param(crate::ty::TypeParamType {
                    name: name.clone(),
                    decl_file: source.path().to_path_buf(),
                    decl_span: param.name.span,
                });
                (name, ty)
            })
            .collect::<Vec<_>>();
        let sig_fun = super::super::lower_fun_with_type_bindings(
            crate::hir::LoweringInputs {
                source,
                file,
                index,
                type_kinds,
                typecheck_types: Some(typecheck_types),
                compilation_unit,
                types,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                materialize_direct_call_targets: true,
            },
            fun_decl,
            param_bindings,
        );
        generic_fun_signatures.insert(lookup_key.clone(), sig_fun);
    }

    // 2) fixed-point：初始 monomorph key + 新生成实例体里的 generic fun 调用，直到收敛。
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<super::super::FunDecl> = Vec::new();
    let mut pending: Vec<((String, crate::span::Span), Vec<crate::ty::TypeId>)> = monomorph_keys
        .iter()
        .map(|key| {
            let re_interned_args = key
                .type_args
                .iter()
                .map(|&arg| types.re_intern_from(typecheck_types, arg))
                .collect::<Vec<_>>();
            (
                (key.symbol.fqn.clone(), key.symbol.decl_span),
                re_interned_args,
            )
        })
        .collect();

    for item in initial_items {
        let super::super::Item::Fun(fun) = item else {
            continue;
        };
        let Some(body) = fun.body.as_ref() else {
            continue;
        };
        collect_generic_fun_calls_in_block(
            body,
            &generic_fun_candidates_by_fqn,
            &generic_fun_type_param_names,
            &generic_fun_signatures,
            types,
            &mut pending,
        );
    }
    for fun in initial_member_funs {
        let Some(body) = fun.body.as_ref() else {
            continue;
        };
        collect_generic_fun_calls_in_block(
            body,
            &generic_fun_candidates_by_fqn,
            &generic_fun_type_param_names,
            &generic_fun_signatures,
            types,
            &mut pending,
        );
    }

    while let Some((lookup_key, re_interned_args)) = pending.pop() {
        if re_interned_args
            .iter()
            .any(|&a| type_contains_param(types, a))
        {
            continue;
        }

        let Some((source, file, fun_decl)) = generic_funs.get(&lookup_key) else {
            continue;
        };
        if fun_decl.type_params.len() != re_interned_args.len() {
            continue;
        }

        let template = TemplateKey {
            fqn: lookup_key.0.clone(),
            source_path: source.path().to_path_buf(),
            decl_span: fun_decl.span,
        };
        let instance_fqn = stable_instance_fqn(
            types,
            &template,
            &re_interned_args,
            &[],
            generic_template_symbol_suffixes
                .get(&template)
                .map(String::as_str)
                .unwrap_or(""),
        );
        if !seen.insert(instance_fqn.clone()) {
            continue;
        }

        let bindings: Vec<(String, crate::ty::TypeId)> = fun_decl
            .type_params
            .iter()
            .zip(re_interned_args.iter())
            .map(|(param, &arg)| (param.name.text(source).to_string(), arg))
            .collect();

        let mut hir_fun = super::super::lower_fun_with_type_bindings(
            crate::hir::LoweringInputs {
                source,
                file,
                index,
                type_kinds,
                typecheck_types: Some(typecheck_types),
                compilation_unit,
                types,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                materialize_direct_call_targets: true,
            },
            fun_decl,
            bindings,
        );

        if let Some(body) = hir_fun.body.as_ref() {
            let mut discovered = Vec::new();
            collect_generic_fun_calls_in_block(
                body,
                &generic_fun_candidates_by_fqn,
                &generic_fun_type_param_names,
                &generic_fun_signatures,
                types,
                &mut discovered,
            );
            pending.extend(discovered);
        }

        hir_fun.fqn = instance_fqn;
        out.push(hir_fun);
    }

    out
}

pub(in crate::hir::lower) struct ExplicitTopLevelGenericFunTemplate<'a> {
    pub(in crate::hir::lower) source: &'a SourceFile,
    pub(in crate::hir::lower) file: &'a ast::File,
    pub(in crate::hir::lower) fun: &'a ast::FunDecl,
    pub(in crate::hir::lower) signature_key: String,
    pub(in crate::hir::lower) has_body: bool,
}

pub(in crate::hir::lower) struct ExplicitGenericFunInstantiationInputs<'a> {
    pub compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    pub instance_keys: &'a [InstanceKey],
    pub instance_types: &'a TypeStore,
    pub index: &'a Index,
    pub type_kinds: &'a HashMap<String, ast::TypeKind>,
    pub types: &'a mut TypeStore,
    pub builtins: BuiltinTypes,
    pub typecheck_types: &'a TypeStore,
    pub stable_cone_key: &'a StableConeKey,
    pub source_cones: &'a HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
}

pub(in crate::hir::lower) fn collect_generic_fun_instantiations_from_instance_keys(
    inputs: ExplicitGenericFunInstantiationInputs<'_>,
) -> Result<Vec<super::super::FunDecl>, crate::hir::HirLowerError> {
    let ExplicitGenericFunInstantiationInputs {
        compilation_unit,
        instance_keys,
        instance_types,
        index,
        type_kinds,
        types,
        builtins,
        typecheck_types,
        stable_cone_key,
        source_cones,
    } = inputs;

    if instance_keys.is_empty() {
        return Ok(Vec::new());
    }

    let generic_funs = collect_explicit_top_level_generic_fun_templates_with_source_cones(
        stable_cone_key,
        index,
        compilation_unit,
        source_cones,
    );
    if generic_funs.is_empty() {
        return Ok(Vec::new());
    }
    let generic_template_symbol_suffixes =
        collect_generic_template_symbol_suffixes_with_source_cones(
            stable_cone_key,
            index,
            compilation_unit,
            source_cones,
        );
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();

    for instance in instance_keys {
        let Some(template) = generic_funs.get(&instance.template) else {
            continue;
        };
        let re_interned_type_args = instance
            .type_args
            .iter()
            .map(|&arg| types.re_intern_from(instance_types, arg))
            .collect::<Vec<_>>();
        if template.fun.type_params.len() != re_interned_type_args.len() {
            return Err(explicit_instance_lowering_error(format!(
                "top-level generic fun `{}` 的 type args 数量不匹配：期望 {}，得到 {}",
                instance.template.fqn,
                template.fun.type_params.len(),
                re_interned_type_args.len()
            )));
        }
        let re_interned_eff_args = instance
            .eff_args
            .iter()
            .map(|row| re_intern_effect_row_from(types, instance_types, row))
            .collect::<Vec<_>>();
        let effect_binding = build_effect_binding(
            template.source,
            &instance.template.fqn,
            &template.fun.eff_param,
            &re_interned_eff_args,
        )?;
        let instance_fqn = stable_instance_fqn(
            types,
            &instance.template,
            &re_interned_type_args,
            &re_interned_eff_args,
            generic_template_symbol_suffixes
                .get(&instance.template)
                .map(String::as_str)
                .unwrap_or(""),
        );
        if !seen.insert(instance_fqn.clone()) {
            continue;
        }

        let bindings = template
            .fun
            .type_params
            .iter()
            .zip(re_interned_type_args.iter())
            .map(|(param, &arg)| (param.name.text(template.source).to_string(), arg))
            .collect::<Vec<_>>();

        let mut hir_fun = super::super::lower_fun_with_bindings(
            crate::hir::LoweringInputs {
                source: template.source,
                file: template.file,
                index,
                type_kinds,
                typecheck_types: Some(typecheck_types),
                compilation_unit,
                types,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                materialize_direct_call_targets: true,
            },
            template.fun,
            bindings,
            effect_binding,
        );
        hir_fun.fqn = instance_fqn;
        out.push(hir_fun);
    }

    Ok(out)
}

pub(in crate::hir::lower) fn collect_explicit_top_level_generic_fun_templates<'a>(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
) -> HashMap<TemplateKey, ExplicitTopLevelGenericFunTemplate<'a>> {
    let source_cones = HashMap::<std::path::PathBuf, crate::cone::SourceConeInfo>::new();
    collect_explicit_top_level_generic_fun_templates_with_source_cones(
        stable_cone_key,
        index,
        compilation_unit,
        &source_cones,
    )
}

pub(in crate::hir::lower) fn collect_explicit_top_level_generic_fun_templates_with_source_cones<
    'a,
>(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &'a [(&'a SourceFile, &'a ast::File)],
    source_cones: &HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
) -> HashMap<TemplateKey, ExplicitTopLevelGenericFunTemplate<'a>> {
    let mut out = HashMap::new();
    for (source, file) in compilation_unit {
        let source_stable_cone_key =
            stable_cone_key_for_source(source, stable_cone_key, source_cones);
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Fun(fun) = item else {
                continue;
            };
            if fun.type_params.is_empty() && fun.eff_param.is_none() {
                continue;
            }
            let local_name = source.slice(fun.name.span);
            let fqn = if pkg_prefix.is_empty() {
                local_name.to_string()
            } else {
                format!("{pkg_prefix}.{local_name}")
            };
            out.insert(
                TemplateKey {
                    fqn: fqn.clone(),
                    source_path: source.path().to_path_buf(),
                    decl_span: fun.span,
                },
                ExplicitTopLevelGenericFunTemplate {
                    source,
                    file,
                    fun,
                    signature_key: canonical_generic_fun_signature_key(
                        source_stable_cone_key,
                        source,
                        file,
                        index,
                        &fqn,
                        &[],
                        fun,
                    ),
                    has_body: !matches!(fun.body, ast::FunBody::Missing),
                },
            );
        }
    }
    out
}

pub(crate) fn collect_generic_template_symbol_suffixes(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> GenericTemplateSymbolSuffixIndex {
    let stable_cone_key = virtual_stable_cone_key_for_compilation_unit(compilation_unit);
    collect_generic_template_symbol_suffixes_with_stable_cone_key(
        &stable_cone_key,
        index,
        compilation_unit,
    )
}

pub(in crate::hir::lower) fn collect_generic_template_symbol_suffixes_with_stable_cone_key(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> GenericTemplateSymbolSuffixIndex {
    let source_cones = HashMap::<std::path::PathBuf, crate::cone::SourceConeInfo>::new();
    collect_generic_template_symbol_suffixes_with_source_cones(
        stable_cone_key,
        index,
        compilation_unit,
        &source_cones,
    )
}

pub(in crate::hir::lower) fn collect_generic_template_symbol_suffixes_with_source_cones(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    source_cones: &HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
) -> GenericTemplateSymbolSuffixIndex {
    let mut candidates = Vec::new();

    for (template, info) in collect_explicit_top_level_generic_fun_templates_with_source_cones(
        stable_cone_key,
        index,
        compilation_unit,
        source_cones,
    ) {
        let source_stable_cone_key = stable_cone_key_for_source_path(
            template.source_path.as_path(),
            stable_cone_key,
            source_cones,
        );
        candidates.push(TemplateSymbolCandidate {
            stable_template_key: stable_template_key_for_template(
                source_stable_cone_key,
                &template,
                StableDefNamespace::Fun,
                generic_fun_decl_kind(info.fun),
                &info.signature_key,
            ),
            template,
            signature_key: info.signature_key,
            prefers_materialized_body: info.has_body,
        });
    }

    for (template, info) in collect_explicit_member_templates_with_source_cones(
        stable_cone_key,
        index,
        compilation_unit,
        source_cones,
    ) {
        let source_stable_cone_key = stable_cone_key_for_source_path(
            template.source_path.as_path(),
            stable_cone_key,
            source_cones,
        );
        let (signature_key, prefers_materialized_body, stable_template_key) = match info {
            ExplicitMemberTemplate::Fun {
                fun,
                signature_key,
                has_body,
                ..
            } => {
                let stable_template_key = stable_template_key_for_template(
                    source_stable_cone_key,
                    &template,
                    StableDefNamespace::Fun,
                    generic_fun_decl_kind(fun),
                    &signature_key,
                );
                (signature_key, has_body, stable_template_key)
            }
            ExplicitMemberTemplate::Getter {
                property,
                signature_key,
                has_body,
                ..
            } => {
                let stable_template_key = stable_template_key_for_template(
                    source_stable_cone_key,
                    &template,
                    StableDefNamespace::PropertyGetter,
                    generic_property_getter_decl_kind(property),
                    &signature_key,
                );
                (signature_key, has_body, stable_template_key)
            }
        };
        candidates.push(TemplateSymbolCandidate {
            stable_template_key,
            template,
            signature_key,
            prefers_materialized_body,
        });
    }

    let canonical_templates = canonical_template_map(&candidates);
    let mut canonical_stable_keys = HashMap::new();
    let mut aliases = HashMap::new();
    for candidate in candidates {
        let group_key = (
            candidate.template.fqn.clone(),
            candidate.signature_key.clone(),
        );
        let canonical = canonical_templates
            .get(&group_key)
            .cloned()
            .expect("every generic template candidate should resolve to a canonical template");
        canonical_stable_keys
            .entry(canonical.clone())
            .or_insert_with(|| candidate.stable_template_key.clone());
        aliases.insert(candidate.template, canonical);
    }

    let canonical_suffixes = build_template_symbol_suffixes(&canonical_stable_keys);
    let mut out = HashMap::new();
    for (template, canonical) in aliases {
        out.insert(
            template,
            canonical_suffixes
                .get(&canonical)
                .cloned()
                .unwrap_or_default(),
        );
    }
    out
}

pub(in crate::hir::lower) fn virtual_stable_cone_key_for_compilation_unit(
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> StableConeKey {
    compilation_unit
        .iter()
        .find_map(|(source, _)| {
            (!source.path().to_string_lossy().starts_with('<')).then_some(source)
        })
        .map(|source| StableConeKey::for_virtual_source_path(source.path()))
        .or_else(|| {
            compilation_unit
                .first()
                .map(|(source, _)| StableConeKey::for_virtual_source_path(source.path()))
        })
        .unwrap_or_else(|| StableConeKey::new("virtual-cone", "0.0.0"))
}

pub(in crate::hir::lower) fn stable_cone_key_for_source<'a>(
    source: &SourceFile,
    fallback: &'a StableConeKey,
    source_cones: &'a HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
) -> &'a StableConeKey {
    stable_cone_key_for_source_path(source.path(), fallback, source_cones)
}

pub(in crate::hir::lower) fn stable_cone_key_for_source_path<'a>(
    source_path: &std::path::Path,
    fallback: &'a StableConeKey,
    source_cones: &'a HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
) -> &'a StableConeKey {
    source_cones
        .get(source_path)
        .map(|info| &info.stable_key)
        .unwrap_or(fallback)
}

pub(in crate::hir::lower) fn stable_template_key_for_template(
    stable_cone_key: &StableConeKey,
    template: &TemplateKey,
    namespace: StableDefNamespace,
    declaration_kind: &str,
    signature_key: &str,
) -> StableTemplateKey {
    StableTemplateKey::new(StableDefKey::new(
        stable_cone_key.clone(),
        namespace,
        &template.fqn,
        declaration_kind,
        Some(signature_key.to_string()),
    ))
}

pub(in crate::hir::lower) fn generic_fun_decl_kind(fun: &ast::FunDecl) -> &'static str {
    match fun.kind {
        ast::FunDeclKind::Regular => "generic_fun",
        ast::FunDeclKind::EffectOp => "generic_effect_op",
    }
}

pub(in crate::hir::lower) fn generic_property_getter_decl_kind(
    _: &ast::PropertyDecl,
) -> &'static str {
    "generic_value_getter"
}

pub(in crate::hir::lower) fn canonical_template_map(
    candidates: &[TemplateSymbolCandidate],
) -> HashMap<(String, String), TemplateKey> {
    let mut groups: HashMap<(String, String), Vec<&TemplateSymbolCandidate>> = HashMap::new();
    for candidate in candidates {
        groups
            .entry((
                candidate.template.fqn.clone(),
                candidate.signature_key.clone(),
            ))
            .or_default()
            .push(candidate);
    }

    let mut out = HashMap::new();
    for (group_key, group_candidates) in groups {
        let preferred = preferred_template_candidate(group_candidates);
        out.insert(group_key, preferred.template.clone());
    }
    out
}

pub(in crate::hir::lower) fn preferred_template_candidate(
    group: Vec<&TemplateSymbolCandidate>,
) -> &TemplateSymbolCandidate {
    let mut preferred = group;
    preferred.sort_by(|lhs, rhs| {
        rhs.prefers_materialized_body
            .cmp(&lhs.prefers_materialized_body)
            .then_with(|| template_key_sort(&lhs.template, &rhs.template))
    });
    preferred
        .into_iter()
        .next()
        .expect("template candidate group must not be empty")
}

pub(in crate::hir::lower) fn re_intern_effect_row_from(
    types: &mut TypeStore,
    other: &TypeStore,
    row: &EffectRow,
) -> EffectRow {
    EffectRow::new(
        row.terms
            .iter()
            .map(|&ty| types.re_intern_from(other, ty))
            .collect(),
    )
}

pub(crate) fn stable_instance_fqn(
    types: &TypeStore,
    template: &TemplateKey,
    type_args: &[TypeId],
    eff_args: &[EffectRow],
    symbol_suffix: &str,
) -> String {
    if type_args.is_empty() && eff_args.is_empty() {
        return format!("{}{symbol_suffix}", template.fqn);
    }
    let mut args = type_args
        .iter()
        .map(|&ty| types.display(ty).to_string())
        .collect::<Vec<_>>();
    args.extend(
        eff_args
            .iter()
            .map(|row| format!("eff {}", stable_effect_row_string(types, row))),
    );
    format!("{}::<{}>{symbol_suffix}", template.fqn, args.join(", "))
}

pub(in crate::hir::lower) fn stable_effect_row_string(
    types: &TypeStore,
    row: &EffectRow,
) -> String {
    if row.terms.is_empty() {
        return "Pure".to_string();
    }
    row.terms
        .iter()
        .map(|&ty| types.display(ty).to_string())
        .collect::<Vec<_>>()
        .join(" + ")
}

pub(in crate::hir::lower) fn build_effect_binding(
    source: &SourceFile,
    fqn: &str,
    eff_param: &Option<ast::EffectRowParam>,
    eff_args: &[EffectRow],
) -> Result<Option<(String, EffectRow)>, crate::hir::HirLowerError> {
    match (eff_param, eff_args) {
        (None, []) => Ok(None),
        (Some(param), [row]) => Ok(Some((param.name.text(source).to_string(), row.clone()))),
        (None, found) => Err(explicit_instance_lowering_error(format!(
            "generic fun `{fqn}` 没有 effect row 参数，但实例请求提供了 {} 个 effect args",
            found.len()
        ))),
        (Some(_), found) => Err(explicit_instance_lowering_error(format!(
            "generic fun `{fqn}` 期望 1 个 effect row 参数，但实例请求提供了 {} 个 effect args",
            found.len()
        ))),
    }
}

pub(in crate::hir::lower) fn explicit_instance_lowering_error(
    message: impl Into<String>,
) -> crate::hir::HirLowerError {
    crate::hir::HirLowerError::Frontend {
        message: message.into(),
    }
}

pub(in crate::hir::lower) fn eval_value_only_enum_discriminant(
    source: &SourceFile,
    expr: &ast::Expr,
) -> Option<i128> {
    match &expr.kind {
        ast::ExprKind::IntLit => {
            let raw = source.slice(expr.span);
            let text: String = raw.chars().filter(|c| *c != '_').collect();
            text.parse::<i128>().ok()
        }
        ast::ExprKind::Unary {
            op: ast::UnaryOp::Neg,
            expr: inner,
            ..
        } => {
            let v = eval_value_only_enum_discriminant(source, inner)?;
            Some(-v)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cone::{ConeId, ConeKind, SourceConeInfo, SourceConeTrust};
    use crate::parser::parse_file;

    fn source_cone_info(id: u32, stable_key: StableConeKey) -> SourceConeInfo {
        SourceConeInfo {
            id: ConeId::new(id),
            kind: ConeKind::Lib,
            stable_key,
            trust: SourceConeTrust::Untrusted,
        }
    }

    #[test]
    fn generic_template_signatures_use_owning_source_cone_key() {
        let dep_source = SourceFile::new_virtual(
            "/tmp/scoop-template-keys/dep/src/lib.scoop",
            "package shared\nfun depId<T>(value: T): T = value\n",
        );
        let app_source = SourceFile::new_virtual(
            "/tmp/scoop-template-keys/app/src/main.scoop",
            "package shared\nfun appId<T>(value: T): T = value\n",
        );
        let dep_ast = parse_file(&dep_source).unwrap();
        let app_ast = parse_file(&app_source).unwrap();
        let index = Index::build(&[(&dep_source, &dep_ast), (&app_source, &app_ast)]).unwrap();
        let dep_key = StableConeKey::new("dep.cone", "0.1.0");
        let app_key = StableConeKey::new("app.cone", "0.1.0");
        let fallback_key = StableConeKey::new("fallback.cone", "0.1.0");
        let source_cones = HashMap::from([
            (
                dep_source.path().to_path_buf(),
                source_cone_info(2, dep_key),
            ),
            (
                app_source.path().to_path_buf(),
                source_cone_info(1, app_key),
            ),
        ]);

        let compilation_unit = [(&dep_source, &dep_ast), (&app_source, &app_ast)];
        let templates = collect_explicit_top_level_generic_fun_templates_with_source_cones(
            &fallback_key,
            &index,
            &compilation_unit,
            &source_cones,
        );
        let signature_keys = templates
            .values()
            .map(|template| template.signature_key.as_str())
            .collect::<Vec<_>>();

        assert!(signature_keys.iter().any(|key| key.contains("dep.cone")));
        assert!(signature_keys.iter().any(|key| key.contains("app.cone")));
        assert!(
            !signature_keys
                .iter()
                .any(|key| key.contains("fallback.cone"))
        );
    }
}
