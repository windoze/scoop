use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::resolve::Visibility;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::call::{
    check_fn_value_to_any_erasure_gate, check_nogc_boxing_gate,
    infer_continuation_resume_call_expr_type, infer_effect_op_call_expr_type,
};
use super::entry::try_infer_fun_return_ty_from_block;
use super::infer::{ExpectedTypeFrom, infer_handle_expr_type};
use super::member::{
    infer_member_access_ty_from_known_receiver, infer_not_null_assert_expr_type,
    resolve_member_value_target_for_receiver,
};
use super::ops::{
    NominalReceiverRef, collect_unique_zero_arg_member_method_sig, literal_absorbs_to_expected,
    record_member_method_effects_as_performed, try_extract_nominal_fqn_and_args,
};
use super::util::{fmt_effect_row, visibility_from_modifiers};

use super::{
    ASYNC_EFFECT_FQN, ExprInferInputs, ExprTypeError, FunSigOwned, ProgramBoundaryKind, TASK_FQN,
};

use super::super::annotations::check_inline_annotation_uses;
use super::super::assignable::is_type_assignable;
use super::super::builtin_annotations::BuiltinAnnotationFlags;
use super::super::lower::{TypeLowering, WhereBoundEntry};
use super::super::type_env::AnnotationTargetKind;
use super::super::{val_pat, when_exhaustiveness, when_pat};

#[derive(Clone, Copy)]
pub(super) struct StmtExprShared<'a> {
    pub(super) source: &'a SourceFile,
    pub(super) builtins: BuiltinTypes,
    pub(super) top_level_types: &'a HashMap<String, TypeId>,
    pub(super) top_level_funs: &'a HashMap<String, Vec<FunSigOwned>>,
    pub(super) member_mutabilities: &'a HashMap<String, bool>,
    pub(super) struct_field_types: &'a HashMap<String, TypeId>,
}

pub(super) struct StmtExprState<'a> {
    pub(super) locals: &'a mut HashMap<Span, TypeId>,
    pub(super) stable_bindings: &'a mut HashSet<Span>,
    pub(super) mutable_bindings: &'a mut HashSet<Span>,
    pub(super) comptime_bindings: &'a mut HashSet<Span>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StmtExprFlow {
    pub(super) loop_depth: usize,
    pub(super) expected_return_ty: Option<TypeId>,
    pub(super) lambda_this_decl_span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExprStmtCallMode {
    StructuralOnly,
    WithUnifiedGate,
}

#[derive(Debug, Clone, Copy)]
struct StmtExprContext {
    flow: StmtExprFlow,
    call_mode: ExprStmtCallMode,
}

pub(super) struct FunBodyCheckInputs<'a> {
    pub(super) source: &'a SourceFile,
    pub(super) builtins: BuiltinTypes,
    pub(super) top_level_types: &'a HashMap<String, TypeId>,
    pub(super) top_level_funs: &'a mut HashMap<String, Vec<FunSigOwned>>,
    pub(super) member_mutabilities: &'a HashMap<String, bool>,
    pub(super) struct_field_types: &'a HashMap<String, TypeId>,
}

pub(super) fn expr_infer_inputs<'a, 'b>(
    shared: StmtExprShared<'a>,
    locals: &'b HashMap<Span, TypeId>,
) -> ExprInferInputs<'b>
where
    'a: 'b,
{
    ExprInferInputs {
        source: shared.source,
        builtins: shared.builtins,
        locals,
        lambda_this_decl_span: None,
        comptime_bindings: None,
        top_level_types: shared.top_level_types,
        top_level_funs: shared.top_level_funs,
        member_mutabilities: Some(shared.member_mutabilities),
        struct_field_types: shared.struct_field_types,
        loop_depth: 0,
        expected_return_ty: None,
    }
}

pub(super) fn expr_infer_inputs_with_flow<'a, 'b>(
    shared: StmtExprShared<'a>,
    state: &'b StmtExprState<'_>,
    flow: StmtExprFlow,
) -> ExprInferInputs<'b>
where
    'a: 'b,
{
    ExprInferInputs {
        source: shared.source,
        builtins: shared.builtins,
        locals: state.locals,
        lambda_this_decl_span: flow.lambda_this_decl_span,
        comptime_bindings: Some(state.comptime_bindings),
        top_level_types: shared.top_level_types,
        top_level_funs: shared.top_level_funs,
        member_mutabilities: Some(shared.member_mutabilities),
        struct_field_types: shared.struct_field_types,
        loop_depth: flow.loop_depth,
        expected_return_ty: flow.expected_return_ty,
    }
}

fn check_lambda_expr_stmt_body(
    shared: StmtExprShared<'_>,
    lam: &ast::LambdaExpr,
    lower: &mut TypeLowering<'_>,
    state: &StmtExprState<'_>,
    flow: StmtExprFlow,
) -> Result<(), ExprTypeError> {
    // 说明：当前阶段 lambda 仍未完整 typecheck；这里仅复用现有的“语句层递归”逻辑来：
    // - 捕获非法 `return`（`return` 只能离开立即包裹的命名函数）
    // - 避免 lambda 内的局部声明污染外层作用域（clone 快照）
    let mut lambda_locals = state.locals.clone();
    let mut lambda_stable = state.stable_bindings.clone();
    let mut lambda_mutable = state.mutable_bindings.clone();
    let mut lambda_comptime = state.comptime_bindings.clone();

    // required effects（T0604）：lambda body 的 effect 属于该函数值，不计入外层函数立即执行的 effects。
    lower.with_effect_collection_suspended(|lower| {
        // `@NoGC`：lambda body 并不在外层函数执行时立即运行，不能把 `@NoGC` 的限制“向内传播”。
        lower.with_nogc_context_suspended(|lower| {
            lower.with_safe_lambda_context(lam, |lower| {
                let mut lambda_state = StmtExprState {
                    locals: &mut lambda_locals,
                    stable_bindings: &mut lambda_stable,
                    mutable_bindings: &mut lambda_mutable,
                    comptime_bindings: &mut lambda_comptime,
                };
                check_expr_stmt_with_mode(
                    shared,
                    lam.body.as_ref(),
                    lower,
                    &mut lambda_state,
                    StmtExprFlow {
                        loop_depth: 0,
                        expected_return_ty: None,
                        lambda_this_decl_span: flow.lambda_this_decl_span,
                    },
                    ExprStmtCallMode::StructuralOnly,
                )
            })
        })
    })
}

fn check_call_expr_stmt_lambda_args(
    shared: StmtExprShared<'_>,
    callee: &ast::Expr,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    state: &StmtExprState<'_>,
    flow: StmtExprFlow,
) -> Result<(), ExprTypeError> {
    let _ = callee;
    for arg in args {
        let ast::ExprKind::Lambda(lam) = &arg.kind else {
            continue;
        };
        check_lambda_expr_stmt_body(shared, lam, lower, state, flow)?;
    }

    Ok(())
}

