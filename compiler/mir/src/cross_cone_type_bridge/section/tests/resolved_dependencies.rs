use super::*;

fn resolve(
    fixture: &Fixture,
    section: &CrossConeMirTypeBridgeSectionV1<'_>,
    dependencies: &[&CrossConeMirTypeBridgeSectionV1<'_>],
    graph: &mut ValidatedIdentityGraph,
) -> DependencyResolvedCrossConeMirTypeBridgeSectionV1 {
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 =
        decode_canonical(&encode(section).unwrap(), DecodeLimits::default()).unwrap();
    decoded
        .resolve_types::<&'static str>(
            fixture.source.provider,
            &fixture.types.foundation,
            dependencies.iter().map(|section| section.types()),
            graph,
            &mut meter(),
        )
        .unwrap()
        .resolve_callables::<&'static str>(
            &fixture.types.foundation,
            dependencies
                .iter()
                .map(|section| (section.types(), section.callables(), section.dispatch())),
            graph,
            &mut meter(),
        )
        .unwrap()
        .resolve_dependencies::<&'static str>(fixture.authority(), graph, &mut meter())
        .unwrap()
}

#[test]
fn owned_dependency_transport_replays_local_fields_and_explicit_shape_roots() {
    let provider = Fixture::new("owned-provider");
    let mut consumer = Fixture::new("owned-consumer");
    let mut graph = graph(&[&provider, &consumer]);
    consumer.with_field(provider.types.payload.id(), &graph);
    let terminal = provider.section(&[], &graph).unwrap();
    for roots in [vec![], vec![provider.shape_use()]] {
        consumer.source.uses = roots.clone();
        let section = consumer.section(&[&terminal], &graph).unwrap();
        let resolved = resolve(&consumer, &section, &[&terminal], &mut graph);
        assert_eq!(resolved.provider(), consumer.source.provider);
        assert_eq!(resolved.exports().types(), section.types());
        resolved
            .replay_dependency_closure::<&'static str>(
                &[],
                &[terminal.view()],
                &roots,
                &graph,
                &mut meter(),
            )
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
    consumer.source.uses = vec![provider.type_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer.section(&[&terminal], &graph).unwrap();
    let mut resolved = resolve(&consumer, &section, &[&terminal], &mut graph);
    let replay = |resolved: &DependencyResolvedCrossConeMirTypeBridgeSectionV1, roots: &[_]| {
        resolved.replay_dependency_closure::<&'static str>(
            &[],
            &[terminal.view()],
            roots,
            &graph,
            &mut meter(),
        )
    };
    assert!(matches!(
        replay(&resolved, &[]),
        Err(MirTypeBridgeSectionError::SelectedClosure)
    ));
    let wrong =
        MirTypeBridgeDependencyV1::new(consumer.source.provider, provider.type_use().target());
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
        replay(&resolved, &consumer.source.uses),
        Err(MirTypeBridgeSectionError::SelectedClosure)
    ));
    resolved.selected.push(provider.type_use());
    resolved.selected.push(provider.shape_use());
    resolved.selected.sort_unstable();
    assert!(matches!(
        replay(&resolved, &consumer.source.uses),
        Err(MirTypeBridgeSectionError::SelectedClosure)
    ));
}

#[test]
fn owned_dependency_graph_rejects_missing_duplicate_providers_and_budget_exhaustion() {
    let provider = Fixture::new("owned-budget-provider");
    let mut consumer = Fixture::new("owned-budget-consumer");
    consumer.source.uses = vec![provider.type_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let section = consumer.section(&[&terminal], &graph).unwrap();
    let resolved = resolve(&consumer, &section, &[&terminal], &mut graph);
    let replay = |dependencies: &[_], meter: &mut BudgetMeter| {
        resolved.replay_dependency_closure::<&'static str>(
            &[],
            dependencies,
            &consumer.source.uses,
            &graph,
            meter,
        )
    };
    assert!(matches!(
        replay(&[], &mut meter()),
        Err(MirTypeBridgeSectionError::MissingDependency(_))
    ));
    assert!(replay(&[terminal.view(), terminal.view()], &mut meter()).is_err());
    let limits = DecodeLimits {
        validation_work_units: 0,
        ..DecodeLimits::default()
    };
    assert!(matches!(
        replay(&[terminal.view()], &mut BudgetMeter::new(limits)),
        Err(MirTypeBridgeSectionError::Resource(_)
            | MirTypeBridgeSectionError::Lookup(MirTypeBridgeLookupError::Resource(_)))
    ));
    let mut measured = meter();
    replay(&[terminal.view()], &mut measured).unwrap();
    let required = measured.usage().validation_work_units;
    let mut inclusive = BudgetMeter::new(DecodeLimits {
        validation_work_units: required,
        ..DecodeLimits::default()
    });
    replay(&[terminal.view()], &mut inclusive).unwrap();
    assert!(replay(&[terminal.view()], &mut inclusive).is_err());
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: required - 1,
        ..DecodeLimits::default()
    });
    assert!(replay(&[terminal.view()], &mut short).is_err());
}
