use super::*;

fn with_constructors(
    owner: &hir::SourceInheritanceInventoryV1,
    constructors: Vec<PersistentConstructorId>,
) -> hir::SourceInheritanceInventoryV1 {
    hir::SourceInheritanceInventoryV1::try_new(
        owner.owner(),
        hir::CanonicalPersistentIdsV1::try_new(constructors).unwrap(),
        owner.protected_members().clone(),
        owner.slot_schemas().clone(),
        &mut meter(),
    )
    .unwrap()
}

#[test]
fn constructor_binding_requires_exact_owner_and_constructor_inventories() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        for expected in ["inheritance owners", "constructor sources"] {
            let mut incomplete = sources.clone();
            if expected == "inheritance owners" {
                let mut records = incomplete.inventory.records().to_vec();
                records.pop().unwrap();
                incomplete.inventory =
                    hir::CanonicalSourceInheritanceInventoriesV1::try_new(records, &mut meter())
                        .unwrap();
            } else {
                let mut records = incomplete.constructors.records().to_vec();
                records.pop().unwrap();
                incomplete.constructors =
                    hir::CanonicalInheritanceSourceConstructorsV1::try_new(records, &mut meter())
                        .unwrap();
            }
            assert!(
                matches!(incomplete.bind(&foundation, &mut meter()), Err(Error::Inventory(actual)) if actual == expected)
            );
        }
    });
}

#[test]
fn unused_real_constructor_is_not_accepted_as_inheritance_source() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let export = output.output().export.module();
        let unused = export
            .class_constructors
            .iter()
            .find_map(|(id, _)| {
                let record = export.constructor_identities[id].source_record()?;
                sources
                    .constructors
                    .get(record.id())
                    .is_none()
                    .then_some(record.id())
            })
            .unwrap();
        let first = &sources.constructors.records()[0];
        let payload = first.payload();
        let extra = hir::NominalSupportConstructorInterfaceV1::try_new(
            unused,
            first.declaration_access().clone(),
            hir::NominalSourceCallablePayloadV1::try_new(
                CallableTemplateOrigin::Constructor(unused),
                payload.owner(),
                payload.type_parameters().clone(),
                payload.parameters().clone(),
                payload.result().clone(),
                payload.effects(),
                payload.modality(),
                payload.slot_relations().clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut records = sources.constructors.records().to_vec();
        records.push(extra);
        sources.constructors =
            hir::CanonicalInheritanceSourceConstructorsV1::try_new(records, &mut meter()).unwrap();
        assert!(matches!(
            sources.bind(&foundation, &mut meter()),
            Err(Error::Inventory("constructor sources"))
        ));
    });
}

#[test]
fn constructor_inventory_cannot_duplicate_or_move_a_constructor_between_owners() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let owner = sources
            .inventory
            .records()
            .iter()
            .position(|owner| !owner.constructors().values().is_empty())
            .unwrap();
        let other = (owner + 1) % sources.inventory.records().len();
        let declaration = sources.inventory.records()[owner].constructors().values()[0];
        for duplicate in [true, false] {
            let mut forged = sources.clone();
            let mut records = forged.inventory.records().to_vec();
            let mut values = records[other].constructors().values().to_vec();
            values.push(declaration);
            records[other] = with_constructors(&records[other], values);
            if !duplicate {
                let values = records[owner]
                    .constructors()
                    .values()
                    .iter()
                    .copied()
                    .filter(|id| *id != declaration)
                    .collect();
                records[owner] = with_constructors(&records[owner], values);
            }
            forged.inventory =
                hir::CanonicalSourceInheritanceInventoriesV1::try_new(records, &mut meter())
                    .unwrap();
            assert!(
                matches!((duplicate, forged.bind(&foundation, &mut meter()).unwrap_err()),
                (true, Error::RepeatedOwner(actual)) | (false, Error::Owner(actual)) if actual == declaration)
            );
        }
    });
}