fn check_call_expr_stmt_fallback(
    shared: StmtExprShared<'_>,
    expr: &ast::Expr,
    callee: &ast::Expr,
    args: &[ast::Expr],
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    ctx: StmtExprContext,
) -> Result<(), ExprTypeError> {
    if let ast::ExprKind::SafeMemberAccess {
        receiver, member, ..
    } = &callee.kind
        && shared.source.slice(member.span) == "resume"
    {
        let receiver_ty =
            expr_infer_inputs_with_flow(shared, state, ctx.flow).infer(lower, receiver)?;
        let is_safe_continuation_resume = match lower.type_kind(receiver_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => matches!(
                lower.type_kind(inner),
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                    if nominal.fqn == "scoop.core.Continuation" && nominal.args.len() >= 2
            ),
            _ => false,
        };

        if is_safe_continuation_resume {
            check_expr_stmt_with_mode(shared, receiver, lower, state, ctx.flow, ctx.call_mode)?;
        } else {
            check_expr_stmt_with_mode(shared, callee, lower, state, ctx.flow, ctx.call_mode)?;
        }
    } else {
        check_expr_stmt_with_mode(shared, callee, lower, state, ctx.flow, ctx.call_mode)?;
    }

    for arg in args {
        if matches!(arg.kind, ast::ExprKind::Lambda(_)) {
            continue;
        }
        check_expr_stmt_with_mode(shared, arg, lower, state, ctx.flow, ctx.call_mode)?;
    }

    let effect_op_taken_over = if let ast::ExprKind::MemberAccess { member, .. } = &callee.kind {
        infer_effect_op_call_expr_type(
            expr_infer_inputs_with_flow(shared, state, ctx.flow),
            expr,
            member,
            args,
            None,
            lower,
        )?
        .is_some()
    } else if let ast::ExprKind::TypeApply {
        callee: inner,
        args: type_args,
    } = &callee.kind
        && let ast::ExprKind::MemberAccess { member, .. } = &inner.kind
    {
        let lowered = type_args
            .iter()
            .map(|a| lower.lower_type_ref(a))
            .collect::<Result<Vec<_>, _>>()?;

        infer_effect_op_call_expr_type(
            expr_infer_inputs_with_flow(shared, state, ctx.flow),
            expr,
            member,
            args,
            Some(lowered.as_slice()),
            lower,
        )?
        .is_some()
    } else {
        false
    };

    // `Continuation.resume(...)` 只在当前 call 没有先被 effect-op 路径接管时才参与；
    // 否则像 `Echo.resume(...)` 这类 effect op 名称碰撞会误入 builtin resume helper。
    if !effect_op_taken_over {
        let _ = infer_continuation_resume_call_expr_type(
            expr_infer_inputs_with_flow(shared, state, ctx.flow),
            expr,
            callee,
            args,
            lower,
        )?;
    }

    Ok(())
}

fn check_executable_main_signature(
    fun: &ast::FunDecl,
    return_ty: TypeId,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    if fun.modifiers.contains(&ast::Modifier::Async) {
        return Err(ExprTypeError::EntryPointMainInvalidSignature {
            found: "`async fun main`".to_string(),
            span: fun.name.span.into(),
        });
    }

    if !fun.type_params.is_empty() {
        return Err(ExprTypeError::EntryPointMainInvalidSignature {
            found: format!("带 {} 个类型参数的 `main`", fun.type_params.len()),
            span: fun.name.span.into(),
        });
    }

    if fun.eff_param.is_some() {
        return Err(ExprTypeError::EntryPointMainInvalidSignature {
            found: "`main` 带 effect row 参数".to_string(),
            span: fun.name.span.into(),
        });
    }

    if fun.where_clause.is_some() {
        return Err(ExprTypeError::EntryPointMainInvalidSignature {
            found: "`main` 带 where 子句".to_string(),
            span: fun.name.span.into(),
        });
    }

    match fun.params.as_slice() {
        [] => {}
        [param] => {
            if param.is_vararg {
                return Err(ExprTypeError::EntryPointMainInvalidSignature {
                    found: "带 vararg 参数的 `main`".to_string(),
                    span: param.name.span.into(),
                });
            }

            if param.default_value.is_some() {
                return Err(ExprTypeError::EntryPointMainInvalidSignature {
                    found: "带默认参数的 `main`".to_string(),
                    span: param.name.span.into(),
                });
            }

            let Some(ty_ref) = &param.ty else {
                return Err(ExprTypeError::EntryPointMainInvalidSignature {
                    found: "参数缺少显式类型标注".to_string(),
                    span: param.name.span.into(),
                });
            };

            let found_ty = lower.lower_type_ref(ty_ref)?;
            let expected_ty = lower.lower_type_fqn_with_args(
                "scoop.core.Array".to_string(),
                vec![builtins.string],
                ty_ref.span(),
            )?;
            if found_ty != expected_ty {
                return Err(ExprTypeError::EntryPointMainInvalidSignature {
                    found: format!("参数类型为 `{}`", lower.fmt_type(found_ty)),
                    span: ty_ref.span().into(),
                });
            }
        }
        params => {
            return Err(ExprTypeError::EntryPointMainInvalidSignature {
                found: format!("带 {} 个参数", params.len()),
                span: fun.params_span.into(),
            });
        }
    }

    if return_ty != builtins.unit && return_ty != builtins.int {
        let span = fun
            .return_ty
            .as_ref()
            .map_or(fun.name.span, ast::TypeRef::span);
        return Err(ExprTypeError::EntryPointMainInvalidSignature {
            found: format!("返回类型为 `{}`", lower.fmt_type(return_ty)),
            span: span.into(),
        });
    }

    Ok(())
}

pub(super) fn check_required_effects_for_fun_decl(
    fun: &ast::FunDecl,
    performed: &[(TypeId, Span)],
    program_boundary: ProgramBoundaryKind,
    fun_fqn: Option<&str>,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    match program_boundary {
        ProgramBoundaryKind::None => {}
        // spec §5.10：entry point 由 runtime 在无 ambient handler 的边界调用，
        // 因此其 effect row 必须收口为闭合纯行 `Pure!`（不能显式声明 non-Pure，也不能通过 internal/private 推断出效果）。
        ProgramBoundaryKind::Main => {
            if let Some(expr) = fun.effects.as_ref() {
                let row = lower.lower_effect_row_expr(Some(expr))?;
                if !row.terms.is_empty() {
                    return Err(ExprTypeError::EntryPointMustBePure {
                        declared: fmt_effect_row(&row, lower),
                        span: expr.span.into(),
                    });
                }
                // spec §5.8.4：entry point 属于 system boundary，必须是闭合 effect row（`Pure!`）。
                // 说明：省略 effects 标注时仍会按 entry point 的规则强制 Pure；这里仅对“显式写了 open row `/ Pure`”
                // 给出更明确的诊断，避免用户误以为 open row 能封住 callback/transitive effects。
                if !expr.closed {
                    return Err(ExprTypeError::EntryPointMustBeClosedPure {
                        declared: fmt_effect_row(&row, lower),
                        span: expr.span.into(),
                    });
                }
            }
        }
        // T0629b：库导出入口 / host entry points。
        //
        // 约束：
        // - 必须显式声明 `/ Pure!`（避免默认 `/ Pure` 的 open row 语义误用在 program boundary 上）
        // - 不允许声明 non-Pure row
        ProgramBoundaryKind::Export => {
            let entry = fun_fqn.unwrap_or("<unknown>").to_string();

            let Some(expr) = fun.effects.as_ref() else {
                return Err(ExprTypeError::ExportEntryPointMustDeclareClosedPure {
                    entry,
                    span: fun.name.span.into(),
                });
            };

            let row = lower.lower_effect_row_expr(Some(expr))?;
            if !row.terms.is_empty() {
                return Err(ExprTypeError::ExportEntryPointMustBePure {
                    entry,
                    declared: fmt_effect_row(&row, lower),
                    span: expr.span.into(),
                });
            }
            if !expr.closed {
                return Err(ExprTypeError::ExportEntryPointMustBeClosedPure {
                    entry,
                    declared: fmt_effect_row(&row, lower),
                    span: expr.span.into(),
                });
            }
        }
    }

    // 即使函数体没有 perform（`performed.is_empty()`），也需要对“显式写出的 effects row”做最小语义校验：
    // - effect row item 必须是 effect 类型
    // - 闭合 row 不能直接引用 row 变量（例如 `E!`，T0628b）
    if matches!(program_boundary, ProgramBoundaryKind::None)
        && let Some(expr) = fun.effects.as_ref()
    {
        let _ = lower.lower_effect_row_expr(Some(expr))?;
    }

    if performed.is_empty() {
        return Ok(());
    }

    // T0508：effect row 推断入口：
    // - entry point：强制为 Pure（spec §5.10；T0629b 的 export entry point 同理）。
    // - public：缺省效果强制为 Pure（perform 任何 effect 都必须显式标注 row 或被 handler 捕获）
    // - private/internal：允许省略 `/ RowExpr`，由函数体内 “立即执行的 perform” 推断出 required effects。
    let declared = match program_boundary {
        ProgramBoundaryKind::Main | ProgramBoundaryKind::Export => EffectRow::pure(),
        ProgramBoundaryKind::None => {
            if fun.effects.is_some() {
                lower.lower_effect_row_expr(fun.effects.as_ref())?
            } else {
                match visibility_from_modifiers(&fun.modifiers) {
                    Visibility::Public => EffectRow::pure(),
                    Visibility::Internal | Visibility::Private => {
                        let mut seen: HashSet<TypeId> = HashSet::new();
                        let mut terms: Vec<TypeId> = Vec::new();
                        for (effect, _) in performed.iter().copied() {
                            if seen.insert(effect) {
                                terms.push(effect);
                            }
                        }
                        EffectRow::new(terms)
                    }
                }
            }
        }
    };

    for (effect, span) in performed.iter().copied() {
        if declared.terms.contains(&effect) {
            continue;
        }

        return Err(ExprTypeError::RequiredEffectNotDeclared {
            required: lower.fmt_type(effect),
            declared: fmt_effect_row(&declared, lower),
            span: span.into(),
        });
    }

    Ok(())
}

