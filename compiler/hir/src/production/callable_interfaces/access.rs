use super::CallableAccessProjectionError;
use crate::{DeclarationAccess, DeclaredVisibility, PublicLookupAccessV1};

pub(super) fn project(
    access: &DeclarationAccess,
) -> Result<PublicLookupAccessV1, CallableAccessProjectionError> {
    if access.declared != DeclaredVisibility::Public {
        return Err(CallableAccessProjectionError::NotDeclaredPublic);
    }
    if !access.lookup.0.is_universal() {
        return Err(CallableAccessProjectionError::NonUniversalLookup);
    }
    match &access.slot {
        None => Ok(PublicLookupAccessV1::DirectOnly),
        Some(slot) if slot.0.is_universal() => Ok(PublicLookupAccessV1::PublicSlot),
        Some(_) => Err(CallableAccessProjectionError::NonUniversalSlot),
    }
}

pub(super) fn project_direct(
    access: &DeclarationAccess,
) -> Result<PublicLookupAccessV1, CallableAccessProjectionError> {
    let access = project(access)?;
    if access == PublicLookupAccessV1::DirectOnly {
        Ok(access)
    } else {
        Err(CallableAccessProjectionError::PublicSlotNotAllowed)
    }
}
