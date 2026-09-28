use super::*;
use scoop_slib::SharedMirDependencyGraphError as Error;

mod wire;

pub(super) fn check(
    name: &str,
    fixtures: &Path,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core: mir::MirTypeBridgeDependencyViewV1<'_>,
    shapes: &[(ConeIdentity, scoop_identity::PersistentTypeId)],
) {
    let foundation = hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: section.provider(),
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    let dependencies = [core];
    let materialized_units = input
        .mir
        .materialization()
        .initialization_roots()
        .iter()
        .map(|root| root.identity())
        .collect::<Vec<_>>();
    let expected = section.selected().relations().collect::<Vec<_>>();
    let resolved = wire::resolve(input, section, &dependencies, &expected, input.identities);
    let replay = |candidate: &_| {
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &[],
            candidate,
            section.initialization_units(),
            &materialized_units,
            &dependencies,
        )
    };
    replay(&resolved).unwrap();
    for (provider, owner) in shapes {
        let shape = core.exports().shapes().get(*owner).unwrap();
        for target in [
            mir::MirTypeBridgeTargetV1::ShapeSupport(*owner),
            mir::MirTypeBridgeTargetV1::Type(shape.coroutine_step()),
            mir::MirTypeBridgeTargetV1::Type(shape.coroutine_slot()),
        ] {
            let required = mir::MirTypeBridgeDependencyV1::new(*provider, target);
            assert!(expected.contains(&required));
            let missing = expected
                .iter()
                .copied()
                .filter(|relation| *relation != required)
                .collect::<Vec<_>>();
            assert!(
                matches!(replay(&wire::resolve(input, section, &dependencies, &missing, input.identities)), Err(Error::Mir(error)) if matches!(*error, mir::MirTypeBridgeSectionError::SelectedClosure))
            );
        }
        if let mir::MirBoxedShapeSupportV1::Available(exact) = shape.boxed() {
            assert!(expected.contains(&mir::MirTypeBridgeDependencyV1::new(
                *provider,
                mir::MirTypeBridgeTargetV1::Type(exact)
            )));
        }
    }
    let unused = core
        .exports()
        .shapes()
        .records()
        .iter()
        .map(|shape| {
            mir::MirTypeBridgeDependencyV1::new(
                core.provider(),
                mir::MirTypeBridgeTargetV1::ShapeSupport(shape.source()),
            )
        })
        .find(|relation| !expected.contains(relation))
        .unwrap();
    let mut extra = expected.clone();
    extra.push(unused);
    extra.sort_unstable();
    assert!(
        matches!(replay(&wire::resolve(input, section, &dependencies, &extra, input.identities)), Err(Error::Mir(error)) if matches!(*error, mir::MirTypeBridgeSectionError::SelectedClosure))
    );

    replay(&resolved).unwrap();
    assert!(matches!(
        scoop_slib::replay_shared_mir_dependency_graph(
            hir::SharedTypeMetadataV1 {
                provider: core.provider(),
                ..metadata
            },
            &[],
            &resolved,
            section.initialization_units(),
            &materialized_units,
            &dependencies
        ),
        Err(Error::InputProvider { .. })
    ));
    assert!(
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &[],
            &resolved,
            section.initialization_units(),
            &materialized_units,
            &[]
        )
        .is_err()
    );
    let dump = expected
        .iter()
        .map(|relation| format!("{} {:?}\n", relation.provider(), relation.target()))
        .collect::<String>();
    snapshot(&fixtures.join(format!("{name}.mir.snap")), &dump);
}
