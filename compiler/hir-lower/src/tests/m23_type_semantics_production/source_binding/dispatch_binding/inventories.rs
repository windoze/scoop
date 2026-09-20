use super::*;

#[test]
fn dispatch_source_binding_requires_all_four_inventories() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        for expected in [
            "inheritance owners",
            "interface owners",
            "slot selections",
            "dispatch callables",
        ] {
            let mut incomplete = sources.clone();
            match expected {
                "inheritance owners" => {
                    incomplete.inventory =
                        hir::CanonicalSourceInheritanceInventoriesV1::try_new(vec![], &mut meter())
                            .unwrap()
                }
                "interface owners" => {
                    incomplete.interfaces = hir::CanonicalInterfaceSourceDispatchesV1::default()
                }
                "slot selections" => {
                    incomplete.selections =
                        hir::CanonicalInheritanceSourceSlotSelectionsV1::default()
                }
                "dispatch callables" => {
                    incomplete.callables = hir::CanonicalInheritanceSourceCallablesV1::default()
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(incomplete.bind(&foundation, &mut meter()), Err(Error::Inventory(actual)) if actual == expected)
            );
        }
    });
}

#[test]
fn unused_callable_sources_are_not_silently_accepted() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let export = output.output().export.module();
        let unused =
            export
                .functions
                .iter()
                .find_map(|(id, _)| {
                    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(
                        record,
                    )) = &export.function_identities[id]
                    else {
                        return None;
                    };
                    let declaration = hir::InheritanceCallableDeclarationV1::Function(record.id());
                    sources
                        .callables
                        .get(declaration)
                        .is_none()
                        .then_some(declaration)
                })
                .unwrap();
        let mut records = sources.callables.records().to_vec();
        let record = &records[0];
        records.push(hir::InheritanceSourceCallableV1::new(
            unused,
            record.signature().clone(),
            record.modality(),
            record.declaration_access().clone(),
        ));
        sources.callables =
            hir::CanonicalInheritanceSourceCallablesV1::try_new(records, &mut meter()).unwrap();
        assert!(matches!(
            sources.bind(&foundation, &mut meter()),
            Err(Error::Inventory("dispatch callables"))
        ));
    });
}

#[test]
fn identity_binding_does_not_replace_independent_slot_order_validation() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let owner = sources
            .interfaces
            .records()
            .iter()
            .find_map(|interface| {
                let owner = sources.inventory.get(interface.owner()).unwrap();
                (owner.slot_schemas().records()[0].slots().len() > 1).then_some(owner.owner())
            })
            .unwrap();
        let mut records = sources.inventory.records().to_vec();
        let record = records
            .iter_mut()
            .find(|record| record.owner() == owner)
            .unwrap();
        let schema = &record.slot_schemas().records()[0];
        let mut reversed = schema.slots().to_vec();
        reversed.reverse();
        *record = hir::SourceInheritanceInventoryV1::try_new(
            owner,
            record.constructors().clone(),
            record.protected_members().clone(),
            hir::CanonicalInheritanceSlotSchemasV1::try_new(vec![
                hir::InheritanceSlotSchemaV1::try_new(schema.role(), reversed.clone()).unwrap(),
            ])
            .unwrap(),
            &mut meter(),
        )
        .unwrap();
        sources.inventory =
            hir::CanonicalSourceInheritanceInventoriesV1::try_new(records, &mut meter()).unwrap();
        let bound = sources.bind(&foundation, &mut meter()).unwrap();
        assert_eq!(bound.schemas(owner).unwrap().records()[0].slots(), reversed);
        let entries = fixture.source.entries();
        let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            &foundation,
            &mut meter(),
        )
        .unwrap();
        assert!(matches!(
            graph.validate_slot_schemas(owner, &bound, &mut meter()),
            Err(hir::InheritanceSlotSchemaSemanticError::InheritedSlots(actual)) if actual == owner
        ));
    });
}

#[test]
fn extra_selection_for_a_real_owner_and_slot_is_rejected() {
    with_source(INTERFACES, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let mut sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let (owner, slot) = sources
            .inventory
            .owners()
            .values()
            .iter()
            .find_map(|owner| {
                sources.selections.records().iter().find_map(|record| {
                    sources
                        .selections
                        .get(*owner, record.slot())
                        .is_none()
                        .then_some((*owner, record.slot()))
                })
            })
            .unwrap();
        let mut records = sources.selections.records().to_vec();
        records.push(hir::InheritanceSourceSlotSelectionRecordV1::new(
            owner,
            slot,
            hir::InheritanceSourceSlotSelectionV1::Abstract,
        ));
        sources.selections =
            hir::CanonicalInheritanceSourceSlotSelectionsV1::try_new(records, &mut meter())
                .unwrap();
        assert!(matches!(
            sources.bind(&foundation, &mut meter()),
            Err(Error::Inventory("slot selections"))
        ));
    });
}
