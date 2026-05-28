//! Top-level call dispatch: infer_call_expr_type plus unsafe ptr primitive helpers.

#![allow(dead_code)]

use super::*;

pub(in crate::typecheck::expr) fn infer_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee: &ast::Expr,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let locals = inputs.locals;
    let top_level_types = inputs.top_level_types;
    let top_level_funs = inputs.top_level_funs;

    // 显式类型实参（T1204）：`callee<T>()` 在 AST 中表示为 `Call(TypeApply(callee, type_args), args)`。
    //
    // 说明：
    // - 在“普通值表达式”位置，`callee<T>` 现由 `infer_top_level_fun_value_expr_type` 处理；
    // - 但当它作为 `Call` 的 callee 出现时，我们仍需要把显式 type args 传给泛型函数实例化逻辑。
    let mut explicit_type_args: Option<Vec<TypeId>> = None;
    let mut explicit_eff_arg: Option<EffectRow> = None;
    let callee_expr: &ast::Expr = match &callee.kind {
        ast::ExprKind::TypeApply {
            callee: inner,
            args,
        } => {
            let lowered = lower_explicit_type_apply_args(args, lower)?;
            explicit_type_args = Some(lowered.type_args);
            explicit_eff_arg = lowered.eff_arg;
            inner.as_ref()
        }
        _ => callee,
    };

    match &callee_expr.kind {
        ast::ExprKind::Ident(id) => {
            let callee_name = source.slice(id.span);
            if id.resolved.is_none() {
                let has_fun_candidates = id.call.as_ref().is_some_and(|call| {
                    call.candidates
                        .iter()
                        .any(|candidate| matches!(candidate, ast::CallCandidate::Fun { .. }))
                });
                if !has_fun_candidates {
                    // T1009：unsafe 指针原语（最小集合）。
                    if let Some(ty) = infer_unsafe_ptr_primitive_call_expr_type(
                        inputs,
                        call_expr,
                        callee_name,
                        args,
                        lower,
                    )? {
                        return Ok(ty);
                    }

                    // T0426：`Some(x)` 这类 enum variant 构造表达式在语法上与普通函数调用一致，
                    // 但 resolver 不会把 `Some` 绑定为顶层函数符号，因此这里在"未 resolve 的 ident"
                    // 情况下尝试按 enum variant ctor 处理。
                    if let Some(ctor_ty) =
                        infer_enum_variant_ctor_call_expr_type(inputs, call_expr, id, args, lower)?
                    {
                        return Ok(ctor_ty);
                    }

                    // T0454/T4010b0：nominal 构造调用（class ctor / struct field constructor）重载决议。
                    if let Some(ctor_ty) = infer_nominal_constructor_call_expr_type(
                        inputs,
                        call_expr,
                        id,
                        args,
                        explicit_type_args.as_deref(),
                        None,
                        lower,
                    )? {
                        return Ok(ctor_ty);
                    }

                    if resolves_to_compiler_owned_continuation_type(callee_name, id.span, lower) {
                        return Err(ExprTypeError::ContinuationNotConstructible {
                            span: call_expr.span.into(),
                        });
                    }

                    return Err(ExprTypeError::CalleeNotCallable {
                        callee: callee_name.to_string(),
                        span: id.span.into(),
                    });
                }
            }

            let (resolved_fqn, callee_span) = match id.resolved.as_ref() {
                Some(ast::ResolvedValueRef::TopLevel { fqn }) => (fqn.clone(), id.span),
                Some(ast::ResolvedValueRef::Local { decl_span, .. }) => {
                    if locals
                        .get(decl_span)
                        .copied()
                        .is_some_and(|ty| is_funptr_type(ty, lower))
                    {
                        return infer_funptr_value_call_expr_type(
                            inputs,
                            call_expr,
                            callee_name,
                            *decl_span,
                            args,
                            lower,
                        );
                    }
                    return infer_function_value_call_expr_type(
                        inputs,
                        call_expr,
                        callee_name,
                        *decl_span,
                        args,
                        lower,
                    );
                }
                None => (callee_name.to_string(), id.span),
            };
            let candidate_fqns = id
                .call
                .as_ref()
                .map(|call| {
                    call.candidates
                        .iter()
                        .filter_map(|candidate| match candidate {
                            ast::CallCandidate::Fun { fqn } => Some(fqn.clone()),
                            ast::CallCandidate::Constructor { .. } => None,
                        })
                        .collect::<Vec<_>>()
                })
                .filter(|fqns| !fqns.is_empty())
                .unwrap_or_else(|| vec![resolved_fqn.clone()]);
            let ctor_owner_fqns =
                collect_ctor_owner_fqns_from_call_candidates(id.call.as_ref(), lower);
            let candidate_storage = collect_fun_sig_candidates_for_fqns(
                candidate_fqns,
                inputs.source,
                top_level_funs,
                lower,
                builtins,
            )?;

            // 扩展函数不能以 `f(args...)` 的形式被直接调用，因此这里只选择普通顶层函数候选。
            let direct_call_candidates: Vec<&CandidateFunSig> = candidate_storage
                .iter()
                .filter(|candidate| !candidate.sig.is_extension)
                .collect();
            let Some(first_candidate) = direct_call_candidates.first().copied() else {
                let top_level_value_ty = top_level_types.get(&resolved_fqn).copied();
                if explicit_type_args
                    .as_ref()
                    .is_some_and(|type_args| !type_args.is_empty())
                    && top_level_value_ty.is_some()
                {
                    return Err(ExprTypeError::CalleeNotCallable {
                        callee: resolved_fqn,
                        span: callee_span.into(),
                    });
                }

                if let Some(callee_ty) = top_level_value_ty
                    && matches!(
                        lower.type_kind(callee_ty),
                        TypeKind::Ref(RefTypeKind::Function(_))
                    )
                {
                    return infer_function_type_call_expr_type(
                        inputs,
                        call_expr,
                        callee_name,
                        callee_ty,
                        args,
                        lower,
                    );
                }

                // 顶层值为函数指针：允许 `fp(args...)` 形态调用（必须在 unsafe context）。
                if top_level_value_ty.is_some_and(|ty| is_funptr_type(ty, lower)) {
                    return infer_funptr_type_call_expr_type(
                        inputs,
                        call_expr,
                        callee_name,
                        top_level_value_ty.unwrap_or(builtins.any),
                        args,
                        lower,
                    );
                }
                if id.call.is_none() && lower.is_object_type(&resolved_fqn) {
                    return Err(ExprTypeError::ObjectNotConstructible {
                        name: resolved_fqn,
                        span: callee_span.into(),
                    });
                }
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: resolved_fqn,
                    span: callee_span.into(),
                });
            };
            let callee_fqn = first_candidate.fqn.clone();
            let sig = &first_candidate.sig;

            // 只有一个可用候选：沿用旧的"给出精确 arity/type mismatch 诊断"的路径，
            // 但补齐命名实参的形参映射（T0453）。
            if direct_call_candidates.len() == 1 && ctor_owner_fqns.is_empty() {
                check_unsafe_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
                check_nogc_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
                emit_deprecated_call_warning(&callee_fqn, sig, call_expr.span, lower);
                let used_unit_sugar = can_use_zero_arg_unit_call_sugar(
                    args,
                    &sig.params,
                    &sig.param_has_defaults,
                    &sig.param_is_vararg,
                    lower,
                );
                let synthesized_args =
                    used_unit_sugar.then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
                let call_args = collect_call_arg_infos(
                    inputs,
                    synthesized_args.as_deref().unwrap_or(args),
                    lower,
                )?;
                check_call_arg_named_rules(&callee_fqn, &call_args)?;
                check_call_named_args_exist_in_any_candidate(
                    &callee_fqn,
                    &call_args,
                    std::iter::once(sig.param_names.as_slice()),
                )?;

                // 默认参数（T0512）：允许省略带默认值的形参。
                //
                // 注意：当前阶段只做"候选可用性/形参映射/类型检查"，不在 AST/HIR 层补齐默认值表达式
                //（默认值补齐语义留给后续任务 T1305）。
                let has_vararg = vararg_param_index(&sig.param_is_vararg).is_some();

                let mapping: Vec<ParamArgBinding> = if !has_vararg {
                    // 旧路径：保持原有 arity mismatch 诊断行为。
                    if call_args.len() > sig.params.len() {
                        return Err(ExprTypeError::CallArityMismatch {
                            callee: callee_fqn,
                            expected: sig.params.len(),
                            found: call_args.len(),
                            span: call_expr.span.into(),
                        });
                    }

                    let required = sig.param_has_defaults.iter().filter(|d| !**d).count();
                    if call_args.len() < required {
                        return Err(ExprTypeError::CallArityMismatch {
                            callee: callee_fqn,
                            expected: required,
                            found: call_args.len(),
                            span: call_expr.span.into(),
                        });
                    }

                    let Some(mapping) = map_call_args_to_params_with_defaults(
                        &call_args,
                        &sig.param_names,
                        &sig.param_has_defaults,
                    ) else {
                        if let Some(missing) = missing_required_param_names_in_named_call(
                            &call_args,
                            &sig.param_names,
                            &sig.param_has_defaults,
                        ) {
                            return Err(ExprTypeError::CallMissingRequiredArgs {
                                callee: callee_fqn,
                                missing: missing.join(", "),
                                span: call_expr.span.into(),
                            });
                        }
                        return Err(ExprTypeError::NoMatchingOverload {
                            callee: callee_fqn,
                            span: call_expr.span.into(),
                        });
                    };

                    mapping
                        .into_iter()
                        .map(|arg_idx| {
                            arg_idx.map_or(ParamArgBinding::Default, ParamArgBinding::Single)
                        })
                        .collect()
                } else {
                    // vararg：允许"多传"，并把多余的实参归入 vararg 槽位。
                    let required =
                        required_param_count(&sig.param_has_defaults, &sig.param_is_vararg)
                            .unwrap_or_else(|| {
                                sig.param_has_defaults.iter().filter(|d| !**d).count()
                            });
                    if call_args.len() < required {
                        return Err(ExprTypeError::CallArityMismatch {
                            callee: callee_fqn,
                            expected: required,
                            found: call_args.len(),
                            span: call_expr.span.into(),
                        });
                    }

                    let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                        &call_args,
                        &sig.param_names,
                        &sig.param_has_defaults,
                        &sig.param_is_vararg,
                    ) else {
                        return Err(ExprTypeError::NoMatchingOverload {
                            callee: callee_fqn,
                            span: call_expr.span.into(),
                        });
                    };
                    mapping
                };

                // spread 实参只能绑定到 vararg 形参（Appendix B.5.5）。
                for (param_idx, binding) in mapping.iter().enumerate() {
                    match binding {
                        ParamArgBinding::Default => {}
                        ParamArgBinding::Single(arg_idx) => {
                            if call_args.get(*arg_idx).is_some_and(|a| a.is_spread) {
                                return Err(ExprTypeError::SpreadArgRequiresVararg {
                                    callee: callee_fqn.clone(),
                                    span: call_args[*arg_idx].expr.span.into(),
                                });
                            }
                        }
                        ParamArgBinding::Vararg(_) => {
                            // ok
                            let _ = param_idx;
                        }
                    }
                }

                check_var_param_lvalue_gate(&callee_fqn, sig, &call_args, &mapping)?;

                let mapping_pairs = expand_param_arg_pairs(&mapping);

                let mut generic_constraints: Vec<GenericArgConstraint> =
                    Vec::with_capacity(mapping_pairs.len());
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args[arg_idx];
                    if arg.is_spread {
                        if !sig.param_is_vararg.get(param_idx).copied().unwrap_or(false) {
                            return Err(ExprTypeError::SpreadArgRequiresVararg {
                                callee: callee_fqn.clone(),
                                span: arg.expr.span.into(),
                            });
                        }

                        let Some(elem_tys) = spread_operand_element_types(arg.ty, lower) else {
                            return Err(ExprTypeError::VarargSpreadRequiresArrayOrTuple {
                                found: lower.fmt_type(arg.ty),
                                hint: vararg_spread_missing_bridge_hint(arg.ty, lower, builtins),
                                span: arg.expr.span.into(),
                            });
                        };
                        for found_elem in elem_tys {
                            generic_constraints.push(GenericArgConstraint {
                                expected: sig.params[param_idx],
                                found: found_elem,
                                found_is_placeholder: false,
                                from: format!("第 {} 个实参（spread）", arg_idx + 1),
                                span: arg.expr.span,
                            });
                        }
                        continue;
                    }

                    generic_constraints.push(GenericArgConstraint {
                        expected: sig.params[param_idx],
                        found: arg.ty,
                        found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                        from: format!("第 {} 个实参", arg_idx + 1),
                        span: arg.expr.span,
                    });
                }

                let mut instantiated =
                    instantiate_fun_sig_for_call_with_optional_explicit_type_args(
                        &callee_fqn,
                        call_expr.span,
                        sig,
                        explicit_type_args.as_deref(),
                        generic_constraints,
                        lower,
                        builtins,
                    )?;

                // T0129：检查 where 约束。
                check_fun_where_constraints_after_instantiation(
                    &callee_fqn,
                    call_expr.span,
                    sig,
                    &instantiated.type_args,
                    lower,
                    builtins,
                )?;

                // 先在"期望类型语境"下推导每个实参的最终类型（lambda 会在此处被真正类型检查）。
                let mut checked_arg_tys: Vec<TypeId> = call_args.iter().map(|a| a.ty).collect();
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args[arg_idx];
                    if arg.is_spread {
                        // spread operand 在 `collect_call_arg_infos` 中已经被 typecheck，这里无需再次进入 expected-context。
                        continue;
                    }

                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = inputs.infer_in_expected(
                        lower,
                        arg.expr,
                        expected_ty,
                        ExpectedTypeFrom::new(format!(
                            "`{}` 的第 {} 个形参 `{}`",
                            callee_fqn,
                            param_idx + 1,
                            sig.param_names[param_idx]
                        )),
                    )?;
                    checked_arg_tys[arg_idx] = found_ty;
                }
                check_thread_spawn_entry_policy(
                    &callee_fqn,
                    &call_args,
                    &checked_arg_tys,
                    &mapping_pairs,
                    lower,
                )?;

                // T0509/T0624：推断 `eff` row 参数：
                // - T0509：从 lambda body 的 required effects 推断 `E`；
                // - T0624：从 `Type<eff E>` 形式的实参类型中提取 row 约束（例如 `Disposable<eff Async>`）。
                let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.clone() {
                    explicit_eff_arg
                } else if let Some(eff_param) = &sig.eff_param {
                    let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

                    // T0624/T0628a：从 `Type<eff Row>` 的"实参类型"中提取 row 约束。
                    //
                    // 约束形态：`found ⊆ (E + base)`，因此对 `E` 的最小贡献为 `found - base`。
                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        let arg = &call_args[arg_idx];
                        if arg.is_spread {
                            continue;
                        }
                        let Some(base) = sig
                            .param_nominal_eff_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        let base = substitute_type_args_in_effect_row(
                            base.clone(),
                            &sig.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        )?;

                        let found_ty = checked_arg_tys[arg_idx];
                        if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                            let delta = effect_row_difference(&found_row, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    // T0509/T0628a：从 lambda body 的 required effects 推断 `E`（同样按 `found - base` 提取增量）。
                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        let arg = &call_args[arg_idx];
                        if arg.is_spread {
                            continue;
                        }
                        let Some(base) = sig
                            .param_fn_effect_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                            continue;
                        }

                        let base = substitute_type_args_in_effect_row(
                            base.clone(),
                            &sig.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        )?;

                        let found_ty = checked_arg_tys[arg_idx];
                        if let TypeKind::Ref(RefTypeKind::Function(found_fun)) =
                            lower.type_kind(found_ty)
                        {
                            let delta = effect_row_difference(&found_fun.effects, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    let inferred = EffectRow::new(terms);
                    substitute_type_args_in_effect_row(
                        inferred,
                        &sig.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    )?
                } else {
                    EffectRow::pure()
                };

                instantiate_eff_row_var_in_sig_types(
                    sig,
                    &mut instantiated,
                    &eff_arg,
                    lower,
                    call_expr.span,
                )?;

                // 再做"可赋值"检查（此时 lambda 的 effects 也已经被推断并写入 found_ty）。
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args[arg_idx];
                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = checked_arg_tys[arg_idx];

                    if arg.is_spread {
                        if !sig.param_is_vararg.get(param_idx).copied().unwrap_or(false) {
                            return Err(ExprTypeError::SpreadArgRequiresVararg {
                                callee: callee_fqn.clone(),
                                span: arg.expr.span.into(),
                            });
                        }

                        let Some(elem_tys) = spread_operand_element_types(found_ty, lower) else {
                            return Err(ExprTypeError::VarargSpreadRequiresArrayOrTuple {
                                found: lower.fmt_type(found_ty),
                                hint: vararg_spread_missing_bridge_hint(found_ty, lower, builtins),
                                span: arg.expr.span.into(),
                            });
                        };

                        for elem_ty in elem_tys {
                            if is_type_assignable(elem_ty, expected_ty, lower, builtins) {
                                check_fn_value_to_any_erasure_gate(
                                    elem_ty,
                                    expected_ty,
                                    arg.expr.span,
                                    lower,
                                    builtins,
                                )?;
                                continue;
                            }
                            return Err(ExprTypeError::VarargSpreadElementTypeMismatch {
                                expected: lower.fmt_type(expected_ty),
                                found: lower.fmt_type(elem_ty),
                                span: arg.expr.span.into(),
                            });
                        }
                        continue;
                    }

                    if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                        check_fn_value_to_any_erasure_gate(
                            found_ty,
                            expected_ty,
                            arg.expr.span,
                            lower,
                            builtins,
                        )?;
                        check_nogc_boxing_gate(
                            found_ty,
                            expected_ty,
                            arg.expr.span,
                            lower,
                            builtins,
                        )?;
                        continue;
                    }

                    // 整数字面量允许被上下文整数参数类型吸收（后续可加入 range check）。
                    if literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins) {
                        continue;
                    }

                    return Err(ExprTypeError::CallArgTypeMismatch {
                        callee: callee_fqn,
                        index: param_idx + 1,
                        expected: lower.fmt_type(expected_ty),
                        found: lower.fmt_type(found_ty),
                        span: arg.expr.span.into(),
                    });
                }

                check_atomic_intrinsic_target_gate(
                    inputs,
                    &callee_fqn,
                    &call_args,
                    &mapping_pairs,
                    lower,
                )?;

                // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
                let type_param_bindings = type_param_bindings_from_sig(&sig.type_params, lower);
                let eff_bindings: Vec<(String, EffectRow)> = sig
                    .eff_param
                    .as_ref()
                    .map(|p| vec![(p.name.clone(), eff_arg.clone())])
                    .unwrap_or_default();
                let lowered_effects = lower.lower_effect_row_expr_in_decl_file_with_scopes(
                    &sig.decl_file,
                    type_param_bindings,
                    eff_bindings,
                    sig.effects.as_ref(),
                );
                let call_effects = substitute_type_args_in_effect_row(
                    lowered_effects?,
                    &sig.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                )?;
                for effect in call_effects.terms.iter().copied() {
                    lower.record_performed_effect(effect, call_expr.span);
                }

                // T0712：记录该泛型函数调用产生的 monomorph key（用于后续生成专用实例）。
                let eff_args = sig
                    .eff_param
                    .as_ref()
                    .map(|_| vec![eff_arg.clone()])
                    .unwrap_or_default();
                lower.record_monomorph_call(
                    callee_fqn.clone(),
                    &sig.decl_file,
                    sig.decl_span,
                    &instantiated.type_args,
                    &eff_args,
                    call_expr.span,
                );
                lower.record_top_level_fun_call_binding(
                    call_expr.span,
                    ast::TopLevelFunCallBinding {
                        fqn: callee_fqn.clone(),
                        decl_file: sig.decl_file.clone(),
                        decl_span: sig.decl_span,
                        is_intrinsic: sig.is_intrinsic,
                        intrinsic_entry_name: sig.intrinsic_entry_name.clone(),
                        type_args: instantiated.type_args.clone(),
                        eff_args,
                    },
                );
                if let Some(binding) = call_arg_binding_from_mapping(&mapping, &call_args) {
                    lower.record_typechecked_call_arg_binding(call_expr.span, binding);
                }
                if used_unit_sugar {
                    lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
                }

                return Ok(instantiated.return_ty);
            }

            // 多候选：先按形参映射过滤，再对剩余候选尝试泛型/eff 推断（T0512）。
            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            let synthesized_unit_args = args
                .is_empty()
                .then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
            let sugar_call_args = if let Some(synthesized_args) = synthesized_unit_args.as_ref() {
                Some(collect_call_arg_infos(inputs, synthesized_args, lower)?)
            } else {
                None
            };
            check_call_arg_named_rules(&callee_fqn, &call_args)?;
            if ctor_owner_fqns.is_empty() {
                check_call_named_args_exist_in_any_candidate(
                    &callee_fqn,
                    &call_args,
                    direct_call_candidates
                        .iter()
                        .map(|c| c.sig.param_names.as_slice()),
                )?;
            }

            #[derive(Debug, Clone)]
            struct MatchedFunOverload<'a> {
                fqn: &'a str,
                sig: &'a FunSigOwned,
                instantiated: InstantiatedFunSig,
                eff_arg: EffectRow,
                /// `call_args[arg_idx]` 对应的"期望类型"。
                expected_arg_tys: Vec<TypeId>,
                /// 形参 -> 实参绑定（用于后续门禁，例如 `addressOf(var: T)`）。
                mapping: Vec<ParamArgBinding>,
                /// 当前候选是否通过 typed `Unit` zero-arg sugar 匹配得到。
                used_unit_sugar: bool,
            }

            let mut matched: Vec<MatchedFunOverload<'_>> = Vec::new();
            for candidate in direct_call_candidates.iter().copied() {
                let callee_fqn = candidate.fqn.as_str();
                let cand = &candidate.sig;
                let exact_mapping = map_call_args_to_params_with_defaults_and_varargs(
                    &call_args,
                    &cand.param_names,
                    &cand.param_has_defaults,
                    &cand.param_is_vararg,
                );
                let (call_args_for_candidate, mapping, used_unit_sugar) =
                    if let Some(mapping) = exact_mapping {
                        (&call_args, mapping, false)
                    } else if can_use_zero_arg_unit_call_sugar(
                        args,
                        &cand.params,
                        &cand.param_has_defaults,
                        &cand.param_is_vararg,
                        lower,
                    ) {
                        let Some(sugar_call_args) = sugar_call_args.as_ref() else {
                            continue;
                        };
                        let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                            sugar_call_args,
                            &cand.param_names,
                            &cand.param_has_defaults,
                            &cand.param_is_vararg,
                        ) else {
                            continue;
                        };
                        (sugar_call_args, mapping, true)
                    } else {
                        continue;
                    };

                // spread 实参只能绑定到 vararg 形参；否则该候选不匹配。
                let mut ok = true;
                for binding in mapping.iter() {
                    match binding {
                        ParamArgBinding::Default => {}
                        ParamArgBinding::Single(arg_idx) => {
                            if call_args_for_candidate
                                .get(*arg_idx)
                                .is_some_and(|a| a.is_spread)
                            {
                                ok = false;
                                break;
                            }
                        }
                        ParamArgBinding::Vararg(_) => {}
                    }
                }
                if !ok {
                    continue;
                }

                let mapping_pairs = expand_param_arg_pairs(&mapping);

                let mut generic_constraints: Vec<GenericArgConstraint> =
                    Vec::with_capacity(mapping_pairs.len());
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args_for_candidate[arg_idx];
                    if arg.is_spread {
                        if !cand
                            .param_is_vararg
                            .get(param_idx)
                            .copied()
                            .unwrap_or(false)
                        {
                            ok = false;
                            break;
                        }
                        let Some(elem_tys) = spread_operand_element_types(arg.ty, lower) else {
                            ok = false;
                            break;
                        };
                        for found_elem in elem_tys {
                            generic_constraints.push(GenericArgConstraint {
                                expected: cand.params[param_idx],
                                found: found_elem,
                                found_is_placeholder: false,
                                from: format!("第 {} 个实参（spread）", arg_idx + 1),
                                span: arg.expr.span,
                            });
                        }
                        continue;
                    }

                    generic_constraints.push(GenericArgConstraint {
                        expected: cand.params[param_idx],
                        found: arg.ty,
                        found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                        from: format!("第 {} 个实参", arg_idx + 1),
                        span: arg.expr.span,
                    });
                }
                if !ok {
                    continue;
                }

                let mut instantiated =
                    match instantiate_fun_sig_for_call_with_optional_explicit_type_args(
                        callee_fqn,
                        call_expr.span,
                        cand,
                        explicit_type_args.as_deref(),
                        generic_constraints,
                        lower,
                        builtins,
                    ) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                // T0129：检查 where 约束；不满足则跳过该候选。
                if check_fun_where_constraints_after_instantiation(
                    callee_fqn,
                    call_expr.span,
                    cand,
                    &instantiated.type_args,
                    lower,
                    builtins,
                )
                .is_err()
                {
                    continue;
                }

                // 只在需要时（lambda）进入 expected-context typecheck，避免在候选尝试期间把"候选相关"的
                // 副作用（例如调用 required effects）写进外层函数体的 effects 集合。
                let mut checked_arg_tys: Vec<TypeId> =
                    call_args_for_candidate.iter().map(|a| a.ty).collect();
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args_for_candidate[arg_idx];
                    if arg.is_spread {
                        continue;
                    }
                    if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                        continue;
                    }

                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = match inputs.infer_in_expected(
                        lower,
                        arg.expr,
                        expected_ty,
                        ExpectedTypeFrom::new(format!(
                            "`{}` 的第 {} 个形参 `{}`",
                            callee_fqn,
                            param_idx + 1,
                            cand.param_names[param_idx]
                        )),
                    ) {
                        Ok(ty) => ty,
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    };
                    checked_arg_tys[arg_idx] = found_ty;
                }
                if !ok {
                    continue;
                }

                // T0509/T0624/T0628a：推断 `eff` row 参数：
                // - 从 lambda body 的 required effects 推断（`found - base`）；
                // - 从 `Type<eff Row>` 形参的实参类型提取 row 约束（`found - base`）。
                let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.clone() {
                    explicit_eff_arg
                } else if let Some(eff_param) = &cand.eff_param {
                    let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        let arg = &call_args_for_candidate[arg_idx];
                        if arg.is_spread {
                            continue;
                        }
                        let Some(base) = cand
                            .param_nominal_eff_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        let base = match substitute_type_args_in_effect_row(
                            base.clone(),
                            &cand.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        ) {
                            Ok(row) => row,
                            Err(_) => continue,
                        };

                        let found_ty = checked_arg_tys[arg_idx];
                        if let Some(found_row) = nominal_eff_row_from_type(found_ty, lower) {
                            let delta = effect_row_difference(&found_row, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        let arg = &call_args_for_candidate[arg_idx];
                        if arg.is_spread {
                            continue;
                        }
                        let Some(base) = cand
                            .param_fn_effect_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        else {
                            continue;
                        };

                        if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
                            continue;
                        }

                        let base = match substitute_type_args_in_effect_row(
                            base.clone(),
                            &cand.type_params,
                            &instantiated.type_args,
                            lower,
                            call_expr.span,
                        ) {
                            Ok(row) => row,
                            Err(_) => continue,
                        };

                        let found_ty = checked_arg_tys[arg_idx];
                        if let TypeKind::Ref(RefTypeKind::Function(found_fun)) =
                            lower.type_kind(found_ty)
                        {
                            let delta = effect_row_difference(&found_fun.effects, &base);
                            terms.extend(delta.terms);
                        }
                    }

                    let inferred = EffectRow::new(terms);
                    match substitute_type_args_in_effect_row(
                        inferred,
                        &cand.type_params,
                        &instantiated.type_args,
                        lower,
                        call_expr.span,
                    ) {
                        Ok(row) => row,
                        Err(_) => continue,
                    }
                } else {
                    EffectRow::pure()
                };

                if cand.eff_param.is_some()
                    && instantiate_eff_row_var_in_sig_types(
                        cand,
                        &mut instantiated,
                        &eff_arg,
                        lower,
                        call_expr.span,
                    )
                    .is_err()
                {
                    ok = false;
                }
                if !ok {
                    continue;
                }
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    let arg = &call_args_for_candidate[arg_idx];
                    let expected_ty = instantiated.params[param_idx];
                    let found_ty = checked_arg_tys[arg_idx];

                    if arg.is_spread {
                        if !cand
                            .param_is_vararg
                            .get(param_idx)
                            .copied()
                            .unwrap_or(false)
                        {
                            ok = false;
                            break;
                        }
                        let Some(elem_tys) = spread_operand_element_types(found_ty, lower) else {
                            ok = false;
                            break;
                        };
                        for elem_ty in elem_tys {
                            if is_type_assignable(elem_ty, expected_ty, lower, builtins) {
                                continue;
                            }
                            ok = false;
                            break;
                        }
                        if !ok {
                            break;
                        }
                        continue;
                    }

                    if is_type_assignable(found_ty, expected_ty, lower, builtins) {
                        continue;
                    }
                    if literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins) {
                        continue;
                    }
                    ok = false;
                    break;
                }

                if ok {
                    let mut expected_arg_tys =
                        vec![builtins.nothing; call_args_for_candidate.len()];
                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        expected_arg_tys[arg_idx] = instantiated.params[param_idx];
                    }

                    matched.push(MatchedFunOverload {
                        fqn: callee_fqn,
                        sig: cand,
                        instantiated,
                        eff_arg,
                        expected_arg_tys,
                        mapping,
                        used_unit_sugar,
                    });
                }
            }

            if matched.iter().any(|cand| !cand.used_unit_sugar) {
                matched.retain(|cand| !cand.used_unit_sugar);
            }

            let mut matched_ctors = Vec::new();
            for owner_fqn in &ctor_owner_fqns {
                matched_ctors.extend(collect_matched_ctor_overloads_for_owner(
                    inputs,
                    owner_fqn,
                    call_expr.span,
                    callee_name,
                    &call_args,
                    None,
                    explicit_type_args.as_deref(),
                    None,
                    false,
                    lower,
                )?);
            }

            let total_matched = matched.len() + matched_ctors.len();
            if total_matched == 0 {
                let mut rejections: Vec<OverloadRejection> = direct_call_candidates
                    .iter()
                    .map(|candidate| {
                        let name = short_name_from_fqn(&candidate.fqn).to_string();
                        let cand = &candidate.sig;
                        OverloadRejection {
                            signature: fmt_overload_signature(&name, None, &cand.params, lower),
                            location: format_candidate_location(
                                lower,
                                &cand.decl_file,
                                cand.decl_span,
                            ),
                            reason: describe_basic_applicability_rejection(
                                BasicApplicabilityRejection {
                                    call_args: &call_args,
                                    param_names: &cand.param_names,
                                    param_has_defaults: &cand.param_has_defaults,
                                    param_is_vararg: &cand.param_is_vararg,
                                    param_tys: &cand.params,
                                    source,
                                    lower,
                                    builtins,
                                },
                            ),
                        }
                    })
                    .collect();
                for owner_fqn in &ctor_owner_fqns {
                    rejections.extend(collect_ctor_overload_rejections_for_owner(
                        inputs,
                        owner_fqn,
                        callee_name,
                        &call_args,
                        None,
                        lower,
                    )?);
                }
                return Err(ExprTypeError::NoApplicableOverload {
                    callee: callee_name.to_string(),
                    candidates: join_overload_rejections(rejections),
                    span: call_expr.span.into(),
                });
            }
            if total_matched > 1 {
                let mut signatures: Vec<String> = matched
                    .iter()
                    .map(|c| {
                        let name = short_name_from_fqn(c.fqn).to_string();
                        fmt_overload_signature(&name, None, &c.instantiated.params, lower)
                    })
                    .collect();
                signatures.extend(matched_ctors.iter().map(|c| c.signature.clone()));
                return Err(ExprTypeError::AmbiguousOverload {
                    callee: callee_name.to_string(),
                    candidates: join_overload_signatures(signatures),
                    span: call_expr.span.into(),
                });
            }

            if matched.is_empty() {
                let chosen = matched_ctors
                    .pop()
                    .expect("single matched constructor candidate");
                lower.record_typechecked_ctor_call_binding(
                    call_expr.span,
                    chosen.owner_fqn.clone(),
                    chosen.ctor_span,
                    legacy_optional_mapping_from_param_mapping(&chosen.arg_mapping),
                );
                if let Some(binding) =
                    call_arg_binding_from_mapping(&chosen.arg_mapping, &call_args)
                {
                    lower.record_typechecked_call_arg_binding(call_expr.span, binding);
                }
                let ty = lower.lower_type_fqn_with_args(
                    chosen.owner_fqn,
                    chosen.inferred_type_args,
                    id.span,
                )?;
                return Ok(ty);
            }

            let chosen = matched.pop().expect("single matched function candidate");

            let chosen_fqn = chosen.fqn;
            check_unsafe_call_gate(chosen_fqn, chosen.sig, call_expr.span, lower)?;
            check_nogc_call_gate(chosen_fqn, chosen.sig, call_expr.span, lower)?;
            emit_deprecated_call_warning(chosen_fqn, chosen.sig, call_expr.span, lower);
            let chosen_call_args = if chosen.used_unit_sugar {
                sugar_call_args
                    .as_ref()
                    .expect("typed Unit sugar 选择的候选应有合成实参")
            } else {
                &call_args
            };
            check_var_param_lvalue_gate(chosen_fqn, chosen.sig, chosen_call_args, &chosen.mapping)?;

            // `@NoGC`：已知分配点（boxing）门禁。
            //
            // 说明：多候选路径中我们不会为所有实参做第二遍 expected-context 推断（避免额外副作用），
            // 这里用"预收集到的实参类型 + 已选定候选的期望实参类型"做最小判定即可：
            // - 若某个实参是值类型（或 type param 占位），且被期望类型吸收到引用类型，则需要 boxing；
            // - 在 `@NoGC` 上下文中应当保守拒绝。
            for (arg_idx, arg) in chosen_call_args.iter().enumerate() {
                let expected_ty = *chosen
                    .expected_arg_tys
                    .get(arg_idx)
                    .unwrap_or(&builtins.nothing);
                if expected_ty == builtins.nothing {
                    continue;
                }
                if is_type_assignable(arg.ty, expected_ty, lower, builtins) {
                    check_fn_value_to_any_erasure_gate(
                        arg.ty,
                        expected_ty,
                        arg.expr.span,
                        lower,
                        builtins,
                    )?;
                    check_nogc_boxing_gate(arg.ty, expected_ty, arg.expr.span, lower, builtins)?;
                }
            }

            // required effects（T0509/§14.7.1）：调用一个带 effect row 的函数，需要把该 row 计入当前函数体的 required effects。
            let type_param_bindings = type_param_bindings_from_sig(&chosen.sig.type_params, lower);
            let eff_bindings: Vec<(String, EffectRow)> = chosen
                .sig
                .eff_param
                .as_ref()
                .map(|p| vec![(p.name.clone(), chosen.eff_arg.clone())])
                .unwrap_or_default();
            let lowered_effects = lower.lower_effect_row_expr_in_decl_file_with_scopes(
                &chosen.sig.decl_file,
                type_param_bindings,
                eff_bindings,
                chosen.sig.effects.as_ref(),
            );
            let call_effects = substitute_type_args_in_effect_row(
                lowered_effects?,
                &chosen.sig.type_params,
                &chosen.instantiated.type_args,
                lower,
                call_expr.span,
            )?;
            for effect in call_effects.terms.iter().copied() {
                lower.record_performed_effect(effect, call_expr.span);
            }

            let eff_args = chosen
                .sig
                .eff_param
                .as_ref()
                .map(|_| vec![chosen.eff_arg.clone()])
                .unwrap_or_default();
            lower.record_monomorph_call(
                chosen_fqn.to_string(),
                &chosen.sig.decl_file,
                chosen.sig.decl_span,
                &chosen.instantiated.type_args,
                &eff_args,
                call_expr.span,
            );
            lower.record_top_level_fun_call_binding(
                call_expr.span,
                ast::TopLevelFunCallBinding {
                    fqn: chosen_fqn.to_string(),
                    decl_file: chosen.sig.decl_file.clone(),
                    decl_span: chosen.sig.decl_span,
                    is_intrinsic: chosen.sig.is_intrinsic,
                    intrinsic_entry_name: chosen.sig.intrinsic_entry_name.clone(),
                    type_args: chosen.instantiated.type_args.clone(),
                    eff_args,
                },
            );
            if let Some(binding) = call_arg_binding_from_mapping(&chosen.mapping, chosen_call_args)
            {
                lower.record_typechecked_call_arg_binding(call_expr.span, binding);
            }
            if chosen.used_unit_sugar {
                lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
            }

            Ok(chosen.instantiated.return_ty)
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            if let Some(ty) = try_infer_qualified_nominal_constructor_call_expr_type(
                inputs,
                call_expr,
                callee_expr,
                args,
                explicit_type_args.as_deref(),
                lower,
            )? {
                return Ok(ty);
            }

            if let Some(ty) = try_infer_qualified_enum_variant_ctor_call_expr_type(
                inputs, call_expr, member, args, lower,
            )? {
                return Ok(ty);
            }

            if let Some(ty) = infer_effect_op_call_expr_type(
                inputs,
                call_expr,
                member,
                args,
                explicit_type_args.as_deref(),
                lower,
            )? {
                return Ok(ty);
            }

            infer_member_call_expr_type(
                inputs,
                MemberCallRequest {
                    call_expr,
                    receiver: receiver.as_ref(),
                    member,
                    args,
                    explicit_type_args: explicit_type_args.as_deref(),
                    explicit_eff_arg: explicit_eff_arg.as_ref(),
                    safe: false,
                },
                lower,
            )
        }
        ast::ExprKind::SafeMemberAccess {
            receiver, member, ..
        } => infer_member_call_expr_type(
            inputs,
            MemberCallRequest {
                call_expr,
                receiver: receiver.as_ref(),
                member,
                args,
                explicit_type_args: explicit_type_args.as_deref(),
                explicit_eff_arg: explicit_eff_arg.as_ref(),
                safe: true,
            },
            lower,
        ),
        other => {
            let callee_ty = inputs.infer(lower, callee_expr)?;
            if matches!(
                lower.type_kind(callee_ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            ) {
                return infer_function_type_call_expr_type(
                    inputs,
                    call_expr,
                    expr_kind_name(other),
                    callee_ty,
                    args,
                    lower,
                );
            }

            if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(callee_ty)
                && nominal.fqn == FUNPTR_FQN
            {
                return infer_funptr_type_call_expr_type(
                    inputs,
                    call_expr,
                    expr_kind_name(other),
                    callee_ty,
                    args,
                    lower,
                );
            }

            Err(ExprTypeError::UnsupportedExpr {
                kind: expr_kind_name(other),
                span: callee.span.into(),
            })
        }
    }
}

