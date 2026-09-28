use super::*;

fn resolve(
    fixture: &Fixture,
    section: &CrossConeMirTypeBridgeSectionV1<'_>,
    dependencies: &[MirTypeBridgeDependencyViewV1<'_>],
    graph: &mut ValidatedIdentityGraph,
) -> DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 =
        decode_canonical(&encode(section).unwrap()).unwrap();
    read(fixture, decoded, dependencies, graph).unwrap()
}

pub(super) fn read(
    fixture: &Fixture,
    decoded: DecodedCrossConeMirTypeBridgeSectionV1,
    dependencies: &[MirTypeBridgeDependencyViewV1<'_>],
    graph: &mut ValidatedIdentityGraph,
) -> Result<DependencyResolvedCrossConeMirTypeBridgeSectionV1, MirTypeBridgeSectionError> {
    let direct = std::iter::once(fixture.authority().ordinary)
        .chain(
            dependencies
                .iter()
                .map(|section| section.direct_callables()),
        )
        .collect::<Vec<_>>();
    decoded
        .resolve_types(
            fixture.provider,
            &fixture.types.foundation,
            dependencies.iter().map(|section| section.exports().types()),
            graph,
        )?
        .resolve_callables(
            &fixture.types.foundation,
            &direct,
            dependencies.iter().map(|section| {
                (
                    section.exports().types(),
                    section.exports().callables(),
                    section.exports().dispatch(),
                )
            }),
            graph,
        )?
        .resolve_dependencies(fixture.authority(), graph)
}

#[test]
fn owned_dependency_transport_replays_local_fields_and_explicit_shape_roots() {
    let provider = Fixture::new("owned-provider");
    let mut consumer = Fixture::new("owned-consumer");
    let mut graph = graph(&[&provider, &consumer]);
    consumer.with_field(provider.types.payload.id(), &graph);
    let terminal = provider.section(&[], &graph).unwrap();
    for roots in [vec![], vec![provider.shape_use()]] {
        consumer.uses = roots.clone();
        let section = consumer
            .section(&[terminal.dependency_view()], &graph)
            .unwrap();
        let resolved = resolve(
            &consumer,
            &section,
            &[terminal.dependency_view()],
            &mut graph,
        );
        assert_eq!(resolved.provider(), consumer.provider);
        assert_eq!(resolved.exports().types(), section.exports().types());
        resolved
            .replay_dependency_closure(&[], &[terminal.dependency_view()], &roots, &graph)
            .unwrap();
        assert_eq!(
            resolved.selected_relations(),
            section.selected().relations().collect::<Vec<_>>(),
        );
        assert_eq!(
            resolved.selected_relations().len(),
            if roots.is_empty() { 1 } else { 5 }
        );
    }
}

#[test]
fn owned_dependency_graph_rejects_candidate_drift_and_invented_roots() {
    let provider = Fixture::new("owned-selected-provider");
    let mut consumer = Fixture::new("owned-selected-consumer");
    consumer.uses = vec![provider.type_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer
        .section(&[terminal.dependency_view()], &graph)
        .unwrap();
    let mut resolved = resolve(
        &consumer,
        &section,
        &[terminal.dependency_view()],
        &mut graph,
    );
    let replay = |resolved: &DependencyResolvedCrossConeMirTypeBridgeSectionV1, roots: &[_]| {
        resolved.replay_dependency_closure(&[], &[terminal.dependency_view()], roots, &graph)
    };
    assert!(matches!(
        replay(&resolved, &[]),
        Err(MirTypeBridgeSectionError::SelectedClosure)
    ));
    let wrong = MirTypeBridgeDependencyV1::new(consumer.provider, provider.type_use().target());
    assert!(matches!(
        replay(&resolved, &[wrong]),
        Err(MirTypeBridgeSectionError::SelectedCurrentProvider)
    ));
    let wrong = MirTypeBridgeDependencyV1::new(ConeIdentity::CORE, provider.type_use().target());
    assert!(matches!(
        replay(&resolved, &[wrong]),
        Err(MirTypeBridgeSectionError::MissingTarget(_))
    ));
    resolved.selected.clear();
    assert!(matches!(
        replay(&resolved, &consumer.uses),
        Err(MirTypeBridgeSectionError::SelectedClosure)
    ));
    resolved.selected.push(provider.type_use());
    resolved.selected.push(provider.shape_use());
    resolved.selected.sort_unstable();
    assert!(matches!(
        replay(&resolved, &consumer.uses),
        Err(MirTypeBridgeSectionError::SelectedClosure)
    ));
}

#[test]
fn owned_dependency_graph_rejects_missing_and_duplicate_providers() {
    let provider = Fixture::new("owned-provider");
    let mut consumer = Fixture::new("owned-consumer");
    consumer.uses = vec![provider.type_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer
        .section(&[terminal.dependency_view()], &graph)
        .unwrap();
    let resolved = resolve(
        &consumer,
        &section,
        &[terminal.dependency_view()],
        &mut graph,
    );
    let replay = |dependencies: &[_]| {
        resolved.replay_dependency_closure(&[], dependencies, &consumer.uses, &graph)
    };
    assert!(matches!(
        replay(&[]),
        Err(MirTypeBridgeSectionError::MissingDependency(_))
    ));
    assert!(replay(&[terminal.dependency_view(), terminal.dependency_view()]).is_err());

    replay(&[terminal.dependency_view()]).unwrap();
}
