use super::*;
use scoop_wire::{BudgetMeter, WirePath};

pub(in super::super) struct DependencyTypeReferences<'a> {
    pub local: LegacyTypeReferences<'a>,
    pub definitions: &'a crate::StrongTypeReferenceDefinitionsV2,
}

impl TypeReferences for DependencyTypeReferences<'_> {
    type Parent = crate::DecodedOptionalStrongTypeDescriptorRefV2;
    type DecodedDescriptor = crate::DecodedStrongTypeDescriptorRefV2;
    type DecodedCallable = crate::DecodedStrongTypeDispatchCallableRefV2;
    type Descriptor = crate::StrongTypeDescriptorRefV2;
    type Callable = crate::StrongTypeDispatchCallableRefV2;

    fn parent(
        &self,
        decoded: Self::Parent,
        _: usize,
        meter: &mut BudgetMeter,
    ) -> Result<Option<Self::Descriptor>, StrongRegistrationProductionValidationError> {
        self.definitions
            .resolve_optional_descriptor(
                decoded,
                self.local.foundation,
                self.local.identities,
                self.local.external,
                meter,
            )
            .map_err(Into::into)
    }
    fn descriptor(
        &self,
        decoded: Self::DecodedDescriptor,
        _: usize,
        meter: &mut BudgetMeter,
    ) -> Result<Self::Descriptor, StrongRegistrationProductionValidationError> {
        self.definitions
            .resolve_descriptor(
                decoded,
                self.local.foundation,
                self.local.identities,
                self.local.external,
                meter,
            )
            .map_err(Into::into)
    }
    fn slots(
        &self,
        decoded: Vec<Self::DecodedCallable>,
        _: usize,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<Self::Callable>, StrongRegistrationProductionValidationError> {
        let mut slots = Vec::new();
        meter.try_reserve_collection_slots(&mut slots, decoded.len(), &WirePath::root())?;
        for decoded in decoded {
            slots.push(self.definitions.resolve_dispatch_callable(
                decoded,
                self.local.foundation,
                self.local.external,
                meter,
            )?);
        }
        Ok(slots)
    }
}
