use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::float_literal::{FloatLiteralSuffix, parse_float_literal};
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::call::{
    GenericArgConstraint, InstantiatedFunSig, check_fn_value_to_any_erasure_gate,
    collect_type_arg_candidates_for_single_type_param, infer_call_expr_type,
    instantiate_fun_sig_for_call, substitute_single_type_param, type_param_name,
};
use super::member::{
    infer_elvis_expr_type, infer_member_access_expr_type, infer_not_null_assert_expr_type,
    infer_safe_member_access_expr_type, infer_splice_field_expr_type,
};
use super::ops::{
    infer_builtin_scalar_binary_expr_type, infer_operator_overload_binary_expr_type,
    infer_unary_expr_type, is_float_type, is_integer_type, literal_absorbs_to_expected,
};
use super::stmt::{
    StmtExprFlow, StmtExprShared, StmtExprState, check_local_val_decl_exprs, check_stmt_exprs,
    detect_smart_cast_for_if_condition,
};
use super::util::expr_kind_name;

use super::lower_type_ref_with_enum_subst;
use super::{
    ASYNC_EFFECT_FQN, EnumTypeSubstContext, ExprInferInputs, ExprTypeError, FunSigOwned, TASK_FQN,
};

use super::super::TypeSymbolKind;
use super::super::assignable::{is_type_assignable, nominal_is_subtype_by_fqn};
use super::super::branch_merge;
use super::super::eff_row_subst::EffRowVarSubstPlan;
use super::super::lower::TypeLowering;
use super::super::type_env::EnumVariantInfo;
use super::super::when_exhaustiveness;
use super::super::when_pat;

pub(super) fn infer_expr_type(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;

    match &expr.kind {
        ast::ExprKind::IntLit => Ok(builtins.int),
        ast::ExprKind::FloatLit => {
            let parsed = parse_float_literal(source.slice(expr.span));
            match parsed.suffix {
                FloatLiteralSuffix::Float64 => Ok(builtins.float64),
                FloatLiteralSuffix::Float32 => Ok(builtins.float32),
            }
        }
        ast::ExprKind::CharLit => Ok(builtins.char_),
        ast::ExprKind::StringLit | ast::ExprKind::InterpolatedString { .. } => Ok(builtins.string),
        ast::ExprKind::UnitLit => Ok(builtins.unit),
        ast::ExprKind::Block(b) => infer_block_value_type(inputs, b, lower),
        ast::ExprKind::DoBlock { body, .. } => infer_block_value_type(inputs, body, lower),
        ast::ExprKind::UnsafeBlock { body, .. } => {
            lower.push_unsafe_context();
            let result = infer_block_value_type(inputs, body, lower);
            lower.pop_unsafe_context();
            result
        }
        ast::ExprKind::SafeBlock { body, .. } => {
            lower.with_unsafe_context_suspended(|lower| infer_block_value_type(inputs, body, lower))
        }
        ast::ExprKind::TupleLit { elements } => {
            if elements.is_empty() {
                return Ok(builtins.unit);
            }

            let mut element_types = Vec::with_capacity(elements.len());
            for e in elements {
                element_types.push(inputs.infer(lower, e)?);
            }

            Ok(lower.ty_tuple(element_types))
        }
        ast::ExprKind::ArrayLit { elements } => {
            infer_array_lit_expr_type(inputs, expr, elements, None, None, lower)
        }
        ast::ExprKind::StructLit { ty, fields } => {
            infer_struct_lit_expr_type(inputs, expr, ty, fields, lower)
        }
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => infer_if_expr_type(
            inputs,
            cond.as_ref(),
            then_branch.as_ref(),
            else_branch.as_deref(),
            lower,
        ),
        ast::ExprKind::Ident(id) => infer_value_ident_type(
            source,
            id,
            lower,
            builtins,
            inputs.locals,
            inputs.top_level_types,
        ),
        ast::ExprKind::MemberAccess { receiver, member } => {
            infer_member_access_expr_type(inputs, receiver.as_ref(), member, lower)
        }
        ast::ExprKind::SpliceField { receiver, field } => {
            infer_splice_field_expr_type(inputs, receiver.as_ref(), field.as_ref(), lower)
        }
        ast::ExprKind::SafeMemberAccess {
            receiver, member, ..
        } => infer_safe_member_access_expr_type(inputs, receiver.as_ref(), member, lower),
        ast::ExprKind::NotNullAssert {
            expr: inner,
            op_span,
        } => infer_not_null_assert_expr_type(inputs, inner.as_ref(), *op_span, lower),
        ast::ExprKind::Call { callee, args } => {
            infer_call_expr_type(inputs, expr, callee, args, lower)
        }
        ast::ExprKind::Cast {
            expr: inner,
            op,
            op_span,
            ty,
        } => {
            let from_ty = inputs.infer(lower, inner)?;
            let target_ty = lower.lower_type_ref(ty)?;

            // spec §7.5：effects 是纯编译期信息，运行时不携带也无法验证；
            // 因此除 `(...)->R / Pure!` 外的函数值不允许擦除/转换为 `Any`（T0632）。
            check_fn_value_to_any_erasure_gate(from_ty, target_ty, *op_span, lower, builtins)?;

            if !is_cast_allowed(from_ty, target_ty, lower, builtins) {
                return Err(ExprTypeError::InvalidCast {
                    from: lower.fmt_type(from_ty),
                    to: lower.fmt_type(target_ty),
                    span: (*op_span).into(),
                });
            }

            match op {
                ast::CastOp::As => {
                    // T0445：`x as T` 的失败语义建模为 `Raise.raise(RuntimeError.ClassCastFailed)`，
                    // 因此在静态 required effects 层面要求 `Raise<RuntimeError>`（除非被 handle/try 捕获）。
                    let runtime_error = lower.lower_type_fqn_with_args(
                        "scoop.core.RuntimeError".to_string(),
                        Vec::new(),
                        *op_span,
                    )?;
                    let raise_runtime_error = lower.lower_type_fqn_with_args(
                        "scoop.core.Raise".to_string(),
                        vec![runtime_error],
                        *op_span,
                    )?;
                    lower.record_performed_effect(raise_runtime_error, *op_span);
                    Ok(target_ty)
                }
                ast::CastOp::AsQ => Ok(lower.ty_option(target_ty)),
            }
        }
        ast::ExprKind::Unary {
            op,
            op_span,
            expr: inner,
        } => infer_unary_expr_type(inputs, *op, *op_span, inner.as_ref(), lower),
        ast::ExprKind::TypeCheck {
            expr: inner, ty, ..
        } => {
            // `is`/`!is` 本身是一个表达式：结果类型为 `Bool`。
            //
            // 当前阶段只做最小检查：
            // - 确保被检查的表达式可推导类型（用于回归覆盖）；
            // - 确保目标类型引用可 lowering（否则应报 type lowering 错误）；
            // - 运行期语义与更强的类型关系约束留到后续阶段（PLAN §4.4 / TODO T0413+）。
            let _ = inputs.infer(lower, inner)?;
            let _ = lower.lower_type_ref(ty)?;
            Ok(builtins.bool_)
        }
        ast::ExprKind::When { subject, arms } => {
            // `when` 表达式结果类型：
            // - 递归类型检查 subject 与每个 arm body（保证覆盖其中的表达式）；
            // - 对所有 arm body 的类型做分支合并（T0514：LUB / 受限 union）；
            // - 若所有分支都是 `Nothing`（不可达），则整体结果为 `Nothing`。
            let subject_ty = inputs.infer(lower, subject)?;

            // 说明：这里必须遍历所有 arm（即使我们已经确定结果会是 `Any`），
            // 以保证：
            // - 分支 body 内的类型错误不会被“短路”吞掉；
            // - 后续的穷尽性检查始终生效。
            let mut result: Option<TypeId> = None;
            for arm in arms {
                // T0427：对 pattern 做最小类型约束，并把 binder 注入到该 arm 的局部环境中。
                let mut arm_locals: HashMap<Span, TypeId> = inputs.locals.clone();
                for (decl_span, ty) in when_pat::infer_when_pat_bindings(
                    source, &arm.pat, subject_ty, lower, builtins,
                )? {
                    arm_locals.insert(decl_span, ty);
                }
                let arm_inputs = inputs.with_locals(&arm_locals);

                // guard：需要在注入 binder 之后检查，这样 `Some(x) if x > 0` 才能在 guard 中引用 `x`。
                if let Some(guard) = &arm.guard {
                    let guard_ty = arm_inputs.infer(lower, guard)?;
                    if !is_type_assignable(guard_ty, builtins.bool_, lower, builtins) {
                        return Err(ExprTypeError::WhenGuardNotBool {
                            found: lower.fmt_type(guard_ty),
                            span: guard.span.into(),
                        });
                    }
                }

                let arm_ty = arm_inputs.infer(lower, &arm.body)?;

                // `Nothing`：不可达分支（例如后续 `Raise.raise`），不影响分支合并结果。
                if arm_ty == builtins.nothing {
                    continue;
                }

                match result {
                    None => result = Some(arm_ty),
                    Some(prev) => {
                        result = Some(branch_merge::merge_branch_result_type(
                            prev, arm_ty, lower, builtins,
                        ));
                    }
                }
            }

            when_exhaustiveness::check_when_exhaustiveness(
                source, expr, subject_ty, arms, lower, builtins,
            )?;

            // 若所有分支都是 `Nothing`，则 `when` 整体也是不可达的。
            Ok(result.unwrap_or(builtins.nothing))
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => infer_handle_expr_type(inputs, expr, body, arms, finally.as_ref(), lower),
        ast::ExprKind::Async { body } => infer_async_expr_type(inputs, expr, body, lower),
        ast::ExprKind::Spawn { body } => infer_spawn_expr_type(inputs, expr, body, lower),
        ast::ExprKind::Await {
            await_span: _,
            expr: inner,
        } => infer_await_expr_type(inputs, expr, inner, lower),
        ast::ExprKind::Join {
            join_span,
            expr: inner,
        } => infer_join_expr_type(inputs, expr, *join_span, inner, lower),
        ast::ExprKind::WithUpdate {
            base,
            updates,
            resolved_struct_fqns,
            ..
        } => infer_with_update_expr_type(inputs, base, updates, resolved_struct_fqns, lower),
        ast::ExprKind::Assign { lhs, rhs, .. } => infer_assign_expr_type(inputs, lhs, rhs, lower),
        ast::ExprKind::Binary {
            lhs,
            op,
            op_span,
            rhs,
        } => match op {
            ast::BinaryOp::Elvis => infer_elvis_expr_type(inputs, lhs, rhs, lower),
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr
            | ast::BinaryOp::Shl
            | ast::BinaryOp::Shr => infer_operator_overload_binary_expr_type(
                inputs,
                expr,
                lhs.as_ref(),
                *op,
                *op_span,
                rhs.as_ref(),
                lower,
            ),
            _ => infer_builtin_scalar_binary_expr_type(
                inputs,
                lhs.as_ref(),
                *op,
                *op_span,
                rhs.as_ref(),
                lower,
            ),
        },
        ast::ExprKind::Lambda(lam) => {
            if lower.in_const_context() {
                return Err(ExprTypeError::ConstFunLambdaNotAllowed {
                    span: expr.span.into(),
                });
            }

            // T0510：lambda 参数推断失败诊断（最小可读解释）。
            //
            // 说明：
            // - 当前实现只支持“期望函数类型向下传播”的 lambda 推断（T0504）；
            // - 当 lambda 出现在缺少 expected type 的位置（例如 `val f = { x -> x }`）时，
            //   我们给出更明确的错误，而不是笼统的 `unsupported_expr`。
            let Some(param) = lam.params.iter().find(|p| p.ty.is_none()) else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "lambda（当前仅支持在期望函数类型语境下推导）",
                    span: expr.span.into(),
                });
            };

            Err(ExprTypeError::LambdaParamTypeNotInferred {
                param: source.slice(param.name.span).to_string(),
                span: param.name.span.into(),
            })
        }
        ast::ExprKind::Missing => Err(ExprTypeError::UnsupportedExpr {
            kind: "missing",
            span: expr.span.into(),
        }),
        other => Err(ExprTypeError::UnsupportedExpr {
            kind: expr_kind_name(other),
            span: expr.span.into(),
        }),
    }
}

