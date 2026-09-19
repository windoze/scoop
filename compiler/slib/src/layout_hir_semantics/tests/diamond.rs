use super::*;

#[test]
fn public_closure_deduplicates_a_diamond_and_keeps_one_terminal_proof() {
    let terminal = artifact_bytes("hir-terminal");
    let terminal_record = dependency_record(&terminal);
    let left = artifact_bytes_with_dependencies("hir-left", vec![terminal_record.clone()]);
    let right = artifact_bytes_with_dependencies("hir-right", vec![terminal_record]);
    let facade = artifact_bytes_with_dependencies(
        "hir-facade",
        vec![dependency_record(&left), dependency_record(&right)],
    );
    let bytes = [terminal, left, right, facade];
    let providers = bytes
        .iter()
        .map(|bytes| checked_artifact(bytes))
        .collect::<Vec<_>>();
    let identities = providers
        .iter()
        .map(|provider| provider.identity())
        .collect::<Vec<_>>();
    let mut closure = layout_hir_semantic_closure_for_test(
        cone_named("hir-current").identity(),
        target(),
        vec![identities[3]],
        providers,
        vec![vec![], vec![0], vec![0], vec![1, 2]],
    );
    let mut factories = identities
        .iter()
        .copied()
        .map(RecordingPublicFactory::new)
        .collect::<Vec<_>>();
    let mut authorities = factories
        .iter_mut()
        .zip(identities.iter().copied())
        .map(|(factory, provider)| LayoutHirProviderPublicAuthorityV1::new(provider, factory))
        .collect::<Vec<_>>();

    closure
        .with_checked_hir_public_support(&mut authorities, |checked| {
            assert_eq!(
                checked
                    .dependency_first()
                    .map(|provider| provider.provider())
                    .collect::<Vec<_>>(),
                identities
            );
            assert_eq!(checked.direct_providers(), &[identities[3]]);
        })
        .unwrap();

    assert_eq!(factories[1].dependency_providers(), &[identities[0]]);
    assert_eq!(factories[2].dependency_providers(), &[identities[0]]);
    let mut facade_dependencies = vec![identities[0], identities[1], identities[2]];
    facade_dependencies.sort_unstable();
    assert_eq!(
        factories[3].dependency_providers(),
        facade_dependencies.as_slice()
    );
    assert_eq!(
        factories[1].dependency_proof_addresses(),
        factories[2].dependency_proof_addresses()
    );
}

#[test]
fn complete_semantic_diamond_borrows_the_terminal_checked_section() {
    let (terminal_bytes, nominal) = nominal_artifact_bytes("hir-type-terminal");
    let terminal = cone_named("hir-type-terminal").identity();
    let selected = SelectedExternalTypeUseV1::new(
        terminal,
        SelectedTypeUseV1::Representation {
            exact: nominal.exact,
        },
    );
    let terminal_record = dependency_record(&terminal_bytes);
    let left_bytes =
        artifact_bytes_with_dependencies("hir-type-left", vec![terminal_record.clone()]);
    let right_bytes = artifact_bytes_with_dependencies("hir-type-right", vec![terminal_record]);
    let facade_bytes = selecting_artifact_bytes(
        "hir-type-facade",
        selected,
        vec![
            dependency_record(&left_bytes),
            dependency_record(&right_bytes),
        ],
    );
    let mut terminal_provider = checked_artifact(&terminal_bytes);
    let mut left_provider =
        checked_artifact_with_authorities(&left_bytes, [identity_graph(&mut terminal_provider)]);
    let mut right_provider =
        checked_artifact_with_authorities(&right_bytes, [identity_graph(&mut terminal_provider)]);
    let facade_provider = checked_artifact_with_authorities(
        &facade_bytes,
        [
            identity_graph(&mut left_provider),
            identity_graph(&mut right_provider),
        ],
    );
    let providers = vec![
        terminal_provider,
        left_provider,
        right_provider,
        facade_provider,
    ];
    let identities = providers
        .iter()
        .map(|provider| provider.identity())
        .collect::<Vec<_>>();
    let mut closure = layout_hir_semantic_closure_for_test(
        cone_named("hir-type-current").identity(),
        target(),
        vec![identities[3]],
        providers,
        vec![vec![], vec![0], vec![0], vec![1, 2]],
    );
    let foundations = [
        EmptyFoundation::with_nominal(&nominal),
        EmptyFoundation::with_dependency(identities[1], &nominal),
        EmptyFoundation::with_dependency(identities[2], &nominal),
        EmptyFoundation::with_dependency(identities[3], &nominal),
    ];
    let mut declarations = [
        EmptyDeclarations::with_nominal(&nominal),
        EmptyDeclarations::new(),
        EmptyDeclarations::new(),
        EmptyDeclarations::new(),
    ];
    let mut defaults = [
        EmptyDefaults::with_nominal(&nominal),
        EmptyDefaults::new(identities[1]),
        EmptyDefaults::new(identities[2]),
        EmptyDefaults::new(identities[3]),
    ];
    let uses = [
        EmptyCommittedUses::default(),
        EmptyCommittedUses::default(),
        EmptyCommittedUses::default(),
        EmptyCommittedUses::with_root(terminal, nominal.exact),
    ];
    let mut public = identities
        .iter()
        .copied()
        .map(RecordingPublicFactory::new)
        .collect::<Vec<_>>();
    let mut authorities = public
        .iter_mut()
        .zip(foundations.iter())
        .zip(declarations.iter_mut())
        .zip(defaults.iter_mut())
        .zip(uses.iter())
        .zip(identities.iter().copied())
        .map(
            |(((((public, foundation), declarations), defaults), committed), provider)| {
                LayoutHirProviderSemanticAuthoritiesV1::new(
                    provider,
                    public,
                    foundation,
                    declarations,
                    defaults,
                    committed,
                )
            },
        )
        .collect::<Vec<_>>();

    closure
        .with_checked_hir_semantics(&mut authorities, |checked| {
            let terminal = checked.provider(identities[0]).unwrap().type_semantics();
            let facade = checked.provider(identities[3]).unwrap().type_semantics();
            assert_eq!(facade.selected().len(), 1);
            assert!(std::ptr::eq(facade.selected()[0].terminal(), terminal));
            assert_eq!(facade.selected()[0].target().request(), selected);
        })
        .unwrap();

    assert_eq!(uses[3].validation_calls(), 1);
}

#[test]
fn semantic_closure_rejects_an_authority_for_the_wrong_provider() {
    let bytes = artifact_bytes("hir-provider");
    let provider = checked_artifact(&bytes);
    let expected = provider.identity();
    let mut closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![provider],
        vec![vec![]],
    );
    let wrong = cone_named("wrong-hir-provider").identity();
    let foundation = EmptyFoundation::new(wrong);
    let mut declarations = EmptyDeclarations::new();
    let mut defaults = EmptyDefaults::new(wrong);
    let committed = EmptyCommittedUses::default();
    let mut factory = RecordingPublicFactory::new(wrong);
    let mut authorities = [LayoutHirProviderSemanticAuthoritiesV1::new(
        wrong,
        &mut factory,
        &foundation,
        &mut declarations,
        &mut defaults,
        &committed,
    )];

    assert!(matches!(
        closure.with_checked_hir_semantics(&mut authorities, |_| ()),
        Err(CrossConeLayoutHirSemanticClosureError::AuthorityProvider {
            position: 0,
            expected: actual_expected,
            actual,
        }) if actual_expected == expected && actual == wrong
    ));
    assert_eq!(factory.build_count(), 0);
}
