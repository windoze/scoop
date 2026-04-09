use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::resolve::Visibility;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::call::{
    check_fn_value_to_any_erasure_gate, check_nogc_boxing_gate, infer_effect_op_call_expr_type,
};
use super::entry::try_infer_fun_return_ty_from_block;
use super::infer::{
    ExpectedTypeFrom, infer_expr_type, infer_expr_type_in_expected_context, infer_handle_expr_type,
};
use super::member::infer_not_null_assert_expr_type;
use super::ops::{
    collect_unique_zero_arg_member_method_sig, is_integer_type,
    record_member_method_effects_as_performed, try_extract_nominal_fqn_and_args,
};
use super::util::{
    expr_kind_name, fmt_effect_row, join_overload_signatures, visibility_from_modifiers,
};

use super::{ASYNC_EFFECT_FQN, ExprTypeError, FunSigOwned, ProgramBoundaryKind, TASK_FQN};

use super::super::assignable::is_type_assignable;
use super::super::builtin_annotations::BuiltinAnnotationFlags;
use super::super::lower::TypeLowering;
use super::super::{val_pat, when_exhaustiveness, when_pat};

#[derive(Debug, Clone, Copy)]
struct CallTargetSig<'a> {
    sig: &'a FunSigOwned,
    /// `args[i]` 对应到 `sig.params[i + arg_param_offset]`。
    arg_param_offset: usize,
}

fn is_function_type(ty: TypeId, lower: &TypeLowering<'_>) -> bool {
    matches!(lower.type_kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
}

fn resolve_call_target_for_expr_stmt<'a>(
    source: &SourceFile,
    callee: &ast::Expr,
    lower: &TypeLowering<'_>,
    top_level_funs: &'a HashMap<String, Vec<FunSigOwned>>,
) -> Option<CallTargetSig<'a>> {
    match &callee.kind {
        ast::ExprKind::Ident(id) => {
            let resolved = id.resolved.as_ref()?;
            let ast::ResolvedValueRef::TopLevel { fqn } = resolved else {
                return None;
            };

            let sigs = top_level_funs.get(fqn)?;

            // 扩展函数不能以 `f(args...)` 的形式被直接调用：这里只考虑普通顶层函数候选。
            let mut direct_call_candidates = sigs.iter().filter(|s| !s.is_extension);
            let sig = direct_call_candidates.next()?;
            if direct_call_candidates.next().is_some() {
                return None;
            }

            Some(CallTargetSig {
                sig,
                arg_param_offset: 0,
            })
        }
        ast::ExprKind::MemberAccess { member, .. }
        | ast::ExprKind::SafeMemberAccess { member, .. } => {
            let callee_fqn = match member.resolved.as_ref() {
                Some(ast::ResolvedMemberRef::ExtensionFun { fqn }) => fqn.clone(),
                Some(ast::ResolvedMemberRef::Fun { .. })
                | Some(ast::ResolvedMemberRef::Value { .. })
                | Some(ast::ResolvedMemberRef::ExtensionValue { .. }) => return None,
                None => {
                    let name = source.slice(member.span);
                    if lower.pkg_prefix().is_empty() {
                        name.to_string()
                    } else {
                        format!("{}.{}", lower.pkg_prefix(), name)
                    }
                }
            };

            let sigs = top_level_funs.get(&callee_fqn)?;
            let mut ext_candidates = sigs.iter().filter(|s| s.is_extension);
            let sig = ext_candidates.next()?;
            if ext_candidates.next().is_some() {
                return None;
            }

            Some(CallTargetSig {
                sig,
                // 扩展调用：`receiver.member(args...)` 的第一个参数是 receiver。
                arg_param_offset: 1,
            })
        }
        _ => None,
    }
}

