use super::*;

pub(super) fn read_concrete_nominal(
    decoded: DecodedCoreProtocolEntryV1,
    index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<PersistentTypeId, CoreCompilerProtocolSurfaceValidationError> {
    let DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::Type(id)) = decoded
    else {
        return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
    };
    let (id, _) = foundation.source_type_by_bytes(id.as_array()).ok_or(
        CoreCompilerProtocolSurfaceValidationError::UnknownType(*id.as_array()),
    )?;
    if id != CoreBuiltinNominal::Unit.identity_record().id() {
        require_origin(foundation, DefinitionOriginSubject::Type(id))?;
    }
    Ok(id)
}

pub(super) fn read_generic_nominal(
    decoded: DecodedCoreProtocolEntryV1,
    index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<PersistentGenericTypeId, CoreCompilerProtocolSurfaceValidationError> {
    let DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::GenericType(id)) =
        decoded
    else {
        return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
    };
    let (id, _) = foundation
        .generic_type_by_bytes(id.as_array())
        .ok_or(CoreCompilerProtocolSurfaceValidationError::UnknownGenericType(*id.as_array()))?;
    require_origin(foundation, DefinitionOriginSubject::GenericType(id))?;
    Ok(id)
}

pub(super) fn read_callable(
    decoded: DecodedCoreProtocolEntryV1,
    index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<CoreProtocolCallableV1, CoreCompilerProtocolSurfaceValidationError> {
    let DecodedCoreProtocolEntryV1::Callable(callable) = decoded else {
        return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
    };
    callable
        .validate_against(foundation)
        .map_err(CoreCompilerProtocolSurfaceValidationError::Callable)
}

pub(super) fn read_enum_variant(
    decoded: DecodedCoreProtocolEntryV1,
    index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<PersistentEnumVariantId, CoreCompilerProtocolSurfaceValidationError> {
    let DecodedCoreProtocolEntryV1::EnumVariant(id) = decoded else {
        return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
    };
    let (id, key) = foundation
        .enum_variant_by_bytes(id.as_array())
        .ok_or(CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariant(*id.as_array()))?;
    require_source_enum_owner(foundation, key.source_owner())?;
    require_origin(foundation, DefinitionOriginSubject::EnumVariant(id))?;
    Ok(id)
}

pub(super) fn read_enum_variant_field(
    decoded: DecodedCoreProtocolEntryV1,
    index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<PersistentEnumVariantFieldId, CoreCompilerProtocolSurfaceValidationError> {
    let DecodedCoreProtocolEntryV1::EnumVariantField(id) = decoded else {
        return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
    };
    let (id, key) = foundation
        .enum_variant_field_by_bytes(id.as_array())
        .ok_or(
            CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariantField(*id.as_array()),
        )?;
    let (_, variant) = foundation
        .enum_variant_by_bytes(key.variant().as_array())
        .ok_or(
            CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariant(
                *key.variant().as_array(),
            ),
        )?;
    require_source_enum_owner(foundation, variant.source_owner())?;
    require_origin(foundation, DefinitionOriginSubject::EnumVariantField(id))?;
    Ok(id)
}

pub(super) fn read_dispatch_slot(
    decoded: DecodedCoreProtocolEntryV1,
    index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<PersistentDispatchSlotId, CoreCompilerProtocolSurfaceValidationError> {
    let DecodedCoreProtocolEntryV1::DispatchSlot(id) = decoded else {
        return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
    };
    let (id, _) = foundation
        .dispatch_slot_by_bytes(id.as_array())
        .ok_or(CoreCompilerProtocolSurfaceValidationError::UnknownDispatchSlot(*id.as_array()))?;
    Ok(id)
}

pub(super) fn read_exact_type(
    decoded: DecodedCoreProtocolEntryV1,
    index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<PersistentExactTypeId, CoreCompilerProtocolSurfaceValidationError> {
    let DecodedCoreProtocolEntryV1::ExactType(id) = decoded else {
        return Err(CoreCompilerProtocolSurfaceValidationError::RoleKindMismatch { index });
    };
    let (id, _) = foundation
        .exact_type_by_bytes(id.as_array())
        .ok_or(CoreCompilerProtocolSurfaceValidationError::UnknownExactType(*id.as_array()))?;
    Ok(id)
}

fn require_source_enum_owner(
    foundation: &CanonicalHirFoundation,
    owner: Option<NominalDeclarationOwner>,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source =
        match owner.ok_or(CoreCompilerProtocolSurfaceValidationError::GeneratedEnumMember)? {
            NominalDeclarationOwner::Concrete(id) => {
                let (_, source) = foundation.source_type_by_bytes(id.as_array()).ok_or(
                    CoreCompilerProtocolSurfaceValidationError::UnknownType(*id.as_array()),
                )?;
                source
            }
            NominalDeclarationOwner::GenericTemplate(id) => {
                let (_, source) = foundation.generic_type_by_bytes(id.as_array()).ok_or(
                    CoreCompilerProtocolSurfaceValidationError::UnknownGenericType(*id.as_array()),
                )?;
                source
            }
        };
    if source.declaration_kind() != SourceDeclarationKind::Enum {
        return Err(CoreCompilerProtocolSurfaceValidationError::ExpectedSourceEnumOwner);
    }
    Ok(())
}

fn require_origin(
    foundation: &CanonicalHirFoundation,
    subject: DefinitionOriginSubject,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    if foundation.definition_origin(subject).is_some() {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::MissingDefinitionOrigin(subject))
    }
}

#[cfg(test)]
mod tests;
