use super::*;

#[test]
fn property_and_setter_origins_must_match_their_exact_foundation_subjects() {
    with_source(DIRECT, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = &sources.properties.records()[0];
        let access = record.declaration_access();
        let old = payload(record);
        let hir::ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } = old.mutability()
        else {
            unreachable!()
        };
        for setter_origin in [false, true] {
            let mut forged = sources.clone();
            let changed = if setter_origin {
                let mutability = hir::ProtectedPropertyMutabilityV1::ReadWrite {
                    setter: *setter,
                    setter_access: hir::DeclarationAccessSourceV1::try_new(
                        setter_access.declared_visibility(),
                        setter_access.lexical_owners().to_vec(),
                        access.definition_origin().clone(),
                    )
                    .unwrap(),
                };
                replace_payload(
                    record,
                    hir::NominalSourcePropertyPayloadV1::try_new(
                        old.owner(),
                        old.value_type().clone(),
                        old.getter(),
                        mutability,
                        old.representation(),
                        old.slot_relations().clone(),
                    )
                    .unwrap(),
                )
            } else {
                Record::try_new(
                    record.declaration(),
                    hir::DeclarationAccessSourceV1::try_new(
                        access.declared_visibility(),
                        access.lexical_owners().to_vec(),
                        setter_access.definition_origin().clone(),
                    )
                    .unwrap(),
                    record.payload().clone(),
                )
                .unwrap()
            };
            forged.replace(changed);
            let subject = if setter_origin {
                DefinitionOriginSubject::PropertyAccessor(*setter)
            } else {
                DefinitionOriginSubject::Property(record.declaration())
            };
            assert!(
                matches!(forged.bind(&foundation), Err(Error::DefinitionOrigin(actual)) if actual == subject)
            );
        }
    });
}

#[test]
fn property_access_must_preserve_the_full_lexical_owner_chain() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let record = sources
            .properties
            .records()
            .iter()
            .find(|r| {
                matches!(
                    payload(r).mutability(),
                    hir::ProtectedPropertyMutabilityV1::ReadOnly
                )
            })
            .unwrap();
        let access = record.declaration_access();
        let other = fixture
            .source
            .entries()
            .sources
            .records()
            .iter()
            .find(|r| r.owner() != record.owner())
            .unwrap()
            .owner();
        let mut owners = access.lexical_owners().to_vec();
        owners.insert(0, other);
        let id = record.declaration();
        sources.replace(
            Record::try_new(
                id,
                hir::DeclarationAccessSourceV1::try_new(
                    access.declared_visibility(),
                    owners,
                    access.definition_origin().clone(),
                )
                .unwrap(),
                record.payload().clone(),
            )
            .unwrap(),
        );
        assert!(
            matches!(sources.bind(&foundation), Err(Error::Access { declaration, .. }) if declaration == id)
        );
    });
}
