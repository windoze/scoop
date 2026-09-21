use super::*;

#[test]
fn default_provider_slots_reject_other_roots_multiple_slots_and_final_overrides() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let root = key(output, "ProviderSlotBase.virtualValue", 0);
        let other = key(output, "ProviderSlotBase.otherValue", 0);
        let source = sources.members.callables.get(root.owner()).unwrap();
        let slot = source.payload().slot_relations().slots()[0];
        let other_slot = sources
            .members
            .callables
            .get(other.owner())
            .unwrap()
            .payload()
            .slot_relations()
            .slots()[0];
        for (case, modality, slots) in [
            (0, hir::CallableModalityV1::Open, vec![other_slot]),
            (1, hir::CallableModalityV1::Open, vec![slot, other_slot]),
            (2, hir::CallableModalityV1::Final, vec![slot]),
        ] {
            let p = source.payload();
            let record = hir::NominalSupportCallableInterfaceV1::try_new(
                source.declaration(),
                source.declaration_access().clone(),
                hir::NominalSourceCallablePayloadV1::try_new(
                    source.declaration(),
                    p.owner(),
                    p.type_parameters().clone(),
                    p.parameters().clone(),
                    p.result().clone(),
                    p.effects(),
                    modality,
                    hir::CanonicalProtectedSlotRefsV1::try_new(slots).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let mut changed = sources.clone();
            let callables = hir::CanonicalNominalSourceCallablesV1::try_new(
                sources
                    .members
                    .callables
                    .records()
                    .iter()
                    .map(|r| {
                        if r.declaration() == record.declaration() {
                            record.clone()
                        } else {
                            r.clone()
                        }
                    })
                    .collect(),
                &mut meter(),
            )
            .unwrap();
            let bytes = encode(&callables).unwrap();
            let decoded: hir::DecodedCanonicalNominalSourceCallablesV1 =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            changed.members.callables = decoded
                .resolve(&mut identity_closure(output), &mut meter())
                .unwrap();
            let foundation = fixture.bind().unwrap();
            changed.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &changed.protocols, &mut meter())
                    .unwrap();
                let error = parameters
                    .bind_default_declarations(&table, &[], &mut meter())
                    .unwrap_err();
                let Error::Record { key, error } = error else {
                    panic!("record error")
                };
                assert_eq!(key, root);
                let Error::ProviderSlot(error) = *error else {
                    panic!("provider slot error")
                };
                match (case, *error) {
                    (0, hir::DefaultSourceProviderSlotError::RootSlot(actual)) => {
                        assert_eq!(actual, other_slot)
                    }
                    (
                        1,
                        hir::DefaultSourceProviderSlotError::SlotCount {
                            expected: 1,
                            actual: 2,
                        },
                    )
                    | (
                        2,
                        hir::DefaultSourceProviderSlotError::SlotCount {
                            expected: 0,
                            actual: 1,
                        },
                    ) => {}
                    (_, error) => panic!("unexpected slot error {error:?}"),
                }
            });
        }
    });
}
