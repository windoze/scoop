use super::*;

#[test]
fn independent_representation_source_join_covers_all_shapes_and_borrows_the_table() {
    let fixture = fixtures::mixed();
    let table = fixture.table();
    let checked = table
        .validate_source_semantics(&fixture, &mut meter(), &path())
        .unwrap();
    assert_eq!(table.records().len(), 6);
    assert!(std::ptr::eq(checked.table(), &table));
    for record in table.records() {
        assert!(std::ptr::eq(checked.get(record.owner()).unwrap(), record));
    }
    assert_eq!(*fixture.calls.borrow(), fixture.required.values());
    let empty = Fixture::new();
    empty
        .table()
        .validate_source_semantics(&empty, &mut meter(), &path())
        .unwrap();
}

#[test]
fn independent_inventory_requires_exact_local_coverage_and_real_sources() {
    let fixture = fixtures::mixed();
    let table = fixture.table();
    let owner = table.records()[0].owner();
    let missing =
        CanonicalNominalRepresentationSupportV1::try_new(table.records()[1..].to_vec()).unwrap();
    assert!(
        matches!(missing.validate_source_semantics(&fixture, &mut meter(), &path()), Err(NominalRepresentationSourceSemanticError::Missing { owner: actual }) if actual == owner)
    );
    let mut less = fixture.clone();
    less.required =
        CanonicalPersistentIdsV1::try_new(fixture.required.values()[1..].to_vec()).unwrap();
    assert!(
        matches!(table.validate_source_semantics(&less, &mut meter(), &path()), Err(NominalRepresentationSourceSemanticError::Extra { index: 0, owner: actual }) if actual == owner)
    );
    let mut absent = fixture.clone();
    absent.sources.remove(&owner);
    assert!(matches!(
        table.validate_source_semantics(&absent, &mut meter(), &path()),
        Err(NominalRepresentationSourceSemanticError::Source {
            index: 0,
            error: "independent source unavailable",
            ..
        })
    ));
    let mut unavailable = fixture.clone();
    unavailable.inventory_failure = true;
    assert!(matches!(
        table.validate_source_semantics(&unavailable, &mut meter(), &path()),
        Err(NominalRepresentationSourceSemanticError::Inventory(
            "independent inventory unavailable"
        ))
    ));
    let mut foreign = fixture;
    foreign.provider = ConeCoordinate::new("example", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    mismatch(
        &table,
        &foreign,
        NominalRepresentationSourceMismatchV1::Provider,
    );
}

#[test]
fn nonpublic_lexical_support_does_not_gain_a_public_value_requirement() {
    let mut source = SourceFixture::new(SourceNominalKind::Struct);
    let outer = PersistentTypeId::from_source_declaration(&key(
        "PrivateOuter",
        SourceNominalKind::Class,
        0,
        vec![],
    ))
    .unwrap();
    source.key = key(
        "Nested",
        SourceNominalKind::Struct,
        0,
        vec![DefinitionOwnerAtom::Type(outer)],
    );
    source.access = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Public,
        vec![SourceNominalId::Concrete(outer)],
        source.access.definition_origin().clone(),
    )
    .unwrap();
    let field = source.struct_field("data", unit());
    let mut fixture = Fixture::new();
    fixture.add(
        &source,
        NominalRepresentationShapeV1::Struct {
            fields: vec![field],
            c_layout_policy: NominalCLayoutPolicyV1::Ordinary,
        },
        None,
    );
    fixture
        .table()
        .validate_source_semantics(&fixture, &mut meter(), &path())
        .unwrap();
}
