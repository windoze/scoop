//! Nominal constructor overload selection and instantiation.

#![allow(dead_code)]

use super::*;

pub(in crate::typecheck::expr) fn is_ctor_visible_from(
    use_cone: ConeId,
    use_source: &SourceFile,
    ctor: &ConstructorOverload,
) -> bool {
    match ctor.visibility {
        Visibility::Public => true,
        Visibility::Internal => ctor.decl_cone == use_cone,
        Visibility::Private => ctor.decl_file.as_path() == use_source.path(),
    }
}

#[derive(Debug, Clone)]
pub(in crate::typecheck::expr) struct MatchedCtorOverload {
    pub(in crate::typecheck::expr) owner_fqn: String,
    pub(in crate::typecheck::expr) ctor_span: Option<Span>,
    pub(in crate::typecheck::expr) arg_mapping: Vec<Option<usize>>,
    /// `call_args[arg_idx]` 对应的"期望类型"。
    pub(in crate::typecheck::expr) expected_arg_tys: Vec<TypeId>,
    /// 调用点需要用默认值补齐的形参个数（越少越"具体"）。
    pub(in crate::typecheck::expr) defaults_used: usize,
    /// 用于歧义诊断打印的 ctor 签名（稳定排序后展示）。
    pub(in crate::typecheck::expr) signature: String,
    /// T0125：从实参类型推断出的泛型 type args（按声明顺序）。
    pub(in crate::typecheck::expr) inferred_type_args: Vec<TypeId>,
}

type InstantiatedCtorParamTypes = (Vec<TypeId>, Vec<TypeId>);

pub(super) struct CtorParamInstantiationRequest<'a> {
    param_tys: &'a [TypeId],
    type_param_names: &'a [String],
    decl_file: &'a std::path::Path,
    mapping: &'a [Option<usize>],
    call_args: &'a [CallArgInfo<'a>],
    builtins: BuiltinTypes,
    call_span: Span,
    /// 显式构造器 type args（`Container<Int>(...)` 中的 `[Int]`）（P4-T01h）。
    ///
    /// - 若提供，长度必须与 `type_param_names` 一致；
    /// - 优先级：显式 > arg-driven 反推 > LHS expected > `Any`；
    /// - 与 arg-driven 反推冲突时返回 `Ok(None)`，由调用点退化为 "no match"。
    explicit_type_args: Option<&'a [TypeId]>,
    /// LHS expected type 的 owner-args（`val c: Container<Int> = Container()` 中的 `[Int]`）（P4-T01h）。
    ///
    /// - 仅当外层 expected 类型是同 FQN 的 nominal generic instantiation 时由调用点提供；
    /// - 长度必须与 `type_param_names` 一致；
    /// - 仅作为 arg-driven 反推未填充时的兜底候选，不主动覆盖 arg-driven 结果（与 explicit 不同）。
    expected_owner_args: Option<&'a [TypeId]>,
}

pub(super) fn is_strictly_more_specific_ctor_overload(
    a: &MatchedCtorOverload,
    b: &MatchedCtorOverload,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    let a_le_b = a
        .expected_arg_tys
        .iter()
        .zip(b.expected_arg_tys.iter())
        .all(|(a_ty, b_ty)| is_type_assignable(*a_ty, *b_ty, lower, builtins));
    let b_le_a = b
        .expected_arg_tys
        .iter()
        .zip(a.expected_arg_tys.iter())
        .all(|(b_ty, a_ty)| is_type_assignable(*b_ty, *a_ty, lower, builtins));

    a_le_b && !b_le_a
}

pub(in crate::typecheck::expr) fn pick_most_specific_ctor_overload(
    candidates: &[MatchedCtorOverload],
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<usize> {
    for (idx, cand) in candidates.iter().enumerate() {
        let mut ok = true;
        for (other_idx, other) in candidates.iter().enumerate() {
            if idx == other_idx {
                continue;
            }
            if !is_strictly_more_specific_ctor_overload(cand, other, lower, builtins) {
                ok = false;
                break;
            }
        }
        if ok {
            return Some(idx);
        }
    }

    let min_defaults = candidates
        .iter()
        .map(|c| c.defaults_used)
        .min()
        .unwrap_or(0);
    let mut it = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.defaults_used == min_defaults);
    let (idx, _) = it.next()?;
    if it.next().is_some() {
        return None;
    }
    Some(idx)
}

