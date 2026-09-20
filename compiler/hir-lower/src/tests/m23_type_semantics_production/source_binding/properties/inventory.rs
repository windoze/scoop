use super::*;

#[test]
fn protected_property_inventory_cannot_move_a_member_to_another_owner() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let mut records = sources.dispatch.inventory.records().to_vec();
        let (index, member) = records
            .iter()
            .enumerate()
            .find_map(|(index, source)| {
                source
                    .protected_members()
                    .values()
                    .iter()
                    .find_map(|member| {
                        matches!(member, hir::ProtectedDeclarationRefV1::Property(_))
                            .then_some((index, *member))
                    })
            })
            .unwrap();
        let other = (index + 1) % records.len();
        for position in [index, other] {
            let source = &records[position];
            let mut members = source.protected_members().values().to_vec();
            if position == index {
                members.retain(|value| *value != member);
            } else {
                members.push(member);
            }
            records[position] = hir::SourceInheritanceInventoryV1::try_new(
                source.owner(),
                source.constructors().clone(),
                hir::CanonicalProtectedDeclarationRefsV1::try_new(members).unwrap(),
                source.slot_schemas().clone(),
                &mut meter(),
            )
            .unwrap();
        }
        sources.dispatch.inventory =
            hir::CanonicalSourceInheritanceInventoriesV1::try_new(records, &mut meter()).unwrap();
        let hir::ProtectedDeclarationRefV1::Property(expected) = member else {
            unreachable!()
        };
        assert!(
            matches!(sources.bind(&foundation, &mut meter()), Err(Error::Owner(actual)) if actual == expected)
        );
    });
}

#[test]
fn property_sources_require_exact_inventory_without_missing_or_unused_records() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let export = output.output().export.module();
        let (unused, _) = export
            .properties
            .iter()
            .find(|(_, r)| r.name == "unrelated")
            .unwrap();
        let hir::HirPropertyIdentity::Ordinary(identity) = &export.property_identities[unused]
        else {
            unreachable!()
        };
        for extra in [false, true] {
            let mut forged = sources.clone();
            let mut records = sources.properties.records().to_vec();
            if extra {
                let first = &records[0];
                records.push(
                    Record::try_new(
                        identity.id(),
                        first.declaration_access().clone(),
                        first.payload().clone(),
                    )
                    .unwrap(),
                );
            } else {
                records.pop().unwrap();
            }
            forged.properties =
                hir::CanonicalInheritanceSourcePropertiesV1::try_new(records, &mut meter())
                    .unwrap();
            assert!(matches!(
                (extra, forged.bind(&foundation, &mut meter())),
                (true, Err(Error::Inventory)) | (false, Err(Error::MissingSource(_)))
            ));
        }
    });
}