fn array_lit_element_ty_from_container(ty: TypeId, lower: &TypeLowering<'_>) -> Option<TypeId> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = lower.type_kind(ty) else {
        return None;
    };
    if nominal.args.len() != 1 {
        return None;
    }
    match nominal.fqn.as_str() {
        "scoop.core.Array"
        | "scoop.core.MutableArray"
        | "scoop.core.List"
        | "scoop.core.MutableList"
        | "scoop.collections.Set"
        | "scoop.collections.MapView"
        | "scoop.collections.MutableSet"
        | "scoop.collections.MutableMap" => nominal.args.first().copied(),
        _ => None,
    }
}

fn infer_array_lit_expr_type(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    elements: &[ast::Expr],
    expected_container_ty: Option<TypeId>,
    expected_from: Option<&ExpectedTypeFrom>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;

    if let Some(expected_ty) = expected_container_ty {
        let Some(element_ty) = array_lit_element_ty_from_container(expected_ty, lower) else {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "array literal",
                span: expr.span.into(),
            });
        };

        let expected_from_desc = expected_from
            .map(|from| from.desc.clone())
            .unwrap_or_else(|| "数组字面量的期望类型".to_string());
        let element_expected_from = ExpectedTypeFrom::new(format!(
            "数组字面量的元素类型（约束来源：{expected_from_desc}）"
        ));

        for (index, element) in elements.iter().enumerate() {
            let found_ty = inputs.infer_in_expected(
                lower,
                element,
                element_ty,
                element_expected_from.clone(),
            )?;

            if is_type_assignable(found_ty, element_ty, lower, builtins) {
                continue;
            }

            if literal_absorbs_to_expected(element, element_ty, source, lower, builtins) {
                continue;
            }

            return Err(ExprTypeError::ArrayLitElementTypeMismatch {
                index: index + 1,
                expected: lower.fmt_type(element_ty),
                found: lower.fmt_type(found_ty),
                span: element.span.into(),
            });
        }

        return Ok(expected_ty);
    }

    let Some(first_element) = elements.first() else {
        return Err(ExprTypeError::ArrayLitTypeAnnotationRequired {
            span: expr.span.into(),
        });
    };

    let mut inferred_elem_ty = inputs.infer(lower, first_element)?;
    let mut inferred_repr_expr = first_element;
    for (index, element) in elements.iter().enumerate().skip(1) {
        let found_ty = inputs.infer(lower, element)?;
        if found_ty == inferred_elem_ty {
            continue;
        }

        if literal_absorbs_to_expected(element, inferred_elem_ty, source, lower, builtins) {
            continue;
        }

        if literal_absorbs_to_expected(inferred_repr_expr, found_ty, source, lower, builtins) {
            inferred_elem_ty = found_ty;
            inferred_repr_expr = element;
            continue;
        }

        return Err(ExprTypeError::ArrayLitElementTypeMismatch {
            index: index + 1,
            expected: lower.fmt_type(inferred_elem_ty),
            found: lower.fmt_type(found_ty),
            span: element.span.into(),
        });
    }

    Ok(lower.lower_type_fqn_with_args(
        "scoop.core.Array".to_string(),
        vec![inferred_elem_ty],
        expr.span,
    )?)
}

/// 推导 `async { ... }` 的类型，并在 required-effects 收集上“捕获 Async”。
///
/// 当前阶段（T0619）最小规则：
/// - async body 的值类型等价于 block 的值类型；
/// - body 内发生的 `await` 会记录一次 `Async` performed effect；
/// - `async { ... }` 作为语法糖会捕获该 `Async`，因此该 effect 不向外层传播。
fn infer_async_expr_type(
    inputs: ExprInferInputs<'_>,
    async_expr: &ast::Expr,
    body: &ast::Block,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let async_effect = lower.lower_type_fqn_with_args(
        ASYNC_EFFECT_FQN.to_string(),
        Vec::new(),
        async_expr.span,
    )?;

    let (body_ty, body_performed) =
        lower.with_nested_effect_collection(|lower| infer_block_value_type(inputs, body, lower))?;

    // 捕获 async 语境内的 `Async` performed effect（其余 effects 正常向外传播）。
    for (effect, span) in body_performed {
        if effect == async_effect {
            continue;
        }
        lower.record_performed_effect(effect, span);
    }

    Ok(body_ty)
}

/// 推导 `spawn { ... }` 的类型，并把 `Async` 计入 required effects（T0620）。
///
/// 当前阶段（最小可回归落点）：
/// - `spawn` 被视为一次 `Async` performed effect（与规范中 desugar 到 `Async.spawn(...)` 对齐）；
/// - 先只支持 `spawn` body 的值类型为 `Int`，并返回一个 `Int` 句柄（后续由 `Task<T>` 替换）；
/// - 更完整的 `Task<T>` / generic spawn / 取消语义留给后续任务（T0622/T0917）。
fn infer_spawn_expr_type(
    inputs: ExprInferInputs<'_>,
    spawn_expr: &ast::Expr,
    body: &ast::Block,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let body_ty = infer_block_value_type(inputs, body, lower)?;

    let expected_ty = inputs.builtins.int;
    if !is_type_assignable(body_ty, expected_ty, lower, inputs.builtins) {
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: "spawn".to_string(),
            index: 1,
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(body_ty),
            span: body.span.into(),
        });
    }

    let async_effect = lower.lower_type_fqn_with_args(
        ASYNC_EFFECT_FQN.to_string(),
        Vec::new(),
        spawn_expr.span,
    )?;
    lower.record_performed_effect(async_effect, spawn_expr.span);

    // T0622：为 `spawn/await` 引入 `Task<T>` 的最小类型模型：
    // - 当前阶段仍只支持 `T = Int` 的可执行落点；
    // - `Task<T>` 的运行期语义（lazy/executor/取消）由后续 runtime 任务补齐（T0917）。
    Ok(lower.lower_type_fqn_with_args(TASK_FQN.to_string(), vec![expected_ty], spawn_expr.span)?)
}

fn task_inner_type(ty: TypeId, lower: &TypeLowering<'_>) -> Option<TypeId> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) | TypeKind::Value(ValueTypeKind::Nominal(n)) => {
            if n.fqn == TASK_FQN && n.args.len() == 1 {
                Some(n.args[0])
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 推导 `await expr` 的类型，并把 `Async` 计入 required effects。
///
/// 当前阶段（T0622）最小规则：
/// - `await` 只接受 `Task<T>`，并返回 `T`；
/// - `await` 视为一次 `Async` effect 的 perform 点；
/// - 运行期的 executor/跨线程 resume 语义留给后续任务（T0917+）。
fn infer_await_expr_type(
    inputs: ExprInferInputs<'_>,
    await_expr: &ast::Expr,
    inner: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let found_ty = inputs.infer(lower, inner)?;

    let Some(result_ty) = task_inner_type(found_ty, lower) else {
        let expected_task = lower.lower_type_fqn_with_args(
            TASK_FQN.to_string(),
            vec![inputs.builtins.any],
            await_expr.span,
        )?;
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: "await".to_string(),
            index: 1,
            expected: lower.fmt_type(expected_task),
            found: lower.fmt_type(found_ty),
            span: inner.span.into(),
        });
    };

    let async_effect = lower.lower_type_fqn_with_args(
        ASYNC_EFFECT_FQN.to_string(),
        Vec::new(),
        await_expr.span,
    )?;
    lower.record_performed_effect(async_effect, await_expr.span);
    Ok(result_ty)
}

/// 推导 `join expr` 的类型，并把 `Async` 计入 required effects（T0620）。
///
/// 当前阶段（最小可回归落点）：
/// - `join` 仅支持等待一个 `Task<T>` 并返回 `T`（当前最小可执行落点仍是 `T = Int`）；
/// - `join` 视为一次 `Async` performed effect（与 `await` 保持一致）。
fn infer_join_expr_type(
    inputs: ExprInferInputs<'_>,
    _join_expr: &ast::Expr,
    join_span: Span,
    inner: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let found_ty = inputs.infer(lower, inner)?;

    let Some(result_ty) = task_inner_type(found_ty, lower) else {
        let expected_task = lower.lower_type_fqn_with_args(
            TASK_FQN.to_string(),
            vec![inputs.builtins.any],
            join_span,
        )?;
        return Err(ExprTypeError::CallArgTypeMismatch {
            callee: "join".to_string(),
            index: 1,
            expected: lower.fmt_type(expected_task),
            found: lower.fmt_type(found_ty),
            span: inner.span.into(),
        });
    };

    let async_effect =
        lower.lower_type_fqn_with_args(ASYNC_EFFECT_FQN.to_string(), Vec::new(), join_span)?;
    lower.record_performed_effect(async_effect, join_span);

    Ok(result_ty)
}

/// 推导 `block` 作为表达式时的结果类型。
///
/// 说明：
/// - 该入口主要用于 `handle { ... }` 与 handler arm body 的类型检查（T0606）；
/// - 当前实现接受 runtime lowering 已覆盖的常规 statement：
///   - `val/var` / 普通表达式（包含 AST 形态的 `lhs = rhs` 赋值表达式）；
///   - `while` / `for`；
///   - `return` / `break` / `continue`（结果类型视为 `Nothing`）。
/// - `comptime` / `missing` 仍不属于这里的 runtime block-expression 子集。
fn infer_block_value_type(
    inputs: ExprInferInputs<'_>,
    block: &ast::Block,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    infer_block_value_type_with_expected(inputs, block, None, lower)
}

fn infer_block_value_type_in_expected_context(
    inputs: ExprInferInputs<'_>,
    block: &ast::Block,
    expected_ty: TypeId,
    expected_from: ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    infer_block_value_type_with_expected(inputs, block, Some((expected_ty, expected_from)), lower)
}

