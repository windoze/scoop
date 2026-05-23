//! Function-value / funptr-value / top-level fun-value call inference.

#![allow(dead_code)]

use super::*;

pub(super) fn infer_function_type_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee_name: &str,
    callee_ty: TypeId,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let builtins = inputs.builtins;

    // `@NoGC`：保守门禁。
    //
    // 说明：当前阶段我们无法证明"某个函数值/闭包"是否为 `@NoGC`，
    // 因此在 `@NoGC` 上下文中一律拒绝这类调用（宁可误杀也不放过）。
    if lower.in_nogc_context() {
        return Err(ExprTypeError::NoGcCallForbidden {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    }

    let TypeKind::Ref(RefTypeKind::Function(fun)) = lower.type_kind(callee_ty) else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    };

    let mut param_tys = Vec::with_capacity(fun.params.len() + usize::from(fun.receiver.is_some()));
    if let Some(receiver_ty) = fun.receiver {
        param_tys.push(receiver_ty);
    }
    param_tys.extend(fun.params.iter().copied());
    let used_unit_sugar = can_use_zero_arg_unit_call_sugar(
        args,
        &param_tys,
        &vec![false; param_tys.len()],
        &vec![false; param_tys.len()],
        lower,
    );
    let synthesized_args = used_unit_sugar.then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
    let call_args = collect_call_arg_infos_allow_expected_type_placeholders(
        inputs,
        synthesized_args.as_deref().unwrap_or(args),
        lower,
    )?;
    let param_names = callable_value_param_names(&fun);
    let expected_arity = param_names.len();
    if call_args.iter().any(|arg| arg.is_spread) {
        let span = call_args
            .iter()
            .find(|arg| arg.is_spread)
            .map(|arg| arg.expr.span)
            .unwrap_or(call_expr.span);
        return Err(ExprTypeError::SpreadArgRequiresVararg {
            callee: callee_name.to_string(),
            span: span.into(),
        });
    }
    check_call_arg_named_rules(callee_name, &call_args)?;
    check_call_named_args_exist_in_any_candidate(
        callee_name,
        &call_args,
        std::iter::once(param_names.as_slice()),
    )?;

    if call_args.len() != expected_arity {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_name.to_string(),
            expected: expected_arity,
            found: call_args.len(),
            span: call_expr.span.into(),
        });
    }

    let Some(mapping) = map_call_args_to_params(&call_args, &param_names) else {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    };
    let mut arg_to_param: Vec<Option<usize>> = vec![None; call_args.len()];
    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
        let slot = arg_to_param
            .get_mut(arg_idx)
            .expect("mapped function-value arg index should stay in range");
        *slot = Some(param_idx);
    }

    let expected_arg_ty = |param_idx: usize| match fun.receiver {
        Some(receiver_ty) if param_idx == 0 => (receiver_ty, true, 0usize),
        Some(_) => (fun.params[param_idx - 1], false, param_idx),
        None => (fun.params[param_idx], false, param_idx + 1),
    };

    // 在"期望类型语境"下推导每个实参的最终类型（lambda 会在此处被真正类型检查）。
    let mut checked_arg_tys: Vec<TypeId> = Vec::with_capacity(call_args.len());
    for (arg_idx, arg) in call_args.iter().enumerate() {
        let param_idx = arg_to_param
            .get(arg_idx)
            .copied()
            .flatten()
            .expect("mapped function-value arg should have target param");
        let (expected_ty, is_receiver, display_idx) = expected_arg_ty(param_idx);
        let found_ty = inputs.infer_in_expected(
            lower,
            arg.expr,
            expected_ty,
            ExpectedTypeFrom::new(if is_receiver {
                format!("函数值 `{callee_name}` 的 receiver")
            } else {
                format!("函数值 `{callee_name}` 的第 {} 个参数", display_idx)
            }),
        )?;
        checked_arg_tys.push(found_ty);
    }

    // 再做"可赋值"检查（此时 lambda 的 effects 也已经被推断并写入 found_ty）。
    for (arg_idx, (arg, found_ty)) in call_args
        .iter()
        .zip(checked_arg_tys.iter().copied())
        .enumerate()
    {
        let param_idx = arg_to_param
            .get(arg_idx)
            .copied()
            .flatten()
            .expect("mapped function-value arg should have target param");
        let (expected_ty, is_receiver, display_idx) = expected_arg_ty(param_idx);
        if is_type_assignable(found_ty, expected_ty, lower, builtins) {
            check_fn_value_to_any_erasure_gate(
                found_ty,
                expected_ty,
                arg.expr.span,
                lower,
                builtins,
            )?;
            check_nogc_boxing_gate(found_ty, expected_ty, arg.expr.span, lower, builtins)?;
            continue;
        }
        // 整数字面量允许被上下文整数参数类型吸收（后续可加入 range check）。
        if literal_absorbs_to_expected(arg.expr, expected_ty, inputs.source, lower, builtins) {
            continue;
        }
        if is_receiver {
            return Err(ExprTypeError::CallReceiverTypeMismatch {
                callee: callee_name.to_string(),
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.expr.span.into(),
            });
        }
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: callee_name.to_string(),
            index: display_idx,
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: arg.expr.span.into(),
        });
    }

    let binding_mapping = mapping.iter().copied().map(Some).collect::<Vec<_>>();
    if let Some(binding) = call_arg_binding_from_optional_mapping(&binding_mapping, &call_args) {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }

    // required effects：调用一个带 effect row 的函数值，需要把该 row 计入当前函数体的 required effects。
    for effect in fun.effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_expr.span);
    }
    if used_unit_sugar {
        lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
    }

    Ok(fun.return_ty)
}

