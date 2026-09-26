//! Reference-version adapters for the shared complete type reader.

use super::*;
use crate::{
    DecodedStrongTypeItableSemanticPlan, DecodedStrongTypeVtableSemanticPlan,
    StrongDescriptorReference, StrongTypeItableSemanticPlan, StrongTypeVtableSemanticPlan,
};

pub(in super::super) trait TypeReferences {
    type Parent: WireEncode;
    type DecodedDescriptor: WireEncode;
    type DecodedCallable: WireEncode;
    type Descriptor: StrongDescriptorReference;
    type Callable: Clone + WireEncode;

    fn parent(
        &self,
        decoded: Self::Parent,
        index: usize,
    ) -> Result<Option<Self::Descriptor>, StrongRegistrationProductionValidationError>;
    fn descriptor(
        &self,
        decoded: Self::DecodedDescriptor,
        index: usize,
    ) -> Result<Self::Descriptor, StrongRegistrationProductionValidationError>;
    fn slots(
        &self,
        decoded: Vec<Self::DecodedCallable>,
        index: usize,
    ) -> Result<Vec<Self::Callable>, StrongRegistrationProductionValidationError>;
}

pub(in super::super) struct LegacyTypeReferences<'a> {
    pub foundation: &'a OdrFreeLirFoundation,
    pub identities: &'a StrongRegistrationIdentitySurfaceV1,
    pub external: &'a StrongExternalLirBridgeSurfaceV1,
}

impl TypeReferences for LegacyTypeReferences<'_> {
    type Parent = DecodedOptionalStrongTypeDescriptorRefV1;
    type DecodedDescriptor = DecodedStrongTypeDescriptorRefV1;
    type DecodedCallable = DecodedStrongTypeDispatchCallableRefV1;
    type Descriptor = StrongTypeDescriptorRefV1;
    type Callable = StrongTypeDispatchCallableRefV1;

    fn parent(
        &self,
        decoded: Self::Parent,
        index: usize,
    ) -> Result<Option<Self::Descriptor>, StrongRegistrationProductionValidationError> {
        validate_optional_type_descriptor_ref(decoded, self.identities, index, "parent")
    }
    fn descriptor(
        &self,
        decoded: Self::DecodedDescriptor,
        index: usize,
    ) -> Result<Self::Descriptor, StrongRegistrationProductionValidationError> {
        resolve_type_descriptor_ref(decoded, self.identities, index, "itable_interface")
    }
    fn slots(
        &self,
        decoded: Vec<Self::DecodedCallable>,
        index: usize,
    ) -> Result<Vec<Self::Callable>, StrongRegistrationProductionValidationError> {
        validate_type_dispatch_slots(decoded, self.foundation, self.external, index)
    }
}

pub(in super::super) fn vtable<R: TypeReferences>(
    decoded: DecodedStrongTypeVtableSemanticPlan<R::DecodedCallable>,
    exact_type: scoop_identity::PersistentExactTypeId,
    foundation: &OdrFreeLirFoundation,
    references: &R,
    index: usize,
) -> Result<StrongTypeVtableSemanticPlan<R::Callable>, StrongRegistrationProductionValidationError>
{
    let table = resolve_dispatch_table(
        decoded.table,
        scoop_identity::DispatchTableKey::vtable(exact_type),
        foundation,
        index,
        "vtable",
    )?;
    let slots = references.slots(decoded.slots, index)?;
    Ok(StrongTypeVtableSemanticPlan::from_artifact(table, slots))
}

pub(in super::super) fn itable<R: TypeReferences>(
    decoded: DecodedStrongTypeItableSemanticPlan<R::DecodedDescriptor, R::DecodedCallable>,
    exact_type: scoop_identity::PersistentExactTypeId,
    foundation: &OdrFreeLirFoundation,
    references: &R,
    index: usize,
) -> Result<
    StrongTypeItableSemanticPlan<R::Descriptor, R::Callable>,
    StrongRegistrationProductionValidationError,
> {
    let interface = references.descriptor(decoded.interface, index)?;

    let table = resolve_dispatch_table(
        decoded.table,
        scoop_identity::DispatchTableKey::itable(exact_type, interface.exact_type()),
        foundation,
        index,
        "itable",
    )?;
    let slots = references.slots(decoded.slots, index)?;
    Ok(StrongTypeItableSemanticPlan::from_artifact(
        table, interface, slots,
    ))
}
