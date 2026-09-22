use super::*;

#[test]
fn dispatch_join_rejects_constructor_and_protected_inventory_omissions() {
    with_sources(SOURCE, |fixture, sources, dispatch, core| {
        let foundation = fixture.bind().unwrap();
        for field in ["constructors", "protected members"] {
            let index = dispatch
                .inventory
                .records()
                .iter()
                .position(|r| {
                    if field == "constructors" {
                        !r.constructors().is_empty()
                    } else {
                        r.protected_members()
                            .values()
                            .iter()
                            .any(|r| matches!(r, hir::ProtectedDeclarationRefV1::NestedNominal(_)))
                    }
                })
                .unwrap();
            let row = &dispatch.inventory.records()[index];
            let constructors = if field == "constructors" {
                hir::CanonicalPersistentIdsV1::empty()
            } else {
                row.constructors().clone()
            };
            let protected = hir::CanonicalProtectedDeclarationRefsV1::try_new(
                row.protected_members()
                    .values()
                    .iter()
                    .copied()
                    .filter(|r| {
                        field != "protected members"
                            || !matches!(r, hir::ProtectedDeclarationRefV1::NestedNominal(_))
                    })
                    .collect(),
            )
            .unwrap();
            let mut records = dispatch.inventory.records().to_vec();
            records[index] = hir::SourceInheritanceInventoryV1::try_new(
                row.owner(),
                constructors,
                protected,
                row.slot_schemas().clone(),
                &mut meter(),
            )
            .unwrap();
            let mut forged = dispatch.clone();
            forged.inventory =
                hir::CanonicalSourceInheritanceInventoriesV1::try_new(records, &mut meter())
                    .unwrap();
            let bound = forged.bind(&foundation, &mut meter()).unwrap();
            let slots = bound.bind_slot_sources(&mut meter()).unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
                assert!(matches!(parameters.bind_dispatch_sources(&slots, &mut meter()),
                    Err(Error::Inventory { owner, field: actual }) if owner == row.owner() && actual == field));
            });
        }
    });
}

#[test]
fn dispatch_join_rejects_another_binding_of_the_same_artifact() {
    with_sources(SOURCE, |fixture, sources, dispatch, core| {
        let foundation = fixture.bind().unwrap();
        let another = fixture.bind().unwrap();
        let bound = dispatch.bind(&another, &mut meter()).unwrap();
        let slots = bound.bind_slot_sources(&mut meter()).unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            assert!(matches!(
                parameters.bind_dispatch_sources(&slots, &mut meter()),
                Err(Error::FoundationMismatch)
            ));
        });
    });
}
