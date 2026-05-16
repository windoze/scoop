//! Call argument collection / param-arg mapping / vararg / default-arg helpers.

#![allow(dead_code)]

use super::*;

/// 把 AST 的调用实参列表归一化为"用于重载筛选"的结构，并预先推导每个实参表达式的类型。
///
/// 说明：
/// - `ExprKind::NamedArg { name = value }` 在调用语境内是"语法糖节点"，其类型应以 `value` 为准；
/// - 这里提前推导所有实参类型，保证：
///   - 子表达式的类型错误不会被重载筛选吞掉；
///   - 后续候选过滤只做纯比较，不再递归进入表达式树。
pub(super) fn infer_call_arg_info_ty(
    inputs: ExprInferInputs<'_>,
    expr_for_ty: &ast::Expr,
    allow_expected_type_placeholder: bool,
    lower: &mut TypeLowering<'_>,
) -> Result<(TypeId, bool), ExprTypeError> {
    match expr_for_ty.kind {
        // lambda 的类型通常依赖 expected type；在"预收集实参信息"阶段先用占位类型，
        // 以便后续在"已选定签名"的语境下重新 typecheck（T0504）。
        ast::ExprKind::Lambda(_) => Ok((inputs.builtins.any, true)),
        _ if is_top_level_fun_value_candidate_expr(inputs, expr_for_ty, lower)? => {
            Ok((inputs.builtins.any, true))
        }
        _ => match inputs.infer(lower, expr_for_ty) {
            Ok(ty) => Ok((ty, false)),
            Err(ExprTypeError::AmbiguousEnumVariantCtor { .. })
                if allow_expected_type_placeholder =>
            {
                // 允许诸如 `Cell(None())` 这类由构造参数 expected type 才能消歧的实参先占位，
                // 等候选构造函数参数类型确定后再在 expected-context 中重做 typecheck。
                Ok((inputs.builtins.any, true))
            }
            Err(err) => Err(err),
        },
    }
}

pub(in crate::typecheck::expr) fn collect_call_arg_infos<'a>(
    inputs: ExprInferInputs<'_>,
    args: &'a [ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<CallArgInfo<'a>>, ExprTypeError> {
    collect_call_arg_infos_impl(inputs, args, lower, false)
}

pub(in crate::typecheck::expr) fn collect_call_arg_infos_allow_expected_type_placeholders<'a>(
    inputs: ExprInferInputs<'_>,
    args: &'a [ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<CallArgInfo<'a>>, ExprTypeError> {
    collect_call_arg_infos_impl(inputs, args, lower, true)
}

pub(super) fn collect_call_arg_infos_impl<'a>(
    inputs: ExprInferInputs<'_>,
    args: &'a [ast::Expr],
    lower: &mut TypeLowering<'_>,
    allow_expected_type_placeholder: bool,
) -> Result<Vec<CallArgInfo<'a>>, ExprTypeError> {
    let mut out: Vec<CallArgInfo<'a>> = Vec::with_capacity(args.len());

    for arg in args {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => {
                let name_text = inputs.source.slice(name.span).to_string();
                let name_span = name.span;
                let expr = value.as_ref();
                let (expr_for_ty, is_spread) = match &expr.kind {
                    ast::ExprKind::SpreadArg { expr: inner, .. } => (inner.as_ref(), true),
                    _ => (expr, false),
                };
                let (ty, needs_expected_type) = infer_call_arg_info_ty(
                    inputs,
                    expr_for_ty,
                    allow_expected_type_placeholder,
                    lower,
                )?;
                out.push(CallArgInfo {
                    kind: CallArgKind::Named {
                        name: name_text,
                        name_span,
                    },
                    expr,
                    ty,
                    is_spread,
                    needs_expected_type,
                });
            }
            _ => {
                let (expr_for_ty, is_spread) = match &arg.kind {
                    ast::ExprKind::SpreadArg { expr: inner, .. } => (inner.as_ref(), true),
                    _ => (arg, false),
                };
                let (ty, needs_expected_type) = infer_call_arg_info_ty(
                    inputs,
                    expr_for_ty,
                    allow_expected_type_placeholder,
                    lower,
                )?;
                out.push(CallArgInfo {
                    kind: CallArgKind::Positional,
                    expr: arg,
                    ty,
                    is_spread,
                    needs_expected_type,
                });
            }
        }
    }

    Ok(out)
}

