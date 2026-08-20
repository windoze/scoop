use super::*;

#[test]
fn interface_contracts_round_trip_without_machine_callable_bindings() {
    let mut fixture = Fixture::new();
    fixture.callables = CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap();
    let records = (ROOT..=DIAMOND)
        .map(|owner| fixture.record(owner))
        .collect();
    let table = CanonicalMirDispatchSchemasV1::try_new(fixture.authority(), records).unwrap();
    for record in table.records() {
        assert_eq!(record.interface_slots().unwrap().len(), 2);
        assert!(record.vtable().is_empty());
        assert!(record.itables().is_empty());
        let references =
            MirTypeBridgeSemanticReferencesV1::of_dispatch(record, &fixture.graph, &fixture.types)
                .unwrap();
        assert!(
            references
                .targets()
                .iter()
                .all(|target| !matches!(target, MirTypeBridgeTargetV1::Callable(_)))
        );
    }
    let decoded: DecodedCanonicalMirDispatchSchemasV1 =
        decode_canonical(&encode(&table).unwrap()).unwrap();
    assert_eq!(
        decoded
            .validate(&mut fixture.graph, &fixture.types, &fixture.callables)
            .unwrap(),
        table,
    );
}

#[test]
fn actual_overrides_do_not_require_the_abstract_root_callable() {
    let mut fixture = Fixture::new();
    let root = CallableDefinitionOwner::Strong(fixture.target(3));
    fixture.callables = CanonicalMirCallableBindingsV1::try_new(
        fixture
            .callables
            .entries()
            .iter()
            .filter(|binding| binding.implementation() != root)
            .cloned()
            .collect(),
    )
    .unwrap();
    let table = fixture.table();
    let derived = table.get(fixture.exact(DERIVED)).unwrap();
    for itable in derived.itables() {
        let declaration = table.get(itable.interface()).unwrap();
        assert!(
            itable
                .entries()
                .iter()
                .map(MirDispatchEntryV1::contract)
                .eq(declaration.interface_slots().unwrap())
        );
    }
}

#[test]
fn interface_contracts_cannot_be_omitted_or_used_as_physical_tables() {
    let fixture = Fixture::new();
    let mut interface = fixture.record(ROOT);
    interface.slots = MirDispatchSlotsV1::NoClassVtable;
    assert!(matches!(
        fixture.check(&interface),
        Err(MirDispatchSchemaError::OwnerKind { .. })
    ));
    let mut interface = fixture.record(ROOT);
    interface.itables.push(MirInterfaceDispatchTableV1::new(
        fixture.exact(ROOT),
        fixture.interface_entries(ROOT, ROOT),
    ));
    assert!(matches!(
        fixture.check(&interface),
        Err(MirDispatchSchemaError::OwnerKind { .. })
    ));
    let mut derived = fixture.record(DERIVED);
    derived.slots = fixture.record(ROOT).slots;
    assert!(matches!(
        fixture.check(&derived),
        Err(MirDispatchSchemaError::OwnerKind { .. })
    ));
}
