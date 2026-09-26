//! Closed v1 descriptor and dispatch reference validation.

use super::*;

pub(super) fn validate_optional_type_descriptor_ref(
    decoded: DecodedOptionalStrongTypeDescriptorRefV1,
    identities: &StrongRegistrationIdentitySurfaceV1,
    index: usize,
    field: &'static str,
) -> Result<Option<StrongTypeDescriptorRefV1>, StrongRegistrationProductionValidationError> {
    match decoded {
        DecodedOptionalStrongTypeDescriptorRefV1::Absent => Ok(None),
        DecodedOptionalStrongTypeDescriptorRefV1::Local(exact_type) => resolve_type_descriptor_ref(
            DecodedStrongTypeDescriptorRefV1::Local(exact_type),
            identities,
            index,
            field,
        )
        .map(Some),
        DecodedOptionalStrongTypeDescriptorRefV1::DependencyExternal { provider, exact } => {
            resolve_type_descriptor_ref(
                DecodedStrongTypeDescriptorRefV1::DependencyExternal { provider, exact },
                identities,
                index,
                field,
            )
            .map(Some)
        }
    }
}

fn resolve_type_descriptor_ref(
    decoded: DecodedStrongTypeDescriptorRefV1,
    identities: &StrongRegistrationIdentitySurfaceV1,
    index: usize,
    field: &'static str,
) -> Result<StrongTypeDescriptorRefV1, StrongRegistrationProductionValidationError> {
    match decoded {
        DecodedStrongTypeDescriptorRefV1::Local(exact_type) => resolve_known(
            exact_type,
            identities
                .type_registrations()
                .iter()
                .map(|identity| identity.semantic_id()),
            RegistrationProductionTableV1::Type,
            index,
            field,
        )
        .map(StrongTypeDescriptorRefV1::Local),
        DecodedStrongTypeDescriptorRefV1::DependencyExternal { .. } => Err(semantic_error(
            RegistrationProductionTableV1::Type,
            index,
            field,
        )),
    }
}

fn resolve_dispatch_table(
    decoded: DecodedPersistentId<scoop_identity::PersistentDispatchTableId>,
    expected_key: scoop_identity::DispatchTableKey,
    foundation: &OdrFreeLirFoundation,
    index: usize,
    field: &'static str,
) -> Result<scoop_identity::PersistentDispatchTableId, StrongRegistrationProductionValidationError>
{
    foundation
        .dispatch_tables()
        .iter()
        .find(|record| {
            record.id().as_array() == decoded.as_array() && record.key() == &expected_key
        })
        .map(|record| record.id())
        .ok_or_else(|| semantic_error(RegistrationProductionTableV1::Type, index, field))
}

fn validate_type_dispatch_slots(
    decoded: Vec<DecodedStrongTypeDispatchCallableRefV1>,
    foundation: &OdrFreeLirFoundation,
    external_bridges: &StrongExternalLirBridgeSurfaceV1,
    index: usize,
) -> Result<Vec<StrongTypeDispatchCallableRefV1>, StrongRegistrationProductionValidationError> {
    decoded
        .into_iter()
        .map(|decoded| match decoded {
            DecodedStrongTypeDispatchCallableRefV1::Local(body) => resolve_known(
                body,
                foundation
                    .callable_bodies()
                    .iter()
                    .map(|record| record.id()),
                RegistrationProductionTableV1::Type,
                index,
                "local_dispatch_callable",
            )
            .map(StrongTypeDispatchCallableRefV1::Local),
            DecodedStrongTypeDispatchCallableRefV1::DependencyExternal { provider, body } => {
                external_bridges
                    .resolve_callable_reference(provider, body)
                    .map(
                        |(provider, body)| StrongTypeDispatchCallableRefV1::DependencyExternal {
                            provider,
                            body,
                        },
                    )
                    .ok_or_else(|| {
                        semantic_error(
                            RegistrationProductionTableV1::Type,
                            index,
                            "external_dispatch_callable",
                        )
                    })
            }
            DecodedStrongTypeDispatchCallableRefV1::Runtime(function) => {
                Ok(StrongTypeDispatchCallableRefV1::Runtime(function))
            }
        })
        .collect()
}

mod version;
pub(super) use version::*;
mod dependency;
pub(super) use dependency::*;
