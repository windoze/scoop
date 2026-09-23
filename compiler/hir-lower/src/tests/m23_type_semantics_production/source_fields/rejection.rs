use super::*;

fn replace_fields(
    table: &hir::CanonicalNominalInterfacesV1,
    owner: hir::SourceNominalId,
    fields: Vec<hir::NominalSourceFieldV1>,
) -> hir::CanonicalNominalInterfacesV1 {
    hir::CanonicalNominalInterfacesV1::with_support(
        table
            .records()
            .iter()
            .map(|record| {
                if record.declaration() != owner {
                    return record.clone();
                }
                hir::NominalInterfaceRecordV1::try_new(
                    record.declaration(),
                    record.kind(),
                    record.type_parameters().clone(),
                    record.exact_supertypes().clone(),
                    record.constructors().clone(),
                    record.members().clone(),
                    record.nested_bindings().clone(),
                    hir::NominalSourceShapeV1::Class(
                        hir::NominalSourceFieldsV1::try_new(fields.clone()).unwrap(),
                    ),
                    record.declaration_details().clone(),
                )
                .unwrap()
            })
            .collect(),
        table.support_records().to_vec(),
    )
    .unwrap()
}

#[test]
fn reference_source_field_inventory_rejects_missing_foreign_and_unbudgeted_storage() {
    with_hir_source(STANDALONE, |output, _| {
        let public = public_interface(output);
        let table = public.nominal_interfaces();
        let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        let class = table.records().iter().find(|record| matches!(record.source_shape(), hir::NominalSourceShapeV1::Class(fields) if !fields.fields().is_empty())).unwrap();
        let mut fields = class.source_shape().declared_fields().to_vec();
        let removed = fields.remove(0);
        let corrupt = replace_fields(table, class.declaration(), fields);
        assert!(
            matches!(corrupt.validate_declared_field_inventory(&foundation, &mut meter()),
            Err(hir::NominalSourceFieldInventoryError::Missing { field, .. }) if field == removed.field())
        );
        let other = table
            .records()
            .iter()
            .find(|record| {
                record.declaration() != class.declaration()
                    && !record.source_shape().declared_fields().is_empty()
            })
            .unwrap();
        let mut fields = class.source_shape().declared_fields().to_vec();
        fields.push(other.source_shape().declared_fields()[0].clone());
        let corrupt = replace_fields(table, class.declaration(), fields);
        assert!(matches!(
            corrupt.validate_declared_field_inventory(&foundation, &mut meter()),
            Err(hir::NominalSourceFieldInventoryError::Extra { .. })
        ));
        let mut meter = BudgetMeter::new(DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        });
        assert!(matches!(
            table.validate_declared_field_inventory(&foundation, &mut meter),
            Err(hir::NominalSourceFieldInventoryError::Resource(_))
        ));
    });
}