pub(super) fn instantiate_ctor_param_tys(
    request: CtorParamInstantiationRequest<'_>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<InstantiatedCtorParamTypes>, ExprTypeError> {
    let CtorParamInstantiationRequest {
        param_tys,
        type_param_names,
        decl_file,
        mapping,
        call_args,
        builtins,
        call_span,
        explicit_type_args,
        expected_owner_args,
    } = request;

    if type_param_names.is_empty() {
        return Ok(Some((Vec::new(), param_tys.to_vec())));
    }

    // 显式 type-args / LHS expected owner-args 的长度必须严格匹配 `type_param_names`，
    // 否则视为不匹配（让调用点退化到 NoMatchingOverload 路径，沿用现有诊断）。
    if let Some(explicit) = explicit_type_args
        && explicit.len() != type_param_names.len()
    {
        return Ok(None);
    }
    if let Some(expected) = expected_owner_args
        && expected.len() != type_param_names.len()
    {
        return Ok(None);
    }

    let fresh_type_params: Vec<TypeId> = type_param_names
        .iter()
        .cloned()
        .map(|name| lower.ty_param_named(name, decl_file.to_path_buf(), Span::new(0, 0)))
        .collect();

    let mut inferred: HashMap<TypeId, TypeId> = HashMap::new();
    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
        let Some(arg_idx) = arg_idx else {
            continue;
        };
        let Some(expected_ty) = param_tys.get(param_idx).copied() else {
            return Ok(None);
        };
        let Some(arg) = call_args.get(arg_idx) else {
            return Ok(None);
        };
        let found_is_placeholder = matches!(arg.expr.kind, ast::ExprKind::Lambda(_));

        for param_ty in fresh_type_params.iter().copied() {
            let mut candidates: Vec<TypeId> = Vec::new();
            collect_type_arg_candidates_for_single_type_param(
                expected_ty,
                arg.ty,
                param_ty,
                &mut candidates,
                lower,
                builtins,
                found_is_placeholder,
            );

            for candidate in candidates {
                match inferred.get(&param_ty).copied() {
                    None => {
                        inferred.insert(param_ty, candidate);
                    }
                    Some(bound) if bound == candidate => {}
                    Some(_) => return Ok(None),
                }
            }
        }
    }

    // P4-T01h：合并显式 type args / LHS expected owner-args。
    //
    // 优先级：显式 > arg-driven 反推 > LHS expected > `Any`。
    // - 显式 type args 与 arg-driven 结果若不一致 → 视为不匹配，让调用点报 NoMatchingOverload；
    // - LHS expected 仅在 arg-driven 没填出来时生效，不主动覆盖 arg-driven。
    let mut inferred_type_args: Vec<TypeId> = Vec::with_capacity(fresh_type_params.len());
    for (idx, param_ty) in fresh_type_params.iter().copied().enumerate() {
        let arg_inferred = inferred.get(&param_ty).copied();
        let explicit_at = explicit_type_args.and_then(|e| e.get(idx).copied());
        let expected_at = expected_owner_args.and_then(|e| e.get(idx).copied());

        let chosen = if let Some(t) = explicit_at {
            if let Some(bound) = arg_inferred
                && bound != t
            {
                return Ok(None);
            }
            t
        } else if let Some(t) = arg_inferred {
            t
        } else if let Some(t) = expected_at {
            t
        } else {
            builtins.any
        };
        inferred_type_args.push(chosen);
    }

    let mut instantiated_param_tys = param_tys.to_vec();
    for (param_ty, arg_ty) in fresh_type_params
        .iter()
        .copied()
        .zip(inferred_type_args.iter().copied())
    {
        for expected_ty in &mut instantiated_param_tys {
            *expected_ty =
                substitute_single_type_param(*expected_ty, param_ty, arg_ty, lower, call_span)?;
        }
    }

    Ok(Some((inferred_type_args, instantiated_param_tys)))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::typecheck::expr) fn collect_matched_ctor_overloads_for_owner(
    inputs: ExprInferInputs<'_>,
    owner_fqn: &str,
    call_span: Span,
    callee_for_diag: &str,
    call_args: &[CallArgInfo<'_>],
    exclude_ctor_span: Option<Span>,
    explicit_type_args: Option<&[TypeId]>,
    expected_owner_args: Option<&[TypeId]>,
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<MatchedCtorOverload>, ExprTypeError> {
    let builtins = inputs.builtins;
    let source = inputs.source;
    let use_cone = lower.index().cone_of_source(source);

    if call_args.iter().any(|arg| arg.is_spread) {
        let span = call_args
            .iter()
            .find(|arg| arg.is_spread)
            .map(|arg| arg.expr.span)
            .unwrap_or(call_span);
        return Err(ExprTypeError::SpreadArgRequiresVararg {
            callee: callee_for_diag.to_string(),
            span: span.into(),
        });
    }

    let Some(ctors) = lower.index().constructors.get(owner_fqn).cloned() else {
        return Ok(Vec::new());
    };

    let mut visible: Vec<&ConstructorOverload> = ctors
        .iter()
        .filter(|ctor| is_ctor_visible_from(use_cone, source, ctor))
        .collect();
    if let Some(exclude) = exclude_ctor_span {
        visible.retain(|ctor| ctor.span != exclude);
    }

    check_call_named_args_exist_in_any_candidate(
        callee_for_diag,
        call_args,
        visible
            .iter()
            .map(|ctor| {
                ctor.params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .iter()
            .map(|names| names.as_slice()),
    )?;

    let type_param_names: Vec<String> = lower
        .env()
        .type_symbol(owner_fqn)
        .map(|sym| sym.type_param_names.clone())
        .unwrap_or_default();

    let mut matched: Vec<MatchedCtorOverload> = Vec::new();
    for ctor in visible {
        let param_names: Vec<String> = ctor.params.iter().map(|p| p.name.clone()).collect();
        let param_has_defaults: Vec<bool> = ctor.params.iter().map(|p| p.has_default).collect();

        let Some(mapping) =
            map_call_args_to_params_with_defaults(call_args, &param_names, &param_has_defaults)
        else {
            continue;
        };

        let mut param_tys: Vec<TypeId> = Vec::with_capacity(ctor.params.len());
        let mut param_ty_strs: Vec<String> = Vec::with_capacity(ctor.params.len());
        let mut ok = true;
        for p in &ctor.params {
            let Some(ty_ref) = p.ty.as_ref() else {
                ok = false;
                break;
            };
            let ty = lower.lower_type_ref_in_decl_file_with_fresh_type_params(
                &ctor.decl_file,
                &type_param_names,
                ty_ref,
            )?;
            param_tys.push(ty);
            param_ty_strs.push(lower.fmt_type(ty));
        }
        if !ok {
            continue;
        }

        let Some((inferred_type_args, instantiated_param_tys)) = instantiate_ctor_param_tys(
            CtorParamInstantiationRequest {
                param_tys: &param_tys,
                type_param_names: &type_param_names,
                decl_file: &ctor.decl_file,
                mapping: &mapping,
                call_args,
                builtins,
                call_span,
                explicit_type_args,
                expected_owner_args,
            },
            lower,
        )?
        else {
            continue;
        };

        let mut expected_arg_tys = vec![builtins.nothing; call_args.len()];
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                continue;
            };
            let Some(expected_ty) = instantiated_param_tys.get(param_idx).copied() else {
                ok = false;
                break;
            };

            expected_arg_tys[arg_idx] = expected_ty;
            let arg = &call_args[arg_idx];
            let found_ty = if arg.needs_expected_type {
                inputs.infer_in_expected(
                    lower,
                    arg.expr,
                    expected_ty,
                    ExpectedTypeFrom::new(format!(
                        "`{}` 的第 {} 个构造参数 `{}`",
                        callee_for_diag,
                        param_idx + 1,
                        param_names[param_idx]
                    )),
                )?
            } else {
                arg.ty
            };

            if is_type_assignable(found_ty, expected_ty, lower, builtins)
                || literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins)
            {
                continue;
            }

            ok = false;
            break;
        }

        if !ok {
            continue;
        }

        let defaults_used = mapping.iter().filter(|arg_idx| arg_idx.is_none()).count();
        matched.push(MatchedCtorOverload {
            owner_fqn: owner_fqn.to_string(),
            ctor_span: Some(ctor.span),
            arg_mapping: mapping,
            expected_arg_tys,
            defaults_used,
            signature: format!("{owner_fqn}({})", param_ty_strs.join(", ")),
            inferred_type_args,
        });
    }

    Ok(matched)
}

pub(in crate::typecheck::expr) fn select_ctor_overload_for_owner(
    inputs: ExprInferInputs<'_>,
    owner_fqn: &str,
    call_span: Span,
    callee_for_diag: &str,
    call_args: &[CallArgInfo<'_>],
    exclude_ctor_span: Option<Span>,
    lower: &mut TypeLowering<'_>,
) -> Result<MatchedCtorOverload, ExprTypeError> {
    let mut matched = collect_matched_ctor_overloads_for_owner(
        inputs,
        owner_fqn,
        call_span,
        callee_for_diag,
        call_args,
        exclude_ctor_span,
        None,
        None,
        lower,
    )?;

    if matched.is_empty() {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_for_diag.to_string(),
            span: call_span.into(),
        });
    }
    if matched.len() == 1 {
        return Ok(matched.pop().expect("len == 1"));
    }

    let Some(idx) = pick_most_specific_ctor_overload(&matched, lower, inputs.builtins) else {
        let candidates =
            join_overload_signatures(matched.iter().map(|m| m.signature.clone()).collect());
        return Err(ExprTypeError::AmbiguousOverload {
            callee: callee_for_diag.to_string(),
            candidates,
            span: call_span.into(),
        });
    };

    Ok(matched.swap_remove(idx))
}