pub(super) fn check_fun_body_exprs(
    fun_fqn: &str,
    fun: &ast::FunDecl,
    program_boundary: ProgramBoundaryKind,
    lower: &mut TypeLowering<'_>,
    inputs: FunBodyCheckInputs<'_>,
) -> Result<(), ExprTypeError> {
    let FunBodyCheckInputs {
        source,
        builtins,
        top_level_types,
        top_level_funs,
        member_mutabilities,
        struct_field_types,
    } = inputs;

    lower.push_type_params(&fun.type_params);

    // T0130：把函数声明处的 where 约束推入 bound 作用域，
    // 以便函数体内对 TypeKind::Param 接收者的方法调用可通过 bound 驱动分发。
    let where_bounds_pushed = if let Some(wc) = &fun.where_clause {
        let bounds = build_where_bound_entries(source, &fun.type_params, wc);
        lower.push_where_bounds(bounds);
        true
    } else {
        false
    };

    let eff_binding_pushed = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        if let Some(expr) = eff_param.default.as_ref()
            && let Err(e) = lower.lower_effect_row_expr(Some(expr))
        {
            if where_bounds_pushed {
                lower.pop_where_bounds();
            }
            lower.pop_type_params(&fun.type_params);
            return Err(e.into());
        }
        lower.push_effect_row_param_marker_binding(name, eff_param.name.span);
        true
    } else {
        false
    };

    let builtin_flags = BuiltinAnnotationFlags::from_annotations(source, &fun.annotations);
    let unsafe_ctx_pushed = builtin_flags.is_unsafe;
    let nogc_ctx_pushed = builtin_flags.is_nogc;
    if unsafe_ctx_pushed {
        lower.push_unsafe_context();
    }
    if nogc_ctx_pushed {
        lower.push_nogc_context();
    }
    let const_ctx_pushed = fun.modifiers.contains(&ast::Modifier::Const);
    if const_ctx_pushed {
        lower.push_const_context();
    }

    lower.begin_effect_collection();
    let body_result: Result<(), ExprTypeError> = {
        let mut check_body = |lower: &mut TypeLowering<'_>| -> Result<(), ExprTypeError> {
            // 函数级的“局部值类型表”（binder decl span → TypeId）。
            //
            // 当前阶段规则（最小子集）：
            // - 参数：必须有类型注解（由 headers check 保证），因此可直接 lowering；
            // - 局部 `val/var`：
            //   - 若显式写了 `: Type`，则以该类型为准，并校验 initializer（若存在）类型匹配；
            //   - 否则若有 initializer，则以 initializer 类型推导；
            //   - 都没有则当前阶段无法推导（后续任务再补齐规则）。
            let mut locals: HashMap<Span, TypeId> = HashMap::new();
            // 可用于 smart cast 的“稳定绑定”（当前阶段仅覆盖：参数 + `val`）。
            let mut stable_bindings: HashSet<Span> = HashSet::new();
            // 可赋值（mutable）的绑定：当前阶段仅覆盖局部 `var`。
            let mut mutable_bindings: HashSet<Span> = HashSet::new();
            let mut comptime_bindings: HashSet<Span> = HashSet::new();

            // 扩展函数：为 `this` 注入隐式绑定（resolver 将 `this` 解析到 receiver 的 decl_span）。
            if let Some(receiver) = &fun.receiver {
                let receiver_ty = lower.lower_type_ref(receiver)?;
                locals.insert(receiver.span(), receiver_ty);
                stable_bindings.insert(receiver.span());
            }

            for p in &fun.params {
                let Some(ty_ref) = &p.ty else {
                    continue;
                };
                let elem_ty = lower.lower_type_ref(ty_ref)?;
                // T0113: vararg 参数在函数体内被视为 Array<T>。
                let ty = if p.is_vararg {
                    lower.lower_type_fqn_with_args(
                        "scoop.core.Array".into(),
                        vec![elem_ty],
                        p.name.span,
                    )?
                } else {
                    elem_ty
                };
                locals.insert(p.name.span, ty);
                stable_bindings.insert(p.name.span);

                // T1305：默认参数的默认值表达式需要在声明处通过类型检查（按形参类型做可赋值检查）。
                if let Some(default_value) = &p.default_value {
                    let param_name = p.name.text(source).to_string();
                    let shared = StmtExprShared {
                        source,
                        builtins,
                        top_level_types,
                        top_level_funs: &*top_level_funs,
                        member_mutabilities,
                        struct_field_types,
                    };
                    let found_ty = expr_infer_inputs(shared, &locals).infer_in_expected(
                        lower,
                        default_value,
                        ty,
                        ExpectedTypeFrom::new(format!(
                            "`{}` 的形参 `{}` 的默认值",
                            fun_fqn, param_name
                        )),
                    )?;

                    if is_type_assignable(found_ty, ty, lower, builtins)
                        || literal_absorbs_to_expected(default_value, ty, source, lower, builtins)
                    {
                        continue;
                    }

                    return Err(ExprTypeError::DefaultParamValueTypeMismatch {
                        fun: fun_fqn.to_string(),
                        param: param_name,
                        expected: lower.fmt_type(ty),
                        found: lower.fmt_type(found_ty),
                        span: default_value.span.into(),
                    });
                }
            }

            // 该函数的期望返回类型（T0417）：用于 `return expr?` 的类型检查。
            let expected_return_ty = match &fun.return_ty {
                Some(ret) => lower.lower_type_ref(ret)?,
                None => match &fun.body {
                    ast::FunBody::Block(b) => {
                        let shared = StmtExprShared {
                            source,
                            builtins,
                            top_level_types,
                            top_level_funs: &*top_level_funs,
                            member_mutabilities,
                            struct_field_types,
                        };
                        let inferred = {
                            let mut state = StmtExprState {
                                locals: &mut locals,
                                stable_bindings: &mut stable_bindings,
                                mutable_bindings: &mut mutable_bindings,
                                comptime_bindings: &mut comptime_bindings,
                            };
                            try_infer_fun_return_ty_from_block(shared, b, lower, &mut state, 0)?
                        }
                        .unwrap_or(builtins.unit);

                        lower.record_inferred_fun_return_ty(fun.name.span, inferred);

                        // 回写到顶层函数签名表：使得后续同文件的调用点能看到推断后的返回类型。
                        if let Some(sigs) = top_level_funs.get_mut(fun_fqn)
                            && let Some(sig) =
                                sigs.iter_mut().find(|s| s.decl_span == fun.name.span)
                        {
                            sig.return_ty = if fun.modifiers.contains(&ast::Modifier::Async) {
                                lower.lower_type_fqn_with_args(
                                    TASK_FQN.to_string(),
                                    vec![inferred],
                                    fun.name.span,
                                )?
                            } else {
                                inferred
                            };
                        }

                        inferred
                    }
                    ast::FunBody::Missing => {
                        lower.record_inferred_fun_return_ty(fun.name.span, builtins.unit);
                        builtins.unit
                    }
                },
            };

            if matches!(program_boundary, ProgramBoundaryKind::Main) {
                check_executable_main_signature(fun, expected_return_ty, lower, builtins)?;
            }

            match &fun.body {
                ast::FunBody::Block(b) => {
                    let shared = StmtExprShared {
                        source,
                        builtins,
                        top_level_types,
                        top_level_funs: &*top_level_funs,
                        member_mutabilities,
                        struct_field_types,
                    };
                    let mut state = StmtExprState {
                        locals: &mut locals,
                        stable_bindings: &mut stable_bindings,
                        mutable_bindings: &mut mutable_bindings,
                        comptime_bindings: &mut comptime_bindings,
                    };
                    check_block_exprs(
                        shared,
                        b,
                        lower,
                        &mut state,
                        StmtExprFlow {
                            loop_depth: 0,
                            expected_return_ty: Some(expected_return_ty),
                            lambda_this_decl_span: None,
                        },
                    )?
                }
                ast::FunBody::Missing => {}
            }

            Ok(())
        };

        if builtin_flags.is_safe {
            lower.with_unsafe_context_suspended(|lower| check_body(lower))
        } else {
            check_body(lower)
        }
    };
    let performed_effects = lower.finish_effect_collection();

    let result = match body_result {
        Ok(()) => {
            // T0623：`async fun` 的 `/ Async` 只存在于 Task 的计算上下文，
            // 因此函数体内的 `Async` performed effects 不应向外层（调用点）传播。
            let performed_for_decl = if fun.modifiers.contains(&ast::Modifier::Async) {
                let async_effect = lower.lower_type_fqn_with_args(
                    ASYNC_EFFECT_FQN.to_string(),
                    Vec::new(),
                    fun.name.span,
                )?;
                performed_effects
                    .iter()
                    .copied()
                    .filter(|(effect, _)| *effect != async_effect)
                    .collect::<Vec<_>>()
            } else {
                performed_effects.clone()
            };

            let boundary_fqn =
                matches!(program_boundary, ProgramBoundaryKind::Export).then_some(fun_fqn);
            check_required_effects_for_fun_decl(
                fun,
                &performed_for_decl,
                program_boundary,
                boundary_fqn,
                lower,
            )?;
            Ok(())
        }
        Err(e) => Err(e),
    };
    if eff_binding_pushed {
        lower.pop_effect_row_param_binding();
    }
    if const_ctx_pushed {
        lower.pop_const_context();
    }
    if nogc_ctx_pushed {
        lower.pop_nogc_context();
    }
    if unsafe_ctx_pushed {
        lower.pop_unsafe_context();
    }
    if where_bounds_pushed {
        lower.pop_where_bounds();
    }
    lower.pop_type_params(&fun.type_params);
    result
}