fn check_lambda_expr_stmt_body(
    source: &SourceFile,
    lam: &ast::LambdaExpr,
    allow_non_local_return: bool,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 说明：当前阶段 lambda 仍未完整 typecheck；这里仅复用现有的“语句层递归”逻辑来：
    // - 捕获非法 `return`（non-local return 门禁，T0444）
    // - 避免 lambda 内的局部声明污染外层作用域（clone 快照）
    let mut lambda_locals = locals.clone();
    let mut lambda_stable = stable_bindings.clone();
    let mut lambda_mutable = mutable_bindings.clone();
    let nested_expected_return_ty = if allow_non_local_return {
        expected_return_ty
    } else {
        None
    };

    // required effects（T0604）：lambda body 的 effect 属于该函数值，不计入外层函数立即执行的 effects。
    lower.with_effect_collection_suspended(|lower| {
        // `@NoGC`：lambda body 并不在外层函数执行时立即运行，不能把 `@NoGC` 的限制“向内传播”。
        lower.with_nogc_context_suspended(|lower| {
            check_expr_stmt(
                source,
                lam.body.as_ref(),
                lower,
                builtins,
                &mut lambda_locals,
                &mut lambda_stable,
                &mut lambda_mutable,
                0,
                nested_expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )
        })
    })
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
        // 因此其 effect row 必须是 `Pure`（不能显式声明 non-Pure，也不能通过 internal/private 推断出效果）。
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
    if matches!(program_boundary, ProgramBoundaryKind::None) {
        if let Some(expr) = fun.effects.as_ref() {
            let _ = lower.lower_effect_row_expr(Some(expr))?;
        }
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
    source: &SourceFile,
    fun_fqn: &str,
    fun: &ast::FunDecl,
    program_boundary: ProgramBoundaryKind,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &mut HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    lower.push_type_params(&fun.type_params);
    let eff_binding_pushed = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => match lower.lower_effect_row_expr(Some(expr)) {
                Ok(row) => row,
                Err(e) => {
                    lower.pop_type_params(&fun.type_params);
                    return Err(e.into());
                }
            },
            None => EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
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
                    let found_ty = infer_expr_type_in_expected_context(
                        source,
                        default_value,
                        ty,
                        ExpectedTypeFrom::new(format!(
                            "`{}` 的形参 `{}` 的默认值",
                            fun_fqn, param_name
                        )),
                        lower,
                        builtins,
                        &locals,
                        top_level_types,
                        &*top_level_funs,
                        struct_field_types,
                    )?;

                    if is_type_assignable(found_ty, ty, lower, builtins)
                        || (matches!(default_value.kind, ast::ExprKind::IntLit)
                            && is_integer_type(ty, lower, builtins))
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
                        let inferred = try_infer_fun_return_ty_from_block(
                            source,
                            b,
                            lower,
                            builtins,
                            &mut locals,
                            &mut stable_bindings,
                            &mut mutable_bindings,
                            0,
                            top_level_types,
                            &*top_level_funs,
                            member_mutabilities,
                            struct_field_types,
                        )?
                        .unwrap_or(builtins.unit);

                        // 回写到顶层函数签名表：使得后续同文件的调用点能看到推断后的返回类型。
                        if let Some(sigs) = top_level_funs.get_mut(fun_fqn) {
                            if let Some(sig) =
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
                        }

                        inferred
                    }
                    ast::FunBody::Missing => builtins.unit,
                },
            };

            match &fun.body {
                ast::FunBody::Block(b) => check_block_exprs(
                    source,
                    b,
                    lower,
                    builtins,
                    &mut locals,
                    &mut stable_bindings,
                    &mut mutable_bindings,
                    0,
                    Some(expected_return_ty),
                    top_level_types,
                    &*top_level_funs,
                    member_mutabilities,
                    struct_field_types,
                )?,
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
    lower.pop_type_params(&fun.type_params);
    result
}

pub(super) fn check_block_exprs(
    source: &SourceFile,
    block: &ast::Block,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 与 resolver 的作用域规则对齐：block 内声明仅在该 block 内可见。
    // 这里用“进入时快照 + 退出时回滚”的方式实现最小作用域，不引入额外的数据结构。
    let saved_locals = locals.clone();
    let saved_stable = stable_bindings.clone();
    let saved_mutable = mutable_bindings.clone();

    for stmt in &block.stmts {
        check_stmt_exprs(
            source,
            stmt,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        )?;
    }

    *locals = saved_locals;
    *stable_bindings = saved_stable;
    *mutable_bindings = saved_mutable;

    Ok(())
}

