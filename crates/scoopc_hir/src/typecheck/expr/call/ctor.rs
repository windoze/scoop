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
    pub(in crate::typecheck::expr) arg_mapping: Vec<ParamArgBinding>,
    /// `call_args[arg_idx]` 对应的"期望类型"。
    pub(in crate::typecheck::expr) expected_arg_tys: Vec<TypeId>,
    /// 用于歧义诊断打印的 ctor 签名（稳定排序后展示）。
    pub(in crate::typecheck::expr) signature: String,
    /// Phase D specificity 使用的声明处 effective 参数类型。
    pub(in crate::typecheck::expr) specificity: SpecificityCandidate,
    /// T0125：从实参类型推断出的泛型 type args（按声明顺序）。
    pub(in crate::typecheck::expr) inferred_type_args: Vec<TypeId>,
    /// owner `eff` 参数的具体 row（若该 class/struct 声明了 owner effect 参数）。
    pub(in crate::typecheck::expr) inferred_eff_arg: Option<EffectRow>,
}

type InstantiatedCtorParamTypes = (Vec<TypeId>, Vec<TypeId>);

struct CtorTypeParamContext {
    owner_type_param_count: usize,
    type_params: Vec<TypeId>,
    bindings: Vec<(String, TypeId)>,
    eff_bindings: Vec<(String, EffectRow)>,
    owner_eff_arg: Option<EffectRow>,
    where_constraints: Vec<FunWhereConstraintInfo>,
}

pub(in crate::typecheck::expr) fn collect_ctor_owner_fqns_from_call_candidates(
    call: Option<&ast::ResolvedCall>,
    lower: &TypeLowering<'_>,
) -> Vec<String> {
    let mut owners: Vec<String> = call
        .into_iter()
        .flat_map(|call| call.candidates.iter())
        .filter_map(|candidate| {
            let ast::CallCandidate::Constructor { ty_fqn } = candidate else {
                return None;
            };
            match lower.env().type_symbol(ty_fqn).map(|sym| sym.kind) {
                Some(TypeSymbolKind::Nominal(ast::TypeKind::Class | ast::TypeKind::Struct)) => {
                    Some(ty_fqn.clone())
                }
                _ => None,
            }
        })
        .collect();
    owners.sort();
    owners.dedup();
    owners
}

fn ctor_type_param_context(
    source: &SourceFile,
    owner_fqn: &str,
    ctor: &ConstructorOverload,
    expected_owner_eff: Option<&EffectRow>,
    lower: &mut TypeLowering<'_>,
) -> Result<CtorTypeParamContext, ExprTypeError> {
    let owner_sym = lower.env().type_symbol(owner_fqn).cloned();
    let owner_names = owner_sym
        .as_ref()
        .map(|sym| sym.type_param_names.clone())
        .unwrap_or_default();
    let owner_decl_file = owner_sym
        .as_ref()
        .map(|sym| sym.decl_file.clone())
        .unwrap_or_else(|| ctor.decl_file.clone());

    let mut type_params = Vec::with_capacity(owner_names.len() + ctor.type_params.len());
    let mut bindings = Vec::with_capacity(owner_names.len() + ctor.type_params.len());
    for name in &owner_names {
        let ty = lower.ty_param_named(name.clone(), owner_decl_file.clone(), Span::new(0, 0));
        type_params.push(ty);
        bindings.push((name.clone(), ty));
    }
    let mut eff_bindings = Vec::new();
    let mut owner_eff_arg = None;
    if let Some(eff_param) = owner_sym.as_ref().and_then(|sym| sym.eff_param.as_ref()) {
        let row = if let Some(expected) = expected_owner_eff {
            expected.clone()
        } else if let Some(sym) = owner_sym.as_ref() {
            lower.lower_effect_row_expr_in_decl_file_with_bindings(
                &sym.decl_file,
                bindings.iter().cloned(),
                eff_param.default.as_ref(),
            )?
        } else {
            EffectRow::pure()
        };
        eff_bindings.push((eff_param.name.clone(), row.clone()));
        owner_eff_arg = Some(row);
    }

    for param in &ctor.type_params {
        let ty = lower.ty_param_named(param.name.clone(), ctor.decl_file.clone(), param.name_span);
        type_params.push(ty);
        bindings.push((param.name.clone(), ty));
    }

    let mut where_constraints = Vec::new();
    if let Some(sym) = owner_sym.as_ref() {
        for constraint in &sym.where_constraints {
            let param_name = sym
                .type_param_names
                .get(constraint.param_index)
                .cloned()
                .unwrap_or_else(|| format!("#{}", constraint.param_index + 1));
            where_constraints.push(FunWhereConstraintInfo {
                _span: constraint.span,
                param_index: constraint.param_index,
                param_name,
                bound: constraint.bound.clone(),
            });
        }
    }

    let decl_source = lower.env().source(&ctor.decl_file).unwrap_or(source);
    let mut ctor_constraints = build_fun_where_constraints_from_resolve_sig(
        decl_source,
        &ctor.type_params,
        ctor.where_clause.as_ref(),
    );
    for constraint in &mut ctor_constraints {
        constraint.param_index += owner_names.len();
    }
    where_constraints.extend(ctor_constraints);

    Ok(CtorTypeParamContext {
        owner_type_param_count: owner_names.len(),
        type_params,
        bindings,
        eff_bindings,
        owner_eff_arg,
        where_constraints,
    })
}

