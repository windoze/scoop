use std::collections::HashMap;

use super::*;

pub(super) fn validate_immortal_objects(module: &Module) -> Result<(), MirValidationError> {
    let mut identities = HashMap::with_capacity(module.strings.len());
    for (string, constant) in module.strings.iter() {
        let invalid_owner = match constant.identity.owner() {
            scoop_identity::ImmortalObjectOwner::Property(_) => None,
            scoop_identity::ImmortalObjectOwner::Callable(materialization) => module
                .meta
                .source_callable_materializations
                .get_by_materialization(materialization)
                .is_none()
                .then_some("the callable materialization does not exist"),
            scoop_identity::ImmortalObjectOwner::InitializationUnit(identity) => module
                .initialization_units
                .iter()
                .all(|(_, unit)| unit.identity.id() != identity)
                .then_some("the initialization unit does not exist"),
        };
        if let Some(reason) = invalid_owner {
            return Err(MirValidationError {
                location: MirValidationLocation::StringConstant { string },
                kind: MirValidationErrorKind::InvalidImmortalObjectOwner { reason },
            });
        }
        if let Some(previous) = identities.insert(&constant.identity, string) {
            return Err(MirValidationError {
                location: MirValidationLocation::StringConstant { string },
                kind: MirValidationErrorKind::DuplicateImmortalObjectIdentity { previous },
            });
        }
    }
    Ok(())
}
