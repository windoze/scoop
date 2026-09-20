use super::*;
use crate::{
    CheckedInheritanceSlotContractV1, InheritanceSlotContractV1, InheritanceSlotImplementationV1,
};

/// Source signature, access and selected implementation for one owner/slot.
/// Whole-table completeness and machine-use permissions remain separate.
#[derive(Clone, Copy, Debug)]
pub struct CheckedInheritanceSourceSlotContractV1<'a> {
    contract: CheckedInheritanceSlotContractV1<'a>,
}
impl CheckedInheritanceSourceSlotContractV1<'_> {
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.contract.owner()
    }
    pub const fn record(&self) -> &InheritanceSlotContractV1 {
        self.contract.record()
    }
}
impl CheckedNominalInheritanceGraphV1<'_> {
    pub fn validate_slot_source_contract<'a, A: InheritanceSlotSourceSemanticAuthority<E>, E>(
        &self,
        owner: PersistentExactTypeId,
        slot: &'a InheritanceSlotContractV1,
        authority: &A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedInheritanceSourceSlotContractV1<'a>, InheritanceInterfaceSemanticError<E>>
    {
        use InheritanceInterfaceSemanticError as Error;
        let contract = self
            .validate_slot_contract(owner, slot, authority, meter)
            .map_err(Error::Slot)?;
        let source = authority
            .inheritance_callable_source(slot.declaration())
            .map_err(Error::Foundation)?;
        compare(slot.signature(), source.signature, meter)?;
        compare(slot.declaration_access(), source.declaration_access, meter)?;
        if source.modality == CallableModalityV1::Final {
            return Err(Error::SourceContract);
        }
        let actual_selection = match slot.implementation() {
            InheritanceSlotImplementationV1::Abstract => InheritanceSourceSlotSelectionV1::Abstract,
            InheritanceSlotImplementationV1::Concrete(target) => {
                InheritanceSourceSlotSelectionV1::Concrete(target.declaration())
            }
            InheritanceSlotImplementationV1::InterfaceDefault(target) => {
                InheritanceSourceSlotSelectionV1::InterfaceDefault(target.declaration())
            }
        };
        meter
            .charge_work(1, &WirePath::root())
            .map_err(Error::Resource)?;
        if actual_selection
            != authority
                .inheritance_slot_selection(owner, slot.slot())
                .map_err(Error::Foundation)?
        {
            return Err(Error::SlotSelection);
        }
        if let Some(target) = slot.implementation().target() {
            let source = authority
                .inheritance_callable_source(target.declaration())
                .map_err(Error::Foundation)?;
            compare(target.signature(), source.signature, meter)?;
            compare(
                target.declaration_access(),
                source.declaration_access,
                meter,
            )?;
            if target.modality() != source.modality {
                return Err(Error::SourceContract);
            }
        }
        Ok(CheckedInheritanceSourceSlotContractV1 { contract })
    }
}
pub(super) fn validate<A: NominalInheritanceInterfaceSemanticAuthority<E>, E>(
    record: &NominalInheritanceInterfaceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &A,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceInterfaceSemanticError<E>> {
    for slot in record.slots().records() {
        graph.validate_slot_source_contract(record.owner(), slot, authority, meter)?;
    }
    Ok(())
}