/// P4-T01h：在 LHS expected nominal type 已知时尝试 ctor 调用推导。
///
/// 形态要求：
/// - `expr` 是 `Call`（或 `Call(TypeApply, ...)`）；
/// - 其 callee（透明展开 `TypeApply` 后）是一个 resolver 阶段尚未绑定为顶层值的 `Ident`
///   ——这样 [`infer_nominal_constructor_call_expr_type`] 才会接管；
/// - `expected_ty` 是 class/struct nominal 的 generic instantiation。
///
/// 命中后把 LHS expected 的 type-args 作为兜底候选喂给 ctor solver；与 ctor arg 反推
/// 结果不冲突时优先使用 arg 反推，仅在 arg 完全没填出来时才用 LHS 兜底。
pub(in crate::typecheck::expr) fn try_infer_nominal_constructor_call_expr_type_with_expected(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let ast::ExprKind::Call { callee, args } = &expr.kind else {
        return Ok(None);
    };

    // 透明展开 callee（`Container<Int>(...)` 中的显式 type-args 在外层 dispatch 已经被消费）。
    let mut explicit_type_args: Option<Vec<TypeId>> = None;
    let inner_callee: &ast::Expr = match &callee.kind {
        ast::ExprKind::TypeApply {
            callee: inner,
            args,
        } => {
            let lowered = lower_explicit_type_apply_args(args, lower)?;
            explicit_type_args = Some(lowered.type_args);
            inner.as_ref()
        }
        _ => callee.as_ref(),
    };

    let ast::ExprKind::Ident(id) = &inner_callee.kind else {
        return Ok(None);
    };
    if id.resolved.is_some() {
        return Ok(None);
    }

    let (expected_fqn, expected_args) = match lower.type_kind(expected_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
        | TypeKind::Ref(RefTypeKind::Nominal(nominal)) => (nominal.fqn, nominal.args),
        _ => return Ok(None),
    };
    if expected_args.is_empty() {
        return Ok(None);
    }

    let expected_owner_args: Option<(&str, &[TypeId])> =
        Some((expected_fqn.as_str(), expected_args.as_slice()));

    infer_nominal_constructor_call_expr_type(
        inputs,
        expr,
        id,
        args,
        explicit_type_args.as_deref(),
        expected_owner_args,
        lower,
    )
}

