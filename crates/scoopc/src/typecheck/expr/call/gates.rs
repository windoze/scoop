//! Call-site gate checks: unsafe, var-param lvalue, nogc, const-fun, deprecated, fn-value-to-Any erasure.

#![allow(dead_code)]

use super::*;

pub(in crate::typecheck::expr) fn check_unsafe_call_gate(
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

pub(super) fn check_var_param_lvalue_gate(
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

pub(in crate::typecheck::expr) fn check_nogc_call_gate(
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

pub(in crate::typecheck::expr) fn check_const_fun_call_gate(
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

pub(super) fn is_const_eval_builtin_fun(callee_fqn: &str) -> bool {
    matches!(
        callee_fqn,
        "scoop.core.substring"
            | "scoop.core.String.trimIndent"
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

pub(super) fn emit_deprecated_call_warning(
    callee_fqn: &str,
    sig: &FunSigOwned,
    call_span: Span,
    lower: &TypeLowering<'_>,
) {
    lower.emit_deprecated_fun_use(callee_fqn, &sig.decl_file, sig.decl_span, call_span);
}

pub(in crate::typecheck::expr) fn check_fn_value_to_any_erasure_gate(
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

pub(in crate::typecheck::expr) fn check_nogc_boxing_gate(
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

