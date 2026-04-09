use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::resolve::{ConeId, Index, Visibility};
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::call::{
    CallArgInfo, CallArgKind, GenericArgConstraint, InstantiatedFunSig, check_const_fun_call_gate,
    check_fn_value_to_any_erasure_gate, check_nogc_boxing_gate, check_nogc_call_gate,
    check_unsafe_call_gate, instantiate_fun_sig_for_call, map_call_args_to_params_with_defaults,
    substitute_type_args_in_effect_row, type_param_name,
};
use super::infer::{ExpectedTypeFrom, infer_expr_type, infer_expr_type_in_expected_context};
use super::util::{fmt_overload_signature, join_overload_signatures};

use super::{ExprTypeError, FunSigOwned};

use super::super::assignable::{is_type_assignable, nominal_is_subtype_by_fqn};
use super::super::eff_row_subst::EffRowVarSubstPlan;
use super::super::lower::TypeLowering;

fn unary_op_text(op: ast::UnaryOp) -> &'static str {
    match op {
        ast::UnaryOp::Not => "!",
        ast::UnaryOp::Neg => "-",
        ast::UnaryOp::BitNot => "~",
    }
}

fn binary_op_text(op: ast::BinaryOp) -> &'static str {
    match op {
        ast::BinaryOp::Add => "+",
        ast::BinaryOp::Sub => "-",
        ast::BinaryOp::Mul => "*",
        ast::BinaryOp::Div => "/",
        ast::BinaryOp::Rem => "%",
        ast::BinaryOp::RangeInclusive => "..",
        ast::BinaryOp::Shl => "<<",
        ast::BinaryOp::Shr => ">>",
        ast::BinaryOp::BitAnd => "&",
        ast::BinaryOp::BitXor => "^",
        ast::BinaryOp::BitOr => "|",
        ast::BinaryOp::Lt => "<",
        ast::BinaryOp::Le => "<=",
        ast::BinaryOp::Gt => ">",
        ast::BinaryOp::Ge => ">=",
        ast::BinaryOp::Eq => "==",
        ast::BinaryOp::Ne => "!=",
        ast::BinaryOp::LogAnd => "&&",
        ast::BinaryOp::LogOr => "||",
        ast::BinaryOp::Elvis => "?:",
    }
}

pub(super) fn is_integer_type(
    ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if ty == builtins.int || ty == builtins.uint {
        return true;
    }

    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::IntN(_) | ValueTypeKind::UIntN(_)) => true,
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => matches!(
            nominal.fqn.as_str(),
            "scoop.core.Int8"
                | "scoop.core.Int16"
                | "scoop.core.Int32"
                | "scoop.core.Int64"
                | "scoop.core.UInt8"
                | "scoop.core.UInt16"
                | "scoop.core.UInt32"
                | "scoop.core.UInt64"
        ),
        _ => false,
    }
}

