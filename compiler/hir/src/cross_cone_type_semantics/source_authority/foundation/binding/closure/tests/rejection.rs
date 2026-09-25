use super::*;

#[test]
fn source_and_public_providers_must_match() {
    let root = Artifact::class("root", None).load(None);
    let bound = root.bind();
    let public = empty_public();
    assert!(matches!(
        TypeFoundationSourceProviderV1::try_new(&bound, public_proof(&public, ConeIdentity::CORE)),
        Err(TypeFoundationReplayError::PublicProvider { .. })
    ));
}

#[test]
fn dependencies_cannot_be_missing_repeated_local_or_out_of_order() {
    let base = Artifact::class("base", None).load(None);
    let child = Artifact::class("child", Some(&base)).load(Some(&base));
    let unrelated = Artifact::class("unrelated", None).load(None);
    let bounds = [base.bind(), child.bind(), unrelated.bind()];
    let public = empty_public();
    let [base, child, unrelated] = bounds.each_ref().map(|bound| provider(bound, &public));
    assert!(matches!(TypeFoundationSourceClosureV1::try_new(child, &[]),
        Err(TypeFoundationReplayError::MissingProvider(id)) if id == base.provider()));
    let mut descending = [base, unrelated];
    descending.sort_by_key(|entry| std::cmp::Reverse(entry.provider()));
    for dependencies in [vec![base, base], vec![child], descending.to_vec()] {
        assert!(matches!(
            TypeFoundationSourceClosureV1::try_new(child, &dependencies),
            Err(TypeFoundationReplayError::DependencyOrder)
        ));
    }
}

#[test]
fn dependency_fact_must_belong_to_the_named_provider_even_when_exact_key_is_available() {
    let base = Artifact::class("base", None).load(None);
    let child = Artifact::class("child", Some(&base)).load(Some(&base));
    let mut entries = child.source.clone().into_entries();
    entries.dependency_facts =
        CanonicalTypeSectionDependencyFactsV1::try_new(vec![TypeSectionDependencyFactV1 {
            provider: child.provider(),
            exact: base.exact,
        }])
        .unwrap();
    let source = TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    let child_bound = source
        .bind_to_foundation(&child.foundation, &child.identities)
        .unwrap();
    let base_bound = base.bind();
    let public = empty_public();
    assert!(
        matches!(TypeFoundationSourceClosureV1::try_new(provider(&child_bound, &public),
        &[provider(&base_bound, &public)]),
        Err(TypeFoundationReplayError::DependencyFact { provider, exact }) if provider == child.provider() && exact == base.exact)
    );
}

#[test]
fn locally_owned_facts_cannot_be_claimed_by_two_providers() {
    let base = Artifact::class("base", None).load(None);
    let child = Artifact::class("child", Some(&base)).load(Some(&base));
    let mut entries = child.source.clone().into_entries();
    entries.dependency_facts = Default::default();
    entries.local_exact_facts =
        CanonicalPersistentIdsV1::try_new(vec![child.exact, base.exact]).unwrap();
    entries.fact_shapes = CanonicalExactTypeFactShapesV1::try_new(vec![
        ExactTypeFactShapeRecordV1::new(child.exact, ExactTypeFactShapeV1::Reference),
        ExactTypeFactShapeRecordV1::new(base.exact, ExactTypeFactShapeV1::Reference),
    ])
    .unwrap();
    let source = TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    let child_bound = source
        .bind_to_foundation(&child.foundation, &child.identities)
        .unwrap();
    let base_bound = base.bind();
    let public = empty_public();
    assert!(
        matches!(TypeFoundationSourceClosureV1::try_new(provider(&child_bound, &public),
        &[provider(&base_bound, &public)]),
        Err(TypeFoundationReplayError::DuplicateFact(exact)) if exact == base.exact)
    );
}

#[test]
fn replay_requires_fact_keys_in_the_owning_source_inventory() {
    let root = Artifact::class("root", None).load(None);
    let mut entries = root.source.clone().into_entries();
    entries.exact_keys = CanonicalPersistentIdsV1::empty();
    let source = TypeFoundationSourceAuthorityV1::try_new(entries).unwrap();
    let bound = source
        .bind_to_foundation(&root.foundation, &root.identities)
        .unwrap();
    let public = empty_public();
    assert!(
        matches!(TypeFoundationSourceClosureV1::try_new(provider(&bound, &public), &[]),
        Err(TypeFoundationReplayError::Binding(TypeFoundationBindingError::MissingExact(exact))) if exact == root.exact)
    );
}

#[test]
fn origins_are_routed_by_provider_and_require_the_exact_record() {
    let root = Artifact::class("root", None).load(None);
    let other = Artifact::class("other", None).load(None);
    let bound = root.bind();
    let public = empty_public();
    let closure = TypeFoundationSourceClosureV1::try_new(provider(&bound, &public), &[]).unwrap();
    let absent = &other.source.entries().definition_sources.sources()[0];
    assert!(matches!(closure.validate_definition_source(absent),
        Err(TypeFoundationReplayError::MissingProvider(provider)) if provider == other.provider()));
    let known = &root.source.entries().definition_sources.sources()[0];
    let changed = ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(
            known.origin().source().clone(),
            SourceSpan::new(0, 0).unwrap(),
            &SourceContextKey::File {
                source: known.origin().source().clone(),
            },
        )
        .unwrap(),
    );
    assert!(matches!(
        closure.validate_definition_source(&changed),
        Err(TypeFoundationReplayError::Binding(
            TypeFoundationBindingError::MissingDefinitionSource
        ))
    ));
}
