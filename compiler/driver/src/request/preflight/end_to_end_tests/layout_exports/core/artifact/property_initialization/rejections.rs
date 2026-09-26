use super::*;
use scoop_slib::SharedMirDependencyGraphError as Error;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    core_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core: mir::MirTypeBridgeDependencyViewV1<'_>,
) {
    let foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(input.hir).unwrap(),
    )
    .unwrap();
    let core_foundation = hir::OdrFreeHirFoundation::try_new(
        hir::CanonicalHirFoundation::from_type_semantics_output(core_input.hir).unwrap(),
    )
    .unwrap();
    let metadata = hir::SharedTypeMetadataV1 {
        provider: section.provider(),
        identities: input.identities,
        foundation: &foundation,
        public: input.public,
    };
    let dependencies = [hir::SharedTypeMetadataV1 {
        provider: core.provider(),
        identities: input.identities,
        foundation: &core_foundation,
        public: core_input.public,
    }];
    let views = [core];
    let replay = |candidate: &_| {
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &dependencies,
            candidate,
            section.initialization_units(),
            &views,
        )
    };
    let records = section.initialization_uses().records();
    let valid = wire::resolve(
        input,
        section,
        &views,
        section.initialization_uses(),
        input.identities,
    );
    replay(&valid).unwrap();
    let first = records[0];
    let mut extra = records.to_vec();
    extra.push(
        mir::SelectedExternalInitializationUseV1::try_new(
            section.provider(),
            input.identities,
            first.local_unit(),
            first.provider(),
            first.dependency_unit(),
            mir::MirExternalInitializationCauseV1::InitializationSupport(first.dependency_unit()),
        )
        .unwrap(),
    );
    for candidate in [vec![], records[1..].to_vec(), extra] {
        let uses = mir::CanonicalMirExternalInitializationUsesV1::try_new(candidate).unwrap();
        assert!(matches!(
            replay(&wire::resolve(
                input,
                section,
                &views,
                &uses,
                input.identities
            ),),
            Err(Error::InitializationUseInventory)
        ));
    }
    assert!(
        mir::SelectedExternalInitializationUseV1::try_new(
            section.provider(),
            input.identities,
            first.local_unit(),
            section.provider(),
            first.dependency_unit(),
            first.cause(),
        )
        .is_err()
    );
    assert!(mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![first, first]).is_err());
    assert!(matches!(
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &dependencies,
            &valid,
            &[],
            &views,
        ),
        Err(Error::MissingInitializationUnit(_))
    ));

    replay(&valid).unwrap();
}