fn unify_integer_operands_for_same_type_rule(
    lhs: &ast::Expr,
    lhs_ty: TypeId,
    rhs: &ast::Expr,
    rhs_ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<TypeId> {
    if lhs_ty == rhs_ty && is_integer_type(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    if matches!(lhs.kind, ast::ExprKind::IntLit) && is_integer_type(rhs_ty, lower, builtins) {
        return Some(rhs_ty);
    }

    if matches!(rhs.kind, ast::ExprKind::IntLit) && is_integer_type(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    None
}

fn operator_overload_method_name(op: ast::BinaryOp) -> Option<&'static str> {
    match op {
        ast::BinaryOp::Add => Some("plus"),
        ast::BinaryOp::Sub => Some("minus"),
        ast::BinaryOp::BitAnd => Some("and"),
        ast::BinaryOp::BitOr => Some("or"),
        ast::BinaryOp::BitXor => Some("xor"),
        ast::BinaryOp::Shl => Some("shl"),
        ast::BinaryOp::Shr => Some("shr"),
        _ => None,
    }
}

fn builtin_binary_op_expected_text(op: ast::BinaryOp) -> &'static str {
    match op {
        ast::BinaryOp::Shl | ast::BinaryOp::Shr => "lhs 为整数且 rhs 为 Int",
        _ => "相同的整数类型",
    }
}

pub(super) fn is_symbol_visible_from_source(
    use_cone: ConeId,
    use_source: &SourceFile,
    symbol: &crate::resolve::Symbol,
) -> bool {
    match symbol.visibility {
        Visibility::Public => true,
        Visibility::Internal => symbol.decl_cone == use_cone,
        Visibility::Private => symbol.decl_file == use_source.path(),
    }
}

fn collect_nominal_type_param_bindings(
    nominal_fqn: &str,
    nominal_args: &[TypeId],
    lower: &TypeLowering<'_>,
) -> Vec<(String, TypeId)> {
    let Some(sym) = lower.env().type_symbol(nominal_fqn) else {
        return Vec::new();
    };

    if sym.type_param_names.len() != nominal_args.len() {
        return Vec::new();
    }

    sym.type_param_names
        .iter()
        .cloned()
        .zip(nominal_args.iter().copied())
        .collect()
}

pub(super) fn collect_member_method_signatures_from_index(
    source: &SourceFile,
    receiver_ty: TypeId,
    receiver_fqn: &str,
    receiver_args: &[TypeId],
    callee_fqn: &str,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<Vec<FunSigOwned>, ExprTypeError> {
    // 仅用于语法糖（operator overloading / for 协议）：限制范围，避免“隐式 receiver 注入”
    // 扩散到普通 member call 路径（普通 `a.b()` 仍走 extension fun 的最小子集）。
    let overloads = match lower.index().by_fqn.get(callee_fqn) {
        Some(syms) => syms.fun.clone(),
        None => Vec::new(),
    };
    if overloads.is_empty() {
        return Ok(Vec::new());
    }

    let use_cone = lower.index().cone_of_source(source);
    let base_type_bindings =
        collect_nominal_type_param_bindings(receiver_fqn, receiver_args, lower);

    let mut out: Vec<FunSigOwned> = Vec::new();
    for o in overloads {
        if !is_symbol_visible_from_source(use_cone, source, &o.symbol) {
            continue;
        }

        // 只允许“类型体内的普通成员方法”：不支持 extension receiver，也不支持 effect op。
        if o.sig.kind != ast::FunDeclKind::Regular {
            continue;
        }
        if o.sig.receiver.is_some() {
            continue;
        }

        // 当前阶段的泛型调用推断仅支持单一 type param；与跨文件顶层函数调用保持一致。
        if o.sig.type_params.len() > 1 {
            continue;
        }

        // operator overloading 当前不接入 `<eff E>`（可在后续任务中补齐）。
        if o.sig.eff_param.is_some() {
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

        // 函数级 type params：用于在 lowering 阶段把 `T` 解析为 `TypeKind::Param`。
        let mut type_params: Vec<TypeId> = Vec::with_capacity(o.sig.type_params.len());
        let mut type_param_bindings: Vec<(String, TypeId)> = base_type_bindings.clone();
        for p in &o.sig.type_params {
            let ty = lower.ty_param_named(p.name.clone(), o.symbol.decl_file.clone(), p.name_span);
            type_param_bindings.push((p.name.clone(), ty));
            type_params.push(ty);
        }

        // NOTE: `Index::Symbol` 的 `ModifierSet` 当前只保留 override/继承语义所需的少量标记（T0439），
        // 不包含 `inline`。跨文件 member method 调用暂按 `inline = false` 处理即可。
        let is_inline = false;

        let mut param_names: Vec<String> = Vec::with_capacity(o.sig.params.len() + 1);
        let mut param_has_defaults: Vec<bool> = Vec::with_capacity(o.sig.params.len() + 1);
        let mut params: Vec<TypeId> = Vec::with_capacity(o.sig.params.len() + 1);

        // 隐式 receiver：作为第一个参数注入。
        param_names.push("<receiver>".to_string());
        param_has_defaults.push(false);
        params.push(receiver_ty);

        for p in &o.sig.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            param_names.push(p.name.clone());
            param_has_defaults.push(p.has_default);
            let ty = lower.lower_type_ref_in_decl_file_with_scopes(
                &o.symbol.decl_file,
                type_param_bindings.iter().cloned(),
                Vec::new(),
                ty_ref,
            )?;
            params.push(ty);
        }

        let return_ty = match &o.sig.return_ty {
            Some(ret) => lower.lower_type_ref_in_decl_file_with_scopes(
                &o.symbol.decl_file,
                type_param_bindings.iter().cloned(),
                Vec::new(),
                ret,
            )?,
            None => builtins.unit,
        };

        // 对 operator method：receiver 不参与 eff var substitution 推断，因此这里按“无基底/无替换”处理。
        let mut param_fn_effect_eff_base: Vec<Option<EffectRow>> = Vec::with_capacity(params.len());
        let mut param_nominal_eff_eff_base: Vec<Option<EffectRow>> =
            Vec::with_capacity(params.len());
        let mut param_eff_row_var_subst: Vec<EffRowVarSubstPlan> = Vec::with_capacity(params.len());

        param_fn_effect_eff_base.push(None);
        param_nominal_eff_eff_base.push(None);
        param_eff_row_var_subst.push(EffRowVarSubstPlan::None);

        for p in &o.sig.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            // operator overloading 当前不支持 `<eff E>`，因此这里不计算相关基底与替换计划。
            let _ = (ty_ref, &decl_source);
            param_fn_effect_eff_base.push(None);
            param_nominal_eff_eff_base.push(None);
            param_eff_row_var_subst.push(EffRowVarSubstPlan::None);
        }

        out.push(FunSigOwned {
            decl_span: o.symbol.span,
            decl_file: o.symbol.decl_file.clone(),
            is_extension: false,
            is_inline,
            is_const: o.sig.is_const,
            is_unsafe: o.sig.builtin_flags.is_unsafe,
            is_nogc: o.sig.builtin_flags.is_nogc,
            is_extern: o.sig.builtin_flags.is_extern,
            is_intrinsic: o.sig.builtin_flags.is_intrinsic,
            param_names,
            param_has_defaults,
            param_is_vararg: vec![false; params.len()],
            type_params,
            eff_param: None,
            param_fn_effect_eff_base,
            param_nominal_eff_eff_base,
            param_eff_row_var_subst,
            return_eff_row_var_subst: EffRowVarSubstPlan::None,
            params,
            return_ty,
            effects: o.sig.effects.clone(),
        });
    }

    Ok(out)
}

pub(super) fn try_extract_nominal_fqn_and_args(
    ty: TypeId,
    lower: &TypeLowering<'_>,
) -> Option<(String, Vec<TypeId>)> {
    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n)) => Some((n.fqn, n.args)),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => Some((n.fqn, n.args)),
        _ => None,
    }
}

