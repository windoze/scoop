use std::collections::HashMap;

use super::*;

pub(super) fn validate_immortal_objects(module: &Module) -> Result<(), MirValidationError> {
    let mut identities = HashMap::with_capacity(module.strings.len());
    for (string, constant) in module.strings.iter() {
        if let Some(previous) = identities.insert(&constant.identity, string) {
            return Err(MirValidationError {
                location: MirValidationLocation::StringConstant { string },
                kind: MirValidationErrorKind::DuplicateImmortalObjectIdentity { previous },
            });
        }
    }
    Ok(())
}
