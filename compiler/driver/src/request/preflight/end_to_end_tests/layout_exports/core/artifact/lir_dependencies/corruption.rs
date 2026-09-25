use super::*;
use scoop_slib::SharedLirDependencyGraphError as Error;

mod resources;
mod wire;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
    core: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap(),
    )
    .unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: layout.provider(),
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    let expected = layout.selected().semantic_relations().collect::<Vec<_>>();
    let resolve = |candidate: &[_]| wire::resolve(layout, candidate, input.identities);
    let replay = |candidate: &lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
                  dependencies: &[_]| {
        scoop_slib::replay_shared_lir_dependency_graph(
            metadata,
            candidate,
            dependencies,
            &mut meter(),
        )
    };
    let resolved = resolve(&expected).unwrap();
    replay(&resolved, &[core.exports()]).unwrap();
    for index in 0..expected.len() {
        let mut missing = expected.clone();
        missing.remove(index);
        assert!(
            matches!(replay(&resolve(&missing).unwrap(), &[core.exports()]), Err(Error::Lir(error)) if matches!(*error, lir::LayoutAbiSectionError::SelectedClosure))
        );
    }
    let unused = core
        .layouts()
        .records()
        .iter()
        .map(|record| {
            lir::LayoutAbiDependencyV1::new(
                core.provider(),
                lir::LayoutAbiSemanticTargetV1::Layout(record.identity().layout()),
            )
        })
        .find(|relation| !expected.contains(relation))
        .unwrap();
    let mut extra = expected.clone();
    extra.push(unused);
    extra.sort_unstable();
    assert!(
        matches!(replay(&resolve(&extra).unwrap(), &[core.exports()]), Err(Error::Lir(error)) if matches!(*error, lir::LayoutAbiSectionError::SelectedClosure))
    );
    let local = lir::LayoutAbiDependencyV1::new(layout.provider(), expected[0].target());
    assert!(matches!(
        resolve(&[local]),
        Err(lir::LayoutAbiSectionError::SelectedCurrentProvider)
    ));
    assert!(matches!(
        resolve(&[expected[0], expected[0]]),
        Err(lir::LayoutAbiSectionError::NonCanonicalSelected { index: 1 })
    ));
    if expected.len() > 1 {
        let mut reversed = expected.clone();
        reversed.reverse();
        assert!(matches!(
            resolve(&reversed),
            Err(lir::LayoutAbiSectionError::NonCanonicalSelected { index: 1 })
        ));
    }
    assert!(matches!(
        replay(&resolved, &[]),
        Err(Error::TypeOccurrences(_))
    ));
    assert!(
        matches!(replay(&resolved, &[core.exports(), core.exports()]), Err(Error::DependencyProvider(provider)) if provider == core.provider())
    );
    assert!(
        matches!(replay(&resolved, &[layout.exports()]), Err(Error::DependencyProvider(provider)) if provider == layout.provider())
    );
    let absent = wire::empty_exports(core.provider(), layout.target_profile());
    assert!(
        matches!(replay(&resolved, &[&absent]), Err(Error::MissingTypeLayout { provider, .. }) if provider == core.provider())
    );
    let wrong_provider = hir::SharedTypeMetadataV1 {
        provider: core.provider(),
        ..metadata
    };
    assert!(matches!(
        scoop_slib::replay_shared_lir_dependency_graph(
            wrong_provider,
            &resolved,
            &[core.exports()],
            &mut meter()
        ),
        Err(Error::InputProvider { .. })
    ));
    resources::check(metadata, &resolved, core.exports());
    if layout.layouts().records().is_empty() {
        missing_occurrences(metadata, &resolved, core.exports());
    }
}

fn missing_occurrences(
    metadata: hir::SharedTypeMetadataV1<'_>,
    layout: &lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
    core: &lir::LayoutAbiExportConstituentsV1,
) {
    let public = metadata.public;
    let incomplete = hir::CrossConeHirInterfaceSectionV1::new(
        public.public_bindings().clone(),
        public.nominal_interfaces().clone(),
        public.callable_interfaces().clone(),
        public.property_interfaces().clone(),
        public.type_aliases().clone(),
        public.source_interfaces().clone(),
        public.default_templates().clone(),
        public.constants().clone(),
        public.definition_sources().clone(),
        hir::CanonicalExternalHirReferencesV1::default(),
    );
    let metadata = hir::SharedTypeMetadataV1 {
        public: &incomplete,
        ..metadata
    };
    assert!(
        matches!(scoop_slib::replay_shared_lir_dependency_graph(metadata, layout, &[core], &mut meter()), Err(Error::Lir(error)) if matches!(*error, lir::LayoutAbiSectionError::SelectedClosure))
    );
}
