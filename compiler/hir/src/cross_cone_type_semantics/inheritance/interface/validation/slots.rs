use super::*;
use crate::InheritanceSlotImplementationV1;

pub(super) fn validate<A: NominalInheritanceInterfaceSemanticAuthority<E>, E>(
    record: &NominalInheritanceInterfaceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    authority: &A,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceInterfaceSemanticError<E>> {
    use InheritanceInterfaceSemanticError as Error;
    for slot in record.slots().records() {
        graph
            .validate_slot_contract(record.owner(), slot, authority, meter)
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
                .inheritance_slot_selection(record.owner(), slot.slot())
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
    }
    Ok(())
}
