//! Generic instantiation: type-param/effect-row substitution, where-bound resolution, cross-thread/spawn gates.

#![allow(dead_code)]

use super::*;

#[derive(Debug, Clone)]
pub(in crate::typecheck::expr) struct InstantiatedFunSig {
    pub(in crate::typecheck::expr) params: Vec<TypeId>,
    pub(in crate::typecheck::expr) return_ty: TypeId,
    /// 推断/显式提供的泛型实参（与 `sig.type_params` 对齐）。
    ///
    /// 当前阶段（T0505）仅支持单一类型参数；未来可扩展为多参数。
    pub(in crate::typecheck::expr) type_args: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub(in crate::typecheck::expr) struct GenericArgConstraint {
    pub(in crate::typecheck::expr) expected: TypeId,
    pub(in crate::typecheck::expr) found: TypeId,
    /// 若为 `true`，表示 `found` 只是"为了 overload 筛选占位"的类型（例如 lambda 在预收集阶段被记为 `Any`），
    /// 不应当用于泛型推断。
    pub(in crate::typecheck::expr) found_is_placeholder: bool,
    /// 该约束来自哪里（用于 diagnostics；例如"第 2 个实参"/"receiver"）。
    pub(in crate::typecheck::expr) from: String,
    /// 约束来源对应的 span（用于把推断失败映射回具体位置）。
    pub(in crate::typecheck::expr) span: Span,
}

pub(super) fn effect_row_base_excluding_eff_var(
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

pub(in crate::typecheck::expr) fn type_ref_fn_effect_eff_base(
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
pub(in crate::typecheck::expr) fn type_ref_nominal_eff_eff_base(
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

pub(in crate::typecheck::expr) fn type_param_name(ty: TypeId, lower: &TypeLowering<'_>) -> String {
    match lower.type_kind(ty) {
        TypeKind::Param(p) => p.name,
        _ => "<type param>".to_string(),
    }
}

pub(in crate::typecheck::expr) fn collect_type_arg_candidates_for_single_type_param(
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

pub(in crate::typecheck::expr) fn substitute_single_type_param(
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
pub(super) fn instantiate_eff_row_var_in_sig_types(
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
pub(super) fn effect_row_difference(found: &EffectRow, base: &EffectRow) -> EffectRow {
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

pub(super) fn nominal_eff_row_from_type(ty: TypeId, lower: &TypeLowering<'_>) -> Option<EffectRow> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => nominal.eff,
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.eff,
        // nullable（`T?`）在 lowering 阶段会变成 `Option<T>`；这里递归剥一层便于推断 `E`。
        TypeKind::Value(ValueTypeKind::Option(inner)) => nominal_eff_row_from_type(inner, lower),
        _ => None,
    }
}

pub(super) fn intrinsic_call_base(callee_fqn: &str) -> &str {
    let base = callee_fqn
        .rsplit_once("::<")
        .map(|(base, _)| base)
        .unwrap_or(callee_fqn);
    base.split_once("$overload")
        .map(|(base, _)| base)
        .unwrap_or(base)
}

pub(super) fn check_thread_spawn_entry_policy(
    callee_fqn: &str,
    call_args: &[CallArgInfo<'_>],
    checked_arg_tys: &[TypeId],
    mapping_pairs: &[(usize, usize)],
    lower: &TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    if intrinsic_call_base(callee_fqn) != "scoop.thread.threadSpawn" {
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

pub(super) fn type_param_bindings_from_sig(
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

pub(in crate::typecheck::expr) fn substitute_type_args_in_effect_row(
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
pub(in crate::typecheck::expr) fn instantiate_fun_sig_for_call_with_optional_explicit_type_args(
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

pub(in crate::typecheck::expr) fn instantiate_fun_sig_for_call(
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
pub(super) fn check_fun_where_constraints_after_instantiation(
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
        let bound_ty = lower.lower_bound_type_ref_in_decl_file_with_bindings(
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
pub(super) fn try_infer_where_bound_method_call(
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
        mutable_bindings: None,
        lambda_this_decl_span: None,
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