fn infer_block_value_type_with_expected(
    inputs: ExprInferInputs<'_>,
    block: &ast::Block,
    tail_expected: Option<(TypeId, ExpectedTypeFrom)>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里用“进入时克隆 + 本地更新”的方式实现最小作用域，不要求外层维护 stable/mutable 信息。
    let mut block_locals = inputs.locals.clone();
    let mut stable_bindings: HashSet<Span> = HashSet::new();
    let mut mutable_bindings: HashSet<Span> = HashSet::new();
    let empty_member_mutabilities: HashMap<String, bool> = HashMap::new();
    let shared = StmtExprShared {
        source: inputs.source,
        builtins: inputs.builtins,
        top_level_types: inputs.top_level_types,
        top_level_funs: inputs.top_level_funs,
        member_mutabilities: inputs
            .member_mutabilities
            .unwrap_or(&empty_member_mutabilities),
        struct_field_types: inputs.struct_field_types,
    };
    let flow = StmtExprFlow {
        loop_depth: inputs.loop_depth,
        expected_return_ty: inputs.expected_return_ty,
    };

    let mut tail_expr_ty: Option<TypeId> = None;
    let mut normal_completion_reachable = true;
    for (idx, stmt) in block.stmts.iter().enumerate() {
        let is_last = idx + 1 == block.stmts.len();

        match &stmt.kind {
            ast::StmtKind::Empty => {
                // no-op
            }
            ast::StmtKind::Val(v) => {
                let mut state = StmtExprState {
                    locals: &mut block_locals,
                    stable_bindings: &mut stable_bindings,
                    mutable_bindings: &mut mutable_bindings,
                };
                check_local_val_decl_exprs(shared, v, lower, &mut state, flow)?;
            }
            ast::StmtKind::Expr(e) => {
                let block_inputs = inputs.with_locals(&block_locals);
                let ty = if is_last {
                    if let Some((expected_ty, expected_from)) = tail_expected.clone() {
                        let found_ty =
                            block_inputs.infer_in_expected(lower, e, expected_ty, expected_from)?;
                        if is_type_assignable(found_ty, expected_ty, lower, inputs.builtins)
                            || literal_absorbs_to_expected(
                                e,
                                expected_ty,
                                inputs.source,
                                lower,
                                inputs.builtins,
                            )
                        {
                            expected_ty
                        } else {
                            found_ty
                        }
                    } else {
                        block_inputs.infer(lower, e)?
                    }
                } else {
                    block_inputs.infer(lower, e)?
                };
                if normal_completion_reachable && is_last {
                    tail_expr_ty = Some(ty);
                }
                if normal_completion_reachable && ty == inputs.builtins.nothing {
                    tail_expr_ty = Some(inputs.builtins.nothing);
                    normal_completion_reachable = false;
                }
            }
            ast::StmtKind::While { .. } | ast::StmtKind::For(_) => {
                let mut state = StmtExprState {
                    locals: &mut block_locals,
                    stable_bindings: &mut stable_bindings,
                    mutable_bindings: &mut mutable_bindings,
                };
                check_stmt_exprs(shared, stmt, lower, &mut state, flow)?;
                if normal_completion_reachable && is_last {
                    tail_expr_ty = Some(inputs.builtins.unit);
                }
            }
            ast::StmtKind::Return { .. }
            | ast::StmtKind::Break { .. }
            | ast::StmtKind::Continue { .. } => {
                let mut state = StmtExprState {
                    locals: &mut block_locals,
                    stable_bindings: &mut stable_bindings,
                    mutable_bindings: &mut mutable_bindings,
                };
                check_stmt_exprs(shared, stmt, lower, &mut state, flow)?;
                if normal_completion_reachable {
                    tail_expr_ty = Some(inputs.builtins.nothing);
                    normal_completion_reachable = false;
                }
            }
            ast::StmtKind::Missing => {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "block expression（missing stmt）",
                    span: stmt.span.into(),
                });
            }
            ast::StmtKind::ComptimeBlock { .. }
            | ast::StmtKind::ComptimeIf(_)
            | ast::StmtKind::ComptimeFor(_) => {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "block expression（comptime stmt）",
                    span: stmt.span.into(),
                });
            }
        }
    }

    Ok(tail_expr_ty.unwrap_or(inputs.builtins.unit))
}

/// 推导赋值表达式 `lhs = rhs` 的类型。
///
/// 说明：
/// - AST 中赋值以 `ExprKind::Assign` 承载，但在 HIR 中会降为 `StmtKind::Assign`；
/// - 在 `infer_expr_type` 这条“表达式语境”的入口里，我们缺少 `stable/mutable bindings`
///   信息（它只在 `check_expr_stmt` 的 statement 语境中维护），因此这里先实现最小可回归规则：
///   - lhs 仅允许标识符或成员访问；
///   - rhs 必须可赋给 lhs 的类型（复用 `is_type_assignable`）；
///   - 赋值表达式的结果类型为 `Unit`；
/// - 对“必须是 `var`”的可写性约束，当前阶段仅在 statement 语境（`check_assign_expr_stmt`）
///   中强制；等 `infer_expr_type` 也携带 stable/mutable 后再统一收敛。
fn infer_assign_expr_type(
    inputs: ExprInferInputs<'_>,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let expected_ty = match &lhs.kind {
        ast::ExprKind::Ident(id) => {
            let Some(resolved) = id.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（unresolved ident）",
                    span: id.span.into(),
                });
            };

            match resolved {
                ast::ResolvedValueRef::Local { name, decl_span } => {
                    inputs.locals.get(decl_span).copied().ok_or_else(|| {
                        ExprTypeError::UnknownLocalValueType {
                            name: name.clone(),
                            span: id.span.into(),
                        }
                    })?
                }
                ast::ResolvedValueRef::TopLevel { fqn } => {
                    inputs.top_level_types.get(fqn).copied().ok_or_else(|| {
                        ExprTypeError::UnsupportedTopLevelValueType {
                            fqn: fqn.clone(),
                            span: id.span.into(),
                        }
                    })?
                }
            }
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            // 先递归 typecheck receiver：保证 `a().b = rhs` 能覆盖 `a()`。
            //
            // 例外：`TypeName.member` 经 companion object 解析时，receiver 不是值表达式；
            // resolver 会保留 receiver ident 为未解析，此处跳过 receiver typecheck。
            let receiver_is_type_name =
                matches!(&receiver.kind, ast::ExprKind::Ident(id) if id.resolved.is_none());
            if !receiver_is_type_name {
                let _ = inputs.infer(lower, receiver)?;
            }

            let Some(resolved) = member.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（member 未 resolve）",
                    span: member.span.into(),
                });
            };

            let fqn = match resolved {
                ast::ResolvedMemberRef::Value { fqn } => fqn,
                ast::ResolvedMemberRef::Fun { fqn }
                | ast::ResolvedMemberRef::ExtensionValue { fqn }
                | ast::ResolvedMemberRef::ExtensionFun { fqn } => {
                    return Err(ExprTypeError::UnsupportedMemberAccess {
                        fqn: fqn.clone(),
                        span: member.span.into(),
                    });
                }
            };

            // 注意：这里不做 member 可写性检查（缺少 member_mutabilities 表）。
            // 若 fqn 不是字段/属性（例如 enum unit variant 值），这里会报 unsupported。
            inputs.struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })?
        }
        _ => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "assignment lhs（仅支持标识符或成员访问）",
                span: lhs.span.into(),
            });
        }
    };

    // 递归 typecheck rhs：保证 `x = f()` 这类表达式也会覆盖 rhs 中的表达式。
    let expected_from = match &lhs.kind {
        ast::ExprKind::Ident(id) => ExpectedTypeFrom::new(format!(
            "赋值目标 `{}` 的类型",
            inputs.source.slice(id.span)
        )),
        ast::ExprKind::MemberAccess { member, .. } => ExpectedTypeFrom::new(format!(
            "赋值目标 `{}` 的字段类型",
            inputs.source.slice(member.span)
        )),
        _ => ExpectedTypeFrom::new("赋值目标的类型"),
    };
    let found_ty = inputs.infer_in_expected(lower, rhs, expected_ty, expected_from)?;

    if !is_type_assignable(found_ty, expected_ty, lower, inputs.builtins)
        && !literal_absorbs_to_expected(rhs, expected_ty, inputs.source, lower, inputs.builtins)
    {
        return Err(ExprTypeError::AssignmentTypeMismatch {
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: rhs.span.into(),
        });
    }

    Ok(inputs.builtins.unit)
}

