use super::*;

#[test]
fn abstract_property_overrides_retain_typed_root_slots_and_logical_value_types() {
    with_source(ABSTRACT_OVERRIDES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let bound = sources.bind(&foundation).unwrap();
        let export = output.output().export.module();
        let mut rows = Vec::new();
        for (id, property) in export.properties.iter() {
            let hir::HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id]
            else {
                continue;
            };
            let Some(record) = sources.properties.get(identity.id()) else {
                continue;
            };
            let hir::PropertyOwner::Class(owner) = property.owner else {
                unreachable!()
            };
            let old = payload(record);
            rows.push(format!(
                "{}.{}: {:?}, slots={}\n",
                export.classes[owner].name,
                property.name,
                old.representation(),
                old.slot_relations().slots().len()
            ));
            if old.representation() != hir::PropertyRepresentationV1::AbstractSlot {
                continue;
            }
            let root = sources
                .properties
                .records()
                .iter()
                .find(|root| {
                    payload(root).representation() == hir::PropertyRepresentationV1::RuntimeAccessor
                        && bound.property_key(root.declaration()).unwrap().name()
                            == bound.property_key(record.declaration()).unwrap().name()
                })
                .unwrap();
            assert_eq!(old.slot_relations(), payload(root).slot_relations());
            let hir::SourceNominalId::Concrete(owner) = old.owner() else {
                unreachable!()
            };
            let mut forged = sources.clone();
            forged.replace(replace_payload(
                record,
                hir::NominalSourcePropertyPayloadV1::try_new(
                    old.owner(),
                    SignatureTypeKey::Nominal(owner),
                    old.getter(),
                    old.mutability().clone(),
                    old.representation(),
                    old.slot_relations().clone(),
                )
                .unwrap(),
            ));
            assert!(
                matches!(forged.bind(&foundation), Err(Error::Signature(id)) if id == record.declaration())
            );
            if old.slot_relations().slots().len() == 2 {
                let slots = hir::CanonicalProtectedSlotRefsV1::try_new(vec![
                    old.slot_relations().slots()[0],
                ])
                .unwrap();
                let mut forged = sources.clone();
                forged.replace(replace_payload(
                    record,
                    hir::NominalSourcePropertyPayloadV1::try_new(
                        old.owner(),
                        old.value_type().clone(),
                        old.getter(),
                        old.mutability().clone(),
                        old.representation(),
                        slots,
                    )
                    .unwrap(),
                ));
                assert!(
                    matches!(forged.bind(&foundation), Err(Error::Slots(id)) if id == record.declaration())
                );
            }
        }
        rows.sort();
        assert_eq!(
            rows.concat(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/property-abstract-overrides.contracts.snap"
            ))
        );
    });
}