pub(super) fn check_block_exprs(
    shared: StmtExprShared<'_>,
    block: &ast::Block,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    flow: StmtExprFlow,
) -> Result<(), ExprTypeError> {
    check_block_exprs_with_mode(
        shared,
        block,
        lower,
        state,
        flow,
        ExprStmtCallMode::WithUnifiedGate,
    )
}

fn check_block_exprs_with_mode(
    shared: StmtExprShared<'_>,
    block: &ast::Block,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    flow: StmtExprFlow,
    call_mode: ExprStmtCallMode,
) -> Result<(), ExprTypeError> {
    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里用“进入时快照 + 退出时回滚”的方式实现最小作用域，不引入额外的数据结构。
    let saved_locals = state.locals.clone();
    let saved_stable = state.stable_bindings.clone();
    let saved_mutable = state.mutable_bindings.clone();

    for stmt in &block.stmts {
        check_stmt_exprs_with_mode(shared, stmt, lower, state, flow, call_mode)?;
    }

    *state.locals = saved_locals;
    *state.stable_bindings = saved_stable;
    *state.mutable_bindings = saved_mutable;

    Ok(())
}

pub(super) fn check_stmt_exprs(
    shared: StmtExprShared<'_>,
    stmt: &ast::Stmt,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    flow: StmtExprFlow,
) -> Result<(), ExprTypeError> {
    check_stmt_exprs_with_mode(
        shared,
        stmt,
        lower,
        state,
        flow,
        ExprStmtCallMode::WithUnifiedGate,
    )
}

