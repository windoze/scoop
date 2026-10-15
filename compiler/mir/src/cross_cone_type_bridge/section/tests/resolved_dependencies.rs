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
fn owned_dependency_transport_reads_local_fields_and_explicit_shape_roots() {
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