/// 推导 `handle { ... } with { ... }` 表达式的类型，并实现 required effects 的 handler 捕获（T0606/T2001）。
///
/// 当前阶段目标：
/// - 支持同一个 `handle` 中混用 non-resuming / immediate-resume / escape-continuation arms；
/// - handler arm head 只支持 effect operation（`Effect.op(...)`）；
/// - effect type param 的推断只支持单一 type param（例如 sysroot 的 `Raise<E>`）；
/// - `-> resume` arm 只负责恢复 handled computation，本身不直接决定 `handle` 表达式的结果类型；
/// - non-resuming / escape-continuation arm 的返回类型需要与 `handle` 的可返回结果保持一致；
/// - required effects：body 内 perform 的 effect 若被某个 arm 捕获，则不向外层传播。
pub(super) fn infer_handle_expr_type(
    inputs: ExprInferInputs<'_>,
    _handle_expr: &ast::Expr,
    body: &ast::Block,
    arms: &[ast::HandleArm],
    finally: Option<&ast::Block>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;

    #[derive(Debug, Clone)]
    struct HandleArmLowered {
        callee_fqn: String,
        handled_effect: TypeId,
        op_return_ty: TypeId,
        binder_tys: Vec<(Span, TypeId)>,
    }

    fn lower_handle_arm_effect_op_sig(
        source: &SourceFile,
        arm: &ast::HandleArm,
        body_performed_effects: &[(TypeId, Span)],
        lower: &mut TypeLowering<'_>,
        builtins: BuiltinTypes,
    ) -> Result<HandleArmLowered, ExprTypeError> {
        // 1) 解析 effect type 与 op FQN（例如 `scoop.core.Raise.raise`）。
        let effect_fqn = lower.resolve_type_path_fqn(&arm.op.effect)?;
        let op_name = arm.op.op.text(source);
        let callee_fqn = format!("{effect_fqn}.{op_name}");

        // 2) 查找该 member 是否为 effect operation。
        let op = lower.index().by_fqn.get(&callee_fqn).and_then(|syms| {
            syms.fun
                .iter()
                .find(|o| o.sig.kind == ast::FunDeclKind::EffectOp)
                .cloned()
        });
        let Some(op) = op else {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（callee is not an effect operation）",
                span: arm.op.op.span.into(),
            });
        };

        // 3) effect type 必须是 effect。
        let Some(effect_sym) = lower.env().type_symbol(&effect_fqn).cloned() else {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（missing effect type symbol）",
                span: arm.op.effect.span.into(),
            });
        };
        let ok = matches!(
            effect_sym.kind,
            TypeSymbolKind::Nominal(ast::TypeKind::Effect)
        );
        if !ok {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（qualifier is not an effect type）",
                span: arm.op.effect.span.into(),
            });
        }

        // 当前阶段（T0606）只支持单一 type param（与 effect op call 的限制保持一致）。
        if effect_sym.type_param_names.len() > 1 {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（multiple effect type params）",
                span: arm.op.effect.span.into(),
            });
        }
        if op.sig.receiver.is_some() {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（effect op receiver not supported）",
                span: arm.op.op.span.into(),
            });
        }

        // 4) 构造 effect op 的“可实例化签名”：其参数/返回类型允许引用：
        // - operation 自身的 type params（例如 `fun <T> await(task: Task<T>): T` 中的 `T`）
        // - effect type 的 type params（例如 `Raise<E>` 中的 `E`）
        //
        // 说明：当前 handler arm 的 v0 规则里：
        // - effect type args 仍需要被确定（用于 handled effect 实例与捕获匹配）；
        // - op type args 默认允许保持为“未实例化的 type params”（便于表达多态 handler）；
        //   但当 binder 提供了参数类型注解（`Effect.op(x: T, ...)`）时，允许用这些注解反推并实例化 op type args，
        //   以支持编写“只处理某个具体实例”的 handler（例如在 stdlib 中只处理 `Task<Int>` 的 `Async.await`）。
        let mut bindings: Vec<(String, TypeId)> = Vec::new();
        let mut op_type_params: Vec<TypeId> = Vec::new();
        for tp in &op.sig.type_params {
            let param_ty =
                lower.ty_param_named(tp.name.clone(), op.symbol.decl_file.clone(), tp.name_span);
            bindings.push((tp.name.clone(), param_ty));
            op_type_params.push(param_ty);
        }

        let mut type_params: Vec<TypeId> = Vec::new();
        if let Some(name) = effect_sym.type_param_names.first() {
            let param_ty =
                lower.ty_param_named(name.clone(), effect_sym.decl_file.clone(), effect_sym.span);
            type_params.push(param_ty);
            bindings.push((name.clone(), param_ty));
        }

        let mut param_names: Vec<String> = Vec::with_capacity(op.sig.params.len());
        let mut op_params: Vec<TypeId> = Vec::with_capacity(op.sig.params.len());
        for p in &op.sig.params {
            param_names.push(p.name.clone());

            let Some(ty_ref) = p.ty.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "handle arm（effect op param missing type）",
                    span: p.name_span.into(),
                });
            };

            let ty = lower.lower_type_ref_in_decl_file_with_bindings(
                &op.symbol.decl_file,
                bindings.clone(),
                ty_ref,
            )?;
            op_params.push(ty);
        }

        let op_return_ty = match &op.sig.return_ty {
            Some(ret) => lower.lower_type_ref_in_decl_file_with_bindings(
                &op.symbol.decl_file,
                bindings.clone(),
                ret,
            )?,
            None => builtins.unit,
        };

        let param_count = op_params.len();
        let sig = FunSigOwned {
            decl_span: op.symbol.span,
            decl_file: op.symbol.decl_file.clone(),
            is_extension: false,
            is_inline: false,
            is_const: false,
            is_unsafe: false,
            is_nogc: false,
            is_extern: false,
            is_intrinsic: false,
            param_names,
            param_has_defaults: vec![false; param_count],
            param_is_vararg: vec![false; param_count],
            type_params: type_params.clone(),
            eff_param: None,
            param_fn_effect_eff_base: vec![None; param_count],
            param_nominal_eff_eff_base: vec![None; param_count],
            param_eff_row_var_subst: vec![EffRowVarSubstPlan::None; param_count],
            return_eff_row_var_subst: EffRowVarSubstPlan::None,
            params: op_params,
            return_ty: op_return_ty,
            effects: None,
            where_constraints: Vec::new(),
        };

        // 5) 决定 effect type args：
        // - 优先使用 handler head 上的显式 type args（`Effect<T>.op(...)`）；
        // - 否则从 binder 的类型注解推断；
        // - 再否则尝试从 handle body 内的 performed effects 反推（仅当唯一候选时）。
        let explicit_args: Vec<TypeId> = arm
            .op
            .effect
            .args
            .iter()
            .map(|a| lower.lower_type_ref(a))
            .collect::<Result<Vec<_>, _>>()?;

        let type_args: Vec<TypeId> = if !explicit_args.is_empty() {
            explicit_args
        } else if type_params.is_empty() {
            Vec::new()
        } else {
            // 先尝试从 binder 的类型注解推断（try/catch lowering 会写回类型注解）。
            let mut constraints: Vec<GenericArgConstraint> = Vec::new();
            for (param_idx, binder) in arm.op.binders.iter().enumerate() {
                let Some(ty_ref) = binder.ty.as_ref() else {
                    continue;
                };
                let binder_ty = lower.lower_type_ref(ty_ref)?;
                constraints.push(GenericArgConstraint {
                    expected: sig.params.get(param_idx).copied().unwrap_or(builtins.unit),
                    found: binder_ty,
                    found_is_placeholder: false,
                    from: format!("handler arm 第 {} 个 binder", param_idx + 1),
                    span: binder.span,
                });
            }

            if !constraints.is_empty() {
                instantiate_fun_sig_for_call(
                    &callee_fqn,
                    arm.span,
                    &sig,
                    constraints,
                    lower,
                    builtins,
                )?
                .type_args
            } else {
                // 没有 binder 类型：尝试从 body 的 performed effects 推断（仅支持“唯一候选”）。
                let mut candidates: Vec<Vec<TypeId>> = Vec::new();
                for (effect, _) in body_performed_effects.iter().copied() {
                    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = lower.type_kind(effect)
                    else {
                        continue;
                    };
                    if nominal.fqn != effect_fqn {
                        continue;
                    }
                    candidates.push(nominal.args);
                }
                candidates.sort();
                candidates.dedup();

                if candidates.len() == 1 {
                    candidates.remove(0)
                } else {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: "handle arm（effect type args not inferred）",
                        span: arm.op.effect.span.into(),
                    });
                }
            }
        };

        // 6) 基于 type args 实例化 op 参数类型，并计算 handled effect 的实例类型。
        let mut instantiated = if !type_params.is_empty() && type_args.len() == type_params.len() {
            let mut params = sig.params.clone();
            let mut return_ty = sig.return_ty;
            for (param_ty, arg_ty) in type_params.iter().copied().zip(type_args.iter().copied()) {
                for p in &mut params {
                    *p = substitute_single_type_param(*p, param_ty, arg_ty, lower, arm.span)?;
                }
                return_ty =
                    substitute_single_type_param(return_ty, param_ty, arg_ty, lower, arm.span)?;
            }
            InstantiatedFunSig {
                params,
                return_ty,
                type_args,
            }
        } else {
            // 无 type params 或者推断失败：退回到未实例化的签名。
            InstantiatedFunSig {
                params: sig.params.clone(),
                return_ty: sig.return_ty,
                type_args,
            }
        };

        let handled_effect =
            lower.lower_type_fqn_with_args(effect_fqn, instantiated.type_args.clone(), arm.span)?;

        // 6b) 若 binder 提供了参数类型注解，尝试进一步实例化 **op 自身的** type params。
        //
        // 说明：
        // - handler arm head 语法不支持 `op<T>(...)` 的显式类型实参；因此这里只能通过 binder 的类型注解反推；
        // - 若无法从注解中推断出某个 op type param，则保留为未实例化的 type param（仍可表达多态 handler）。
        if !op_type_params.is_empty() {
            #[derive(Debug, Clone)]
            struct InferredTypeArgSource {
                from: String,
                span: Span,
            }

            let mut inferred: HashMap<TypeId, (TypeId, InferredTypeArgSource)> = HashMap::new();

            // 仅从 binder 的类型注解生成约束：未注解的 binder 仍视为多态。
            let mut constraints: Vec<GenericArgConstraint> = Vec::new();
            for (param_idx, binder) in arm.op.binders.iter().enumerate() {
                let Some(ty_ref) = binder.ty.as_ref() else {
                    continue;
                };
                let binder_ty = lower.lower_type_ref(ty_ref)?;
                constraints.push(GenericArgConstraint {
                    expected: instantiated
                        .params
                        .get(param_idx)
                        .copied()
                        .unwrap_or(builtins.unit),
                    found: binder_ty,
                    found_is_placeholder: false,
                    from: format!("handler arm 第 {} 个 binder", param_idx + 1),
                    span: binder.span,
                });
            }

            for c in constraints {
                for param_ty in op_type_params.iter().copied() {
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
                                callee: Box::new(callee_fqn.clone()),
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

            for param_ty in op_type_params.iter().copied() {
                let Some((arg_ty, _)) = inferred.get(&param_ty) else {
                    continue;
                };
                for p in &mut instantiated.params {
                    *p = substitute_single_type_param(*p, param_ty, *arg_ty, lower, arm.span)?;
                }
                instantiated.return_ty = substitute_single_type_param(
                    instantiated.return_ty,
                    param_ty,
                    *arg_ty,
                    lower,
                    arm.span,
                )?;
            }
        }

        // 7) binder 数量校验。
        if arm.op.binders.len() != instantiated.params.len() {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "handle arm（binder arity mismatch）",
                span: arm.op.span.into(),
            });
        }

        // 8) 校验 binder 类型注解（若存在），并计算 binder 在 arm body 内的类型。
        let mut binder_tys: Vec<(Span, TypeId)> = Vec::with_capacity(arm.op.binders.len());
        for (idx, binder) in arm.op.binders.iter().enumerate() {
            let expected = instantiated.params[idx];

            let binder_ty = match binder.ty.as_ref() {
                Some(ty_ref) => {
                    let binder_ty = lower.lower_type_ref(ty_ref)?;
                    if !is_type_assignable(expected, binder_ty, lower, builtins) {
                        return Err(ExprTypeError::CallArgTypeMismatch {
                            callee: callee_fqn.clone(),
                            index: idx + 1,
                            expected: lower.fmt_type(expected),
                            found: lower.fmt_type(binder_ty),
                            span: binder.span.into(),
                        });
                    }
                    binder_ty
                }
                None => expected,
            };
            binder_tys.push((binder.name.span, binder_ty));
            lower.record_inferred_binding_ty(binder.name.span, binder_ty);
        }

        Ok(HandleArmLowered {
            callee_fqn,
            handled_effect,
            op_return_ty: instantiated.return_ty,
            binder_tys,
        })
    }

    fn record_handle_return_path(
        result_ty: &mut Option<TypeId>,
        found: TypeId,
        lower: &mut TypeLowering<'_>,
        builtins: BuiltinTypes,
        span: Span,
    ) -> Result<(), ExprTypeError> {
        // `Nothing` 表示该路径不会正常返回，不应该把 handle 的结果类型锁死。
        if found == builtins.nothing {
            return Ok(());
        }

        match *result_ty {
            Some(expected) => {
                if is_type_assignable(found, expected, lower, builtins) {
                    return Ok(());
                }

                Err(ExprTypeError::HandleArmReturnTypeMismatch {
                    expected: lower.fmt_type(expected),
                    found: lower.fmt_type(found),
                    span: span.into(),
                })
            }
            None => {
                *result_ty = Some(found);
                Ok(())
            }
        }
    }

    // 1) 先在嵌套 effect collection 中 typecheck handle body，
    //    以便：
    //    - 推导 body 的结果类型（用于 handler arm 返回类型一致性检查）
    //    - 收集 performed effects，并在后续根据 handler arms 做过滤（实现 handler 捕获）
    let (body_ty, body_performed) =
        lower.with_nested_effect_collection(|lower| infer_block_value_type(inputs, body, lower))?;

    // 2) 处理 handler arms：lower effect op、计算 handled effect 实例、并 typecheck arm bodies。
    // `HandleArm` 的匹配顺序遵循源码中的书写顺序：
    // - 多个 arm 同时可匹配同一个 performed effect 时，选择最先出现的那个；
    // - 若某个 arm 的 handled effect 已被更早的 arm 覆盖，则该 arm 不可达（T0631）。
    let mut handled_effects: Vec<TypeId> = Vec::new();
    let mut seen_by_callee: HashMap<String, Vec<TypeId>> = HashMap::new();
    let mut seen_head_by_callee: HashMap<String, Vec<(String, TypeId)>> = HashMap::new();

    // handle 表达式的“期望结果类型”：
    // - 若 body 可正常返回（非 Nothing），则它给出 handle 的主结果类型；
    // - `-> resume` arm 仅恢复 body，不直接贡献结果类型；
    // - 若 body 为 Nothing，则由 non-resuming / escape-continuation arm 中第一个“可正常返回”的
    //   路径确定结果类型；返回 `Nothing` 的 arm 视为“不贡献结果类型”。
    let mut result_ty: Option<TypeId> = if body_ty != builtins.nothing {
        Some(body_ty)
    } else {
        None
    };

    for arm in arms {
        let lowered =
            lower_handle_arm_effect_op_sig(source, arm, &body_performed, lower, builtins)?;

        let seen = seen_by_callee
            .entry(lowered.callee_fqn.clone())
            .or_default();
        let seen_heads = seen_head_by_callee
            .entry(lowered.callee_fqn.clone())
            .or_default();
        let current_head = source.slice(arm.op.span).to_string();
        if let Some((_, prev)) = seen_heads
            .iter()
            .find(|(prev_head, _)| *prev_head == current_head)
        {
            return Err(ExprTypeError::HandleArmUnreachable {
                previous: lower.fmt_type(*prev),
                current: lower.fmt_type(lowered.handled_effect),
                span: arm.arrow_span.into(),
            });
        }
        let current_effect = lower.fmt_type(lowered.handled_effect);
        let mut shadowed_by: Option<TypeId> = None;
        for prev in seen.iter().copied() {
            if lower.fmt_type(prev) == current_effect
                || is_type_assignable(prev, lowered.handled_effect, lower, builtins)
            {
                shadowed_by = Some(prev);
                break;
            }
        }
        if let Some(prev) = shadowed_by {
            return Err(ExprTypeError::HandleArmUnreachable {
                previous: lower.fmt_type(prev),
                current: current_effect,
                span: arm.arrow_span.into(),
            });
        }
        seen_heads.push((current_head, lowered.handled_effect));
        seen.push(lowered.handled_effect);
        handled_effects.push(lowered.handled_effect);

        let mut arm_locals = inputs.locals.clone();
        for (decl_span, ty) in lowered.binder_tys.iter().copied() {
            arm_locals.insert(decl_span, ty);
        }

        match arm.kind {
            ast::HandleArmKind::ImmediateResume { resume_span } => {
                // `resume(value)`：注入一个局部函数值 `resume: (T) -> Nothing`。
                //
                // 说明：
                // - 当前阶段先用“局部函数值调用”的类型规则复用 call-check；
                // - `resume` 调用的控制流语义由 lowering/codegen（T0616）决定。
                let resume_fun_ty = lower.ty_function(
                    None,
                    vec![lowered.op_return_ty],
                    builtins.unit,
                    EffectRow::pure(),
                    false,
                );
                arm_locals.insert(resume_span, resume_fun_ty);

                // arm body：只要求可类型检查；不参与 handle 的结果类型推导。
                let arm_inputs = inputs.with_locals(&arm_locals);
                let _ = arm_inputs.infer(lower, &arm.body)?;
            }
            ast::HandleArmKind::EscapeContinuation { k_span } => {
                // `, k ->`：注入 continuation binder 的类型 `Continuation<T>`（T 为 op 返回类型）。
                //
                // 说明：
                // - 当前阶段 continuation 的 effect row 参数仍使用 sysroot 默认值（`Pure`）；
                // - `k.resume(value)` 的 required-effects 传播在 `Continuation.resume` 的内建规则中处理（spec §5.5）。
                let cont_ty = lower.lower_type_fqn_with_args(
                    "scoop.core.Continuation".to_string(),
                    vec![lowered.op_return_ty],
                    arm.span,
                )?;
                arm_locals.insert(k_span, cont_ty);
                let arm_inputs = inputs.with_locals(&arm_locals);

                // arm body 的类型必须与 handle 的可返回结果一致
                // （与 non-resuming 等价：perform 时 handle 立即返回 arm 值）。
                let arm_body_ty = match result_ty {
                    Some(expected) => arm_inputs.infer_in_expected(
                        lower,
                        &arm.body,
                        expected,
                        ExpectedTypeFrom::new("handle 表达式的期望结果类型"),
                    )?,
                    None => arm_inputs.infer(lower, &arm.body)?,
                };

                record_handle_return_path(
                    &mut result_ty,
                    arm_body_ty,
                    lower,
                    builtins,
                    arm.body.span,
                )?;
            }
            ast::HandleArmKind::NonResuming => {
                let arm_inputs = inputs.with_locals(&arm_locals);
                // arm body 的类型必须与 handle 的可返回结果一致（try/catch 等价语义）。
                let arm_body_ty = match result_ty {
                    Some(expected) => arm_inputs.infer_in_expected(
                        lower,
                        &arm.body,
                        expected,
                        ExpectedTypeFrom::new("handle 表达式的期望结果类型"),
                    )?,
                    None => arm_inputs.infer(lower, &arm.body)?,
                };

                record_handle_return_path(
                    &mut result_ty,
                    arm_body_ty,
                    lower,
                    builtins,
                    arm.body.span,
                )?;
            }
        }
    }

    // 3) finally block：当前阶段仅递归 typecheck（不参与结果类型），其 performed effects 向外传播。
    if let Some(finally) = finally {
        let _ = infer_block_value_type(inputs, finally, lower)?;
    }

    // 4) required effects：body 内 performed 的 effects 若被 handler 捕获，则不向外层传播。
    for (effect, span) in body_performed {
        // handler 捕获语义以“可赋值/子类型”为准：若某个 arm 的 handled effect
        // 可以匹配该 performed effect（handled <: performed），则该 effect 不向外传播。
        //
        // 说明：
        // - 对于 invariant effect（或 type args 为 value types 的场景），该关系会退化为全等；
        // - 对于带 `in/out` 的 effect type params，则按声明处变型规则参与匹配。
        let captured = handled_effects
            .iter()
            .copied()
            .any(|handled| is_type_assignable(handled, effect, lower, builtins));
        if captured {
            continue;
        }
        lower.record_performed_effect(effect, span);
    }

    Ok(result_ty.unwrap_or(inputs.builtins.nothing))
}