pub(super) fn infer_function_value_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee_name: &str,
    callee_decl_span: Span,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let Some(callee_ty) = inputs.locals.get(&callee_decl_span).copied() else {
        // 防御性：resolver 已把该引用绑定为 local，但 typecheck locals 未包含该 decl。
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "函数值调用（缺少局部绑定类型信息）",
            span: call_expr.span.into(),
        });
    };

    infer_function_type_call_expr_type(inputs, call_expr, callee_name, callee_ty, args, lower)
}

pub(super) fn infer_funptr_value_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee_name: &str,
    callee_decl_span: Span,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let Some(callee_ty) = inputs.locals.get(&callee_decl_span).copied() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "函数指针调用（缺少局部绑定类型信息）",
            span: call_expr.span.into(),
        });
    };

    infer_funptr_type_call_expr_type(inputs, call_expr, callee_name, callee_ty, args, lower)
}

pub(super) fn infer_funptr_type_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee_name: &str,
    callee_ty: TypeId,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let builtins = inputs.builtins;

    if !lower.in_unsafe_context() {
        return Err(ExprTypeError::FunPtrCallRequiresUnsafeContext {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    }

    let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(callee_ty) else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    };
    if nominal.fqn != FUNPTR_FQN || nominal.args.len() != 1 {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    }

    let sig_ty = nominal.args[0];
    let TypeKind::Ref(RefTypeKind::Function(fun)) = lower.type_kind(sig_ty) else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    };

    let mut param_tys = Vec::with_capacity(fun.params.len() + usize::from(fun.receiver.is_some()));
    if let Some(receiver_ty) = fun.receiver {
        param_tys.push(receiver_ty);
    }
    param_tys.extend(fun.params.iter().copied());
    let used_unit_sugar = can_use_zero_arg_unit_call_sugar(
        args,
        &param_tys,
        &vec![false; param_tys.len()],
        &vec![false; param_tys.len()],
        lower,
    );
    let synthesized_args = used_unit_sugar.then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
    let call_args = collect_call_arg_infos_allow_expected_type_placeholders(
        inputs,
        synthesized_args.as_deref().unwrap_or(args),
        lower,
    )?;
    let param_names = callable_value_param_names(&fun);
    let expected_arity = param_names.len();
    if call_args.iter().any(|arg| arg.is_spread) {
        let span = call_args
            .iter()
            .find(|arg| arg.is_spread)
            .map(|arg| arg.expr.span)
            .unwrap_or(call_expr.span);
        return Err(ExprTypeError::SpreadArgRequiresVararg {
            callee: callee_name.to_string(),
            span: span.into(),
        });
    }
    check_call_arg_named_rules(callee_name, &call_args)?;
    check_call_named_args_exist_in_any_candidate(
        callee_name,
        &call_args,
        std::iter::once(param_names.as_slice()),
    )?;

    if call_args.len() != expected_arity {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_name.to_string(),
            expected: expected_arity,
            found: call_args.len(),
            span: call_expr.span.into(),
        });
    }

    let Some(mapping) = map_call_args_to_params(&call_args, &param_names) else {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    };
    let mut arg_to_param: Vec<Option<usize>> = vec![None; call_args.len()];
    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
        let slot = arg_to_param
            .get_mut(arg_idx)
            .expect("mapped funptr arg index should stay in range");
        *slot = Some(param_idx);
    }

    let expected_arg_ty = |param_idx: usize| match fun.receiver {
        Some(receiver_ty) if param_idx == 0 => (receiver_ty, true, 0usize),
        Some(_) => (fun.params[param_idx - 1], false, param_idx),
        None => (fun.params[param_idx], false, param_idx + 1),
    };

    let mut checked_arg_tys: Vec<TypeId> = Vec::with_capacity(call_args.len());
    for (arg_idx, arg) in call_args.iter().enumerate() {
        let param_idx = arg_to_param
            .get(arg_idx)
            .copied()
            .flatten()
            .expect("mapped funptr arg should have target param");
        let (expected_ty, is_receiver, display_idx) = expected_arg_ty(param_idx);
        let found_ty = inputs.infer_in_expected(
            lower,
            arg.expr,
            expected_ty,
            ExpectedTypeFrom::new(if is_receiver {
                format!("函数指针 `{callee_name}` 的 receiver")
            } else {
                format!("函数指针 `{callee_name}` 的第 {} 个参数", display_idx)
            }),
        )?;
        checked_arg_tys.push(found_ty);
    }

    for (arg_idx, (arg, found_ty)) in call_args
        .iter()
        .zip(checked_arg_tys.iter().copied())
        .enumerate()
    {
        let param_idx = arg_to_param
            .get(arg_idx)
            .copied()
            .flatten()
            .expect("mapped funptr arg should have target param");
        let (expected_ty, is_receiver, display_idx) = expected_arg_ty(param_idx);
        if is_type_assignable(found_ty, expected_ty, lower, builtins) {
            check_fn_value_to_any_erasure_gate(
                found_ty,
                expected_ty,
                arg.expr.span,
                lower,
                builtins,
            )?;
            check_nogc_boxing_gate(found_ty, expected_ty, arg.expr.span, lower, builtins)?;
            continue;
        }
        if literal_absorbs_to_expected(arg.expr, expected_ty, inputs.source, lower, builtins) {
            continue;
        }
        if is_receiver {
            return Err(ExprTypeError::CallReceiverTypeMismatch {
                callee: callee_name.to_string(),
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.expr.span.into(),
            });
        }
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: callee_name.to_string(),
            index: display_idx,
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: arg.expr.span.into(),
        });
    }

    let binding_mapping = mapping.iter().copied().map(Some).collect::<Vec<_>>();
    if let Some(binding) = call_arg_binding_from_optional_mapping(&binding_mapping, &call_args) {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }

    for effect in fun.effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_expr.span);
    }
    if used_unit_sugar {
        lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
    }

    Ok(fun.return_ty)
}