fn check_stmt_exprs_with_mode(
    shared: StmtExprShared<'_>,
    stmt: &ast::Stmt,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    flow: StmtExprFlow,
    call_mode: ExprStmtCallMode,
) -> Result<(), ExprTypeError> {
    match &stmt.kind {
        ast::StmtKind::Val(v) => check_local_val_decl_exprs(shared, v, lower, state, flow)?,
        ast::StmtKind::Expr(e) => {
            check_expr_stmt_with_mode(shared, e, lower, state, flow, call_mode)?
        }
        ast::StmtKind::Return { return_span, value } => {
            let Some(expected) = flow.expected_return_ty else {
                return Err(ExprTypeError::ReturnNotInFunctionBody {
                    span: (*return_span).into(),
                });
            };

            match value {
                Some(v) => {
                    let found = expr_infer_inputs_with_flow(shared, state, flow)
                        .infer_in_expected(
                            lower,
                            v,
                            expected,
                            ExpectedTypeFrom::new("函数返回类型"),
                        )?;
                    if !is_type_assignable(found, expected, lower, shared.builtins)
                        && !literal_absorbs_to_expected(
                            v,
                            expected,
                            shared.source,
                            lower,
                            shared.builtins,
                        )
                    {
                        return Err(ExprTypeError::ReturnTypeMismatch {
                            expected: lower.fmt_type(expected),
                            found: lower.fmt_type(found),
                            span: v.span.into(),
                        });
                    }
                    check_fn_value_to_any_erasure_gate(
                        found,
                        expected,
                        v.span,
                        lower,
                        shared.builtins,
                    )?;
                    check_nogc_boxing_gate(found, expected, v.span, lower, shared.builtins)?;
                }
                None => {
                    // `return` 不带返回值：等价于返回 `Unit`。
                    if expected != shared.builtins.unit {
                        return Err(ExprTypeError::ReturnValueRequired {
                            expected: lower.fmt_type(expected),
                            span: (*return_span).into(),
                        });
                    }
                }
            }
        }
        ast::StmtKind::While { cond, body, .. } => {
            let cond_ty = expr_infer_inputs_with_flow(shared, state, flow).infer(lower, cond)?;

            if !is_type_assignable(cond_ty, shared.builtins.bool_, lower, shared.builtins) {
                return Err(ExprTypeError::WhileConditionNotBool {
                    found: lower.fmt_type(cond_ty),
                    span: cond.span.into(),
                });
            }

            check_block_exprs_with_mode(
                shared,
                body,
                lower,
                state,
                StmtExprFlow {
                    loop_depth: flow.loop_depth + 1,
                    expected_return_ty: flow.expected_return_ty,
                    lambda_this_decl_span: flow.lambda_this_decl_span,
                },
                call_mode,
            )?;
        }
        ast::StmtKind::Break { break_span } => {
            if flow.loop_depth == 0 {
                return Err(ExprTypeError::BreakNotInLoop {
                    span: (*break_span).into(),
                });
            }
        }
        ast::StmtKind::Continue { continue_span } => {
            if flow.loop_depth == 0 {
                return Err(ExprTypeError::ContinueNotInLoop {
                    span: (*continue_span).into(),
                });
            }
        }
        ast::StmtKind::For(f) => {
            // Kotlin-like：`for (x in xs)` 按迭代协议降糖：
            // - `xs.iterator(): Iter`
            // - `Iter.next(): Option<Elem>`
            //
            // 当前阶段仅做“协议存在性 + 元素类型推导 + 作用域规则 + effects 计入”。
            let iter_ty = expr_infer_inputs_with_flow(shared, state, flow).infer(lower, &f.iter)?;

            let Some((iter_fqn, iter_args)) = try_extract_nominal_fqn_and_args(iter_ty, lower)
            else {
                return Err(ExprTypeError::ForMissingIteratorMethod {
                    found: lower.fmt_type(iter_ty),
                    span: f.iter.span.into(),
                });
            };

            // T0110：Array / IntProgression は .iterator() を持たないため、
            // 型特化で直接要素型を決定し、iterator protocol をバイパスする。
            use crate::ast::{ForLoopCustomResolvedInfo, ForLoopIterableKind, ForLoopResolvedInfo};

            let elem_ty = if iter_fqn == "scoop.core.Array" || iter_fqn == "scoop.core.MutableArray"
            {
                let _ = f.resolved_for_info.set(ForLoopResolvedInfo {
                    kind: ForLoopIterableKind::ArrayInt,
                    custom: None,
                });
                // Array<T> — 要素型は最初の型引数
                iter_args.first().copied().unwrap_or(shared.builtins.any)
            } else if iter_fqn == "scoop.core.IntProgression" {
                let _ = f.resolved_for_info.set(ForLoopResolvedInfo {
                    kind: ForLoopIterableKind::IntProgression,
                    custom: None,
                });
                // IntProgression — 要素型は常に Int
                shared.builtins.int
            } else {
                // Generic iterator protocol: xs.iterator().next(): Option<Elem>
                let iterator_method_fqn = format!("{iter_fqn}.iterator");
                let Some(iterator_sig) = collect_unique_zero_arg_member_method_sig(
                    shared.source,
                    NominalReceiverRef {
                        ty: iter_ty,
                        fqn: &iter_fqn,
                        args: &iter_args,
                    },
                    "iterator",
                    f.iter.span,
                    lower,
                    shared.builtins,
                )?
                else {
                    return Err(ExprTypeError::ForMissingIteratorMethod {
                        found: lower.fmt_type(iter_ty),
                        span: f.iter.span.into(),
                    });
                };

                record_member_method_effects_as_performed(
                    &iter_fqn,
                    &iter_args,
                    &iterator_sig,
                    f.for_span,
                    lower,
                )?;
                let iterator_ty = iterator_sig.return_ty;

                let Some((iterator_fqn, iterator_args)) =
                    try_extract_nominal_fqn_and_args(iterator_ty, lower)
                else {
                    return Err(ExprTypeError::ForMissingNextMethod {
                        found: lower.fmt_type(iterator_ty),
                        span: f.iter.span.into(),
                    });
                };

                let next_method_fqn = format!("{iterator_fqn}.next");
                let Some(next_sig) = collect_unique_zero_arg_member_method_sig(
                    shared.source,
                    NominalReceiverRef {
                        ty: iterator_ty,
                        fqn: &iterator_fqn,
                        args: &iterator_args,
                    },
                    "next",
                    f.iter.span,
                    lower,
                    shared.builtins,
                )?
                else {
                    return Err(ExprTypeError::ForMissingNextMethod {
                        found: lower.fmt_type(iterator_ty),
                        span: f.iter.span.into(),
                    });
                };
                record_member_method_effects_as_performed(
                    &iterator_fqn,
                    &iterator_args,
                    &next_sig,
                    f.for_span,
                    lower,
                )?;
                let elem = match lower.type_kind(next_sig.return_ty) {
                    TypeKind::Value(ValueTypeKind::Option(inner)) => inner,
                    _ => {
                        return Err(ExprTypeError::ForNextNotOption {
                            found: lower.fmt_type(next_sig.return_ty),
                            span: f.iter.span.into(),
                        });
                    }
                };

                let _ = f.resolved_for_info.set(ForLoopResolvedInfo {
                    kind: ForLoopIterableKind::Custom,
                    custom: Some(ForLoopCustomResolvedInfo {
                        iterator_method_fqn,
                        iterator_ty,
                        next_method_fqn,
                        elem_ty: elem,
                    }),
                });

                elem
            };

            // binder 仅在 body 作用域内可见：进入时注入，退出时回滚。
            let saved_locals = state.locals.clone();
            let saved_stable = state.stable_bindings.clone();
            let saved_mutable = state.mutable_bindings.clone();

            state.locals.insert(f.binder.span, elem_ty);
            state.stable_bindings.insert(f.binder.span);

            check_block_exprs_with_mode(
                shared,
                &f.body,
                lower,
                state,
                StmtExprFlow {
                    loop_depth: flow.loop_depth + 1,
                    expected_return_ty: flow.expected_return_ty,
                    lambda_this_decl_span: flow.lambda_this_decl_span,
                },
                call_mode,
            )?;

            *state.locals = saved_locals;
            *state.stable_bindings = saved_stable;
            *state.mutable_bindings = saved_mutable;
        }
        ast::StmtKind::ComptimeBlock { body, .. } => {
            check_block_exprs_with_mode(shared, body, lower, state, flow, call_mode)?;
        }
        ast::StmtKind::ComptimeIf(ci) => {
            check_block_exprs_with_mode(shared, &ci.then_branch, lower, state, flow, call_mode)?;
            if let Some(else_branch) = &ci.else_branch {
                match &**else_branch {
                    ast::ComptimeIfElse::Block(b) => {
                        check_block_exprs_with_mode(shared, b, lower, state, flow, call_mode)?
                    }
                    ast::ComptimeIfElse::If(next) => {
                        // 递归跟进 else-if 链。
                        let mut cur: &ast::ComptimeIf = next;
                        loop {
                            check_block_exprs_with_mode(
                                shared,
                                &cur.then_branch,
                                lower,
                                state,
                                flow,
                                call_mode,
                            )?;
                            match &cur.else_branch {
                                Some(e) => match &**e {
                                    ast::ComptimeIfElse::Block(b) => {
                                        check_block_exprs_with_mode(
                                            shared, b, lower, state, flow, call_mode,
                                        )?;
                                        break;
                                    }
                                    ast::ComptimeIfElse::If(next) => cur = next,
                                },
                                None => break,
                            }
                        }
                    }
                }
            }
        }
        ast::StmtKind::ComptimeFor(cf) => {
            let iter_ty =
                expr_infer_inputs_with_flow(shared, state, flow).infer(lower, &cf.iter)?;
            let elem_ty = match lower.type_kind(iter_ty) {
                TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                    elements.first().copied().unwrap_or(shared.builtins.any)
                }
                _ => try_extract_nominal_fqn_and_args(iter_ty, lower)
                    .and_then(|(iter_fqn, iter_args)| {
                        if iter_fqn == "scoop.core.Array"
                            || iter_fqn == "scoop.core.MutableArray"
                            || iter_fqn == "scoop.core.ComptimeList"
                        {
                            Some(iter_args.first().copied().unwrap_or(shared.builtins.any))
                        } else if iter_fqn == "scoop.core.IntProgression" {
                            Some(shared.builtins.int)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(shared.builtins.any),
            };

            let saved_locals = state.locals.clone();
            let saved_stable = state.stable_bindings.clone();
            let saved_mutable = state.mutable_bindings.clone();
            let saved_comptime = state.comptime_bindings.clone();

            state.locals.insert(cf.binder.span, elem_ty);
            state.stable_bindings.insert(cf.binder.span);
            state.comptime_bindings.insert(cf.binder.span);

            check_block_exprs_with_mode(
                shared,
                &cf.body,
                lower,
                state,
                StmtExprFlow {
                    loop_depth: flow.loop_depth + 1,
                    expected_return_ty: flow.expected_return_ty,
                    lambda_this_decl_span: flow.lambda_this_decl_span,
                },
                call_mode,
            )?;

            *state.locals = saved_locals;
            *state.stable_bindings = saved_stable;
            *state.mutable_bindings = saved_mutable;
            *state.comptime_bindings = saved_comptime;
        }
        ast::StmtKind::Empty | ast::StmtKind::Missing => {}
    }

    Ok(())
}

pub(super) fn check_local_val_decl_exprs(
    shared: StmtExprShared<'_>,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    flow: StmtExprFlow,
) -> Result<(), ExprTypeError> {
    check_inline_annotation_uses(
        shared.source,
        &v.annotations,
        AnnotationTargetKind::LocalVariable,
    )?;

    let declared_ty = match &v.ty {
        Some(ty_ref) => Some(lower.lower_type_ref(ty_ref)?),
        None => None,
    };
    let expected_from = match &v.binding {
        ast::ValBinding::Name(name) => ExpectedTypeFrom::new(format!(
            "局部绑定 `{}` 的类型注解",
            shared.source.slice(name.span)
        )),
        ast::ValBinding::Pattern(_) => ExpectedTypeFrom::new("局部解构绑定的类型注解"),
    };

    // 先类型检查 initializer（语义：局部变量在其声明之后可见，因此 init 内不能引用自身）。
    let init_ty =
        match &v.init {
            Some(init) => Some(match declared_ty {
                Some(expected) => expr_infer_inputs_with_flow(shared, state, flow)
                    .infer_in_expected(lower, init, expected, expected_from.clone())?,
                None => expr_infer_inputs_with_flow(shared, state, flow).infer(lower, init)?,
            }),
            None => None,
        };

    if let (Some(expected), Some(found)) = (declared_ty, init_ty) {
        let init = v.init.as_ref().unwrap();
        if !is_type_assignable(found, expected, lower, shared.builtins) {
            if literal_absorbs_to_expected(init, expected, shared.source, lower, shared.builtins) {
                // ok
            } else {
                // 复用顶层 initializer 的错误码与文本（保持 fixtures 断言稳定）。
                return Err(ExprTypeError::InitializerTypeMismatch {
                    expected: lower.fmt_type(expected),
                    found: lower.fmt_type(found),
                    span: init.span.into(),
                });
            }
        }
        check_fn_value_to_any_erasure_gate(found, expected, init.span, lower, shared.builtins)?;
        check_nogc_boxing_gate(found, expected, init.span, lower, shared.builtins)?;
    }

    let inferred = declared_ty.or(init_ty);

    match &v.binding {
        ast::ValBinding::Name(name) => {
            let Some(ty) = inferred else {
                // 当前阶段不支持“无类型注解 + 无 initializer”的局部绑定推导。
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "局部 val/var（缺少类型与 initializer）",
                    span: name.span.into(),
                });
            };
            lower.record_inferred_binding_ty(name.span, ty);
            state.locals.insert(name.span, ty);
            match v.kind {
                ast::ValKind::Val => {
                    state.stable_bindings.insert(name.span);
                }
                ast::ValKind::Var => {
                    state.mutable_bindings.insert(name.span);
                }
            }
        }
        ast::ValBinding::Pattern(pat) => {
            // spec §4.2：`var` 不支持 destructuring patterns（只允许简单绑定）。
            if matches!(v.kind, ast::ValKind::Var) {
                return Err(ExprTypeError::DestructuringVarNotAllowed {
                    span: pat.span.into(),
                });
            }

            let Some(init_ty) = init_ty else {
                // parser 已强制 pattern binding 必须有 initializer；这里仅做健壮性兜底。
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "解构绑定（缺少 initializer）",
                    span: pat.span.into(),
                });
            };

            let bindings = val_pat::infer_val_pat_bindings(
                shared.source,
                pat,
                init_ty,
                lower,
                shared.builtins,
                shared.struct_field_types,
            )?;

            // `val` 解构引入的绑定与普通 `val x = ...` 一样：
            // - 在其声明之后可见（resolver 已建立作用域）
            // - 属于稳定绑定，可用于 smart cast（当前阶段仅记录）
            for (decl_span, ty) in bindings {
                lower.record_inferred_binding_ty(decl_span, ty);
                state.locals.insert(decl_span, ty);
                state.stable_bindings.insert(decl_span);
            }
        }
    }

    Ok(())
}