fn infer_if_expr_type(
    inputs: ExprInferInputs<'_>,
    cond: &ast::Expr,
    then_branch: &ast::Expr,
    else_branch: Option<&ast::Expr>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    // `if` 表达式结果类型：
    // - 递归类型检查 cond / then / else（保证覆盖内部表达式）；
    // - then/else 通过 T0514 分支合并规则计算结果类型；
    // - 没有 else 时视为 `Unit`（更接近语句形式）。

    // 先 typecheck cond：保证其中的表达式也会被覆盖（错误不应被吞掉）。
    let _ = inputs.infer(lower, cond)?;

    // smart cast（T0413）的表达式语境版本（最小实现）：
    // - 与 `check_if_expr_stmt` 保持一致的语义：识别 `if (x is T)` / `if (x !is T)`；
    // - 由于 `infer_expr_type` 当前不携带 stable/mutable bindings 信息，这里采用保守近似：
    //   把当前 `locals` 中出现的绑定视为“可收窄”候选。
    let stable_bindings: HashSet<Span> = inputs.locals.keys().copied().collect();
    let smart_cast =
        detect_smart_cast_for_if_condition(cond, lower, inputs.locals, &stable_bindings)?;

    let mut then_locals = inputs.locals.clone();
    if let Some(sc) = smart_cast
        && sc.narrow_in_then
    {
        then_locals.insert(sc.decl_span, sc.target_ty);
    }
    let then_ty = inputs.with_locals(&then_locals).infer(lower, then_branch)?;

    let Some(else_branch) = else_branch else {
        // `if` 没有 else：语义上更接近“语句形式”，结果类型视为 `Unit`。
        // 仍然需要确保 then branch 内的表达式被覆盖（见上方 `then_ty`）。
        return Ok(inputs.builtins.unit);
    };

    let mut else_locals = inputs.locals.clone();
    if let Some(sc) = smart_cast
        && !sc.narrow_in_then
    {
        else_locals.insert(sc.decl_span, sc.target_ty);
    }
    let else_ty = inputs.with_locals(&else_locals).infer(lower, else_branch)?;

    Ok(branch_merge::merge_branch_result_type(
        then_ty,
        else_ty,
        lower,
        inputs.builtins,
    ))
}

/// “期望类型”的来源说明（用于推断失败诊断）。
///
/// 说明：
/// - 该信息会被拼进错误信息的 `Display` 文本，便于 fixtures 用 `EXPECT-ERROR` 做子串断言；
/// - 目前只要求“最小可读解释”，不追求穷尽的来源链路（TODO：后续可扩展为来源栈）。
#[derive(Debug, Clone)]
pub(super) struct ExpectedTypeFrom {
    desc: String,
}

impl ExpectedTypeFrom {
    pub(super) fn new(desc: impl Into<String>) -> Self {
        Self { desc: desc.into() }
    }
}

fn expr_matches_expected_type(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    expected_ty: TypeId,
    expected_from: ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
) -> Result<bool, ExprTypeError> {
    let found_ty = inputs.infer_in_expected(lower, expr, expected_ty, expected_from)?;
    Ok(
        is_type_assignable(found_ty, expected_ty, lower, inputs.builtins)
            || literal_absorbs_to_expected(
                expr,
                expected_ty,
                inputs.source,
                lower,
                inputs.builtins,
            ),
    )
}

fn try_infer_numeric_unary_expr_type_by_expected(
    inputs: ExprInferInputs<'_>,
    op: ast::UnaryOp,
    inner: &ast::Expr,
    expected_ty: TypeId,
    expected_from: &ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let accepts_same_numeric = match op {
        ast::UnaryOp::Neg => {
            is_integer_type(expected_ty, lower, inputs.builtins)
                || is_float_type(expected_ty, lower, inputs.builtins)
        }
        ast::UnaryOp::BitNot => is_integer_type(expected_ty, lower, inputs.builtins),
        ast::UnaryOp::Not => false,
    };
    if !accepts_same_numeric {
        return Ok(None);
    }

    let operand_matches = expr_matches_expected_type(
        inputs,
        inner,
        expected_ty,
        ExpectedTypeFrom::new(format!(
            "一元运算操作数（约束来源：{}）",
            expected_from.desc
        )),
        lower,
    )?;

    if operand_matches {
        Ok(Some(expected_ty))
    } else {
        Ok(None)
    }
}

fn try_infer_numeric_binary_expr_type_by_expected(
    inputs: ExprInferInputs<'_>,
    op: ast::BinaryOp,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    expected_ty: TypeId,
    expected_from: &ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    if is_integer_type(expected_ty, lower, inputs.builtins) {
        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => {
                let lhs_matches = expr_matches_expected_type(
                    inputs,
                    lhs,
                    expected_ty,
                    ExpectedTypeFrom::new(format!(
                        "二元运算左操作数（约束来源：{}）",
                        expected_from.desc
                    )),
                    lower,
                )?;
                let rhs_matches = expr_matches_expected_type(
                    inputs,
                    rhs,
                    expected_ty,
                    ExpectedTypeFrom::new(format!(
                        "二元运算右操作数（约束来源：{}）",
                        expected_from.desc
                    )),
                    lower,
                )?;
                if lhs_matches && rhs_matches {
                    return Ok(Some(expected_ty));
                }
            }
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                let lhs_matches = expr_matches_expected_type(
                    inputs,
                    lhs,
                    expected_ty,
                    ExpectedTypeFrom::new(format!(
                        "移位左操作数（约束来源：{}）",
                        expected_from.desc
                    )),
                    lower,
                )?;
                let rhs_matches = expr_matches_expected_type(
                    inputs,
                    rhs,
                    inputs.builtins.int,
                    ExpectedTypeFrom::new(format!(
                        "移位右操作数（约束来源：{}）",
                        expected_from.desc
                    )),
                    lower,
                )?;
                if lhs_matches && rhs_matches {
                    return Ok(Some(expected_ty));
                }
            }
            _ => {}
        }
    }

    if is_float_type(expected_ty, lower, inputs.builtins)
        && matches!(
            op,
            ast::BinaryOp::Add
                | ast::BinaryOp::Sub
                | ast::BinaryOp::Mul
                | ast::BinaryOp::Div
                | ast::BinaryOp::Rem
        )
    {
        let lhs_matches = expr_matches_expected_type(
            inputs,
            lhs,
            expected_ty,
            ExpectedTypeFrom::new(format!(
                "浮点二元运算左操作数（约束来源：{}）",
                expected_from.desc
            )),
            lower,
        )?;
        let rhs_matches = expr_matches_expected_type(
            inputs,
            rhs,
            expected_ty,
            ExpectedTypeFrom::new(format!(
                "浮点二元运算右操作数（约束来源：{}）",
                expected_from.desc
            )),
            lower,
        )?;
        if lhs_matches && rhs_matches {
            return Ok(Some(expected_ty));
        }
    }

    Ok(None)
}

