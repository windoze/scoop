use super::*;
use crate::{
    CheckedInheritanceSlotContractV1, InheritanceSlotContractV1, InheritanceSlotImplementationV1,
};

/// Source signature, access and selected implementation for one owner/slot.
/// The artifact reader retains the checked contract for subsequent use.
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
    ) -> Result<CheckedInheritanceSourceSlotContractV1<'a>, InheritanceInterfaceSemanticError<E>>
    {
        use InheritanceInterfaceSemanticError as Error;
        let contract = self
            .validate_slot_contract(owner, slot, authority)
            .map_err(Error::Slot)?;
        let source = authority
            .inheritance_callable_source(slot.declaration(), slot.signature().receiver())
            .map_err(Error::Foundation)?;
        compare(slot.signature(), source.signature)?;
        compare(slot.declaration_access(), source.declaration_access)?;
        if source.modality == CallableModalityV1::Final {
            return Err(Error::SourceContract);
        }
        let actual_selection = match slot.implementation() {
            InheritanceSlotImplementationV1::Abstract(target) => {
                InheritanceSourceSlotSelectionV1::Abstract(target.declaration())
            }
            InheritanceSlotImplementationV1::Concrete(target) => {
                InheritanceSourceSlotSelectionV1::Concrete(target.declaration())
            }
            InheritanceSlotImplementationV1::InterfaceDefault(target) => {
                InheritanceSourceSlotSelectionV1::InterfaceDefault(target.declaration())
            }
        };

        if actual_selection
            != authority
                .inheritance_slot_selection(owner, slot.role(), slot.slot())
                .map_err(Error::Foundation)?
        {
            return Err(Error::SlotSelection);
        }
        let target = slot.implementation().target();
        let source = authority
            .inheritance_callable_source(target.declaration(), target.signature().receiver())
            .map_err(Error::Foundation)?;
        compare(target.signature(), source.signature)?;
        compare(target.declaration_access(), source.declaration_access)?;
        if target.modality() != source.modality {
            return Err(Error::SourceContract);
        }
        Ok(CheckedInheritanceSourceSlotContractV1 { contract })
    }
}