pub(super) fn collect_unique_zero_arg_member_method_sig(
    source: &SourceFile,
    receiver_ty: TypeId,
    receiver_fqn: &str,
    receiver_args: &[TypeId],
    method: &str,
    call_site_span: Span,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<Option<FunSigOwned>, ExprTypeError> {
    let callee_fqn = format!("{receiver_fqn}.{method}");
    let sigs = collect_member_method_signatures_from_index(
        source,
        receiver_ty,
        receiver_fqn,
        receiver_args,
        &callee_fqn,
        lower,
        builtins,
    )?;

    // for/操作符语法糖当前只支持无实参（除隐式 receiver 外）的方法。
    let mut filtered: Vec<FunSigOwned> = sigs
        .into_iter()
        .filter(|s| s.params.len() == 1 && s.type_params.is_empty() && s.eff_param.is_none())
        .collect();

    match filtered.len() {
        0 => Ok(None),
        1 => Ok(Some(filtered.remove(0))),
        _ => {
            let candidates = filtered
                .iter()
                .map(|sig| {
                    let receiver_ty = sig.params.first().copied();
                    fmt_overload_signature(method, receiver_ty, &[], lower)
                })
                .collect::<Vec<_>>();
            Err(ExprTypeError::AmbiguousOverload {
                callee: callee_fqn,
                candidates: join_overload_signatures(candidates),
                span: call_site_span.into(),
            })
        }
    }
}

pub(super) fn record_member_method_effects_as_performed(
    receiver_fqn: &str,
    receiver_args: &[TypeId],
    sig: &FunSigOwned,
    call_site_span: Span,
    lower: &mut TypeLowering<'_>,
) -> Result<(), ExprTypeError> {
    let type_param_bindings =
        collect_nominal_type_param_bindings(receiver_fqn, receiver_args, lower);
    let call_effects = lower.lower_effect_row_expr_in_decl_file_with_bindings(
        &sig.decl_file,
        type_param_bindings,
        sig.effects.as_ref(),
    )?;

    for effect in call_effects.terms.iter().copied() {
        lower.record_performed_effect(effect, call_site_span);
    }

    Ok(())
}

pub(super) fn infer_unary_expr_type(
    source: &SourceFile,
    op: ast::UnaryOp,
    op_span: Span,
    operand: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let operand_ty = infer_expr_type(
        source,
        operand,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    match op {
        ast::UnaryOp::Not => {
            if operand_ty == builtins.bool_ {
                return Ok(builtins.bool_);
            }

            Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                op: unary_op_text(op).to_string(),
                expected: "Bool".to_string(),
                found: lower.fmt_type(operand_ty),
                span: op_span.into(),
            })
        }
        ast::UnaryOp::Neg => {
            if is_integer_type(operand_ty, lower, builtins) {
                return Ok(operand_ty);
            }

            Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                op: unary_op_text(op).to_string(),
                expected: "整数".to_string(),
                found: lower.fmt_type(operand_ty),
                span: op_span.into(),
            })
        }
        ast::UnaryOp::BitNot => {
            if is_integer_type(operand_ty, lower, builtins) {
                return Ok(operand_ty);
            }

            let Some((receiver_fqn, receiver_args)) =
                try_extract_nominal_fqn_and_args(operand_ty, lower)
            else {
                return Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                    op: unary_op_text(op).to_string(),
                    expected: "整数".to_string(),
                    found: lower.fmt_type(operand_ty),
                    span: op_span.into(),
                });
            };

            // 只对 struct/class 启用 operator overloading（T1301 目标约束）。
            if !matches!(
                lower.nominal_decl_kind(&receiver_fqn),
                Some(ast::TypeKind::Struct | ast::TypeKind::Class)
            ) {
                return Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                    op: unary_op_text(op).to_string(),
                    expected: "整数".to_string(),
                    found: lower.fmt_type(operand_ty),
                    span: op_span.into(),
                });
            }

            let method = "inv";
            let callee_fqn = format!("{receiver_fqn}.{method}");
            let Some(sig) = collect_unique_zero_arg_member_method_sig(
                source,
                operand_ty,
                &receiver_fqn,
                &receiver_args,
                method,
                op_span,
                lower,
                builtins,
            )?
            else {
                return Err(ExprTypeError::UnaryOperatorOverloadNotFound {
                    op: unary_op_text(op).to_string(),
                    receiver: lower.fmt_type(operand_ty),
                    method: method.to_string(),
                    span: op_span.into(),
                });
            };

            // operator method 调用：禁止 unsafe/nogc/const 门禁绕过，沿用普通调用的 gate。
            check_unsafe_call_gate(&callee_fqn, &sig, op_span, lower)?;
            check_nogc_call_gate(&callee_fqn, &sig, op_span, lower)?;
            check_const_fun_call_gate(&callee_fqn, &sig, op_span, lower)?;

            // required effects：把被调用方法的 effect row 计入当前函数体的 performed effects。
            record_member_method_effects_as_performed(
                &receiver_fqn,
                &receiver_args,
                &sig,
                op_span,
                lower,
            )?;

            Ok(sig.return_ty)
        }
    }
}

