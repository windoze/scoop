use super::*;

mod origins;

#[test]
fn property_logical_type_must_match_its_dispatch_accessor() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = sources
            .properties
            .records()
            .iter()
            .find(|r| !payload(r).slot_relations().is_empty())
            .unwrap();
        let old = payload(record);
        let hir::SourceNominalId::Concrete(owner) = old.owner() else {
            panic!("param-free owner");
        };
        let forged = hir::NominalSourcePropertyPayloadV1::try_new(
            old.owner(),
            SignatureTypeKey::Nominal(owner),
            old.getter(),
            old.mutability().clone(),
            old.representation(),
            old.slot_relations().clone(),
        )
        .unwrap();
        let id = record.declaration();
        sources.replace(replace_payload(record, forged));
        assert!(matches!(sources.bind(&foundation), Err(Error::Signature(actual)) if actual == id));
    });
}

#[test]
fn private_setter_cannot_widen_a_protected_getter_domain() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = sources
            .properties
            .records()
            .iter()
            .find(|r| {
                matches!(payload(r).mutability(),
            hir::ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. }
            if setter_access.declared_visibility() == hir::DeclaredVisibilityV1::Private)
            })
            .unwrap();
        let old = payload(record);
        let hir::ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } = old.mutability()
        else {
            unreachable!()
        };
        let mutability = hir::ProtectedPropertyMutabilityV1::ReadWrite {
            setter: *setter,
            setter_access: hir::DeclarationAccessSourceV1::try_new(
                hir::DeclaredVisibilityV1::Public,
                setter_access.lexical_owners().to_vec(),
                setter_access.definition_origin().clone(),
            )
            .unwrap(),
        };
        let forged = hir::NominalSourcePropertyPayloadV1::try_new(
            old.owner(),
            old.value_type().clone(),
            old.getter(),
            mutability,
            old.representation(),
            old.slot_relations().clone(),
        )
        .unwrap();
        let id = record.declaration();
        sources.replace(replace_payload(record, forged));
        assert!(
            matches!(sources.bind(&foundation), Err(Error::SetterDomain(actual)) if actual == id)
        );
    });
}

#[test]
fn property_source_cannot_drop_a_required_setter_or_use_it_as_getter() {
    with_source(DIRECT, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = &sources.properties.records()[0];
        let old = payload(record);
        let hir::ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } = old.mutability() else {
            unreachable!()
        };
        for getter in [old.getter(), *setter] {
            let mut forged = sources.clone();
            forged.replace(replace_payload(
                record,
                hir::NominalSourcePropertyPayloadV1::try_new(
                    old.owner(),
                    old.value_type().clone(),
                    getter,
                    hir::ProtectedPropertyMutabilityV1::ReadOnly,
                    old.representation(),
                    old.slot_relations().clone(),
                )
                .unwrap(),
            ));
            assert!(matches!(forged.bind(&foundation), Err(Error::Accessor(_))));
        }
    });
}

#[test]
fn property_slot_relations_cannot_be_omitted_or_borrowed_from_another_property() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let with_slots = sources
            .properties
            .records()
            .iter()
            .find(|r| payload(r).slot_relations().slots().len() == 2)
            .unwrap();
        let without_slots = sources
            .properties
            .records()
            .iter()
            .find(|r| payload(r).slot_relations().is_empty())
            .unwrap();
        for (record, slots) in [(with_slots, without_slots), (without_slots, with_slots)] {
            let old = payload(record);
            let mut forged = sources.clone();
            forged.replace(replace_payload(
                record,
                hir::NominalSourcePropertyPayloadV1::try_new(
                    old.owner(),
                    old.value_type().clone(),
                    old.getter(),
                    old.mutability().clone(),
                    old.representation(),
                    payload(slots).slot_relations().clone(),
                )
                .unwrap(),
            ));
            assert!(
                matches!(forged.bind(&foundation), Err(Error::Slots(actual)) if actual == record.declaration())
            );
        }
    });
}
