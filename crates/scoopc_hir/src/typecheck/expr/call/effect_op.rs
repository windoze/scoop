//! Member-call helpers (member fqn / instance type args), effect op + continuation resume call inference.

#![allow(dead_code)]

use std::collections::VecDeque;

use super::*;

pub(super) fn implicit_builtin_type_fqn(local_or_fqn: &str) -> Option<&'static str> {
    match local_or_fqn {
        // allow both `Int` and `scoop.core.Int` spellings
        "Any" | "scoop.core.Any" => Some("scoop.core.Any"),
        "String" | "scoop.core.String" => Some("scoop.core.String"),
        "Unit" | "scoop.core.Unit" => Some("scoop.core.Unit"),
        "Nothing" | "scoop.core.Nothing" => Some("scoop.core.Nothing"),
        "Bool" | "scoop.core.Bool" => Some("scoop.core.Bool"),
        "Char" | "scoop.core.Char" => Some("scoop.core.Char"),
        "Float64" | "scoop.core.Float64" => Some("scoop.core.Float64"),
        "Float32" | "scoop.core.Float32" => Some("scoop.core.Float32"),
        "Int" | "scoop.core.Int" => Some("scoop.core.Int"),
        "UInt" | "scoop.core.UInt" => Some("scoop.core.UInt"),
        "Int8" | "scoop.core.Int8" => Some("scoop.core.Int8"),
        "Int16" | "scoop.core.Int16" => Some("scoop.core.Int16"),
        "Int32" | "scoop.core.Int32" => Some("scoop.core.Int32"),
        "Int64" | "scoop.core.Int64" => Some("scoop.core.Int64"),
        "UInt8" | "scoop.core.UInt8" => Some("scoop.core.UInt8"),
        "UInt16" | "scoop.core.UInt16" => Some("scoop.core.UInt16"),
        "UInt32" | "scoop.core.UInt32" => Some("scoop.core.UInt32"),
        "UInt64" | "scoop.core.UInt64" => Some("scoop.core.UInt64"),
        "Option" | "scoop.core.Option" => Some("scoop.core.Option"),
        _ => None,
    }
}

