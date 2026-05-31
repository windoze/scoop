//! HIR direct-call analysis: walks HIR blocks/statements/expressions to discover direct-call instances and pick the matching template signature for each call site.

use super::*;

#[derive(Clone)]
pub(super) struct HirDirectCallTemplateInfo {
    pub(super) template: TemplateKey,
    pub(super) type_param_names: Vec<String>,
    pub(super) params: Vec<CallableSignatureParam>,
    pub(super) has_effect_param: bool,
    pub(super) has_body: bool,
}

pub(super) fn collect_hir_direct_call_instance_requests(
    lowered_hir: &mut crate::hir::LoweredHir,
    typecheck_types: &TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
) -> HashMap<(PathBuf, Span), Vec<InstanceKey>> {
    let file_items = &lowered_hir.file.items;
    let member_funs = &lowered_hir.member_funs;
    let types = &mut lowered_hir.types;
    let mut templates_by_fqn: HashMap<String, Vec<HirDirectCallTemplateInfo>> = HashMap::new();
    for fun in file_items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(member_funs.iter())
    {
        let mut type_param_names = Vec::new();
        for param in &fun.params {
            collect_type_param_names_in_type(&*types, param.ty, &mut type_param_names);
        }
        collect_type_param_names_in_type(&*types, fun.return_ty, &mut type_param_names);
        let has_effect_param = function_type_has_effect_param(&*types, fun.ty);
        if type_param_names.is_empty() && !has_effect_param {
            continue;
        }
        let template = TemplateKey {
            fqn: fun.fqn.clone(),
            source_path: fun.source_path.clone(),
            decl_span: fun.span,
        };
        let entry = templates_by_fqn.entry(fun.fqn.clone()).or_default();
        if entry.iter().any(|existing| existing.template == template) {
            continue;
        }
        entry.push(HirDirectCallTemplateInfo {
            template,
            type_param_names,
            params: fun
                .params
                .iter()
                .map(|param| CallableSignatureParam {
                    name: param.name.clone(),
                    ty: param.ty,
                })
                .collect(),
            has_effect_param,
            has_body: fun.body.is_some(),
        });
    }

    let mut out = HashMap::new();
    for fun in file_items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(member_funs.iter())
    {
        let Some(body) = &fun.body else {
            continue;
        };
        let mut fun_instances = HashSet::new();
        collect_hir_direct_call_instances_in_block(
            body,
            &fun.source_path,
            &templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            &mut fun_instances,
        );
        if !fun_instances.is_empty() {
            out.insert(
                (fun.source_path.clone(), fun.span),
                fun_instances.into_iter().collect(),
            );
        }
    }

    out
}

pub(super) fn collect_hir_direct_call_instances_in_block(
    block: &crate::hir::Block,
    source_path: &Path,
    templates_by_fqn: &HashMap<String, Vec<HirDirectCallTemplateInfo>>,
    typecheck_types: &TypeStore,
    types: &mut TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    out: &mut HashSet<InstanceKey>,
) {
    for stmt in &block.stmts {
        collect_hir_direct_call_instances_in_stmt(
            stmt,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        );
    }
}

