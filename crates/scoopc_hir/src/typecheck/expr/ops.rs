use crate::ast;
use crate::cone::ConeId;
use crate::resolve::Visibility;
use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::float_literal::{FloatLiteralSuffix, parse_float_literal};
use crate::syntax::int_literal::{IntLiteralSuffix, parse_int_literal_suffix};
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, ValueTypeKind,
};

use super::call::{
    CallArgInfo, CallArgKind, GenericArgConstraint, InstantiatedFunSig,
    check_fn_value_to_any_erasure_gate, check_fun_where_constraints_after_instantiation,
    check_nogc_boxing_gate, check_nogc_call_gate, check_unsafe_call_gate,
    collect_member_method_signature_groups_from_receiver_ty, combined_member_instance_type_args,
    default_eff_arg_for_fun_sig, format_ambiguous_specificity_candidates,
    format_candidate_location, instantiate_eff_row_var_in_sig_types, instantiate_fun_sig_for_call,
    map_call_args_to_params_with_defaults, pick_most_specific_overload,
    specificity_candidate_for_fun_sig, substitute_type_args_in_effect_row, type_param_name,
    type_ref_fn_effect_eff_base, type_ref_nominal_eff_eff_base,
};
use super::infer::ExpectedTypeFrom;
use super::util::{fmt_overload_signature, join_overload_signatures};

use super::{EffParamSig, ExprInferInputs, ExprTypeError, FunSigOwned};

use super::super::assignable::is_type_assignable;
use super::super::eff_row_subst::{EffRowVarSubstPlan, build_eff_row_var_subst_plan};
use super::super::int_literals::check_negated_int_literal_for_type;
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

pub(super) fn is_float_type(ty: TypeId, lower: &TypeLowering<'_>, builtins: BuiltinTypes) -> bool {
    if ty == builtins.float64 || ty == builtins.float32 {
        return true;
    }

    matches!(
        lower.type_kind(ty),
        TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32)
    )
}

fn is_char_type(ty: TypeId, builtins: BuiltinTypes) -> bool {
    ty == builtins.char_
}

fn progression_ty_for_integer_ty(ty: TypeId, lower: &mut TypeLowering<'_>) -> Option<TypeId> {
    let fqn = match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Int) => "scoop.core.IntProgression",
        TypeKind::Value(ValueTypeKind::UInt) => "scoop.core.UIntProgression",
        TypeKind::Value(ValueTypeKind::IntN(64)) => "scoop.core.LongProgression",
        TypeKind::Value(ValueTypeKind::UIntN(64)) => "scoop.core.ULongProgression",
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.Int64" => {
            "scoop.core.LongProgression"
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.UInt64" => {
            "scoop.core.ULongProgression"
        }
        _ => return None,
    };

    Some(
        lower.intern_type_kind(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
            fqn: fqn.to_string(),
            args: Vec::new(),
            eff: None,
        }))),
    )
}

fn unsuffixed_int_literal(expr: &ast::Expr, source: &SourceFile) -> bool {
    matches!(expr.kind, ast::ExprKind::IntLit)
        && parse_int_literal_suffix(source.slice(expr.span)) == IntLiteralSuffix::None
}

fn is_unsuffixed_float_literal(expr: &ast::Expr, source: &SourceFile) -> bool {
    matches!(expr.kind, ast::ExprKind::FloatLit)
        && matches!(
            parse_float_literal(source.slice(expr.span)).suffix,
            FloatLiteralSuffix::Float64
        )
}

fn block_tail_expr(block: &ast::Block) -> Option<&ast::Expr> {
    let last = block.stmts.last()?;
    // T3102: semicolon-terminated expression statements are NOT tail values.
    if last.has_trailing_semi {
        return None;
    }
    match &last.kind {
        ast::StmtKind::Expr(expr) => Some(expr),
        _ => None,
    }
}

pub(super) fn literal_absorbs_to_expected(
    expr: &ast::Expr,
    expected_ty: TypeId,
    source: &SourceFile,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    match &expr.kind {
        ast::ExprKind::IntLit => {
            unsuffixed_int_literal(expr, source) && is_integer_type(expected_ty, lower, builtins)
        }
        ast::ExprKind::FloatLit => {
            is_unsuffixed_float_literal(expr, source) && expected_ty == builtins.float32
        }
        ast::ExprKind::Block(block) | ast::ExprKind::DoBlock { body: block, .. } => {
            block_tail_expr(block).is_some_and(|tail| {
                literal_absorbs_to_expected(tail, expected_ty, source, lower, builtins)
            })
        }
        ast::ExprKind::UnsafeBlock { body, .. } | ast::ExprKind::SafeBlock { body, .. } => {
            block_tail_expr(body).is_some_and(|tail| {
                literal_absorbs_to_expected(tail, expected_ty, source, lower, builtins)
            })
        }
        _ => false,
    }
}

