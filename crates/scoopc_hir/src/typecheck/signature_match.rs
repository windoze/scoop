use crate::ast;
use crate::resolve::FunOverload;
use crate::source::SourceFile;
use crate::ty::{EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, ValueTypeKind};

use super::TypeEnv;
use super::lower::{TypeLowerError, TypeLowering};

#[derive(Clone, Copy)]
pub(super) struct OwnerInstantiation<'a> {
    pub fqn: &'a str,
    pub nominal: &'a NominalType,
}

#[derive(Debug)]
struct SignatureParts {
    receiver: Option<TypeId>,
    params: Vec<TypeId>,
}

pub(super) fn fun_decl_matches_overload_signature(
    fallback_source: &SourceFile,
    fun: &ast::FunDecl,
    target: &FunOverload,
    target_owner: OwnerInstantiation<'_>,
    lower: &mut TypeLowering<'_>,
    env: &TypeEnv,
) -> Result<bool, TypeLowerError> {
    if fun.receiver.is_some() != target.sig.receiver.is_some()
        || fun.params.len() != target.sig.params.len()
        || fun.type_params.len() != target.sig.type_params.len()
    {
        return Ok(false);
    }

    let Some(left) = lower_fun_decl_signature_parts(fallback_source, fun, lower)? else {
        return Ok(false);
    };
    let Some(right) =
        lower_fun_overload_signature_parts(fallback_source, target, target_owner, lower, env)?
    else {
        return Ok(false);
    };

    Ok(signature_parts_equal(&left, &right))
}

pub(super) fn fun_overloads_have_same_signature(
    fallback_source: &SourceFile,
    left: &FunOverload,
    left_owner: OwnerInstantiation<'_>,
    right: &FunOverload,
    right_owner: OwnerInstantiation<'_>,
    lower: &mut TypeLowering<'_>,
    env: &TypeEnv,
) -> Result<bool, TypeLowerError> {
    if left.sig.receiver.is_some() != right.sig.receiver.is_some()
        || left.sig.params.len() != right.sig.params.len()
        || left.sig.type_params.len() != right.sig.type_params.len()
    {
        return Ok(false);
    }

    let Some(left) =
        lower_fun_overload_signature_parts(fallback_source, left, left_owner, lower, env)?
    else {
        return Ok(false);
    };
    let Some(right) =
        lower_fun_overload_signature_parts(fallback_source, right, right_owner, lower, env)?
    else {
        return Ok(false);
    };

    Ok(signature_parts_equal(&left, &right))
}

pub(super) fn nominal_from_type_id(ty: TypeId, lower: &TypeLowering<'_>) -> Option<NominalType> {
    match lower.type_kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal)) => Some(nominal),
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal),
        _ => None,
    }
}

fn signature_parts_equal(left: &SignatureParts, right: &SignatureParts) -> bool {
    left.receiver == right.receiver && left.params == right.params
}

fn lower_fun_decl_signature_parts(
    source: &SourceFile,
    fun: &ast::FunDecl,
    lower: &mut TypeLowering<'_>,
) -> Result<Option<SignatureParts>, TypeLowerError> {
    lower.push_type_params(&fun.type_params);
    let fun_eff_binding = if let Some(eff_param) = &fun.eff_param {
        let name = source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr(Some(expr))?,
            None => EffectRow::pure(),
        };
        lower.push_effect_row_param_binding(name, default);
        true
    } else {
        false
    };

    let result = (|| {
        let receiver = match &fun.receiver {
            Some(receiver) => Some(lower.lower_type_ref(receiver)?),
            None => None,
        };
        let mut params = Vec::with_capacity(fun.params.len());
        for param in &fun.params {
            let Some(ty) = &param.ty else {
                return Ok(None);
            };
            params.push(lower.lower_type_ref(ty)?);
        }
        Ok(Some(SignatureParts { receiver, params }))
    })();

    if fun_eff_binding {
        lower.pop_effect_row_param_binding();
    }
    lower.pop_type_params(&fun.type_params);

    result
}

fn lower_fun_overload_signature_parts(
    fallback_source: &SourceFile,
    overload: &FunOverload,
    owner: OwnerInstantiation<'_>,
    lower: &mut TypeLowering<'_>,
    env: &TypeEnv,
) -> Result<Option<SignatureParts>, TypeLowerError> {
    let mut type_bindings = owner_type_bindings(env, owner);
    for type_param in &overload.sig.type_params {
        let id = lower.intern_decl_type_param(
            type_param.name.clone(),
            overload.symbol.decl_file.clone(),
            type_param.name_span,
        );
        type_bindings.push((type_param.name.clone(), id));
    }

    let mut eff_bindings = owner_eff_bindings(env, owner);
    if let Some(eff_param) = &overload.sig.eff_param {
        let decl_source = env
            .source(&overload.symbol.decl_file)
            .unwrap_or(fallback_source);
        let name = decl_source.slice(eff_param.name.span).to_string();
        let default = match eff_param.default.as_ref() {
            Some(expr) => lower.lower_effect_row_expr_in_decl_file_with_scopes(
                &overload.symbol.decl_file,
                type_bindings.iter().cloned(),
                eff_bindings.iter().cloned(),
                Some(expr),
            )?,
            None => EffectRow::pure(),
        };
        eff_bindings.push((name, default));
    }

    let receiver = match &overload.sig.receiver {
        Some(receiver) => Some(lower.lower_type_ref_in_decl_file_with_scopes(
            &overload.symbol.decl_file,
            type_bindings.iter().cloned(),
            eff_bindings.iter().cloned(),
            receiver,
        )?),
        None => None,
    };

    let mut params = Vec::with_capacity(overload.sig.params.len());
    for param in &overload.sig.params {
        let Some(ty) = &param.ty else {
            return Ok(None);
        };
        params.push(lower.lower_type_ref_in_decl_file_with_scopes(
            &overload.symbol.decl_file,
            type_bindings.iter().cloned(),
            eff_bindings.iter().cloned(),
            ty,
        )?);
    }

    Ok(Some(SignatureParts { receiver, params }))
}

fn owner_type_bindings(env: &TypeEnv, owner: OwnerInstantiation<'_>) -> Vec<(String, TypeId)> {
    env.type_symbol(owner.fqn)
        .map(|sym| {
            sym.type_param_names
                .iter()
                .cloned()
                .zip(owner.nominal.args.iter().copied())
                .collect()
        })
        .unwrap_or_default()
}

fn owner_eff_bindings(env: &TypeEnv, owner: OwnerInstantiation<'_>) -> Vec<(String, EffectRow)> {
    let Some(sym) = env.type_symbol(owner.fqn) else {
        return Vec::new();
    };
    let Some(eff_param) = &sym.eff_param else {
        return Vec::new();
    };
    vec![(
        eff_param.name.clone(),
        owner.nominal.eff.clone().unwrap_or_else(EffectRow::pure),
    )]
}