/// 在“存在明确期望类型”的语境下推导表达式类型。
///
/// 当前该入口会优先尝试把 expected type 向下传播到：
/// - block / `@Unsafe` / `@Safe` 的 tail expr；
/// - `if` / `when` 等控制流表达式；
/// - 数值一元/二元运算、数组字面量、lambda 等已支持 expected-type 吸收的节点；
/// - 同名 enum variant ctor 的期望类型消歧。
///
/// 若某个节点当前无法从 expected type 中获益，则回退到常规推导路径，
/// 保持既有诊断行为稳定。
pub(super) fn infer_expr_type_in_expected_context(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    expected_ty: TypeId,
    expected_from: ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;

    if matches!(expr.kind, ast::ExprKind::IntLit | ast::ExprKind::FloatLit)
        && literal_absorbs_to_expected(expr, expected_ty, source, lower, builtins)
    {
        return Ok(expected_ty);
    }

    match &expr.kind {
        ast::ExprKind::Block(block) | ast::ExprKind::DoBlock { body: block, .. } => {
            return infer_block_value_type_in_expected_context(
                inputs,
                block,
                expected_ty,
                expected_from,
                lower,
            );
        }
        ast::ExprKind::UnsafeBlock { body, .. } => {
            lower.push_unsafe_context();
            let result = infer_block_value_type_in_expected_context(
                inputs,
                body,
                expected_ty,
                expected_from,
                lower,
            );
            lower.pop_unsafe_context();
            return result;
        }
        ast::ExprKind::SafeBlock { body, .. } => {
            return lower.with_unsafe_context_suspended(|lower| {
                infer_block_value_type_in_expected_context(
                    inputs,
                    body,
                    expected_ty,
                    expected_from,
                    lower,
                )
            });
        }
        _ => {}
    }

    // T0504：lambda 参数类型下推（spec §14.7.2）。
    //
    // 说明：lambda 的参数类型通常由“期望的函数类型”向下传播而来，因此这里在存在 expected type 时
    // 优先尝试用该信息推断 lambda 的参数类型与返回类型。
    if let ast::ExprKind::Lambda(lam) = &expr.kind {
        if lower.in_const_context() {
            return Err(ExprTypeError::ConstFunLambdaNotAllowed {
                span: expr.span.into(),
            });
        }
        if let Some(ty) =
            try_infer_lambda_expr_type_by_expected(inputs, expr, lam, expected_ty, lower)?
        {
            return Ok(ty);
        }
    }

    if let ast::ExprKind::Unary {
        op, expr: inner, ..
    } = &expr.kind
        && let Some(ty) = try_infer_numeric_unary_expr_type_by_expected(
            inputs,
            *op,
            inner.as_ref(),
            expected_ty,
            &expected_from,
            lower,
        )?
    {
        return Ok(ty);
    }

    if let ast::ExprKind::Binary { lhs, op, rhs, .. } = &expr.kind
        && let Some(ty) = try_infer_numeric_binary_expr_type_by_expected(
            inputs,
            *op,
            lhs.as_ref(),
            rhs.as_ref(),
            expected_ty,
            &expected_from,
            lower,
        )?
    {
        return Ok(ty);
    }

    if let ast::ExprKind::ArrayLit { elements } = &expr.kind {
        if array_lit_element_ty_from_container(expected_ty, lower).is_some() {
            return infer_array_lit_expr_type(
                inputs,
                expr,
                elements,
                Some(expected_ty),
                Some(&expected_from),
                lower,
            );
        }

        return inputs.infer(lower, expr);
    }

    if let ast::ExprKind::When { subject, arms } = &expr.kind {
        return infer_when_expr_type_in_expected_context(
            inputs,
            expr,
            subject,
            arms,
            expected_ty,
            &expected_from,
            lower,
        );
    }

    // T0510：分支类型不一致时，把推断失败精确映射到具体分支表达式。
    //
    // 说明：
    // - 当前 `if` 表达式的“无 expected type”结果类型推导仍采用 T0503 的最小规则（相同类型否则 Any fallback）；
    // - 但当 `if` 处于“存在明确 expected type”的语境下时，我们可以直接对每个分支做可赋值检查，
    //   并把错误定位到具体分支，而不是让它先退化为 `Any` 再在外层报一个模糊的 mismatch。
    if let ast::ExprKind::If {
        cond,
        then_branch,
        else_branch,
    } = &expr.kind
    {
        let expected_from_desc = expected_from.desc.clone();

        // 先覆盖 cond（不在此处强制 Bool 规则；相关诊断留给控制流/语句层）。
        let _ = inputs.infer(lower, cond.as_ref())?;

        let then_ty = inputs.infer_in_expected(
            lower,
            then_branch.as_ref(),
            expected_ty,
            expected_from.clone(),
        )?;

        let then_matches_expected = is_type_assignable(then_ty, expected_ty, lower, builtins)
            || literal_absorbs_to_expected(
                then_branch.as_ref(),
                expected_ty,
                source,
                lower,
                builtins,
            );
        if !then_matches_expected {
            return Err(ExprTypeError::IfBranchTypeMismatch {
                branch: "then",
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(then_ty),
                expected_from: expected_from_desc.clone(),
                span: then_branch.span.into(),
            });
        }

        let Some(else_branch) = else_branch.as_deref() else {
            return Ok(builtins.unit);
        };

        let else_ty = inputs.infer_in_expected(lower, else_branch, expected_ty, expected_from)?;

        let else_matches_expected = is_type_assignable(else_ty, expected_ty, lower, builtins)
            || literal_absorbs_to_expected(else_branch, expected_ty, source, lower, builtins);
        if !else_matches_expected {
            return Err(ExprTypeError::IfBranchTypeMismatch {
                branch: "else",
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(else_ty),
                expected_from: expected_from_desc,
                span: else_branch.span.into(),
            });
        }

        // 两个分支都可赋值给 expected type：直接把整个 `if` 视为该 expected type。
        return Ok(expected_ty);
    }

    if let ast::ExprKind::Call { callee, args } = &expr.kind
        && let ast::ExprKind::Ident(id) = &callee.kind
        && id.resolved.is_none()
        && let Some(ty) = try_infer_ambiguous_enum_variant_ctor_call_expr_type_by_expected(
            inputs,
            expr,
            id,
            args,
            expected_ty,
            lower,
        )?
    {
        return Ok(ty);
    }

    // T0124: When a struct literal's type path has no type args but the expected type is a
    // generic instantiation of the same struct, use the expected type to drive inference.
    if let ast::ExprKind::StructLit { ty, fields } = &expr.kind
        && ty.args.is_empty()
        && let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(expected_ty)
        && !nominal.args.is_empty()
    {
        let local_name = source.slice(ty.segments.last().map(|s| s.span).unwrap_or(ty.span));
        let fqn_matches = nominal.fqn.ends_with(local_name)
            && (nominal.fqn.len() == local_name.len()
                || nominal
                    .fqn
                    .as_bytes()
                    .get(nominal.fqn.len() - local_name.len() - 1)
                    == Some(&b'.'));
        if fqn_matches {
            return infer_generic_struct_lit_expr_type(inputs, expr, expected_ty, fields, lower);
        }
    }

    inputs.infer(lower, expr)
}

fn infer_when_expr_type_in_expected_context(
    inputs: ExprInferInputs<'_>,
    expr: &ast::Expr,
    subject: &ast::Expr,
    arms: &[ast::WhenArm],
    expected_ty: TypeId,
    expected_from: &ExpectedTypeFrom,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let subject_ty = inputs.infer(lower, subject)?;

    let arm_expected_from = ExpectedTypeFrom::new(format!(
        "when 分支结果类型（约束来源：{}）",
        expected_from.desc
    ));

    let mut result: Option<TypeId> = None;
    let mut all_match_expected = true;

    for arm in arms {
        let mut arm_locals: HashMap<Span, TypeId> = inputs.locals.clone();
        for (decl_span, ty) in
            when_pat::infer_when_pat_bindings(source, &arm.pat, subject_ty, lower, builtins)?
        {
            arm_locals.insert(decl_span, ty);
        }
        let arm_inputs = inputs.with_locals(&arm_locals);

        if let Some(guard) = &arm.guard {
            let guard_ty = arm_inputs.infer(lower, guard)?;
            if !is_type_assignable(guard_ty, builtins.bool_, lower, builtins) {
                return Err(ExprTypeError::WhenGuardNotBool {
                    found: lower.fmt_type(guard_ty),
                    span: guard.span.into(),
                });
            }
        }

        let arm_ty = arm_inputs.infer_in_expected(
            lower,
            &arm.body,
            expected_ty,
            arm_expected_from.clone(),
        )?;
        let arm_matches_expected = arm_ty == builtins.nothing
            || is_type_assignable(arm_ty, expected_ty, lower, builtins)
            || literal_absorbs_to_expected(&arm.body, expected_ty, source, lower, builtins);
        if !arm_matches_expected {
            all_match_expected = false;
        }

        if arm_ty == builtins.nothing {
            continue;
        }

        match result {
            None => result = Some(arm_ty),
            Some(prev) => {
                result = Some(branch_merge::merge_branch_result_type(
                    prev, arm_ty, lower, builtins,
                ));
            }
        }
    }

    when_exhaustiveness::check_when_exhaustiveness(
        source, expr, subject_ty, arms, lower, builtins,
    )?;

    if all_match_expected {
        Ok(expected_ty)
    } else {
        Ok(result.unwrap_or(builtins.nothing))
    }
}

