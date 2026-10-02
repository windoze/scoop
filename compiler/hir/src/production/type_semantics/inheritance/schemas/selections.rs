use super::super::source_errors::invalid;
use super::*;

pub(in crate::production::type_semantics) fn project(
    metadata: SharedTypeMetadataV1<'_>,
    nominals: &[ConcreteNominal<'_>],
) -> Result<CanonicalInheritanceSourceSlotSelectionsV1, Error> {
    let mut records = Vec::new();
    for nominal in nominals {
        let selections = metadata
            .public
            .nominal_interfaces()
            .declaration(SourceNominalId::Concrete(nominal.owner))
            .ok_or_else(|| invalid("nominal has no shared dispatch declaration"))?
            .declaration_details()
            .dispatch_selections();
        for selection in selections.records() {
            scoop_wire::allocation::try_reserve(&mut records, 1, &WirePath::root())
                .map_err(resource)?;
            records.push(InheritanceSourceSlotSelectionRecordV1::new(
                nominal.exact,
                match selection.role() {
                    NominalDispatchSelectionRoleV1::ClassVtable => {
                        InheritanceSlotSchemaRoleV1::ClassVtable
                    }
                    NominalDispatchSelectionRoleV1::Interface { interface } => {
                        InheritanceSlotSchemaRoleV1::Interface {
                            interface_exact: metadata
                                .signature_exact_type(interface)
                                .map_err(invalid)?,
                        }
                    }
                },
                metadata
                    .signature_exact_type(selection.receiver())
                    .map_err(invalid)?,
                selection.slot(),
                selection.selection(),
            ));
        }
    }
    CanonicalInheritanceSourceSlotSelectionsV1::try_new(records).map_err(Error::SourceInventory)
}