pub(super) fn infer_operator_overload_binary_expr_type(
    source: &SourceFile,
    binary_expr: &ast::Expr,
    lhs: &ast::Expr,
    op: ast::BinaryOp,
    op_span: Span,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = infer_expr_type(
        source,
        lhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    // Kotlin-like：对整数保留内建规则（避免要求 sysroot 的 Int/Int8/... 必须定义 `plus/and/shl/...`）。
    if is_integer_type(lhs_ty, lower, builtins) {
        let rhs_ty = infer_expr_type(
            source,
            rhs,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;

        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => {
                // 注意：复用 `unify_integer_operands_for_same_type_rule` 允许整数字面量被上下文整数类型吸收。
                let Some(ty) = unify_integer_operands_for_same_type_rule(
                    lhs, lhs_ty, rhs, rhs_ty, lower, builtins,
                ) else {
                    return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
                        op: binary_op_text(op).to_string(),
                        expected: "相同的整数类型".to_string(),
                        lhs: lower.fmt_type(lhs_ty),
                        rhs: lower.fmt_type(rhs_ty),
                        span: op_span.into(),
                    });
                };

                return Ok(ty);
            }
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if rhs_ty == builtins.int {
                    return Ok(lhs_ty);
                }

                return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
                    op: binary_op_text(op).to_string(),
                    expected: "lhs 为整数且 rhs 为 Int".to_string(),
                    lhs: lower.fmt_type(lhs_ty),
                    rhs: lower.fmt_type(rhs_ty),
                    span: op_span.into(),
                });
            }
            _ => {}
        }
    }

    let Some(method) = operator_overload_method_name(op) else {
        return Err(ExprTypeError::UnsupportedExpr {
            kind: "binary operator overload（unsupported op）",
            span: op_span.into(),
        });
    };

    let (receiver_fqn, receiver_args) = match lower.type_kind(lhs_ty) {
        TypeKind::Value(ValueTypeKind::Nominal(n)) => (n.fqn, n.args),
        TypeKind::Ref(RefTypeKind::Nominal(n)) => (n.fqn, n.args),
        _ => {
            let rhs_ty = infer_expr_type(
                source,
                rhs,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
                op: binary_op_text(op).to_string(),
                expected: builtin_binary_op_expected_text(op).to_string(),
                lhs: lower.fmt_type(lhs_ty),
                rhs: lower.fmt_type(rhs_ty),
                span: op_span.into(),
            });
        }
    };

    // 只对 struct/class 启用 operator overloading（T1301 目标约束）。
    if !matches!(
        lower.nominal_decl_kind(&receiver_fqn),
        Some(ast::TypeKind::Struct | ast::TypeKind::Class)
    ) {
        let rhs_ty = infer_expr_type(
            source,
            rhs,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
        return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
            op: binary_op_text(op).to_string(),
            expected: builtin_binary_op_expected_text(op).to_string(),
            lhs: lower.fmt_type(lhs_ty),
            rhs: lower.fmt_type(rhs_ty),
            span: op_span.into(),
        });
    }

    let callee_fqn = format!("{receiver_fqn}.{method}");
    let sigs = collect_member_method_signatures_from_index(
        source,
        lhs_ty,
        &receiver_fqn,
        &receiver_args,
        &callee_fqn,
        lower,
        builtins,
    )?;

    if sigs.is_empty() {
        let rhs_ty = infer_expr_type(
            source,
            rhs,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
        return Err(ExprTypeError::OperatorOverloadNotFound {
            op: binary_op_text(op).to_string(),
            receiver: lower.fmt_type(lhs_ty),
            method: method.to_string(),
            rhs: lower.fmt_type(rhs_ty),
            span: op_span.into(),
        });
    }

    // operator overloading 的 call args：隐式 receiver + rhs（仅位置实参）。
    let rhs_ty_for_selection = match rhs.kind {
        ast::ExprKind::Lambda(_) => builtins.any,
        _ => infer_expr_type(
            source,
            rhs,
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?,
    };
    let call_args: Vec<CallArgInfo<'_>> = vec![
        CallArgInfo {
            kind: CallArgKind::Positional,
            expr: lhs,
            ty: lhs_ty,
            is_int_lit: matches!(lhs.kind, ast::ExprKind::IntLit),
            is_spread: false,
        },
        CallArgInfo {
            kind: CallArgKind::Positional,
            expr: rhs,
            ty: rhs_ty_for_selection,
            is_int_lit: matches!(rhs.kind, ast::ExprKind::IntLit),
            is_spread: false,
        },
    ];

    let mut matched: Vec<(FunSigOwned, InstantiatedFunSig)> = Vec::new();
    for sig in sigs.iter() {
        // operator method 调用：禁止 unsafe/nogc/const 门禁绕过，沿用普通调用的 gate。
        check_unsafe_call_gate(&callee_fqn, sig, binary_expr.span, lower)?;
        check_nogc_call_gate(&callee_fqn, sig, binary_expr.span, lower)?;
        check_const_fun_call_gate(&callee_fqn, sig, binary_expr.span, lower)?;

        let Some(mapping) = map_call_args_to_params_with_defaults(
            &call_args,
            &sig.param_names,
            &sig.param_has_defaults,
        ) else {
            continue;
        };

        let instantiated = instantiate_fun_sig_for_call(
            &callee_fqn,
            binary_expr.span,
            sig,
            mapping
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(param_idx, arg_idx)| {
                    let Some(arg_idx) = arg_idx else {
                        return None;
                    };
                    let arg = &call_args[arg_idx];
                    Some(GenericArgConstraint {
                        expected: sig.params[param_idx],
                        found: arg.ty,
                        found_is_placeholder: matches!(arg.expr.kind, ast::ExprKind::Lambda(_)),
                        from: format!("第 {} 个实参", arg_idx + 1),
                        span: arg.expr.span,
                    })
                }),
            lower,
            builtins,
        )?;

        let mut ok = true;
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                ok = false;
                break;
            };
            let expected_ty = instantiated.params[param_idx];
            let arg = &call_args[arg_idx];
            let found_ty = arg.ty;

            if !is_type_assignable(found_ty, expected_ty, lower, builtins)
                && !(arg.is_int_lit && is_integer_type(expected_ty, lower, builtins))
            {
                ok = false;
                break;
            }
        }

        if ok {
            matched.push((sig.clone(), instantiated));
        }
    }

    let (sig, instantiated) = match matched.len() {
        0 => {
            let rhs_ty = infer_expr_type(
                source,
                rhs,
                lower,
                builtins,
                locals,
                top_level_types,
                top_level_funs,
                struct_field_types,
            )?;
            return Err(ExprTypeError::OperatorOverloadNotFound {
                op: binary_op_text(op).to_string(),
                receiver: lower.fmt_type(lhs_ty),
                method: method.to_string(),
                rhs: lower.fmt_type(rhs_ty),
                span: op_span.into(),
            });
        }
        1 => matched.remove(0),
        _ => {
            let candidates = matched
                .iter()
                .map(|(sig, _)| {
                    let receiver_ty = sig.params.first().copied();
                    fmt_overload_signature(
                        method,
                        receiver_ty,
                        sig.params.get(1..).unwrap_or_default(),
                        lower,
                    )
                })
                .collect::<Vec<_>>();
            return Err(ExprTypeError::AmbiguousOverload {
                callee: callee_fqn,
                candidates: join_overload_signatures(candidates),
                span: op_span.into(),
            });
        }
    };

    // rhs 最终类型检查：在期望类型语境下覆盖（lambda 下推推断等）。
    if let Some(expected_rhs_ty) = instantiated.params.get(1).copied() {
        let found_rhs_ty = infer_expr_type_in_expected_context(
            source,
            rhs,
            expected_rhs_ty,
            ExpectedTypeFrom::new(format!(
                "`{}` 的第 2 个形参 `{}`",
                callee_fqn,
                sig.param_names
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "<arg>".to_string())
            )),
            lower,
            builtins,
            locals,
            top_level_types,
            top_level_funs,
            struct_field_types,
        )?;
        if !is_type_assignable(found_rhs_ty, expected_rhs_ty, lower, builtins)
            && !(matches!(rhs.kind, ast::ExprKind::IntLit)
                && is_integer_type(expected_rhs_ty, lower, builtins))
        {
            return Err(ExprTypeError::OperatorOverloadNotFound {
                op: binary_op_text(op).to_string(),
                receiver: lower.fmt_type(lhs_ty),
                method: method.to_string(),
                rhs: lower.fmt_type(found_rhs_ty),
                span: op_span.into(),
            });
        }

        check_fn_value_to_any_erasure_gate(
            found_rhs_ty,
            expected_rhs_ty,
            rhs.span,
            lower,
            builtins,
        )?;
        check_nogc_boxing_gate(found_rhs_ty, expected_rhs_ty, rhs.span, lower, builtins)?;
    }

    // required effects：把被调用方法的 effect row 计入当前函数体的 performed effects。
    let mut type_param_bindings =
        collect_nominal_type_param_bindings(&receiver_fqn, &receiver_args, lower);
    for p in sig.type_params.iter().copied() {
        type_param_bindings.push((type_param_name(p, lower), p));
    }
    let lowered_effects = lower.lower_effect_row_expr_in_decl_file_with_bindings(
        &sig.decl_file,
        type_param_bindings,
        sig.effects.as_ref(),
    );
    let call_effects = substitute_type_args_in_effect_row(
        lowered_effects?,
        &sig.type_params,
        &instantiated.type_args,
        lower,
        binary_expr.span,
    )?;
    for effect in call_effects.terms.iter().copied() {
        lower.record_performed_effect(effect, binary_expr.span);
    }

    Ok(instantiated.return_ty)
}