pub(super) fn infer_unsafe_ptr_primitive_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee_name: &str,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let builtins = inputs.builtins;
    let primitive = match callee_name {
        "addrOf" | "load" | "store" => callee_name,
        _ => return Ok(None),
    };

    if !lower.in_unsafe_context() {
        return Err(ExprTypeError::UnsafePtrPrimitiveRequiresUnsafeContext {
            primitive: primitive.to_string(),
            span: call_expr.span.into(),
        });
    }

    // 当前阶段（T1009）实现为"语言内建函数"形态：
    // - `addrOf(x)`：返回 `Ptr<T>`（T 为 x 的类型）
    // - `load(p)`：`p: Ptr<T>` 时返回 `T`
    // - `store(p, v)`：`p: Ptr<T>` 且 `v: T`，返回 `Unit`
    let ptr_fqn = pick_ptr_type_fqn(lower);

    match primitive {
        "addrOf" => {
            if args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: primitive.to_string(),
                    expected: 1,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }

            let pointee_ty = inputs.infer(lower, &args[0])?;

            let ptr_ty = lower.lower_type_fqn_with_args(
                ptr_fqn.clone(),
                vec![pointee_ty],
                call_expr.span,
            )?;
            Ok(Some(ptr_ty))
        }
        "load" => {
            if args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: primitive.to_string(),
                    expected: 1,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }

            let ptr_arg_ty = inputs.infer(lower, &args[0])?;

            let Some(pointee) = extract_ptr_pointee(ptr_arg_ty, &ptr_fqn, lower) else {
                return Err(ExprTypeError::UnsafePtrPrimitiveRequiresPtrType {
                    primitive: primitive.to_string(),
                    found: lower.fmt_type(ptr_arg_ty),
                    span: args[0].span.into(),
                });
            };

            Ok(Some(pointee))
        }
        "store" => {
            if args.len() != 2 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: primitive.to_string(),
                    expected: 2,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }

            let ptr_arg_ty = inputs.infer(lower, &args[0])?;

            let Some(pointee) = extract_ptr_pointee(ptr_arg_ty, &ptr_fqn, lower) else {
                return Err(ExprTypeError::UnsafePtrPrimitiveRequiresPtrType {
                    primitive: primitive.to_string(),
                    found: lower.fmt_type(ptr_arg_ty),
                    span: args[0].span.into(),
                });
            };

            let value_ty = inputs.infer_in_expected(
                lower,
                &args[1],
                pointee,
                ExpectedTypeFrom::new("store 的 pointee 类型".to_string()),
            )?;

            if !is_type_assignable(value_ty, pointee, lower, builtins) {
                return Err(ExprTypeError::AssignmentTypeMismatch {
                    expected: lower.fmt_type(pointee),
                    found: lower.fmt_type(value_ty),
                    span: args[1].span.into(),
                });
            }

            Ok(Some(builtins.unit))
        }
        _ => Ok(None),
    }
}