pub(super) fn is_funptr_type(ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            nominal.fqn == FUNPTR_FQN && nominal.args.len() == 1
        }
        _ => false,
    }
}

pub(super) fn collect_top_level_fun_signatures_from_index(
    callee_fqn: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<Vec<FunSigOwned>, ExprTypeError> {
    // 先把 overload 列表复制出来，避免在持有 `lower.index()` 的不可变借用时再调用
    // `lower.lower_type_ref_in_decl_file(...)`（需要可变借用）。
    let overloads = match lower.index().by_fqn.get(callee_fqn) {
        Some(syms) => syms.fun.clone(),
        None => Vec::new(),
    };

    if overloads.is_empty() {
        return Ok(Vec::new());
    }

    // sysroot 的 "declaration-only" overload（`has_body = false`）用于 resolver/typecheck 可见性；
    // 但当当前编译单元提供了同签名的实现（`has_body = true`）时，
    // 若把两者同时暴露给重载决议，会导致"同签名重复候选 → ambiguous overload"。
    //
    // 因此这里先收集一份"已有实现的签名 key"，并在生成 `FunSigOwned` 时过滤掉同 key 的无 body 声明。
    fn normalize_sig_piece(s: &str) -> String {
        s.split_whitespace().collect()
    }

    fn fun_overload_sig_key(o: &crate::resolve::FunOverload, decl_source: &SourceFile) -> String {
        let mut out = String::new();
        out.push_str("fun|");
        for p in &o.sig.type_params {
            out.push_str(&p.name);
            out.push(',');
        }
        out.push('|');
        if let Some(eff) = &o.sig.eff_param {
            out.push_str(&normalize_sig_piece(decl_source.slice(eff.span)));
        }
        out.push('|');
        if let Some(receiver) = &o.sig.receiver {
            out.push_str(&normalize_sig_piece(decl_source.slice(receiver.span())));
        }
        out.push('|');
        for p in &o.sig.params {
            if let Some(ty) = &p.ty {
                out.push_str(&normalize_sig_piece(decl_source.slice(ty.span())));
            } else {
                out.push('_');
            }
            out.push(';');
        }
        out.push('|');
        match &o.sig.return_ty {
            Some(ret) => out.push_str(&normalize_sig_piece(decl_source.slice(ret.span()))),
            None => out.push_str("Unit"),
        }
        out.push('|');
        if let Some(effects) = &o.sig.effects {
            out.push_str(&normalize_sig_piece(decl_source.slice(effects.span)));
        }
        out
    }

    let mut implemented_keys: HashSet<String> = HashSet::new();
    for o in &overloads {
        if !o.has_body {
            continue;
        }

        let decl_source = lower
            .env()
            .source(&o.symbol.decl_file)
            .cloned()
            .ok_or_else(|| ExprTypeError::UnsupportedExpr {
                kind: "cross-file signature lowering（missing decl source）",
                span: o.symbol.span.into(),
            })?;
        implemented_keys.insert(fun_overload_sig_key(o, &decl_source));
    }

    let mut out: Vec<FunSigOwned> = Vec::new();
    for o in &overloads {
        let decl_source = lower
            .env()
            .source(&o.symbol.decl_file)
            .cloned()
            .ok_or_else(|| ExprTypeError::UnsupportedExpr {
                kind: "cross-file signature lowering（missing decl source）",
                span: o.symbol.span.into(),
            })?;

        if !o.has_body {
            let key = fun_overload_sig_key(o, &decl_source);
            if implemented_keys.contains(&key) {
                continue;
            }
        }

        // 注意：跨文件签名 lowering 在早期阶段曾只收集"单一 type param"的候选；
        // 但随着泛型实例化能力扩展（例如 `Ptr<T>.cast<U>()` 需要 2 个 type params），
        // 这里需要允许多 type params 的函数进入候选集，由 typecheck 在调用点做推断/门禁。

        // `FunSigOwned` 要求"扩展函数 receiver 降糖为第一个参数"；这里与
        // `collect_top_level_fun_signatures` 的约定保持一致。
        let is_extension = o.sig.receiver.is_some();

        let mut type_params: Vec<TypeId> = Vec::with_capacity(o.sig.type_params.len());
        for p in &o.sig.type_params {
            type_params.push(lower.ty_param_named(
                p.name.clone(),
                o.symbol.decl_file.clone(),
                p.name_span,
            ));
        }
        let type_param_bindings = o
            .sig
            .type_params
            .iter()
            .zip(type_params.iter().copied())
            .map(|(p, ty)| (p.name.clone(), ty))
            .collect::<Vec<_>>();

        // T0509：effect row 参数（`<eff E = Pure>`）。
        //
        // 说明：
        // - 跨文件签名里可能出现 `(...) -> T / E`；因此这里需要先把 `E` 绑定到默认值（缺省 Pure），
        //   让类型能顺利 lowering；
        // - 调用点会再根据 lambda body 推断出 `E_arg` 并实例化替换。
        let eff_param_sig = if let Some(eff_param) = &o.sig.eff_param {
            let name = eff_param.name.text(&decl_source).to_string();
            let default = match eff_param.default.as_ref() {
                Some(expr) => lower.lower_effect_row_expr_in_decl_file_with_bindings(
                    &o.symbol.decl_file,
                    type_param_bindings.iter().cloned(),
                    Some(expr),
                )?,
                None => EffectRow::pure(),
            };
            Some(EffParamSig { name, default })
        } else {
            None
        };
        let eff_bindings: Vec<(String, EffectRow)> = eff_param_sig
            .as_ref()
            .map(|p| vec![(p.name.clone(), p.default.clone())])
            .unwrap_or_default();

        let mut param_names = Vec::with_capacity(o.sig.params.len() + usize::from(is_extension));
        let mut param_has_defaults =
            Vec::with_capacity(o.sig.params.len() + usize::from(is_extension));
        let mut param_is_vararg =
            Vec::with_capacity(o.sig.params.len() + usize::from(is_extension));
        let mut params = Vec::with_capacity(o.sig.params.len() + usize::from(is_extension));

        if let Some(receiver) = &o.sig.receiver {
            param_names.push("<receiver>".to_string());
            param_has_defaults.push(false);
            param_is_vararg.push(false);
            let receiver_ty = lower.lower_type_ref_in_decl_file_with_scopes(
                &o.symbol.decl_file,
                type_param_bindings.iter().cloned(),
                eff_bindings.clone(),
                receiver,
            )?;
            params.push(receiver_ty);
        }

        for p in &o.sig.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            param_names.push(p.name.clone());
            param_has_defaults.push(p.has_default);
            param_is_vararg.push(false);
            let ty = lower.lower_type_ref_in_decl_file_with_scopes(
                &o.symbol.decl_file,
                type_param_bindings.iter().cloned(),
                eff_bindings.clone(),
                ty_ref,
            )?;
            params.push(ty);
        }

        let return_ty = match &o.sig.return_ty {
            Some(ret) => lower.lower_type_ref_in_decl_file_with_scopes(
                &o.symbol.decl_file,
                type_param_bindings.iter().cloned(),
                eff_bindings.clone(),
                ret,
            )?,
            None => builtins.unit,
        };

        // T0509/T0628b：为跨文件签名补齐 `eff` row 参数相关的基底与替换计划，
        // 以便调用点可以从 lambda body 推断 `E` 并实例化替换。
        let mut param_fn_effect_eff_base: Vec<Option<EffectRow>> = Vec::with_capacity(params.len());
        let mut param_nominal_eff_eff_base: Vec<Option<EffectRow>> =
            Vec::with_capacity(params.len());
        let mut param_eff_row_var_subst: Vec<EffRowVarSubstPlan> = Vec::with_capacity(params.len());

        if let Some(receiver_ref) = &o.sig.receiver {
            param_fn_effect_eff_base.push(None);
            let nominal_eff_base = if let Some(eff_param) = &eff_param_sig {
                type_ref_nominal_eff_eff_base(receiver_ref, &eff_param.name, &decl_source, lower)?
            } else {
                None
            };
            param_nominal_eff_eff_base.push(nominal_eff_base);
            let subst_plan = if let Some(eff_param) = &eff_param_sig {
                build_eff_row_var_subst_plan(
                    receiver_ref,
                    params[0],
                    &eff_param.name,
                    &decl_source,
                    lower,
                )?
            } else {
                EffRowVarSubstPlan::None
            };
            param_eff_row_var_subst.push(subst_plan);
        }

        let mut param_pos = usize::from(is_extension);
        for p in &o.sig.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            let param_ty = params[param_pos];
            param_pos += 1;
            let fn_eff_base = if let Some(eff_param) = &eff_param_sig {
                type_ref_fn_effect_eff_base(ty_ref, &eff_param.name, &decl_source, lower)?
            } else {
                None
            };
            let nominal_eff_base = if let Some(eff_param) = &eff_param_sig {
                type_ref_nominal_eff_eff_base(ty_ref, &eff_param.name, &decl_source, lower)?
            } else {
                None
            };
            param_fn_effect_eff_base.push(fn_eff_base);
            param_nominal_eff_eff_base.push(nominal_eff_base);
            let subst_plan = if let Some(eff_param) = &eff_param_sig {
                build_eff_row_var_subst_plan(
                    ty_ref,
                    param_ty,
                    &eff_param.name,
                    &decl_source,
                    lower,
                )?
            } else {
                EffRowVarSubstPlan::None
            };
            param_eff_row_var_subst.push(subst_plan);
        }

        // T0129：从 resolve 的 FunSig 构建 where constraints。
        let where_constraints = build_fun_where_constraints_from_resolve_sig(
            &decl_source,
            &o.sig.type_params,
            o.sig.where_clause.as_ref(),
        );

        out.push(FunSigOwned {
            decl_span: o.symbol.span,
            decl_file: o.symbol.decl_file.clone(),
            is_extension,
            is_unsafe: o.sig.builtin_flags.is_unsafe,
            is_nogc: o.sig.builtin_flags.is_nogc,
            is_extern: o.sig.builtin_flags.is_extern,
            is_intrinsic: o.sig.builtin_flags.is_intrinsic,
            intrinsic_entry_name: o.sig.builtin_flags.intrinsic_entry_name.clone(),
            param_names,
            param_has_defaults,
            param_is_vararg,
            type_params: type_params.clone(),
            eff_param: eff_param_sig.clone(),
            param_fn_effect_eff_base,
            param_nominal_eff_eff_base,
            param_eff_row_var_subst,
            return_eff_row_var_subst: EffRowVarSubstPlan::None,
            params,
            return_ty,
            effects: o.sig.effects.clone(),
            where_constraints,
        });
    }

    Ok(out)
}

