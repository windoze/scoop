use super::*;
use scoop_slib::SharedMirDependencyGraphError as Error;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    core_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    core: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    source: &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
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
    let resolved_core = wire::resolve(core, &[], core.initialization_uses(), input.identities);
    let views = [resolved_core.dependency_view(core.initialization_units())];
    let replay = |candidate: &_, meter: &mut BudgetMeter| {
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &dependencies,
            candidate,
            section.initialization_units(),
            &views,
            meter,
        )
    };
    let records = section.initialization_uses().records();
    let valid = wire::resolve(
        section,
        &[core],
        section.initialization_uses(),
        input.identities,
    );
    replay(&valid, &mut meter()).unwrap();
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
            &mut meter(),
        )
        .unwrap(),
    );
    for candidate in [vec![], records[1..].to_vec(), extra] {
        let uses = mir::CanonicalMirExternalInitializationUsesV1::try_new(candidate, &mut meter())
            .unwrap();
        let exports = section.exports();
        let candidate = mir::MirTypeBridgeExportConstituentsV1::new(
            exports.types().clone(),
            exports.callables().clone(),
            exports.dispatch().clone(),
            exports.objects().clone(),
            exports.shapes().clone(),
            uses.clone(),
        );
        assert!(matches!(
            candidate.validate_sources(section.provider(), input.identities, source, &mut meter(),),
            Err(mir::MirTypeBridgeSourceJoinError::Record(
                mir::MirTypeBridgeSourceRecordV1::InitializationUses
            ))
        ));
        assert!(matches!(
            replay(
                &wire::resolve(section, &[core], &uses, input.identities),
                &mut meter(),
            ),
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
            &mut meter(),
        )
        .is_err()
    );
    assert!(
        mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![first, first], &mut meter())
            .is_err()
    );
    assert!(matches!(
        scoop_slib::replay_shared_mir_dependency_graph(
            metadata,
            &dependencies,
            &valid,
            &[],
            &views,
            &mut meter(),
        ),
        Err(Error::MissingInitializationUnit(_))
    ));
    let mut measured = meter();
    replay(&valid, &mut measured).unwrap();
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(&valid, &mut exact).unwrap();
    assert!(replay(&valid, &mut exact).is_err());
}
