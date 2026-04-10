//! effect row 变量（`<eff E>`）在类型中的实例化/替换（T0628b）。
//!
//! 目标：
//! - 支持把签名里出现的 `E + R`（以及 `Type<eff (E + R)>`）从“仅顶层参数”扩展到任意嵌套位置：
//!   - `Option<T>` / `T?`
//!   - tuple
//!   - union（运行期 LUB 的保守表示）
//!   - 多层 function type（参数/返回里再嵌套 function type）
//!   - nominal type 的 type args
//! - 避免在多个 call-site 路径里复制“替换 effect row”逻辑：用一棵 plan 表达“哪些位置需要替换”。
//!
//! 设计：
//! - 在收集函数签名时（仍有 AST `TypeRef`），构造 `EffRowVarSubstPlan`：
//!   - 只标记那些 **确实引用了 eff 变量名**（例如 `E`）的 row；
//!   - 对于每个被标记的 row，记录其 base row（把 `E` 移除后剩余的项，仍保留为 AST 以便后续按 type args 重新 lowering）。
//! - 调用点推断出 `E` 后，按 plan 遍历 `TypeId`，局部重建并写回替换后的类型。

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::expr::ExprTypeError;
use super::lower::TypeLowering;

/// 一棵“在 `TypeId` 中替换 `E` 的 plan”。
///
/// 说明：
/// - 该结构刻意只关心“可能出现 `E + ...` 的位置”（function effects / nominal eff arg）；
/// - `Option/tuple/union` 只是为了把 plan 延伸到更深的嵌套位置；
/// - plan 的 shape 以 `TypeId` 的 kind 为准（而非 `TypeRef`），避免 `Option<T>` 与 `T?` 的差异。
#[derive(Debug, Clone)]
pub(super) enum EffRowVarSubstPlan {
    None,
    Option {
        inner: Box<EffRowVarSubstPlan>,
    },
    Tuple {
        elements: Vec<EffRowVarSubstPlan>,
    },
    // 说明：当前阶段 union type 尚不出现在 type position 的 `TypeRef` 中（主要由 LUB 产生），
    // 但把该分支保留下来可以让后续扩展更直接。
    #[allow(dead_code)]
    Union {
        variants: Vec<EffRowVarSubstPlan>,
    },
    Function {
        receiver: Option<Box<EffRowVarSubstPlan>>,
        params: Vec<EffRowVarSubstPlan>,
        return_ty: Box<EffRowVarSubstPlan>,
        /// `Some(base)` 表示该 function type 的 effects row 直接引用了 `E`，需要用 `E_arg ∪ base` 替换。
        effects_base: Option<ast::EffectRowExpr>,
    },
    Nominal {
        args: Vec<EffRowVarSubstPlan>,
        /// `Some(base)` 表示该 nominal type 的 use-site `eff` 实参引用了 `E`，需要用 `E_arg ∪ base` 替换。
        eff_base: Option<ast::EffectRowExpr>,
    },
}

impl EffRowVarSubstPlan {
    /// 该 plan 是否包含任何“需要依赖 `E` 才能确定最终类型”的位置。
    pub(super) fn uses_eff_var(&self) -> bool {
        match self {
            EffRowVarSubstPlan::None => false,
            EffRowVarSubstPlan::Option { inner } => inner.uses_eff_var(),
            EffRowVarSubstPlan::Tuple { elements } => elements.iter().any(|p| p.uses_eff_var()),
            EffRowVarSubstPlan::Union { variants } => variants.iter().any(|p| p.uses_eff_var()),
            EffRowVarSubstPlan::Function {
                receiver,
                params,
                return_ty,
                effects_base,
            } => {
                effects_base.is_some()
                    || receiver.as_ref().is_some_and(|p| p.uses_eff_var())
                    || params.iter().any(|p| p.uses_eff_var())
                    || return_ty.uses_eff_var()
            }
            EffRowVarSubstPlan::Nominal { args, eff_base } => {
                eff_base.is_some() || args.iter().any(|p| p.uses_eff_var())
            }
        }
    }
}

fn is_eff_var_term(term: &ast::TypePath, eff_name: &str, source: &SourceFile) -> bool {
    term.segments.len() == 1 && term.args.is_empty() && term.segments[0].text(source) == eff_name
}

