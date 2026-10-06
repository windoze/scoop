use super::*;

pub(super) fn claim_accessors(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<(), HirFunctionIdentityError> {
    for (getter, declaration) in inputs.property_getters.iter() {
        if let PropertyAccessorImplementation::Body(function)
        | PropertyAccessorImplementation::StorageBody(function)
        | PropertyAccessorImplementation::AbstractSlot(function) = declaration.implementation
        {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::PropertyGetter,
                raw_index(getter),
                ClaimKind::PropertyAccessor(HirPropertyAccessorFunction::Getter(getter)),
            )?;
        }
    }
    for (setter, declaration) in inputs.property_setters.iter() {
        if let PropertyAccessorImplementation::Body(function)
        | PropertyAccessorImplementation::StorageBody(function)
        | PropertyAccessorImplementation::AbstractSlot(function) = declaration.implementation
        {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::PropertySetter,
                raw_index(setter),
                ClaimKind::PropertyAccessor(HirPropertyAccessorFunction::Setter(setter)),
            )?;
        }
    }
    Ok(())
}

pub(super) fn claim_lexical_functions(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<(), HirFunctionIdentityError> {
    for (lambda, declaration) in inputs.lambdas.iter() {
        let Some((source_function, source_root)) = declaration.definition.source() else {
            continue;
        };
        claim(
            inputs,
            claims,
            source_function,
            FunctionIdentityRelation::Lambda,
            raw_index(lambda),
            ClaimKind::Lexical {
                root: source_root,
                role: LexicalCallableRole::LambdaBody,
                path: declaration.definition_path.clone(),
            },
        )?;
    }
    for (anonymous, declaration) in inputs.anonymous_functions.iter() {
        let Some((source_function, source_root)) = declaration.definition.source() else {
            continue;
        };
        claim(
            inputs,
            claims,
            source_function,
            FunctionIdentityRelation::AnonymousFunction,
            raw_index(anonymous),
            ClaimKind::Lexical {
                root: source_root,
                role: LexicalCallableRole::AnonymousFunctionBody,
                path: declaration.definition_path.clone(),
            },
        )?;
    }
    Ok(())
}

pub(super) fn claim_local_functions(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<(), HirFunctionIdentityError> {
    for (local, declaration) in inputs.local_functions.iter() {
        let Some((source_function, source_root)) = declaration.source() else {
            continue;
        };
        claim(
            inputs,
            claims,
            source_function,
            FunctionIdentityRelation::LocalFunction,
            raw_index(local),
            ClaimKind::LocalSource {
                root: source_root,
                path: declaration.definition_path.clone(),
                declaration_function_type: declaration.declaration_function_type,
                owner_type_parameter_count: declaration.owner_type_param_count,
                origin: declaration.origin,
                capture_bindings: declaration
                    .captures
                    .iter()
                    .map(|capture| capture.binding)
                    .collect(),
            },
        )?;
    }
    Ok(())
}

pub(super) fn claim_initialization(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<(), HirFunctionIdentityError> {
    for (unit, declaration) in inputs.initialization_units.iter() {
        for (function, relation, role) in [
            (
                declaration.initializer,
                FunctionIdentityRelation::InitializationInitializer,
                InitializationCallableRole::Initializer,
            ),
            (
                declaration.ensure,
                FunctionIdentityRelation::InitializationEnsure,
                InitializationCallableRole::Ensure,
            ),
        ] {
            claim(
                inputs,
                claims,
                function,
                relation,
                raw_index(unit),
                ClaimKind::Initialization { unit, role },
            )?;
        }
    }
    Ok(())
}

pub(super) fn claim_derived_equality(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
) -> Result<HashSet<FunctionId>, HirFunctionIdentityError> {
    let mut nominal = HashMap::new();
    for (owner, declaration) in inputs.structs.iter() {
        if let Some(function) = declaration.derived_equality {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::StructDerivedEquality,
                raw_index(owner),
                ClaimKind::DerivedEquality,
            )?;
            if let Some(first) =
                nominal.insert(function, FunctionIdentityRelation::StructDerivedEquality)
            {
                return Err(HirFunctionIdentityError::ConflictingClaim {
                    function: raw_index(function),
                    first,
                    second: FunctionIdentityRelation::StructDerivedEquality,
                });
            }
        }
    }
    for (owner, declaration) in inputs.enums.iter() {
        if let Some(function) = declaration.derived_equality {
            claim(
                inputs,
                claims,
                function,
                FunctionIdentityRelation::EnumDerivedEquality,
                raw_index(owner),
                ClaimKind::DerivedEquality,
            )?;
            if let Some(first) =
                nominal.insert(function, FunctionIdentityRelation::EnumDerivedEquality)
            {
                return Err(HirFunctionIdentityError::ConflictingClaim {
                    function: raw_index(function),
                    first,
                    second: FunctionIdentityRelation::EnumDerivedEquality,
                });
            }
        }
    }
    for (application, declaration) in inputs.derived_equality_applications.iter() {
        claim(
            inputs,
            claims,
            declaration.function,
            FunctionIdentityRelation::DerivedEqualityApplication,
            raw_index(application),
            ClaimKind::DerivedEquality,
        )?;
    }
    Ok(nominal.into_keys().collect())
}

fn claim(
    inputs: &HirFunctionIdentityInputs<'_>,
    claims: &mut [Option<Claim>],
    function: FunctionId,
    relation: FunctionIdentityRelation,
    owner: u32,
    kind: ClaimKind,
) -> Result<(), HirFunctionIdentityError> {
    let index = local_index(function);
    if index >= inputs.functions.len() {
        return Err(HirFunctionIdentityError::UnknownFunction {
            relation,
            owner,
            function: raw_index(function),
        });
    }
    if let Some(existing) = &claims[index] {
        if existing.kind == kind
            && matches!(
                kind,
                ClaimKind::Lexical { .. }
                    | ClaimKind::LocalSource { .. }
                    | ClaimKind::DerivedEquality
            )
        {
            return Ok(());
        }
        return Err(HirFunctionIdentityError::ConflictingClaim {
            function: raw_index(function),
            first: existing.relation,
            second: relation,
        });
    }
    claims[index] = Some(Claim { relation, kind });
    Ok(())
}