pub(super) struct CtorParamInstantiationRequest<'a> {
    param_tys: &'a [TypeId],
    type_params: &'a [TypeId],
    owner_type_param_count: usize,
    where_constraints: &'a [FunWhereConstraintInfo],
    decl_file: &'a std::path::Path,
    mapping: &'a [ParamArgBinding],
    call_args: &'a [CallArgInfo<'a>],
    builtins: BuiltinTypes,
    call_span: Span,
    /// 显式构造器 type args（`Container<Int>(...)` 中的 `[Int]`）（P4-T01h）。
    ///
    /// - 若提供，长度必须与 owner type params 一致；
    /// - 优先级：显式 > arg-driven 反推 > LHS expected > `Any`；
    /// - 与 arg-driven 反推冲突时返回 `Ok(None)`，由调用点退化为 "no match"。
    explicit_type_args: Option<&'a [TypeId]>,
    /// LHS expected type 的 owner-args（`val c: Container<Int> = Container()` 中的 `[Int]`）（P4-T01h）。
    ///
    /// - 仅当外层 expected 类型是同 FQN 的 nominal generic instantiation 时由调用点提供；
    /// - 长度必须与 owner type params 一致；
    /// - 仅作为 arg-driven 反推未填充时的兜底候选，不主动覆盖 arg-driven 结果（与 explicit 不同）。
    expected_owner_args: Option<&'a [TypeId]>,
}