pub(super) fn is_top_level_fun_value_candidate_expr(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<bool, ExprTypeError> {
    let Some((callee_fqn, _)) = extract_top_level_fun_value_target(expr, lower)? else {
        return Ok(false);
    };

    if inputs.top_level_types.contains_key(&callee_fqn) || lower.is_object_type(&callee_fqn) {
        return Ok(false);
    }

    let sigs_from_index: Vec<FunSigOwned>;
    let sigs: &[FunSigOwned] = match inputs.top_level_funs.get(&callee_fqn) {
        Some(sigs) => sigs.as_slice(),
        None => {
            sigs_from_index =
                collect_top_level_fun_signatures_from_index(&callee_fqn, lower, inputs.builtins)?;
            sigs_from_index.as_slice()
        }
    };

    Ok(!sigs.is_empty())
}

pub(super) fn default_eff_arg_for_fun_sig(sig: &FunSigOwned) -> EffectRow {
    sig.eff_param
        .as_ref()
        .map(|p| p.default.clone())
        .unwrap_or_else(EffectRow::pure)
}

pub(super) fn function_type_shape_from_sig_params(
    sig: &FunSigOwned,
    params: &[TypeId],
    return_ty: TypeId,
    effects: EffectRow,
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<TypeId, ExprTypeError> {
    let (receiver, positional_params): (Option<TypeId>, Vec<TypeId>) = if sig.is_extension {
        let Some(receiver) = params.first().copied() else {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "top-level function value（extension receiver 缺失）",
                span: use_span.into(),
            });
        };
        (Some(receiver), params[1..].to_vec())
    } else {
        (None, params.to_vec())
    };

    Ok(lower.ty_function(
        receiver,
        positional_params,
        return_ty,
        effects,
        sig.effects.as_ref().is_some_and(|row| row.closed),
    ))
}