fn unify_integer_operands_for_same_type_rule(
    lhs: &ast::Expr,
    lhs_ty: TypeId,
    rhs: &ast::Expr,
    rhs_ty: TypeId,
    source: &SourceFile,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<TypeId> {
    if lhs_ty == rhs_ty && is_integer_type(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    if unsuffixed_int_literal(lhs, source) && is_integer_type(rhs_ty, lower, builtins) {
        return Some(rhs_ty);
    }

    if unsuffixed_int_literal(rhs, source) && is_integer_type(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    None
}

fn unify_float_operands_for_same_type_rule(
    lhs: &ast::Expr,
    lhs_ty: TypeId,
    rhs: &ast::Expr,
    rhs_ty: TypeId,
    source: &SourceFile,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<TypeId> {
    if lhs_ty == rhs_ty && is_float_type(lhs_ty, lower, builtins) {
        return Some(lhs_ty);
    }

    if literal_absorbs_to_expected(lhs, rhs_ty, source, lower, builtins)
        && is_float_type(rhs_ty, lower, builtins)
    {
        return Some(rhs_ty);
    }

    if literal_absorbs_to_expected(rhs, lhs_ty, source, lower, builtins)
        && is_float_type(lhs_ty, lower, builtins)
    {
        return Some(lhs_ty);
    }

    None
}

fn operator_overload_method_name(op: ast::BinaryOp) -> Option<&'static str> {
    match op {
        ast::BinaryOp::Add => Some("plus"),
        ast::BinaryOp::Sub => Some("minus"),
        ast::BinaryOp::Mul => Some("times"),
        ast::BinaryOp::Div => Some("div"),
        ast::BinaryOp::Rem => Some("rem"),
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

fn scalar_operator_method_name(op: ast::BinaryOp) -> Option<&'static str> {
    match op {
        ast::BinaryOp::Add => Some("plus"),
        ast::BinaryOp::Sub => Some("minus"),
        ast::BinaryOp::Mul => Some("times"),
        ast::BinaryOp::Div => Some("div"),
        ast::BinaryOp::Rem => Some("rem"),
        ast::BinaryOp::BitAnd => Some("and"),
        ast::BinaryOp::BitOr => Some("or"),
        ast::BinaryOp::BitXor => Some("xor"),
        ast::BinaryOp::Shl => Some("shl"),
        ast::BinaryOp::Shr => Some("shr"),
        ast::BinaryOp::Lt => Some("lt"),
        ast::BinaryOp::Le => Some("le"),
        ast::BinaryOp::Gt => Some("gt"),
        ast::BinaryOp::Ge => Some("ge"),
        ast::BinaryOp::Eq => Some("equals"),
        ast::BinaryOp::Ne => Some("notEquals"),
        ast::BinaryOp::RangeInclusive
        | ast::BinaryOp::LogAnd
        | ast::BinaryOp::LogOr
        | ast::BinaryOp::Elvis => None,
    }
}

fn filter_operator_positioned_candidates(
    sigs: Vec<FunSigOwned>,
    op: &str,
    receiver: String,
    method: &str,
    span: Span,
) -> Result<Vec<FunSigOwned>, ExprTypeError> {
    let had_same_named_candidates = !sigs.is_empty();
    let out: Vec<FunSigOwned> = sigs.into_iter().filter(|sig| sig.is_operator).collect();
    if had_same_named_candidates && out.is_empty() {
        return Err(ExprTypeError::OperatorModifierRequired {
            op: op.to_string(),
            receiver,
            method: method.to_string(),
            span: span.into(),
        });
    }
    Ok(out)
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

        let mut param_names: Vec<String> = Vec::with_capacity(o.sig.params.len() + 1);
        let mut param_has_defaults: Vec<bool> = Vec::with_capacity(o.sig.params.len() + 1);
        let mut param_is_vararg: Vec<bool> = Vec::with_capacity(o.sig.params.len() + 1);
        let mut params: Vec<TypeId> = Vec::with_capacity(o.sig.params.len() + 1);

        // 隐式 receiver：作为第一个参数注入。
        param_names.push("<receiver>".to_string());
        param_has_defaults.push(false);
        param_is_vararg.push(false);
        params.push(receiver_ty);

        for p in &o.sig.params {
            let Some(ty_ref) = &p.ty else {
                continue;
            };
            param_names.push(p.name.clone());
            param_has_defaults.push(p.has_default);
            param_is_vararg.push(p.is_vararg);
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

        // 成员方法的隐式 receiver 不直接携带函数级 `eff` row 变量，因此第 0 个 receiver 参数
        // 保持“无基底/无替换”；其余显式参数则按顶层/扩展函数相同规则构建 effect facts。
        let mut param_fn_effect_eff_base: Vec<Option<EffectRow>> = Vec::with_capacity(params.len());
        let mut param_nominal_eff_eff_base: Vec<Option<EffectRow>> =
            Vec::with_capacity(params.len());
        let mut param_eff_row_var_subst: Vec<EffRowVarSubstPlan> = Vec::with_capacity(params.len());

        param_fn_effect_eff_base.push(None);
        param_nominal_eff_eff_base.push(None);
        param_eff_row_var_subst.push(EffRowVarSubstPlan::None);

        let mut param_pos = 1usize;
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
            param_fn_effect_eff_base.push(fn_eff_base);
            param_nominal_eff_eff_base.push(nominal_eff_base);
            param_eff_row_var_subst.push(subst_plan);
        }

        // T0129：member method 签名收集中也填充 where_constraints。
        let where_constraints = super::collect::build_fun_where_constraints_from_resolve_sig(
            &decl_source,
            &o.sig.type_params,
            o.sig.where_clause.as_ref(),
        );

        out.push(FunSigOwned {
            decl_span: o.symbol.span,
            decl_file: o.symbol.decl_file.clone(),
            is_extension: false,
            is_operator: o.symbol.modifiers.operator,
            is_unsafe: o.sig.builtin_flags.is_unsafe,
            is_nogc: o.sig.builtin_flags.is_nogc,
            is_extern: o.sig.builtin_flags.is_extern,
            is_intrinsic: o.sig.builtin_flags.is_intrinsic,
            intrinsic_entry_name: o.sig.builtin_flags.intrinsic_entry_name.clone(),
            param_names,
            param_has_defaults,
            param_is_vararg,
            type_params,
            eff_param: eff_param_sig,
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

/// P4-T01l：把 builtin scalar / `String` 的 nominal FQN 提取统一进 typecheck member-call 主线。
///
/// 说明：
/// - `try_extract_nominal_fqn_and_args` 只识别 `TypeKind::Value(ValueTypeKind::Nominal)` /
///   `TypeKind::Ref(RefTypeKind::Nominal)`，对 `Bool/Char/Int/Float32/Float64` / `String`
///   返回 `None`，导致 `42.toString()` / `true.toString()` 即使在 sysroot 已经声明
///   `Int.toString` body method 后，late-resolve 仍然不会把 receiver `Int` 映射到
///   `scoop.core.Int.toString` direct-call。
/// - 本 helper 把以上 builtin 形态映射到 `scoop.core.X` FQN（无 type args），
///   让 builtin scalar receiver 也能进入 nominal member-call 主线。
/// - 与 `try_extract_nominal_fqn_and_args` 保持同构：返回 `(fqn, args)`，args 为空 vec。
pub(super) fn try_extract_member_call_receiver_fqn_and_args(
    ty: TypeId,
    lower: &TypeLowering<'_>,
) -> Option<(String, Vec<TypeId>)> {
    if let Some(found) = try_extract_nominal_fqn_and_args(ty, lower) {
        return Some(found);
    }
    let fqn = match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Bool) => "scoop.core.Bool",
        TypeKind::Value(ValueTypeKind::Char) => "scoop.core.Char",
        TypeKind::Value(ValueTypeKind::Int) => "scoop.core.Int",
        TypeKind::Value(ValueTypeKind::UInt) => "scoop.core.UInt",
        TypeKind::Value(ValueTypeKind::IntN(8)) => "scoop.core.Int8",
        TypeKind::Value(ValueTypeKind::IntN(16)) => "scoop.core.Int16",
        TypeKind::Value(ValueTypeKind::IntN(32)) => "scoop.core.Int32",
        TypeKind::Value(ValueTypeKind::IntN(64)) => "scoop.core.Int64",
        TypeKind::Value(ValueTypeKind::UIntN(8)) => "scoop.core.UInt8",
        TypeKind::Value(ValueTypeKind::UIntN(16)) => "scoop.core.UInt16",
        TypeKind::Value(ValueTypeKind::UIntN(32)) => "scoop.core.UInt32",
        TypeKind::Value(ValueTypeKind::UIntN(64)) => "scoop.core.UInt64",
        TypeKind::Value(ValueTypeKind::Float32) => "scoop.core.Float32",
        TypeKind::Value(ValueTypeKind::Float64) => "scoop.core.Float64",
        TypeKind::Ref(RefTypeKind::String) => "scoop.core.String",
        _ => return None,
    };
    Some((fqn.to_string(), Vec::new()))
}

#[derive(Clone, Copy)]
pub(super) struct NominalReceiverRef<'a> {
    pub(super) ty: TypeId,
    pub(super) fqn: &'a str,
    pub(super) args: &'a [TypeId],
}

pub(super) fn collect_unique_zero_arg_member_method_sig(
    source: &SourceFile,
    receiver: NominalReceiverRef<'_>,
    method: &str,
    call_site_span: Span,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<Option<FunSigOwned>, ExprTypeError> {
    let callee_fqn = format!("{}.{}", receiver.fqn, method);
    let sigs = collect_member_method_signatures_from_index(
        source,
        receiver.ty,
        receiver.fqn,
        receiver.args,
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

fn collect_unique_zero_arg_operator_member_method_sig(
    source: &SourceFile,
    receiver: NominalReceiverRef<'_>,
    method: &str,
    op_text: &str,
    call_site_span: Span,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<Option<FunSigOwned>, ExprTypeError> {
    let callee_fqn = format!("{}.{}", receiver.fqn, method);
    let sigs = collect_member_method_signatures_from_index(
        source,
        receiver.ty,
        receiver.fqn,
        receiver.args,
        &callee_fqn,
        lower,
        builtins,
    )?;
    let sigs = filter_operator_positioned_candidates(
        sigs,
        op_text,
        lower.fmt_type(receiver.ty),
        method,
        call_site_span,
    )?;

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

struct MemberDirectCallInstance<'a> {
    type_args: &'a [TypeId],
    eff_args: &'a [EffectRow],
    param_tys: &'a [TypeId],
    return_ty: TypeId,
}

fn record_member_direct_call_binding(
    lower: &mut TypeLowering<'_>,
    call_site_span: Span,
    callee_fqn: &str,
    sig: &FunSigOwned,
    receiver_ty: TypeId,
    instance: MemberDirectCallInstance<'_>,
) -> Result<(), ExprTypeError> {
    let type_args =
        combined_member_instance_type_args(callee_fqn, receiver_ty, instance.type_args, lower)?;
    lower.record_monomorph_call(
        callee_fqn.to_string(),
        &sig.decl_file,
        sig.decl_span,
        &type_args,
        instance.eff_args,
        call_site_span,
    );
    lower.record_top_level_fun_call_binding(
        call_site_span,
        ast::TopLevelFunCallBinding {
            fqn: callee_fqn.to_string(),
            decl_file: sig.decl_file.clone(),
            decl_span: sig.decl_span,
            is_intrinsic: sig.is_intrinsic,
            intrinsic_entry_name: sig.intrinsic_entry_name.clone(),
            param_tys: instance.param_tys.to_vec(),
            return_ty: Some(instance.return_ty),
            type_args,
            eff_args: instance.eff_args.to_vec(),
        },
    );
    Ok(())
}

fn record_scalar_operator_method_binding(
    inputs: ExprInferInputs<'_>,
    call_site_span: Span,
    receiver_ty: TypeId,
    method: &str,
    op_text: &str,
    explicit_args: &[(&ast::Expr, TypeId)],
    lower: &mut TypeLowering<'_>,
) -> Result<Option<TypeId>, ExprTypeError> {
    let Some((receiver_fqn, receiver_args)) =
        try_extract_member_call_receiver_fqn_and_args(receiver_ty, lower)
    else {
        return Ok(None);
    };
    if receiver_fqn == "scoop.core.String" {
        return Ok(None);
    }

    let callee_fqn = format!("{receiver_fqn}.{method}");
    let sigs = collect_member_method_signatures_from_index(
        inputs.source,
        receiver_ty,
        &receiver_fqn,
        &receiver_args,
        &callee_fqn,
        lower,
        inputs.builtins,
    )?;
    let sigs = filter_operator_positioned_candidates(
        sigs,
        op_text,
        lower.fmt_type(receiver_ty),
        method,
        call_site_span,
    )?;
    let mut matched = Vec::new();
    for sig in sigs {
        if sig.params.len() != explicit_args.len() + 1
            || !sig.type_params.is_empty()
            || sig.eff_param.is_some()
        {
            continue;
        }

        let args_match = explicit_args
            .iter()
            .enumerate()
            .all(|(idx, (expr, found_ty))| {
                let expected_ty = sig.params[idx + 1];
                is_type_assignable(*found_ty, expected_ty, lower, inputs.builtins)
                    || literal_absorbs_to_expected(
                        expr,
                        expected_ty,
                        inputs.source,
                        lower,
                        inputs.builtins,
                    )
            });
        if args_match {
            matched.push(sig);
        }
    }

    let sig = match matched.len() {
        0 => return Ok(None),
        1 => matched.remove(0),
        _ => {
            let specificity = matched
                .iter()
                .map(|sig| {
                    let receiver_ty = sig.params.first().copied();
                    specificity_candidate_for_fun_sig(
                        fmt_overload_signature(
                            method,
                            receiver_ty,
                            sig.params.get(1..).unwrap_or_default(),
                            lower,
                        ),
                        format_candidate_location(lower, &sig.decl_file, sig.decl_span),
                        sig,
                        lower,
                        inputs.builtins,
                        call_site_span,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(chosen_idx) =
                pick_most_specific_overload(&specificity, lower, inputs.builtins)
            {
                matched.remove(chosen_idx)
            } else {
                let candidates =
                    format_ambiguous_specificity_candidates(&specificity, lower, inputs.builtins);
                return Err(ExprTypeError::AmbiguousOverload {
                    callee: callee_fqn,
                    candidates,
                    span: call_site_span.into(),
                });
            }
        }
    };

    check_unsafe_call_gate(&callee_fqn, &sig, call_site_span, lower)?;
    check_nogc_call_gate(&callee_fqn, &sig, call_site_span, lower)?;

    record_member_method_effects_as_performed(
        &receiver_fqn,
        &receiver_args,
        &sig,
        call_site_span,
        lower,
    )?;
    record_member_direct_call_binding(
        lower,
        call_site_span,
        &callee_fqn,
        &sig,
        receiver_ty,
        MemberDirectCallInstance {
            type_args: &[],
            eff_args: &[],
            param_tys: &sig.params,
            return_ty: sig.return_ty,
        },
    )?;

    Ok(Some(sig.return_ty))
}

pub(super) fn infer_unary_expr_type(
    inputs: ExprInferInputs<'_>,
    unary_expr: &ast::Expr,
    op: ast::UnaryOp,
    op_span: Span,
    operand: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    if op == ast::UnaryOp::Neg && matches!(operand.kind, ast::ExprKind::IntLit) {
        let operand_ty =
            int_literal_type_from_suffix(inputs.source, operand.span, lower, inputs.builtins);
        check_negated_int_literal_for_type(
            inputs.source,
            unary_expr.span,
            operand.span,
            operand_ty,
            lower,
            inputs.builtins,
        )?;
        lower.record_inferred_expr_ty(operand.span, operand_ty);
        record_scalar_operator_method_binding(
            inputs,
            unary_expr.span,
            operand_ty,
            "unaryMinus",
            unary_op_text(op),
            &[],
            lower,
        )?;
        return Ok(operand_ty);
    }

    let operand_ty = inputs.infer(lower, operand)?;

    match op {
        ast::UnaryOp::Not => {
            if operand_ty == inputs.builtins.bool_ {
                record_scalar_operator_method_binding(
                    inputs,
                    unary_expr.span,
                    operand_ty,
                    "not",
                    unary_op_text(op),
                    &[],
                    lower,
                )?;
                return Ok(inputs.builtins.bool_);
            }

            Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                op: unary_op_text(op).to_string(),
                expected: "Bool".to_string(),
                found: lower.fmt_type(operand_ty),
                span: op_span.into(),
            })
        }
        ast::UnaryOp::Neg => {
            if is_integer_type(operand_ty, lower, inputs.builtins)
                || is_float_type(operand_ty, lower, inputs.builtins)
            {
                record_scalar_operator_method_binding(
                    inputs,
                    unary_expr.span,
                    operand_ty,
                    "unaryMinus",
                    unary_op_text(op),
                    &[],
                    lower,
                )?;
                return Ok(operand_ty);
            }

            Err(ExprTypeError::UnaryOpOperandTypeMismatch {
                op: unary_op_text(op).to_string(),
                expected: "整数或 Float".to_string(),
                found: lower.fmt_type(operand_ty),
                span: op_span.into(),
            })
        }
        ast::UnaryOp::BitNot => {
            if is_integer_type(operand_ty, lower, inputs.builtins) {
                record_scalar_operator_method_binding(
                    inputs,
                    unary_expr.span,
                    operand_ty,
                    "inv",
                    unary_op_text(op),
                    &[],
                    lower,
                )?;
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
            let Some(sig) = collect_unique_zero_arg_operator_member_method_sig(
                inputs.source,
                NominalReceiverRef {
                    ty: operand_ty,
                    fqn: &receiver_fqn,
                    args: &receiver_args,
                },
                method,
                unary_op_text(op),
                op_span,
                lower,
                inputs.builtins,
            )?
            else {
                return Err(ExprTypeError::UnaryOperatorOverloadNotFound {
                    op: unary_op_text(op).to_string(),
                    receiver: lower.fmt_type(operand_ty),
                    method: method.to_string(),
                    span: op_span.into(),
                });
            };

            // operator method 调用：禁止 unsafe/nogc 门禁绕过，沿用普通调用的 gate。
            check_unsafe_call_gate(&callee_fqn, &sig, op_span, lower)?;
            check_nogc_call_gate(&callee_fqn, &sig, op_span, lower)?;

            // required effects：把被调用方法的 effect row 计入当前函数体的 performed effects。
            record_member_method_effects_as_performed(
                &receiver_fqn,
                &receiver_args,
                &sig,
                op_span,
                lower,
            )?;
            record_member_direct_call_binding(
                lower,
                unary_expr.span,
                &callee_fqn,
                &sig,
                operand_ty,
                MemberDirectCallInstance {
                    type_args: &[],
                    eff_args: &[],
                    param_tys: &sig.params,
                    return_ty: sig.return_ty,
                },
            )?;

            Ok(sig.return_ty)
        }
    }
}

fn int_literal_type_from_suffix(
    source: &SourceFile,
    span: Span,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> TypeId {
    match parse_int_literal_suffix(source.slice(span)) {
        IntLiteralSuffix::None => builtins.int,
        IntLiteralSuffix::UInt => builtins.uint,
        IntLiteralSuffix::Long => {
            lower.intern_type_kind(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: "scoop.core.Int64".to_string(),
                args: Vec::new(),
                eff: None,
            })))
        }
        IntLiteralSuffix::ULong => {
            lower.intern_type_kind(TypeKind::Value(ValueTypeKind::Nominal(NominalType {
                fqn: "scoop.core.UInt64".to_string(),
                args: Vec::new(),
                eff: None,
            })))
        }
    }
}

pub(super) fn infer_operator_overload_binary_expr_type(
    inputs: ExprInferInputs<'_>,
    binary_expr: &ast::Expr,
    lhs: &ast::Expr,
    op: ast::BinaryOp,
    op_span: Span,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = inputs.infer(lower, lhs)?;

    // T0123：String `+` 走内建字符串拼接规则，而不是用户态 operator overloading。
    //
    // 说明：这里补齐前端静态类型规则，避免在普通表达式里被误当成整数加法。
    if op == ast::BinaryOp::Add && lhs_ty == inputs.builtins.string {
        let rhs_ty = inputs.infer(lower, rhs)?;
        if rhs_ty == inputs.builtins.string {
            return Ok(inputs.builtins.string);
        }

        return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
            op: binary_op_text(op).to_string(),
            expected: "rhs 为 String".to_string(),
            lhs: lower.fmt_type(lhs_ty),
            rhs: lower.fmt_type(rhs_ty),
            span: op_span.into(),
        });
    }

    if lhs_ty == inputs.builtins.char_ {
        let rhs_ty = inputs.infer(lower, rhs)?;
        match op {
            ast::BinaryOp::Add if rhs_ty == inputs.builtins.int => {
                record_scalar_operator_method_binding(
                    inputs,
                    binary_expr.span,
                    lhs_ty,
                    "plus",
                    binary_op_text(op),
                    &[(rhs, rhs_ty)],
                    lower,
                )?;
                return Ok(inputs.builtins.char_);
            }
            ast::BinaryOp::Sub if rhs_ty == inputs.builtins.int => {
                record_scalar_operator_method_binding(
                    inputs,
                    binary_expr.span,
                    lhs_ty,
                    "minus",
                    binary_op_text(op),
                    &[(rhs, rhs_ty)],
                    lower,
                )?;
                return Ok(inputs.builtins.char_);
            }
            ast::BinaryOp::Sub if rhs_ty == inputs.builtins.char_ => {
                let Some(return_ty) = record_scalar_operator_method_binding(
                    inputs,
                    binary_expr.span,
                    lhs_ty,
                    "minus",
                    binary_op_text(op),
                    &[(rhs, rhs_ty)],
                    lower,
                )?
                else {
                    return Err(ExprTypeError::OperatorOverloadNotFound {
                        op: binary_op_text(op).to_string(),
                        receiver: lower.fmt_type(lhs_ty),
                        method: "minus".to_string(),
                        rhs: lower.fmt_type(rhs_ty),
                        span: op_span.into(),
                    });
                };
                return Ok(return_ty);
            }
            _ => {}
        }
    }

    if lhs_ty == inputs.builtins.bool_ {
        let rhs_ty = inputs.infer(lower, rhs)?;
        if matches!(
            op,
            ast::BinaryOp::BitAnd | ast::BinaryOp::BitOr | ast::BinaryOp::BitXor
        ) && rhs_ty == inputs.builtins.bool_
        {
            let method = scalar_operator_method_name(op).expect("bool bit op has method");
            record_scalar_operator_method_binding(
                inputs,
                binary_expr.span,
                lhs_ty,
                method,
                binary_op_text(op),
                &[(rhs, rhs_ty)],
                lower,
            )?;
            return Ok(inputs.builtins.bool_);
        }
    }

    // Kotlin-like：对整数保留内建规则（避免要求 sysroot 的 Int/Int8/... 必须定义 `plus/and/shl/...`）。
    if is_integer_type(lhs_ty, lower, inputs.builtins) {
        let rhs_ty = inputs.infer(lower, rhs)?;

        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => {
                // 注意：复用 `unify_integer_operands_for_same_type_rule` 允许整数字面量被上下文整数类型吸收。
                let Some(ty) = unify_integer_operands_for_same_type_rule(
                    lhs,
                    lhs_ty,
                    rhs,
                    rhs_ty,
                    inputs.source,
                    lower,
                    inputs.builtins,
                ) else {
                    return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
                        op: binary_op_text(op).to_string(),
                        expected: "相同的整数类型".to_string(),
                        lhs: lower.fmt_type(lhs_ty),
                        rhs: lower.fmt_type(rhs_ty),
                        span: op_span.into(),
                    });
                };

                let method = scalar_operator_method_name(op).expect("integer op has method");
                record_scalar_operator_method_binding(
                    inputs,
                    binary_expr.span,
                    ty,
                    method,
                    binary_op_text(op),
                    &[(rhs, rhs_ty)],
                    lower,
                )?;
                return Ok(ty);
            }
            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if rhs_ty == inputs.builtins.int {
                    let method = scalar_operator_method_name(op).expect("shift op has method");
                    record_scalar_operator_method_binding(
                        inputs,
                        binary_expr.span,
                        lhs_ty,
                        method,
                        binary_op_text(op),
                        &[(rhs, rhs_ty)],
                        lower,
                    )?;
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

    if is_float_type(lhs_ty, lower, inputs.builtins) {
        let rhs_ty = inputs.infer(lower, rhs)?;

        if matches!(
            op,
            ast::BinaryOp::Add
                | ast::BinaryOp::Sub
                | ast::BinaryOp::Mul
                | ast::BinaryOp::Div
                | ast::BinaryOp::Rem
        ) {
            let Some(ty) = unify_float_operands_for_same_type_rule(
                lhs,
                lhs_ty,
                rhs,
                rhs_ty,
                inputs.source,
                lower,
                inputs.builtins,
            ) else {
                return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
                    op: binary_op_text(op).to_string(),
                    expected: "相同的 Float 类型".to_string(),
                    lhs: lower.fmt_type(lhs_ty),
                    rhs: lower.fmt_type(rhs_ty),
                    span: op_span.into(),
                });
            };

            let method = scalar_operator_method_name(op).expect("float op has method");
            record_scalar_operator_method_binding(
                inputs,
                binary_expr.span,
                ty,
                method,
                binary_op_text(op),
                &[(rhs, rhs_ty)],
                lower,
            )?;
            return Ok(ty);
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
            let rhs_ty = inputs.infer(lower, rhs)?;
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
        let rhs_ty = inputs.infer(lower, rhs)?;
        return Err(ExprTypeError::BinaryOpOperandTypeMismatch {
            op: binary_op_text(op).to_string(),
            expected: builtin_binary_op_expected_text(op).to_string(),
            lhs: lower.fmt_type(lhs_ty),
            rhs: lower.fmt_type(rhs_ty),
            span: op_span.into(),
        });
    }

    let callee_fqn = format!("{receiver_fqn}.{method}");
    let mut sigs: Vec<(String, FunSigOwned)> =
        collect_member_method_signature_groups_from_receiver_ty(inputs, lhs_ty, method, lower)?
            .into_iter()
            .flat_map(|(fqn, sigs)| sigs.into_iter().map(move |sig| (fqn.clone(), sig)))
            .collect();
    let had_same_named_candidates = !sigs.is_empty();
    sigs.retain(|(_, sig)| sig.is_operator);
    if had_same_named_candidates && sigs.is_empty() {
        return Err(ExprTypeError::OperatorModifierRequired {
            op: binary_op_text(op).to_string(),
            receiver: lower.fmt_type(lhs_ty),
            method: method.to_string(),
            span: op_span.into(),
        });
    }

    if sigs.is_empty() {
        let rhs_ty = inputs.infer(lower, rhs)?;
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
        ast::ExprKind::Lambda(_) => inputs.builtins.any,
        _ => inputs.infer(lower, rhs)?,
    };
    let call_args: Vec<CallArgInfo<'_>> = vec![
        CallArgInfo {
            kind: CallArgKind::Positional,
            expr: lhs,
            ty: lhs_ty,
            is_spread: false,
            needs_expected_type: false,
        },
        CallArgInfo {
            kind: CallArgKind::Positional,
            expr: rhs,
            ty: rhs_ty_for_selection,
            is_spread: false,
            needs_expected_type: matches!(rhs.kind, ast::ExprKind::Lambda(_)),
        },
    ];

    struct MatchedOperatorOverload {
        fqn: String,
        sig: FunSigOwned,
        instantiated: InstantiatedFunSig,
        eff_arg: EffectRow,
    }

    let mut matched: Vec<MatchedOperatorOverload> = Vec::new();
    for (sig_fqn, sig) in sigs.iter() {
        let Some(mapping) = map_call_args_to_params_with_defaults(
            &call_args,
            &sig.param_names,
            &sig.param_has_defaults,
        ) else {
            continue;
        };

        let mut instantiated = match instantiate_fun_sig_for_call(
            sig_fqn,
            binary_expr.span,
            sig,
            mapping
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(param_idx, arg_idx)| {
                    let arg_idx = arg_idx?;
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
            inputs.builtins,
        ) {
            Ok(instantiated) => instantiated,
            Err(_) => continue,
        };

        if check_fun_where_constraints_after_instantiation(
            sig_fqn,
            binary_expr.span,
            sig,
            &instantiated.type_args,
            lower,
            inputs.builtins,
        )
        .is_err()
        {
            continue;
        }

        let eff_arg = default_eff_arg_for_fun_sig(sig);
        if sig.eff_param.is_some()
            && instantiate_eff_row_var_in_sig_types(
                sig,
                &mut instantiated,
                &eff_arg,
                lower,
                binary_expr.span,
            )
            .is_err()
        {
            continue;
        }

        let mut ok = true;
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                ok = false;
                break;
            };
            let expected_ty = instantiated.params[param_idx];
            let arg = &call_args[arg_idx];
            let found_ty = arg.ty;

            let arg_matches_expected =
                is_type_assignable(found_ty, expected_ty, lower, inputs.builtins)
                    || literal_absorbs_to_expected(
                        arg.expr,
                        expected_ty,
                        inputs.source,
                        lower,
                        inputs.builtins,
                    );
            if !arg_matches_expected {
                ok = false;
                break;
            }
        }

        if ok {
            matched.push(MatchedOperatorOverload {
                fqn: sig_fqn.to_string(),
                sig: sig.clone(),
                instantiated,
                eff_arg,
            });
        }
    }

    let selected = match matched.len() {
        0 => {
            let rhs_ty = inputs.infer(lower, rhs)?;
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
            let specificity = matched
                .iter()
                .map(|matched| {
                    let sig = &matched.sig;
                    let receiver_ty = sig.params.first().copied();
                    specificity_candidate_for_fun_sig(
                        fmt_overload_signature(
                            method,
                            receiver_ty,
                            sig.params.get(1..).unwrap_or_default(),
                            lower,
                        ),
                        format_candidate_location(lower, &sig.decl_file, sig.decl_span),
                        sig,
                        lower,
                        inputs.builtins,
                        binary_expr.span,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let chosen_idx = pick_most_specific_overload(&specificity, lower, inputs.builtins);
            if let Some(chosen_idx) = chosen_idx {
                matched.remove(chosen_idx)
            } else {
                let candidates =
                    format_ambiguous_specificity_candidates(&specificity, lower, inputs.builtins);
                return Err(ExprTypeError::AmbiguousOverload {
                    callee: callee_fqn,
                    candidates,
                    span: op_span.into(),
                });
            }
        }
    };
    let chosen_fqn = selected.fqn.as_str();
    let sig = &selected.sig;
    let instantiated = &selected.instantiated;

    // operator method 调用：禁止 unsafe/nogc 门禁绕过，沿用普通调用的 gate。
    check_unsafe_call_gate(chosen_fqn, sig, binary_expr.span, lower)?;
    check_nogc_call_gate(chosen_fqn, sig, binary_expr.span, lower)?;

    // rhs 最终类型检查：在期望类型语境下覆盖（lambda 下推推断等）。
    if let Some(expected_rhs_ty) = instantiated.params.get(1).copied() {
        let found_rhs_ty = inputs.infer_in_expected(
            lower,
            rhs,
            expected_rhs_ty,
            ExpectedTypeFrom::new(format!(
                "`{}` 的第 2 个形参 `{}`",
                chosen_fqn,
                sig.param_names
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "<arg>".to_string())
            )),
        )?;
        let rhs_matches_expected =
            is_type_assignable(found_rhs_ty, expected_rhs_ty, lower, inputs.builtins)
                || literal_absorbs_to_expected(
                    rhs,
                    expected_rhs_ty,
                    inputs.source,
                    lower,
                    inputs.builtins,
                );
        if !rhs_matches_expected {
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
            inputs.builtins,
        )?;
        check_nogc_boxing_gate(
            found_rhs_ty,
            expected_rhs_ty,
            rhs.span,
            lower,
            inputs.builtins,
        )?;
    }

    // required effects：把被调用方法的 effect row 计入当前函数体的 performed effects。
    let mut type_param_bindings =
        collect_nominal_type_param_bindings(&receiver_fqn, &receiver_args, lower);
    for p in sig.type_params.iter().copied() {
        type_param_bindings.push((type_param_name(p, lower), p));
    }
    let eff_bindings: Vec<(String, EffectRow)> = sig
        .eff_param
        .as_ref()
        .map(|p| vec![(p.name.clone(), selected.eff_arg.clone())])
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
        binary_expr.span,
    )?;
    for effect in call_effects.terms.iter().copied() {
        lower.record_performed_effect(effect, binary_expr.span);
    }
    let eff_args = sig
        .eff_param
        .as_ref()
        .map(|_| vec![selected.eff_arg.clone()])
        .unwrap_or_default();
    record_member_direct_call_binding(
        lower,
        binary_expr.span,
        chosen_fqn,
        sig,
        lhs_ty,
        MemberDirectCallInstance {
            type_args: &instantiated.type_args,
            eff_args: &eff_args,
            param_tys: &instantiated.params,
            return_ty: instantiated.return_ty,
        },
    )?;

    Ok(instantiated.return_ty)
}

pub(super) fn infer_builtin_scalar_binary_expr_type(
    inputs: ExprInferInputs<'_>,
    binary_expr: &ast::Expr,
    lhs: &ast::Expr,
    op: ast::BinaryOp,
    op_span: Span,
    rhs: &ast::Expr,
    lower: &mut TypeLowering<'_>,
) -> Result<TypeId, ExprTypeError> {
    let lhs_ty = inputs.infer(lower, lhs)?;
    let rhs_ty = inputs.infer(lower, rhs)?;

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
                unify_integer_operands_for_same_type_rule(
                    lhs,
                    lhs_ty,
                    rhs,
                    rhs_ty,
                    inputs.source,
                    lower,
                    inputs.builtins,
                )
            else {
                return Err(mismatch("相同的整数类型"));
            };
            Ok(ty)
        }
        // shifts: T << Int -> T
        ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
            if is_integer_type(lhs_ty, lower, inputs.builtins) && rhs_ty == inputs.builtins.int {
                return Ok(lhs_ty);
            }
            Err(mismatch("lhs 为整数且 rhs 为 Int"))
        }
        // comparisons: T < T -> Bool
        ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
            if unify_integer_operands_for_same_type_rule(
                lhs,
                lhs_ty,
                rhs,
                rhs_ty,
                inputs.source,
                lower,
                inputs.builtins,
            )
                .is_some()
            {
                return Ok(inputs.builtins.bool_);
            }
            if unify_float_operands_for_same_type_rule(
                lhs,
                lhs_ty,
                rhs,
                rhs_ty,
                inputs.source,
                lower,
                inputs.builtins,
            )
            .is_some()
            {
                return Ok(inputs.builtins.bool_);
            }
            if is_char_type(lhs_ty, inputs.builtins) && is_char_type(rhs_ty, inputs.builtins) {
                return Ok(inputs.builtins.bool_);
            }
            if infer_compare_to_overload_binary_expr_type(
                inputs,
                CompareToBinarySite {
                    binary_expr,
                    lhs,
                    rhs,
                    op,
                    lhs_ty,
                    rhs_ty,
                    op_span,
                },
                lower,
            )?
            .is_some()
            {
                return Ok(inputs.builtins.bool_);
            }
            Err(ExprTypeError::BinaryOpOperandTypeMismatch {
                op: binary_op_text(op).to_string(),
                expected: "相同的整数类型、Char 或相同的 Float 类型".to_string(),
                lhs: lower.fmt_type(lhs_ty),
                rhs: lower.fmt_type(rhs_ty),
                span: op_span.into(),
            })
        }
        // equality: (T == T) -> Bool; (Bool == Bool) -> Bool; (String == String) -> Bool; (Char == Char) -> Bool
        ast::BinaryOp::Eq | ast::BinaryOp::Ne => {
            if lhs_ty == inputs.builtins.bool_ && rhs_ty == inputs.builtins.bool_ {
                return Ok(inputs.builtins.bool_);
            }
            // T0107: String == String
            if lhs_ty == inputs.builtins.string && rhs_ty == inputs.builtins.string {
                return Ok(inputs.builtins.bool_);
            }
            if is_char_type(lhs_ty, inputs.builtins) && is_char_type(rhs_ty, inputs.builtins) {
                return Ok(inputs.builtins.bool_);
            }
            if unify_float_operands_for_same_type_rule(
                lhs,
                lhs_ty,
                rhs,
                rhs_ty,
                inputs.source,
                lower,
                inputs.builtins,
            )
            .is_some()
            {
                return Ok(inputs.builtins.bool_);
            }
            if unify_integer_operands_for_same_type_rule(
                lhs,
                lhs_ty,
                rhs,
                rhs_ty,
                inputs.source,
                lower,
                inputs.builtins,
            )
                .is_some()
            {
                return Ok(inputs.builtins.bool_);
            }
            Err(mismatch("相同的整数类型、相同的 Float 类型、Bool、Char 或 String"))
        }
        // boolean logic: Bool op Bool -> Bool
        ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
            if lhs_ty == inputs.builtins.bool_ && rhs_ty == inputs.builtins.bool_ {
                return Ok(inputs.builtins.bool_);
            }
            Err(mismatch("Bool"))
        }

        // elvis handled by caller
        ast::BinaryOp::Elvis => Err(ExprTypeError::UnsupportedExpr {
            kind: "elvis expression（internal）",
            span: op_span.into(),
        }),

        // range/progression（Appendix B.12）：核心整数类型映射到对应 Progression。
        ast::BinaryOp::RangeInclusive => {
            if let Some(integer_ty) = unify_integer_operands_for_same_type_rule(
                lhs,
                lhs_ty,
                rhs,
                rhs_ty,
                inputs.source,
                lower,
                inputs.builtins,
            ) && let Some(progression_ty) = progression_ty_for_integer_ty(integer_ty, lower)
            {
                return Ok(progression_ty);
            }
            Err(ExprTypeError::BinaryOpOperandTypeMismatch {
                op: binary_op_text(op).to_string(),
                expected: "Int、Long、UInt 或 ULong".to_string(),
                lhs: lower.fmt_type(lhs_ty),
                rhs: lower.fmt_type(rhs_ty),
                span: op_span.into(),
            })
        }
    }
}

