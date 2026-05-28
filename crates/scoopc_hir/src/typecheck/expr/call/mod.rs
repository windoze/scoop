use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::ast;
use crate::cone::ConeId;
use crate::resolve::{ConstructorOverload, FunOverload, Visibility};
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
    expr_kind_name, fmt_overload_signature, join_overload_signatures, short_name_from_fqn,
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

#[derive(Debug, Clone)]
pub(super) struct OverloadRejection {
    pub(super) signature: String,
    pub(super) location: String,
    pub(super) reason: String,
}

pub(super) fn format_candidate_location(
    lower: &TypeLowering<'_>,
    decl_file: &Path,
    decl_span: Span,
) -> String {
    let Some(source) = lower.env().source(decl_file) else {
        return decl_file.display().to_string();
    };
    let Ok((line, col)) = source.offset_to_line_col(decl_span.start) else {
        return decl_file.display().to_string();
    };
    format!("{}:{line}:{col}", decl_file.display())
}

pub(super) fn join_overload_rejections(rejections: Vec<OverloadRejection>) -> String {
    let mut parts = rejections
        .into_iter()
        .map(|r| format!("{} @ {} - {}", r.signature, r.location, r.reason))
        .collect::<Vec<_>>();
    parts.sort();
    parts.dedup();
    parts.join("; ")
}

pub(super) struct BasicApplicabilityRejection<'a, 'expr, 'tcx> {
    pub(super) call_args: &'a [CallArgInfo<'expr>],
    pub(super) param_names: &'a [String],
    pub(super) param_has_defaults: &'a [bool],
    pub(super) param_is_vararg: &'a [bool],
    pub(super) param_tys: &'a [TypeId],
    pub(super) source: &'a SourceFile,
    pub(super) lower: &'a TypeLowering<'tcx>,
    pub(super) builtins: BuiltinTypes,
}

pub(super) fn describe_basic_applicability_rejection(
    request: BasicApplicabilityRejection<'_, '_, '_>,
) -> String {
    let BasicApplicabilityRejection {
        call_args,
        param_names,
        param_has_defaults,
        param_is_vararg,
        param_tys,
        source,
        lower,
        builtins,
    } = request;
    let Some(mapping) = args::map_call_args_to_params_with_defaults_and_varargs(
        call_args,
        param_names,
        param_has_defaults,
        param_is_vararg,
    ) else {
        let required = args::required_param_count(param_has_defaults, param_is_vararg)
            .unwrap_or_else(|| param_has_defaults.iter().filter(|d| !**d).count());
        let vararg = args::vararg_param_index(param_is_vararg).is_some();
        if call_args.len() < required {
            return format!(
                "arity mismatch: expected at least {required} argument(s), found {}",
                call_args.len()
            );
        }
        if !vararg && call_args.len() > param_names.len() {
            return format!(
                "arity mismatch: expected at most {} argument(s), found {}",
                param_names.len(),
                call_args.len()
            );
        }
        return "argument names, defaults, or vararg mapping do not match".to_string();
    };

    for (param_idx, binding) in mapping.iter().enumerate() {
        if let args::ParamArgBinding::Single(arg_idx) = binding
            && call_args.get(*arg_idx).is_some_and(|a| a.is_spread)
            && !param_is_vararg.get(param_idx).copied().unwrap_or(false)
        {
            return format!(
                "argument {} uses spread but parameter `{}` is not vararg",
                arg_idx + 1,
                param_names
                    .get(param_idx)
                    .map(String::as_str)
                    .unwrap_or("<unknown>")
            );
        }
    }

    for (param_idx, arg_idx) in args::expand_param_arg_pairs(&mapping) {
        let Some(expected_ty) = param_tys.get(param_idx).copied() else {
            return "candidate signature is malformed".to_string();
        };
        let Some(arg) = call_args.get(arg_idx) else {
            return "argument mapping points outside the call".to_string();
        };
        if matches!(lower.type_kind(expected_ty), TypeKind::Param(_)) {
            return "generic type arguments or where constraints could not be inferred/satisfied"
                .to_string();
        }

        if arg.is_spread {
            let Some(elem_tys) = args::spread_operand_element_types(arg.ty, lower) else {
                return format!(
                    "argument {} uses spread but its type {} is not Array or tuple",
                    arg_idx + 1,
                    lower.fmt_type(arg.ty)
                );
            };
            for elem_ty in elem_tys {
                if is_type_assignable(elem_ty, expected_ty, lower, builtins) {
                    continue;
                }
                return format!(
                    "spread element type {} is not a subtype of parameter `{}` type {}",
                    lower.fmt_type(elem_ty),
                    param_names
                        .get(param_idx)
                        .map(String::as_str)
                        .unwrap_or("<unknown>"),
                    lower.fmt_type(expected_ty)
                );
            }
            continue;
        }

        if is_type_assignable(arg.ty, expected_ty, lower, builtins)
            || literal_absorbs_to_expected(arg.expr, expected_ty, source, lower, builtins)
        {
            continue;
        }
        return format!(
            "argument {} type {} is not a subtype of parameter `{}` type {}",
            arg_idx + 1,
            lower.fmt_type(arg.ty),
            param_names
                .get(param_idx)
                .map(String::as_str)
                .unwrap_or("<unknown>"),
            lower.fmt_type(expected_ty)
        );
    }

    "generic, effect, or where constraints rejected this candidate".to_string()
}

mod args;
mod ctor;
mod dispatch;
mod effect_op;
mod enum_variant;
mod gates;
mod generic;
mod member_call;
mod value_call;

// Re-export each submodule's API up to `expr::call::*` so the rest of
// `expr` keeps its `super::call::Foo` access pattern. Sibling submodules
// also pick these items up via `use super::*;`. Some globs only re-export
// `pub(super)` helpers used by sibling submodules (visible to call only)
// and contribute nothing at expr level — `#[allow(unused_imports)]`
// silences the corresponding warning.
#[allow(unused_imports)]
pub(super) use {
    args::*, ctor::*, dispatch::*, effect_op::*, enum_variant::*, gates::*, generic::*,
    member_call::*, value_call::*,
};

// `lower_type_ref_with_enum_subst` is consumed by sibling typecheck modules
// (`when_pat`, `when_exhaustiveness`) so it stays visible at the wider
// `crate::typecheck` scope.
pub(in crate::typecheck) use enum_variant::lower_type_ref_with_enum_subst;
