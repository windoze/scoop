use super::*;

pub(super) fn validate_foreign_callback_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let callback = &surface.foreign_callback_protocol;
    validate_variant_owners(
        callback.mode,
        &[(2, callback.reusable), (3, callback.one_shot)],
        foundation,
    )?;
    validate_variant_owners(
        callback.state,
        &[
            (5, callback.registered),
            (6, callback.active),
            (7, callback.completed),
            (8, callback.failed),
        ],
        foundation,
    )?;
    let failure = callback.failure_result;
    let (_, failure_key) = foundation
        .exact_type_by_bytes(failure.as_array())
        .expect("the callback failure exact type was resolved");
    let option = surface.option_protocol.option;
    let throwable = surface.exception_protocol.throwable;
    let valid = match failure_key {
        scoop_identity::ExactTypeKey::NominalApplication { origin, arguments }
            if *origin == option && arguments.as_slice().len() == 1 =>
        {
            foundation
                .exact_type_by_bytes(arguments.as_slice()[0].as_array())
                .is_some_and(|(_, key)| key == &scoop_identity::ExactTypeKey::Nominal(throwable))
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::CallbackFailureTypeMismatch)
    }
}

fn validate_variant_owners(
    owner: PersistentTypeId,
    variants: &[(usize, PersistentEnumVariantId)],
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let owner = NominalDeclarationOwner::Concrete(owner);
    for &(index, variant) in variants {
        let (_, key) = foundation
            .enum_variant_by_bytes(variant.as_array())
            .expect("the callback enum variant was resolved");
        if key.source_owner() != Some(owner) {
            return Err(
                CoreCompilerProtocolSurfaceValidationError::CallbackVariantOwnerMismatch { index },
            );
        }
    }
    Ok(())
}

pub(super) fn validate_option_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let option = NominalDeclarationOwner::GenericTemplate(surface.option_protocol.option);
    let some = surface.option_some();
    let none = surface.option_none();
    for variant in [some, none] {
        let (_, key) = foundation
            .enum_variant_by_bytes(variant.as_array())
            .expect("the Option variant was resolved");
        if key.source_owner() != Some(option) {
            return Err(CoreCompilerProtocolSurfaceValidationError::OptionOwnerMismatch);
        }
    }
    for (variant, expected) in [(some, 1), (none, 0)] {
        let actual = foundation.enum_variant_field_count(variant);
        if actual != expected {
            return Err(
                CoreCompilerProtocolSurfaceValidationError::OptionVariantFieldCount {
                    variant,
                    expected,
                    actual,
                },
            );
        }
    }
    let field = surface.option_some_payload();
    let (_, key) = foundation
        .enum_variant_field_by_bytes(field.as_array())
        .expect("the Option payload field was resolved");
    if key.variant() != some
        || key.selector()
            != &(EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            })
    {
        return Err(CoreCompilerProtocolSurfaceValidationError::OptionPayloadMismatch);
    }
    Ok(())
}
