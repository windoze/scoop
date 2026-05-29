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
use super::{
    EffParamSig, ExprInferInputs, ExprTypeError, FUNPTR_FQN, FunSigOwned, FunWhereConstraintInfo,
    PTR_FQN,
};

use super::super::assignable::is_type_assignable;
use super::super::eff_row_subst::{
    EffRowVarSubstPlan, apply_eff_row_var_subst_plan, build_eff_row_var_subst_plan,
};
use super::super::lower::{LoweredGenericBound, TypeLowering};
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

#[derive(Debug, Clone)]
pub(super) enum EffectiveSpecificityType {
    Single(TypeId),
    Intersection(Vec<TypeId>),
}

#[derive(Debug, Clone)]
pub(super) struct SpecificityParam {
    pub(super) ty: EffectiveSpecificityType,
    pub(super) source: String,
}

#[derive(Debug, Clone)]
pub(super) struct SpecificityCandidate {
    pub(super) signature: String,
    pub(super) location: String,
    pub(super) params: Vec<SpecificityParam>,
}

pub(super) fn specificity_candidate_for_fun_sig(
    signature: String,
    location: String,
    sig: &FunSigOwned,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    use_span: Span,
) -> Result<SpecificityCandidate, ExprTypeError> {
    specificity_candidate_for_declared_params(
        signature,
        location,
        &sig.params,
        &sig.type_params,
        &sig.where_constraints,
        &sig.decl_file,
        lower,
        builtins,
        use_span,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn specificity_candidate_for_declared_params(
    signature: String,
    location: String,
    declared_params: &[TypeId],
    type_params: &[TypeId],
    where_constraints: &[FunWhereConstraintInfo],
    decl_file: &Path,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    use_span: Span,
) -> Result<SpecificityCandidate, ExprTypeError> {
    let mut params = Vec::with_capacity(declared_params.len());
    for param in declared_params.iter().copied() {
        params.push(effective_param_from_declared_type(
            param,
            type_params,
            where_constraints,
            decl_file,
            lower,
            builtins,
            use_span,
            &mut HashSet::new(),
        )?);
    }
    Ok(SpecificityCandidate {
        signature,
        location,
        params,
    })
}

#[allow(clippy::too_many_arguments)]
fn effective_param_from_declared_type(
    ty: TypeId,
    type_params: &[TypeId],
    where_constraints: &[FunWhereConstraintInfo],
    decl_file: &Path,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    use_span: Span,
    visiting: &mut HashSet<TypeId>,
) -> Result<SpecificityParam, ExprTypeError> {
    if matches!(lower.type_kind(ty), TypeKind::Param(_)) && type_params.contains(&ty) {
        return type_param_effective_bound(
            ty,
            type_params,
            where_constraints,
            decl_file,
            lower,
            builtins,
            use_span,
            visiting,
        );
    }

    let mut alternatives = vec![ty];
    let mut substitutions = Vec::new();
    for type_param in type_params.iter().copied() {
        let bound = type_param_effective_bound(
            type_param,
            type_params,
            where_constraints,
            decl_file,
            lower,
            builtins,
            use_span,
            visiting,
        )?;
        let replacements = match &bound.ty {
            EffectiveSpecificityType::Single(ty) => vec![*ty],
            EffectiveSpecificityType::Intersection(items) => items.clone(),
        };
        let mut next_alternatives = Vec::with_capacity(alternatives.len() * replacements.len());
        let mut changed = false;
        for cur in alternatives.iter().copied() {
            for replacement in replacements.iter().copied() {
                let next = generic::substitute_single_type_param(
                    cur,
                    type_param,
                    replacement,
                    lower,
                    use_span,
                )?;
                if next != cur {
                    changed = true;
                }
                next_alternatives.push(next);
            }
        }
        if changed {
            substitutions.push(format!(
                "{} -> {} ({})",
                fmt_type_param_name(type_param, lower),
                fmt_effective_specificity_type(&bound.ty, lower),
                bound.source
            ));
            next_alternatives.sort();
            next_alternatives.dedup();
            alternatives = next_alternatives;
        }
    }

    let source = if substitutions.is_empty() {
        "declared concrete type".to_string()
    } else {
        format!("declared composite type with {}", substitutions.join(", "))
    };
    alternatives.sort();
    alternatives.dedup();
    let ty = if alternatives.len() == 1 {
        EffectiveSpecificityType::Single(alternatives[0])
    } else {
        EffectiveSpecificityType::Intersection(alternatives)
    };
    Ok(SpecificityParam { ty, source })
}

#[allow(clippy::too_many_arguments)]
fn type_param_effective_bound(
    type_param: TypeId,
    type_params: &[TypeId],
    where_constraints: &[FunWhereConstraintInfo],
    decl_file: &Path,
    lower: &mut TypeLowering<'_>,
    builtins: BuiltinTypes,
    use_span: Span,
    visiting: &mut HashSet<TypeId>,
) -> Result<SpecificityParam, ExprTypeError> {
    let name = fmt_type_param_name(type_param, lower);
    let Some(param_index) = type_params.iter().position(|p| *p == type_param) else {
        return Ok(SpecificityParam {
            ty: EffectiveSpecificityType::Single(type_param),
            source: format!("outer type parameter `{name}`"),
        });
    };

    if !visiting.insert(type_param) {
        return Ok(SpecificityParam {
            ty: EffectiveSpecificityType::Single(builtins.any),
            source: format!("cyclic bound on `{name}`; default Any"),
        });
    }

    let bindings = type_param_bindings_for_specificity(type_params, lower);
    let mut type_bounds = Vec::new();
    let mut kind_bounds = Vec::new();
    for constraint in where_constraints
        .iter()
        .filter(|constraint| constraint.param_index == param_index)
    {
        let lowered = lower.lower_generic_bound_in_decl_file_with_bindings(
            decl_file,
            bindings.iter().cloned(),
            &constraint.bound,
        )?;
        match lowered {
            LoweredGenericBound::Type(bound_ty) => {
                let bound = effective_param_from_declared_type(
                    bound_ty,
                    type_params,
                    where_constraints,
                    decl_file,
                    lower,
                    builtins,
                    use_span,
                    visiting,
                )?;
                match bound.ty {
                    EffectiveSpecificityType::Single(ty) => type_bounds.push(ty),
                    EffectiveSpecificityType::Intersection(items) => type_bounds.extend(items),
                }
            }
            LoweredGenericBound::Ref => kind_bounds.push("ref"),
            LoweredGenericBound::Value => kind_bounds.push("value"),
        }
    }

    visiting.remove(&type_param);

    type_bounds.sort();
    type_bounds.dedup();
    if type_bounds.is_empty() {
        let source = if kind_bounds.is_empty() {
            format!("`{name}` has no declared type bound; default Any")
        } else {
            format!(
                "`{name}` has only {} kind bound(s); specificity uses Any",
                kind_bounds.join(" + ")
            )
        };
        return Ok(SpecificityParam {
            ty: EffectiveSpecificityType::Single(builtins.any),
            source,
        });
    }

    if type_bounds.len() == 1 {
        return Ok(SpecificityParam {
            ty: EffectiveSpecificityType::Single(type_bounds[0]),
            source: format!("from `{name}` declared bound"),
        });
    }

    Ok(SpecificityParam {
        ty: EffectiveSpecificityType::Intersection(type_bounds),
        source: format!("from `{name}` declared bounds intersection"),
    })
}

fn type_param_bindings_for_specificity(
    type_params: &[TypeId],
    lower: &TypeLowering<'_>,
) -> Vec<(String, TypeId)> {
    type_params
        .iter()
        .copied()
        .filter_map(|ty| match lower.type_kind(ty) {
            TypeKind::Param(param) => Some((param.name, ty)),
            _ => None,
        })
        .collect()
}

fn fmt_type_param_name(ty: TypeId, lower: &TypeLowering<'_>) -> String {
    match lower.type_kind(ty) {
        TypeKind::Param(param) => param.name,
        _ => lower.fmt_type(ty),
    }
}

pub(super) fn pick_most_specific_overload(
    candidates: &[SpecificityCandidate],
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<usize> {
    let maximal = maximal_specificity_candidates(candidates, lower, builtins);
    if maximal.len() == 1 {
        maximal.first().copied()
    } else {
        None
    }
}

fn maximal_specificity_candidates(
    candidates: &[SpecificityCandidate],
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Vec<usize> {
    let mut maximal = Vec::new();
    'candidate: for idx in 0..candidates.len() {
        for other_idx in 0..candidates.len() {
            if idx == other_idx {
                continue;
            }
            if is_strictly_more_specific_candidate(
                &candidates[other_idx],
                &candidates[idx],
                lower,
                builtins,
            ) {
                continue 'candidate;
            }
        }
        maximal.push(idx);
    }
    maximal
}

fn is_strictly_more_specific_candidate(
    lhs: &SpecificityCandidate,
    rhs: &SpecificityCandidate,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    if lhs.params.len() != rhs.params.len() {
        return false;
    }

    let mut strict = false;
    for (lhs_param, rhs_param) in lhs.params.iter().zip(&rhs.params) {
        if !effective_specificity_subtype(&lhs_param.ty, &rhs_param.ty, lower, builtins) {
            return false;
        }
        if !effective_specificity_subtype(&rhs_param.ty, &lhs_param.ty, lower, builtins) {
            strict = true;
        }
    }
    strict
}

fn effective_specificity_subtype(
    lhs: &EffectiveSpecificityType,
    rhs: &EffectiveSpecificityType,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> bool {
    match (lhs, rhs) {
        (EffectiveSpecificityType::Single(lhs), EffectiveSpecificityType::Single(rhs)) => {
            is_type_assignable(*lhs, *rhs, lower, builtins)
        }
        (EffectiveSpecificityType::Intersection(lhs), EffectiveSpecificityType::Single(rhs)) => lhs
            .iter()
            .copied()
            .any(|lhs| is_type_assignable(lhs, *rhs, lower, builtins)),
        (EffectiveSpecificityType::Single(lhs), EffectiveSpecificityType::Intersection(rhs)) => rhs
            .iter()
            .copied()
            .all(|rhs| is_type_assignable(*lhs, rhs, lower, builtins)),
        (
            EffectiveSpecificityType::Intersection(lhs),
            EffectiveSpecificityType::Intersection(rhs),
        ) => rhs.iter().copied().all(|rhs| {
            lhs.iter()
                .copied()
                .any(|lhs| is_type_assignable(lhs, rhs, lower, builtins))
        }),
    }
}

pub(super) fn format_ambiguous_specificity_candidates(
    candidates: &[SpecificityCandidate],
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> String {
    let summary = join_overload_signatures(
        candidates
            .iter()
            .map(|candidate| candidate.signature.clone())
            .collect(),
    );
    let mut details: Vec<String> = candidates
        .iter()
        .map(|candidate| format_specificity_candidate(candidate, lower))
        .collect();
    details.sort();
    details.dedup();

    let maximal = maximal_specificity_candidates(candidates, lower, builtins);
    let reason_indices = if maximal.len() >= 2 {
        maximal
    } else {
        (0..candidates.len()).collect()
    };
    let mut reasons = Vec::new();
    for (pos, lhs_idx) in reason_indices.iter().copied().enumerate() {
        for rhs_idx in reason_indices.iter().copied().skip(pos + 1) {
            if is_strictly_more_specific_candidate(
                &candidates[lhs_idx],
                &candidates[rhs_idx],
                lower,
                builtins,
            ) || is_strictly_more_specific_candidate(
                &candidates[rhs_idx],
                &candidates[lhs_idx],
                lower,
                builtins,
            ) {
                continue;
            }
            reasons.push(format_specificity_pair_reason(
                &candidates[lhs_idx],
                &candidates[rhs_idx],
                lower,
                builtins,
            ));
        }
    }
    if reasons.is_empty() {
        reasons.push("no unique most-specific candidate".to_string());
    }
    reasons.sort();
    reasons.dedup();

    format!(
        "{summary}; applicable candidates: {}; reason: {}",
        details.join("; "),
        reasons.join("; ")
    )
}

fn format_specificity_candidate(
    candidate: &SpecificityCandidate,
    lower: &TypeLowering<'_>,
) -> String {
    let params = candidate
        .params
        .iter()
        .enumerate()
        .map(|(idx, param)| {
            format!(
                "pos {idx}: {} ({})",
                fmt_effective_specificity_type(&param.ty, lower),
                param.source
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{} @ {} [effective: {params}]",
        candidate.signature, candidate.location
    )
}

fn format_specificity_pair_reason(
    lhs: &SpecificityCandidate,
    rhs: &SpecificityCandidate,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> String {
    if lhs.params.len() != rhs.params.len() {
        return format!(
            "{} vs {}: parameter counts differ after applicability",
            lhs.signature, rhs.signature
        );
    }

    let mut positions = Vec::new();
    let mut lhs_strict = false;
    let mut rhs_strict = false;
    let mut any_incomparable = false;
    for (idx, (lhs_param, rhs_param)) in lhs.params.iter().zip(&rhs.params).enumerate() {
        let lhs_sub_rhs =
            effective_specificity_subtype(&lhs_param.ty, &rhs_param.ty, lower, builtins);
        let rhs_sub_lhs =
            effective_specificity_subtype(&rhs_param.ty, &lhs_param.ty, lower, builtins);
        let lhs_ty = fmt_effective_specificity_type(&lhs_param.ty, lower);
        let rhs_ty = fmt_effective_specificity_type(&rhs_param.ty, lower);
        match (lhs_sub_rhs, rhs_sub_lhs) {
            (true, true) => {
                positions.push(format!("position {idx}: equal effective types {lhs_ty}"))
            }
            (true, false) => {
                lhs_strict = true;
                positions.push(format!(
                    "position {idx}: {lhs_ty} ({}) <: {rhs_ty} ({})",
                    lhs_param.source, rhs_param.source
                ));
            }
            (false, true) => {
                rhs_strict = true;
                positions.push(format!(
                    "position {idx}: {rhs_ty} ({}) <: {lhs_ty} ({})",
                    rhs_param.source, lhs_param.source
                ));
            }
            (false, false) => {
                any_incomparable = true;
                positions.push(format!(
                    "position {idx}: {lhs_ty} ({}) and {rhs_ty} ({}) are incomparable",
                    lhs_param.source, rhs_param.source
                ));
            }
        }
    }

    let conclusion = match (lhs_strict, rhs_strict) {
        (true, true) => "cross-incomparable; no candidate is strictly more specific",
        (false, false) if any_incomparable => {
            "effective parameter types are incomparable; no strict winner"
        }
        (false, false) => "effective parameter types are equivalent; no strict winner",
        _ => "incomparable positions prevent a unique winner",
    };
    format!(
        "{} vs {}: {}; {conclusion}",
        lhs.signature,
        rhs.signature,
        positions.join(", ")
    )
}

fn fmt_effective_specificity_type(
    ty: &EffectiveSpecificityType,
    lower: &TypeLowering<'_>,
) -> String {
    match ty {
        EffectiveSpecificityType::Single(ty) => lower.fmt_type(*ty),
        EffectiveSpecificityType::Intersection(items) => items
            .iter()
            .copied()
            .map(|ty| lower.fmt_type(ty))
            .collect::<Vec<_>>()
            .join(" & "),
    }
}

pub(super) fn format_candidate_location(
    lower: &TypeLowering<'_>,
    decl_file: &Path,
    decl_span: Span,
) -> String {
    let Some(source) = lower.env().source(decl_file) else {
        return format!("{}:<unknown>:<unknown>", decl_file.display());
    };
    let Ok((line, col)) = source.offset_to_line_col(decl_span.start) else {
        return format!("{}:<unknown>:<unknown>", decl_file.display());
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
        return describe_call_arg_mapping_rejection(
            call_args,
            param_names,
            param_has_defaults,
            param_is_vararg,
        );
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
            return format!(
                "generic type arguments or where constraints could not be inferred/satisfied for argument {} bound to parameter `{}`",
                arg_idx + 1,
                param_names
                    .get(param_idx)
                    .map(String::as_str)
                    .unwrap_or("<unknown>")
            );
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

    "all arity and argument type checks passed, but generic, effect, or where constraints rejected this candidate".to_string()
}

fn describe_call_arg_mapping_rejection(
    call_args: &[CallArgInfo<'_>],
    param_names: &[String],
    param_has_defaults: &[bool],
    param_is_vararg: &[bool],
) -> String {
    if param_names.len() != param_has_defaults.len() || param_names.len() != param_is_vararg.len() {
        return "candidate signature is malformed".to_string();
    }

    let vararg_idx = args::vararg_param_index(param_is_vararg);
    if vararg_idx.is_none() && param_is_vararg.iter().any(|is_vararg| *is_vararg) {
        return "candidate has more than one vararg parameter".to_string();
    }
    if let Some(idx) = vararg_idx
        && idx + 1 != param_names.len()
    {
        return format!(
            "vararg parameter `{}` must be the final parameter",
            param_names
                .get(idx)
                .map(String::as_str)
                .unwrap_or("<unknown>")
        );
    }

    let mut seen_named = false;
    let mut positional_count = 0usize;
    for (arg_idx, arg) in call_args.iter().enumerate() {
        match &arg.kind {
            CallArgKind::Positional => {
                if seen_named {
                    return format!(
                        "argument {} is positional after a named argument",
                        arg_idx + 1
                    );
                }
                positional_count += 1;
            }
            CallArgKind::Named { .. } => seen_named = true,
        }
    }

    let mut mapping: Vec<Option<usize>> = vec![None; param_names.len()];
    if vararg_idx.is_none() && positional_count > param_names.len() {
        return format!(
            "arity mismatch: expected at most {} argument(s), found {}",
            param_names.len(),
            call_args.len()
        );
    }
    for (arg_idx, slot) in mapping.iter_mut().enumerate().take(positional_count) {
        if let Some(v_idx) = vararg_idx
            && arg_idx >= v_idx
        {
            continue;
        }
        *slot = Some(arg_idx);
    }

    for (arg_idx, arg) in call_args.iter().enumerate().skip(positional_count) {
        let CallArgKind::Named { name, .. } = &arg.kind else {
            continue;
        };
        let Some(slot_idx) = param_names.iter().position(|param| param == name) else {
            return format!("unknown named argument `{name}` for this candidate");
        };
        if Some(slot_idx) == vararg_idx {
            continue;
        }
        if mapping.get(slot_idx).and_then(|slot| *slot).is_some() {
            return format!("argument `{name}` assigns parameter `{name}` more than once");
        }
        mapping[slot_idx] = Some(arg_idx);
    }

    let missing = mapping
        .iter()
        .enumerate()
        .filter_map(|(idx, arg_idx)| {
            if Some(idx) == vararg_idx || arg_idx.is_some() || param_has_defaults[idx] {
                None
            } else {
                Some(format!("`{}`", param_names[idx]))
            }
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return format!("missing required parameter(s): {}", missing.join(", "));
    }

    if let Some(v_idx) = vararg_idx {
        return format!(
            "arguments could not be mapped to vararg parameter `{}` with defaults/trailing lambdas",
            param_names
                .get(v_idx)
                .map(String::as_str)
                .unwrap_or("<unknown>")
        );
    }

    "argument names and default parameters do not map to this candidate".to_string()
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
