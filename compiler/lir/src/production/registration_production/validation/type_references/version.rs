//! Reference-version adapters for the shared complete type reader.

use super::*;
use crate::{
    DecodedStrongTypeItableSemanticPlan, DecodedStrongTypeVtableSemanticPlan,
    StrongDescriptorReference, StrongTypeItableSemanticPlan, StrongTypeVtableSemanticPlan,
};
use scoop_wire::{BudgetMeter, WirePath};

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
        meter: &mut BudgetMeter,
    ) -> Result<Option<Self::Descriptor>, StrongRegistrationProductionValidationError>;
    fn descriptor(
        &self,
        decoded: Self::DecodedDescriptor,
        index: usize,
        meter: &mut BudgetMeter,
    ) -> Result<Self::Descriptor, StrongRegistrationProductionValidationError>;
    fn slots(
        &self,
        decoded: Vec<Self::DecodedCallable>,
        index: usize,
        meter: &mut BudgetMeter,
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
        meter: &mut BudgetMeter,
    ) -> Result<Option<Self::Descriptor>, StrongRegistrationProductionValidationError> {
        self.charge_descriptor_search(meter)?;
        validate_optional_type_descriptor_ref(
            decoded,
            self.identities,
            self.external,
            index,
            "parent",
        )
    }
    fn descriptor(
        &self,
        decoded: Self::DecodedDescriptor,
        index: usize,
        meter: &mut BudgetMeter,
    ) -> Result<Self::Descriptor, StrongRegistrationProductionValidationError> {
        self.charge_descriptor_search(meter)?;
        resolve_type_descriptor_ref(
            decoded,
            self.identities,
            self.external,
            index,
            "itable_interface",
        )
    }
    fn slots(
        &self,
        decoded: Vec<Self::DecodedCallable>,
        index: usize,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<Self::Callable>, StrongRegistrationProductionValidationError> {
        let path = WirePath::root();
        meter.charge_collection_slots(decoded.len() as u64, &path)?;
        meter.charge_work(
            (decoded.len() as u64).saturating_mul(
                (self.foundation.callable_bodies().len() as u64)
                    .saturating_add(self.external.bridges().len() as u64)
                    .saturating_add(1),
            ),
            &path,
        )?;
        validate_type_dispatch_slots(decoded, self.foundation, self.external, index)
    }
}

impl LegacyTypeReferences<'_> {
    fn charge_descriptor_search(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<(), scoop_wire::WireError> {
        meter.charge_work(
            (self.identities.type_registrations().len() as u64)
                .saturating_add(self.external.bridges().len() as u64)
                .saturating_add(1),
            &WirePath::root(),
        )
    }
}

pub(in super::super) fn vtable<R: TypeReferences>(
    decoded: DecodedStrongTypeVtableSemanticPlan<R::DecodedCallable>,
    exact_type: scoop_identity::PersistentExactTypeId,
    foundation: &OdrFreeLirFoundation,
    references: &R,
    index: usize,
    meter: &mut BudgetMeter,
) -> Result<StrongTypeVtableSemanticPlan<R::Callable>, StrongRegistrationProductionValidationError>
{
    meter.charge_work(foundation.dispatch_tables().len() as u64, &WirePath::root())?;
    let table = resolve_dispatch_table(
        decoded.table,
        scoop_identity::DispatchTableKey::vtable(exact_type),
        foundation,
        index,
        "vtable",
    )?;
    let slots = references.slots(decoded.slots, index, meter)?;
    Ok(StrongTypeVtableSemanticPlan::from_artifact(table, slots))
}

pub(in super::super) fn itable<R: TypeReferences>(
    decoded: DecodedStrongTypeItableSemanticPlan<R::DecodedDescriptor, R::DecodedCallable>,
    exact_type: scoop_identity::PersistentExactTypeId,
    foundation: &OdrFreeLirFoundation,
    references: &R,
    index: usize,
    meter: &mut BudgetMeter,
) -> Result<
    StrongTypeItableSemanticPlan<R::Descriptor, R::Callable>,
    StrongRegistrationProductionValidationError,
> {
    let interface = references.descriptor(decoded.interface, index, meter)?;
    meter.charge_work(foundation.dispatch_tables().len() as u64, &WirePath::root())?;
    let table = resolve_dispatch_table(
        decoded.table,
        scoop_identity::DispatchTableKey::itable(exact_type, interface.exact_type()),
        foundation,
        index,
        "itable",
    )?;
    let slots = references.slots(decoded.slots, index, meter)?;
    Ok(StrongTypeItableSemanticPlan::from_artifact(
        table, interface, slots,
    ))
}
