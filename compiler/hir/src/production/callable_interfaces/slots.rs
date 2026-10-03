use super::CallableProjectionError as Error;
use crate::{
    CanonicalPersistentIdsV1, ExportHir, Method, MethodDispatch, PropertyAccessorImplementation,
};
use scoop_identity::PersistentDispatchSlotId;

pub(super) fn method(
    export: &ExportHir,
    method: Option<Method>,
) -> Result<CanonicalPersistentIdsV1<PersistentDispatchSlotId>, Error> {
    let slot = match method.map(|method| method.dispatch) {
        None | Some(MethodDispatch::Direct) => return Ok(CanonicalPersistentIdsV1::empty()),
        Some(MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family)) => {
            export.dispatch_slot_identities.get_virtual(family)
        }
        Some(MethodDispatch::Interface(member)) => {
            export.dispatch_slot_identities.get_interface(member)
        }
    }
    .ok_or(Error::MissingIdentity)?
    .id();
    CanonicalPersistentIdsV1::try_new(vec![slot]).map_err(|_| Error::MissingIdentity)
}

pub(super) fn accessor(
    export: &ExportHir,
    implementation: PropertyAccessorImplementation,
) -> Result<CanonicalPersistentIdsV1<PersistentDispatchSlotId>, Error> {
    match implementation {
        PropertyAccessorImplementation::Body(function)
        | PropertyAccessorImplementation::AbstractSlot(function) => {
            let function =
                super::arena_get(&export.functions, function).ok_or(Error::UnknownDeclaration)?;
            method(export, function.method)
        }
        PropertyAccessorImplementation::Storage | PropertyAccessorImplementation::Constant => {
            Ok(CanonicalPersistentIdsV1::empty())
        }
    }
}