/// 若 `row` 引用了 `eff_name`，返回“去掉 `E` 后的 base row”（仍保持为 AST）。
fn effect_row_base_expr_excluding_eff_var(
    row: &ast::EffectRowExpr,
    eff_name: &str,
    source: &SourceFile,
) -> Option<ast::EffectRowExpr> {
    if row.terms.is_empty() {
        return None;
    }

    let mut used = false;
    let mut base_terms: Vec<ast::TypePath> = Vec::with_capacity(row.terms.len());
    for term in &row.terms {
        if is_eff_var_term(term, eff_name, source) {
            used = true;
            continue;
        }
        base_terms.push(term.clone());
    }

    if !used {
        return None;
    }

    Some(ast::EffectRowExpr {
        span: row.span,
        terms: base_terms,
        closed: row.closed,
    })
}

fn option_inner_type_ref(ty_ref: &ast::TypeRef) -> Option<&ast::TypeRef> {
    match ty_ref {
        ast::TypeRef::Nullable { inner, .. } => Some(inner.as_ref()),
        ast::TypeRef::Path(p) => p
            .args
            .iter()
            .find(|a| !matches!(a, ast::TypeRef::EffectRowArg { .. })),
        _ => None,
    }
}

/// 基于 “`TypeId` 的 shape + `TypeRef` 里出现的 row 语法” 构造替换 plan。
///
/// 注意：
/// - 该函数只在“当前文件内顶层函数”的签名收集阶段使用；
/// - 目前不尝试穿透 typealias 的语法表面（`TypeId` 已可能是展开后的类型），因此 plan 可能变为 `None`；
///   该行为与当前阶段的 TODO/fixtures 一致，后续若引入跨文件/跨 alias 的调用，可再补齐更精确的追踪。
pub(super) fn build_eff_row_var_subst_plan(
    ty_ref: &ast::TypeRef,
    ty: TypeId,
    eff_name: &str,
    source: &SourceFile,
    lower: &mut TypeLowering<'_>,
) -> Result<EffRowVarSubstPlan, ExprTypeError> {
    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            let Some(inner_ref) = option_inner_type_ref(ty_ref) else {
                return Ok(EffRowVarSubstPlan::None);
            };
            let inner_plan =
                build_eff_row_var_subst_plan(inner_ref, inner, eff_name, source, lower)?;
            if !inner_plan.uses_eff_var() {
                return Ok(EffRowVarSubstPlan::None);
            }
            Ok(EffRowVarSubstPlan::Option {
                inner: Box::new(inner_plan),
            })
        }
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
            let ast::TypeRef::Tuple(t) = ty_ref else {
                return Ok(EffRowVarSubstPlan::None);
            };
            if t.elements.len() != elements.len() {
                return Ok(EffRowVarSubstPlan::None);
            }
            let mut plans = Vec::with_capacity(elements.len());
            for (elem_ref, elem_ty) in t.elements.iter().zip(elements.iter().copied()) {
                plans.push(build_eff_row_var_subst_plan(
                    elem_ref, elem_ty, eff_name, source, lower,
                )?);
            }
            if !plans.iter().any(|p| p.uses_eff_var()) {
                return Ok(EffRowVarSubstPlan::None);
            }
            Ok(EffRowVarSubstPlan::Tuple { elements: plans })
        }
        TypeKind::Ref(RefTypeKind::Union(union)) => {
            // union 类型当前不会出现在 type position 的 TypeRef 中，但它可能出现在：
            // - 分支类型合并（LUB）结果
            // - 后续更复杂的推断/泛型实例化结果
            //
            // 由于 union 没有对应的 TypeRef 结构信息（无法判断哪些 row 来自 `E`），这里保守返回 None。
            let _ = union;
            Ok(EffRowVarSubstPlan::None)
        }
        TypeKind::Ref(RefTypeKind::Function(fun)) => {
            let ast::TypeRef::Function(f) = ty_ref else {
                return Ok(EffRowVarSubstPlan::None);
            };

            let effects_base = f
                .effects
                .as_ref()
                .and_then(|row| effect_row_base_expr_excluding_eff_var(row, eff_name, source));

            let receiver = match (&f.receiver, fun.receiver) {
                (Some(r_ref), Some(r_ty)) => Some(Box::new(build_eff_row_var_subst_plan(
                    r_ref, r_ty, eff_name, source, lower,
                )?)),
                _ => None,
            };

            if f.params.len() != fun.params.len() {
                return Ok(EffRowVarSubstPlan::None);
            }
            let mut params: Vec<EffRowVarSubstPlan> = Vec::with_capacity(fun.params.len());
            for (p_ref, p_ty) in f.params.iter().zip(fun.params.iter().copied()) {
                params.push(build_eff_row_var_subst_plan(
                    p_ref, p_ty, eff_name, source, lower,
                )?);
            }

            let return_ty = Box::new(build_eff_row_var_subst_plan(
                f.return_ty.as_ref(),
                fun.return_ty,
                eff_name,
                source,
                lower,
            )?);

            let plan = EffRowVarSubstPlan::Function {
                receiver,
                params,
                return_ty,
                effects_base,
            };
            if !plan.uses_eff_var() {
                return Ok(EffRowVarSubstPlan::None);
            }
            Ok(plan)
        }
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            let ast::TypeRef::Path(p) = ty_ref else {
                return Ok(EffRowVarSubstPlan::None);
            };

            // type args（不含 use-site eff arg）。
            let type_args = p
                .args
                .iter()
                .filter(|a| !matches!(a, ast::TypeRef::EffectRowArg { .. }))
                .collect::<Vec<_>>();
            if type_args.len() != nominal.args.len() {
                return Ok(EffRowVarSubstPlan::None);
            }

            let mut args: Vec<EffRowVarSubstPlan> = Vec::with_capacity(nominal.args.len());
            for (a_ref, a_ty) in type_args.iter().zip(nominal.args.iter().copied()) {
                args.push(build_eff_row_var_subst_plan(
                    a_ref, a_ty, eff_name, source, lower,
                )?);
            }

            let mut eff_base: Option<ast::EffectRowExpr> = None;
            for a in &p.args {
                let ast::TypeRef::EffectRowArg { row, .. } = a else {
                    continue;
                };
                eff_base = effect_row_base_expr_excluding_eff_var(row, eff_name, source);
                break;
            }

            let plan = EffRowVarSubstPlan::Nominal { args, eff_base };
            if !plan.uses_eff_var() {
                return Ok(EffRowVarSubstPlan::None);
            }
            Ok(plan)
        }
        // 其它类型不包含可替换的 effect row 信息。
        _ => Ok(EffRowVarSubstPlan::None),
    }
}