pub(super) fn function_value_type_from_instantiated_sig(
    sig: &FunSigOwned,
    instantiated: &InstantiatedFunSig,
    eff_arg: &EffectRow,
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<TypeId, ExprTypeError> {
    let mut instantiated = instantiated.clone();
    instantiate_eff_row_var_in_sig_types(sig, &mut instantiated, eff_arg, lower, use_span)?;

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
    )?;
    let effects = substitute_type_args_in_effect_row(
        lowered_effects,
        &sig.type_params,
        &instantiated.type_args,
        lower,
        use_span,
    )?;

    function_type_shape_from_sig_params(
        sig,
        &instantiated.params,
        instantiated.return_ty,
        effects,
        lower,
        use_span,
    )
}

pub(super) fn generic_constraints_from_expected_fun_value(
    sig: &FunSigOwned,
    expected_fun_ty: TypeId,
    expected_from: &ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<Vec<GenericArgConstraint>, ExprTypeError> {
    if sig.type_params.is_empty() {
        return Ok(Vec::new());
    }

    let placeholder = InstantiatedFunSig {
        params: sig.params.clone(),
        return_ty: sig.return_ty,
        type_args: Vec::new(),
    };
    let placeholder_fun_ty = function_value_type_from_instantiated_sig(
        sig,
        &placeholder,
        &default_eff_arg_for_fun_sig(sig),
        lower,
        use_span,
    )?;

    Ok(vec![GenericArgConstraint {
        expected: placeholder_fun_ty,
        found: expected_fun_ty,
        found_is_placeholder: false,
        from: format!("值位置的期望函数类型（约束来源：{}）", expected_from.desc()),
        span: use_span,
    }])
}

pub(super) fn extract_top_level_fun_value_target(
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<(String, ExplicitTypeApplyArgs)>, ExprTypeError> {
    match &expr.kind {
        ast::ExprKind::Ident(id) => {
            let Some(ast::ResolvedValueRef::TopLevel { fqn }) = id.resolved.as_ref() else {
                return Ok(None);
            };
            Ok(Some((fqn.clone(), ExplicitTypeApplyArgs::default())))
        }
        ast::ExprKind::TypeApply { callee, args } => {
            let ast::ExprKind::Ident(id) = &callee.kind else {
                return Ok(None);
            };
            let Some(ast::ResolvedValueRef::TopLevel { fqn }) = id.resolved.as_ref() else {
                return Ok(None);
            };
            Ok(Some((
                fqn.clone(),
                lower_explicit_type_apply_args(args, lower)?,
            )))
        }
        _ => Ok(None),
    }
}

pub(super) fn lower_explicit_type_apply_args(
    args: &[ast::TypeRef],
    lower: &mut TypeLowering<'_>,
) -> Result<ExplicitTypeApplyArgs, ExprTypeError> {
    let mut lowered = ExplicitTypeApplyArgs::default();
    for arg in args {
        match arg {
            ast::TypeRef::EffectRowArg { row, .. } => {
                if lowered.eff_arg.is_none() {
                    lowered.eff_arg =
                        Some(lower.lower_effect_row_expr_preserving_params(Some(row))?);
                }
            }
            other => lowered.type_args.push(lower.lower_type_ref(other)?),
        }
    }
    Ok(lowered)
}

pub(in crate::typecheck::expr) fn infer_top_level_fun_value_expr_type(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    expected_ty: Option<TypeId>,
    expected_from: Option<&ExpectedTypeFrom>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let Some((callee_fqn, explicit_apply_args)) = extract_top_level_fun_value_target(expr, lower)?
    else {
        return Ok(None);
    };

    let builtins = inputs.builtins;
    let top_level_funs = inputs.top_level_funs;

    let sigs_from_index: Vec<FunSigOwned>;
    let sigs: &[FunSigOwned] = match top_level_funs.get(&callee_fqn) {
        Some(sigs) => sigs.as_slice(),
        None => {
            sigs_from_index =
                collect_top_level_fun_signatures_from_index(&callee_fqn, lower, builtins)?;
            sigs_from_index.as_slice()
        }
    };
    if sigs.is_empty() {
        return Ok(None);
    }

    let expected_fun_ty = expected_ty.and_then(|ty| match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Function(_)) => Some(ty),
        _ => None,
    });
    let callee_name = short_name_from_fqn(&callee_fqn).to_string();

    #[derive(Clone)]
    struct MatchCandidate {
        sig: FunSigOwned,
        instantiated: InstantiatedFunSig,
        eff_arg: EffectRow,
        fun_ty: TypeId,
    }

    let mut matches: Vec<MatchCandidate> = Vec::new();
    let mut first_error: Option<ExprTypeError> = None;

    for sig in sigs {
        if explicit_apply_args.eff_arg.is_some() && sig.eff_param.is_none() {
            continue;
        }

        let constraints = match (expected_fun_ty, expected_from) {
            (Some(expected_fun_ty), Some(expected_from)) => {
                generic_constraints_from_expected_fun_value(
                    sig,
                    expected_fun_ty,
                    expected_from,
                    lower,
                    expr.span,
                )?
            }
            _ => Vec::new(),
        };

        let instantiated = match instantiate_fun_sig_for_call_with_optional_explicit_type_args(
            &callee_name,
            expr.span,
            sig,
            (!explicit_apply_args.type_args.is_empty())
                .then_some(explicit_apply_args.type_args.as_slice()),
            constraints,
            lower,
            builtins,
        ) {
            Ok(instantiated) => instantiated,
            Err(err) => {
                if first_error.is_none() {
                    first_error = Some(err);
                }
                continue;
            }
        };

        let eff_arg = explicit_apply_args
            .eff_arg
            .clone()
            .unwrap_or_else(|| default_eff_arg_for_fun_sig(sig));
        let fun_ty = function_value_type_from_instantiated_sig(
            sig,
            &instantiated,
            &eff_arg,
            lower,
            expr.span,
        )?;

        if let Some(expected_fun_ty) = expected_fun_ty
            && !is_type_assignable(fun_ty, expected_fun_ty, lower, builtins)
        {
            continue;
        }

        matches.push(MatchCandidate {
            sig: sig.clone(),
            instantiated,
            eff_arg,
            fun_ty,
        });
    }

    let selected = match matches.len() {
        0 => {
            if let Some(err) = first_error
                && (sigs.len() == 1
                    || !explicit_apply_args.type_args.is_empty()
                    || explicit_apply_args.eff_arg.is_some())
            {
                return Err(err);
            }

            if expected_fun_ty.is_some()
                || !explicit_apply_args.type_args.is_empty()
                || explicit_apply_args.eff_arg.is_some()
            {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_name,
                    span: expr.span.into(),
                });
            }

            return Ok(None);
        }
        1 => matches.pop().unwrap(),
        _ => {
            let candidates = matches
                .iter()
                .map(|cand| match lower.type_kind(cand.fun_ty) {
                    TypeKind::Ref(RefTypeKind::Function(fun)) => {
                        fmt_overload_signature(&callee_name, fun.receiver, &fun.params, lower)
                    }
                    _ => callee_name.clone(),
                })
                .collect::<Vec<_>>();
            return Err(ExprTypeError::AmbiguousOverload {
                callee: callee_name,
                candidates: join_overload_signatures(candidates),
                span: expr.span.into(),
            });
        }
    };

    let eff_args = selected
        .sig
        .eff_param
        .as_ref()
        .map(|_| vec![selected.eff_arg.clone()])
        .unwrap_or_default();
    lower.record_monomorph_call(
        callee_fqn.clone(),
        &selected.sig.decl_file,
        selected.sig.decl_span,
        &selected.instantiated.type_args,
        &eff_args,
        expr.span,
    );
    lower.emit_deprecated_fun_use(
        &callee_fqn,
        &selected.sig.decl_file,
        selected.sig.decl_span,
        expr.span,
    );
    lower.record_top_level_fun_value_ref(
        expr.span,
        callee_fqn,
        selected.sig.decl_file.clone(),
        selected.sig.decl_span,
        selected.instantiated.type_args.clone(),
        eff_args,
    );

    Ok(Some(selected.fun_ty))
}