struct CompareToBinarySite<'a> {
    binary_expr: &'a ast::Expr,
    lhs: &'a ast::Expr,
    rhs: &'a ast::Expr,
    op: ast::BinaryOp,
    lhs_ty: TypeId,
    rhs_ty: TypeId,
    op_span: Span,
}

fn infer_compare_to_overload_binary_expr_type(
    inputs: ExprInferInputs<'_>,
    site: CompareToBinarySite<'_>,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<()>, ExprTypeError> {
    let CompareToBinarySite {
        binary_expr,
        lhs,
        rhs,
        op,
        lhs_ty,
        rhs_ty,
        op_span,
    } = site;

    let Some((receiver_fqn, receiver_args)) = try_extract_nominal_fqn_and_args(lhs_ty, lower)
    else {
        return Ok(None);
    };
    if !matches!(
        lower.nominal_decl_kind(&receiver_fqn),
        Some(ast::TypeKind::Struct | ast::TypeKind::Class)
    ) {
        return Ok(None);
    }

    let method = "compareTo";
    let callee_fqn = format!("{receiver_fqn}.{method}");
    let mut sigs: Vec<(String, FunSigOwned)> =
        collect_member_method_signature_groups_from_receiver_ty(inputs, lhs_ty, method, lower)?
            .into_iter()
            .flat_map(|(fqn, sigs)| sigs.into_iter().map(move |sig| (fqn.clone(), sig)))
            .collect();
    let had_same_named_candidates = !sigs.is_empty();
    sigs.retain(|(_, sig)| sig.is_operator);
    if had_same_named_candidates && sigs.is_empty() {
        return Err(ExprTypeError::OperatorModifierRequired {
            op: binary_op_text(op).to_string(),
            receiver: lower.fmt_type(lhs_ty),
            method: method.to_string(),
            span: op_span.into(),
        });
    }
    if sigs.is_empty() {
        return Ok(None);
    }

    let rhs_ty_for_selection = match rhs.kind {
        ast::ExprKind::Lambda(_) => inputs.builtins.any,
        _ => rhs_ty,
    };
    let call_args = vec![
        CallArgInfo {
            kind: CallArgKind::Positional,
            expr: lhs,
            ty: lhs_ty,
            is_spread: false,
            needs_expected_type: false,
        },
        CallArgInfo {
            kind: CallArgKind::Positional,
            expr: rhs,
            ty: rhs_ty_for_selection,
            is_spread: false,
            needs_expected_type: matches!(rhs.kind, ast::ExprKind::Lambda(_)),
        },
    ];

    struct MatchedCompareToOverload {
        fqn: String,
        sig: FunSigOwned,
        instantiated: InstantiatedFunSig,
        eff_arg: EffectRow,
    }

    let mut matched: Vec<MatchedCompareToOverload> = Vec::new();
    for (sig_fqn, sig) in &sigs {
        if sig.params.len() != 2 {
            continue;
        }

        let Some(mapping) = map_call_args_to_params_with_defaults(
            &call_args,
            &sig.param_names,
            &sig.param_has_defaults,
        ) else {
            continue;
        };
        if mapping.iter().any(Option::is_none) {
            continue;
        }

        let mut instantiated = match instantiate_fun_sig_for_call(
            sig_fqn,
            binary_expr.span,
            sig,
            mapping
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(param_idx, arg_idx)| {
                    let arg_idx = arg_idx?;
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
            inputs.builtins,
        ) {
            Ok(instantiated) => instantiated,
            Err(_) => continue,
        };

        if check_fun_where_constraints_after_instantiation(
            sig_fqn,
            binary_expr.span,
            sig,
            &instantiated.type_args,
            lower,
            inputs.builtins,
        )
        .is_err()
        {
            continue;
        }

        let eff_arg = default_eff_arg_for_fun_sig(sig);
        if sig.eff_param.is_some()
            && instantiate_eff_row_var_in_sig_types(
                sig,
                &mut instantiated,
                &eff_arg,
                lower,
                binary_expr.span,
            )
            .is_err()
        {
            continue;
        }

        let mut ok = instantiated.return_ty == inputs.builtins.int;
        for (param_idx, arg_idx) in mapping.iter().copied().enumerate() {
            let Some(arg_idx) = arg_idx else {
                ok = false;
                break;
            };
            let expected_ty = instantiated.params[param_idx];
            let arg = &call_args[arg_idx];
            let found_ty = arg.ty;
            let arg_matches_expected =
                is_type_assignable(found_ty, expected_ty, lower, inputs.builtins)
                    || literal_absorbs_to_expected(
                        arg.expr,
                        expected_ty,
                        inputs.source,
                        lower,
                        inputs.builtins,
                    );
            if !arg_matches_expected {
                ok = false;
                break;
            }
        }

        if ok {
            matched.push(MatchedCompareToOverload {
                fqn: sig_fqn.to_string(),
                sig: sig.clone(),
                instantiated,
                eff_arg,
            });
        }
    }

    let selected = match matched.len() {
        0 => return Ok(None),
        1 => matched.remove(0),
        _ => {
            let specificity = matched
                .iter()
                .map(|matched| {
                    let sig = &matched.sig;
                    let receiver_ty = sig.params.first().copied();
                    specificity_candidate_for_fun_sig(
                        fmt_overload_signature(
                            method,
                            receiver_ty,
                            sig.params.get(1..).unwrap_or_default(),
                            lower,
                        ),
                        format_candidate_location(lower, &sig.decl_file, sig.decl_span),
                        sig,
                        lower,
                        inputs.builtins,
                        binary_expr.span,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let chosen_idx = pick_most_specific_overload(&specificity, lower, inputs.builtins);
            if let Some(chosen_idx) = chosen_idx {
                matched.remove(chosen_idx)
            } else {
                let candidates =
                    format_ambiguous_specificity_candidates(&specificity, lower, inputs.builtins);
                return Err(ExprTypeError::AmbiguousOverload {
                    callee: callee_fqn,
                    candidates,
                    span: op_span.into(),
                });
            }
        }
    };
    let chosen_fqn = selected.fqn.as_str();
    let sig = &selected.sig;
    let instantiated = &selected.instantiated;

    check_unsafe_call_gate(chosen_fqn, sig, binary_expr.span, lower)?;
    check_nogc_call_gate(chosen_fqn, sig, binary_expr.span, lower)?;

    let rhs_expected = instantiated.params.get(1).copied();
    if let Some(expected_rhs_ty) = rhs_expected {
        let found_rhs_ty = inputs.infer_in_expected(
            lower,
            rhs,
            expected_rhs_ty,
            ExpectedTypeFrom::new(format!(
                "`{}` 的第 2 个形参 `{}`",
                chosen_fqn,
                sig.param_names
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "<arg>".to_string())
            )),
        )?;
        let rhs_matches_expected =
            is_type_assignable(found_rhs_ty, expected_rhs_ty, lower, inputs.builtins)
                || literal_absorbs_to_expected(
                    rhs,
                    expected_rhs_ty,
                    inputs.source,
                    lower,
                    inputs.builtins,
                );
        if !rhs_matches_expected {
            return Ok(None);
        }

        check_fn_value_to_any_erasure_gate(
            found_rhs_ty,
            expected_rhs_ty,
            rhs.span,
            lower,
            inputs.builtins,
        )?;
        check_nogc_boxing_gate(
            found_rhs_ty,
            expected_rhs_ty,
            rhs.span,
            lower,
            inputs.builtins,
        )?;
    }

    let mut type_param_bindings =
        collect_nominal_type_param_bindings(&receiver_fqn, &receiver_args, lower);
    for p in sig.type_params.iter().copied() {
        type_param_bindings.push((type_param_name(p, lower), p));
    }
    let eff_bindings: Vec<(String, EffectRow)> = sig
        .eff_param
        .as_ref()
        .map(|p| vec![(p.name.clone(), selected.eff_arg.clone())])
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
        binary_expr.span,
    )?;
    for effect in call_effects.terms.iter().copied() {
        lower.record_performed_effect(effect, binary_expr.span);
    }

    let eff_args = sig
        .eff_param
        .as_ref()
        .map(|_| vec![selected.eff_arg.clone()])
        .unwrap_or_default();
    record_member_direct_call_binding(
        lower,
        binary_expr.span,
        chosen_fqn,
        sig,
        lhs_ty,
        MemberDirectCallInstance {
            type_args: &instantiated.type_args,
            eff_args: &eff_args,
            param_tys: &instantiated.params,
            return_ty: instantiated.return_ty,
        },
    )?;

    Ok(Some(()))
}
