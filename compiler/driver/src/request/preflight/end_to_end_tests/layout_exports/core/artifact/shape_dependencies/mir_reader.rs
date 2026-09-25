use super::*;
use scoop_slib::SharedMirDependencyGraphError as Error;

mod wire;

pub(super) fn check(
    name: &str,
    fixtures: &Path,
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    shapes: &[(ConeIdentity, scoop_identity::PersistentTypeId)],
) {
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap(),
    )
    .unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: section.provider(),
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    let core_resolved = wire::resolve(core, &[], &[], input.identities);
    let dependencies = [core_resolved.dependency_view(core.initialization_units())];
    let expected = section.selected().relations().collect::<Vec<_>>();
    let resolved = wire::resolve(section, &[core], &expected, input.identities);
    let replay = |candidate: &_, meter: &mut BudgetMeter| {
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &[],
            candidate,
            section.initialization_units(),
            &dependencies,
            meter,
        )
    };
    replay(&resolved, &mut meter()).unwrap();
    for (provider, owner) in shapes {
        let shape = core.shape_support().get(*owner).unwrap();
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
                matches!(replay(&wire::resolve(section, &[core], &missing, input.identities), &mut meter()), Err(Error::Mir(error)) if matches!(*error, mir::MirTypeBridgeSectionError::SelectedClosure))
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
        .shape_support()
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
        matches!(replay(&wire::resolve(section, &[core], &extra, input.identities), &mut meter()), Err(Error::Mir(error)) if matches!(*error, mir::MirTypeBridgeSectionError::SelectedClosure))
    );
    let mut measured = meter();
    replay(&resolved, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(&resolved, &mut shared).unwrap();
    assert!(replay(&resolved, &mut shared).is_err());
    assert!(matches!(
        scoop_slib::replay_shared_mir_dependency_graph(
            hir::SharedTypeMetadataV1 {
                provider: core.provider(),
                ..metadata
            },
            &[],
            &resolved,
            section.initialization_units(),
            &dependencies,
            &mut meter()
        ),
        Err(Error::InputProvider { .. })
    ));
    assert!(
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &[],
            &resolved,
            section.initialization_units(),
            &[],
            &mut meter()
        )
        .is_err()
    );
    let dump = expected
        .iter()
        .map(|relation| format!("{} {:?}\n", relation.provider(), relation.target()))
        .collect::<String>();
    snapshot(&fixtures.join(format!("{name}.mir.snap")), &dump);
}