pub(super) fn call_args_have_named(call_args: &[CallArgInfo<'_>]) -> bool {
    call_args
        .iter()
        .any(|a| matches!(a.kind, CallArgKind::Named { .. }))
}

pub(super) fn first_named_arg_span(call_args: &[CallArgInfo<'_>]) -> Option<Span> {
    call_args.iter().find_map(|arg| match arg.kind {
        CallArgKind::Named { name_span, .. } => Some(name_span),
        CallArgKind::Positional => None,
    })
}

pub(super) fn funptr_invoke_rejects_named_args(
    callee_fqn: &str,
    receiver_ty: TypeId,
    lower: &TypeLowering<'_>,
) -> bool {
    callee_fqn == "scoop.unsafe.invoke" && is_funptr_type(receiver_ty, lower)
}

pub(super) fn missing_required_param_names_in_named_call(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
    param_has_defaults: &[bool],
) -> Option<Vec<String>> {
    if param_names.len() != param_has_defaults.len() {
        return None;
    }
    if !call_args_have_named(call_args) {
        return None;
    }

    // Kotlin-like：一旦出现命名实参，后续实参必须全部为命名。
    let mut seen_named = false;
    let mut positional_count = 0usize;
    for arg in call_args {
        match &arg.kind {
            CallArgKind::Positional => {
                if seen_named {
                    return None;
                }
                positional_count += 1;
            }
            CallArgKind::Named { .. } => {
                seen_named = true;
            }
        }
    }
    if positional_count > param_names.len() {
        return None;
    }

    let mut mapping: Vec<Option<usize>> = vec![None; param_names.len()];

    // 位置实参：按从左到右依次绑定到形参（不跳槽）。
    for arg_idx in 0..positional_count {
        let slot = mapping.get_mut(arg_idx)?;
        *slot = Some(arg_idx);
    }

    // 命名实参：按 name 匹配形参槽位。
    for (arg_idx, arg) in call_args.iter().enumerate().skip(positional_count) {
        let CallArgKind::Named { name, .. } = &arg.kind else {
            return None;
        };
        let slot_idx = param_names.iter().position(|p| p == name)?;
        let slot = mapping.get_mut(slot_idx)?;
        if slot.is_some() {
            // 同一形参不能被重复赋值（位置 + 命名 / 命名重复）。
            return None;
        }
        *slot = Some(arg_idx);
    }

    let mut missing: Vec<String> = Vec::new();
    for (idx, arg_idx) in mapping.iter().enumerate() {
        if arg_idx.is_some() {
            continue;
        }
        if !param_has_defaults.get(idx).copied().unwrap_or(false) {
            missing.push(param_names[idx].clone());
        }
    }

    if missing.is_empty() {
        None
    } else {
        Some(missing)
    }
}

/// 检查调用点的命名参数规则（Kotlin-like）。
///
/// 当前阶段（T1306）强约束：
/// - 命名参数之后不能再出现位置参数；
/// - 同名命名参数不允许出现两次（无论是否能匹配到重载）。
pub(in crate::typecheck::expr) fn check_call_arg_named_rules(
    callee: &str,
    call_args: &[CallArgInfo<'_>],
) -> Result<(), ExprTypeError> {
    let mut seen_named = false;
    let mut seen_names: HashSet<&str> = HashSet::new();

    for (idx, arg) in call_args.iter().enumerate() {
        match &arg.kind {
            CallArgKind::Positional => {
                if seen_named {
                    // Kotlin-like：允许把 trailing lambda 写在命名参数之后。
                    //
                    // T1324：支持多个 trailing lambda，因此这里放开一个更一般的例外：
                    // - 一旦出现命名实参，后续若出现位置实参，则必须全部为"末尾连续的 lambda 实参"。
                    let all_trailing_lambdas = call_args[idx..].iter().all(|a| {
                        matches!(a.kind, CallArgKind::Positional)
                            && matches!(a.expr.kind, ast::ExprKind::Lambda(_))
                    });
                    if all_trailing_lambdas {
                        break;
                    }
                    return Err(ExprTypeError::CallArgPositionalAfterNamed {
                        callee: callee.to_string(),
                        span: arg.expr.span.into(),
                    });
                }
            }
            CallArgKind::Named { name, name_span } => {
                seen_named = true;
                if !seen_names.insert(name.as_str()) {
                    return Err(ExprTypeError::CallArgDuplicate {
                        callee: callee.to_string(),
                        name: name.clone(),
                        span: (*name_span).into(),
                    });
                }
            }
        }
    }

    Ok(())
}

pub(super) fn trailing_lambda_suffix_len(call_args: &[CallArgInfo<'_>]) -> usize {
    let mut n = 0usize;
    for arg in call_args.iter().rev() {
        if !matches!(arg.kind, CallArgKind::Positional) {
            break;
        }
        if !matches!(arg.expr.kind, ast::ExprKind::Lambda(_)) {
            break;
        }
        n += 1;
    }
    n
}

/// 若调用点包含命名实参，则检查这些 name 是否至少能匹配到一个候选签名的形参名集合。
///
/// 说明：
/// - 对于重载集合：若某个 name 不存在于任何候选签名的形参名中，则该调用必然非法；
/// - 这样可以在"重载筛选失败"之前给出更精确的 name-span 诊断（满足 fixtures 断言）。
pub(super) fn check_call_named_args_exist_in_any_candidate<'a>(
    callee: &str,
    call_args: &[CallArgInfo<'_>],
    candidate_param_names: impl IntoIterator<Item = &'a [String]>,
) -> Result<(), ExprTypeError> {
    if !call_args_have_named(call_args) {
        return Ok(());
    }

    let mut all_names: HashSet<String> = HashSet::new();
    for params in candidate_param_names {
        for p in params {
            all_names.insert(p.clone());
        }
    }

    for arg in call_args {
        let CallArgKind::Named { name, name_span } = &arg.kind else {
            continue;
        };
        if !all_names.contains(name) {
            return Err(ExprTypeError::UnknownCallArgName {
                callee: callee.to_string(),
                name: name.clone(),
                span: (*name_span).into(),
            });
        }
    }

    Ok(())
}

/// 将调用点的"位置/命名实参"映射到某个候选签名的形参槽位。
///
/// 当前阶段（T0453）最小规则：
/// - 不支持默认参数：必须为每个形参提供一个实参；
/// - 命名实参仅按"同名形参"匹配；
/// - 位置实参按从左到右填充尚未被命名实参占用的槽位。
pub(super) fn map_call_args_to_params(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
) -> Option<Vec<usize>> {
    if call_args.len() != param_names.len() {
        return None;
    }

    // Kotlin-like：一旦出现命名实参，后续实参必须全部为命名。
    let mut seen_named = false;
    let mut positional_count = 0usize;
    for arg in call_args {
        match &arg.kind {
            CallArgKind::Positional => {
                if seen_named {
                    return None;
                }
                positional_count += 1;
            }
            CallArgKind::Named { .. } => {
                seen_named = true;
            }
        }
    }

    let mut mapping: Vec<Option<usize>> = vec![None; param_names.len()];

    // 位置实参：按从左到右依次绑定到形参（不跳槽）。
    for arg_idx in 0..positional_count {
        let slot = mapping.get_mut(arg_idx)?;
        *slot = Some(arg_idx);
    }

    // 命名实参：按 name 匹配形参槽位。
    for (arg_idx, arg) in call_args.iter().enumerate().skip(positional_count) {
        let CallArgKind::Named { name, .. } = &arg.kind else {
            // 已被 `seen_named` 规则拒绝；防御性处理。
            return None;
        };
        let slot_idx = param_names.iter().position(|p| p == name)?;
        let slot = mapping.get_mut(slot_idx)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(arg_idx);
    }

    if mapping.iter().any(|x| x.is_none()) {
        return None;
    }

    Some(mapping.into_iter().map(|x| x.expect("checked")).collect())
}

/// 将调用点的"位置/命名实参"映射到某个候选签名的形参槽位（支持默认参数）。
///
/// 当前阶段（T0454）最小规则：
/// - 允许省略带默认值的形参；
/// - 命名实参仅按"同名形参"匹配；
/// - 位置实参按从左到右填充尚未被命名实参占用的槽位；
/// - 若某个未填充的槽位没有默认值，则该候选不匹配。
pub(super) fn map_call_args_to_params_with_defaults_strict(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
    param_has_defaults: &[bool],
) -> Option<Vec<Option<usize>>> {
    if param_names.len() != param_has_defaults.len() {
        return None;
    }

    // 默认参数允许"少传"，但不能"多传"。
    if call_args.len() > param_names.len() {
        return None;
    }

    // 最少需要提供的实参数量：无默认值的形参个数。
    let required = param_has_defaults.iter().filter(|d| !**d).count();
    if call_args.len() < required {
        return None;
    }

    // Kotlin-like：一旦出现命名实参，后续实参必须全部为命名。
    let mut seen_named = false;
    let mut positional_count = 0usize;
    for arg in call_args {
        match &arg.kind {
            CallArgKind::Positional => {
                if seen_named {
                    return None;
                }
                positional_count += 1;
            }
            CallArgKind::Named { .. } => {
                seen_named = true;
            }
        }
    }

    let mut mapping: Vec<Option<usize>> = vec![None; param_names.len()];

    // 位置实参：按从左到右依次绑定到形参（不跳槽）。
    for arg_idx in 0..positional_count {
        let slot = mapping.get_mut(arg_idx)?;
        *slot = Some(arg_idx);
    }

    // 命名实参：按 name 匹配形参槽位。
    for (arg_idx, arg) in call_args.iter().enumerate().skip(positional_count) {
        let CallArgKind::Named { name, .. } = &arg.kind else {
            return None;
        };
        let slot_idx = param_names.iter().position(|p| p == name)?;
        let slot = mapping.get_mut(slot_idx)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(arg_idx);
    }

    // 未填充的槽位必须有默认值。
    for (idx, arg_idx) in mapping.iter().copied().enumerate() {
        if arg_idx.is_some() {
            continue;
        }
        if !param_has_defaults.get(idx).copied().unwrap_or(false) {
            return None;
        }
    }

    Some(mapping)
}

pub(in crate::typecheck::expr) fn map_call_args_to_params_with_defaults(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
    param_has_defaults: &[bool],
) -> Option<Vec<Option<usize>>> {
    // 先尝试严格规则（与已有 fixtures 行为保持一致）。
    if let Some(mapping) =
        map_call_args_to_params_with_defaults_strict(call_args, param_names, param_has_defaults)
    {
        return Some(mapping);
    }

    // fallback：trailing lambda + 默认参数交互（Kotlin-like）。
    //
    // 允许：
    // - 调用点末尾连续 N 个实参为 lambda（trailing lambdas）；
    // - 这些 lambda 绑定到"最后 N 个形参槽位"；
    // - 中间缺失的槽位若有默认值则可省略（用于 `f(1) { ... }` 匹配 `f(x, y = 0, block)`）。
    let k = trailing_lambda_suffix_len(call_args);
    if k == 0 {
        return None;
    }
    if param_names.len() < k {
        return None;
    }

    let prefix_args = call_args.get(..call_args.len().saturating_sub(k))?;
    let prefix_param_names = param_names.get(..param_names.len().saturating_sub(k))?;
    let prefix_param_has_defaults =
        param_has_defaults.get(..param_has_defaults.len().saturating_sub(k))?;

    let mut mapping = map_call_args_to_params_with_defaults_strict(
        prefix_args,
        prefix_param_names,
        prefix_param_has_defaults,
    )?;
    for arg_idx in (call_args.len().saturating_sub(k))..call_args.len() {
        mapping.push(Some(arg_idx));
    }
    Some(mapping)
}

pub(super) fn callable_value_param_names(fun: &crate::ty::FunctionType) -> Vec<String> {
    let mut out = Vec::with_capacity(fun.params.len() + usize::from(fun.receiver.is_some()));
    if fun.receiver.is_some() {
        out.push("receiver".to_string());
    }
    for idx in 0..fun.params.len() {
        out.push(format!("a{idx}"));
    }
    out
}

#[derive(Debug, Clone)]
pub(super) enum ParamArgBinding {
    /// 该形参由默认值补齐（调用点未提供实参）。
    Default,
    /// 单个实参绑定到该形参。
    Single(usize),
    /// `vararg` 形参：绑定到 0..N 个实参（按调用点顺序）。
    Vararg(Vec<usize>),
}

pub(super) fn call_arg_element_binding(
    call_args: &[CallArgInfo<'_>],
    arg_idx: usize,
) -> Option<ast::CallArgElementBinding> {
    Some(ast::CallArgElementBinding {
        arg_index: arg_idx,
        spread: call_args.get(arg_idx)?.is_spread,
    })
}

pub(super) fn call_arg_binding_from_mapping(
    mapping: &[ParamArgBinding],
    call_args: &[CallArgInfo<'_>],
) -> Option<ast::CallArgBinding> {
    let mut params = Vec::with_capacity(mapping.len());
    for binding in mapping {
        let param = match binding {
            ParamArgBinding::Default => ast::CallArgParamBinding::Default,
            ParamArgBinding::Single(arg_idx) => {
                ast::CallArgParamBinding::Explicit(call_arg_element_binding(call_args, *arg_idx)?)
            }
            ParamArgBinding::Vararg(arg_idxs) => ast::CallArgParamBinding::Vararg(
                arg_idxs
                    .iter()
                    .copied()
                    .map(|arg_idx| call_arg_element_binding(call_args, arg_idx))
                    .collect::<Option<Vec<_>>>()?,
            ),
        };
        params.push(param);
    }
    Some(ast::CallArgBinding { params })
}

pub(super) fn call_arg_binding_from_mapping_with_receiver(
    mapping: &[ParamArgBinding],
    call_args_with_receiver: &[CallArgInfo<'_>],
) -> Option<ast::CallArgBinding> {
    let mut params = Vec::with_capacity(mapping.len());
    for binding in mapping {
        let param = match binding {
            ParamArgBinding::Default => ast::CallArgParamBinding::Default,
            ParamArgBinding::Single(0) => ast::CallArgParamBinding::Receiver,
            ParamArgBinding::Single(arg_idx) => {
                let source_arg_idx = arg_idx.checked_sub(1)?;
                ast::CallArgParamBinding::Explicit(ast::CallArgElementBinding {
                    arg_index: source_arg_idx,
                    spread: call_args_with_receiver.get(*arg_idx)?.is_spread,
                })
            }
            ParamArgBinding::Vararg(arg_idxs) => ast::CallArgParamBinding::Vararg(
                arg_idxs
                    .iter()
                    .copied()
                    .map(|arg_idx| {
                        Some(ast::CallArgElementBinding {
                            arg_index: arg_idx.checked_sub(1)?,
                            spread: call_args_with_receiver.get(arg_idx)?.is_spread,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?,
            ),
        };
        params.push(param);
    }
    Some(ast::CallArgBinding { params })
}

pub(super) fn call_arg_binding_from_mapping_with_receiver_prefix(
    mapping: &[ParamArgBinding],
    call_args: &[CallArgInfo<'_>],
) -> Option<ast::CallArgBinding> {
    let mut params = Vec::with_capacity(mapping.len() + 1);
    params.push(ast::CallArgParamBinding::Receiver);
    params.extend(call_arg_binding_from_mapping(mapping, call_args)?.params);
    Some(ast::CallArgBinding { params })
}

pub(super) fn record_receiver_prefixed_extension_call_binding(
    lower: &mut TypeLowering<'_>,
    call_span: Span,
    member_span: Span,
    callee_fqn: &str,
    mapping: &[Option<usize>],
    call_args: &[CallArgInfo<'_>],
) {
    lower.record_typechecked_member_resolution(
        member_span,
        ast::ResolvedMemberRef::ExtensionFun {
            fqn: callee_fqn.to_string(),
        },
    );
    let mapping = mapping
        .iter()
        .copied()
        .map(|arg_idx| arg_idx.map_or(ParamArgBinding::Default, ParamArgBinding::Single))
        .collect::<Vec<_>>();
    if let Some(binding) = call_arg_binding_from_mapping_with_receiver_prefix(&mapping, call_args) {
        lower.record_typechecked_call_arg_binding(call_span, binding);
    }
}

pub(super) fn call_arg_binding_from_optional_mapping(
    mapping: &[Option<usize>],
    call_args: &[CallArgInfo<'_>],
) -> Option<ast::CallArgBinding> {
    let params = mapping
        .iter()
        .copied()
        .map(|arg_idx| match arg_idx {
            Some(arg_idx) => {
                call_arg_element_binding(call_args, arg_idx).map(ast::CallArgParamBinding::Explicit)
            }
            None => Some(ast::CallArgParamBinding::Default),
        })
        .collect::<Option<Vec<_>>>()?;
    Some(ast::CallArgBinding { params })
}

pub(super) fn record_call_arg_binding_from_optional_mapping(
    lower: &mut TypeLowering<'_>,
    call_span: Span,
    mapping: &[Option<usize>],
    call_args: &[CallArgInfo<'_>],
) {
    if let Some(binding) = call_arg_binding_from_optional_mapping(mapping, call_args) {
        lower.record_typechecked_call_arg_binding(call_span, binding);
    }
}

pub(super) fn vararg_param_index(param_is_vararg: &[bool]) -> Option<usize> {
    let mut found: Option<usize> = None;
    for (idx, is_vararg) in param_is_vararg.iter().copied().enumerate() {
        if !is_vararg {
            continue;
        }
        if found.is_some() {
            // 当前阶段：只支持一个 vararg；多于一个视为"无法映射"。
            return None;
        }
        found = Some(idx);
    }
    found
}

pub(super) fn expand_param_arg_pairs(mapping: &[ParamArgBinding]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (param_idx, binding) in mapping.iter().enumerate() {
        match binding {
            ParamArgBinding::Default => {}
            ParamArgBinding::Single(arg_idx) => out.push((param_idx, *arg_idx)),
            ParamArgBinding::Vararg(arg_idxs) => {
                out.extend(arg_idxs.iter().copied().map(|arg_idx| (param_idx, arg_idx)));
            }
        }
    }
    out
}

pub(super) fn is_unit_type(ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    matches!(lower.type_kind(ty), TypeKind::Value(ValueTypeKind::Unit))
}

pub(super) fn can_use_zero_arg_unit_call_sugar(
    args: &[ast::Expr],
    param_tys: &[TypeId],
    param_has_defaults: &[bool],
    param_is_vararg: &[bool],
    lower: &TypeLowering<'_>,
) -> bool {
    args.is_empty()
        && param_tys.len() == 1
        && param_has_defaults.len() == 1
        && param_is_vararg.len() == 1
        && !param_has_defaults[0]
        && !param_is_vararg[0]
        && is_unit_type(param_tys[0], lower)
}

pub(super) fn user_visible_param_slices_after_receiver<'a>(
    param_tys: &'a [TypeId],
    param_has_defaults: &'a [bool],
    param_is_vararg: &'a [bool],
) -> Option<(&'a [TypeId], &'a [bool], &'a [bool])> {
    Some((
        param_tys.get(1..)?,
        param_has_defaults.get(1..)?,
        param_is_vararg.get(1..)?,
    ))
}

pub(super) fn synthesize_unit_arg_expr(call_span: Span) -> ast::Expr {
    ast::Expr {
        span: call_span,
        kind: ast::ExprKind::UnitLit,
    }
}

pub(super) fn required_param_count(param_has_defaults: &[bool], param_is_vararg: &[bool]) -> Option<usize> {
    if param_has_defaults.len() != param_is_vararg.len() {
        return None;
    }

    let mut required = 0usize;
    for (has_default, is_vararg) in param_has_defaults
        .iter()
        .copied()
        .zip(param_is_vararg.iter().copied())
    {
        if is_vararg {
            // Kotlin-like：vararg 可接受 0 个参数，因此不计入 required。
            continue;
        }
        if !has_default {
            required += 1;
        }
    }
    Some(required)
}

pub(super) fn map_call_args_to_params_with_defaults_and_varargs(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
    param_has_defaults: &[bool],
    param_is_vararg: &[bool],
) -> Option<Vec<ParamArgBinding>> {
    // 先尝试严格规则（保持既有行为）。
    if let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs_strict(
        call_args,
        param_names,
        param_has_defaults,
        param_is_vararg,
    ) {
        return Some(mapping);
    }

    // fallback：trailing lambdas（末尾连续 N 个 lambda 实参）：
    // - 对 vararg（必须为最后形参）则视为追加 N 个 vararg 元素；
    // - 否则绑定到最后 N 个形参槽位，并允许跳过中间默认参数。
    let k = trailing_lambda_suffix_len(call_args);
    if k == 0 {
        return None;
    }

    let vararg_idx = vararg_param_index(param_is_vararg);

    // 1) vararg：把 trailing lambdas 追加到 vararg 槽位。
    if let Some(v_idx) = vararg_idx {
        let prefix_args = call_args.get(..call_args.len().saturating_sub(k))?;
        let mut mapping = map_call_args_to_params_with_defaults_and_varargs_strict(
            prefix_args,
            param_names,
            param_has_defaults,
            param_is_vararg,
        )?;

        let slot = mapping.get_mut(v_idx)?;
        let ParamArgBinding::Vararg(arg_idxs) = slot else {
            return None;
        };
        for arg_idx in (call_args.len().saturating_sub(k))..call_args.len() {
            arg_idxs.push(arg_idx);
        }
        return Some(mapping);
    }

    // 2) no-vararg：把 trailing lambdas 绑定到最后 N 个形参。
    if param_names.len() < k {
        return None;
    }
    let prefix_args = call_args.get(..call_args.len().saturating_sub(k))?;
    let prefix_param_names = param_names.get(..param_names.len().saturating_sub(k))?;
    let prefix_param_has_defaults =
        param_has_defaults.get(..param_has_defaults.len().saturating_sub(k))?;
    let prefix_param_is_vararg = param_is_vararg.get(..param_is_vararg.len().saturating_sub(k))?;

    let mut mapping = map_call_args_to_params_with_defaults_and_varargs_strict(
        prefix_args,
        prefix_param_names,
        prefix_param_has_defaults,
        prefix_param_is_vararg,
    )?;
    for arg_idx in (call_args.len().saturating_sub(k))..call_args.len() {
        mapping.push(ParamArgBinding::Single(arg_idx));
    }
    Some(mapping)
}

pub(super) fn map_call_args_to_params_with_defaults_and_varargs_strict(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
    param_has_defaults: &[bool],
    param_is_vararg: &[bool],
) -> Option<Vec<ParamArgBinding>> {
    if param_names.len() != param_has_defaults.len() || param_names.len() != param_is_vararg.len() {
        return None;
    }

    let vararg_idx = vararg_param_index(param_is_vararg);
    if let Some(vararg_idx) = vararg_idx {
        // 当前阶段最小规则：vararg 必须为最后一个形参。
        if vararg_idx + 1 != param_names.len() {
            return None;
        }
    }

    // 无 vararg 时：默认参数允许"少传"，但不能"多传"。
    if vararg_idx.is_none() && call_args.len() > param_names.len() {
        return None;
    }

    let required = required_param_count(param_has_defaults, param_is_vararg)?;
    if call_args.len() < required {
        return None;
    }

    // Kotlin-like：命名实参之后不能再出现位置实参（该规则已由 `check_call_arg_named_rules` 检查）。
    // 这里仅用于找到"位置实参段"的长度。
    let mut positional_count = 0usize;
    for arg in call_args {
        match arg.kind {
            CallArgKind::Positional => positional_count += 1,
            CallArgKind::Named { .. } => break,
        }
    }

    let mut mapping: Vec<ParamArgBinding> = vec![ParamArgBinding::Default; param_names.len()];
    let mut vararg_args: Vec<usize> = Vec::new();

    // 1) 位置实参：按从左到右依次绑定到形参（不跳槽）。
    let mut next_param = 0usize;
    for arg_idx in 0..positional_count {
        let Some(v_idx) = vararg_idx else {
            if next_param >= param_names.len() {
                return None;
            }
            mapping[next_param] = ParamArgBinding::Single(arg_idx);
            next_param += 1;
            continue;
        };

        if next_param < v_idx {
            mapping[next_param] = ParamArgBinding::Single(arg_idx);
            next_param += 1;
            continue;
        }

        // `vararg` 形参：接收剩余全部位置实参。
        vararg_args.push(arg_idx);
    }

    // 2) 命名实参：按 name 匹配形参槽位。
    for (arg_idx, arg) in call_args.iter().enumerate().skip(positional_count) {
        let CallArgKind::Named { name, .. } = &arg.kind else {
            return None;
        };

        let slot_idx = param_names.iter().position(|p| p == name)?;
        if Some(slot_idx) == vararg_idx {
            vararg_args.push(arg_idx);
            continue;
        }

        match mapping.get(slot_idx) {
            Some(ParamArgBinding::Default) => {}
            _ => return None, // 同一形参不能被重复赋值（位置 + 命名）
        }
        mapping[slot_idx] = ParamArgBinding::Single(arg_idx);
    }

    // 3) 未填充的非-vararg 槽位必须有默认值。
    for (idx, binding) in mapping.iter().enumerate() {
        if Some(idx) == vararg_idx {
            continue;
        }
        if !matches!(binding, ParamArgBinding::Default) {
            continue;
        }
        if !param_has_defaults.get(idx).copied().unwrap_or(false) {
            return None;
        }
    }

    // 4) 写回 vararg 槽位（可为空）。
    if let Some(vararg_idx) = vararg_idx {
        mapping[vararg_idx] = ParamArgBinding::Vararg(vararg_args);
    }

    Some(mapping)
}

pub(super) fn spread_operand_element_types(
    operand_ty: TypeId,
    lower: &TypeLowering<'_>,
) -> Option<Vec<TypeId>> {
    match lower.type_kind(operand_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            if n.fqn == "scoop.core.Array" && n.args.len() == 1 {
                return Some(vec![n.args[0]]);
            }
            None
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => Some(elements.clone()),
        _ => None,
    }
}

/// 为 vararg spread 的"常见集合类型"提供迁移诊断提示（T1325b）。
///
/// 说明：
/// - 语言层的 spread 当前只接受 `Array<T>` 与 tuple（Appendix B.5.5）。
/// - 对 `MutableArray/MutableList/MutableSet/MutableMap` 等集合，应在调用点通过约定 helper
///   显式桥接为 `Array`/只读视图后再 spread（例如 `*xs.toArray()`）。
pub(super) fn vararg_spread_missing_bridge_hint(
    operand_ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> String {
    let (TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n))) =
        lower.type_kind(operand_ty)
    else {
        return String::new();
    };

    // `MutableList<T>` 等为 typealias，lowering 后通常会被展开为 `MutableArray<T>`。
    if n.fqn != "scoop.core.MutableArray" || n.args.len() != 1 {
        return String::new();
    }

    // 当前阶段（std v0）集合桥接以 `Int` 专用落点为主：`toArray()/asSet()/asMapView()`。
    if n.args[0] == builtins.int {
        return "；提示：对常见集合请先显式桥接为 Array/视图再 spread：`MutableArray/MutableList` 可用 `toArray()`（例如 `f(*xs.toArray())`），`MutableSet` 可用 `asSet()`，`MutableMap` 可用 `asMapView()`".to_string();
    }

    // 非 `Int`：当前 std v0 可能尚无现成桥接，仍给出方向性的迁移提示。
    "；提示：对集合做 spread 前请先显式转换为 `Array<...>`（当前 std v0 的桥接多为 `Int` 专用，例如 `toArray()`）".to_string()
}

