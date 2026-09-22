//! Immortal object registration validation.

use super::*;

pub(super) fn validate_immortal_objects(
    decoded: Vec<DecodedStrongImmortalObjectRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    external_bridges: &StrongExternalLirBridgeSurfaceV1,
) -> Result<StrongImmortalObjectSemanticPlanSetV1, StrongRegistrationProductionValidationError> {
    require_length(
        RegistrationProductionTableV1::ImmortalObject,
        decoded.len(),
        identities.immortal_objects().len(),
    )?;
    let alignment = target.metadata_pointer_layout().alignment_bytes().max(
        target
            .scalar_layout(BackendScalarKind::I64)
            .alignment_bytes(),
    );
    let minimum_size = target
        .metadata_pointer_layout()
        .size_bytes()
        .checked_add(
            target
                .scalar_layout(BackendScalarKind::I64)
                .size_bytes()
                .saturating_mul(2),
        )
        .ok_or_else(|| {
            semantic_error(
                RegistrationProductionTableV1::ImmortalObject,
                0,
                "object_size",
            )
        })?;
    let mut objects = Vec::with_capacity(decoded.len());
    for (index, (decoded, identity)) in decoded
        .into_iter()
        .zip(identities.immortal_objects())
        .enumerate()
    {
        let object = verify_expected(
            decoded.object,
            identity.semantic_id(),
            RegistrationProductionTableV1::ImmortalObject,
            index,
            "object",
        )?;
        if decoded.required_alignment != alignment
            || decoded.object_size < minimum_size
            || decoded.object_size % alignment != 0
            || decoded.object_size > target.contract().maximum_managed_object_size()
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::ImmortalObject,
                index,
                "object_shape",
            ));
        }
        let type_registration = match decoded.type_registration {
            DecodedImmortalObjectTypeRegistrationRefV1::Local(exact_type) => {
                let exact_type = resolve_known(
                    exact_type,
                    identities
                        .type_registrations()
                        .iter()
                        .map(|identity| identity.semantic_id()),
                    RegistrationProductionTableV1::ImmortalObject,
                    index,
                    "local_type_registration",
                )?;
                ImmortalObjectTypeRegistrationRefV1::Local(exact_type)
            }
            DecodedImmortalObjectTypeRegistrationRefV1::DependencyExternal { provider, exact } => {
                let descriptor = external_bridges
                    .resolve_descriptor_reference(provider, exact)
                    .ok_or_else(|| {
                        semantic_error(
                            RegistrationProductionTableV1::ImmortalObject,
                            index,
                            "external_type_registration",
                        )
                    })?;
                ImmortalObjectTypeRegistrationRefV1::DependencyExternal {
                    provider: descriptor.provider(),
                    exact: descriptor.target(),
                }
            }
        };
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::ImmortalObject(object),
            LinkageClass::ConeStrong,
        )
        .map_err(|_| {
            semantic_error(
                RegistrationProductionTableV1::ImmortalObject,
                index,
                "object_symbol",
            )
        })?;
        objects.push(StrongImmortalObjectSemanticPlanV1::from_artifact(
            object,
            symbol,
            decoded.object_size,
            decoded.required_alignment,
            type_registration,
        ));
    }
    Ok(StrongImmortalObjectSemanticPlanSetV1::from_artifact(
        foundation.producer(),
        objects,
    ))
}
