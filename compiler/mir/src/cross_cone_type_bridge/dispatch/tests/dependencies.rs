use super::*;

struct Tables {
    local_types: CanonicalParamFreeMirTypeExportsV1,
    dependency_types: CanonicalParamFreeMirTypeExportsV1,
    local_callables: CanonicalMirCallableBindingsV1,
    dependency_callables: CanonicalMirCallableBindingsV1,
    dependency_schemas: CanonicalMirDispatchSchemasV1,
}
impl Tables {
    fn new(fixture: &Fixture) -> Self {
        let types = |owners: &[usize]| {
            CanonicalParamFreeMirTypeExportsV1::try_new(
                owners
                    .iter()
                    .map(|owner| fixture.types.get(fixture.exact(*owner)).unwrap().clone())
                    .collect(),
            )
            .unwrap()
        };
        let callables = |methods: &[usize]| {
            CanonicalMirCallableBindingsV1::try_new(
                methods
                    .iter()
                    .map(|index| {
                        fixture
                            .callables
                            .get(fixture.target(*index))
                            .unwrap()
                            .clone()
                    })
                    .collect(),
            )
            .unwrap()
        };
        Self {
            local_types: types(&[DERIVED]),
            dependency_types: types(&[UNIT, BASE, ROOT, LEFT, RIGHT, DIAMOND]),
            local_callables: callables(&[1, 5]),
            dependency_callables: callables(&[0, 3, 4]),
            dependency_schemas: CanonicalMirDispatchSchemasV1::try_new(
                fixture.authority(),
                [BASE, ROOT, LEFT, RIGHT, DIAMOND]
                    .into_iter()
                    .map(|owner| fixture.record(owner))
                    .collect(),
            )
            .unwrap(),
        }
    }
    fn types(&self) -> MirTypeBridgeTypeIndexV1<'_> {
        MirTypeBridgeTypeIndexV1::try_new(&[&self.local_types, &self.dependency_types]).unwrap()
    }
    fn callables(&self) -> MirTypeBridgeCallableIndexV1<'_> {
        MirTypeBridgeCallableIndexV1::try_new(
            &[&self.local_callables, &self.dependency_callables],
            &[],
        )
        .unwrap()
    }
}

#[test]
fn foreign_base_and_interface_schemas_are_borrowed_without_copying_exports() {
    let mut fixture = Fixture::with_dependency_cone(true);
    let tables = Tables::new(&fixture);
    let (types, callables) = (tables.types(), tables.callables());
    let local = CanonicalMirDispatchSchemasV1::try_new_with_dependencies(
        MirDispatchSchemaAuthority {
            identities: &fixture.graph,
            types: &types,
            callables: &callables,
        },
        vec![fixture.record(DERIVED)],
        &[&tables.dependency_schemas],
    )
    .unwrap();
    assert_eq!(fixture.source[BASE].key().origin(), ConeIdentity::CORE);
    assert_eq!(
        fixture.source[DERIVED].key().origin(),
        ConeIdentity::SINGLE_FILE
    );
    assert_eq!(local.records().len(), 1);
    assert!(local.get(fixture.exact(BASE)).is_none());
    assert!(tables.local_types.get(fixture.exact(BASE)).is_none());
    assert!(tables.local_callables.get(fixture.target(0)).is_none());
    let schema_view =
        MirTypeBridgeSchemaIndexV1::try_new(&[&local, &tables.dependency_schemas]).unwrap();
    assert!(std::ptr::eq(
        schema_view.get(fixture.exact(BASE)).unwrap(),
        tables.dependency_schemas.get(fixture.exact(BASE)).unwrap()
    ));
    let decoded: DecodedCanonicalMirDispatchSchemasV1 =
        decode_canonical(&encode(&local).unwrap()).unwrap();
    let round_trip = decoded
        .validate_with_dependencies(
            &mut fixture.graph,
            &types,
            &callables,
            &[&tables.dependency_schemas],
        )
        .unwrap();
    assert_eq!(round_trip, local);
    assert_eq!(round_trip.records().len(), 1);
}

#[test]
fn local_callable_reader_uses_imported_exact_types_but_keeps_its_own_table() {
    let mut fixture = Fixture::with_dependency_cone(true);
    let tables = Tables::new(&fixture);
    let types = tables.types();
    let decoded: DecodedCanonicalMirCallableBindingsV1 =
        decode_canonical(&encode(&tables.local_callables).unwrap()).unwrap();
    let table = decoded
        .validate(&mut fixture.graph, &fixture.foundation, &types)
        .unwrap();
    assert_eq!(table, tables.local_callables);
    assert_eq!(table.entries().len(), 2);
    assert!(table.get(fixture.target(0)).is_none());
}

#[test]
fn missing_dependency_schema_and_missing_dependency_types_fail_closed() {
    let fixture = Fixture::with_dependency_cone(true);
    let tables = Tables::new(&fixture);
    let (types, callables) = (tables.types(), tables.callables());
    let record = fixture.record(DERIVED);
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new(
            MirDispatchSchemaAuthority {
                identities: &fixture.graph,
                types: &types,
                callables: &callables
            },
            vec![record.clone()],
        ),
        Err(MirDispatchSchemaError::MissingSchema { .. })
    ));
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new_with_dependencies(
            MirDispatchSchemaAuthority {
                identities: &fixture.graph,
                types: &tables.local_types,
                callables: &callables
            },
            vec![record],
            &[&tables.dependency_schemas],
        ),
        Err(MirDispatchSchemaError::MissingType { .. })
    ));
}

#[test]
fn imported_base_still_requires_the_full_local_vtable_prefix() {
    let fixture = Fixture::with_dependency_cone(true);
    let tables = Tables::new(&fixture);
    let (types, callables) = (tables.types(), tables.callables());
    let mut record = fixture.record(DERIVED);
    record.vtable = MirClassVtableSchemaV1::ClassVtable(vec![]);
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new_with_dependencies(
            MirDispatchSchemaAuthority {
                identities: &fixture.graph,
                types: &types,
                callables: &callables
            },
            vec![record],
            &[&tables.dependency_schemas],
        ),
        Err(MirDispatchSchemaError::BasePrefix { .. })
    ));
}

#[test]
fn repeated_dependency_authority_is_not_silently_merged() {
    let fixture = Fixture::with_dependency_cone(true);
    let tables = Tables::new(&fixture);
    let (types, callables) = (tables.types(), tables.callables());
    assert!(matches!(
        MirTypeBridgeCallableIndexV1::try_new(
            &[&tables.dependency_callables, &tables.dependency_callables],
            &[]
        ),
        Err(MirTypeBridgeLookupError::DuplicateCallable { .. })
    ));
    assert!(matches!(
        CanonicalMirDispatchSchemasV1::try_new_with_dependencies(
            MirDispatchSchemaAuthority {
                identities: &fixture.graph,
                types: &types,
                callables: &callables
            },
            vec![fixture.record(DERIVED)],
            &[&tables.dependency_schemas, &tables.dependency_schemas],
        ),
        Err(MirDispatchSchemaError::Lookup(
            MirTypeBridgeLookupError::DuplicateSchema { .. }
        ))
    ));
}