pub(super) fn try_infer_qualified_nominal_constructor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee: &ast::Expr,
    args: &[ast::Expr],
    explicit_type_args: Option<&[TypeId]>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let mut segments = Vec::new();
    if !collect_member_access_path(inputs.source, callee, &mut segments) || segments.len() < 2 {
        return Ok(None);
    }

    let use_span = segments
        .last()
        .map(|(_, span)| *span)
        .unwrap_or(callee.span);
    let segment_names = segments
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let owner_fqn = match lower.resolve_type_path_fqn_by_name(&segment_names, use_span) {
        Ok(fqn) => fqn,
        Err(_) => return Ok(None),
    };
    let Some(TypeSymbolKind::Nominal(ast::TypeKind::Class | ast::TypeKind::Struct)) =
        lower.env().type_symbol(&owner_fqn).map(|sym| sym.kind)
    else {
        return Ok(None);
    };

    let call_args = collect_call_arg_infos_allow_expected_type_placeholders(inputs, args, lower)?;
    let callee_name = segment_names.join(".");
    check_call_arg_named_rules(&callee_name, &call_args)?;

    if call_args_have_named(&call_args) {
        let mut all_names: HashSet<String> = HashSet::new();
        let use_cone = lower.index().cone_of_source(inputs.source);
        if let Some(ctors) = lower.index().constructors.get(&owner_fqn) {
            for ctor in ctors
                .iter()
                .filter(|ctor| is_ctor_visible_from(use_cone, inputs.source, ctor))
            {
                for p in &ctor.params {
                    all_names.insert(p.name.clone());
                }
            }
        }
        for arg in &call_args {
            let CallArgKind::Named { name, name_span } = &arg.kind else {
                continue;
            };
            if !all_names.contains(name) {
                return Err(ExprTypeError::UnknownCallArgName {
                    callee: callee_name.clone(),
                    name: name.clone(),
                    span: (*name_span).into(),
                });
            }
        }
    }

    let mut matched = collect_matched_ctor_overloads_for_owner(
        inputs,
        &owner_fqn,
        call_expr.span,
        &callee_name,
        &call_args,
        None,
        explicit_type_args,
        None,
        lower,
    )?;

    if matched.is_empty() {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_name,
            span: call_expr.span.into(),
        });
    }
    let chosen = if matched.len() == 1 {
        matched.pop().expect("len == 1")
    } else {
        let Some(idx) = pick_most_specific_ctor_overload(&matched, lower, inputs.builtins) else {
            let candidates =
                join_overload_signatures(matched.iter().map(|m| m.signature.clone()).collect());
            return Err(ExprTypeError::AmbiguousOverload {
                callee: callee_name,
                candidates,
                span: call_expr.span.into(),
            });
        };
        matched.swap_remove(idx)
    };

    lower.record_typechecked_ctor_call_binding(
        call_expr.span,
        chosen.owner_fqn.clone(),
        chosen.ctor_span,
        chosen.arg_mapping.clone(),
    );
    if let Some(binding) = call_arg_binding_from_optional_mapping(&chosen.arg_mapping, &call_args) {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }
    let ty =
        lower.lower_type_fqn_with_args(chosen.owner_fqn, chosen.inferred_type_args, use_span)?;
    Ok(Some(ty))
}