pub(super) fn late_resolve_direct_member_fun_fqn_from_receiver_ty(
    inputs: ExprInferInputs<'_>,
    receiver_ty: TypeId,
    member_name: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<String>, ExprTypeError> {
    Ok(collect_member_method_signature_groups_from_receiver_ty(
        inputs,
        receiver_ty,
        member_name,
        lower,
    )?
    .into_iter()
    .next()
    .map(|(fqn, _)| fqn))
}

pub(in crate::typecheck::expr) fn collect_member_method_signature_groups_from_receiver_ty(
    inputs: ExprInferInputs<'_>,
    receiver_ty: TypeId,
    member_name: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<(String, Vec<FunSigOwned>)>, ExprTypeError> {
    if try_extract_member_call_receiver_fqn_and_args(receiver_ty, lower).is_none() {
        return Ok(Vec::new());
    }

    let mut visited: HashSet<TypeId> = HashSet::new();
    let mut queue: VecDeque<TypeId> = VecDeque::new();
    queue.push_back(receiver_ty);
    let mut seen_signatures: HashSet<(Vec<TypeId>, Vec<bool>)> = HashSet::new();
    let mut groups = Vec::new();

    while let Some(owner_ty) = queue.pop_front() {
        if !visited.insert(owner_ty) {
            continue;
        }

        // P4-T01l：对 builtin scalar / `String` receiver 统一走 nominal FQN 提取，让
        // `<scalar>.method()` 在 sysroot 提供 body method 时也能进入 direct-call 主线。
        let Some((owner_fqn, owner_args)) =
            try_extract_member_call_receiver_fqn_and_args(owner_ty, lower)
        else {
            continue;
        };

        let candidate_fqn = format!("{owner_fqn}.{member_name}");
        let mut sigs = collect_member_method_signatures_from_index(
            inputs.source,
            owner_ty,
            &owner_fqn,
            &owner_args,
            &candidate_fqn,
            lower,
            inputs.builtins,
        )?;
        sigs.retain(|sig| {
            let key = member_overload_signature_key(sig);
            seen_signatures.insert(key)
        });
        if !sigs.is_empty() {
            groups.push((candidate_fqn, sigs));
        }

        let mut super_tys = lower.instantiated_direct_supertypes(owner_ty)?;
        if let Some(super_fqns) = lower
            .env()
            .direct_supertypes(&owner_fqn)
            .map(|s| s.to_vec())
        {
            for super_fqn in super_fqns {
                if let Ok(super_ty) =
                    lower.lower_type_fqn_with_args(super_fqn, Vec::new(), Span::new(0, 0))
                {
                    super_tys.push(super_ty);
                }
            }
        }
        for super_ty in super_tys {
            queue.push_back(super_ty);
        }
    }

    Ok(groups)
}

fn member_overload_signature_key(sig: &FunSigOwned) -> (Vec<TypeId>, Vec<bool>) {
    (
        sig.params.iter().copied().skip(1).collect(),
        sig.param_is_vararg.iter().copied().skip(1).collect(),
    )
}

pub(in crate::typecheck::expr) fn combined_member_instance_type_args(
    callee_fqn: &str,
    receiver_ty: TypeId,
    fun_type_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<TypeId>, ExprTypeError> {
    let mut type_args = find_member_owner_nominal_instantiation(receiver_ty, callee_fqn, lower)?
        .map(|(_, owner_args, _)| owner_args)
        .unwrap_or_default();
    type_args.extend(fun_type_args.iter().copied());
    Ok(type_args)
}

#[derive(Debug, Clone)]
pub(in crate::typecheck::expr) struct LoweredEffectOpSig {
    pub(in crate::typecheck::expr) sig: FunSigOwned,
    pub(in crate::typecheck::expr) op_type_params: Vec<TypeId>,
    pub(in crate::typecheck::expr) effect_type_params: Vec<TypeId>,
}

pub(in crate::typecheck::expr) fn lower_effect_op_signature(
    op: &FunOverload,
    effect_sym: &TypeSymbol,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<LoweredEffectOpSig, ExprTypeError> {
    // effect op 的 type params 由两部分构成：
    // - operation 自身的 type params：例如 `effect Box<T> { fun <U> map(...): U }` 中的 `U`
    // - effect type 的 type params：例如 `effect Raise<in E> { fun raise(error: E): Nothing }` 中的 `E`
    //
    // 约定：把 op type params 放在前面，使 `Effect.op<T>(...)` 这类显式 type args
    // 按“函数泛型”直觉绑定到 op，而不是 effect type。
    let mut type_params: Vec<TypeId> = Vec::new();
    let mut bindings: Vec<(String, TypeId)> = Vec::new();
    let mut op_type_params: Vec<TypeId> = Vec::new();

    for tp in &op.sig.type_params {
        let param_ty =
            lower.ty_param_named(tp.name.clone(), op.symbol.decl_file.clone(), tp.name_span);
        type_params.push(param_ty);
        op_type_params.push(param_ty);
        bindings.push((tp.name.clone(), param_ty));
    }

    let mut effect_type_params: Vec<TypeId> = Vec::new();
    for name in &effect_sym.type_param_names {
        let param_ty =
            lower.ty_param_named(name.clone(), effect_sym.decl_file.clone(), effect_sym.span);
        type_params.push(param_ty);
        effect_type_params.push(param_ty);
        bindings.push((name.clone(), param_ty));
    }

    // receiver effect op 也统一按“receiver 作为显式第 0 个参数”进入调用绑定主线。
    let mut param_names: Vec<String> =
        Vec::with_capacity(op.sig.params.len() + usize::from(op.sig.receiver.is_some()));
    let mut params: Vec<TypeId> =
        Vec::with_capacity(op.sig.params.len() + usize::from(op.sig.receiver.is_some()));

    if let Some(receiver_ref) = &op.sig.receiver {
        param_names.push("receiver".to_string());
        let receiver_ty = lower.lower_type_ref_in_decl_file_with_bindings(
            &op.symbol.decl_file,
            bindings.clone(),
            receiver_ref,
        )?;
        params.push(receiver_ty);
    }

    for p in &op.sig.params {
        param_names.push(p.name.clone());

        let Some(ty_ref) = p.ty.as_ref() else {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "effect op param missing type",
                span: p.name_span.into(),
            });
        };

        let ty = lower.lower_type_ref_in_decl_file_with_bindings(
            &op.symbol.decl_file,
            bindings.clone(),
            ty_ref,
        )?;
        params.push(ty);
    }

    let return_ty = match &op.sig.return_ty {
        Some(ret) => lower.lower_type_ref_in_decl_file_with_bindings(
            &op.symbol.decl_file,
            bindings.clone(),
            ret,
        )?,
        None => builtins.unit,
    };

    let param_count = params.len();
    let sig = FunSigOwned {
        decl_span: op.symbol.span,
        decl_file: op.symbol.decl_file.clone(),
        is_extension: false,
        is_operator: false,
        is_unsafe: false,
        is_nogc: false,
        is_extern: false,
        is_intrinsic: false,
        intrinsic_entry_name: None,
        param_names,
        param_has_defaults: vec![false; param_count],
        param_is_vararg: vec![false; param_count],
        type_params,
        eff_param: None,
        param_fn_effect_eff_base: vec![None; param_count],
        param_nominal_eff_eff_base: vec![None; param_count],
        param_eff_row_var_subst: vec![EffRowVarSubstPlan::None; param_count],
        return_eff_row_var_subst: EffRowVarSubstPlan::None,
        params,
        return_ty,
        effects: None,
        where_constraints: Vec::new(),
    };

    Ok(LoweredEffectOpSig {
        sig,
        op_type_params,
        effect_type_params,
    })
}

pub(in crate::typecheck::expr) fn infer_effect_op_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    explicit_type_args: Option<&[TypeId]>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let builtins = inputs.builtins;

    let Some(ast::ResolvedMemberRef::Fun { fqn }) = member.resolved.as_ref() else {
        return Ok(None);
    };

    let callee_fqn = fqn.clone();

    // 仅当该 member 解析到一个 effect operation 时，本函数才接管类型检查逻辑；
    // 否则返回 None 让外层继续走 extension/member call 的路径。
    let op = lower.index().by_fqn.get(&callee_fqn).and_then(|syms| {
        syms.fun
            .iter()
            .find(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
            .cloned()
    });
    let Some(op) = op else {
        return Ok(None);
    };

    // effect op 的 qualifier 必须是 effect type（例如 `Raise.raise`），因此这里从 `a.B.op`
    // 反推 effect type FQN 为 `a.B`。
    let Some((effect_ty_fqn, _op_name)) = callee_fqn.rsplit_once('.') else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（bad fqn）",
            span: member.span.into(),
        });
    };

    let Some(effect_sym) = lower.env().type_symbol(effect_ty_fqn).cloned() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（missing effect type symbol）",
            span: member.span.into(),
        });
    };

    let lowered_sig = lower_effect_op_signature(&op, &effect_sym, lower, builtins)?;
    let sig = lowered_sig.sig;

    let used_unit_sugar = can_use_zero_arg_unit_call_sugar(
        args,
        &sig.params,
        &sig.param_has_defaults,
        &sig.param_is_vararg,
        lower,
    );
    let synthesized_args = used_unit_sugar.then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
    let call_args =
        collect_call_arg_infos(inputs, synthesized_args.as_deref().unwrap_or(args), lower)?;
    check_call_arg_named_rules(&callee_fqn, &call_args)?;
    check_call_named_args_exist_in_any_candidate(
        &callee_fqn,
        &call_args,
        std::iter::once(sig.param_names.as_slice()),
    )?;

    if call_args.len() != sig.params.len() {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_fqn,
            expected: sig.params.len(),
            found: call_args.len(),
            span: call_expr.span.into(),
        });
    }

    let Some(mapping) = map_call_args_to_params(&call_args, &sig.param_names) else {
        return Err(ExprTypeError::NoMatchingOverload {
            callee: callee_fqn,
            span: call_expr.span.into(),
        });
    };

    let instantiated = instantiate_fun_sig_for_call_with_optional_explicit_type_args(
        &callee_fqn,
        call_expr.span,
        &sig,
        explicit_type_args,
        mapping
            .iter()
            .copied()
            .enumerate()
            .map(|(param_idx, arg_idx)| {
                let arg = &call_args[arg_idx];
                GenericArgConstraint {
                    expected: sig.params[param_idx],
                    found: arg.ty,
                    found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                    from: format!("第 {} 个实参", arg_idx + 1),
                    span: arg.expr.span,
                }
            }),
        lower,
        builtins,
    )?;

    // T0129：检查 where 约束。
    check_fun_where_constraints_after_instantiation(
        &callee_fqn,
        call_expr.span,
        &sig,
        &instantiated.type_args,
        lower,
        builtins,
    )?;

    for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
        let arg = &call_args[arg_idx];
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

        if is_type_assignable(found_ty, expected_ty, lower, builtins) {
            continue;
        }
        if literal_absorbs_to_expected(arg.expr, expected_ty, inputs.source, lower, builtins) {
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

    let op_type_param_count = lowered_sig.op_type_params.len();
    let op_type_args = instantiated
        .type_args
        .iter()
        .copied()
        .take(op_type_param_count)
        .collect::<Vec<_>>();
    lower.record_typechecked_effect_op_call_binding(call_expr.span, mapping.clone(), op_type_args);
    if used_unit_sugar {
        lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
    }

    // required effects（T0604）：effect op call 视为"立即执行的 perform"，记录到当前函数体的 effects 集合中。
    let effect_param_count = lowered_sig.effect_type_params.len();
    let effect_type_args = if effect_param_count == 0 {
        Vec::new()
    } else if effect_param_count <= instantiated.type_args.len() {
        instantiated.type_args[instantiated.type_args.len() - effect_param_count..].to_vec()
    } else {
        // 理论上不应发生：type params 数量不足时 instantiate 已经报错。
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "effect op call（effect type args missing after instantiation）",
            span: call_expr.span.into(),
        });
    };

    let effect_instance = lower.lower_type_fqn_with_args(
        effect_ty_fqn.to_string(),
        effect_type_args,
        call_expr.span,
    )?;
    lower.record_inferred_performed_effect_ty(call_expr.span, effect_instance);
    lower.record_performed_effect(effect_instance, call_expr.span);

    Ok(Some(instantiated.return_ty))
}

