use super::*;

#[test]
fn source_origins_require_foundation_context_file_points_and_typed_subject() {
    let fixture = Fixture::from_output(&lower_public_nominals());
    for field in [1, 2, 3] {
        let mut canonical = fixture.foundation.as_canonical().clone();
        match field {
            1 => canonical.set_source_contexts(vec![]).unwrap(),
            2 => canonical.set_sources(vec![]).unwrap(),
            3 => canonical.set_definition_origins(vec![]).unwrap(),
            _ => unreachable!(),
        }
        let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
        let error = fixture
            .source
            .bind_to_foundation(&incomplete, &fixture.identities)
            .unwrap_err();
        assert!(matches!(
            (field, error),
            (1, hir::TypeFoundationBindingError::MissingSourceContext(_))
                | (2, hir::TypeFoundationBindingError::MissingSourceRecord)
                | (3, hir::TypeFoundationBindingError::DeclarationOrigin(_))
        ));
    }
    let mut entries = fixture.source.clone().into_entries();
    let origin = entries.definition_sources.sources()[0].origin();
    let context = fixture
        .foundation
        .source_context_key(origin.context())
        .unwrap();
    let new_origin = scoop_identity::DefinitionOrigin::new(
        origin.source().clone(),
        scoop_identity::SourceSpan::new(0, 1).unwrap(),
        context,
    )
    .unwrap();
    let mut sources = entries.definition_sources.sources().to_vec();
    sources.push(hir::ExportDefinitionSourceV1::new(new_origin));
    entries.definition_sources = hir::CanonicalExportDefinitionSourcesV1::try_new(sources).unwrap();
    let source = hir::TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    assert!(matches!(
        source.bind_to_foundation(&fixture.foundation, &fixture.identities),
        Err(hir::TypeFoundationBindingError::MissingSourcePoint(1))
    ));
}

#[test]
fn a_source_snapshot_cannot_choose_a_foreign_provider_or_fake_lexical_owner() {
    let fixture = Fixture::from_output(&lower_public_nominals());
    let mut entries = fixture.source.clone().into_entries();
    entries.provider = ConeIdentity::CORE;
    let source = hir::TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    assert!(matches!(
        source.bind_to_foundation(&fixture.foundation, &fixture.identities),
        Err(hir::TypeFoundationBindingError::ForeignNominal(_))
    ));

    let mut entries = fixture.source.clone().into_entries();
    let owner = entries
        .sources
        .records()
        .iter()
        .find(|record| matches!(record.owner(), hir::SourceNominalId::Concrete(_)))
        .unwrap()
        .owner();
    entries.sources = hir::CanonicalTypeSourceNominalsV1::try_new(
        entries
            .sources
            .records()
            .iter()
            .map(|record| {
                if matches!(record.owner(), hir::SourceNominalId::Concrete(_)) {
                    return record.clone();
                }
                hir::TypeSourceNominalV1::new(
                    record.owner(),
                    hir::DeclarationAccessSourceV1::try_new(
                        record.access().declared_visibility(),
                        vec![owner],
                        record.access().definition_origin().clone(),
                    )
                    .unwrap(),
                )
            })
            .collect(),
    )
    .unwrap();
    let source = hir::TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    assert!(matches!(
        source.bind_to_foundation(&fixture.foundation, &fixture.identities),
        Err(hir::TypeFoundationBindingError::Access { .. })
    ));
}

#[test]
fn a_valid_generated_key_for_another_object_is_not_backing_authority() {
    let output = lower_public_declarations(vec![object_decl("First"), object_decl("Second")]);
    let fixture = Fixture::from_output(&output);
    fixture.bind().unwrap();
    let entries = fixture.source.clone().into_entries();
    let other_backing = match entries.representations.records()[1].shape() {
        hir::NominalRepresentationShapeV1::Object { backing_class, .. } => *backing_class,
        _ => unreachable!(),
    };
    let record = &entries.representations.records()[0];
    let bound = fixture.bind().unwrap();
    let forged = hir::NominalRepresentationSupportV1::try_new(
        bound
            .nominal_key(hir::SourceNominalId::Concrete(record.owner()))
            .unwrap(),
        record.declaration_access().clone(),
        hir::NominalRepresentationShapeV1::Object {
            backing_class: other_backing,
            declared_fields: vec![],
        },
    );
    assert!(matches!(
        forged,
        Err(hir::NominalRepresentationBuildError::ObjectBackingClass)
    ));
}

#[test]
fn object_binding_requires_both_declared_generated_and_exact_source_refs() {
    let fixture = Fixture::from_output(&lower_public_declarations(vec![object_decl("Object")]));
    let backing = fixture.source.entries().generated_nominals.values()[0];
    let exact =
        PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(backing)).unwrap();
    let mut entries = fixture.source.clone().into_entries();
    entries.generated_nominals = hir::CanonicalPersistentIdsV1::empty();
    let source = hir::TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    assert!(matches!(
        source.bind_to_foundation(&fixture.foundation, &fixture.identities),
        Err(hir::TypeFoundationBindingError::MissingGenerated(id)) if id == backing
    ));
    let mut entries = fixture.source.clone().into_entries();
    entries.exact_keys = hir::CanonicalPersistentIdsV1::try_new(
        entries
            .exact_keys
            .values()
            .iter()
            .copied()
            .filter(|id| *id != exact)
            .collect(),
    )
    .unwrap();
    let source = hir::TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    assert!(
        matches!(source.bind_to_foundation(&fixture.foundation, &fixture.identities), Err(hir::TypeFoundationBindingError::MissingExact(id)) if id == exact)
    );
}