pub(super) fn check_expr_stmt(
    shared: StmtExprShared<'_>,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    flow: StmtExprFlow,
) -> Result<(), ExprTypeError> {
    check_expr_stmt_with_mode(
        shared,
        expr,
        lower,
        state,
        flow,
        ExprStmtCallMode::WithUnifiedGate,
    )
}

fn check_expr_stmt_with_mode(
    shared: StmtExprShared<'_>,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    state: &mut StmtExprState<'_>,
    flow: StmtExprFlow,
    call_mode: ExprStmtCallMode,
) -> Result<(), ExprTypeError> {
    // 当前阶段的表达式语句仅用于支持控制流结构内部的“局部 val/var 推导”回归：
    // - `if (...) { val ... } else { ... }`
    // - `call { ... }`：递归进入 lambda body 捕获非法 `return`（T0444）
    //
    // 其他表达式语句（例如单独的调用）暂不强制 typecheck，以避免在未实现更多 ExprKind
    // 的阶段引入大量不相关的回归失败。
    match &expr.kind {
        ast::ExprKind::Annotated {
            annotations,
            expr: inner,
        } => {
            check_inline_annotation_uses(
                shared.source,
                annotations,
                AnnotationTargetKind::Expression,
            )?;
            check_expr_stmt_with_mode(shared, inner, lower, state, flow, call_mode)
        }
        ast::ExprKind::Block(b) | ast::ExprKind::DoBlock { body: b, .. } => {
            check_block_exprs_with_mode(shared, b, lower, state, flow, call_mode)
        }
        ast::ExprKind::UnsafeBlock { body, .. } => {
            lower.push_unsafe_context();
            let result = check_block_exprs_with_mode(shared, body, lower, state, flow, call_mode);
            lower.pop_unsafe_context();
            result
        }
        ast::ExprKind::SafeBlock { body, .. } => lower.with_unsafe_context_suspended(|lower| {
            check_block_exprs_with_mode(shared, body, lower, state, flow, call_mode)
        }),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => check_if_expr_stmt(
            shared,
            cond.as_ref(),
            then_branch.as_ref(),
            else_branch.as_deref(),
            lower,
            state,
            StmtExprContext { flow, call_mode },
        ),
        ast::ExprKind::When { subject, arms } => {
            // `when` 表达式作为语句时：
            // - 递归进入分支 body，以覆盖其中的局部绑定/控制流；
            // - T0427：为每个 arm 建立独立的“局部类型表”快照，并注入 pattern binder 的类型。
            check_expr_stmt_with_mode(shared, subject.as_ref(), lower, state, flow, call_mode)?;

            let subject_ty = expr_infer_inputs_with_flow(shared, state, flow)
                .infer(lower, subject.as_ref())
                .ok();

            if let Some(subject_ty) = subject_ty {
                when_exhaustiveness::check_when_exhaustiveness(
                    shared.source,
                    expr,
                    subject_ty,
                    arms,
                    lower,
                    shared.builtins,
                )?;
            }

            for arm in arms {
                let mut arm_locals = state.locals.clone();
                let mut arm_stable = state.stable_bindings.clone();
                let mut arm_mutable = state.mutable_bindings.clone();
                let mut arm_comptime = state.comptime_bindings.clone();

                if let Some(subject_ty) = subject_ty {
                    let bindings = when_pat::infer_when_pat_bindings(
                        shared.source,
                        &arm.pat,
                        subject_ty,
                        lower,
                        shared.builtins,
                    )?;
                    for (decl_span, ty) in bindings {
                        lower.record_inferred_binding_ty(decl_span, ty);
                        arm_locals.insert(decl_span, ty);
                        arm_stable.insert(decl_span);
                    }
                }

                let mut arm_state = StmtExprState {
                    locals: &mut arm_locals,
                    stable_bindings: &mut arm_stable,
                    mutable_bindings: &mut arm_mutable,
                    comptime_bindings: &mut arm_comptime,
                };
                check_expr_stmt_with_mode(
                    shared,
                    &arm.body,
                    lower,
                    &mut arm_state,
                    flow,
                    call_mode,
                )?;
            }
            Ok(())
        }
        ast::ExprKind::Handle {
            body,
            arms,
            finally,
        } => {
            // `handle` 在表达式语句位置仍需递归 typecheck：
            // - 以便捕获 handler arms 内的类型错误
            // - 以便正确记录 required effects（body 内被 handler 捕获的 effects 不应向外传播）
            let _ = infer_handle_expr_type(
                expr_infer_inputs_with_flow(shared, state, flow),
                expr,
                body,
                arms,
                finally.as_ref(),
                None,
                lower,
            )?;
            Ok(())
        }
        ast::ExprKind::NotNullAssert {
            expr: inner,
            op_span,
        } => {
            // `!!` 的语义属于“立即执行的表达式”（会在运行期做 null assertion），
            // 因此即使它出现在表达式语句位置，也必须参与 typecheck/required-effects 收集（T0421b）。
            let _ = infer_not_null_assert_expr_type(
                expr_infer_inputs_with_flow(shared, state, flow),
                inner.as_ref(),
                *op_span,
                lower,
            )?;
            Ok(())
        }
        ast::ExprKind::SafeMemberAccess { .. } => {
            // T0152：表达式语句中的调用实参会递归走 `check_expr_stmt`；
            // 若这里跳过 `receiver?.member`，typecheck 就无法补回 safe member access 的解析结果，
            // 后续 lowering/codegen 也拿不到稳定的成员目标。
            let _ = expr_infer_inputs_with_flow(shared, state, flow).infer(lower, expr)?;
            Ok(())
        }
        ast::ExprKind::Cast { .. } => {
            // T0445：`x as T` 的失败语义会触发 `Raise<RuntimeError>`。
            // 与 `!!` 一样，它属于“立即执行的表达式”，即使出现在表达式语句位置也必须参与
            // required-effects 收集；否则 `/ Pure` 函数体内的 `as` 会被错误放过。
            match expr_infer_inputs_with_flow(shared, state, flow).infer(lower, expr) {
                Ok(_) => Ok(()),
                Err(ExprTypeError::UnsupportedExpr { .. }) => Ok(()),
                Err(e) => Err(e),
            }
        }
        ast::ExprKind::Call { callee, args } => {
            // 先单独检查 lambda 实参，确保其中的 `return` 仍按“只能离开立即包裹的命名函数”处理。
            check_call_expr_stmt_lambda_args(shared, callee, args, lower, state, flow)?;

            // 然后复用 value-position 的统一调用 typecheck，但暂停普通调用的 required-effects 收集。
            // 本任务只收口 statement-position 的调用门禁；若把普通 callee effect row 也一并接通，
            // 会把未单独跟踪的 effect 传播语义变更混入本轮。
            if matches!(call_mode, ExprStmtCallMode::WithUnifiedGate) {
                match lower.with_effect_collection_suspended(|lower| {
                    expr_infer_inputs_with_flow(shared, state, flow).infer(lower, expr)
                }) {
                    Ok(_) | Err(ExprTypeError::UnsupportedExpr { .. }) => {}
                    Err(e) => return Err(e),
                }
            }

            // 继续保留现有的子表达式递归与“立即执行调用”required-effects 记录：
            // - callee / args 中的 cast / `!!` / nested call 仍需覆盖；
            // - effect op 与 `Continuation.resume(...)` 仍属于 statement 位置必须记录的立即效果。
            check_call_expr_stmt_fallback(
                shared,
                expr,
                callee,
                args,
                lower,
                state,
                StmtExprContext { flow, call_mode },
            )
        }
        ast::ExprKind::Lambda(lam) => check_lambda_expr_stmt_body(shared, lam, lower, state, flow),
        ast::ExprKind::Assign { lhs, rhs, .. } => {
            check_assign_expr_stmt(shared, expr.span, lhs, rhs, lower, state, flow)
        }
        _ => Ok(()),
    }
}

