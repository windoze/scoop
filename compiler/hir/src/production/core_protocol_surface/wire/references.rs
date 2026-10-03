use super::*;

pub(super) fn validate_entry(
    decoded: DecodedCoreProtocolEntryV1,
    foundation: &CanonicalHirFoundation,
) -> Result<CoreProtocolEntryV1, CoreCompilerProtocolSurfaceValidationError> {
    match decoded {
        DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::Type(id)) => {
            let (id, _) = foundation.source_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownType(*id.as_array()),
            )?;
            if id != CoreBuiltinNominal::Unit.identity_record().id() {
                require_origin(foundation, DefinitionOriginSubject::Type(id))?;
            }
            Ok(CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(
                id,
            )))
        }
        DecodedCoreProtocolEntryV1::Nominal(DecodedCoreProtocolNominalV1::GenericType(id)) => {
            let (id, _) = foundation.generic_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownGenericType(*id.as_array()),
            )?;
            require_origin(foundation, DefinitionOriginSubject::GenericType(id))?;
            Ok(CoreProtocolEntryV1::Nominal(
                CoreProtocolNominalV1::GenericType(id),
            ))
        }
        DecodedCoreProtocolEntryV1::Callable(callable) => callable
            .validate_against(foundation)
            .map(CoreProtocolEntryV1::Callable)
            .map_err(CoreCompilerProtocolSurfaceValidationError::Callable),
        DecodedCoreProtocolEntryV1::EnumVariant(id) => {
            let (id, key) = foundation.enum_variant_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariant(*id.as_array()),
            )?;
            require_source_enum_owner(foundation, key.source_owner())?;
            require_origin(foundation, DefinitionOriginSubject::EnumVariant(id))?;
            Ok(CoreProtocolEntryV1::EnumVariant(id))
        }
        DecodedCoreProtocolEntryV1::EnumVariantField(id) => {
            let (id, key) = foundation
                .enum_variant_field_by_bytes(id.as_array())
                .ok_or(
                    CoreCompilerProtocolSurfaceValidationError::UnknownEnumVariantField(
                        *id.as_array(),
                    ),
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
            Ok(CoreProtocolEntryV1::EnumVariantField(id))
        }
        DecodedCoreProtocolEntryV1::DispatchSlot(id) => {
            let (id, _) = foundation.dispatch_slot_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownDispatchSlot(*id.as_array()),
            )?;
            Ok(CoreProtocolEntryV1::DispatchSlot(id))
        }
        DecodedCoreProtocolEntryV1::ExactType(id) => {
            let (id, _) = foundation.exact_type_by_bytes(id.as_array()).ok_or(
                CoreCompilerProtocolSurfaceValidationError::UnknownExactType(*id.as_array()),
            )?;
            Ok(CoreProtocolEntryV1::ExactType(id))
        }
    }
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
