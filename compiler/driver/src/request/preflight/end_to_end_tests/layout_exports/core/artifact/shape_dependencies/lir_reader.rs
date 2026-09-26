//! Isolates shared graph replay from the still separate machine/object join.

use super::super::lir_dependencies::corruption::wire;
use super::*;
use scoop_slib::SharedLirDependencyGraphError as Error;

pub(super) fn check(
    name: &str,
    fixtures: &Path,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    core: &lir::CrossConeLayoutAbiSectionV1<'_>,
    shapes: &[(ConeIdentity, scoop_identity::PersistentTypeId)],
) {
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap(),
    )
    .unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: input.mir.module().cone,
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    let exports = wire::empty_exports(metadata.provider, core.target_profile());
    let mut roots = metadata
        .public
        .external_references()
        .materialized_type_dependencies(metadata.provider, metadata.identities)
        .unwrap()
        .into_iter()
        .map(|(provider, exact)| {
            let value = core
                .layouts()
                .find_exact_role(exact, scoop_identity::RepresentationRole::ManagedValue)
                .unwrap();
            lir::LayoutAbiDependencyV1::new(
                provider,
                lir::LayoutAbiSemanticTargetV1::Layout(value.identity().layout()),
            )
        })
        .collect::<Vec<_>>();
    roots.extend(shapes.iter().map(|(provider, owner)| {
        lir::LayoutAbiDependencyV1::new(
            *provider,
            lir::LayoutAbiSemanticTargetV1::ShapeSupport(*owner),
        )
    }));
    roots.sort_unstable();
    roots.dedup();
    let section = lir::CrossConeLayoutAbiSectionV1::try_new(
        exports.clone(),
        &[core.exports()],
        vec![],
        &roots,
    )
    .unwrap();
    let expected = section.selected().semantic_relations().collect::<Vec<_>>();
    let resolved = wire::resolve(&section, &expected, input.identities).unwrap();
    let replay = |candidate: &_| {
        scoop_slib::replay_shared_lir_dependency_graph(metadata, candidate, &[core.exports()])
    };
    replay(&resolved).unwrap();
    for (provider, owner) in shapes {
        let shape = lir::LayoutAbiDependencyV1::new(
            *provider,
            lir::LayoutAbiSemanticTargetV1::ShapeSupport(*owner),
        );
        assert!(expected.contains(&shape));
        let missing = expected
            .iter()
            .copied()
            .filter(|relation| *relation != shape)
            .collect::<Vec<_>>();
        assert!(
            matches!(replay(&wire::resolve(&section, &missing, input.identities).unwrap()), Err(Error::Lir(error)) if matches!(*error, lir::LayoutAbiSectionError::SelectedClosure))
        );
    }
    let descriptors = expected
        .iter()
        .copied()
        .filter(|relation| {
            matches!(
                relation.target(),
                lir::LayoutAbiSemanticTargetV1::Descriptor(_)
            )
        })
        .collect::<Vec<_>>();
    assert!(!descriptors.is_empty());
    for descriptor in &descriptors {
        let missing = expected
            .iter()
            .copied()
            .filter(|relation| relation != descriptor)
            .collect::<Vec<_>>();
        assert!(
            matches!(replay(&wire::resolve(&section, &missing, input.identities).unwrap()), Err(Error::Lir(error)) if matches!(*error, lir::LayoutAbiSectionError::SelectedClosure))
        );
    }
    let only_layouts = expected
        .iter()
        .copied()
        .filter(|relation| matches!(relation.target(), lir::LayoutAbiSemanticTargetV1::Layout(_)))
        .collect::<Vec<_>>();
    assert!(
        matches!(replay(&wire::resolve(&section, &only_layouts, input.identities).unwrap()), Err(Error::Lir(error)) if matches!(*error, lir::LayoutAbiSectionError::SelectedClosure))
    );

    replay(&resolved).unwrap();
    let dump = expected
        .iter()
        .map(|relation| format!("{} {:?}\n", relation.provider(), relation.target()))
        .collect::<String>();
    snapshot(&fixtures.join(format!("{name}.lir.snap")), &dump);
}