fn check_if_expr_stmt(
    shared: StmtExprShared<'_>,
    cond: &ast::Expr,
    then_branch: &ast::Expr,
    else_branch: Option<&ast::Expr>,
    lower: &mut TypeLowering<'_>,
    state: &StmtExprState<'_>,
    ctx: StmtExprContext,
) -> Result<(), ExprTypeError> {
    // 条件表达式仍需走完整 `infer`：
    // - 记录 compareTo / operator overload / effect call 等 typed side tables；
    // - 确保 `if` 作为语句出现时不会跳过条件里的表达式检查。
    let _ = expr_infer_inputs_with_flow(shared, state, ctx.flow).infer(lower, cond)?;

    // smart cast（T0413）最小子集：仅识别 `if (x is T)` / `if (x !is T)` 形式，
    // 并且只对“稳定绑定”（参数 + `val`）在对应分支内做类型收窄。
    let smart_cast =
        detect_smart_cast_for_if_condition(cond, lower, state.locals, state.stable_bindings)?;

    // then 分支：在 `x is T` 时收窄；在 `x !is T` 时保持原类型。
    let mut then_locals = state.locals.clone();
    let mut then_stable = state.stable_bindings.clone();
    let mut then_mutable = state.mutable_bindings.clone();
    let mut then_comptime = state.comptime_bindings.clone();
    if let Some(smart_cast) = smart_cast
        && smart_cast.narrow_in_then
    {
        then_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
    }
    let mut then_state = StmtExprState {
        locals: &mut then_locals,
        stable_bindings: &mut then_stable,
        mutable_bindings: &mut then_mutable,
        comptime_bindings: &mut then_comptime,
    };
    check_expr_stmt_with_mode(
        shared,
        then_branch,
        lower,
        &mut then_state,
        ctx.flow,
        ctx.call_mode,
    )?;

    // else 分支：在 `x !is T` 且存在 else 时收窄；否则保持原类型。
    if let Some(else_branch) = else_branch {
        let mut else_locals = state.locals.clone();
        let mut else_stable = state.stable_bindings.clone();
        let mut else_mutable = state.mutable_bindings.clone();
        let mut else_comptime = state.comptime_bindings.clone();
        if let Some(smart_cast) = smart_cast
            && !smart_cast.narrow_in_then
        {
            else_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
        }

        let mut else_state = StmtExprState {
            locals: &mut else_locals,
            stable_bindings: &mut else_stable,
            mutable_bindings: &mut else_mutable,
            comptime_bindings: &mut else_comptime,
        };
        check_expr_stmt_with_mode(
            shared,
            else_branch,
            lower,
            &mut else_state,
            ctx.flow,
            ctx.call_mode,
        )?;
    }

    Ok(())
}

