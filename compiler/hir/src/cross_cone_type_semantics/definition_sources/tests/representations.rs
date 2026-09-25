use super::*;

pub(super) fn fixture(count: usize) -> (Fixture, Vec<SourceDeclarationKey>) {
    let foreign = ConeCoordinate::new("example", "foreign-origins", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let mut fixture = Fixture::default();
    let mut records = Vec::new();
    let mut keys = Vec::new();
    for index in 0..count {
        let cone = if index % 2 == 0 {
            ConeIdentity::CORE
        } else {
            foreign
        };
        let key = nominal_key(
            cone,
            &[],
            &format!("Source{index}"),
            SourceNominalKind::Struct,
            0,
        );
        let source = origin(cone, (index % 2) as u64);
        let record = NominalRepresentationSupportV1::try_new(
            &key,
            access(vec![], DeclaredVisibilityV1::Public, &source),
            NominalRepresentationShapeV1::Struct {
                fields: vec![],
                c_layout_policy: NominalCLayoutPolicyV1::Ordinary,
            },
        )
        .unwrap();
        fixture.expect(UseKey::Representation(record.owner()), &source);
        records.push(record);
        keys.push(key);
    }
    fixture.representations = CanonicalNominalRepresentationSupportV1::try_new(records).unwrap();
    (fixture, keys)
}
#[test]
fn mixed_local_and_foreign_inline_origins_are_preserved_and_exact() {
    let (fixture, _) = fixture(2);
    let declared = fixture.declared();
    assert_eq!(declared.sources().len(), 2);
    assert_eq!(fixture.validate(&declared).unwrap(), 2);
    let missing =
        CanonicalExportDefinitionSourcesV1::try_new(vec![declared.sources()[0].clone()]).unwrap();
    assert!(matches!(
        fixture.validate(&missing),
        Err(TypeDefinitionSourceClosureError::Missing { .. })
    ));
    let mut values = declared.sources().to_vec();
    values.push(origin(ConeIdentity::CORE, 99));
    let extra = CanonicalExportDefinitionSourcesV1::try_new(values).unwrap();
    assert!(matches!(
        fixture.validate(&extra),
        Err(TypeDefinitionSourceClosureError::Extra { .. })
    ));
}
#[test]
fn full_origin_span_and_context_are_part_of_the_exact_source_set() {
    let (fixture, keys) = fixture(1);
    let original = &fixture.expected[0].origin;
    let source = original.origin().source().clone();
    let altered_context = ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(
            source.clone(),
            original.origin().span(),
            &SourceContextKey::Nominal {
                source,
                owner: NominalDeclarationOwner::from_source_declaration(&keys[0]).unwrap(),
            },
        )
        .unwrap(),
    );
    for altered in [origin(ConeIdentity::CORE, 99), altered_context] {
        assert_eq!(altered.origin().source(), original.origin().source());
        assert_ne!(&altered, original);
        let declared = CanonicalExportDefinitionSourcesV1::try_new(vec![altered]).unwrap();
        assert!(matches!(
            fixture.validate(&declared),
            Err(TypeDefinitionSourceClosureError::Missing { .. })
        ));
    }
}
#[test]
fn rewriting_foreign_inline_origin_and_field7_together_cannot_replace_provider_truth() {
    let (mut fixture, keys) = fixture(2);
    let foreign_key = &keys[1];
    let foreign_id = PersistentTypeId::from_source_declaration(foreign_key).unwrap();
    let forged = origin(ConeIdentity::CORE, 1);
    let mut records = fixture.representations.records().to_vec();
    let record = records
        .iter_mut()
        .find(|record| record.owner() == foreign_id)
        .unwrap();
    *record = NominalRepresentationSupportV1::try_new(
        foreign_key,
        access(vec![], DeclaredVisibilityV1::Public, &forged),
        record.shape().clone(),
    )
    .unwrap();
    fixture.representations = CanonicalNominalRepresentationSupportV1::try_new(records).unwrap();
    let declared = CanonicalExportDefinitionSourcesV1::try_new(vec![
        fixture.expected[0].origin.clone(),
        forged,
    ])
    .unwrap();
    assert!(matches!(
        fixture.validate(&declared),
        Err(TypeDefinitionSourceClosureError::Source {
            error: "origin disagrees with independent typed provider use",
            ..
        })
    ));
}
