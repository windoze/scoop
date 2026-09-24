use super::*;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourceSlotSelectionsV1, Error> {
    let mut records = Vec::new();
    for nominal in nominals {
        let owner = match nominal.local {
            NominalLocalId::Class(id) => NominalOwner::Class(id),
            NominalLocalId::Interface(id) => NominalOwner::Interface(id),
            NominalLocalId::Struct(id) => NominalOwner::Struct(id),
            NominalLocalId::Enum(id) => NominalOwner::Enum(id),
            NominalLocalId::Object(id) => NominalOwner::Object(id),
        };
        let selections = crate::production::nominal_dispatch::project(export, owner, meter)?;
        for selection in selections.records() {
            meter
                .try_reserve_collection_slots(&mut records, 1, &WirePath::root())
                .map_err(resource)?;
            records.push(InheritanceSourceSlotSelectionRecordV1::new(
                nominal.exact,
                selection.slot(),
                selection.selection(),
            ));
        }
    }
    CanonicalInheritanceSourceSlotSelectionsV1::try_new(records, meter)
        .map_err(Error::SourceInventory)
}