pub(super) fn check_stmt_exprs(
    source: &SourceFile,
    stmt: &ast::Stmt,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    match &stmt.kind {
        ast::StmtKind::Val(v) => check_local_val_decl_exprs(
            source,
            v,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?,
        ast::StmtKind::Expr(e) => check_expr_stmt(
            source,
            e,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        )?,
        ast::StmtKind::Return { return_span, value } => {
            let Some(expected) = expected_return_ty else {
                return Err(ExprTypeError::ReturnNotInFunctionBody {
                    span: (*return_span).into(),
                });
            };

            match value {
                Some(v) => {
                    let found = infer_expr_type_in_expected_context(
                        source,
                        v,
                        expected,
                        ExpectedTypeFrom::new("函数返回类型"),
                        lower,
                        builtins,
                        locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?;
                    if !is_type_assignable(found, expected, lower, builtins) {
                        return Err(ExprTypeError::ReturnTypeMismatch {
                            expected: lower.fmt_type(expected),
                            found: lower.fmt_type(found),
                            span: v.span.into(),
                        });
                    }
                    check_fn_value_to_any_erasure_gate(found, expected, v.span, lower, builtins)?;
                    check_nogc_boxing_gate(found, expected, v.span, lower, builtins)?;
                }
                None => {
                    // `return` 不带返回值：等价于返回 `Unit`。
                    if expected != builtins.unit {
                        return Err(ExprTypeError::ReturnValueRequired {
                            expected: lower.fmt_type(expected),
                            span: (*return_span).into(),
                        });
                    }
                }
            }
        }
        ast::StmtKind::While { cond, body, .. } => {
            let cond_ty = infer_expr_type(
                source,
                cond,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;

            if !is_type_assignable(cond_ty, builtins.bool_, lower, builtins) {
                return Err(ExprTypeError::WhileConditionNotBool {
                    found: lower.fmt_type(cond_ty),
                    span: cond.span.into(),
                });
            }

            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth + 1,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
        }
        ast::StmtKind::Break { break_span } => {
            if loop_depth == 0 {
                return Err(ExprTypeError::BreakNotInLoop {
                    span: (*break_span).into(),
                });
            }
        }
        ast::StmtKind::Continue { continue_span } => {
            if loop_depth == 0 {
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
            let iter_ty = infer_expr_type(
                source,
                &f.iter,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;

            let Some((iter_fqn, iter_args)) = try_extract_nominal_fqn_and_args(iter_ty, lower)
            else {
                return Err(ExprTypeError::ForMissingIteratorMethod {
                    found: lower.fmt_type(iter_ty),
                    span: f.iter.span.into(),
                });
            };

            // T0110：Array / IntProgression は .iterator() を持たないため、
            // 型特化で直接要素型を決定し、iterator protocol をバイパスする。
            use crate::ast::{ForLoopIterableKind, ForLoopResolvedInfo};

            let elem_ty = if iter_fqn == "scoop.core.Array"
                || iter_fqn == "scoop.core.MutableArray"
            {
                let _ = f.resolved_for_info.set(ForLoopResolvedInfo {
                    kind: ForLoopIterableKind::ArrayInt,
                });
                // Array<T> — 要素型は最初の型引数
                iter_args.first().copied().unwrap_or(builtins.any)
            } else if iter_fqn == "scoop.core.IntProgression" {
                let _ = f.resolved_for_info.set(ForLoopResolvedInfo {
                    kind: ForLoopIterableKind::IntProgression,
                });
                // IntProgression — 要素型は常に Int
                builtins.int
            } else {
                // Generic iterator protocol: xs.iterator().next(): Option<Elem>
                let Some(iterator_sig) = collect_unique_zero_arg_member_method_sig(
                    source,
                    iter_ty,
                    &iter_fqn,
                    &iter_args,
                    "iterator",
                    f.iter.span,
                    lower,
                    builtins,
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

                let Some(next_sig) = collect_unique_zero_arg_member_method_sig(
                    source,
                    iterator_ty,
                    &iterator_fqn,
                    &iterator_args,
                    "next",
                    f.iter.span,
                    lower,
                    builtins,
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
                });

                elem
            };

            // binder 仅在 body 作用域内可见：进入时注入，退出时回滚。
            let saved_locals = locals.clone();
            let saved_stable = stable_bindings.clone();
            let saved_mutable = mutable_bindings.clone();

            locals.insert(f.binder.span, elem_ty);
            stable_bindings.insert(f.binder.span);

            check_block_exprs(
                source,
                &f.body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth + 1,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;

            *locals = saved_locals;
            *stable_bindings = saved_stable;
            *mutable_bindings = saved_mutable;
        }
        ast::StmtKind::ComptimeBlock { body, .. } => {
            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
        }
        ast::StmtKind::ComptimeIf(ci) => {
            check_block_exprs(
                source,
                &ci.then_branch,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
            if let Some(else_branch) = &ci.else_branch {
                match &**else_branch {
                    ast::ComptimeIfElse::Block(b) => check_block_exprs(
                        source,
                        b,
                        lower,
                        builtins,
                        locals,
                        stable_bindings,
                        mutable_bindings,
                        loop_depth,
                        expected_return_ty,
                        top_level_types,
                        top_level_funs,
                        member_mutabilities,
                        struct_field_types,
                    )?,
                    ast::ComptimeIfElse::If(next) => {
                        // 递归跟进 else-if 链。
                        let mut cur: &ast::ComptimeIf = next;
                        loop {
                            check_block_exprs(
                                source,
                                &cur.then_branch,
                                lower,
                                builtins,
                                locals,
                                stable_bindings,
                                mutable_bindings,
                                loop_depth,
                                expected_return_ty,
                                top_level_types,
                                top_level_funs,
                                member_mutabilities,
                                struct_field_types,
                            )?;
                            match &cur.else_branch {
                                Some(e) => match &**e {
                                    ast::ComptimeIfElse::Block(b) => {
                                        check_block_exprs(
                                            source,
                                            b,
                                            lower,
                                            builtins,
                                            locals,
                                            stable_bindings,
                                            mutable_bindings,
                                            loop_depth,
                                            expected_return_ty,
                                            top_level_types,
                                            top_level_funs,
                                            member_mutabilities,
                                            struct_field_types,
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
            check_block_exprs(
                source,
                &cf.body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth + 1,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;
        }
        ast::StmtKind::Empty | ast::StmtKind::Missing => {}
    }

    Ok(())
}

pub(super) fn check_local_val_decl_exprs(
    source: &SourceFile,
    v: &ast::ValDecl,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    let declared_ty = match &v.ty {
        Some(ty_ref) => Some(lower.lower_type_ref(ty_ref)?),
        None => None,
    };
    let expected_from = match &v.binding {
        ast::ValBinding::Name(name) => {
            ExpectedTypeFrom::new(format!("局部绑定 `{}` 的类型注解", source.slice(name.span)))
        }
        ast::ValBinding::Pattern(_) => ExpectedTypeFrom::new("局部解构绑定的类型注解"),
    };

    // 先类型检查 initializer（语义：局部变量在其声明之后可见，因此 init 内不能引用自身）。
    let init_ty = match &v.init {
        Some(init) => Some(match declared_ty {
            Some(expected) => infer_expr_type_in_expected_context(
                source,
                init,
                expected,
                expected_from.clone(),
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?,
            None => infer_expr_type(
                source,
                init,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?,
        }),
        None => None,
    };

    if let (Some(expected), Some(found)) = (declared_ty, init_ty) {
        let init = v.init.as_ref().unwrap();
        if !is_type_assignable(found, expected, lower, builtins) {
            // 与顶层 initializer 一致：允许整数字面量被上下文整数类型吸收（后续可加入 range check）。
            if matches!(init.kind, ast::ExprKind::IntLit)
                && is_integer_type(expected, lower, builtins)
            {
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
        check_fn_value_to_any_erasure_gate(found, expected, init.span, lower, builtins)?;
        check_nogc_boxing_gate(found, expected, init.span, lower, builtins)?;
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
            locals.insert(name.span, ty);
            match v.kind {
                ast::ValKind::Val => {
                    stable_bindings.insert(name.span);
                }
                ast::ValKind::Var => {
                    mutable_bindings.insert(name.span);
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
                source,
                pat,
                init_ty,
                lower,
                builtins,
                struct_field_types,
            )?;

            // `val` 解构引入的绑定与普通 `val x = ...` 一样：
            // - 在其声明之后可见（resolver 已建立作用域）
            // - 属于稳定绑定，可用于 smart cast（当前阶段仅记录）
            for (decl_span, ty) in bindings {
                locals.insert(decl_span, ty);
                stable_bindings.insert(decl_span);
            }
        }
    }

    Ok(())
}

pub(super) fn check_expr_stmt(
    source: &SourceFile,
    expr: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &mut HashMap<Span, TypeId>,
    stable_bindings: &mut HashSet<Span>,
    mutable_bindings: &mut HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // 当前阶段的表达式语句仅用于支持控制流结构内部的“局部 val/var 推导”回归：
    // - `if (...) { val ... } else { ... }`
    // - `call { ... }`：递归进入 lambda body 捕获非法 `return`（T0444）
    //
    // 其他表达式语句（例如单独的调用）暂不强制 typecheck，以避免在未实现更多 ExprKind
    // 的阶段引入大量不相关的回归失败。
    match &expr.kind {
        ast::ExprKind::Block(b) => check_block_exprs(
            source,
            b,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        ),
        ast::ExprKind::UnsafeBlock { body, .. } => {
            lower.push_unsafe_context();
            let result = check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            );
            lower.pop_unsafe_context();
            result
        }
        ast::ExprKind::SafeBlock { body, .. } => lower.with_unsafe_context_suspended(|lower| {
            check_block_exprs(
                source,
                body,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )
        }),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => check_if_expr_stmt(
            source,
            cond.as_ref(),
            then_branch.as_ref(),
            else_branch.as_deref(),
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        ),
        ast::ExprKind::When { subject, arms } => {
            // `when` 表达式作为语句时：
            // - 递归进入分支 body，以覆盖其中的局部绑定/控制流；
            // - T0427：为每个 arm 建立独立的“局部类型表”快照，并注入 pattern binder 的类型。
            check_expr_stmt(
                source,
                subject.as_ref(),
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;

            let subject_ty = infer_expr_type(
                source,
                subject.as_ref(),
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )
            .ok();

            if let Some(subject_ty) = subject_ty {
                when_exhaustiveness::check_when_exhaustiveness(
                    source, expr, subject_ty, arms, lower, builtins,
                )?;
            }

            for arm in arms {
                let mut arm_locals = locals.clone();
                let mut arm_stable = stable_bindings.clone();
                let mut arm_mutable = mutable_bindings.clone();

                if let Some(subject_ty) = subject_ty {
                    for (decl_span, ty) in when_pat::infer_when_pat_bindings(
                        source, &arm.pat, subject_ty, lower, builtins,
                    )? {
                        arm_locals.insert(decl_span, ty);
                        arm_stable.insert(decl_span);
                    }
                }

                check_expr_stmt(
                    source,
                    &arm.body,
                    lower,
                    builtins,
                    &mut arm_locals,
                    &mut arm_stable,
                    &mut arm_mutable,
                    loop_depth,
                    expected_return_ty,
                    top_level_types,
                    top_level_funs,
                    member_mutabilities,
                    struct_field_types,
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
                source,
                expr,
                body,
                arms,
                finally.as_ref(),
                lower,
                builtins,
                &*locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
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
                source,
                inner.as_ref(),
                *op_span,
                lower,
                builtins,
                &*locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            Ok(())
        }
        ast::ExprKind::Cast { .. } => {
            // T0445：`x as T` 的失败语义会触发 `Raise<RuntimeError>`。
            // 与 `!!` 一样，它属于“立即执行的表达式”，即使出现在表达式语句位置也必须参与
            // required-effects 收集；否则 `/ Pure` 函数体内的 `as` 会被错误放过。
            match infer_expr_type(
                source,
                expr,
                lower,
                builtins,
                &*locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            ) {
                Ok(_) => Ok(()),
                Err(ExprTypeError::UnsupportedExpr { .. }) => Ok(()),
                Err(e) => Err(e),
            }
        }
        ast::ExprKind::Call { callee, args } => {
            // `@NoGC`：在表达式语句位置也必须强制检查调用点，
            // 否则会被 `call();` 这类“仅为副作用的调用”绕过门禁。
            if lower.in_nogc_context() {
                let _ = infer_expr_type(
                    source,
                    expr,
                    lower,
                    builtins,
                    &*locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            }

            // T0444：`inline` 与 non-local return 的最小语义门禁：
            // - 默认：lambda body 内出现 `return` 一律报错
            // - 例外：当该 lambda 是 inline 函数的“lambda 参数实参”时，允许 non-local return
            //
            // 注意：当前阶段不做完整的调用类型检查（包括 lambda 类型推导），这里只做结构化递归与门禁，
            // 以便在不引入更多 type inference 复杂度的前提下先把语义边界钉死。
            let target =
                resolve_call_target_for_expr_stmt(source, callee.as_ref(), lower, top_level_funs);

            // 递归进入 callee 与 args：保证 `f({ return ... })` 这类结构也能被覆盖。
            check_expr_stmt(
                source,
                callee.as_ref(),
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                loop_depth,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )?;

            for (idx, arg) in args.iter().enumerate() {
                let ast::ExprKind::Lambda(lam) = &arg.kind else {
                    check_expr_stmt(
                        source,
                        arg,
                        lower,
                        builtins,
                        locals,
                        stable_bindings,
                        mutable_bindings,
                        loop_depth,
                        expected_return_ty,
                        top_level_types,
                        top_level_funs,
                        member_mutabilities,
                        struct_field_types,
                    )?;
                    continue;
                };

                let allow_non_local_return = match target {
                    Some(t) if t.sig.is_inline => {
                        let param_idx = idx + t.arg_param_offset;
                        match t.sig.params.get(param_idx).copied() {
                            Some(ty) => is_function_type(ty, lower),
                            None => false,
                        }
                    }
                    _ => false,
                };

                check_lambda_expr_stmt_body(
                    source,
                    lam,
                    allow_non_local_return,
                    lower,
                    builtins,
                    locals,
                    stable_bindings,
                    mutable_bindings,
                    expected_return_ty,
                    top_level_types,
                    top_level_funs,
                    member_mutabilities,
                    struct_field_types,
                )?;
            }

            // required effects（T0604）：
            // call 作为“表达式语句”时，typecheck 默认不会对其做完整调用检查；
            // 但 effect op call（例如 `Raise.raise(e)`）属于“立即执行的 perform”，必须被记录。
            if let ast::ExprKind::MemberAccess { member, .. } = &callee.kind {
                let _ = infer_effect_op_call_expr_type(
                    source,
                    expr,
                    member,
                    args,
                    None,
                    lower,
                    builtins,
                    &*locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
            } else if let ast::ExprKind::TypeApply {
                callee: inner,
                args: type_args,
            } = &callee.kind
            {
                if let ast::ExprKind::MemberAccess { member, .. } = &inner.kind {
                    let lowered = type_args
                        .iter()
                        .map(|a| lower.lower_type_ref(a))
                        .collect::<Result<Vec<_>, _>>()?;

                    let _ = infer_effect_op_call_expr_type(
                        source,
                        expr,
                        member,
                        args,
                        Some(lowered.as_slice()),
                        lower,
                        builtins,
                        &*locals,
                        top_level_types,
                        top_level_funs,
                        struct_field_types,
                    )?;
                }
            }

            Ok(())
        }
        ast::ExprKind::Lambda(lam) => {
            // spec §7.3：默认不允许 lambda non-local return。
            //
            // 例外：当 lambda 作为 inline 函数调用的 lambda 实参时允许（见 `ExprKind::Call` 分支）。
            check_lambda_expr_stmt_body(
                source,
                lam,
                false,
                lower,
                builtins,
                locals,
                stable_bindings,
                mutable_bindings,
                expected_return_ty,
                top_level_types,
                top_level_funs,
                member_mutabilities,
                struct_field_types,
            )
        }
        ast::ExprKind::Assign { lhs, rhs, .. } => check_assign_expr_stmt(
            source,
            lhs,
            rhs,
            lower,
            builtins,
            locals,
            stable_bindings,
            mutable_bindings,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        ),
        _ => Ok(()),
    }
}

fn check_if_expr_stmt(
    source: &SourceFile,
    cond: &ast::Expr,
    then_branch: &ast::Expr,
    else_branch: Option<&ast::Expr>,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    loop_depth: usize,
    expected_return_ty: Option<TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // smart cast（T0413）最小子集：仅识别 `if (x is T)` / `if (x !is T)` 形式，
    // 并且只对“稳定绑定”（参数 + `val`）在对应分支内做类型收窄。
    let smart_cast = detect_smart_cast_for_if_condition(cond, lower, locals, stable_bindings)?;

    // then 分支：在 `x is T` 时收窄；在 `x !is T` 时保持原类型。
    let mut then_locals = locals.clone();
    let mut then_stable = stable_bindings.clone();
    let mut then_mutable = mutable_bindings.clone();
    if let Some(smart_cast) = smart_cast {
        if smart_cast.narrow_in_then {
            then_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
        }
    }
    check_expr_stmt(
        source,
        then_branch,
        lower,
        builtins,
        &mut then_locals,
        &mut then_stable,
        &mut then_mutable,
        loop_depth,
        expected_return_ty,
        top_level_types,
        top_level_funs,
        member_mutabilities,
        struct_field_types,
    )?;

    // else 分支：在 `x !is T` 且存在 else 时收窄；否则保持原类型。
    if let Some(else_branch) = else_branch {
        let mut else_locals = locals.clone();
        let mut else_stable = stable_bindings.clone();
        let mut else_mutable = mutable_bindings.clone();
        if let Some(smart_cast) = smart_cast {
            if !smart_cast.narrow_in_then {
                else_locals.insert(smart_cast.decl_span, smart_cast.target_ty);
            }
        }

        check_expr_stmt(
            source,
            else_branch,
            lower,
            builtins,
            &mut else_locals,
            &mut else_stable,
            &mut else_mutable,
            loop_depth,
            expected_return_ty,
            top_level_types,
            top_level_funs,
            member_mutabilities,
            struct_field_types,
        )?;
    }

    Ok(())
}

fn check_assign_expr_stmt(
    source: &SourceFile,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    stable_bindings: &HashSet<Span>,
    mutable_bindings: &HashSet<Span>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    member_mutabilities: &HashMap<String, bool>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<(), ExprTypeError> {
    // T0443：赋值语句 `lhs = rhs` 最小规则：
    // - lhs 必须是可写目标：局部 `var` 绑定 或 可写属性（`var` property / ctor `var` param）
    // - rhs 类型必须可赋给 lhs（复用 `is_type_assignable` 的最小子类型/boxing 规则）
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
                    if stable_bindings.contains(decl_span) || !mutable_bindings.contains(decl_span)
                    {
                        return Err(ExprTypeError::AssignmentTargetNotMutable {
                            name: name.clone(),
                            span: id.span.into(),
                        });
                    }

                    let expected_ty = locals.get(decl_span).copied().ok_or_else(|| {
                        ExprTypeError::UnknownLocalValueType {
                            name: name.clone(),
                            span: id.span.into(),
                        }
                    })?;

                    expected_ty
                }
                ast::ResolvedValueRef::TopLevel { fqn } => {
                    let expected_ty = top_level_types.get(fqn).copied().ok_or_else(|| {
                        ExprTypeError::UnsupportedTopLevelValueType {
                            fqn: fqn.clone(),
                            span: id.span.into(),
                        }
                    })?;

                    if !lower.is_top_level_value_mutable(fqn) {
                        return Err(ExprTypeError::AssignmentTargetNotMutable {
                            name: source.slice(id.span).to_string(),
                            span: id.span.into(),
                        });
                    }

                    expected_ty
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
                let _ = infer_expr_type(
                    source,
                    receiver,
                    lower,
                    builtins,
                    locals,
                    top_level_types,
                    top_level_funs,
                    struct_field_types,
                )?;
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

            if !member_mutabilities.get(fqn).copied().unwrap_or(false) {
                return Err(ExprTypeError::AssignmentTargetNotMutable {
                    name: source.slice(member.span).to_string(),
                    span: member.span.into(),
                });
            }

            let expected_ty = struct_field_types.get(fqn).copied().ok_or_else(|| {
                ExprTypeError::UnsupportedMemberAccess {
                    fqn: fqn.clone(),
                    span: member.span.into(),
                }
            })?;

            expected_ty
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
        ast::ExprKind::Ident(id) => {
            ExpectedTypeFrom::new(format!("赋值目标 `{}` 的类型", source.slice(id.span)))
        }
        ast::ExprKind::MemberAccess { member, .. } => ExpectedTypeFrom::new(format!(
            "赋值目标 `{}` 的字段类型",
            source.slice(member.span)
        )),
        _ => ExpectedTypeFrom::new("赋值目标的类型"),
    };
    let found_ty = infer_expr_type_in_expected_context(
        source,
        rhs,
        expected_ty,
        expected_from,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    if !is_type_assignable(found_ty, expected_ty, lower, builtins) {
        // 与 initializer/call args 一致：允许整数字面量被上下文整数类型吸收（后续可加入 range check）。
        if matches!(rhs.kind, ast::ExprKind::IntLit)
            && is_integer_type(expected_ty, lower, builtins)
        {
            return Ok(());
        }
        return Err(ExprTypeError::AssignmentTypeMismatch {
            expected: lower.fmt_type(expected_ty),
            found: lower.fmt_type(found_ty),
            span: rhs.span.into(),
        });
    }

    Ok(())
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
