use super::*;
use scoop_identity::{PersistentCallableBodyId, PersistentExactTypeId};

pub(super) fn validate_policy(
    policy: crate::ReleasePolicy<DecodedPersistentId<PersistentCallableBodyId>>,
    owner: PersistentExactTypeId,
    shape: &crate::TypeInstanceShapeV1,
    foundation: &ConeLirFoundation,
    index: usize,
) -> Result<crate::ReleasePolicy, StrongRegistrationProductionValidationError> {
    let crate::ReleasePolicy::SynchronousGcFree { hook } = policy else {
        return Ok(crate::ReleasePolicy::None);
    };
    let record = foundation
        .callable_bodies()
        .iter()
        .find(|record| record.id().as_array() == hook.as_array())
        .ok_or_else(|| {
            semantic_error(RegistrationProductionTableV1::Type, index, "release_hook")
        })?;
    if shape.instance_kind() != crate::TypeInstanceKindV1::FixedObject
        || !matches!(record.key().kind(), scoop_identity::CallableBodyKeyKind::ReleaseHook { owner: actual } if actual == owner)
    {
        return Err(semantic_error(
            RegistrationProductionTableV1::Type,
            index,
            "release_hook_owner",
        ));
    }
    Ok(crate::ReleasePolicy::SynchronousGcFree { hook: record.id() })
}