pub(super) fn pick_ptr_type_fqn(lower: &TypeLowering<'_>) -> String {
    // 优先使用未来 sysroot 预计提供的 `scoop.unsafe.Ptr`（T1010）。
    if lower.env().type_symbol("scoop.unsafe.Ptr").is_some() {
        return "scoop.unsafe.Ptr".to_string();
    }

    // T1009 阶段允许 fixtures 在"当前包"内声明一个 `struct Ptr<T>` 作为最小落点。
    let pkg = lower.pkg_prefix();
    if pkg.is_empty() {
        return "Ptr".to_string();
    }

    let local = format!("{pkg}.Ptr");
    if lower.env().type_symbol(&local).is_some() {
        return local;
    }

    // 回退：交给后续 lowering 报更贴近语义的错误。
    "Ptr".to_string()
}

pub(super) fn extract_ptr_pointee(
    ptr_ty: TypeId,
    ptr_fqn: &str,
    lower: &TypeLowering<'_>,
) -> Option<TypeId> {
    match lower.type_kind(ptr_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n)) if n.fqn == ptr_fqn && n.args.len() == 1 => {
            Some(n.args[0])
        }
        TypeKind::Ref(RefTypeKind::Nominal(n)) if n.fqn == ptr_fqn && n.args.len() == 1 => {
            Some(n.args[0])
        }
        _ => None,
    }
}