pub(super) fn instantiate_ctor_param_tys(
    request: CtorParamInstantiationRequest<'_>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<InstantiatedCtorParamTypes>, ExprTypeError> {
    let CtorParamInstantiationRequest {
        param_tys,
        type_params,
        owner_type_param_count,
        where_constraints,
        decl_file,
        mapping,
        call_args,
        builtins,
        call_span,
        explicit_type_args,
        expected_owner_args,
    } = request;

    if type_params.is_empty() {
        return Ok(Some((Vec::new(), param_tys.to_vec())));
    }

    // 显式 type-args / LHS expected owner-args 只对应 class owner type params；
    // constructor-level type params 只能从实参或默认 `Any` 获得。
    // 否则视为不匹配（让调用点退化到 NoMatchingOverload 路径，沿用现有诊断）。
    if let Some(explicit) = explicit_type_args
        && explicit.len() != owner_type_param_count
    {
        return Ok(None);
    }
    if let Some(expected) = expected_owner_args
        && expected.len() != owner_type_param_count
    {
        return Ok(None);
    }

    let mut inferred: HashMap<TypeId, TypeId> = HashMap::new();
    for (param_idx, arg_idx) in expand_param_arg_pairs(mapping) {
        let Some(expected_ty) = param_tys.get(param_idx).copied() else {
            return Ok(None);
        };
        let Some(arg) = call_args.get(arg_idx) else {
            return Ok(None);
        };
        let found_is_placeholder = matches!(arg.expr.kind, ast::ExprKind::Lambda(_));
        let found_tys = if arg.is_spread {
            let Some(elem_tys) = spread_operand_element_types(arg.ty, lower) else {
                return Ok(None);
            };
            elem_tys
        } else {
            vec![arg.ty]
        };

        for param_ty in type_params.iter().copied() {
            let mut candidates: Vec<TypeId> = Vec::new();
            for found_ty in &found_tys {
                if expected_ty == param_ty
                    && matches!(lower.type_kind(*found_ty), TypeKind::Param(_))
                {
                    candidates.push(*found_ty);
                }
                collect_type_arg_candidates_for_single_type_param(
                    expected_ty,
                    *found_ty,
                    param_ty,
                    &mut candidates,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }

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
    let mut owner_type_args: Vec<TypeId> = Vec::with_capacity(owner_type_param_count);
    let mut all_type_args: Vec<TypeId> = Vec::with_capacity(type_params.len());
    for (idx, param_ty) in type_params.iter().copied().enumerate() {
        let arg_inferred = inferred.get(&param_ty).copied();
        let owner_param = idx < owner_type_param_count;
        let explicit_at = owner_param
            .then(|| explicit_type_args.and_then(|e| e.get(idx).copied()))
            .flatten();
        let expected_at = owner_param
            .then(|| expected_owner_args.and_then(|e| e.get(idx).copied()))
            .flatten();

        let chosen = if let Some(t) = explicit_at {
            if let Some(bound) = arg_inferred
                && bound != t
                && !matches!(
                    (lower.type_kind(bound), lower.type_kind(t)),
                    (TypeKind::Param(_), TypeKind::Param(_))
                )
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
        if owner_param {
            owner_type_args.push(chosen);
        }
        all_type_args.push(chosen);
    }

    check_where_constraints_after_type_arg_instantiation(
        "constructor",
        call_span,
        decl_file,
        type_params,
        where_constraints,
        &all_type_args,
        lower,
        builtins,
    )?;

    let mut instantiated_param_tys = param_tys.to_vec();
    for (param_ty, arg_ty) in type_params
        .iter()
        .copied()
        .zip(all_type_args.iter().copied())
    {
        for expected_ty in &mut instantiated_param_tys {
            *expected_ty =
                substitute_single_type_param(*expected_ty, param_ty, arg_ty, lower, call_span)?;
        }
    }

    Ok(Some((owner_type_args, instantiated_param_tys)))
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
    expected_owner_eff: Option<&EffectRow>,
    strict_named_args: bool,
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<MatchedCtorOverload>, ExprTypeError> {
    let builtins = inputs.builtins;
    let source = inputs.source;
    let use_cone = lower.index().cone_of_source(source);

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

    if strict_named_args {
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
    }

    let mut matched: Vec<MatchedCtorOverload> = Vec::new();
    for ctor in visible {
        let type_param_context =
            ctor_type_param_context(source, owner_fqn, ctor, expected_owner_eff, lower)?;
        let param_names: Vec<String> = ctor.params.iter().map(|p| p.name.clone()).collect();
        let param_has_defaults: Vec<bool> = ctor.params.iter().map(|p| p.has_default).collect();
        let param_is_vararg: Vec<bool> = ctor.params.iter().map(|p| p.is_vararg).collect();

        let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
            call_args,
            &param_names,
            &param_has_defaults,
            &param_is_vararg,
        ) else {
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
            let ty = lower.lower_type_ref_in_decl_file_with_scopes(
                &ctor.decl_file,
                type_param_context.bindings.iter().cloned(),
                type_param_context.eff_bindings.iter().cloned(),
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
                type_params: &type_param_context.type_params,
                owner_type_param_count: type_param_context.owner_type_param_count,
                where_constraints: &type_param_context.where_constraints,
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

        let mapping_pairs = expand_param_arg_pairs(&mapping);
        let mut expected_arg_tys = vec![builtins.nothing; call_args.len()];
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
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

            if arg.is_spread {
                if !param_is_vararg.get(param_idx).copied().unwrap_or(false) {
                    ok = false;
                    break;
                }
                let Some(elem_tys) = spread_operand_element_types(found_ty, lower) else {
                    ok = false;
                    break;
                };
                if elem_tys.into_iter().all(|elem_ty| {
                    is_type_assignable(elem_ty, expected_ty, lower, builtins)
                        || literal_absorbs_to_expected(
                            arg.expr,
                            expected_ty,
                            source,
                            lower,
                            builtins,
                        )
                }) {
                    continue;
                }
                ok = false;
                break;
            }

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

        let signature = format!("{owner_fqn}({})", param_ty_strs.join(", "));
        let location = format_candidate_location(lower, &ctor.decl_file, ctor.span);
        let specificity = specificity_candidate_for_declared_params(
            signature.clone(),
            location,
            &param_tys,
            &type_param_context.type_params,
            &type_param_context.where_constraints,
            &ctor.decl_file,
            lower,
            builtins,
            call_span,
        )?;
        matched.push(MatchedCtorOverload {
            owner_fqn: owner_fqn.to_string(),
            ctor_span: Some(ctor.span),
            arg_mapping: mapping,
            expected_arg_tys,
            specificity,
            signature,
            inferred_type_args,
            inferred_eff_arg: type_param_context.owner_eff_arg.clone(),
        });
    }

    Ok(matched)
}

pub(in crate::typecheck::expr) fn collect_ctor_overload_rejections_for_owner(
    inputs: ExprInferInputs<'_>,
    owner_fqn: &str,
    callee_for_diag: &str,
    call_args: &[CallArgInfo<'_>],
    exclude_ctor_span: Option<Span>,
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<OverloadRejection>, ExprTypeError> {
    let builtins = inputs.builtins;
    let use_cone = lower.index().cone_of_source(inputs.source);
    let Some(ctors) = lower.index().constructors.get(owner_fqn).cloned() else {
        return Ok(Vec::new());
    };
    let mut rejections = Vec::new();
    for ctor in ctors
        .iter()
        .filter(|ctor| is_ctor_visible_from(use_cone, inputs.source, ctor))
    {
        if exclude_ctor_span.is_some_and(|exclude| ctor.span == exclude) {
            continue;
        }

        let type_param_context =
            ctor_type_param_context(inputs.source, owner_fqn, ctor, None, lower)?;
        let mut param_tys = Vec::with_capacity(ctor.params.len());
        let mut param_ty_strs = Vec::with_capacity(ctor.params.len());
        let mut malformed = false;
        for p in &ctor.params {
            let Some(ty_ref) = p.ty.as_ref() else {
                malformed = true;
                break;
            };
            let ty = lower.lower_type_ref_in_decl_file_with_scopes(
                &ctor.decl_file,
                type_param_context.bindings.iter().cloned(),
                type_param_context.eff_bindings.iter().cloned(),
                ty_ref,
            )?;
            param_ty_strs.push(lower.fmt_type(ty));
            param_tys.push(ty);
        }

        let param_names = ctor
            .params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        let param_has_defaults = ctor
            .params
            .iter()
            .map(|p| p.has_default)
            .collect::<Vec<_>>();
        let param_is_vararg = ctor.params.iter().map(|p| p.is_vararg).collect::<Vec<_>>();
        let reason = if malformed {
            "candidate signature is malformed".to_string()
        } else {
            describe_basic_applicability_rejection(BasicApplicabilityRejection {
                call_args,
                param_names: &param_names,
                param_has_defaults: &param_has_defaults,
                param_is_vararg: &param_is_vararg,
                param_tys: &param_tys,
                source: inputs.source,
                lower,
                builtins,
            })
        };
        rejections.push(OverloadRejection {
            signature: format!("{callee_for_diag}({})", param_ty_strs.join(", ")),
            location: format_candidate_location(lower, &ctor.decl_file, ctor.span),
            reason,
        });
    }

    Ok(rejections)
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
        None,
        true,
        lower,
    )?;

    if matched.is_empty() {
        let candidates = join_overload_rejections(collect_ctor_overload_rejections_for_owner(
            inputs,
            owner_fqn,
            callee_for_diag,
            call_args,
            exclude_ctor_span,
            lower,
        )?);
        return Err(ExprTypeError::NoApplicableOverload {
            callee: callee_for_diag.to_string(),
            candidates,
            span: call_span.into(),
        });
    }
    if matched.len() > 1 {
        let specificity = matched
            .iter()
            .map(|m| m.specificity.clone())
            .collect::<Vec<_>>();
        if let Some(chosen_idx) = pick_most_specific_overload(&specificity, lower, inputs.builtins)
        {
            return Ok(matched.remove(chosen_idx));
        }
        let candidates =
            format_ambiguous_specificity_candidates(&specificity, lower, inputs.builtins);
        return Err(ExprTypeError::AmbiguousOverload {
            callee: callee_for_diag.to_string(),
            candidates,
            span: call_span.into(),
        });
    }

    Ok(matched.pop().expect("non-empty matched constructor set"))
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

    let (expected_fqn, expected_args, expected_eff) = match lower.type_kind(expected_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
        | TypeKind::Ref(RefTypeKind::Nominal(nominal)) => (nominal.fqn, nominal.args, nominal.eff),
        _ => return Ok(None),
    };
    if expected_args.is_empty() {
        return Ok(None);
    }

    let expected_owner: Option<(&str, &[TypeId], Option<&EffectRow>)> = Some((
        expected_fqn.as_str(),
        expected_args.as_slice(),
        expected_eff.as_ref(),
    ));

    infer_nominal_constructor_call_expr_type(
        inputs,
        expr,
        id,
        args,
        explicit_type_args.as_deref(),
        expected_owner,
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
        None,
        true,
        lower,
    )?;

    if matched.is_empty() {
        let candidates = join_overload_rejections(collect_ctor_overload_rejections_for_owner(
            inputs,
            &owner_fqn,
            &callee_name,
            &call_args,
            None,
            lower,
        )?);
        return Err(ExprTypeError::NoApplicableOverload {
            callee: callee_name,
            candidates,
            span: call_expr.span.into(),
        });
    }
    if matched.len() > 1 {
        let specificity = matched
            .iter()
            .map(|m| m.specificity.clone())
            .collect::<Vec<_>>();
        if let Some(chosen_idx) = pick_most_specific_overload(&specificity, lower, inputs.builtins)
        {
            let chosen = matched.remove(chosen_idx);
            lower.record_typechecked_ctor_call_binding(
                call_expr.span,
                chosen.owner_fqn.clone(),
                chosen.ctor_span,
                legacy_optional_mapping_from_param_mapping(&chosen.arg_mapping),
            );
            if let Some(binding) = call_arg_binding_from_mapping(&chosen.arg_mapping, &call_args) {
                lower.record_typechecked_call_arg_binding(call_expr.span, binding);
            }
            let ty = lower.lower_type_fqn_with_args_and_eff(
                chosen.owner_fqn,
                chosen.inferred_type_args,
                chosen.inferred_eff_arg,
                use_span,
            )?;
            return Ok(Some(ty));
        }
        let candidates =
            format_ambiguous_specificity_candidates(&specificity, lower, inputs.builtins);
        return Err(ExprTypeError::AmbiguousOverload {
            callee: callee_name,
            candidates,
            span: call_expr.span.into(),
        });
    }
    let chosen = matched.pop().expect("non-empty matched constructor set");

    lower.record_typechecked_ctor_call_binding(
        call_expr.span,
        chosen.owner_fqn.clone(),
        chosen.ctor_span,
        legacy_optional_mapping_from_param_mapping(&chosen.arg_mapping),
    );
    if let Some(binding) = call_arg_binding_from_mapping(&chosen.arg_mapping, &call_args) {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }
    let ty = lower.lower_type_fqn_with_args_and_eff(
        chosen.owner_fqn,
        chosen.inferred_type_args,
        chosen.inferred_eff_arg,
        use_span,
    )?;
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
    expected_owner: Option<(&str, &[TypeId], Option<&EffectRow>)>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let source = inputs.source;

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
        let owner_expected = expected_owner.filter(|(fqn, _, _)| *fqn == owner_fqn.as_str());
        let owner_expected_args = owner_expected.map(|(_, args, _)| args);
        let owner_expected_eff = owner_expected.and_then(|(_, _, eff)| eff);
        matched.extend(collect_matched_ctor_overloads_for_owner(
            inputs,
            owner_fqn,
            call_expr.span,
            &callee_name,
            &call_args,
            None,
            explicit_type_args,
            owner_expected_args,
            owner_expected_eff,
            true,
            lower,
        )?);
    }

    if matched.is_empty() {
        let mut rejections = Vec::new();
        for (owner_fqn, _) in &ctor_owners {
            rejections.extend(collect_ctor_overload_rejections_for_owner(
                inputs,
                owner_fqn,
                &callee_name,
                &call_args,
                None,
                lower,
            )?);
        }
        return Err(ExprTypeError::NoApplicableOverload {
            callee: callee_name,
            candidates: join_overload_rejections(rejections),
            span: call_expr.span.into(),
        });
    }
    if matched.len() > 1 {
        let specificity = matched
            .iter()
            .map(|m| m.specificity.clone())
            .collect::<Vec<_>>();
        if let Some(chosen_idx) = pick_most_specific_overload(&specificity, lower, inputs.builtins)
        {
            let chosen = matched.remove(chosen_idx);
            lower.record_typechecked_ctor_call_binding(
                call_expr.span,
                chosen.owner_fqn.clone(),
                chosen.ctor_span,
                legacy_optional_mapping_from_param_mapping(&chosen.arg_mapping),
            );
            if let Some(binding) = call_arg_binding_from_mapping(&chosen.arg_mapping, &call_args) {
                lower.record_typechecked_call_arg_binding(call_expr.span, binding);
            }
            let ty = lower.lower_type_fqn_with_args_and_eff(
                chosen.owner_fqn,
                chosen.inferred_type_args,
                chosen.inferred_eff_arg,
                callee.span,
            )?;
            return Ok(Some(ty));
        }
        let candidates =
            format_ambiguous_specificity_candidates(&specificity, lower, inputs.builtins);
        return Err(ExprTypeError::AmbiguousOverload {
            callee: callee_name,
            candidates,
            span: call_expr.span.into(),
        });
    }
    let chosen = matched.pop().expect("non-empty matched constructor set");

    lower.record_typechecked_ctor_call_binding(
        call_expr.span,
        chosen.owner_fqn.clone(),
        chosen.ctor_span,
        legacy_optional_mapping_from_param_mapping(&chosen.arg_mapping),
    );
    if let Some(binding) = call_arg_binding_from_mapping(&chosen.arg_mapping, &call_args) {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }
    let ty = lower.lower_type_fqn_with_args_and_eff(
        chosen.owner_fqn,
        chosen.inferred_type_args,
        chosen.inferred_eff_arg,
        callee.span,
    )?;
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