pub(super) fn try_infer_continuation_resume_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    receiver_ty: TypeId,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    safe: bool,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let callee_name = "scoop.core.Continuation.resume";

    // spec §5.5：`k.resume(...)`。
    //
    // 说明：
    // - 当前阶段 typecheck 尚未支持 class/interface 的实例方法调用；因此这里把 `resume` 视为一个
    //   "内建 member call 形态"，独立于扩展函数解析。
    // - `Continuation<Resume, Answer, eff E>` 的 `E` 视为"调用 resume 可能执行的 required effects"；
    // - `resume(...): Answer` 的 authoritative 静态返回类型来自 receiver continuation 的
    //   第二个类型实参；safe-call `receiver?.resume(...)` 则进一步包成 `Option<Answer>`。
    if source.slice(member.span) != "resume" {
        return Ok(None);
    }

    let (expected_value_ty, answer_ty, effects) = match lower.type_kind(receiver_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
            if nominal.fqn == "scoop.core.Continuation" && nominal.args.len() >= 2 =>
        {
            (
                nominal.args[0],
                nominal.args[1],
                nominal.eff.unwrap_or_else(EffectRow::pure),
            )
        }
        _ => return Ok(None),
    };

    let param_names = vec!["value".to_string()];
    let param_has_defaults = vec![false];
    let param_is_vararg = vec![false];
    let used_unit_sugar = can_use_zero_arg_unit_call_sugar(
        args,
        std::slice::from_ref(&expected_value_ty),
        &param_has_defaults,
        &param_is_vararg,
        lower,
    );
    let synthesized_args = used_unit_sugar.then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
    let call_args =
        collect_call_arg_infos(inputs, synthesized_args.as_deref().unwrap_or(args), lower)?;
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
    if call_args.len() != 1 {
        return Err(ExprTypeError::CallArityMismatch {
            callee: callee_name.to_string(),
            expected: 1,
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
    let arg_idx = mapping[0];
    let value_expr = call_args[arg_idx].expr;
    let found_value_ty = inputs.infer_in_expected(
        lower,
        value_expr,
        expected_value_ty,
        ExpectedTypeFrom::new("Continuation.resume payload".to_string()),
    )?;

    if !is_type_assignable(found_value_ty, expected_value_ty, lower, builtins)
        && !literal_absorbs_to_expected(value_expr, expected_value_ty, source, lower, builtins)
    {
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: callee_name.to_string(),
            index: 1,
            expected: lower.fmt_type(expected_value_ty),
            found: lower.fmt_type(found_value_ty),
            span: value_expr.span.into(),
        });
    }

    for effect in effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_expr.span);
    }
    let runtime_error = lower.lower_type_fqn_with_args(
        "scoop.core.RuntimeError".to_string(),
        Vec::new(),
        call_expr.span,
    )?;
    let raise_runtime_error = lower.lower_type_fqn_with_args(
        "scoop.core.Raise".to_string(),
        vec![runtime_error],
        call_expr.span,
    )?;
    lower.record_performed_effect(raise_runtime_error, call_expr.span);
    let resolved_resume = ast::ResolvedMemberRef::Fun {
        fqn: callee_name.to_string(),
    };
    if safe {
        lower.record_safe_member_access_resolution(member.span, resolved_resume);
    } else {
        lower.record_typechecked_member_resolution(member.span, resolved_resume);
    }
    lower.record_continuation_resume_call_site(call_expr.span, !effects.is_pure());
    if used_unit_sugar {
        lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
    }

    let ret = if safe {
        lower.ty_option(answer_ty)
    } else {
        answer_ty
    };
    Ok(Some(ret))
}

