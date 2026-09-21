use super::*;

#[test]
fn default_source_struct_fields_resolve_from_bound_declaration_order_and_owner() {
    with_sources(SOURCE, |_, fixture, sources, _| {
        let foundation = fixture.bind().unwrap();
        let bound = foundation
            .bind_nominal_sources(&sources.members.nominals, &mut meter())
            .unwrap();
        let structs: Vec<_> = sources
            .members
            .nominals
            .records()
            .iter()
            .filter_map(|record| match record.source_shape() {
                hir::NominalSourceShapeV1::Struct(shape) => Some((record.owner(), shape)),
                _ => None,
            })
            .collect();
        assert_eq!(structs.len(), 2);
        for (owner, shape) in &structs {
            for (index, field) in shape.fields().iter().enumerate() {
                assert_eq!(
                    bound
                        .struct_field_index(*owner, field.field(), &mut meter(), &WirePath::root())
                        .unwrap(),
                    index as u32
                );
                let foreign = structs.iter().find(|(other, _)| other != owner).unwrap().0;
                assert!(
                    matches!(bound.struct_field_index(foreign, field.field(), &mut meter(), &WirePath::root()),
                    Err(hir::NominalSourceBindingError::FieldOwner { owner: actual, field: actual_field }) if actual == foreign && actual_field == field.field())
                );
            }
        }
        let (owner, shape) = structs[0];
        assert!(matches!(
            bound.struct_field_index(
                owner,
                shape.fields()[0].field(),
                &mut BudgetMeter::new(DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                }),
                &WirePath::root()
            ),
            Err(hir::NominalSourceBindingError::Resource(_))
        ));
    });
}