fn try_infer_lambda_expr_type_by_expected(
    inputs: ExprInferInputs<'_>,
    lam_expr: &ast::Expr,
    lam: &ast::LambdaExpr,
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let TypeKind::Ref(RefTypeKind::Function(expected_fun)) = lower.type_kind(expected_ty) else {
        return Ok(None);
    };

    // 当前阶段目标（T0504/T0509）：
    // - 支持 0/1/2 参数 lambda（`() -> T` / `(A) -> T` / `(A, B) -> T`）
    // - 支持 receiver function type（`T.() -> R`）：把 receiver 写入 lambda 的函数类型；
    //   注意：当前阶段 resolver 尚未为 lambda body 引入 `this` 绑定，因此这里不额外注入局部 `this`。

    let mut lambda_locals = inputs.locals.clone();
    let mut param_tys: Vec<TypeId> = Vec::new();
    let kind_param_count_limit = "lambda（当前仅支持 0/1/2 参数，且参数类型需来自期望函数类型）";

    match expected_fun.params.len() {
        0 => {
            if !lam.params.is_empty() {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: kind_param_count_limit,
                    span: lam_expr.span.into(),
                });
            }
        }
        1 => {
            let expected_param_ty = expected_fun.params[0];

            // Kotlin-like：`{ body }` 形式的 lambda 在期望函数类型为 `(T) -> R` 时，
            // 允许省略形参列表，并隐式引入单参数 `it: T`（T1307a）。
            if lam.params.is_empty() && lam.arrow_span.is_none() {
                let implicit_it_decl_span = Span::new(lam_expr.span.start, lam_expr.span.start);
                lambda_locals.insert(implicit_it_decl_span, expected_param_ty);
                param_tys.push(expected_param_ty);
            } else {
                if lam.params.len() != 1 {
                    return Err(ExprTypeError::UnsupportedExpr {
                        kind: kind_param_count_limit,
                        span: lam_expr.span.into(),
                    });
                }

                let param = &lam.params[0];
                let param_ty = match &param.ty {
                    Some(ty_ref) => lower.lower_type_ref(ty_ref)?,
                    None => expected_param_ty,
                };
                lambda_locals.insert(param.name.span, param_ty);
                param_tys.push(param_ty);
            }
        }
        2 => {
            if lam.params.len() != 2 {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: kind_param_count_limit,
                    span: lam_expr.span.into(),
                });
            }

            for (idx, param) in lam.params.iter().enumerate() {
                let expected_param_ty = expected_fun
                    .params
                    .get(idx)
                    .copied()
                    .expect("expected_fun.params.len() == 2");
                let param_ty = match &param.ty {
                    Some(ty_ref) => lower.lower_type_ref(ty_ref)?,
                    None => expected_param_ty,
                };
                lambda_locals.insert(param.name.span, param_ty);
                param_tys.push(param_ty);
            }
        }
        _ => return Ok(None),
    }

    // 返回类型推导（最小）：以 body 表达式的类型为 lambda 返回类型。
    // 当前阶段不做“expected return type 向下传播”（避免引入多段推断链）。
    let lambda_inputs = inputs.with_locals(&lambda_locals);
    let (body_ty, performed_effects) = lower
        .with_nested_effect_collection(|lower| lambda_inputs.infer(lower, lam.body.as_ref()))?;

    let effects = EffectRow::new(
        performed_effects
            .into_iter()
            .map(|(effect, _)| effect)
            .collect(),
    );
    // lambda 本身没有 `/ R!` 的语法标注，因此这里默认视为 open row（`closed=false`）；
    // 若用户需要把 lambda 擦除到 `Any`，必须通过显式类型注解得到 `(...)->R / Pure!`（见 T0632）。
    let lam_ty = lower.ty_function(expected_fun.receiver, param_tys, body_ty, effects, false);
    Ok(Some(lam_ty))
}

