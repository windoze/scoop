use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::ast;
use crate::resolve::{ConeId, ConstructorOverload, FunOverload, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::infer::ExpectedTypeFrom;
use super::member::find_member_owner_nominal_instantiation;
use super::ops::{
    collect_member_method_signatures_from_index, is_symbol_visible_from_source,
    literal_absorbs_to_expected, try_extract_member_call_receiver_fqn_and_args,
    try_extract_nominal_fqn_and_args,
};
use super::util::{
    expr_kind_name, fmt_effect_row, fmt_overload_signature, join_overload_signatures,
    short_name_from_fqn,
};

use super::collect::build_fun_where_constraints_from_resolve_sig;
use super::{EffParamSig, ExprInferInputs, ExprTypeError, FUNPTR_FQN, FunSigOwned, PTR_FQN};

use super::super::assignable::is_type_assignable;
use super::super::eff_row_subst::{
    EffRowVarSubstPlan, apply_eff_row_var_subst_plan, build_eff_row_var_subst_plan,
};
use super::super::lower::TypeLowering;
use super::super::type_env::{EnumVariantInfo, TypeSymbol};
use super::super::{TypeLowerError, TypeSymbolKind};

#[derive(Debug, Clone)]
pub(super) enum CallArgKind {
    Positional,
    Named { name: String, name_span: Span },
}

#[derive(Debug, Clone)]
pub(super) struct CallArgInfo<'a> {
    pub(super) kind: CallArgKind,
    pub(super) expr: &'a ast::Expr,
    pub(super) ty: TypeId,
    pub(super) is_spread: bool,
    pub(super) needs_expected_type: bool,
}

#[derive(Clone, Copy)]
struct MemberCallRequest<'a> {
    call_expr: &'a ast::Expr,
    receiver: &'a ast::Expr,
    member: &'a ast::MemberIdent,
    args: &'a [ast::Expr],
    explicit_type_args: Option<&'a [TypeId]>,
    explicit_eff_arg: Option<&'a EffectRow>,
    safe: bool,
}

#[derive(Clone, Copy)]
pub(super) struct EnumVariantCtorTarget<'a> {
    pub(super) enum_fqn: &'a str,
    pub(super) variant_name: &'a str,
    pub(super) callee_span: Span,
}

#[derive(Clone, Copy)]
pub(in super::super) struct EnumTypeSubstContext<'a> {
    pub(in super::super) decl_file: &'a Path,
    pub(in super::super) enum_source: &'a SourceFile,
    pub(in super::super) use_span: Span,
    pub(in super::super) enum_fqn: &'a str,
    pub(in super::super) builtins: BuiltinTypes,
    pub(in super::super) type_param_set: &'a HashSet<String>,
    pub(in super::super) subst: &'a HashMap<String, TypeId>,
}

#[derive(Debug, Clone, Default)]
struct ExplicitTypeApplyArgs {
    type_args: Vec<TypeId>,
    eff_arg: Option<EffectRow>,
}