pub(in crate::typecheck::expr) fn infer_continuation_resume_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee: &ast::Expr,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let callee_expr: &ast::Expr = match &callee.kind {
        ast::ExprKind::TypeApply {
            callee: inner,
            args,
        } => {
            let _explicit_apply_args = lower_explicit_type_apply_args(args, lower)?;
            inner.as_ref()
        }
        _ => callee,
    };

    let source = inputs.source;

    let (receiver, member, safe) = match &callee_expr.kind {
        ast::ExprKind::MemberAccess { receiver, member } => (receiver.as_ref(), member, false),
        ast::ExprKind::SafeMemberAccess {
            receiver, member, ..
        } => (receiver.as_ref(), member, true),
        _ => return Ok(None),
    };

    if source.slice(member.span) != "resume" {
        return Ok(None);
    }

    let receiver_ty = inputs.infer(lower, receiver)?;
    let actual_receiver_ty = if safe {
        match lower.type_kind(receiver_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
            _ => {
                return Err(ExprTypeError::SafeAccessReceiverNotNullable {
                    found: lower.fmt_type(receiver_ty),
                    span: receiver.span.into(),
                });
            }
        }
    } else {
        receiver_ty
    };

    try_infer_continuation_resume_call_expr_type(
        inputs,
        call_expr,
        actual_receiver_ty,
        member,
        args,
        safe,
        lower,
    )
}