fn try_infer_ambiguous_enum_variant_ctor_call_expr_type_by_expected(
    inputs: ExprInferInputs<'_>,
    call_expr: &ast::Expr,
    callee: &ast::ValueIdent,
    args: &[ast::Expr],
    expected_ty: TypeId,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let variant_name = source.slice(callee.span);
    let candidates = lower.env().find_enum_variants_named(variant_name);

    // 说明：
    // - 当 callee ident 未被 resolver 绑定（`resolved == None`）时，我们会尝试把 `Foo(...)` 视为
    //   enum variant ctor（T0426）。
    // - 若该 call 处于“有 expected type”的语境下（例如 `val x: Option<Int> = Some(1)`），
    //   我们优先用 expected enum type 来确定“它应该构造哪个 enum 的 variant”。
    //
    // 这同时覆盖两类场景：
    // - 同名 variant 跨多个 enum 存在（需要按 expected type 消歧）；
    // - variant 名在全局唯一（仍然可以用 expected type 把字段参数的 expected type 下推到子表达式，
    //   例如 `Some(None())` 里的 `None()` 需要 `Option<T>` 的 expected type 才能推断）。
    if candidates.is_empty() {
        return Ok(None);
    }

    let Some((expected_enum_fqn, expected_enum_args)) =
        enum_instance_fqn_and_args_from_type(expected_ty, lower)
    else {
        return Ok(None);
    };

    // 期望类型指向一个明确的 enum：从同名候选中选出“该 enum 的 variant”。
    let mut matched: Vec<(String, EnumVariantInfo)> = candidates
        .into_iter()
        .filter(|(enum_fqn, _)| enum_fqn == &expected_enum_fqn)
        .collect();
    if matched.len() != 1 {
        return Ok(None);
    }
    let (enum_fqn, variant) = matched.pop().expect("len == 1");

    let Some((type_params, enum_source)) = lower.env().enum_decl(&enum_fqn).map(|d| {
        let type_params = d.type_params.clone();
        let source = lower
            .env()
            .source(&d.decl_file)
            .cloned()
            .unwrap_or_else(|| source.clone());
        (type_params, source)
    }) else {
        // 防御性：`matched` 来源于 `TypeEnv.enums`，理论上一定存在。
        return Ok(None);
    };

    if type_params.len() != expected_enum_args.len() {
        // 期望类型与 enum 声明的 arity 不一致时，交给常规推导路径处理并给出诊断。
        return Ok(None);
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
        .zip(expected_enum_args)
        .collect();

    for (idx, (field, arg_expr)) in variant.fields.iter().zip(args.iter()).enumerate() {
        let expected_field_ty = lower_type_ref_with_enum_subst(
            EnumTypeSubstContext {
                enum_source: &enum_source,
                use_span: call_expr.span,
                enum_fqn: &enum_fqn,
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

    Ok(Some(expected_ty))
}

fn enum_instance_fqn_and_args_from_type(
    ty: TypeId,
    lower: &TypeLowering<'_>,
) -> Option<(String, Vec<TypeId>)> {
    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Option(inner)) => {
            Some(("scoop.core.Option".to_string(), vec![inner]))
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if matches!(
                lower.nominal_decl_kind(&nominal.fqn),
                Some(ast::TypeKind::Enum)
            ) =>
        {
            Some((nominal.fqn.clone(), nominal.args.clone()))
        }
        _ => None,
    }
}

fn infer_struct_lit_expr_type(
    inputs: ExprInferInputs<'_>,
    struct_lit_expr: &ast::Expr,
    ty: &ast::TypePath,
    fields: &[ast::StructLitField],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    // 先把 TypeName lowering 为一个 nominal value type（struct/enum）；并进一步约束必须是 struct。
    let struct_ty = lower.lower_type_ref(&ast::TypeRef::Path(ty.clone()))?;

    let (struct_fqn, struct_name) = match lower.type_kind(struct_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            (nominal.fqn, lower.fmt_type(struct_ty))
        }
        _ => {
            return Err(ExprTypeError::StructLitNotStruct {
                found: lower.fmt_type(struct_ty),
                span: ty.span.into(),
            });
        }
    };

    if !matches!(
        lower.nominal_decl_kind(&struct_fqn),
        Some(ast::TypeKind::Struct)
    ) {
        return Err(ExprTypeError::StructLitNotStruct {
            found: struct_name,
            span: ty.span.into(),
        });
    }

    // 收集该 struct 的“直接字段”（不包含 nested type 的字段）。
    //
    // 说明：`collect_struct_field_types` 会为 nested struct 生成形如：
    //   `Outer.Inner.x`
    // 对于 `Outer { ... }` 的 struct literal，我们只接受 `Outer.<field>` 这一层。
    let prefix = format!("{struct_fqn}.");
    let mut expected_fields: HashMap<String, TypeId> = HashMap::new();
    for (field_fqn, field_ty) in inputs.struct_field_types {
        let Some(rest) = field_fqn.strip_prefix(&prefix) else {
            continue;
        };
        if rest.contains('.') {
            continue;
        }
        expected_fields.insert(rest.to_string(), *field_ty);
    }

    // 逐项检查：
    // - 字段名不可重复
    // - 字段必须存在于 struct 声明中
    // - 字段初始化表达式类型必须可赋值给字段类型（最小 assignable 规则）
    let mut seen: HashMap<String, Span> = HashMap::new();
    for f in fields {
        let field_name = source.slice(f.name.span).to_string();

        if let Some(prev) = seen.get(&field_name).copied() {
            return Err(ExprTypeError::StructLitDuplicateField {
                struct_name: struct_name.clone(),
                field: field_name,
                first: prev.into(),
                second: f.name.span.into(),
            });
        }
        seen.insert(field_name.clone(), f.name.span);

        let Some(expected_ty) = expected_fields.get(&field_name).copied() else {
            return Err(ExprTypeError::StructLitUnknownField {
                struct_name: struct_name.clone(),
                field: field_name,
                span: f.name.span.into(),
            });
        };

        let found_ty = inputs.infer_in_expected(
            lower,
            &f.value,
            expected_ty,
            ExpectedTypeFrom::new(format!(
                "struct `{}` 字段 `{}` 的类型",
                struct_name, field_name
            )),
        )?;

        if !is_type_assignable(found_ty, expected_ty, lower, builtins)
            && !literal_absorbs_to_expected(&f.value, expected_ty, source, lower, builtins)
        {
            return Err(ExprTypeError::StructLitFieldTypeMismatch {
                struct_name: struct_name.clone(),
                field: field_name,
                expected: lower.fmt_type(expected_ty),
                found: lower.fmt_type(found_ty),
                span: f.value.span.into(),
            });
        }
    }

    // 当前阶段（T0423）约束：struct literal 必须显式提供所有字段（不支持默认值/可选字段）。
    let mut missing: Vec<String> = expected_fields
        .keys()
        .filter(|name| !seen.contains_key(*name))
        .cloned()
        .collect();
    missing.sort();
    if !missing.is_empty() {
        // 尽量把错误定位到右花括号 `}`（缺字段通常发生在结尾）。
        let close_brace = if struct_lit_expr.span.end > 0 {
            Span::new(struct_lit_expr.span.end - 1, struct_lit_expr.span.end)
        } else {
            struct_lit_expr.span
        };

        return Err(ExprTypeError::StructLitMissingFields {
            struct_name,
            fields: missing.join(", "),
            span: close_brace.into(),
        });
    }

    Ok(struct_ty)
}

/// T0124: Infer the type of a generic struct literal using the expected type context.
///
/// When the struct literal omits type arguments (e.g., `Pair { first: 10, second: 20 }`)
/// but the expected type provides concrete instantiation (e.g., `Pair<Int, String>`),
/// we use the expected type to drive type inference.
fn infer_generic_struct_lit_expr_type(
    inputs: ExprInferInputs<'_>,
    struct_lit_expr: &ast::Expr,
    expected_ty: TypeId,
    fields: &[ast::StructLitField],
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    let builtins = inputs.builtins;
    let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = lower.type_kind(expected_ty) else {
        unreachable!("caller guarantees expected_ty is a Nominal");
    };
    let struct_fqn = nominal.fqn.clone();
    let type_args = nominal.args.clone();
    let struct_name = lower.fmt_type(expected_ty);

    // Build substitution map: type param name → concrete TypeId.
    let param_names = lower
        .env()
        .type_symbol(&struct_fqn)
        .map(|s| s.type_param_names.clone())
        .unwrap_or_default();
    let subst: HashMap<String, TypeId> = param_names
        .into_iter()
        .zip(type_args.iter().copied())
        .collect();

    // Collect direct fields of this struct (same prefix logic as non-generic path).
    let prefix = format!("{struct_fqn}.");
    let mut expected_fields: HashMap<String, TypeId> = HashMap::new();
    for (field_fqn, field_ty) in inputs.struct_field_types {
        let Some(rest) = field_fqn.strip_prefix(&prefix) else {
            continue;
        };
        if rest.contains('.') {
            continue;
        }
        // Substitute type params to concrete types.
        let concrete_ty = match lower.type_kind(*field_ty) {
            TypeKind::Param(p) => subst.get(&p.name).copied().unwrap_or(*field_ty),
            _ => *field_ty,
        };
        expected_fields.insert(rest.to_string(), concrete_ty);
    }

    // Validate fields (same logic as infer_struct_lit_expr_type).
    let mut seen: HashMap<String, Span> = HashMap::new();
    for f in fields {
        let field_name = source.slice(f.name.span).to_string();

        if let Some(prev) = seen.get(&field_name).copied() {
            return Err(ExprTypeError::StructLitDuplicateField {
                struct_name: struct_name.clone(),
                field: field_name,
                first: prev.into(),
                second: f.name.span.into(),
            });
        }
        seen.insert(field_name.clone(), f.name.span);

        let Some(field_expected_ty) = expected_fields.get(&field_name).copied() else {
            return Err(ExprTypeError::StructLitUnknownField {
                struct_name: struct_name.clone(),
                field: field_name,
                span: f.name.span.into(),
            });
        };

        let found_ty = inputs.infer_in_expected(
            lower,
            &f.value,
            field_expected_ty,
            ExpectedTypeFrom::new(format!(
                "struct `{}` 字段 `{}` 的类型",
                struct_name, field_name
            )),
        )?;

        if !is_type_assignable(found_ty, field_expected_ty, lower, builtins)
            && !literal_absorbs_to_expected(&f.value, field_expected_ty, source, lower, builtins)
        {
            return Err(ExprTypeError::StructLitFieldTypeMismatch {
                struct_name: struct_name.clone(),
                field: field_name,
                expected: lower.fmt_type(field_expected_ty),
                found: lower.fmt_type(found_ty),
                span: f.value.span.into(),
            });
        }
    }

    // Check for missing fields.
    let mut missing: Vec<String> = expected_fields
        .keys()
        .filter(|name| !seen.contains_key(*name))
        .cloned()
        .collect();
    missing.sort();
    if !missing.is_empty() {
        let close_brace = if struct_lit_expr.span.end > 0 {
            Span::new(struct_lit_expr.span.end - 1, struct_lit_expr.span.end)
        } else {
            struct_lit_expr.span
        };
        return Err(ExprTypeError::StructLitMissingFields {
            struct_name,
            fields: missing.join(", "),
            span: close_brace.into(),
        });
    }

    Ok(expected_ty)
}

fn infer_with_update_expr_type(
    inputs: ExprInferInputs<'_>,
    base: &ast::Expr,
    updates: &[ast::WithUpdateField],
    resolved_struct_fqns: &std::cell::OnceCell<std::collections::HashMap<String, String>>,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let source = inputs.source;
    // 先递归类型检查 base：保证 `p with { ... }` 中的 `p` 自身也会被覆盖。
    let base_ty = inputs.infer(lower, base)?;

    // 当前阶段（T0415）仅支持 struct 字段更新：
    // - base 必须是名义值类型，并且其声明 kind 为 `struct`
    let (base_struct_fqn, base_struct_name) = match lower.type_kind(base_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => (nominal.fqn, lower.fmt_type(base_ty)),
        _ => {
            return Err(ExprTypeError::WithUpdateBaseNotSupported {
                found: lower.fmt_type(base_ty),
                span: base.span.into(),
            });
        }
    };

    if !matches!(
        lower.nominal_decl_kind(&base_struct_fqn),
        Some(ast::TypeKind::Struct)
    ) {
        return Err(ExprTypeError::WithUpdateBaseNotSupported {
            found: base_struct_name,
            span: base.span.into(),
        });
    }

    // 收集各层 struct FQN：key 为路径前缀，value 为 struct FQN。
    let mut fqn_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    fqn_map.insert(String::new(), base_struct_fqn.clone());

    // `with` 的并行语义：update 之间没有顺序依赖，因此要求：
    // - 完全相同的 path 不能重复出现（否则“谁覆盖谁”会引入顺序）
    // - 一条 path 不能包含另一条 path（例如 `start` 与 `start.x`），否则更新含义不明确
    let mut seen_exact: HashMap<String, Span> = HashMap::new();
    let mut seen_paths: Vec<(Vec<String>, String, Span)> = Vec::new();

    let is_strict_prefix = |a: &[String], b: &[String]| -> bool {
        if a.len() >= b.len() {
            return false;
        }
        a.iter().zip(b.iter()).all(|(x, y)| x == y)
    };

    for u in updates {
        let segments: Vec<String> = u
            .path
            .segments
            .iter()
            .map(|seg| source.slice(seg.span).to_string())
            .collect();
        let path = segments.join(".");

        if let Some(first) = seen_exact.get(&path).copied() {
            return Err(ExprTypeError::WithUpdateDuplicatePath {
                path,
                first: first.into(),
                second: u.path.span.into(),
            });
        }

        for (prev_segments, prev_path, prev_span) in &seen_paths {
            if is_strict_prefix(prev_segments, &segments)
                || is_strict_prefix(&segments, prev_segments)
            {
                // `prev` 与当前 `u` 存在包含关系：报冲突并定位到“第二次出现的那一条”。
                let (parent, child) = if is_strict_prefix(prev_segments, &segments) {
                    (prev_path.clone(), path.clone())
                } else {
                    (path.clone(), prev_path.clone())
                };
                return Err(ExprTypeError::WithUpdateOverlappingPaths {
                    parent,
                    child,
                    first: (*prev_span).into(),
                    second: u.path.span.into(),
                });
            }
        }

        seen_exact.insert(path.clone(), u.path.span);
        seen_paths.push((segments, path, u.path.span));
    }

    for u in updates {
        // 路径可以多段：`a.b.c: value`。
        //
        // 当前阶段限制：
        // - 每一段都必须是 struct 字段
        // - 中间段字段类型必须是 struct（才能继续向下更新）
        let mut current_struct_fqn = base_struct_fqn.clone();
        let mut current_struct_name = lower.fmt_type(base_ty);
        let mut path_prefix_parts: Vec<String> = Vec::new();

        if u.path.segments.is_empty() {
            // parser 不会产生空路径；这里仅保持健壮性。
            return Err(ExprTypeError::WithUpdateNestedPathNotSupported {
                path: "<empty>".to_string(),
                span: u.path.span.into(),
            });
        }

        for (i, seg) in u.path.segments.iter().enumerate() {
            let field = source.slice(seg.span).to_string();
            let field_fqn = format!("{current_struct_fqn}.{field}");
            let Some(field_ty) = inputs.struct_field_types.get(&field_fqn).copied() else {
                return Err(ExprTypeError::WithUpdateUnknownField {
                    struct_name: current_struct_name.clone(),
                    field,
                    span: seg.span.into(),
                });
            };

            let is_last = i + 1 == u.path.segments.len();
            if is_last {
                let expected_ty = field_ty;
                let found_ty = inputs.infer_in_expected(
                    lower,
                    &u.value,
                    expected_ty,
                    ExpectedTypeFrom::new(format!(
                        "with-update `{}` 字段 `{}` 的类型",
                        current_struct_name, field
                    )),
                )?;

                if found_ty != expected_ty
                    && !literal_absorbs_to_expected(
                        &u.value,
                        expected_ty,
                        inputs.source,
                        lower,
                        inputs.builtins,
                    )
                {
                    return Err(ExprTypeError::WithUpdateFieldTypeMismatch {
                        struct_name: current_struct_name.clone(),
                        field,
                        expected: lower.fmt_type(expected_ty),
                        found: lower.fmt_type(found_ty),
                        span: u.value.span.into(),
                    });
                }

                break;
            }

            // 中间段：必须是 struct 才能继续向下。
            let (next_fqn, next_name) = match lower.type_kind(field_ty) {
                TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                    (nominal.fqn, lower.fmt_type(field_ty))
                }
                _ => {
                    return Err(ExprTypeError::WithUpdateNestedPathNotStruct {
                        struct_name: current_struct_name.clone(),
                        field,
                        found: lower.fmt_type(field_ty),
                        span: seg.span.into(),
                    });
                }
            };

            if !matches!(
                lower.nominal_decl_kind(&next_fqn),
                Some(ast::TypeKind::Struct)
            ) {
                return Err(ExprTypeError::WithUpdateNestedPathNotStruct {
                    struct_name: current_struct_name.clone(),
                    field,
                    found: next_name,
                    span: seg.span.into(),
                });
            }

            // 记录中间 struct FQN：path_prefix → struct FQN。
            path_prefix_parts.push(field.clone());
            let prefix_key = path_prefix_parts.join(".");
            fqn_map.insert(prefix_key, next_fqn.clone());

            current_struct_fqn = next_fqn;
            current_struct_name = next_name;
        }

        // loop 中在最后一段已完成 value typecheck；这里无需额外动作。
    }

    // 写回所有层级的 struct FQN，供 HIR lowering 使用。
    let _ = resolved_struct_fqns.set(fqn_map);

    Ok(base_ty)
}

fn is_cast_allowed(
    from: TypeId,
    to: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if from == to {
        return true;
    }

    // spec §4.4：`as`/`as?` 不做值类型之间的“数值转换”；
    // 但 spec §2.5 允许 value → interface/Any 的显式转换（boxing）。
    //
    // 当前阶段策略：
    // - ref → ref：允许（运行期检查式转换）
    // - value → Any / interface：允许（boxing）
    // - ref → value：不允许（unboxing 需要运行期支持，后续任务补齐）
    if lower.is_ref(from) && lower.is_ref(to) {
        return true;
    }

    // value → Any：允许（boxing）。
    if to == builtins.any && matches!(lower.type_kind(from), TypeKind::Value(_)) {
        return true;
    }

    // value → interface：允许（boxing）。
    match (lower.type_kind(from), lower.type_kind(to)) {
        (
            TypeKind::Value(ValueTypeKind::Nominal(found_nominal)),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        ) => {
            expected_nominal.args.is_empty()
                && expected_nominal.eff.is_none()
                && nominal_is_subtype_by_fqn(&found_nominal.fqn, &expected_nominal.fqn, lower.env())
        }
        _ => false,
    }
}

fn infer_value_ident_type(
    source: &SourceFile,
    id: &ast::ValueIdent,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    // `true/false` 当前阶段仍以 ident token 形式存在，但语义上属于字面量。
    let name = source.slice(id.span);
    if name == "true" || name == "false" {
        return Ok(builtins.bool_);
    }

    let Some(resolved) = id.resolved.as_ref() else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "ident（未 resolve）",
            span: id.span.into(),
        });
    };

    match resolved {
        ast::ResolvedValueRef::Local { decl_span, .. } => locals
            .get(decl_span)
            .copied()
            .ok_or_else(|| ExprTypeError::UnknownLocalValueType {
                name: name.to_string(),
                span: id.span.into(),
            }),
        ast::ResolvedValueRef::TopLevel { fqn } => {
            if let Some(ty) = top_level_types.get(fqn).copied() {
                return Ok(ty);
            }

            // Kotlin-like：`object Foo` 同时引入一个“类型名 Foo”与一个“值名 Foo”；
            // 在表达式位置引用 `Foo` 时，类型为该 object 的名义类型 `Foo`。
            if lower.is_object_type(fqn) {
                return Ok(lower.lower_type_fqn_with_args(fqn.clone(), Vec::new(), id.span)?);
            }

            Err(ExprTypeError::UnsupportedTopLevelValueType {
                fqn: fqn.clone(),
                span: id.span.into(),
            })
        }
    }
}