/// 把 AST 的调用实参列表归一化为"用于重载筛选"的结构，并预先推导每个实参表达式的类型。
///
/// 说明：
/// - `ExprKind::NamedArg { name = value }` 在调用语境内是"语法糖节点"，其类型应以 `value` 为准；
/// - 这里提前推导所有实参类型，保证：
///   - 子表达式的类型错误不会被重载筛选吞掉；
///   - 后续候选过滤只做纯比较，不再递归进入表达式树。
fn infer_call_arg_info_ty(
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

pub(super) fn collect_call_arg_infos<'a>(
    inputs: ExprInferInputs<'_>,
    args: &'a [ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<CallArgInfo<'a>>, ExprTypeError> {
    collect_call_arg_infos_impl(inputs, args, lower, false)
}

pub(super) fn collect_call_arg_infos_allow_expected_type_placeholders<'a>(
    inputs: ExprInferInputs<'_>,
    args: &'a [ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<CallArgInfo<'a>>, ExprTypeError> {
    collect_call_arg_infos_impl(inputs, args, lower, true)
}

fn collect_call_arg_infos_impl<'a>(
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

fn call_args_have_named(call_args: &[CallArgInfo<'_>]) -> bool {
    call_args
        .iter()
        .any(|a| matches!(a.kind, CallArgKind::Named { .. }))
}

fn first_named_arg_span(call_args: &[CallArgInfo<'_>]) -> Option<Span> {
    call_args.iter().find_map(|arg| match arg.kind {
        CallArgKind::Named { name_span, .. } => Some(name_span),
        CallArgKind::Positional => None,
    })
}

fn funptr_invoke_rejects_named_args(
    callee_fqn: &str,
    receiver_ty: TypeId,
    lower: &TypeLowering<'_>,
) -> bool {
    callee_fqn == "scoop.unsafe.invoke" && is_funptr_type(receiver_ty, lower)
}

fn missing_required_param_names_in_named_call(
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
pub(super) fn check_call_arg_named_rules(
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

fn trailing_lambda_suffix_len(call_args: &[CallArgInfo<'_>]) -> usize {
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
fn check_call_named_args_exist_in_any_candidate<'a>(
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
fn map_call_args_to_params(
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
fn map_call_args_to_params_with_defaults_strict(
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

pub(super) fn map_call_args_to_params_with_defaults(
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

fn callable_value_param_names(fun: &crate::ty::FunctionType) -> Vec<String> {
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
enum ParamArgBinding {
    /// 该形参由默认值补齐（调用点未提供实参）。
    Default,
    /// 单个实参绑定到该形参。
    Single(usize),
    /// `vararg` 形参：绑定到 0..N 个实参（按调用点顺序）。
    Vararg(Vec<usize>),
}

fn call_arg_element_binding(
    call_args: &[CallArgInfo<'_>],
    arg_idx: usize,
) -> Option<ast::CallArgElementBinding> {
    Some(ast::CallArgElementBinding {
        arg_index: arg_idx,
        spread: call_args.get(arg_idx)?.is_spread,
    })
}

fn call_arg_binding_from_mapping(
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

fn call_arg_binding_from_mapping_with_receiver(
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

fn call_arg_binding_from_mapping_with_receiver_prefix(
    mapping: &[ParamArgBinding],
    call_args: &[CallArgInfo<'_>],
) -> Option<ast::CallArgBinding> {
    let mut params = Vec::with_capacity(mapping.len() + 1);
    params.push(ast::CallArgParamBinding::Receiver);
    params.extend(call_arg_binding_from_mapping(mapping, call_args)?.params);
    Some(ast::CallArgBinding { params })
}

fn record_receiver_prefixed_extension_call_binding(
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

fn call_arg_binding_from_optional_mapping(
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

fn record_call_arg_binding_from_optional_mapping(
    lower: &mut TypeLowering<'_>,
    call_span: Span,
    mapping: &[Option<usize>],
    call_args: &[CallArgInfo<'_>],
) {
    if let Some(binding) = call_arg_binding_from_optional_mapping(mapping, call_args) {
        lower.record_typechecked_call_arg_binding(call_span, binding);
    }
}

fn vararg_param_index(param_is_vararg: &[bool]) -> Option<usize> {
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

fn expand_param_arg_pairs(mapping: &[ParamArgBinding]) -> Vec<(usize, usize)> {
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

fn is_unit_type(ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    matches!(lower.type_kind(ty), TypeKind::Value(ValueTypeKind::Unit))
}

fn can_use_zero_arg_unit_call_sugar(
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

fn user_visible_param_slices_after_receiver<'a>(
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

fn synthesize_unit_arg_expr(call_span: Span) -> ast::Expr {
    ast::Expr {
        span: call_span,
        kind: ast::ExprKind::UnitLit,
    }
}

fn required_param_count(param_has_defaults: &[bool], param_is_vararg: &[bool]) -> Option<usize> {
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

fn map_call_args_to_params_with_defaults_and_varargs(
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

fn map_call_args_to_params_with_defaults_and_varargs_strict(
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

fn spread_operand_element_types(
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
fn vararg_spread_missing_bridge_hint(
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

fn infer_function_type_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee_name: &str,
    callee_ty: TypeId,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let builtins = inputs.builtins;

    // spec §6.2：`const fun` 禁止闭包/lambda；因此也禁止调用"函数值"（无论其来源是参数还是局部绑定）。
    if lower.in_const_context() {
        return Err(ExprTypeError::ConstFunFunctionValueCallNotAllowed {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    }

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

fn infer_function_value_call_expr_type(
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

fn infer_funptr_value_call_expr_type(
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

fn infer_funptr_type_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee_name: &str,
    callee_ty: TypeId,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let builtins = inputs.builtins;

    if lower.in_const_context() {
        return Err(ExprTypeError::ConstFunFunPtrCallNotAllowed {
            callee: callee_name.to_string(),
            span: call_expr.span.into(),
        });
    }

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

fn is_funptr_type(ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            nominal.fqn == FUNPTR_FQN && nominal.args.len() == 1
        }
        _ => false,
    }
}

fn collect_top_level_fun_signatures_from_index(
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
    // 但当当前编译单元（或其注入的 stdlib）提供了同签名的实现（`has_body = true`）时，
    // 若把两者同时暴露给重载决议，会导致"同签名重复候选 → ambiguous overload"。
    //
    // 因此这里先收集一份"已有实现的签名 key"，并在生成 `FunSigOwned` 时过滤掉同 key 的无 body 声明。
    fn normalize_sig_piece(s: &str) -> String {
        s.split_whitespace().collect()
    }

    fn fun_overload_sig_key(o: &crate::resolve::FunOverload, decl_source: &SourceFile) -> String {
        let mut out = String::new();
        out.push_str(if o.sig.is_const { "const" } else { "fun" });
        out.push('|');
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
            is_const: o.sig.is_const,
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

fn is_top_level_fun_value_candidate_expr(
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

fn default_eff_arg_for_fun_sig(sig: &FunSigOwned) -> EffectRow {
    sig.eff_param
        .as_ref()
        .map(|p| p.default.clone())
        .unwrap_or_else(EffectRow::pure)
}

fn function_type_shape_from_sig_params(
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

fn function_value_type_from_instantiated_sig(
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

fn generic_constraints_from_expected_fun_value(
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

fn extract_top_level_fun_value_target(
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

fn lower_explicit_type_apply_args(
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

pub(super) fn infer_top_level_fun_value_expr_type(
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

pub(super) fn check_unsafe_call_gate(
    callee_fqn: &str,
    sig: &FunSigOwned,
    call_span: Span,
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let native_extern =
        sig.is_extern && !lower.is_extern_scoop_fun_decl(&sig.decl_file, sig.decl_span);
    if native_extern && !lower.in_unsafe_context() {
        return Err(ExprTypeError::ExternCallRequiresUnsafeContext {
            callee: callee_fqn.to_string(),
            span: call_span.into(),
        });
    }
    if sig.is_unsafe && !lower.in_unsafe_context() {
        return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
            callee: callee_fqn.to_string(),
            span: call_span.into(),
        });
    }
    Ok(())
}

fn check_var_param_lvalue_gate(
    callee_fqn: &str,
    sig: &FunSigOwned,
    call_args: &[CallArgInfo<'_>],
    mapping: &[ParamArgBinding],
) -> Result<(), ExprTypeError> {
    fn is_addressable_lvalue(expr: &ast::Expr) -> bool {
        match &expr.kind {
            ast::ExprKind::Ident(id) => id.resolved.is_some(),
            ast::ExprKind::MemberAccess { member, .. } => {
                matches!(member.resolved, Some(ast::ResolvedMemberRef::Value { .. }))
            }
            _ => false,
        }
    }

    for (param_idx, name) in sig.param_names.iter().enumerate() {
        if name != "var" {
            continue;
        }

        let Some(binding) = mapping.get(param_idx) else {
            continue;
        };

        match binding {
            ParamArgBinding::Default => {
                // 该形参由默认值补齐：这里不做额外门禁（由 arity/default rules 负责）。
            }
            ParamArgBinding::Single(arg_idx) => {
                let Some(arg) = call_args.get(*arg_idx) else {
                    continue;
                };
                if is_addressable_lvalue(arg.expr) {
                    continue;
                }
                return Err(ExprTypeError::VarParamRequiresLValue {
                    callee: callee_fqn.to_string(),
                    span: arg.expr.span.into(),
                });
            }
            ParamArgBinding::Vararg(arg_idxs) => {
                for arg_idx in arg_idxs {
                    let Some(arg) = call_args.get(*arg_idx) else {
                        continue;
                    };
                    if is_addressable_lvalue(arg.expr) {
                        continue;
                    }
                    return Err(ExprTypeError::VarParamRequiresLValue {
                        callee: callee_fqn.to_string(),
                        span: arg.expr.span.into(),
                    });
                }
            }
        }
    }

    Ok(())
}

pub(super) fn check_nogc_call_gate(
    callee_fqn: &str,
    sig: &FunSigOwned,
    call_span: Span,
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    if !lower.in_nogc_context() {
        return Ok(());
    }

    // spec §15.8：`@NoGC` 函数体内必须保守拒绝"可能分配"的调用点。
    //
    // 当前阶段（TODO T1005）最小实现：
    // - 仅允许调用显式 `@NoGC` 函数，以及 native `@Extern` leaf；
    // - `abi = "scoop"` 必须继续按 ordinary managed call 对待，不能在 `@NoGC` 上下文放行；
    // - 其它调用一律视为"可能分配/可能触发 GC"，直接报错。
    let native_extern =
        sig.is_extern && !lower.is_extern_scoop_fun_decl(&sig.decl_file, sig.decl_span);
    if sig.is_nogc || native_extern {
        return Ok(());
    }

    Err(ExprTypeError::NoGcCallForbidden {
        callee: callee_fqn.to_string(),
        span: call_span.into(),
    })
}

pub(super) fn check_const_fun_call_gate(
    callee_fqn: &str,
    sig: &FunSigOwned,
    call_span: Span,
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    if !lower.in_const_context() {
        return Ok(());
    }

    // spec §6.2：`const fun` 允许调用：
    // - 其它 `const fun`
    // - 编译器 intrinsics（即便 sysroot 声明未显式标记为 const）
    //
    // 另外，部分 sysroot/stdlib API 虽然在源代码上是普通函数声明，
    // 但 const/comptime 解释器会直接以内建逻辑执行，不会真的进入其函数体。
    // 这些调用点在前端 const gate 上也必须视为“编译器 intrinsic”同类目标，
    // 否则 compilation-unit 级 typecheck 会先把它们误拒绝。
    if sig.is_const || sig.is_intrinsic || is_const_eval_builtin_fun(callee_fqn) {
        return Ok(());
    }

    Err(ExprTypeError::ConstFunCallForbidden {
        callee: callee_fqn.to_string(),
        span: call_span.into(),
    })
}

fn is_const_eval_builtin_fun(callee_fqn: &str) -> bool {
    matches!(
        callee_fqn,
        "scoop.core.substring"
            | "scoop.core.indexOf"
            | "scoop.core.contains"
            | "scoop.core.startsWith"
            | "scoop.core.endsWith"
            | "scoop.core.split"
            | "scoop.core.trimStart"
            | "scoop.core.trimEnd"
            | "scoop.core.trim"
    )
}

fn emit_deprecated_call_warning(
    callee_fqn: &str,
    sig: &FunSigOwned,
    call_span: Span,
    lower: &TypeLowering<'_>,
) {
    lower.emit_deprecated_fun_use(callee_fqn, &sig.decl_file, sig.decl_span, call_span);
}

pub(super) fn check_fn_value_to_any_erasure_gate(
    found: TypeId,
    expected: TypeId,
    at: Span,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    // 仅在"擦除/上转到 Any"的位置生效。
    if expected != builtins.any {
        return Ok(());
    }

    let TypeKind::Ref(RefTypeKind::Function(fun)) = lower.type_kind(found) else {
        return Ok(());
    };

    // spec §7.5：effects 为编译期信息；只有闭合 `Pure!` 的函数类型允许擦除到 `Any`，
    // 否则运行时无法验证该函数值是否真的"只可能是 Pure"。
    if fun.effects.is_pure() && fun.effects_closed {
        return Ok(());
    }

    Err(ExprTypeError::FnValueToAnyRequiresClosedPure {
        found: lower.fmt_type(found),
        span: at.into(),
    })
}

pub(super) fn check_nogc_boxing_gate(
    found: TypeId,
    expected: TypeId,
    at: Span,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    let forbid_alloc = lower.in_nogc_context() || lower.in_const_context();
    if !forbid_alloc {
        return Ok(());
    }

    // `Nothing` 不会在运行时产生值；将其视为"不会发生装箱/分配"。
    if found == builtins.nothing {
        return Ok(());
    }

    // 当前阶段的"已知分配点"之一：值类型（或无法判定是否为值类型的 type param）
    // 被上下文吸收到引用类型（`Any`/interface 等）时，需要 boxing（T0817）。
    let expected_is_ref = matches!(lower.type_kind(expected), TypeKind::Ref(_));
    if !expected_is_ref {
        return Ok(());
    }

    let found_may_need_boxing = matches!(
        lower.type_kind(found),
        TypeKind::Value(_) | TypeKind::Param(_)
    );
    if !found_may_need_boxing {
        return Ok(());
    }

    if lower.in_nogc_context() {
        return Err(ExprTypeError::NoGcBoxingForbidden {
            from: lower.fmt_type(found),
            to: lower.fmt_type(expected),
            span: at.into(),
        });
    }

    Err(ExprTypeError::ConstFunBoxingForbidden {
        from: lower.fmt_type(found),
        to: lower.fmt_type(expected),
        span: at.into(),
    })
}

pub(super) fn infer_call_expr_type(
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
            let Some(resolved) = &id.resolved else {
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
            };

            let (callee_fqn, callee_span) = match resolved {
                ast::ResolvedValueRef::TopLevel { fqn } => (fqn.clone(), id.span),
                ast::ResolvedValueRef::Local { decl_span, .. } => {
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
            };
            let top_level_value_ty = top_level_types.get(&callee_fqn).copied();

            // 当前阶段：优先使用"当前文件内"的函数签名信息（支持 return type 推断等回写），
            // 并在缺失时回退到 `Index`（用于 sysroot / 跨文件顶层函数调用）。
            let sigs_from_index: Vec<FunSigOwned>;
            let sigs: &[FunSigOwned] = match top_level_funs.get(&callee_fqn) {
                Some(s) => s.as_slice(),
                None => {
                    sigs_from_index =
                        collect_top_level_fun_signatures_from_index(&callee_fqn, lower, builtins)?;
                    if sigs_from_index.is_empty() {
                        if explicit_type_args
                            .as_ref()
                            .is_some_and(|type_args| !type_args.is_empty())
                            && top_level_value_ty.is_some()
                        {
                            return Err(ExprTypeError::CalleeNotCallable {
                                callee: callee_fqn,
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
                        if id.call.is_none() && lower.is_object_type(&callee_fqn) {
                            return Err(ExprTypeError::ObjectNotConstructible {
                                name: callee_fqn,
                                span: callee_span.into(),
                            });
                        }
                        return Err(ExprTypeError::CalleeNotCallable {
                            callee: callee_fqn,
                            span: callee_span.into(),
                        });
                    }
                    sigs_from_index.as_slice()
                }
            };

            // 扩展函数不能以 `f(args...)` 的形式被直接调用，因此这里只选择普通顶层函数候选。
            let direct_call_candidates: Vec<&FunSigOwned> =
                sigs.iter().filter(|s| !s.is_extension).collect();
            let Some(sig) = direct_call_candidates.first().copied() else {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_fqn,
                    span: callee_span.into(),
                });
            };

            // 只有一个可用候选：沿用旧的"给出精确 arity/type mismatch 诊断"的路径，
            // 但补齐命名实参的形参映射（T0453）。
            if direct_call_candidates.len() == 1 {
                check_unsafe_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
                check_nogc_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
                check_const_fun_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
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
                check_cross_thread_resume_policy(
                    &callee_fqn,
                    &call_args,
                    &checked_arg_tys,
                    &mapping_pairs,
                    lower,
                )?;
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
            check_call_named_args_exist_in_any_candidate(
                &callee_fqn,
                &call_args,
                direct_call_candidates
                    .iter()
                    .map(|c| c.param_names.as_slice()),
            )?;

            #[derive(Debug, Clone)]
            struct MatchedFunOverload<'a> {
                sig: &'a FunSigOwned,
                instantiated: InstantiatedFunSig,
                eff_arg: EffectRow,
                /// `call_args[arg_idx]` 对应的"期望类型"。
                expected_arg_tys: Vec<TypeId>,
                /// 调用点需要用默认值补齐的形参个数（越少越"具体"）。
                defaults_used: usize,
                /// 形参 -> 实参绑定（用于后续门禁，例如 `addressOf(var: T)`）。
                mapping: Vec<ParamArgBinding>,
                /// 当前候选是否通过 typed `Unit` zero-arg sugar 匹配得到。
                used_unit_sugar: bool,
            }

            fn is_strictly_more_specific_fun_overload(
                a: &MatchedFunOverload<'_>,
                b: &MatchedFunOverload<'_>,
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

            fn pick_most_specific_fun_overload(
                candidates: &[MatchedFunOverload<'_>],
                lower: &TypeLowering<'_>,
                builtins: BuiltinTypes,
            ) -> Option<usize> {
                // 1) Kotlin-like most-specific：候选 A 的每个形参类型都"更具体"（可赋值到 B 的形参类型），
                //    且至少有一个位置严格更具体，则认为 A 严格更具体。
                for (idx, cand) in candidates.iter().enumerate() {
                    let mut ok = true;
                    for (other_idx, other) in candidates.iter().enumerate() {
                        if idx == other_idx {
                            continue;
                        }
                        if !is_strictly_more_specific_fun_overload(cand, other, lower, builtins) {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        return Some(idx);
                    }
                }

                // 2) tie-break：默认参数更少者优先（"非默认参数优先"）。
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

            let mut matched: Vec<MatchedFunOverload<'_>> = Vec::new();
            for cand in direct_call_candidates {
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
                        &callee_fqn,
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
                    &callee_fqn,
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
                    let defaults_used = mapping
                        .iter()
                        .filter(|b| matches!(b, ParamArgBinding::Default))
                        .count();
                    let mut expected_arg_tys =
                        vec![builtins.nothing; call_args_for_candidate.len()];
                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        expected_arg_tys[arg_idx] = instantiated.params[param_idx];
                    }

                    matched.push(MatchedFunOverload {
                        sig: cand,
                        instantiated,
                        eff_arg,
                        expected_arg_tys,
                        defaults_used,
                        mapping,
                        used_unit_sugar,
                    });
                }
            }

            if matched.iter().any(|cand| !cand.used_unit_sugar) {
                matched.retain(|cand| !cand.used_unit_sugar);
            }

            let chosen = match matched.len() {
                0 => {
                    return Err(ExprTypeError::NoMatchingOverload {
                        callee: callee_fqn,
                        span: call_expr.span.into(),
                    });
                }
                1 => matched.pop().expect("len == 1"),
                _ => {
                    let Some(idx) = pick_most_specific_fun_overload(&matched, lower, builtins)
                    else {
                        let name = short_name_from_fqn(&callee_fqn).to_string();
                        let candidates = join_overload_signatures(
                            matched
                                .iter()
                                .map(|c| {
                                    fmt_overload_signature(
                                        &name,
                                        None,
                                        &c.instantiated.params,
                                        lower,
                                    )
                                })
                                .collect(),
                        );
                        return Err(ExprTypeError::AmbiguousOverload {
                            callee: callee_fqn,
                            candidates,
                            span: call_expr.span.into(),
                        });
                    };
                    matched.swap_remove(idx)
                }
            };

            check_unsafe_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;
            check_nogc_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;
            check_const_fun_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;
            emit_deprecated_call_warning(&callee_fqn, chosen.sig, call_expr.span, lower);
            let chosen_call_args = if chosen.used_unit_sugar {
                sugar_call_args
                    .as_ref()
                    .expect("typed Unit sugar 选择的候选应有合成实参")
            } else {
                &call_args
            };
            check_var_param_lvalue_gate(
                &callee_fqn,
                chosen.sig,
                chosen_call_args,
                &chosen.mapping,
            )?;

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
                callee_fqn.clone(),
                &chosen.sig.decl_file,
                chosen.sig.decl_span,
                &chosen.instantiated.type_args,
                &eff_args,
                call_expr.span,
            );
            lower.record_top_level_fun_call_binding(
                call_expr.span,
                ast::TopLevelFunCallBinding {
                    fqn: callee_fqn.clone(),
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

fn infer_unsafe_ptr_primitive_call_expr_type(
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

fn pick_ptr_type_fqn(lower: &TypeLowering<'_>) -> String {
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

fn extract_ptr_pointee(ptr_ty: TypeId, ptr_fqn: &str, lower: &TypeLowering<'_>) -> Option<TypeId> {
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

pub(super) fn is_ctor_visible_from(
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
pub(super) struct MatchedCtorOverload {
    pub(super) owner_fqn: String,
    pub(super) ctor_span: Option<Span>,
    pub(super) arg_mapping: Vec<Option<usize>>,
    /// `call_args[arg_idx]` 对应的"期望类型"。
    pub(super) expected_arg_tys: Vec<TypeId>,
    /// 调用点需要用默认值补齐的形参个数（越少越"具体"）。
    pub(super) defaults_used: usize,
    /// 用于歧义诊断打印的 ctor 签名（稳定排序后展示）。
    pub(super) signature: String,
    /// T0125：从实参类型推断出的泛型 type args（按声明顺序）。
    pub(super) inferred_type_args: Vec<TypeId>,
}

type InstantiatedCtorParamTypes = (Vec<TypeId>, Vec<TypeId>);

struct CtorParamInstantiationRequest<'a> {
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

fn is_strictly_more_specific_ctor_overload(
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

pub(super) fn pick_most_specific_ctor_overload(
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

fn instantiate_ctor_param_tys(
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
pub(super) fn collect_matched_ctor_overloads_for_owner(
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

pub(super) fn select_ctor_overload_for_owner(
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
pub(super) fn try_infer_nominal_constructor_call_expr_type_with_expected(
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

fn infer_nominal_constructor_call_expr_type(
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

    let chosen_kind = ctor_owners
        .iter()
        .find_map(|(owner_fqn, kind)| (owner_fqn == &chosen.owner_fqn).then_some(*kind))
        .expect("chosen ctor owner kind should exist");

    if lower.in_const_context() && matches!(chosen_kind, ast::TypeKind::Class) {
        return Err(ExprTypeError::ConstFunRefTypeConstructionNotAllowed {
            ty: source.slice(callee.span).to_string(),
            span: call_expr.span.into(),
        });
    }

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

fn resolves_to_compiler_owned_continuation_type(
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

fn lookup_enum_variant_decl_data(
    source: &SourceFile,
    lower: &TypeLowering<'_>,
    enum_fqn: &str,
    variant_name: &str,
) -> Option<(Vec<String>, SourceFile, EnumVariantInfo)> {
    let decl = lower.env().enum_decl(enum_fqn)?;
    let type_params = decl.type_params.clone();
    let enum_source = lower
        .env()
        .source(&decl.decl_file)
        .cloned()
        .unwrap_or_else(|| source.clone());
    let variant = decl
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)?
        .clone();
    Some((type_params, enum_source, variant))
}

fn resolved_qualified_enum_variant_value_fqn(
    source: &SourceFile,
    member: &ast::MemberIdent,
    lower: &TypeLowering<'_>,
) -> Option<(String, String)> {
    let ast::ResolvedMemberRef::Value { fqn } = member.resolved.as_ref()? else {
        return None;
    };
    let (enum_fqn, variant_name) = fqn.rsplit_once('.')?;
    lookup_enum_variant_decl_data(source, lower, enum_fqn, variant_name)?;
    Some((enum_fqn.to_string(), variant_name.to_string()))
}

fn infer_specific_enum_variant_ctor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    target: EnumVariantCtorTarget<'_>,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let EnumVariantCtorTarget {
        enum_fqn,
        variant_name,
        callee_span,
    } = target;
    let Some((type_params, enum_source, variant)) =
        lookup_enum_variant_decl_data(source, lower, enum_fqn, variant_name)
    else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "enum variant ctor（缺少 enum 声明信息）",
            span: call_expr.span.into(),
        });
    };

    let variant_fqn = format!("{enum_fqn}.{variant_name}");
    let expected = variant.fields.len();
    let found = args.len();
    if expected != found {
        return Err(ExprTypeError::EnumVariantCtorArityMismatch {
            variant: variant_fqn,
            expected,
            found,
            span: call_expr.span.into(),
        });
    }

    let mut arg_types: Vec<TypeId> = Vec::with_capacity(args.len());
    for arg in args {
        arg_types.push(inputs.infer(lower, arg)?);
    }

    let type_param_set: HashSet<String> = type_params.iter().cloned().collect();
    let mut subst: HashMap<String, TypeId> = HashMap::new();
    for (idx, (field, found_ty)) in variant
        .fields
        .iter()
        .zip(arg_types.iter().copied())
        .enumerate()
    {
        let ast::TypeRef::Path(p) = &field.ty else {
            continue;
        };
        if !p.args.is_empty() || p.segments.len() != 1 {
            continue;
        }
        let name = enum_source.slice(p.segments[0].span);
        if !type_param_set.contains(name) {
            continue;
        }

        match subst.get(name).copied() {
            None => {
                subst.insert(name.to_string(), found_ty);
            }
            Some(prev) if prev == found_ty => {}
            Some(prev) if prev == builtins.nothing => {
                subst.insert(name.to_string(), found_ty);
            }
            Some(_prev) if found_ty == builtins.nothing => {}
            Some(prev) => {
                return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                    variant: format!("{enum_fqn}.{variant_name}"),
                    index: idx + 1,
                    expected: lower.fmt_type(prev),
                    found: lower.fmt_type(found_ty),
                    span: args[idx].span.into(),
                });
            }
        }
    }

    for (idx, (field, found_ty)) in variant
        .fields
        .iter()
        .zip(arg_types.iter().copied())
        .enumerate()
    {
        let expected_ty = lower_type_ref_with_enum_subst(
            EnumTypeSubstContext {
                decl_file: enum_source.path(),
                enum_source: &enum_source,
                use_span: call_expr.span,
                enum_fqn,
                builtins,
                type_param_set: &type_param_set,
                subst: &subst,
            },
            &field.ty,
            lower,
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
            return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                variant: format!("{enum_fqn}.{variant_name}"),
                index: idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: args[idx].span.into(),
            });
        }
    }

    let mut enum_args: Vec<TypeId> = Vec::with_capacity(type_params.len());
    for name in &type_params {
        let Some(id) = subst.get(name).copied() else {
            return Err(ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                enum_fqn: enum_fqn.to_string(),
                param: name.clone(),
                span: callee_span.into(),
            });
        };
        enum_args.push(id);
    }

    Ok(lower.lower_type_fqn_with_args(enum_fqn.to_string(), enum_args, call_expr.span)?)
}

fn infer_enum_variant_ctor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee: &ast::ValueIdent,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let source = inputs.source;
    let variant_name = source.slice(callee.span);
    let candidates = lower
        .env()
        .find_visible_enum_variants_named(variant_name, source);

    if candidates.is_empty() {
        return Ok(None);
    }
    if candidates.len() != 1 {
        let mut names: Vec<String> = candidates
            .iter()
            .map(|(enum_fqn, _)| format!("{enum_fqn}.{variant_name}"))
            .collect();
        names.sort();
        names.dedup();

        return Err(ExprTypeError::AmbiguousEnumVariantCtor {
            name: variant_name.to_string(),
            candidates: names.join(" | "),
            span: callee.span.into(),
        });
    }

    let (enum_fqn, variant) = candidates.into_iter().next().expect("len == 1");
    Ok(Some(infer_specific_enum_variant_ctor_call_expr_type(
        inputs,
        call_expr,
        EnumVariantCtorTarget {
            enum_fqn: &enum_fqn,
            variant_name: &variant.name,
            callee_span: callee.span,
        },
        args,
        lower,
    )?))
}

pub(super) fn try_infer_qualified_enum_variant_ctor_call_expr_type(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let Some((enum_fqn, variant_name)) =
        resolved_qualified_enum_variant_value_fqn(inputs.source, member, lower)
    else {
        return Ok(None);
    };

    Ok(Some(infer_specific_enum_variant_ctor_call_expr_type(
        inputs,
        call_expr,
        EnumVariantCtorTarget {
            enum_fqn: &enum_fqn,
            variant_name: &variant_name,
            callee_span: member.span,
        },
        args,
        lower,
    )?))
}

pub(super) fn infer_specific_enum_variant_ctor_call_expr_type_by_expected(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    target: EnumVariantCtorTarget<'_>,
    args: &[ast::Expr],
    expected_enum_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let EnumVariantCtorTarget {
        enum_fqn,
        variant_name,
        callee_span,
    } = target;
    let Some((type_params, enum_source, variant)) =
        lookup_enum_variant_decl_data(source, lower, enum_fqn, variant_name)
    else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "enum variant ctor（缺少 enum 声明信息）",
            span: call_expr.span.into(),
        });
    };

    if type_params.len() != expected_enum_args.len() {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "enum variant ctor（expected enum type args 数量异常）",
            span: call_expr.span.into(),
        });
    }

    let variant_fqn = format!("{enum_fqn}.{variant_name}");
    let expected_arity = variant.fields.len();
    let found_arity = args.len();
    if expected_arity != found_arity {
        return Err(ExprTypeError::EnumVariantCtorArityMismatch {
            variant: variant_fqn,
            expected: expected_arity,
            found: found_arity,
            span: call_expr.span.into(),
        });
    }

    let type_param_set: HashSet<String> = type_params.iter().cloned().collect();
    let subst: HashMap<String, TypeId> = type_params
        .iter()
        .cloned()
        .zip(expected_enum_args.iter().copied())
        .collect();

    for (idx, (field, arg_expr)) in variant.fields.iter().zip(args.iter()).enumerate() {
        let expected_field_ty = lower_type_ref_with_enum_subst(
            EnumTypeSubstContext {
                decl_file: enum_source.path(),
                enum_source: &enum_source,
                use_span: call_expr.span,
                enum_fqn,
                builtins,
                type_param_set: &type_param_set,
                subst: &subst,
            },
            &field.ty,
            lower,
        )?;

        let found_ty = inputs.infer_in_expected(
            lower,
            arg_expr,
            expected_field_ty,
            ExpectedTypeFrom::new(format!(
                "enum variant `{enum_fqn}.{variant_name}` 第 {} 个参数",
                idx + 1
            )),
        )?;

        if !is_type_assignable(found_ty, expected_field_ty, lower, builtins)
            && !literal_absorbs_to_expected(arg_expr, expected_field_ty, source, lower, builtins)
        {
            return Err(ExprTypeError::EnumVariantCtorArgTypeMismatch {
                variant: format!("{enum_fqn}.{variant_name}"),
                index: idx + 1,
                expected: lower.fmt_type(expected_field_ty),
                found: lower.fmt_type(found_ty),
                span: arg_expr.span.into(),
            });
        }
    }

    let mut enum_args: Vec<TypeId> = Vec::with_capacity(type_params.len());
    for name in &type_params {
        let Some(id) = subst.get(name).copied() else {
            return Err(ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                enum_fqn: enum_fqn.to_string(),
                param: name.clone(),
                span: callee_span.into(),
            });
        };
        enum_args.push(id);
    }

    Ok(lower.lower_type_fqn_with_args(enum_fqn.to_string(), enum_args, call_expr.span)?)
}

pub(in super::super) fn lower_type_ref_with_enum_subst(
    ctx: EnumTypeSubstContext<'_>,
    ty: &ast::TypeRef,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    lower.with_decl_file_context(ctx.decl_file, |lower| match ty {
        ast::TypeRef::Path(p) => {
            // 单段名且无 type args：可能是对 enum type param 的引用（例如 `T`）。
            if p.segments.len() == 1 && p.args.is_empty() {
                let name = ctx.enum_source.slice(p.segments[0].span);
                if ctx.type_param_set.contains(name) {
                    return ctx.subst.get(name).copied().ok_or_else(|| {
                        ExprTypeError::EnumVariantCtorTypeArgNotInferred {
                            enum_fqn: ctx.enum_fqn.to_string(),
                            param: name.to_string(),
                            span: ctx.use_span.into(),
                        }
                    });
                }
            }

            let segments: Vec<String> = p
                .segments
                .iter()
                .map(|id| ctx.enum_source.slice(id.span).to_string())
                .collect();

            let fqn = match lower.resolve_type_path_fqn_by_name(&segments, ctx.use_span) {
                Ok(fqn) => fqn,
                Err(TypeLowerError::UnresolvedType { name, span }) => {
                    let Some(builtin_fqn) = implicit_builtin_type_fqn(&name) else {
                        return Err(TypeLowerError::UnresolvedType { name, span }.into());
                    };
                    builtin_fqn.to_string()
                }
                Err(other) => return Err(other.into()),
            };

            let mut eff_arg: Option<EffectRow> = None;
            let mut args: Vec<TypeId> = Vec::with_capacity(p.args.len());
            for a in &p.args {
                match a {
                    ast::TypeRef::EffectRowArg { row, .. } => {
                        if eff_arg.is_none() {
                            eff_arg = Some(lower.lower_effect_row_expr(Some(row))?);
                        }
                    }
                    _ => args.push(lower_type_ref_with_enum_subst(ctx, a, lower)?),
                }
            }

            Ok(lower.lower_type_fqn_with_args_and_eff(fqn, args, eff_arg, ctx.use_span)?)
        }
        ast::TypeRef::Tuple(t) => {
            if t.elements.is_empty() {
                return Ok(ctx.builtins.unit);
            }
            let mut elements: Vec<TypeId> = Vec::with_capacity(t.elements.len());
            for e in &t.elements {
                elements.push(lower_type_ref_with_enum_subst(ctx, e, lower)?);
            }
            Ok(lower.ty_tuple(elements))
        }
        ast::TypeRef::Nullable { inner, .. } => {
            let inner = lower_type_ref_with_enum_subst(ctx, inner, lower)?;
            Ok(lower.ty_option(inner))
        }
        ast::TypeRef::Star { .. } => Ok(lower.ty_star_projection()),
        ast::TypeRef::EffectRowArg { .. } => Err(TypeLowerError::UnsupportedTypeRef {
            kind: "use-site effect row arg (`eff ...`)",
            span: ctx.use_span.into(),
        }
        .into()),
        ast::TypeRef::Function(f) => {
            let receiver = match &f.receiver {
                Some(r) => Some(lower_type_ref_with_enum_subst(ctx, r, lower)?),
                None => None,
            };

            let mut params = Vec::with_capacity(f.params.len());
            for p in &f.params {
                params.push(lower_type_ref_with_enum_subst(ctx, p, lower)?);
            }

            let return_ty = lower_type_ref_with_enum_subst(ctx, &f.return_ty, lower)?;

            let effects = match &f.effects {
                None => EffectRow::pure(),
                Some(e) if e.terms.is_empty() => EffectRow::pure(),
                Some(e) => {
                    let mut terms: Vec<TypeId> = Vec::with_capacity(e.terms.len());
                    for term in &e.terms {
                        let term_ref = ast::TypeRef::Path(term.clone());
                        let ty = lower_type_ref_with_enum_subst(ctx, &term_ref, lower)?;

                        let ok = match lower.type_kind(ty) {
                            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => matches!(
                                lower.nominal_decl_kind(&nominal.fqn),
                                Some(ast::TypeKind::Effect)
                            ),
                            _ => false,
                        };
                        if !ok {
                            return Err(TypeLowerError::EffectRowItemNotEffect {
                                item: ctx.enum_source.slice(term.span).to_string(),
                                found: lower.fmt_type(ty),
                                span: term.span.into(),
                            }
                            .into());
                        }

                        terms.push(ty);
                    }
                    EffectRow::new(terms)
                }
            };

            let effects_closed = f.effects.as_ref().is_some_and(|r| r.closed);
            Ok(lower.ty_function(receiver, params, return_ty, effects, effects_closed))
        }
    })
}

fn implicit_builtin_type_fqn(local_or_fqn: &str) -> Option<&'static str> {
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
        "Option" | "scoop.core.Option" => Some("scoop.core.Option"),
        _ => None,
    }
}

fn late_resolve_direct_member_fun_fqn_from_receiver_ty(
    inputs: ExprInferInputs<'_>,
    receiver_ty: TypeId,
    member_name: &str,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<String>, ExprTypeError> {
    let Some((receiver_fqn, receiver_args)) = try_extract_nominal_fqn_and_args(receiver_ty, lower)
    else {
        return Ok(None);
    };

    let direct_fqn = format!("{receiver_fqn}.{member_name}");
    let sigs = collect_member_method_signatures_from_index(
        inputs.source,
        receiver_ty,
        &receiver_fqn,
        &receiver_args,
        &direct_fqn,
        lower,
        inputs.builtins,
    )?;

    if sigs.is_empty() {
        Ok(None)
    } else {
        Ok(Some(direct_fqn))
    }
}

/// P4-T01l：在 builtin scalar receiver 的 by-name short-circuit 短路内补登记
/// "对应 sysroot bodied helper" 的 typed call-site contract，让 HIR / MIR /
/// late lowering 即使在 short-circuit 的 fast path 上也能找到 `<TypeFqn>.<member>`
/// 的 direct-call binding；否则 HIR stage 的 `call expression missing typed
/// call-site contract` 会立刻拦截 `42.toString()` / `true.toString()` /
/// `'a'.toString()` / `<float>.toString()` / `"x".toString()` 这类调用。
///
/// 仅注册"成员引用 + 顶层 fun call binding + receiver-only arg binding"，
/// 既保留 by-name codegen path 作为过渡 surface，也不会重写 vtable / itable layout；
/// 删除 by-name 路径的动作仍由 P4-T01 完成。
fn register_builtin_scalar_member_call_binding(
    inputs: ExprInferInputs<'_>,
    member_span: Span,
    actual_receiver_ty: TypeId,
    body_fqn: &str,
    call_span: Span,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    if try_extract_member_call_receiver_fqn_and_args(actual_receiver_ty, lower).is_none() {
        return Ok(());
    }

    // 必须在 sysroot by_fqn 中已经存在 bodied helper（P4-T01 sysroot rewrite 之后
    // 应当成立）；若不在，留给现有 by-name 路径继续承担，不强制注册 contract。
    let sigs = collect_member_method_signatures_from_index(
        inputs.source,
        actual_receiver_ty,
        body_fqn
            .rsplit_once('.')
            .map(|(owner, _)| owner)
            .unwrap_or(body_fqn),
        &[],
        body_fqn,
        lower,
        inputs.builtins,
    )?;
    if sigs.is_empty() {
        return Ok(());
    }

    lower.record_typechecked_member_resolution(
        member_span,
        ast::ResolvedMemberRef::Fun {
            fqn: body_fqn.to_string(),
        },
    );
    let chosen_sig = sigs[0].clone();
    lower.record_top_level_fun_call_binding(
        call_span,
        ast::TopLevelFunCallBinding {
            fqn: body_fqn.to_string(),
            decl_file: chosen_sig.decl_file.clone(),
            decl_span: chosen_sig.decl_span,
            is_intrinsic: chosen_sig.is_intrinsic,
            intrinsic_entry_name: chosen_sig.intrinsic_entry_name.clone(),
            type_args: Vec::new(),
            eff_args: Vec::new(),
        },
    );
    let arg_binding = ast::CallArgBinding {
        params: vec![ast::CallArgParamBinding::Receiver],
    };
    lower.record_typechecked_call_arg_binding(call_span, arg_binding);
    Ok(())
}

pub(super) fn combined_member_instance_type_args(
    callee_fqn: &str,
    receiver_ty: TypeId,
    fun_type_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
) -> Result<Vec<TypeId>, ExprTypeError> {
    let mut type_args = find_member_owner_nominal_instantiation(receiver_ty, callee_fqn, lower)?
        .map(|(_, owner_args)| owner_args)
        .unwrap_or_default();
    type_args.extend(fun_type_args.iter().copied());
    Ok(type_args)
}

#[derive(Debug, Clone)]
pub(super) struct LoweredEffectOpSig {
    pub(super) sig: FunSigOwned,
    pub(super) op_type_params: Vec<TypeId>,
    pub(super) effect_type_params: Vec<TypeId>,
}

pub(super) fn lower_effect_op_signature(
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
        is_const: false,
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

pub(super) fn infer_effect_op_call_expr_type(
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

fn try_infer_continuation_resume_call_expr_type(
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

pub(super) fn infer_continuation_resume_call_expr_type(
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

fn infer_member_call_expr_type(
    inputs: ExprInferInputs<'_>,
    request: MemberCallRequest<'_>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let MemberCallRequest {
        call_expr,
        receiver,
        member,
        args,
        explicit_type_args,
        explicit_eff_arg,
        safe,
    } = request;
    let source = inputs.source;
    let builtins = inputs.builtins;
    let locals = inputs.locals;
    let top_level_types = inputs.top_level_types;
    let top_level_funs = inputs.top_level_funs;
    let struct_field_types = inputs.struct_field_types;

    // 先递归类型检查 receiver：保证 `a?.b()` 中的 `a` 自身也会被覆盖。
    //
    // 例外：`TypeName.member(...)` 的 companion dispatch 中，receiver 是一个类型名而不是值表达式；
    // resolver 会刻意保留该 ident 为未解析状态，而实际运行期 receiver 应是 companion object 单例值。
    // 这里直接把 receiver 视为 companion object 的名义类型，避免把 `TypeName` 当普通值去推导。
    let companion_receiver_owner_fqn = if let ast::ExprKind::Ident(id) = &receiver.kind
        && id.resolved.is_none()
        && source.slice(id.span) != "this"
    {
        match member.resolved.as_ref() {
            Some(ast::ResolvedMemberRef::Fun { fqn })
            | Some(ast::ResolvedMemberRef::Value { fqn }) => {
                if let Some((owner_fqn, _)) = fqn.rsplit_once('.') {
                    lower
                        .is_object_type(owner_fqn)
                        .then(|| owner_fqn.to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    };
    let receiver_ty = if let Some(owner_fqn) = companion_receiver_owner_fqn {
        lower.lower_type_fqn_with_args(owner_fqn, Vec::new(), receiver.span)?
    } else {
        inputs.infer(lower, receiver)?
    };

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

    if let Some(ret) = try_infer_continuation_resume_call_expr_type(
        inputs,
        call_expr,
        actual_receiver_ty,
        member,
        args,
        safe,
        lower,
    )? {
        return Ok(ret);
    }

    // Built-in String API (early stage).
    //
    // T1811: String P0 methods (length/substring/startsWith/endsWith/indexOf/contains/split).
    let member_name = source.slice(member.span);
    if actual_receiver_ty == builtins.string {
        // T1817: String.hash() — 0 args, returns Int.
        if member_name == "hash" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: "hash".into(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            return Ok(builtins.int);
        }
        if member_name == "length" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: member_name.to_string(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            return Ok(builtins.int);
        }
        // T1816/T0115: `String.concat/compareTo` 没有普通 sysroot 函数体，
        // 但 production HIR/MIR/codegen 需要 authoritative direct-call contract，
        // 因此这里显式发布一个 extension-style member resolution + receiver-prefixed arg binding。
        if matches!(member_name, "concat" | "compareTo") {
            let callee_fqn = format!("scoop.core.{member_name}");
            let return_ty = if member_name == "concat" {
                builtins.string
            } else {
                builtins.int
            };
            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(&callee_fqn, &call_args)?;

            let param_names = vec!["other".to_string()];
            check_call_named_args_exist_in_any_candidate(
                &callee_fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;

            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: member_name.into(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_fqn.clone(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: member_name.into(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };
            let other_ty = call_args[arg_idx].ty;
            if !is_type_assignable(other_ty, builtins.string, lower, builtins) {
                return Err(ExprTypeError::CallArgTypeMismatch {
                    callee: callee_fqn.clone(),
                    index: 1,
                    expected: lower.fmt_type(builtins.string),
                    found: lower.fmt_type(other_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            lower.record_typechecked_member_resolution(
                member.span,
                ast::ResolvedMemberRef::ExtensionFun {
                    fqn: callee_fqn.clone(),
                },
            );
            let mapping = mapping
                .into_iter()
                .map(|arg_idx| arg_idx.map_or(ParamArgBinding::Default, ParamArgBinding::Single))
                .collect::<Vec<_>>();
            if let Some(binding) =
                call_arg_binding_from_mapping_with_receiver_prefix(&mapping, &call_args)
            {
                lower.record_typechecked_call_arg_binding(call_expr.span, binding);
            }
            return Ok(return_ty);
        }

        // T0122: substring/indexOf/contains/startsWith/endsWith/split/trim/trimStart/trimEnd
        // 已迁移到 sysroot/string.scoop 的纯 Scoop 扩展函数，由 extension fun 路径处理。

        // T1812: String.toInt() — 文本→数值转换。
        if member_name == "toInt" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: "toInt".into(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            return Ok(builtins.int);
        }
        // T0115/T0120/T0121: 这些 String builtin surface 没有普通 sysroot 函数体，
        // 但 production HIR/MIR/codegen 仍必须消费稳定的 extension-style direct-call contract，
        // 不能退化成 unresolved `MemberAccess` + `FunValue` callee。
        if matches!(
            member_name,
            "trimIndent"
                | "isEmpty"
                | "replace"
                | "charAt"
                | "repeat"
                | "byteLength"
                | "getByte"
                | "unsafeSliceBytes"
        ) {
            let callee_fqn = format!("scoop.core.{member_name}");
            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(&callee_fqn, &call_args)?;
            let (param_names, param_tys, return_ty, requires_unsafe) = match member_name {
                "trimIndent" => (Vec::new(), Vec::new(), builtins.string, false),
                "isEmpty" => (Vec::new(), Vec::new(), builtins.bool_, false),
                "replace" => (
                    vec!["old".to_string(), "new".to_string()],
                    vec![builtins.string, builtins.string],
                    builtins.string,
                    false,
                ),
                "charAt" => (
                    vec!["index".to_string()],
                    vec![builtins.int],
                    builtins.int,
                    false,
                ),
                "repeat" => (
                    vec!["n".to_string()],
                    vec![builtins.int],
                    builtins.string,
                    false,
                ),
                "byteLength" => (Vec::new(), Vec::new(), builtins.int, false),
                "getByte" => (
                    vec!["index".to_string()],
                    vec![builtins.int],
                    builtins.int,
                    false,
                ),
                "unsafeSliceBytes" => (
                    vec!["byteOffset".to_string(), "byteLength".to_string()],
                    vec![builtins.int, builtins.int],
                    builtins.string,
                    true,
                ),
                _ => unreachable!("filtered by matches!"),
            };
            if requires_unsafe && !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: format!("String.{member_name}"),
                    span: call_expr.span.into(),
                });
            }
            check_call_named_args_exist_in_any_candidate(
                &callee_fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            if call_args.len() != param_names.len() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: member_name.into(),
                    expected: param_names.len(),
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }
            let param_has_defaults = vec![false; param_names.len()];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_fqn.clone(),
                    span: call_expr.span.into(),
                });
            };
            for (param_idx, expected_ty) in param_tys.iter().copied().enumerate() {
                let Some(arg_idx) = mapping.get(param_idx).copied().flatten() else {
                    return Err(ExprTypeError::CallArityMismatch {
                        callee: member_name.into(),
                        expected: param_names.len(),
                        found: call_args.len(),
                        span: call_expr.span.into(),
                    });
                };
                let arg = &call_args[arg_idx];
                if !is_type_assignable(arg.ty, expected_ty, lower, builtins)
                    && !literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins)
                {
                    return Err(ExprTypeError::CallArgTypeMismatch {
                        callee: callee_fqn.clone(),
                        index: param_idx + 1,
                        expected: lower.fmt_type(expected_ty),
                        found: lower.fmt_type(arg.ty),
                        span: arg.expr.span.into(),
                    });
                }
            }
            record_receiver_prefixed_extension_call_binding(
                lower,
                call_expr.span,
                member.span,
                &callee_fqn,
                &mapping,
                &call_args,
            );
            return Ok(return_ty);
        }
    }

    // T1812: Int.toString() — 数值→文本転換。
    //
    // P4-T01l：保留 by-name short-circuit 作为过渡（删除留给 P4-T01），但同时把
    // body method 的 typed call-site contract 写回 HIR side table，让 HIR / MIR / late lowering
    // 即使在 builtin scalar receiver 下也能找到 `<TypeFqn>.<member>` 的 direct call binding。
    if actual_receiver_ty == builtins.int {
        if member_name == "toString" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: "toString".into(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            register_builtin_scalar_member_call_binding(
                inputs,
                member.span,
                actual_receiver_ty,
                "scoop.core.Int.toString",
                call_expr.span,
                lower,
            )?;
            return Ok(builtins.string);
        }
        // T1817: Int.hash() — SplitMix64 bit-mixing, returns Int.
        if member_name == "hash" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: "hash".into(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            return Ok(builtins.int);
        }
    }

    // T0114: Bool.toString() — 布尔値→文本転換。
    if actual_receiver_ty == builtins.bool_ && member_name == "toString" {
        if !args.is_empty() {
            return Err(ExprTypeError::CallArityMismatch {
                callee: "toString".into(),
                expected: 0,
                found: args.len(),
                span: call_expr.span.into(),
            });
        }
        register_builtin_scalar_member_call_binding(
            inputs,
            member.span,
            actual_receiver_ty,
            "scoop.core.Bool.toString",
            call_expr.span,
            lower,
        )?;
        return Ok(builtins.string);
    }

    // T0146c2: Char 内建 API —— toInt()/toString()/hash().
    if actual_receiver_ty == builtins.char_ {
        if member_name == "toInt" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: "toInt".into(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            return Ok(builtins.int);
        }
        if member_name == "toString" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: "toString".into(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            register_builtin_scalar_member_call_binding(
                inputs,
                member.span,
                actual_receiver_ty,
                "scoop.core.Char.toString",
                call_expr.span,
                lower,
            )?;
            return Ok(builtins.string);
        }
        if member_name == "hash" {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: "hash".into(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }
            return Ok(builtins.int);
        }
    }

    // T0147c: Float 内建 API —— toInt()/toString()/hash()/abs()/isNaN()/isInfinite().
    if actual_receiver_ty == builtins.float64 || actual_receiver_ty == builtins.float32 {
        let is_known_float_method = member_name == "toInt"
            || member_name == "toString"
            || member_name == "hash"
            || member_name == "abs"
            || member_name == "isNaN"
            || member_name == "isInfinite";
        if is_known_float_method {
            if !args.is_empty() {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: member_name.to_string(),
                    expected: 0,
                    found: args.len(),
                    span: call_expr.span.into(),
                });
            }

            if member_name == "toString" {
                let body_fqn = if actual_receiver_ty == builtins.float64 {
                    "scoop.core.Float64.toString"
                } else {
                    "scoop.core.Float32.toString"
                };
                register_builtin_scalar_member_call_binding(
                    inputs,
                    member.span,
                    actual_receiver_ty,
                    body_fqn,
                    call_expr.span,
                    lower,
                )?;
            }
            return Ok(match member_name {
                "toInt" | "hash" => builtins.int,
                "toString" => builtins.string,
                "abs" => actual_receiver_ty,
                "isNaN" | "isInfinite" => builtins.bool_,
                _ => unreachable!("filtered by is_known_float_method"),
            });
        }
    }

    let current_lambda_this = inputs.is_current_lambda_this_expr(receiver);
    let late_direct_member_fun_fqn = if current_lambda_this || member.resolved.is_none() {
        late_resolve_direct_member_fun_fqn_from_receiver_ty(
            inputs,
            actual_receiver_ty,
            member_name,
            lower,
        )?
    } else {
        None
    };
    let resolved_member_fun_fqn = late_direct_member_fun_fqn.as_deref().or({
        if current_lambda_this {
            None
        } else {
            match member.resolved.as_ref() {
                Some(ast::ResolvedMemberRef::Fun { fqn }) => Some(fqn.as_str()),
                _ => None,
            }
        }
    });
    if let Some(fqn) = resolved_member_fun_fqn {
        lower.record_typechecked_member_resolution(
            member.span,
            ast::ResolvedMemberRef::Fun {
                fqn: fqn.to_string(),
            },
        );
    }

    // spec §15.10 / §15.10.1：GC pin/handle intrinsic surface。
    //
    // 说明：
    // - `GC.pin/unpin` 与 `GC.handleNew/Get/Drop` 是 sysroot 固定的 intrinsic member-call surface；
    //   它们的 authoritative contract 由前端 gate、MIR transport metadata 与 runtime lowering 共同定义。
    // - 这里保留专门分支，是为了在 ordinary member-call desugaring 之前锁定支持面与诊断，避免把它们
    //   误降成普通成员调用后再由后端兜底。
    // - `pin/handleNew` 只接受可追踪引用对象；`unpin`/`handleGet`/`handleDrop` 只接受对应 token 类型。
    if let Some(fqn) = resolved_member_fun_fqn {
        // `handleNew` 可能分配，因此在 `@NoGC` 上下文中必须拒绝；其余入口沿 sysroot `@NoGC`/
        // `@Unsafe` contract 执行。
        if fqn == "scoop.core.GC.handleNew" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }
            if lower.in_nogc_context() {
                return Err(ExprTypeError::NoGcCallForbidden {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["obj".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let obj_ty = call_args[arg_idx].ty;
            if !matches!(lower.type_kind(obj_ty), TypeKind::Ref(_)) {
                return Err(ExprTypeError::GcHandleNewRequiresRefType {
                    found: lower.fmt_type(obj_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            let handle_ty = lower.lower_type_fqn_with_args(
                "scoop.core.GcHandle".to_string(),
                Vec::new(),
                call_expr.span,
            )?;
            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(handle_ty);
        }

        if fqn == "scoop.core.GC.handleGet" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["h".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let handle_ty = call_args[arg_idx].ty;
            let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(handle_ty)
            else {
                return Err(ExprTypeError::GcHandleGetRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            };
            if nominal.fqn != "scoop.core.GcHandle" || !nominal.args.is_empty() {
                return Err(ExprTypeError::GcHandleGetRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(builtins.any);
        }

        if fqn == "scoop.core.GC.handleDrop" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["h".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let handle_ty = call_args[arg_idx].ty;
            let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(handle_ty)
            else {
                return Err(ExprTypeError::GcHandleDropRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            };
            if nominal.fqn != "scoop.core.GcHandle" || !nominal.args.is_empty() {
                return Err(ExprTypeError::GcHandleDropRequiresGcHandle {
                    found: lower.fmt_type(handle_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(builtins.unit);
        }

        if fqn == "scoop.core.GC.pin" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["obj".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let obj_ty = call_args[arg_idx].ty;
            if !matches!(lower.type_kind(obj_ty), TypeKind::Ref(_)) {
                return Err(ExprTypeError::GcPinRequiresRefType {
                    found: lower.fmt_type(obj_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            let pinned_ty = lower.lower_type_fqn_with_args(
                "scoop.core.Pinned".to_string(),
                Vec::new(),
                call_expr.span,
            )?;
            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(pinned_ty);
        }

        if fqn == "scoop.core.GC.unpin" {
            if !lower.in_unsafe_context() {
                return Err(ExprTypeError::UnsafeCallRequiresUnsafeContext {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            }

            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            check_call_arg_named_rules(fqn, &call_args)?;
            if call_args.len() != 1 {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            }

            let param_names = vec!["pinned".to_string()];
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                std::iter::once(param_names.as_slice()),
            )?;
            let param_has_defaults = vec![false];
            let Some(mapping) = map_call_args_to_params_with_defaults(
                &call_args,
                &param_names,
                &param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: fqn.to_string(),
                    span: call_expr.span.into(),
                });
            };
            let Some(arg_idx) = mapping.first().copied().flatten() else {
                return Err(ExprTypeError::CallArityMismatch {
                    callee: fqn.to_string(),
                    expected: 1,
                    found: call_args.len(),
                    span: call_expr.span.into(),
                });
            };

            let pinned_ty = call_args[arg_idx].ty;
            let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(pinned_ty)
            else {
                return Err(ExprTypeError::GcUnpinRequiresRefType {
                    found: lower.fmt_type(pinned_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            };
            if nominal.fqn != "scoop.core.Pinned" || !nominal.args.is_empty() {
                return Err(ExprTypeError::GcUnpinRequiresRefType {
                    found: lower.fmt_type(pinned_ty),
                    span: call_args[arg_idx].expr.span.into(),
                });
            }

            record_call_arg_binding_from_optional_mapping(
                lower,
                call_expr.span,
                &mapping,
                &call_args,
            );
            return Ok(builtins.unit);
        }
    }

    // T1508a：直连成员函数调用（final/private）。
    //
    // 说明：
    // - resolver 在 member access 阶段只做"存在性 + FQN 写回"，不会为 member fun call 收集 overload set；
    // - 这里把 `receiver.method(args...)` 降到"对 FQN overload set 的普通调用"来做重载决议，
    //   并把 `receiver` 作为隐式第 0 个参数参与类型检查；
    // - 当前入口统一覆盖 direct call / class vtable / interface itable 三类成员调用形态；
    //   具体走哪条后端路径由 receiver 类型与 slot 解析结果决定。
    if let Some(fqn) = resolved_member_fun_fqn {
        // 注意：`GC.pin/unpin` / `GC.handle*` 走的是专门的 GC intrinsic contract；这里不要把它们当作普通 member call。
        if fqn != "scoop.core.GC.pin"
            && fqn != "scoop.core.GC.unpin"
            && fqn != "scoop.core.GC.handleNew"
            && fqn != "scoop.core.GC.handleGet"
            && fqn != "scoop.core.GC.handleDrop"
            // T0130 修复：当 receiver 为 TypeKind::Param 时，跳过直连成员调用路径，
            // 让后续 where-bound 驱动的方法分发来处理（否则 try_extract_nominal_fqn_and_args
            // 会因 Param 非 nominal 而返回 CalleeNotCallable）。
            && !matches!(lower.type_kind(actual_receiver_ty), TypeKind::Param(_))
        {
            let Some((receiver_fqn, receiver_args)) =
                try_extract_nominal_fqn_and_args(actual_receiver_ty, lower)
            else {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: fqn.to_string(),
                    span: member.span.into(),
                });
            };

            let sigs = collect_member_method_signatures_from_index(
                source,
                actual_receiver_ty,
                &receiver_fqn,
                &receiver_args,
                fqn,
                lower,
                builtins,
            )?;
            if sigs.is_empty() {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: fqn.to_string(),
                    span: member.span.into(),
                });
            }

            // 预先推导所有"显式实参"的类型（不含 receiver），并归一化 named arg 的语法糖节点，
            // 以便在重载筛选中复用这份结果并避免把子表达式错误吞掉。
            let call_args = collect_call_arg_infos(inputs, args, lower)?;
            let synthesized_unit_args = args
                .is_empty()
                .then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
            let sugar_call_args = if let Some(synthesized_args) = synthesized_unit_args.as_ref() {
                Some(collect_call_arg_infos(inputs, synthesized_args, lower)?)
            } else {
                None
            };
            check_call_arg_named_rules(fqn, &call_args)?;
            check_call_named_args_exist_in_any_candidate(
                fqn,
                &call_args,
                sigs.iter().filter_map(|s| s.param_names.get(1..)),
            )?;

            let receiver_arg = CallArgInfo {
                kind: CallArgKind::Positional,
                expr: receiver,
                ty: actual_receiver_ty,
                is_spread: false,
                needs_expected_type: false,
            };

            let mut call_args_with_receiver = Vec::with_capacity(call_args.len() + 1);
            call_args_with_receiver.push(receiver_arg.clone());
            call_args_with_receiver.extend(call_args.iter().cloned());

            #[derive(Debug, Clone)]
            struct MatchedMemberOverload<'a> {
                sig: &'a FunSigOwned,
                instantiated: InstantiatedFunSig,
                eff_arg: EffectRow,
                /// `call_args_with_receiver[arg_idx]` 对应的"期望类型"。
                expected_arg_tys: Vec<TypeId>,
                /// 调用点需要用默认值补齐的形参个数（越少越"具体"）。
                defaults_used: usize,
                /// 形参 -> 实参绑定（用于后续门禁，例如 `addressOf(var: T)`）。
                mapping: Vec<ParamArgBinding>,
                /// 当前候选是否通过 typed `Unit` zero-arg sugar 匹配得到。
                used_unit_sugar: bool,
            }

            fn is_strictly_more_specific_member_overload(
                a: &MatchedMemberOverload<'_>,
                b: &MatchedMemberOverload<'_>,
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

            fn pick_most_specific_member_overload(
                candidates: &[MatchedMemberOverload<'_>],
                lower: &TypeLowering<'_>,
                builtins: BuiltinTypes,
            ) -> Option<usize> {
                // 1) Kotlin-like most-specific：候选 A 的每个形参类型都"更具体"（可赋值到 B 的形参类型），
                //    且至少有一个位置严格更具体，则认为 A 严格更具体。
                for (idx, cand) in candidates.iter().enumerate() {
                    let mut ok = true;
                    for (other_idx, other) in candidates.iter().enumerate() {
                        if idx == other_idx {
                            continue;
                        }
                        if !is_strictly_more_specific_member_overload(cand, other, lower, builtins)
                        {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        return Some(idx);
                    }
                }

                // 2) tie-break：默认参数更少者优先（"非默认参数优先"）。
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

            let mut matched: Vec<MatchedMemberOverload<'_>> = Vec::new();
            for cand in sigs.iter() {
                let Some((user_param_tys, param_has_defaults, param_is_vararg)) =
                    user_visible_param_slices_after_receiver(
                        &cand.params,
                        &cand.param_has_defaults,
                        &cand.param_is_vararg,
                    )
                else {
                    continue;
                };
                let exact_mapping = map_call_args_to_params_with_defaults_and_varargs(
                    &call_args_with_receiver,
                    &cand.param_names,
                    &cand.param_has_defaults,
                    &cand.param_is_vararg,
                );
                let (call_args_for_candidate, mapping, used_unit_sugar) =
                    if let Some(mapping) = exact_mapping {
                        (call_args_with_receiver.clone(), mapping, false)
                    } else if can_use_zero_arg_unit_call_sugar(
                        args,
                        user_param_tys,
                        param_has_defaults,
                        param_is_vararg,
                        lower,
                    ) {
                        let Some(sugar_call_args) = sugar_call_args.as_ref() else {
                            continue;
                        };
                        let mut sugar_call_args_with_receiver =
                            Vec::with_capacity(sugar_call_args.len() + 1);
                        sugar_call_args_with_receiver.push(receiver_arg.clone());
                        sugar_call_args_with_receiver.extend(sugar_call_args.iter().cloned());
                        let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                            &sugar_call_args_with_receiver,
                            &cand.param_names,
                            &cand.param_has_defaults,
                            &cand.param_is_vararg,
                        ) else {
                            continue;
                        };
                        (sugar_call_args_with_receiver, mapping, true)
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
                        fqn,
                        call_expr.span,
                        cand,
                        explicit_type_args,
                        generic_constraints,
                        lower,
                        builtins,
                    ) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                // T0129：检查 where 约束；不满足则跳过该候选。
                if check_fun_where_constraints_after_instantiation(
                    fqn,
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
                            fqn,
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

                let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.cloned() {
                    explicit_eff_arg
                } else if let Some(eff_param) = &cand.eff_param {
                    let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

                    for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                        let arg = &call_args_for_candidate[arg_idx];
                        if arg.is_spread {
                            continue;
                        }

                        if let Some(base) = cand
                            .param_nominal_eff_eff_base
                            .get(param_idx)
                            .and_then(|b| b.as_ref())
                        {
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
                if !ok {
                    continue;
                }

                let defaults_used = mapping
                    .iter()
                    .filter(|b| matches!(b, ParamArgBinding::Default))
                    .count();
                let mut expected_arg_tys = vec![builtins.nothing; call_args_for_candidate.len()];
                for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                    expected_arg_tys[arg_idx] = instantiated.params[param_idx];
                }

                matched.push(MatchedMemberOverload {
                    sig: cand,
                    instantiated,
                    eff_arg,
                    expected_arg_tys,
                    defaults_used,
                    mapping,
                    used_unit_sugar,
                });
            }

            if matched.iter().any(|cand| !cand.used_unit_sugar) {
                matched.retain(|cand| !cand.used_unit_sugar);
            }

            let chosen = match matched.len() {
                0 => {
                    return Err(ExprTypeError::NoMatchingOverload {
                        callee: fqn.to_string(),
                        span: call_expr.span.into(),
                    });
                }
                1 => matched.pop().expect("len == 1"),
                _ => {
                    let Some(idx) = pick_most_specific_member_overload(&matched, lower, builtins)
                    else {
                        let name = short_name_from_fqn(fqn).to_string();
                        let candidates = join_overload_signatures(
                            matched
                                .iter()
                                .map(|c| {
                                    fmt_overload_signature(
                                        &name,
                                        None,
                                        &c.instantiated.params,
                                        lower,
                                    )
                                })
                                .collect(),
                        );
                        return Err(ExprTypeError::AmbiguousOverload {
                            callee: fqn.to_string(),
                            candidates,
                            span: call_expr.span.into(),
                        });
                    };
                    matched.swap_remove(idx)
                }
            };

            check_unsafe_call_gate(fqn, chosen.sig, call_expr.span, lower)?;
            check_nogc_call_gate(fqn, chosen.sig, call_expr.span, lower)?;
            check_const_fun_call_gate(fqn, chosen.sig, call_expr.span, lower)?;
            emit_deprecated_call_warning(fqn, chosen.sig, call_expr.span, lower);
            let chosen_call_args = if chosen.used_unit_sugar {
                let sugar_call_args = sugar_call_args
                    .as_ref()
                    .expect("typed Unit sugar 选择的成员调用应有合成实参");
                let mut chosen_call_args = Vec::with_capacity(sugar_call_args.len() + 1);
                chosen_call_args.push(receiver_arg.clone());
                chosen_call_args.extend(sugar_call_args.iter().cloned());
                chosen_call_args
            } else {
                call_args_with_receiver.clone()
            };
            check_var_param_lvalue_gate(fqn, chosen.sig, &chosen_call_args, &chosen.mapping)?;

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

            // T0712/T5000e2b：记录带 receiver 的 direct-call 实例请求。
            // 对 generic owner member/getter，这里需要把 owner-specialization 的 concrete args
            // 放在函数自身 type args 之前，形成可复用的实例身份。
            let eff_args = chosen
                .sig
                .eff_param
                .as_ref()
                .map(|_| vec![chosen.eff_arg.clone()])
                .unwrap_or_default();
            let type_args = combined_member_instance_type_args(
                fqn,
                actual_receiver_ty,
                &chosen.instantiated.type_args,
                lower,
            )?;
            lower.record_monomorph_call(
                fqn.to_string(),
                &chosen.sig.decl_file,
                chosen.sig.decl_span,
                &type_args,
                &eff_args,
                call_expr.span,
            );
            lower.record_top_level_fun_call_binding(
                call_expr.span,
                ast::TopLevelFunCallBinding {
                    fqn: fqn.to_string(),
                    decl_file: chosen.sig.decl_file.clone(),
                    decl_span: chosen.sig.decl_span,
                    is_intrinsic: chosen.sig.is_intrinsic,
                    intrinsic_entry_name: chosen.sig.intrinsic_entry_name.clone(),
                    type_args,
                    eff_args,
                },
            );
            if let Some(binding) =
                call_arg_binding_from_mapping_with_receiver(&chosen.mapping, &chosen_call_args)
            {
                lower.record_typechecked_call_arg_binding(call_expr.span, binding);
            }
            if chosen.used_unit_sugar {
                lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
            }

            let ret = if safe {
                lower.ty_option(chosen.instantiated.return_ty)
            } else {
                chosen.instantiated.return_ty
            };

            return Ok(ret);
        }
    }

    // T0130：bound 驱动的方法分发——当 receiver 为 TypeKind::Param 时，
    // 通过 where 约束查找 bound 接口的方法集合。
    if let TypeKind::Param(p) = lower.type_kind(actual_receiver_ty) {
        let param_name = p.name.clone();

        if let Some(ret) = try_infer_where_bound_method_call(
            source,
            call_expr,
            receiver,
            actual_receiver_ty,
            &param_name,
            member,
            args,
            explicit_type_args,
            safe,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )? {
            return Ok(ret);
        }
    }

    if !matches!(
        member.resolved.as_ref(),
        Some(ast::ResolvedMemberRef::Fun { .. } | ast::ResolvedMemberRef::ExtensionFun { .. })
    ) {
        let value_resolved = super::member::resolve_member_value_target_for_receiver(
            inputs,
            receiver,
            Some(actual_receiver_ty),
            member,
            lower,
        );
        if matches!(
            value_resolved.as_ref(),
            Some(
                ast::ResolvedMemberRef::Value { .. }
                    | ast::ResolvedMemberRef::ExtensionValue { .. }
            )
        ) {
            if let Some(resolved) = value_resolved.as_ref() {
                lower.record_typechecked_member_resolution(member.span, resolved.clone());
            }

            let callee_ty = super::member::infer_member_access_ty_from_known_receiver(
                inputs,
                Some(actual_receiver_ty),
                member,
                value_resolved.as_ref(),
                lower,
            )?;
            lower.record_inferred_expr_ty(
                Span::new(receiver.span.start, member.span.end),
                callee_ty,
            );

            if is_funptr_type(callee_ty, lower) {
                return infer_funptr_type_call_expr_type(
                    inputs,
                    call_expr,
                    member_name,
                    callee_ty,
                    args,
                    lower,
                );
            }

            if matches!(
                lower.type_kind(callee_ty),
                TypeKind::Ref(RefTypeKind::Function(_))
            ) {
                return infer_function_type_call_expr_type(
                    inputs,
                    call_expr,
                    member_name,
                    callee_ty,
                    args,
                    lower,
                );
            }

            let callee = match value_resolved.as_ref() {
                Some(ast::ResolvedMemberRef::Value { fqn })
                | Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => fqn.clone(),
                _ => member_name.to_string(),
            };
            return Err(ExprTypeError::CalleeNotCallable {
                callee,
                span: member.span.into(),
            });
        }
    }

    // 当前阶段只支持"扩展函数调用"（T0312）：`receiver.member(args...)`。
    // - 若 resolver 已写回 `ExtensionFun`，优先使用；
    // - 否则（例如 `receiver` 为 `T?` 时 resolver 无法静态确定 receiver 类型），
    //   尝试在"当前包"内按同名顶层 fun 查找 receiver fun。
    let callee_fqn = match if current_lambda_this {
        None
    } else {
        member.resolved.as_ref()
    } {
        Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => fqn.clone(),
        Some(ast::ResolvedMemberRef::Fun { fqn })
        | Some(ast::ResolvedMemberRef::Value { fqn })
        | Some(ast::ResolvedMemberRef::ExtensionValue { fqn }) => {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: fqn.clone(),
                span: member.span.into(),
            });
        }
        None => {
            // resolver 无法静态确定 receiver 类型时（例如 `Shared.t1Go.recv()` 这类非裸 ident receiver），
            // `member.resolved` 可能为空；此时在 typecheck 阶段用"已推导出的 receiver 类型 + import 表"
            // 再做一次 extension fun 查找（与 resolver 的 extension fallback 规则保持一致）。

            // T1317f2：`List/MutableList` 等为 typealias（resolver 侧按名义 FQN 匹配，这里做同样归一化）。
            fn normalize_collections_alias(fqn: &str) -> &str {
                match fqn {
                    "scoop.core.List" => "scoop.core.Array",
                    "scoop.core.MutableList" => "scoop.core.MutableArray",
                    "scoop.collections.Set" => "scoop.core.Array",
                    "scoop.collections.MapView" => "scoop.core.Array",
                    "scoop.collections.MutableSet" => "scoop.core.MutableArray",
                    "scoop.collections.MutableMap" => "scoop.core.MutableArray",
                    _ => fqn,
                }
            }

            let name = source.slice(member.span);
            let use_cone = lower.index().cone_of_source(source);

            let receiver_ty_fqn = match lower.type_kind(actual_receiver_ty) {
                TypeKind::Ref(RefTypeKind::Nominal(n))
                | TypeKind::Value(ValueTypeKind::Nominal(n)) => Some(n.fqn),
                _ => None,
            };
            let receiver_ty_fqn_norm = receiver_ty_fqn.as_deref().map(normalize_collections_alias);

            let mut candidates: Vec<String> = Vec::new();

            let imports = lower.imports();

            // 1) 同包（同 cone）隐式可见。
            for ext in &lower.index().extension_funs {
                if ext.decl_cone != use_cone {
                    continue;
                }
                if ext.pkg_prefix != lower.pkg_prefix() {
                    continue;
                }
                if ext.name != name {
                    continue;
                }

                let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                    Some(ext_receiver) => {
                        ext_receiver == "scoop.core.Any"
                            || receiver_ty_fqn_norm
                                .is_some_and(|r| normalize_collections_alias(ext_receiver) == r)
                    }
                    None => ext.receiver_is_type_param,
                };
                if !receiver_matches {
                    continue;
                }

                let Some(syms) = lower.index().by_fqn.get(&ext.fqn) else {
                    continue;
                };
                if syms
                    .fun
                    .iter()
                    .any(|o| is_symbol_visible_from_source(use_cone, source, &o.symbol))
                {
                    candidates.push(ext.fqn.clone());
                }
            }

            // 2) star import：`import pkg.*`。
            for prefix in &imports.star {
                for ext in &lower.index().extension_funs {
                    if ext.pkg_prefix != *prefix {
                        continue;
                    }
                    if ext.name != name {
                        continue;
                    }

                    let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                        Some(ext_receiver) => {
                            ext_receiver == "scoop.core.Any"
                                || receiver_ty_fqn_norm
                                    .is_some_and(|r| normalize_collections_alias(ext_receiver) == r)
                        }
                        None => ext.receiver_is_type_param,
                    };
                    if !receiver_matches {
                        continue;
                    }

                    let Some(syms) = lower.index().by_fqn.get(&ext.fqn) else {
                        continue;
                    };
                    if syms
                        .fun
                        .iter()
                        .any(|o| is_symbol_visible_from_source(use_cone, source, &o.symbol))
                    {
                        candidates.push(ext.fqn.clone());
                    }
                }
            }

            // 3) 显式 import（含 alias）：通过 local 名字 → fqn 查找 extension。
            if let Some(imported) = imports.value.explicit.get(name) {
                for imported_fqn in imported {
                    for ext in lower
                        .index()
                        .extension_funs
                        .iter()
                        .filter(|e| e.fqn == *imported_fqn)
                    {
                        let receiver_matches = match ext.receiver_ty_fqn.as_deref() {
                            Some(ext_receiver) => {
                                ext_receiver == "scoop.core.Any"
                                    || receiver_ty_fqn_norm.is_some_and(|r| {
                                        normalize_collections_alias(ext_receiver) == r
                                    })
                            }
                            None => ext.receiver_is_type_param,
                        };
                        if !receiver_matches {
                            continue;
                        }

                        let Some(syms) = lower.index().by_fqn.get(&ext.fqn) else {
                            continue;
                        };
                        if syms
                            .fun
                            .iter()
                            .any(|o| is_symbol_visible_from_source(use_cone, source, &o.symbol))
                        {
                            candidates.push(ext.fqn.clone());
                        }
                    }
                }
            }

            candidates.sort();
            candidates.dedup();

            match candidates.len() {
                0 => {
                    if lower.pkg_prefix().is_empty() {
                        name.to_string()
                    } else {
                        format!("{}.{}", lower.pkg_prefix(), name)
                    }
                }
                1 => candidates.pop().expect("len == 1"),
                _ => {
                    return Err(ExprTypeError::AmbiguousCall {
                        callee: name.to_string(),
                        span: member.span.into(),
                    });
                }
            }
        }
    };
    lower.record_typechecked_member_resolution(
        member.span,
        ast::ResolvedMemberRef::ExtensionFun {
            fqn: callee_fqn.clone(),
        },
    );

    // 当前阶段：优先使用"当前文件内"的函数签名信息；缺失时回退到 `Index`
    //（用于 sysroot / 跨文件扩展函数调用）。
    let sigs_from_index: Vec<FunSigOwned>;
    let sigs: &[FunSigOwned] = match top_level_funs.get(&callee_fqn) {
        Some(s) => s.as_slice(),
        None => {
            sigs_from_index =
                collect_top_level_fun_signatures_from_index(&callee_fqn, lower, builtins)?;
            if sigs_from_index.is_empty() {
                return Err(ExprTypeError::CalleeNotCallable {
                    callee: callee_fqn,
                    span: member.span.into(),
                });
            }
            sigs_from_index.as_slice()
        }
    };

    // 只选择扩展函数候选（同名顶层普通函数不参与 `receiver.member()`）。
    let ext_candidates: Vec<&FunSigOwned> = sigs.iter().filter(|s| s.is_extension).collect();
    let Some(sig) = ext_candidates.first().copied() else {
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    // 预先推导所有"显式实参"的类型（不含 receiver），并归一化 named arg 的语法糖节点，
    // 以便在重载筛选中复用这份结果并避免把子表达式错误吞掉。
    let call_args = collect_call_arg_infos(inputs, args, lower)?;
    let synthesized_unit_args = args
        .is_empty()
        .then(|| vec![synthesize_unit_arg_expr(call_expr.span)]);
    let sugar_call_args = if let Some(synthesized_args) = synthesized_unit_args.as_ref() {
        Some(collect_call_arg_infos(inputs, synthesized_args, lower)?)
    } else {
        None
    };
    if funptr_invoke_rejects_named_args(&callee_fqn, actual_receiver_ty, lower)
        && let Some(span) = first_named_arg_span(&call_args)
    {
        return Err(ExprTypeError::NamedArgsNotSupportedForCallableType {
            callee: member_name.to_string(),
            span: span.into(),
        });
    }
    check_call_arg_named_rules(&callee_fqn, &call_args)?;
    check_call_named_args_exist_in_any_candidate(
        &callee_fqn,
        &call_args,
        ext_candidates.iter().filter_map(|c| c.param_names.get(1..)),
    )?;

    let Some(expected_receiver_ty) = sig.params.first().copied() else {
        // 健壮性：扩展函数至少应该包含 receiver 这一参数。
        return Err(ExprTypeError::CalleeNotCallable {
            callee: callee_fqn,
            span: member.span.into(),
        });
    };

    // 只有一个扩展候选：沿用旧的"给出精确 mismatch 诊断"的路径，但补齐命名实参映射（T0453）。
    if ext_candidates.len() == 1 {
        check_unsafe_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
        check_nogc_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
        check_const_fun_call_gate(&callee_fqn, sig, call_expr.span, lower)?;
        emit_deprecated_call_warning(&callee_fqn, sig, call_expr.span, lower);
        let expected_args = sig.params.len().saturating_sub(1);

        let Some(param_names) = sig.param_names.get(1..) else {
            // 健壮性：扩展函数至少应该包含 receiver 的占位形参名。
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };
        let Some(param_has_defaults) = sig.param_has_defaults.get(1..) else {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };
        let Some(param_is_vararg) = sig.param_is_vararg.get(1..) else {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };

        let Some((user_param_tys, _, _)) = user_visible_param_slices_after_receiver(
            &sig.params,
            &sig.param_has_defaults,
            &sig.param_is_vararg,
        ) else {
            return Err(ExprTypeError::CalleeNotCallable {
                callee: callee_fqn,
                span: member.span.into(),
            });
        };
        let used_unit_sugar = can_use_zero_arg_unit_call_sugar(
            args,
            user_param_tys,
            param_has_defaults,
            param_is_vararg,
            lower,
        );
        let effective_call_args = if used_unit_sugar {
            sugar_call_args
                .as_ref()
                .expect("typed Unit sugar 选择的扩展调用应有合成实参")
        } else {
            &call_args
        };

        let has_vararg = vararg_param_index(param_is_vararg).is_some();
        if !has_vararg && effective_call_args.len() > expected_args {
            return Err(ExprTypeError::CallArityMismatch {
                callee: callee_fqn,
                expected: expected_args,
                found: effective_call_args.len(),
                span: call_expr.span.into(),
            });
        }

        let required = if has_vararg {
            required_param_count(param_has_defaults, param_is_vararg)
                .unwrap_or_else(|| param_has_defaults.iter().filter(|d| !**d).count())
        } else {
            param_has_defaults.iter().filter(|d| !**d).count()
        };
        if effective_call_args.len() < required {
            return Err(ExprTypeError::CallArityMismatch {
                callee: callee_fqn,
                expected: required,
                found: effective_call_args.len(),
                span: call_expr.span.into(),
            });
        }

        let mapping: Vec<ParamArgBinding> = if !has_vararg {
            let Some(mapping) = map_call_args_to_params_with_defaults(
                effective_call_args,
                param_names,
                param_has_defaults,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_fqn,
                    span: call_expr.span.into(),
                });
            };
            mapping
                .into_iter()
                .map(|arg_idx| arg_idx.map_or(ParamArgBinding::Default, ParamArgBinding::Single))
                .collect()
        } else {
            let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                effective_call_args,
                param_names,
                param_has_defaults,
                param_is_vararg,
            ) else {
                return Err(ExprTypeError::NoMatchingOverload {
                    callee: callee_fqn,
                    span: call_expr.span.into(),
                });
            };
            mapping
        };

        // spread 实参只能绑定到 vararg 形参。
        for binding in mapping.iter() {
            if let ParamArgBinding::Single(arg_idx) = binding
                && effective_call_args
                    .get(*arg_idx)
                    .is_some_and(|a| a.is_spread)
            {
                return Err(ExprTypeError::SpreadArgRequiresVararg {
                    callee: callee_fqn.clone(),
                    span: effective_call_args[*arg_idx].expr.span.into(),
                });
            }
        }
        let mapping_pairs = expand_param_arg_pairs(&mapping);

        let mut arg_constraints: Vec<GenericArgConstraint> = Vec::new();
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let sig_param_idx = param_idx + 1; // 跳过 receiver
            let arg = &effective_call_args[arg_idx];
            if arg.is_spread {
                if !sig
                    .param_is_vararg
                    .get(sig_param_idx)
                    .copied()
                    .unwrap_or(false)
                {
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
                    arg_constraints.push(GenericArgConstraint {
                        expected: sig.params[sig_param_idx],
                        found: found_elem,
                        found_is_placeholder: false,
                        from: format!("第 {} 个实参（spread）", arg_idx + 1),
                        span: arg.expr.span,
                    });
                }
                continue;
            }

            arg_constraints.push(GenericArgConstraint {
                expected: sig.params[sig_param_idx],
                found: arg.ty,
                found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                from: format!("第 {} 个实参", arg_idx + 1),
                span: arg.expr.span,
            });
        }

        let mut instantiated = instantiate_fun_sig_for_call_with_optional_explicit_type_args(
            &callee_fqn,
            call_expr.span,
            sig,
            explicit_type_args,
            std::iter::once(GenericArgConstraint {
                expected: expected_receiver_ty,
                found: actual_receiver_ty,
                found_is_placeholder: false,
                from: "接收者（receiver）".to_string(),
                span: receiver.span,
            })
            .chain(arg_constraints),
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

        // receiver mismatch 检查：
        // - 默认路径：在推断 `eff` row 参数之前就可以做 receiver 可赋值检查，给出更精确诊断；
        // - 但当 receiver 的期望类型依赖 `E`（例如 `Type<eff (E + IO)>`，或更深的嵌套位置）时，
        //   receiver 的"期望类型"必须等到 `E` 被实例化后才能确定（T0624）。
        let receiver_uses_eff = sig.eff_param.is_some()
            && sig
                .param_eff_row_var_subst
                .first()
                .is_some_and(|p| p.uses_eff_var());
        if !receiver_uses_eff {
            let expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(expected_receiver_ty);
            if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
                return Err(ExprTypeError::CallReceiverTypeMismatch {
                    callee: callee_fqn,
                    expected: lower.fmt_type(expected_receiver_ty),
                    found: lower.fmt_type(actual_receiver_ty),
                    span: receiver.span.into(),
                });
            }
            check_fn_value_to_any_erasure_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
            check_nogc_boxing_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
        }

        // 先在"期望类型语境"下推导每个显式实参的最终类型（lambda 会在此处被真正类型检查）。
        let mut checked_arg_tys: Vec<TypeId> = effective_call_args.iter().map(|a| a.ty).collect();
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &effective_call_args[arg_idx];
            if arg.is_spread {
                continue;
            }
            let found_ty = inputs.infer_in_expected(
                lower,
                arg.expr,
                expected_ty,
                ExpectedTypeFrom::new(format!(
                    "`{}` 的第 {} 个形参 `{}`",
                    callee_fqn,
                    param_idx + 2,
                    sig.param_names[param_idx + 1]
                )),
            )?;
            checked_arg_tys[arg_idx] = found_ty;
        }

        // T0509/T0624/T0628a：推断 `eff` row 参数：
        // - 从 lambda body 的 required effects 推断（`found - base`）；
        // - 从 `Type<eff Row>` receiver/形参的实参类型提取 row 约束（`found - base`）。
        let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.cloned() {
            explicit_eff_arg
        } else if let Some(eff_param) = &sig.eff_param {
            let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

            // receiver 约束：`ReceiverType<eff Row>`。
            if let Some(base) = sig
                .param_nominal_eff_eff_base
                .first()
                .and_then(|b| b.as_ref())
            {
                let base = substitute_type_args_in_effect_row(
                    base.clone(),
                    &sig.type_params,
                    &instantiated.type_args,
                    lower,
                    call_expr.span,
                )?;
                if let Some(found_row) = nominal_eff_row_from_type(actual_receiver_ty, lower) {
                    let delta = effect_row_difference(&found_row, &base);
                    terms.extend(delta.terms);
                }
            }

            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                let arg = &effective_call_args[arg_idx];
                if arg.is_spread {
                    continue;
                }
                let sig_param_idx = param_idx + 1; // 跳过 receiver

                // `Type<eff Row>` 形参约束。
                if let Some(base) = sig
                    .param_nominal_eff_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                {
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

                let Some(base) = sig
                    .param_fn_effect_eff_base
                    .get(sig_param_idx)
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
                if let TypeKind::Ref(RefTypeKind::Function(found_fun)) = lower.type_kind(found_ty) {
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

        // 若 receiver 依赖 `E`，现在 `E` 已实例化完毕，补做 receiver mismatch 检查。
        if receiver_uses_eff {
            let expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(expected_receiver_ty);
            if !is_type_assignable(actual_receiver_ty, expected_receiver_ty, lower, builtins) {
                return Err(ExprTypeError::CallReceiverTypeMismatch {
                    callee: callee_fqn,
                    expected: lower.fmt_type(expected_receiver_ty),
                    found: lower.fmt_type(actual_receiver_ty),
                    span: receiver.span.into(),
                });
            }
            check_fn_value_to_any_erasure_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
            check_nogc_boxing_gate(
                actual_receiver_ty,
                expected_receiver_ty,
                receiver.span,
                lower,
                builtins,
            )?;
        }

        // 再做"可赋值"检查（此时 lambda 的 effects 也已经被推断并写入 found_ty）。
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &effective_call_args[arg_idx];
            let found_ty = checked_arg_tys[arg_idx];

            if arg.is_spread {
                let sig_param_idx = param_idx + 1;
                if !sig
                    .param_is_vararg
                    .get(sig_param_idx)
                    .copied()
                    .unwrap_or(false)
                {
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
                check_nogc_boxing_gate(found_ty, expected_ty, arg.expr.span, lower, builtins)?;
                continue;
            }
            if literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins) {
                continue;
            }

            return Err(ExprTypeError::CallArgTypeMismatch {
                callee: callee_fqn,
                // extension 调用：`receiver.member(arg1, arg2, ...)` 的第 1 个"显式参数"
                // 对应 `sig.params[1]`（跳过 receiver 参数）。
                index: param_idx + 1,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: arg.expr.span.into(),
            });
        }

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

        // T0712/T5000e2b：记录带 receiver 的 direct-call 实例请求。
        // 对 generic owner member/getter，这里需要把 owner-specialization 的 concrete args
        // 放在函数自身 type args 之前，形成可复用的实例身份。
        let eff_args = sig
            .eff_param
            .as_ref()
            .map(|_| vec![eff_arg.clone()])
            .unwrap_or_default();
        let type_args = combined_member_instance_type_args(
            &callee_fqn,
            actual_receiver_ty,
            &instantiated.type_args,
            lower,
        )?;
        lower.record_monomorph_call(
            callee_fqn.clone(),
            &sig.decl_file,
            sig.decl_span,
            &type_args,
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
                type_args,
                eff_args,
            },
        );
        if let Some(binding) =
            call_arg_binding_from_mapping_with_receiver_prefix(&mapping, effective_call_args)
        {
            lower.record_typechecked_call_arg_binding(call_expr.span, binding);
        }
        if used_unit_sugar {
            lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
        }

        let ret = if safe {
            lower.ty_option(instantiated.return_ty)
        } else {
            instantiated.return_ty
        };

        return Ok(ret);
    }

    #[derive(Debug, Clone)]
    struct MatchedExtensionOverload<'a> {
        sig: &'a FunSigOwned,
        instantiated: InstantiatedFunSig,
        eff_arg: EffectRow,
        receiver_ty: TypeId,
        /// `call_args[arg_idx]` 对应的"期望类型"（排除了 receiver 参数）。
        expected_arg_tys: Vec<TypeId>,
        /// 调用点需要用默认值补齐的形参个数（越少越"具体"）。
        defaults_used: usize,
        /// 形参 -> 实参绑定（不含 receiver，receiver 由调用形状隐式提供）。
        mapping: Vec<ParamArgBinding>,
        /// 当前候选是否通过 typed `Unit` zero-arg sugar 匹配得到。
        used_unit_sugar: bool,
    }

    fn is_strictly_more_specific_extension_overload(
        a: &MatchedExtensionOverload<'_>,
        b: &MatchedExtensionOverload<'_>,
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> bool {
        let a_le_b = is_type_assignable(a.receiver_ty, b.receiver_ty, lower, builtins)
            && a.expected_arg_tys
                .iter()
                .zip(b.expected_arg_tys.iter())
                .all(|(a_ty, b_ty)| is_type_assignable(*a_ty, *b_ty, lower, builtins));
        let b_le_a = is_type_assignable(b.receiver_ty, a.receiver_ty, lower, builtins)
            && b.expected_arg_tys
                .iter()
                .zip(a.expected_arg_tys.iter())
                .all(|(b_ty, a_ty)| is_type_assignable(*b_ty, *a_ty, lower, builtins));

        a_le_b && !b_le_a
    }

    fn pick_most_specific_extension_overload(
        candidates: &[MatchedExtensionOverload<'_>],
        lower: &TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Option<usize> {
        for (idx, cand) in candidates.iter().enumerate() {
            let mut ok = true;
            for (other_idx, other) in candidates.iter().enumerate() {
                if idx == other_idx {
                    continue;
                }
                if !is_strictly_more_specific_extension_overload(cand, other, lower, builtins) {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(idx);
            }
        }

        // tie-break：默认参数更少者优先（"非默认参数优先"）。
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

    // 多候选：先按 receiver/参数匹配筛选，再用 receiver/参数 specificity 选出 most-specific（T0455）。
    let mut matched: Vec<MatchedExtensionOverload<'_>> = Vec::new();

    for cand in ext_candidates {
        let Some((user_param_tys, param_has_defaults, param_is_vararg)) =
            user_visible_param_slices_after_receiver(
                &cand.params,
                &cand.param_has_defaults,
                &cand.param_is_vararg,
            )
        else {
            continue;
        };
        let Some(param_names) = cand.param_names.get(1..) else {
            continue;
        };

        let exact_mapping = map_call_args_to_params_with_defaults_and_varargs(
            &call_args,
            param_names,
            param_has_defaults,
            param_is_vararg,
        );
        let (call_args_for_candidate, mapping, used_unit_sugar) =
            if let Some(mapping) = exact_mapping {
                (&call_args, mapping, false)
            } else if can_use_zero_arg_unit_call_sugar(
                args,
                user_param_tys,
                param_has_defaults,
                param_is_vararg,
                lower,
            ) {
                let Some(sugar_call_args) = sugar_call_args.as_ref() else {
                    continue;
                };
                let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                    sugar_call_args,
                    param_names,
                    param_has_defaults,
                    param_is_vararg,
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

        let mut arg_constraints: Vec<GenericArgConstraint> = Vec::new();
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let sig_param_idx = param_idx + 1; // 跳过 receiver
            let arg = &call_args_for_candidate[arg_idx];
            if arg.is_spread {
                if !cand
                    .param_is_vararg
                    .get(sig_param_idx)
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
                    arg_constraints.push(GenericArgConstraint {
                        expected: cand.params[sig_param_idx],
                        found: found_elem,
                        found_is_placeholder: false,
                        from: format!("第 {} 个实参（spread）", arg_idx + 1),
                        span: arg.expr.span,
                    });
                }
                continue;
            }

            arg_constraints.push(GenericArgConstraint {
                expected: cand.params[sig_param_idx],
                found: arg.ty,
                found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                from: format!("第 {} 个实参", arg_idx + 1),
                span: arg.expr.span,
            });
        }
        if !ok {
            continue;
        }

        let mut instantiated = match instantiate_fun_sig_for_call_with_optional_explicit_type_args(
            &callee_fqn,
            call_expr.span,
            cand,
            explicit_type_args,
            std::iter::once(GenericArgConstraint {
                expected: cand.params[0],
                found: actual_receiver_ty,
                found_is_placeholder: false,
                from: "接收者（receiver）".to_string(),
                span: receiver.span,
            })
            .chain(arg_constraints),
            lower,
            builtins,
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // T0129：检查 where 约束；不满足则跳过该候选。
        if check_fun_where_constraints_after_instantiation(
            &callee_fqn,
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

        // receiver mismatch 检查：同单候选路径，若 receiver 的期望类型依赖 `E`，
        // 必须等到 `E` 推断/实例化后才能确定 receiver 是否匹配（T0624）。
        let receiver_uses_eff = cand.eff_param.is_some()
            && cand
                .param_eff_row_var_subst
                .first()
                .is_some_and(|p| p.uses_eff_var());
        let mut cand_expected_receiver_ty = instantiated
            .params
            .first()
            .copied()
            .unwrap_or(cand.params[0]);
        if !receiver_uses_eff
            && !is_type_assignable(
                actual_receiver_ty,
                cand_expected_receiver_ty,
                lower,
                builtins,
            )
        {
            continue;
        }

        // 只在需要时（lambda）进入 expected-context typecheck（与 direct call 多候选路径保持一致）。
        let mut ok = true;
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

            let expected_ty = instantiated.params[param_idx + 1];
            let found_ty = match inputs.infer_in_expected(
                lower,
                arg.expr,
                expected_ty,
                ExpectedTypeFrom::new(format!(
                    "`{}` 的第 {} 个形参 `{}`",
                    callee_fqn,
                    param_idx + 2,
                    cand.param_names[param_idx + 1]
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
        // - 从 `Type<eff Row>` receiver/形参的实参类型提取 row 约束（`found - base`）。
        let eff_arg = if let Some(explicit_eff_arg) = explicit_eff_arg.cloned() {
            explicit_eff_arg
        } else if let Some(eff_param) = &cand.eff_param {
            let mut terms: Vec<TypeId> = eff_param.default.terms.clone();

            if let Some(base) = cand
                .param_nominal_eff_eff_base
                .first()
                .and_then(|b| b.as_ref())
            {
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
                if let Some(found_row) = nominal_eff_row_from_type(actual_receiver_ty, lower) {
                    let delta = effect_row_difference(&found_row, &base);
                    terms.extend(delta.terms);
                }
            }

            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                let arg = &call_args_for_candidate[arg_idx];
                if arg.is_spread {
                    continue;
                }
                let sig_param_idx = param_idx + 1; // 跳过 receiver

                if let Some(base) = cand
                    .param_nominal_eff_eff_base
                    .get(sig_param_idx)
                    .and_then(|b| b.as_ref())
                {
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

                let Some(base) = cand
                    .param_fn_effect_eff_base
                    .get(sig_param_idx)
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
                if let TypeKind::Ref(RefTypeKind::Function(found_fun)) = lower.type_kind(found_ty) {
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

        // 若 receiver 依赖 `E`，现在 `E` 已实例化完毕，补做 receiver mismatch 检查。
        if receiver_uses_eff {
            cand_expected_receiver_ty = instantiated
                .params
                .first()
                .copied()
                .unwrap_or(cand.params[0]);
            if !is_type_assignable(
                actual_receiver_ty,
                cand_expected_receiver_ty,
                lower,
                builtins,
            ) {
                continue;
            }
        }

        // 参数可赋值检查（跳过 receiver；只检查显式实参）。
        for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
            let expected_ty = instantiated.params[param_idx + 1];
            let arg = &call_args_for_candidate[arg_idx];
            let found_ty = checked_arg_tys[arg_idx];

            if arg.is_spread {
                let sig_param_idx = param_idx + 1;
                if !cand
                    .param_is_vararg
                    .get(sig_param_idx)
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
            let defaults_used = mapping
                .iter()
                .filter(|b| matches!(b, ParamArgBinding::Default))
                .count();
            let mut expected_arg_tys = vec![builtins.nothing; call_args_for_candidate.len()];
            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                expected_arg_tys[arg_idx] = instantiated.params[param_idx + 1];
            }

            matched.push(MatchedExtensionOverload {
                sig: cand,
                receiver_ty: cand_expected_receiver_ty,
                expected_arg_tys,
                instantiated,
                eff_arg,
                defaults_used,
                mapping,
                used_unit_sugar,
            });
        }
    }

    if matched.iter().any(|cand| !cand.used_unit_sugar) {
        matched.retain(|cand| !cand.used_unit_sugar);
    }

    let chosen = match matched.len() {
        0 => {
            return Err(ExprTypeError::NoMatchingOverload {
                callee: callee_fqn,
                span: call_expr.span.into(),
            });
        }
        1 => matched.pop().expect("len == 1"),
        _ => {
            let Some(idx) = pick_most_specific_extension_overload(&matched, lower, builtins) else {
                let name = short_name_from_fqn(&callee_fqn).to_string();
                let candidates = join_overload_signatures(
                    matched
                        .iter()
                        .map(|c| {
                            fmt_overload_signature(
                                &name,
                                Some(c.receiver_ty),
                                c.instantiated.params.get(1..).unwrap_or_default(),
                                lower,
                            )
                        })
                        .collect(),
                );
                return Err(ExprTypeError::AmbiguousOverload {
                    callee: callee_fqn,
                    candidates,
                    span: call_expr.span.into(),
                });
            };
            matched.swap_remove(idx)
        }
    };

    check_unsafe_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;
    check_nogc_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;
    check_const_fun_call_gate(&callee_fqn, chosen.sig, call_expr.span, lower)?;
    emit_deprecated_call_warning(&callee_fqn, chosen.sig, call_expr.span, lower);

    // `@NoGC`：已知分配点（boxing）门禁（receiver + 显式实参）。
    check_fn_value_to_any_erasure_gate(
        actual_receiver_ty,
        chosen.receiver_ty,
        receiver.span,
        lower,
        builtins,
    )?;
    check_nogc_boxing_gate(
        actual_receiver_ty,
        chosen.receiver_ty,
        receiver.span,
        lower,
        builtins,
    )?;
    let chosen_call_args = if chosen.used_unit_sugar {
        sugar_call_args
            .as_ref()
            .expect("typed Unit sugar 选择的扩展调用应有合成实参")
    } else {
        &call_args
    };
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

    // T0712/T5000e2b：记录带 receiver 的 direct-call 实例请求。
    // 对 generic owner member/getter，这里需要把 owner-specialization 的 concrete args
    // 放在函数自身 type args 之前，形成可复用的实例身份。
    let eff_args = chosen
        .sig
        .eff_param
        .as_ref()
        .map(|_| vec![chosen.eff_arg.clone()])
        .unwrap_or_default();
    let type_args = combined_member_instance_type_args(
        &callee_fqn,
        actual_receiver_ty,
        &chosen.instantiated.type_args,
        lower,
    )?;
    lower.record_monomorph_call(
        callee_fqn.clone(),
        &chosen.sig.decl_file,
        chosen.sig.decl_span,
        &type_args,
        &eff_args,
        call_expr.span,
    );
    lower.record_top_level_fun_call_binding(
        call_expr.span,
        ast::TopLevelFunCallBinding {
            fqn: callee_fqn.clone(),
            decl_file: chosen.sig.decl_file.clone(),
            decl_span: chosen.sig.decl_span,
            is_intrinsic: chosen.sig.is_intrinsic,
            intrinsic_entry_name: chosen.sig.intrinsic_entry_name.clone(),
            type_args,
            eff_args,
        },
    );
    if let Some(binding) =
        call_arg_binding_from_mapping_with_receiver_prefix(&chosen.mapping, chosen_call_args)
    {
        lower.record_typechecked_call_arg_binding(call_expr.span, binding);
    }
    if chosen.used_unit_sugar {
        lower.record_zero_arg_unit_call_sugar_site(call_expr.span);
    }

    let ret = if safe {
        lower.ty_option(chosen.instantiated.return_ty)
    } else {
        chosen.instantiated.return_ty
    };

    Ok(ret)
}
#[derive(Debug, Clone)]
pub(super) struct InstantiatedFunSig {
    pub(super) params: Vec<TypeId>,
    pub(super) return_ty: TypeId,
    /// 推断/显式提供的泛型实参（与 `sig.type_params` 对齐）。
    ///
    /// 当前阶段（T0505）仅支持单一类型参数；未来可扩展为多参数。
    pub(super) type_args: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub(super) struct GenericArgConstraint {
    pub(super) expected: TypeId,
    pub(super) found: TypeId,
    /// 若为 `true`，表示 `found` 只是"为了 overload 筛选占位"的类型（例如 lambda 在预收集阶段被记为 `Any`），
    /// 不应当用于泛型推断。
    pub(super) found_is_placeholder: bool,
    /// 该约束来自哪里（用于 diagnostics；例如"第 2 个实参"/"receiver"）。
    pub(super) from: String,
    /// 约束来源对应的 span（用于把推断失败映射回具体位置）。
    pub(super) span: Span,
}

fn effect_row_base_excluding_eff_var(
    row: &ast::EffectRowExpr,
    eff_name: &str,
    source: &SourceFile,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<EffectRow>, ExprTypeError> {
    if row.terms.is_empty() {
        return Ok(None);
    }

    let mut used = false;
    let mut base_terms: Vec<ast::TypePath> = Vec::with_capacity(row.terms.len());

    for term in &row.terms {
        let is_eff_var = term.segments.len() == 1
            && term.args.is_empty()
            && term.segments[0].text(source) == eff_name;
        if is_eff_var {
            used = true;
            continue;
        }
        base_terms.push(term.clone());
    }

    if !used {
        return Ok(None);
    }

    let base_expr = ast::EffectRowExpr {
        span: row.span,
        terms: base_terms,
        // `!` 的语义当前仅在函数声明处使用（见 T0626/T0627）。
        // 对函数类型/`Type<eff ...>` 的 row 这里只保留结构信息以便未来扩展。
        closed: row.closed,
    };

    Ok(Some(lower.lower_effect_row_expr(Some(&base_expr))?))
}

pub(super) fn type_ref_fn_effect_eff_base(
    ty_ref: &ast::TypeRef,
    eff_name: &str,
    source: &SourceFile,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<EffectRow>, ExprTypeError> {
    let ast::TypeRef::Function(fun) = ty_ref else {
        return Ok(None);
    };

    let Some(effects) = fun.effects.as_ref() else {
        return Ok(None);
    };

    effect_row_base_excluding_eff_var(effects, eff_name, source, lower)
}

/// `Type<eff Row>`：use-site effect row 实参引用函数级 `eff` 变量（例如 `eff E` / `eff (E + IO)`）。
///
/// 返回值：
/// - `Ok(None)`：不引用 `E`
/// - `Ok(Some(base))`：引用了 `E`，其中 `base` 为把 `E` 移除后剩余的常量项（已 lowering）
pub(super) fn type_ref_nominal_eff_eff_base(
    ty_ref: &ast::TypeRef,
    eff_name: &str,
    source: &SourceFile,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<EffectRow>, ExprTypeError> {
    match ty_ref {
        ast::TypeRef::Nullable { inner, .. } => {
            type_ref_nominal_eff_eff_base(inner, eff_name, source, lower)
        }
        ast::TypeRef::Path(path) => {
            let Some(ast::TypeRef::EffectRowArg { row, .. }) = path
                .args
                .iter()
                .find(|a| matches!(a, ast::TypeRef::EffectRowArg { .. }))
            else {
                return Ok(None);
            };

            effect_row_base_excluding_eff_var(row, eff_name, source, lower)
        }
        _ => Ok(None),
    }
}

pub(super) fn type_param_name(ty: TypeId, lower: &TypeLowering<'_>) -> String {
    match lower.type_kind(ty) {
        TypeKind::Param(p) => p.name,
        _ => "<type param>".to_string(),
    }
}

pub(super) fn collect_type_arg_candidates_for_single_type_param(
    expected: TypeId,
    found: TypeId,
    param_ty: TypeId,
    out: &mut Vec<TypeId>,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
    found_is_placeholder: bool,
) {
    if expected == param_ty {
        if found == builtins.nothing {
            return;
        }
        if found_is_placeholder && found == builtins.any {
            return;
        }
        out.push(found);
        return;
    }

    let expected_kind = lower.type_kind(expected);
    let found_kind = lower.type_kind(found);

    match (expected_kind, found_kind) {
        (
            TypeKind::Value(ValueTypeKind::Option(expected_inner)),
            TypeKind::Value(ValueTypeKind::Option(found_inner)),
        ) => {
            collect_type_arg_candidates_for_single_type_param(
                expected_inner,
                found_inner,
                param_ty,
                out,
                lower,
                builtins,
                found_is_placeholder,
            );
        }
        (
            TypeKind::Value(ValueTypeKind::Tuple(expected_elems)),
            TypeKind::Value(ValueTypeKind::Tuple(found_elems)),
        ) => {
            if expected_elems.len() != found_elems.len() {
                return;
            }
            for (e, f) in expected_elems.into_iter().zip(found_elems) {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
            TypeKind::Ref(RefTypeKind::Nominal(found_nominal)),
        ) => {
            if expected_nominal.fqn != found_nominal.fqn {
                return;
            }
            if expected_nominal.args.len() != found_nominal.args.len() {
                return;
            }
            for (e, f) in expected_nominal.args.into_iter().zip(found_nominal.args) {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }
        }
        (
            TypeKind::Value(ValueTypeKind::Nominal(expected_nominal)),
            TypeKind::Value(ValueTypeKind::Nominal(found_nominal)),
        ) => {
            if expected_nominal.fqn != found_nominal.fqn {
                return;
            }
            if expected_nominal.args.len() != found_nominal.args.len() {
                return;
            }
            for (e, f) in expected_nominal.args.into_iter().zip(found_nominal.args) {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }
        }
        (
            TypeKind::Ref(RefTypeKind::Function(expected_fun)),
            TypeKind::Ref(RefTypeKind::Function(found_fun)),
        ) => {
            if expected_fun.receiver.is_some() != found_fun.receiver.is_some() {
                return;
            }
            if expected_fun.params.len() != found_fun.params.len() {
                return;
            }

            if let (Some(e), Some(f)) = (expected_fun.receiver, found_fun.receiver) {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }

            for (e, f) in expected_fun.params.into_iter().zip(found_fun.params) {
                collect_type_arg_candidates_for_single_type_param(
                    e,
                    f,
                    param_ty,
                    out,
                    lower,
                    builtins,
                    found_is_placeholder,
                );
            }

            collect_type_arg_candidates_for_single_type_param(
                expected_fun.return_ty,
                found_fun.return_ty,
                param_ty,
                out,
                lower,
                builtins,
                found_is_placeholder,
            );
        }
        _ => {}
    }
}

pub(super) fn substitute_single_type_param(
    ty: TypeId,
    param_ty: TypeId,
    arg_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<TypeId, ExprTypeError> {
    if ty == param_ty {
        return Ok(arg_ty);
    }

    match lower.type_kind(ty) {
        TypeKind::Param(_) => Ok(ty),
        TypeKind::StarProjection(_) => Ok(ty),
        TypeKind::Ref(RefTypeKind::Any | RefTypeKind::String) => Ok(ty),
        TypeKind::Value(ValueTypeKind::Unit)
        | TypeKind::Value(ValueTypeKind::Nothing)
        | TypeKind::Value(ValueTypeKind::Bool)
        | TypeKind::Value(ValueTypeKind::Char)
        | TypeKind::Value(ValueTypeKind::Float64)
        | TypeKind::Value(ValueTypeKind::Float32)
        | TypeKind::Value(ValueTypeKind::Int)
        | TypeKind::Value(ValueTypeKind::UInt)
        | TypeKind::Value(ValueTypeKind::IntN(_))
        | TypeKind::Value(ValueTypeKind::UIntN(_)) => Ok(ty),
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let new_inner = substitute_single_type_param(inner, param_ty, arg_ty, lower, use_span)?;
            if new_inner == inner {
                return Ok(ty);
            }
            Ok(lower.ty_option(new_inner))
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let mut changed = false;
            let mut out: Vec<TypeId> = Vec::with_capacity(elements.len());
            for e in elements {
                let new_e = substitute_single_type_param(e, param_ty, arg_ty, lower, use_span)?;
                if new_e != e {
                    changed = true;
                }
                out.push(new_e);
            }
            if !changed {
                return Ok(ty);
            }
            Ok(lower.ty_tuple(out))
        }
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let mut args: Vec<TypeId> = Vec::with_capacity(nominal.args.len());
            for a in nominal.args {
                let new_a = substitute_single_type_param(a, param_ty, arg_ty, lower, use_span)?;
                if new_a != a {
                    changed = true;
                }
                args.push(new_a);
            }

            // T0624：名义类型的 `eff` row 参数同样需要参与 substitution（例如 `Raise<T>` 出现在 row 里）。
            let eff = match nominal.eff {
                Some(row) => {
                    let mut eff_changed = false;
                    let mut out_terms: Vec<TypeId> = Vec::with_capacity(row.terms.len());
                    for term in row.terms {
                        let new_term =
                            substitute_single_type_param(term, param_ty, arg_ty, lower, use_span)?;
                        if new_term != term {
                            eff_changed = true;
                        }
                        out_terms.push(new_term);
                    }
                    if eff_changed {
                        changed = true;
                        Some(EffectRow::new(out_terms))
                    } else {
                        Some(EffectRow { terms: out_terms })
                    }
                }
                None => None,
            };

            if !changed {
                return Ok(ty);
            }

            // T1011/T1025：`Ptr<T>` 的 pointee 必须是 GC-free 值类型；该门禁必须在泛型实例化/替换时同样生效，
            // 否则可通过 `uintPtrToPtr<String>` / `p.cast<String>()` 等绕过。
            if nominal.fqn == PTR_FQN
                && let Some(pointee) = args.first().copied()
            {
                // 与 TypeLowering::check_ptr_pointee_gc_free 一致：允许 sysroot 内部未实例化的 `Ptr<T>` 出现在签名中。
                if let TypeKind::Param(p) = lower.type_kind(pointee) {
                    if p.decl_file
                        .components()
                        .any(|c| c.as_os_str() == std::ffi::OsStr::new("sysroot"))
                    {
                        // ok
                    } else if !lower.is_gc_free_value_type(pointee)? {
                        return Err(TypeLowerError::PtrPointeeMustBeGcFree {
                            found: lower.fmt_type(pointee),
                            span: use_span.into(),
                        }
                        .into());
                    }
                } else if !lower.is_gc_free_value_type(pointee)? {
                    return Err(TypeLowerError::PtrPointeeMustBeGcFree {
                        found: lower.fmt_type(pointee),
                        span: use_span.into(),
                    }
                    .into());
                }
            }

            Ok(lower.intern_type_kind(TypeKind::Ref(RefTypeKind::Nominal(
                crate::ty::NominalType {
                    fqn: nominal.fqn,
                    args,
                    eff,
                },
            ))))
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let mut changed = false;
            let mut args: Vec<TypeId> = Vec::with_capacity(nominal.args.len());
            for a in nominal.args {
                let new_a = substitute_single_type_param(a, param_ty, arg_ty, lower, use_span)?;
                if new_a != a {
                    changed = true;
                }
                args.push(new_a);
            }

            let eff = match nominal.eff {
                Some(row) => {
                    let mut eff_changed = false;
                    let mut out_terms: Vec<TypeId> = Vec::with_capacity(row.terms.len());
                    for term in row.terms {
                        let new_term =
                            substitute_single_type_param(term, param_ty, arg_ty, lower, use_span)?;
                        if new_term != term {
                            eff_changed = true;
                        }
                        out_terms.push(new_term);
                    }
                    if eff_changed {
                        changed = true;
                        Some(EffectRow::new(out_terms))
                    } else {
                        Some(EffectRow { terms: out_terms })
                    }
                }
                None => None,
            };

            if !changed {
                return Ok(ty);
            }

            // T1011/T1025：同上（value nominal）。
            if nominal.fqn == PTR_FQN
                && let Some(pointee) = args.first().copied()
            {
                if let TypeKind::Param(p) = lower.type_kind(pointee) {
                    if p.decl_file
                        .components()
                        .any(|c| c.as_os_str() == std::ffi::OsStr::new("sysroot"))
                    {
                        // ok
                    } else if !lower.is_gc_free_value_type(pointee)? {
                        return Err(TypeLowerError::PtrPointeeMustBeGcFree {
                            found: lower.fmt_type(pointee),
                            span: use_span.into(),
                        }
                        .into());
                    }
                } else if !lower.is_gc_free_value_type(pointee)? {
                    return Err(TypeLowerError::PtrPointeeMustBeGcFree {
                        found: lower.fmt_type(pointee),
                        span: use_span.into(),
                    }
                    .into());
                }
            }

            // `FunPtr<F>` 的 native surface gate 也必须在泛型实例化/替换时重放，避免通过
            // `uintPtrToFunPtr<() -> Int / Ask>(...)` 或 `uintPtrToFunPtr<(Point) -> Int>(...)`
            // 这类路径绕过前端约束。
            if nominal.fqn == FUNPTR_FQN
                && let Some(sig) = args.first().copied()
            {
                lower.check_funptr_signature_contract(sig, use_span)?;
            }

            Ok(
                lower.intern_type_kind(TypeKind::Value(ValueTypeKind::Nominal(
                    crate::ty::NominalType {
                        fqn: nominal.fqn,
                        args,
                        eff,
                    },
                ))),
            )
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let mut changed = false;

            let receiver = match fun.receiver {
                Some(r) => {
                    let new_r = substitute_single_type_param(r, param_ty, arg_ty, lower, use_span)?;
                    if new_r != r {
                        changed = true;
                    }
                    Some(new_r)
                }
                None => None,
            };

            let mut params: Vec<TypeId> = Vec::with_capacity(fun.params.len());
            for p in fun.params {
                let new_p = substitute_single_type_param(p, param_ty, arg_ty, lower, use_span)?;
                if new_p != p {
                    changed = true;
                }
                params.push(new_p);
            }

            let return_ty =
                substitute_single_type_param(fun.return_ty, param_ty, arg_ty, lower, use_span)?;
            if return_ty != fun.return_ty {
                changed = true;
            }

            let mut effects_changed = false;
            let original_terms = fun.effects.terms;
            let mut effect_terms: Vec<TypeId> = Vec::with_capacity(original_terms.len());
            for e in original_terms {
                let new_e = substitute_single_type_param(e, param_ty, arg_ty, lower, use_span)?;
                if new_e != e {
                    effects_changed = true;
                }
                effect_terms.push(new_e);
            }
            let effects = if effects_changed {
                changed = true;
                EffectRow::new(effect_terms)
            } else {
                EffectRow {
                    terms: effect_terms,
                }
            };

            if !changed {
                return Ok(ty);
            }

            Ok(lower.ty_function(receiver, params, return_ty, effects, fun.effects_closed))
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            let mut changed = false;
            let mut variants: Vec<TypeId> = Vec::with_capacity(union.variants.len());
            for v in union.variants {
                let new_v = substitute_single_type_param(v, param_ty, arg_ty, lower, use_span)?;
                if new_v != v {
                    changed = true;
                }
                variants.push(new_v);
            }
            if !changed {
                return Ok(ty);
            }
            Ok(lower.ty_union(variants))
        }
    }
}

/// 将签名类型里出现的 `E + base`（包含嵌套位置）统一实例化为 `E_arg + base`（T0628b）。
///
/// 说明：
/// - `sig` 来自"声明处默认 `E = default`"语境下的 lowering；
/// - `instantiated` 已完成 type args 的 substitution（T0505），但其内部仍可能残留：
///   - function type effects 上的默认 `E` 结果（例如默认 `Pure`）
///   - nominal use-site `eff` 实参里的默认 `E` 结果
/// - 该函数只负责把这些位置替换为调用点推断出的 `eff_arg`，并返回新的 `TypeId`。
fn instantiate_eff_row_var_in_sig_types(
    sig: &FunSigOwned,
    instantiated: &mut InstantiatedFunSig,
    eff_arg: &EffectRow,
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<(), ExprTypeError> {
    if sig.eff_param.is_none() {
        return Ok(());
    }

    if instantiated.params.len() != sig.param_eff_row_var_subst.len() {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "eff row substitution（sig/instantiated param arity mismatch）",
            span: use_span.into(),
        });
    }

    for (idx, plan) in sig.param_eff_row_var_subst.iter().enumerate() {
        if !plan.uses_eff_var() {
            continue;
        }
        let cur = instantiated.params[idx];
        instantiated.params[idx] = apply_eff_row_var_subst_plan(
            cur,
            plan,
            eff_arg,
            &sig.type_params,
            &instantiated.type_args,
            lower,
            use_span,
        )?;
    }

    if sig.return_eff_row_var_subst.uses_eff_var() {
        instantiated.return_ty = apply_eff_row_var_subst_plan(
            instantiated.return_ty,
            &sig.return_eff_row_var_subst,
            eff_arg,
            &sig.type_params,
            &instantiated.type_args,
            lower,
            use_span,
        )?;
    }

    Ok(())
}

/// `found - base`：用于从 `found ⊆ (E + base)` 这类约束中提取 `E` 的最小增量项。
fn effect_row_difference(found: &EffectRow, base: &EffectRow) -> EffectRow {
    if found.terms.is_empty() {
        return EffectRow::pure();
    }
    if base.terms.is_empty() {
        return found.clone();
    }

    // terms 已排序；线性差集即可。
    let mut out: Vec<TypeId> = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < found.terms.len() {
        if j >= base.terms.len() {
            out.extend(found.terms[i..].iter().copied());
            break;
        }

        let a = found.terms[i];
        let b = base.terms[j];
        if a == b {
            i += 1;
            j += 1;
            continue;
        }
        if a < b {
            out.push(a);
            i += 1;
            continue;
        }
        // a > b：base 继续前进尝试追上 a
        j += 1;
    }

    EffectRow::new(out)
}

fn nominal_eff_row_from_type(ty: TypeId, lower: &TypeLowering<'_>) -> Option<EffectRow> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => nominal.eff,
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.eff,
        // nullable（`T?`）在 lowering 阶段会变成 `Option<T>`；这里递归剥一层便于推断 `E`。
        TypeKind::Value(ValueTypeKind::Option(inner)) => nominal_eff_row_from_type(inner, lower),
        _ => None,
    }
}

fn check_cross_thread_resume_policy(
    callee_fqn: &str,
    call_args: &[CallArgInfo<'_>],
    checked_arg_tys: &[TypeId],
    mapping_pairs: &[(usize, usize)],
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let base = cross_thread_resume_intrinsic_base(callee_fqn);
    if !matches!(
        base,
        "scoop.core.__scoop_thread_spawn_join_resume"
            | "scoop.core.__scoop_thread_spawn_join_resume_u64"
    ) {
        return Ok(());
    }
    let Some(arg_idx) = mapping_pairs
        .iter()
        .find_map(|(param_idx, arg_idx)| (*param_idx == 0).then_some(*arg_idx))
    else {
        return Ok(());
    };
    let Some(found_ty) = checked_arg_tys.get(arg_idx).copied() else {
        return Ok(());
    };
    let Some(row) = continuation_effect_row(found_ty, lower) else {
        return Ok(());
    };
    if row.is_pure() {
        return Ok(());
    }
    Err(ExprTypeError::CrossThreadResumeOutwardEffectsUnsupported {
        effects: fmt_effect_row(&row, lower),
        span: call_args[arg_idx].expr.span.into(),
    })
}

fn cross_thread_resume_intrinsic_base(callee_fqn: &str) -> &str {
    let base = callee_fqn
        .rsplit_once("::<")
        .map(|(base, _)| base)
        .unwrap_or(callee_fqn);
    base.split_once("$overload")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

fn check_thread_spawn_entry_policy(
    callee_fqn: &str,
    call_args: &[CallArgInfo<'_>],
    checked_arg_tys: &[TypeId],
    mapping_pairs: &[(usize, usize)],
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    if cross_thread_resume_intrinsic_base(callee_fqn) != "scoop.thread.threadSpawn" {
        return Ok(());
    }
    let Some(arg_idx) = mapping_pairs
        .iter()
        .find_map(|(param_idx, arg_idx)| (*param_idx == 0).then_some(*arg_idx))
    else {
        return Ok(());
    };
    let Some(found_ty) = checked_arg_tys.get(arg_idx).copied() else {
        return Ok(());
    };
    let is_effectively_pure = matches!(
        lower.type_kind(found_ty),
        TypeKind::Ref(RefTypeKind::Function(fun)) if fun.effects.is_pure()
    );
    if is_effectively_pure {
        return Ok(());
    }
    Err(ExprTypeError::ThreadSpawnEntryMustBePure {
        found: lower.fmt_type(found_ty),
        span: call_args[arg_idx].expr.span.into(),
    })
}

fn continuation_effect_row(ty: TypeId, lower: &TypeLowering<'_>) -> Option<EffectRow> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
            if nominal.fqn == "scoop.core.Continuation" =>
        {
            Some(nominal.eff.unwrap_or_else(EffectRow::pure))
        }
        TypeKind::Value(ValueTypeKind::Option(inner)) => continuation_effect_row(inner, lower),
        _ => None,
    }
}

fn type_param_bindings_from_sig(
    type_params: &[TypeId],
    lower: &TypeLowering<'_>,
) -> Vec<(String, TypeId)> {
    type_params
        .iter()
        .copied()
        .filter_map(|ty| match lower.type_kind(ty) {
            TypeKind::Param(p) => Some((p.name, ty)),
            _ => None,
        })
        .collect()
}

pub(super) fn substitute_type_args_in_effect_row(
    row: EffectRow,
    type_params: &[TypeId],
    type_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<EffectRow, ExprTypeError> {
    if type_params.is_empty() || type_args.is_empty() {
        return Ok(row);
    }

    let mut out_terms: Vec<TypeId> = Vec::with_capacity(row.terms.len());
    for effect in row.terms {
        let mut cur = effect;
        for (param_ty, arg_ty) in type_params.iter().copied().zip(type_args.iter().copied()) {
            cur = substitute_single_type_param(cur, param_ty, arg_ty, lower, use_span)?;
        }
        out_terms.push(cur);
    }

    Ok(EffectRow::new(out_terms))
}

/// 用显式类型实参实例化一个函数签名（`callee<T>()`）。
///
/// 说明：
/// - 该路径只做 substitution，不做类型实参推断；
/// - 主要用于"无值实参可用于推断"的调用（例如反射 intrinsics：`nameOf<T>()`）。
pub(super) fn instantiate_fun_sig_for_call_with_optional_explicit_type_args(
    callee: &str,
    call_span: Span,
    sig: &FunSigOwned,
    explicit_type_args: Option<&[TypeId]>,
    constraints: impl IntoIterator<Item = GenericArgConstraint>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<InstantiatedFunSig, ExprTypeError> {
    let explicit_len = explicit_type_args.map(|a| a.len()).unwrap_or(0);
    if explicit_len > sig.type_params.len() {
        return Err(ExprTypeError::GenericTypeArgArityMismatch {
            callee: callee.to_string(),
            expected: sig.type_params.len(),
            found: explicit_len,
            span: call_span.into(),
        });
    }

    if sig.type_params.is_empty() {
        if explicit_len != 0 {
            return Err(ExprTypeError::GenericTypeArgArityMismatch {
                callee: callee.to_string(),
                expected: 0,
                found: explicit_len,
                span: call_span.into(),
            });
        }
        return Ok(InstantiatedFunSig {
            params: sig.params.clone(),
            return_ty: sig.return_ty,
            type_args: Vec::new(),
        });
    }

    #[derive(Debug, Clone)]
    struct InferredTypeArgSource {
        from: String,
        span: Span,
    }

    // 先写入"显式类型实参"（若存在）；剩余 type params 尝试从约束中推断。
    let mut inferred: HashMap<TypeId, (TypeId, InferredTypeArgSource)> = HashMap::new();
    if let Some(explicit_type_args) = explicit_type_args {
        for (idx, arg_ty) in explicit_type_args.iter().copied().enumerate() {
            let Some(param_ty) = sig.type_params.get(idx).copied() else {
                continue;
            };
            inferred.insert(
                param_ty,
                (
                    arg_ty,
                    InferredTypeArgSource {
                        from: "显式类型实参".to_string(),
                        span: call_span,
                    },
                ),
            );
        }
    }

    // 逐约束收集每个 type param 的候选绑定并做一致性检查。
    for c in constraints {
        for param_ty in sig.type_params.iter().copied() {
            let mut candidates: Vec<TypeId> = Vec::new();
            collect_type_arg_candidates_for_single_type_param(
                c.expected,
                c.found,
                param_ty,
                &mut candidates,
                lower,
                builtins,
                c.found_is_placeholder,
            );

            for candidate in candidates {
                if let Some((bound, src)) = inferred.get_mut(&param_ty) {
                    if *bound == candidate {
                        continue;
                    }

                    let param_name = type_param_name(param_ty, lower);
                    return Err(ExprTypeError::GenericTypeArgInferenceConflict {
                        callee: Box::new(callee.to_string()),
                        param: Box::new(param_name),
                        left: Box::new(lower.fmt_type(*bound)),
                        right: Box::new(lower.fmt_type(candidate)),
                        left_from: Box::new(src.from.clone()),
                        right_from: Box::new(c.from.clone()),
                        span: c.span.into(),
                        previous: src.span.into(),
                    });
                }

                inferred.insert(
                    param_ty,
                    (
                        candidate,
                        InferredTypeArgSource {
                            from: c.from.clone(),
                            span: c.span,
                        },
                    ),
                );
            }
        }
    }

    // 确保每个 type param 都有绑定（显式或推断）。
    let mut type_args: Vec<TypeId> = Vec::with_capacity(sig.type_params.len());
    for param_ty in sig.type_params.iter().copied() {
        let Some((binding, _)) = inferred.get(&param_ty) else {
            let param_name = type_param_name(param_ty, lower);
            return Err(ExprTypeError::GenericTypeArgNotInferred {
                callee: callee.to_string(),
                param: param_name,
                span: call_span.into(),
            });
        };
        type_args.push(*binding);
    }

    let mut params: Vec<TypeId> = sig.params.clone();
    let mut return_ty: TypeId = sig.return_ty;
    for (param_ty, arg_ty) in sig
        .type_params
        .iter()
        .copied()
        .zip(type_args.iter().copied())
    {
        for p in &mut params {
            *p = substitute_single_type_param(*p, param_ty, arg_ty, lower, call_span)?;
        }
        return_ty = substitute_single_type_param(return_ty, param_ty, arg_ty, lower, call_span)?;
    }

    Ok(InstantiatedFunSig {
        params,
        return_ty,
        type_args,
    })
}

pub(super) fn instantiate_fun_sig_for_call(
    callee: &str,
    call_span: Span,
    sig: &FunSigOwned,
    constraints: impl IntoIterator<Item = GenericArgConstraint>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<InstantiatedFunSig, ExprTypeError> {
    instantiate_fun_sig_for_call_with_optional_explicit_type_args(
        callee,
        call_span,
        sig,
        None,
        constraints,
        lower,
        builtins,
    )
}

/// T0129：泛型函数调用处 where 约束检查。
///
/// 在 `instantiate_fun_sig_for_call*` 推断出具体 type args 后调用：
/// 遍历 `sig.where_constraints`，在声明处文件上下文中 lower bound，
/// 检查 `type_args[c.param_index]` 是否 assignable to bound_ty。
/// 当 type arg 仍为 `TypeKind::Param` 时跳过（泛型传递调用）。
fn check_fun_where_constraints_after_instantiation(
    callee: &str,
    call_span: Span,
    sig: &FunSigOwned,
    type_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    if sig.where_constraints.is_empty() {
        return Ok(());
    }

    // 构建 type param name → concrete type arg 的绑定表，
    // 用于在 lower bound TypeRef 时将 bound 中出现的 type param 替换为具体类型。
    let bindings: Vec<(String, TypeId)> = sig
        .type_params
        .iter()
        .zip(type_args.iter().copied())
        .map(|(param_ty, arg_ty)| {
            let name = match lower.type_kind(*param_ty) {
                TypeKind::Param(p) => p.name.clone(),
                _ => format!(
                    "#{}",
                    sig.type_params
                        .iter()
                        .position(|t| t == param_ty)
                        .unwrap_or(0)
                ),
            };
            (name, arg_ty)
        })
        .collect();

    for c in &sig.where_constraints {
        let Some(arg_ty) = type_args.get(c.param_index).copied() else {
            continue;
        };

        // 当 type arg 仍为 type param 时跳过（泛型传递调用，无法在此刻验证）。
        if matches!(lower.type_kind(arg_ty), TypeKind::Param(_)) {
            continue;
        }

        // 在声明处文件上下文中 lower bound TypeRef，应用 type arg 替换。
        let bound_ty = lower.lower_type_ref_in_decl_file_with_bindings(
            &sig.decl_file,
            bindings.iter().cloned(),
            &c.bound,
        )?;

        if is_type_assignable(arg_ty, bound_ty, lower, builtins) {
            continue;
        }

        return Err(ExprTypeError::FunWhereConstraintNotSatisfied {
            callee: callee.to_string(),
            param: c.param_name.clone(),
            arg: lower.fmt_type(arg_ty),
            bound: lower.fmt_type(bound_ty),
            span: call_span.into(),
        });
    }

    Ok(())
}

/// T0130：尝试通过 where 约束的 bound 接口解析 TypeKind::Param 接收者上的方法调用。
///
/// 当 receiver 类型为 `TypeKind::Param`（如 `T`）时，查找该 type param 的 where 约束，
/// 将 bound 接口的方法集合纳入候选。如果找到匹配的方法，返回 `Some(return_ty)`。
#[allow(clippy::too_many_arguments)]
fn try_infer_where_bound_method_call(
    source: &SourceFile,
    call_expr: &ast::Expr,
    receiver: &ast::Expr,
    receiver_ty: TypeId,
    param_name: &str,
    member: &ast::MemberIdent,
    args: &[ast::Expr],
    explicit_type_args: Option<&[TypeId]>,
    safe: bool,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let member_name = source.slice(member.span);
    let inputs = ExprInferInputs {
        source,
        builtins,
        locals,
        lambda_this_decl_span: None,
        comptime_bindings: None,
        top_level_types,
        top_level_funs,
        member_mutabilities: None,
        struct_field_types,
        loop_depth: 0,
        expected_return_ty: None,
    };
    let bounds = lower.lookup_where_bounds_for_param(param_name);
    if bounds.is_empty() {
        return Ok(None);
    }

    // 对每个 bound，lower 其 type ref 得到 bound 接口类型，然后查找接口方法。
    let bound_entries: Vec<_> = bounds
        .into_iter()
        .map(|b| (b.bound.clone(), b.decl_file.clone()))
        .collect();

    let call_args = collect_call_arg_infos(inputs, args, lower)?;

    for (bound_ref, decl_file) in &bound_entries {
        // Lower bound type ref 在声明处文件上下文中。
        let bound_ty = match lower.lower_type_ref_in_decl_file(decl_file, bound_ref) {
            Ok(ty) => ty,
            Err(_) => continue,
        };

        // 从 bound 类型中提取名义 FQN（接口 FQN）。
        let (bound_fqn, bound_args) = match try_extract_nominal_fqn_and_args(bound_ty, lower) {
            Some(pair) => pair,
            None => continue,
        };

        // 构造 interface 方法的 FQN：`InterfaceFqn.methodName`。
        let method_fqn = format!("{bound_fqn}.{member_name}");

        // 在索引中查找该方法。
        let sigs = collect_member_method_signatures_from_index(
            source,
            bound_ty,
            &bound_fqn,
            &bound_args,
            &method_fqn,
            lower,
            builtins,
        )?;

        if sigs.is_empty() {
            continue;
        }

        // 找到了匹配的 bound 方法——按照普通 member method call 的模式进行类型检查。
        check_call_arg_named_rules(&method_fqn, &call_args)?;
        check_call_named_args_exist_in_any_candidate(
            &method_fqn,
            &call_args,
            sigs.iter().filter_map(|sig| sig.param_names.get(1..)),
        )?;

        // 用 receiver 作为隐式第 0 个参数（使用 TypeKind::Param 的原始类型）。
        let receiver_arg = CallArgInfo {
            kind: CallArgKind::Positional,
            expr: receiver,
            ty: receiver_ty,
            is_spread: false,
            needs_expected_type: false,
        };

        let mut call_args_with_receiver = Vec::with_capacity(call_args.len() + 1);
        call_args_with_receiver.push(receiver_arg);
        call_args_with_receiver.extend(call_args.iter().cloned());

        // 尝试对每个签名候选进行匹配。
        'candidates: for cand in &sigs {
            let Some(mapping) = map_call_args_to_params_with_defaults_and_varargs(
                &call_args_with_receiver,
                &cand.param_names,
                &cand.param_has_defaults,
                &cand.param_is_vararg,
            ) else {
                continue;
            };

            let mapping_pairs = expand_param_arg_pairs(&mapping);
            let mut generic_constraints: Vec<GenericArgConstraint> =
                Vec::with_capacity(mapping_pairs.len());
            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                let arg = &call_args_with_receiver[arg_idx];
                generic_constraints.push(GenericArgConstraint {
                    expected: cand.params[param_idx],
                    found: arg.ty,
                    found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                    from: if arg_idx == 0 {
                        "接收者（receiver）".to_string()
                    } else {
                        format!("第 {} 个实参", arg_idx)
                    },
                    span: arg.expr.span,
                });
            }

            let mut instantiated =
                match instantiate_fun_sig_for_call_with_optional_explicit_type_args(
                    &method_fqn,
                    call_expr.span,
                    cand,
                    explicit_type_args,
                    generic_constraints,
                    lower,
                    builtins,
                ) {
                    Ok(s) => s,
                    Err(_) => continue 'candidates,
                };

            if check_fun_where_constraints_after_instantiation(
                &method_fqn,
                call_expr.span,
                cand,
                &instantiated.type_args,
                lower,
                builtins,
            )
            .is_err()
            {
                continue 'candidates;
            }

            for (param_idx, arg_idx) in mapping_pairs.iter().copied() {
                let expected_ty = instantiated.params[param_idx];
                let arg = &call_args_with_receiver[arg_idx];
                let found_ty = arg.ty;

                if arg.is_spread {
                    if !cand
                        .param_is_vararg
                        .get(param_idx)
                        .copied()
                        .unwrap_or(false)
                    {
                        continue 'candidates;
                    }
                    let Some(elem_tys) = spread_operand_element_types(found_ty, lower) else {
                        continue 'candidates;
                    };
                    if elem_tys
                        .into_iter()
                        .any(|elem_ty| !is_type_assignable(elem_ty, expected_ty, lower, builtins))
                    {
                        continue 'candidates;
                    }
                    continue;
                }

                if is_type_assignable(found_ty, expected_ty, lower, builtins)
                    || literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins)
                {
                    continue;
                }
                continue 'candidates;
            }

            let eff_arg = cand
                .eff_param
                .as_ref()
                .map(|param| param.default.clone())
                .unwrap_or_else(EffectRow::pure);
            if instantiate_eff_row_var_in_sig_types(
                cand,
                &mut instantiated,
                &eff_arg,
                lower,
                call_expr.span,
            )
            .is_err()
            {
                continue 'candidates;
            }

            check_unsafe_call_gate(&method_fqn, cand, call_expr.span, lower)?;
            check_nogc_call_gate(&method_fqn, cand, call_expr.span, lower)?;
            check_const_fun_call_gate(&method_fqn, cand, call_expr.span, lower)?;
            emit_deprecated_call_warning(&method_fqn, cand, call_expr.span, lower);

            let type_param_bindings = type_param_bindings_from_sig(&cand.type_params, lower);
            let eff_bindings: Vec<(String, EffectRow)> = cand
                .eff_param
                .as_ref()
                .map(|param| vec![(param.name.clone(), eff_arg.clone())])
                .unwrap_or_default();
            let lowered_effects = lower.lower_effect_row_expr_in_decl_file_with_scopes(
                &cand.decl_file,
                type_param_bindings,
                eff_bindings,
                cand.effects.as_ref(),
            );
            let call_effects = substitute_type_args_in_effect_row(
                lowered_effects?,
                &cand.type_params,
                &instantiated.type_args,
                lower,
                call_expr.span,
            )?;
            for effect in call_effects.terms.iter().copied() {
                lower.record_performed_effect(effect, call_expr.span);
            }

            // where-bound receiver 没有名义 owner 可从 `receiver_ty` 反推；实例身份应显式保留
            // bound 接口实参，再拼接方法自身的泛型实参。
            let mut type_args = bound_args.clone();
            type_args.extend(instantiated.type_args.iter().copied());
            let eff_args = cand
                .eff_param
                .as_ref()
                .map(|_| vec![eff_arg.clone()])
                .unwrap_or_default();

            lower.record_typechecked_member_resolution(
                member.span,
                ast::ResolvedMemberRef::Fun {
                    fqn: method_fqn.clone(),
                },
            );
            lower.record_monomorph_call(
                method_fqn.clone(),
                &cand.decl_file,
                cand.decl_span,
                &type_args,
                &eff_args,
                call_expr.span,
            );
            lower.record_top_level_fun_call_binding(
                call_expr.span,
                ast::TopLevelFunCallBinding {
                    fqn: method_fqn.clone(),
                    decl_file: cand.decl_file.clone(),
                    decl_span: cand.decl_span,
                    is_intrinsic: cand.is_intrinsic,
                    intrinsic_entry_name: cand.intrinsic_entry_name.clone(),
                    type_args,
                    eff_args,
                },
            );
            if let Some(binding) =
                call_arg_binding_from_mapping_with_receiver(&mapping, &call_args_with_receiver)
            {
                lower.record_typechecked_call_arg_binding(call_expr.span, binding);
            }

            let ret = if safe {
                lower.ty_option(instantiated.return_ty)
            } else {
                instantiated.return_ty
            };

            return Ok(Some(ret));
        }
    }

    Ok(None)
}