pub(super) fn infer_builtin_scalar_binary_expr_type(
    source: &SourceFile,
    lhs: &ast::Expr,
    op: ast::BinaryOp,
    op_span: Span,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    locals: &HashMap<Span, TypeId>,
    top_level_types: &HashMap<String, TypeId>,
    top_level_funs: &HashMap<String, Vec<FunSigOwned>>,
    struct_field_types: &HashMap<String, TypeId>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = infer_expr_type(
        source,
        lhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;
    let rhs_ty = infer_expr_type(
        source,
        rhs,
        lower,
        builtins,
        locals,
        top_level_types,
        top_level_funs,
        struct_field_types,
    )?;

    let mismatch = |expected: &'static str| ExprTypeError::BinaryOpOperandTypeMismatch {
        op: binary_op_text(op).to_string(),
        expected: expected.to_string(),
        lhs: lower.fmt_type(lhs_ty),
        rhs: lower.fmt_type(rhs_ty),
        span: op_span.into(),
    };

    match op {
        // arithmetic: T op T -> T
        ast::BinaryOp::Add
        | ast::BinaryOp::Sub
        | ast::BinaryOp::Mul
        | ast::BinaryOp::Div
        | ast::BinaryOp::Rem
        // bitwise: T op T -> T
        | ast::BinaryOp::BitAnd
        | ast::BinaryOp::BitXor
        | ast::BinaryOp::BitOr => {
            let Some(ty) =
                unify_integer_operands_for_same_type_rule(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
            else {
                return Err(mismatch("相同的整数类型"));
            };
            Ok(ty)
        }
        // shifts: T << Int -> T
        ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
            if is_integer_type(lhs_ty, lower, builtins) && rhs_ty == builtins.int {
                return Ok(lhs_ty);
            }
            Err(mismatch("lhs 为整数且 rhs 为 Int"))
        }
        // comparisons: T < T -> Bool
        ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
            if unify_integer_operands_for_same_type_rule(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
                .is_some()
            {
                return Ok(builtins.bool_);
            }
            Err(mismatch("相同的整数类型"))
        }
        // equality: (T == T) -> Bool; (Bool == Bool) -> Bool; (String == String) -> Bool
        ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
            if lhs_ty == builtins.bool_ && rhs_ty == builtins.bool_ {
                return Ok(builtins.bool_);
            }
            // T0107: String == String
            if lhs_ty == builtins.string && rhs_ty == builtins.string {
                return Ok(builtins.bool_);
            }
            if unify_integer_operands_for_same_type_rule(lhs, lhs_ty, rhs, rhs_ty, lower, builtins)
                .is_some()
            {
                return Ok(builtins.bool_);
            }
            Err(mismatch("相同的整数类型、Bool 或 String"))
        }
        // boolean logic: Bool op Bool -> Bool
        ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
            if lhs_ty == builtins.bool_ && rhs_ty == builtins.bool_ {
                return Ok(builtins.bool_);
            }
            Err(mismatch("Bool"))
        }

        // elvis handled by caller
        ast::BinaryOp::Elvis => Err(ExprTypeError::UnsupportedExpr {
            kind: "elvis expression（internal）",
            span: op_span.into(),
        }),

        // range/progression（Appendix B.12）：语义由 stdlib/lowering 补齐；v0 先放行以服务 comptime for（T1207）。
        ast::BinaryOp::RangeInclusive => Ok(builtins.any),
    }
}
