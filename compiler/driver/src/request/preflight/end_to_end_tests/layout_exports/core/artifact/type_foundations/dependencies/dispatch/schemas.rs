use super::*;

pub(super) fn check(
    checked: CheckedSharedTypeFoundationV1<'_>,
    core: CheckedSharedTypeFoundationV1<'_>,
) {
    for interface in [false, true] {
        let mut records = checked.section().inheritance().records().to_vec();
        let record = records
            .iter_mut()
            .find(|record| {
                record.slot_schemas().records().iter().any(|schema| {
                    matches!(schema.role(), InheritanceSlotSchemaRoleV1::Interface { .. })
                        == interface
                        && schema.slots().len() > 1
                })
            })
            .unwrap();
        let mut schemas = record.slot_schemas().records().to_vec();
        let schema = schemas
            .iter_mut()
            .find(|schema| {
                matches!(schema.role(), InheritanceSlotSchemaRoleV1::Interface { .. }) == interface
                    && schema.slots().len() > 1
            })
            .unwrap();
        let mut slots = schema.slots().to_vec();
        slots.swap(0, 1);
        *schema = InheritanceSlotSchemaV1::try_new(schema.role(), slots).unwrap();
        *record = NominalInheritanceInterfaceV1::try_new(
            record.edges().clone(),
            record.constructors().clone(),
            record.slots().clone(),
            record.protected_members().clone(),
            CanonicalInheritanceSlotSchemasV1::try_new(schemas).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            reject_section(checked, core, records),
            Error::SlotOrder(_) | Error::SlotSchemas(_)
        ));
    }
}