pub(super) fn collect_hir_direct_call_instances_in_stmt(
    stmt: &crate::hir::Stmt,
    source_path: &Path,
    templates_by_fqn: &HashMap<String, Vec<HirDirectCallTemplateInfo>>,
    typecheck_types: &TypeStore,
    types: &mut TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    out: &mut HashSet<InstanceKey>,
) {
    match &stmt.kind {
        crate::hir::StmtKind::Expr(expr) => collect_hir_direct_call_instances_in_expr(
            expr,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::StmtKind::Val(decl) => {
            if let Some(init) = &decl.init {
                collect_hir_direct_call_instances_in_expr(
                    init,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_hir_direct_call_instances_in_expr(
                lhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_expr(
                rhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::StmtKind::While { cond, body } => {
            collect_hir_direct_call_instances_in_expr(
                cond,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_block(
                body,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::StmtKind::Return { value } => {
            if let Some(value) = value {
                collect_hir_direct_call_instances_in_expr(
                    value,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::StmtKind::Empty
        | crate::hir::StmtKind::Break { .. }
        | crate::hir::StmtKind::Continue { .. }
        | crate::hir::StmtKind::Todo(_) => {}
    }
}

pub(super) fn collect_hir_direct_call_instances_in_expr(
    expr: &crate::hir::Expr,
    source_path: &Path,
    templates_by_fqn: &HashMap<String, Vec<HirDirectCallTemplateInfo>>,
    typecheck_types: &TypeStore,
    types: &mut TypeStore,
    top_level_fun_call_bindings: &HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    out: &mut HashSet<InstanceKey>,
) {
    match &expr.kind {
        crate::hir::ExprKind::Call { callee, args } => {
            collect_hir_direct_call_instances_in_expr(
                callee,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            for arg in args {
                match arg {
                    crate::hir::CallArg::Positional(value) => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                    crate::hir::CallArg::Named { value, .. } => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                }
            }

            let crate::hir::ExprKind::VarRef(crate::hir::ValueRef::TopLevel { fqn, .. }) =
                &callee.kind
            else {
                return;
            };
            let binding = top_level_fun_call_bindings.get(&(source_path.to_path_buf(), expr.span));
            let Some(candidates) = binding
                .and_then(|binding| templates_by_fqn.get(&binding.fqn))
                .or_else(|| templates_by_fqn.get(fqn))
            else {
                return;
            };
            if let Some(binding) = binding {
                let candidate =
                    choose_hir_direct_call_template_for_binding(candidates, binding, &*types)
                        .or_else(|| choose_hir_direct_call_template(candidates, &*types));
                if let Some(candidate) = candidate {
                    let type_args = binding
                        .type_args
                        .iter()
                        .map(|&ty| {
                            if binding.types_are_hir {
                                ty
                            } else {
                                types.re_intern_from(typecheck_types, ty)
                            }
                        })
                        .collect::<Vec<_>>();
                    let eff_args = binding
                        .eff_args
                        .iter()
                        .map(|row| {
                            if binding.types_are_hir {
                                row.clone()
                            } else {
                                re_intern_effect_row_from(types, typecheck_types, row)
                            }
                        })
                        .collect::<Vec<_>>();
                    if !type_args.is_empty() || !eff_args.is_empty() {
                        let instance = InstanceKey {
                            template: candidate.template.clone(),
                            type_args,
                            eff_args,
                        };
                        if instance_request_is_concrete(
                            types,
                            &instance.type_args,
                            &instance.eff_args,
                        ) {
                            out.insert(instance);
                        }
                    }
                    return;
                }
            }

            let Some(candidate) = choose_hir_direct_call_template(candidates, &*types) else {
                return;
            };
            if candidate.has_effect_param || candidate.type_param_names.is_empty() {
                return;
            }

            let Some(arg_to_param) = map_hir_call_args_to_signature_params(&candidate.params, args)
            else {
                return;
            };
            let mut bindings = HashMap::new();
            for (arg_idx, param_idx) in arg_to_param.into_iter().enumerate() {
                let Some(param) = candidate.params.get(param_idx) else {
                    return;
                };
                if !type_contains_param(types, param.ty) {
                    continue;
                }
                let arg_ty = match args.get(arg_idx) {
                    Some(crate::hir::CallArg::Positional(value)) => value.ty,
                    Some(crate::hir::CallArg::Named { value, .. }) => value.ty,
                    None => return,
                };
                if type_contains_param(types, arg_ty) {
                    return;
                }
                collect_type_param_bindings(types, param.ty, arg_ty, &mut bindings);
            }

            let mut ordered = Vec::with_capacity(candidate.type_param_names.len());
            for name in &candidate.type_param_names {
                let Some(ty) = bindings.get(name).copied() else {
                    return;
                };
                if type_contains_param(types, ty) {
                    return;
                }
                ordered.push(ty);
            }
            if ordered.is_empty() {
                return;
            }

            let instance = InstanceKey {
                template: candidate.template.clone(),
                type_args: ordered,
                eff_args: Vec::new(),
            };
            if instance_request_is_concrete(types, &instance.type_args, &instance.eff_args) {
                out.insert(instance);
            }
        }
        crate::hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_hir_direct_call_instances_in_expr(
                    &field.value,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_hir_direct_call_instances_in_expr(
                    element,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_hir_direct_call_instances_in_expr(
                        expr,
                        source_path,
                        templates_by_fqn,
                        typecheck_types,
                        types,
                        top_level_fun_call_bindings,
                        out,
                    );
                }
            }
        }
        crate::hir::ExprKind::Unary { expr, .. } => collect_hir_direct_call_instances_in_expr(
            expr,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_hir_direct_call_instances_in_expr(
                lhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_expr(
                rhs,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::ExprKind::TypeCheck { expr, .. } | crate::hir::ExprKind::Cast { expr, .. } => {
            collect_hir_direct_call_instances_in_expr(
                expr,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
        }
        crate::hir::ExprKind::Block(block) => collect_hir_direct_call_instances_in_block(
            block,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::ExprKind::Closure(closure) => collect_hir_direct_call_instances_in_expr(
            &closure.body,
            source_path,
            templates_by_fqn,
            typecheck_types,
            types,
            top_level_fun_call_bindings,
            out,
        ),
        crate::hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_hir_direct_call_instances_in_expr(
                cond,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            collect_hir_direct_call_instances_in_expr(
                then_branch,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            if let Some(else_branch) = else_branch {
                collect_hir_direct_call_instances_in_expr(
                    else_branch,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::When { subject, arms } => {
            collect_hir_direct_call_instances_in_expr(
                subject,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_hir_direct_call_instances_in_expr(
                        guard,
                        source_path,
                        templates_by_fqn,
                        typecheck_types,
                        types,
                        top_level_fun_call_bindings,
                        out,
                    );
                }
                collect_hir_direct_call_instances_in_expr(
                    &arm.body,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_hir_direct_call_instances_in_expr(
                receiver,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            )
        }
        crate::hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    crate::hir::CallArg::Positional(value) => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                    crate::hir::CallArg::Named { value, .. } => {
                        collect_hir_direct_call_instances_in_expr(
                            value,
                            source_path,
                            templates_by_fqn,
                            typecheck_types,
                            types,
                            top_level_fun_call_bindings,
                            out,
                        )
                    }
                }
            }
        }
        crate::hir::ExprKind::Handle(handle) => {
            collect_hir_direct_call_instances_in_block(
                &handle.body,
                source_path,
                templates_by_fqn,
                typecheck_types,
                types,
                top_level_fun_call_bindings,
                out,
            );
            for arm in &handle.arms {
                collect_hir_direct_call_instances_in_expr(
                    &arm.body,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
            if let Some(finally) = &handle.finally {
                collect_hir_direct_call_instances_in_block(
                    finally,
                    source_path,
                    templates_by_fqn,
                    typecheck_types,
                    types,
                    top_level_fun_call_bindings,
                    out,
                );
            }
        }
        crate::hir::ExprKind::Missing
        | crate::hir::ExprKind::Literal(_)
        | crate::hir::ExprKind::VarRef(_)
        | crate::hir::ExprKind::UnresolvedIdent { .. }
        | crate::hir::ExprKind::ClassLiteral(_)
        | crate::hir::ExprKind::Todo(_) => {}
    }
}

pub(super) fn choose_hir_direct_call_template_for_binding<'a>(
    candidates: &'a [HirDirectCallTemplateInfo],
    binding: &ast::TopLevelFunCallBinding,
    types: &TypeStore,
) -> Option<&'a HirDirectCallTemplateInfo> {
    let chosen = candidates.iter().find(|candidate| {
        candidate.template.source_path == binding.decl_file
            && candidate.template.decl_span == binding.decl_span
    })?;
    let mut preferred = candidates
        .iter()
        .filter(|candidate| {
            hir_direct_call_templates_have_same_signature(candidate, chosen, types)
                && candidate.has_body
        })
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred.extend(candidates.iter().filter(|candidate| {
            hir_direct_call_templates_have_same_signature(candidate, chosen, types)
        }));
    }
    preferred.into_iter().min_by(|lhs, rhs| {
        lhs.template
            .source_path
            .cmp(&rhs.template.source_path)
            .then_with(|| {
                lhs.template
                    .decl_span
                    .start
                    .cmp(&rhs.template.decl_span.start)
            })
            .then_with(|| lhs.template.decl_span.end.cmp(&rhs.template.decl_span.end))
    })
}

pub(super) fn hir_direct_call_templates_have_same_signature(
    lhs: &HirDirectCallTemplateInfo,
    rhs: &HirDirectCallTemplateInfo,
    types: &TypeStore,
) -> bool {
    lhs.type_param_names == rhs.type_param_names
        && lhs.has_effect_param == rhs.has_effect_param
        && lhs.params.len() == rhs.params.len()
        && lhs.params.iter().zip(rhs.params.iter()).all(|(lhs, rhs)| {
            lhs.name == rhs.name
                && types.display(lhs.ty).to_string() == types.display(rhs.ty).to_string()
        })
}

pub(super) fn map_hir_call_args_to_signature_params(
    params: &[CallableSignatureParam],
    args: &[crate::hir::CallArg],
) -> Option<Vec<usize>> {
    let mut used = vec![false; params.len()];
    let mut next_pos = 0;
    let mut out = Vec::with_capacity(args.len());

    for arg in args {
        let param_idx = match arg {
            crate::hir::CallArg::Named { name, .. } => params
                .iter()
                .enumerate()
                .find_map(|(idx, param)| (!used[idx] && param.name == *name).then_some(idx))?,
            crate::hir::CallArg::Positional(_) => {
                while used.get(next_pos).copied().unwrap_or(false) {
                    next_pos += 1;
                }
                let idx = next_pos;
                if idx >= params.len() {
                    return None;
                }
                next_pos += 1;
                idx
            }
        };
        used[param_idx] = true;
        out.push(param_idx);
    }

    Some(out)
}

pub(super) fn choose_hir_direct_call_template<'a>(
    candidates: &'a [HirDirectCallTemplateInfo],
    types: &TypeStore,
) -> Option<&'a HirDirectCallTemplateInfo> {
    let first = candidates.first()?;
    let same_signature = candidates
        .iter()
        .skip(1)
        .all(|candidate| hir_direct_call_templates_have_same_signature(candidate, first, types));
    if !same_signature {
        return None;
    }

    let mut preferred = candidates
        .iter()
        .filter(|candidate| candidate.has_body)
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred.extend(candidates.iter());
    }
    preferred.into_iter().min_by(|lhs, rhs| {
        lhs.template
            .source_path
            .cmp(&rhs.template.source_path)
            .then_with(|| {
                lhs.template
                    .decl_span
                    .start
                    .cmp(&rhs.template.decl_span.start)
            })
            .then_with(|| lhs.template.decl_span.end.cmp(&rhs.template.decl_span.end))
    })
}