fn check_assign_expr_stmt(
    shared: StmtExprShared<'_>,
    assign_span: Span,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    state: &StmtExprState<'_>,
    flow: StmtExprFlow,
) -> Result<(), ExprTypeError> {
    // T0443：赋值语句 `lhs = rhs` 最小规则：
    // - lhs 必须是可写目标：局部 `var` 绑定 或 可写属性（`var` property / ctor `var` param）
    // - rhs 类型必须可赋给 lhs（复用 `is_type_assignable` 的最小子类型/boxing 规则）
    let (expected_ty, place_kind, write_barrier, unsafe_required) = match &lhs.kind {
        ast::ExprKind::Ident(id) => {
            let Some(resolved) = id.resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（unresolved ident）",
                    span: id.span.into(),
                });
            };

            match resolved {
                ast::ResolvedValueRef::Local { name, decl_span } => {
                    if state.stable_bindings.contains(decl_span)
                        || !state.mutable_bindings.contains(decl_span)
                    {
                        return Err(ExprTypeError::AssignmentTargetNotMutable {
                            name: name.clone(),
                            span: id.span.into(),
                        });
                    }

                    let expected_ty = state.locals.get(decl_span).copied().ok_or_else(|| {
                        ExprTypeError::UnknownLocalValueType {
                            name: name.clone(),
                            span: id.span.into(),
                        }
                    })?;
                    (
                        expected_ty,
                        ast::AssignPlaceContractKind::Local {
                            name: name.clone(),
                            decl_span: *decl_span,
                        },
                        ast::AssignWriteBarrierRequirement::NotRequired,
                        false,
                    )
                }
                ast::ResolvedValueRef::TopLevel { fqn } => {
                    lower.emit_deprecated_value_use(fqn, id.span, "属性");
                    let expected_ty =
                        shared.top_level_types.get(fqn).copied().ok_or_else(|| {
                            ExprTypeError::UnsupportedTopLevelValueType {
                                fqn: fqn.clone(),
                                span: id.span.into(),
                            }
                        })?;

                    if !lower.is_top_level_value_mutable(fqn) {
                        return Err(ExprTypeError::AssignmentTargetNotMutable {
                            name: shared.source.slice(id.span).to_string(),
                            span: id.span.into(),
                        });
                    }

                    (
                        expected_ty,
                        ast::AssignPlaceContractKind::TopLevel { fqn: fqn.clone() },
                        ast::AssignWriteBarrierRequirement::StorageSlot {
                            slot_ty: expected_ty,
                        },
                        false,
                    )
                }
            }
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            let receiver_inputs = expr_infer_inputs_with_flow(shared, state, flow);
            // 先递归 typecheck receiver：保证 `a().b = rhs` 能覆盖 `a()`。
            //
            // 例外：`TypeName.member` 经 companion object 解析时，receiver 不是值表达式；
            // resolver 会保留 receiver ident 为未解析，此处跳过 receiver typecheck。
            let receiver_is_type_name =
                matches!(&receiver.kind, ast::ExprKind::Ident(id) if id.resolved.is_none());
            let receiver_ty = if receiver_is_type_name {
                None
            } else {
                Some(receiver_inputs.infer(lower, receiver)?)
            };
            let resolved = resolve_member_value_target_for_receiver(
                receiver_inputs,
                receiver,
                receiver_ty,
                member,
                lower,
            );

            let Some(resolved) = resolved.as_ref() else {
                return Err(ExprTypeError::UnsupportedExpr {
                    kind: "assignment lhs（member 未 resolve）",
                    span: member.span.into(),
                });
            };
            lower.record_typechecked_member_resolution(member.span, resolved.clone());

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

            if !shared
                .member_mutabilities
                .get(fqn)
                .copied()
                .unwrap_or(false)
            {
                return Err(ExprTypeError::AssignmentTargetNotMutable {
                    name: shared.source.slice(member.span).to_string(),
                    span: member.span.into(),
                });
            }

            let expected_ty = infer_member_access_ty_from_known_receiver(
                receiver_inputs,
                receiver_ty,
                member,
                Some(resolved),
                lower,
            )?;
            let member_name = shared.source.slice(member.span).to_string();
            let owner_fqn = owner_fqn_from_member_fqn(fqn, &member_name);
            (
                expected_ty,
                ast::AssignPlaceContractKind::Member {
                    owner_fqn,
                    member_fqn: fqn.clone(),
                    member_name,
                    receiver_ty,
                },
                ast::AssignWriteBarrierRequirement::StorageSlot {
                    slot_ty: expected_ty,
                },
                false,
            )
        }
        _ => {
            return Err(ExprTypeError::UnsupportedExpr {
                kind: "assignment lhs（仅支持标识符或成员访问）",
                span: lhs.span.into(),
            });
        }
    };

    // 递归 typecheck rhs：保证 `x = f()` 这类语句也会覆盖 rhs 中的表达式。
    let expected_from = match &lhs.kind {
        ast::ExprKind::Ident(id) => ExpectedTypeFrom::new(format!(
            "赋值目标 `{}` 的类型",
            shared.source.slice(id.span)
        )),
        ast::ExprKind::MemberAccess { member, .. } => ExpectedTypeFrom::new(format!(
            "赋值目标 `{}` 的字段类型",
            shared.source.slice(member.span)
        )),
        _ => ExpectedTypeFrom::new("赋值目标的类型"),
    };
    let found_ty = expr_infer_inputs_with_flow(shared, state, flow).infer_in_expected(
        lower,
        rhs,
        expected_ty,
        expected_from,
    )?;

    let value_ty = if is_type_assignable(found_ty, expected_ty, lower, shared.builtins) {
        found_ty
    } else if literal_absorbs_to_expected(rhs, expected_ty, shared.source, lower, shared.builtins) {
        expected_ty
    } else {
        return Err(ExprTypeError::AssignmentTypeMismatch {
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: rhs.span.into(),
        });
    };

    lower.record_assign_place_contract(
        assign_span,
        ast::AssignPlaceContract {
            kind: place_kind,
            place_ty: expected_ty,
            value_ty,
            mutable: true,
            write_barrier,
            unsafe_required,
        },
    );

    Ok(())
}

fn owner_fqn_from_member_fqn(fqn: &str, member_name: &str) -> Option<String> {
    fqn.strip_suffix(member_name)
        .and_then(|prefix| prefix.strip_suffix('.'))
        .filter(|owner| !owner.is_empty())
        .map(ToOwned::to_owned)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SmartCastHint {
    pub(super) decl_span: Span,
    pub(super) target_ty: TypeId,
    pub(super) narrow_in_then: bool,
}

pub(super) fn detect_smart_cast_for_if_condition(
    cond: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
) -> Result<Option<SmartCastHint>, ExprTypeError> {
    let ast::ExprKind::TypeCheck { expr, op, ty, .. } = &cond.kind else {
        return Ok(None);
    };

    let ast::ExprKind::Ident(id) = &expr.kind else {
        return Ok(None);
    };

    let Some(ast::ResolvedValueRef::Local { decl_span, .. }) = id.resolved.as_ref() else {
        return Ok(None);
    };

    if !stable_bindings.contains(decl_span) {
        return Ok(None);
    }

    let Some(from_ty) = locals.get(decl_span).copied() else {
        return Ok(None);
    };

    let target_ty = lower.lower_type_ref(ty)?;

    // spec §4.3：smart cast 只对引用类型生效（值类型使用 enum/pattern 进行收窄）。
    if !(lower.is_ref(from_ty) && lower.is_ref(target_ty)) {
        return Ok(None);
    }

    Ok(Some(SmartCastHint {
        decl_span: *decl_span,
        target_ty,
        narrow_in_then: matches!(op, ast::TypeCheckOp::Is),
    }))
}

/// 从 AST `where_clause` 和 `type_params` 构建 `WhereBoundEntry` 列表（T0130）。
///
/// 每条 `where T: Bound` 约束映射为一个 `WhereBoundEntry`。
fn build_where_bound_entries(
    source: &SourceFile,
    type_params: &[ast::TypeParam],
    where_clause: &ast::WhereClause,
) -> Vec<WhereBoundEntry> {
    let param_names: Vec<String> = type_params
        .iter()
        .map(|p| source.slice(p.name.span).to_string())
        .collect();

    let mut out = Vec::new();
    for c in &where_clause.constraints {
        let target_name = source.slice(c.ty_param.span).to_string();
        // 只收集当前声明的 type params 的约束。
        if !param_names.contains(&target_name) {
            continue;
        }
        out.push(WhereBoundEntry {
            param_name: target_name,
            bound: c.bound.clone(),
            decl_file: source.path().to_path_buf(),
        });
    }
    out
}