fn collect_member_access_path(
    source: &SourceFile,
    expr: &ast::Expr,
    out: &mut Vec<(String, Span)>,
) -> bool {
    match &expr.kind {
        ast::ExprKind::Ident(id) => {
            out.push((source.slice(id.span).to_string(), id.span));
            true
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            if !collect_member_access_path(source, receiver, out) {
                return false;
            }
            out.push((source.slice(member.span).to_string(), member.span));
            true
        }
        _ => false,
    }
}

pub(super) fn infer_nominal_constructor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee: &ast::ValueIdent,
    args: &[ast::Expr],
    explicit_type_args: Option<&[TypeId]>,
    expected_owner_args: Option<(&str, &[TypeId])>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;

    let Some(call) = callee.call.as_ref() else {
        return Ok(None);
    };

    let mut ctor_owners: Vec<(String, ast::TypeKind)> = call
        .candidates
        .iter()
        .filter_map(|candidate| {
            let ast::CallCandidate::Constructor { ty_fqn } = candidate else {
                return None;
            };
            match lower.env().type_symbol(ty_fqn).map(|sym| sym.kind) {
                Some(TypeSymbolKind::Nominal(
                    kind @ (ast::TypeKind::Class | ast::TypeKind::Struct),
                )) => Some((ty_fqn.clone(), kind)),
                _ => None,
            }
        })
        .collect();
    ctor_owners.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));
    ctor_owners.dedup_by(|(lhs, _), (rhs, _)| lhs == rhs);

    if ctor_owners.is_empty() {
        return Ok(None);
    }

    let call_args = collect_call_arg_infos_allow_expected_type_placeholders(inputs, args, lower)?;
    let use_cone = lower.index().cone_of_source(source);
    let callee_name = source.slice(callee.span).to_string();
    check_call_arg_named_rules(&callee_name, &call_args)?;

    if call_args_have_named(&call_args) {
        let mut all_names: HashSet<String> = HashSet::new();
        for (owner_fqn, _) in &ctor_owners {
            let Some(ctors) = lower.index().constructors.get(owner_fqn) else {
                continue;
            };
            for ctor in ctors
                .iter()
                .filter(|c| is_ctor_visible_from(use_cone, source, c))
            {
                for p in &ctor.params {
                    all_names.insert(p.name.clone());
                }
            }
        }

        for arg in &call_args {
            let CallArgKind::Named { name, name_span } = &arg.kind else {
                continue;
            };
            if !all_names.contains(name) {
                return Err(ExprTypeError::UnknownCallArgName {
                    callee: callee_name.clone(),
                    name: name.clone(),
                    span: (*name_span).into(),
                });
            }
        }
    }

    let mut matched: Vec<MatchedCtorOverload> = Vec::new();
    for (owner_fqn, _) in &ctor_owners {
        let owner_expected_args = expected_owner_args
            .filter(|(fqn, _)| *fqn == owner_fqn.as_str())
            .map(|(_, args)| args);
        matched.extend(collect_matched_ctor_overloads_for_owner(
            inputs,
            owner_fqn,
            call_expr.span,
            &callee_name,
            &call_args,
            None,
            explicit_type_args,
            owner_expected_args,
            lower,
        )?);
    }

    if matched.is_empty() {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_name,
            span: call_expr.span.into(),
        });
    }
    let chosen = if matched.len() == 1 {
        matched.pop().expect("len == 1")
    } else {
        let Some(idx) = pick_most_specific_ctor_overload(&matched, lower, builtins) else {
            let candidates =
                join_overload_signatures(matched.iter().map(|m| m.signature.clone()).collect());
            return Err(ExprTypeError::AmbiguousOverload {
                callee: callee_name,
                candidates,
                span: call_expr.span.into(),
            });
        };
        matched.swap_remove(idx)
    };

    lower.record_typechecked_ctor_call_binding(
        call_expr.span,
        chosen.owner_fqn.clone(),
        chosen.ctor_span,
        chosen.arg_mapping.clone(),
    );
    if let Some(binding) = call_arg_binding_from_optional_mapping(&chosen.arg_mapping, &call_args) {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }
    let ty =
        lower.lower_type_fqn_with_args(chosen.owner_fqn, chosen.inferred_type_args, callee.span)?;
    Ok(Some(ty))
}

pub(super) fn resolves_to_compiler_owned_continuation_type(
    callee_name: &str,
    use_span: Span,
    lower: &TypeLowering<'_>,
) -> bool {
    lower
        .resolve_type_path_fqn_by_name(&[callee_name.to_string()], use_span)
        .ok()
        .as_deref()
        == Some("scoop.core.Continuation")
}