fn effect_row_union(a: &EffectRow, b: &EffectRow) -> EffectRow {
    if a.terms.is_empty() {
        return b.clone();
    }
    if b.terms.is_empty() {
        return a.clone();
    }
    let mut terms: Vec<TypeId> = Vec::with_capacity(a.terms.len() + b.terms.len());
    terms.extend(a.terms.iter().copied());
    terms.extend(b.terms.iter().copied());
    EffectRow::new(terms)
}

fn lower_effect_row_with_type_args(
    base: &ast::EffectRowExpr,
    type_params: &[TypeId],
    type_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
) -> Result<EffectRow, ExprTypeError> {
    if type_params.is_empty() || type_args.is_empty() {
        return Ok(lower.lower_effect_row_expr(Some(base))?);
    }

    let mut bindings: Vec<(String, TypeId)> = Vec::new();
    for (param_ty, arg_ty) in type_params.iter().copied().zip(type_args.iter().copied()) {
        let TypeKind::Param(p) = lower.type_kind(param_ty) else {
            continue;
        };
        bindings.push((p.name, arg_ty));
    }

    lower.push_type_param_bindings(bindings);
    let out = lower.lower_effect_row_expr(Some(base));
    lower.pop_type_param_bindings();
    Ok(out?)
}

/// 按 plan 把 `ty` 中出现的 “`E + base`” 替换为 “`E_arg + base`”。
///
/// 说明：
/// - `eff_arg` 是调用点推断/显式提供的 `E` 实参；
/// - `type_params/type_args` 用于在 base row 内部做类型实参替换（例如 `Raise<T>`）。
pub(super) fn apply_eff_row_var_subst_plan(
    ty: TypeId,
    plan: &EffRowVarSubstPlan,
    eff_arg: &EffectRow,
    type_params: &[TypeId],
    type_args: &[TypeId],
    lower: &mut TypeLowering<'_>,
    use_span: Span,
) -> Result<TypeId, ExprTypeError> {
    match plan {
        EffRowVarSubstPlan::None => Ok(ty),
        EffRowVarSubstPlan::Option { inner } => {
            let TypeKind::Value(ValueTypeKind::Option(inner_ty)) = lower.type_kind(ty) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "eff row substitution（Option plan mismatch）",
                    span: use_span.into(),
                });
            };
            let new_inner = apply_eff_row_var_subst_plan(
                inner_ty,
                inner.as_ref(),
                eff_arg,
                type_params,
                type_args,
                lower,
                use_span,
            )?;
            if new_inner == inner_ty {
                return Ok(ty);
            }
            Ok(lower.ty_option(new_inner))
        }
        EffRowVarSubstPlan::Tuple { elements } => {
            let TypeKind::Value(ValueTypeKind::Tuple(old)) = lower.type_kind(ty) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "eff row substitution（tuple plan mismatch）",
                    span: use_span.into(),
                });
            };
            if old.len() != elements.len() {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "eff row substitution（tuple arity mismatch）",
                    span: use_span.into(),
                });
            }
            let mut changed = false;
            let mut out: Vec<TypeId> = Vec::with_capacity(old.len());
            for ((elem_ty, elem_plan), idx) in old.into_iter().zip(elements.iter()).zip(0usize..) {
                let new_elem = apply_eff_row_var_subst_plan(
                    elem_ty,
                    elem_plan,
                    eff_arg,
                    type_params,
                    type_args,
                    lower,
                    use_span,
                )?;
                if new_elem != elem_ty {
                    changed = true;
                }
                out.push(new_elem);
                let _ = idx;
            }
            if !changed {
                return Ok(ty);
            }
            Ok(lower.ty_tuple(out))
        }
        EffRowVarSubstPlan::Union { variants } => {
            let TypeKind::Ref(RefTypeKind::Union(union)) = lower.type_kind(ty) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "eff row substitution（union plan mismatch）",
                    span: use_span.into(),
                });
            };
            if union.variants.len() != variants.len() {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "eff row substitution（union arity mismatch）",
                    span: use_span.into(),
                });
            }
            let mut changed = false;
            let mut out: Vec<TypeId> = Vec::with_capacity(union.variants.len());
            for (v_ty, v_plan) in union.variants.into_iter().zip(variants.iter()) {
                let new_v = apply_eff_row_var_subst_plan(
                    v_ty,
                    v_plan,
                    eff_arg,
                    type_params,
                    type_args,
                    lower,
                    use_span,
                )?;
                if new_v != v_ty {
                    changed = true;
                }
                out.push(new_v);
            }
            if !changed {
                return Ok(ty);
            }
            Ok(lower.ty_union(out))
        }
        EffRowVarSubstPlan::Function {
            receiver,
            params,
            return_ty,
            effects_base,
        } => {
            let TypeKind::Ref(RefTypeKind::Function(fun)) = lower.type_kind(ty) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "eff row substitution（function plan mismatch）",
                    span: use_span.into(),
                });
            };

            let mut changed = false;

            let new_receiver = match (fun.receiver, receiver.as_ref()) {
                (None, None) => None,
                (Some(r_ty), Some(r_plan)) => {
                    let new_r = apply_eff_row_var_subst_plan(
                        r_ty,
                        r_plan.as_ref(),
                        eff_arg,
                        type_params,
                        type_args,
                        lower,
                        use_span,
                    )?;
                    if new_r != r_ty {
                        changed = true;
                    }
                    Some(new_r)
                }
                _ => {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "eff row substitution（receiver plan mismatch）",
                        span: use_span.into(),
                    });
                }
            };

            if fun.params.len() != params.len() {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "eff row substitution（function param arity mismatch）",
                    span: use_span.into(),
                });
            }
            let mut new_params: Vec<TypeId> = Vec::with_capacity(fun.params.len());
            for (p_ty, p_plan) in fun.params.iter().copied().zip(params.iter()) {
                let new_p = apply_eff_row_var_subst_plan(
                    p_ty,
                    p_plan,
                    eff_arg,
                    type_params,
                    type_args,
                    lower,
                    use_span,
                )?;
                if new_p != p_ty {
                    changed = true;
                }
                new_params.push(new_p);
            }

            let new_return = apply_eff_row_var_subst_plan(
                fun.return_ty,
                return_ty.as_ref(),
                eff_arg,
                type_params,
                type_args,
                lower,
                use_span,
            )?;
            if new_return != fun.return_ty {
                changed = true;
            }

            let new_effects = if let Some(base_expr) = effects_base.as_ref() {
                let base =
                    lower_effect_row_with_type_args(base_expr, type_params, type_args, lower)?;
                let out = effect_row_union(eff_arg, &base);
                if out != fun.effects {
                    changed = true;
                }
                out
            } else {
                fun.effects
            };

            if !changed {
                return Ok(ty);
            }
            Ok(lower.ty_function(
                new_receiver,
                new_params,
                new_return,
                new_effects,
                fun.effects_closed,
            ))
        }
        EffRowVarSubstPlan::Nominal { args, eff_base } => match lower.type_kind(ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                if nominal.args.len() != args.len() {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "eff row substitution（nominal arg arity mismatch）",
                        span: use_span.into(),
                    });
                }
                let mut changed = false;

                let mut new_args: Vec<TypeId> = Vec::with_capacity(nominal.args.len());
                for (a_ty, a_plan) in nominal.args.iter().copied().zip(args.iter()) {
                    let new_a = apply_eff_row_var_subst_plan(
                        a_ty,
                        a_plan,
                        eff_arg,
                        type_params,
                        type_args,
                        lower,
                        use_span,
                    )?;
                    if new_a != a_ty {
                        changed = true;
                    }
                    new_args.push(new_a);
                }

                let new_eff = if let Some(base_expr) = eff_base.as_ref() {
                    let base =
                        lower_effect_row_with_type_args(base_expr, type_params, type_args, lower)?;
                    let out = effect_row_union(eff_arg, &base);
                    if nominal.eff.as_ref() != Some(&out) {
                        changed = true;
                    }
                    Some(out)
                } else {
                    nominal.eff
                };

                if !changed {
                    return Ok(ty);
                }
                Ok(lower.intern_type_kind(TypeKind::Ref(RefTypeKind::Nominal(
                    crate::ty::NominalType {
                        fqn: nominal.fqn,
                        args: new_args,
                        eff: new_eff,
                    },
                ))))
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if nominal.args.len() != args.len() {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "eff row substitution（nominal arg arity mismatch）",
                        span: use_span.into(),
                    });
                }
                let mut changed = false;

                let mut new_args: Vec<TypeId> = Vec::with_capacity(nominal.args.len());
                for (a_ty, a_plan) in nominal.args.iter().copied().zip(args.iter()) {
                    let new_a = apply_eff_row_var_subst_plan(
                        a_ty,
                        a_plan,
                        eff_arg,
                        type_params,
                        type_args,
                        lower,
                        use_span,
                    )?;
                    if new_a != a_ty {
                        changed = true;
                    }
                    new_args.push(new_a);
                }

                let new_eff = if let Some(base_expr) = eff_base.as_ref() {
                    let base =
                        lower_effect_row_with_type_args(base_expr, type_params, type_args, lower)?;
                    let out = effect_row_union(eff_arg, &base);
                    if nominal.eff.as_ref() != Some(&out) {
                        changed = true;
                    }
                    Some(out)
                } else {
                    nominal.eff
                };

                if !changed {
                    return Ok(ty);
                }
                Ok(
                    lower.intern_type_kind(TypeKind::Value(ValueTypeKind::Nominal(
                        crate::ty::NominalType {
                            fqn: nominal.fqn,
                            args: new_args,
                            eff: new_eff,
                        },
                    ))),
                )
            }
            _ => Err(ExprTypeError::UnsupportedExpr {
                kind: "eff row substitution（nominal plan mismatch）",
                span: use_span.into(),
            }),
        },
    }
}
