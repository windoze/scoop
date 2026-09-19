//! Initialization dependencies are retained across complete surface replay.

use super::*;

#[test]
fn full_surface_replays_foreign_initialization_and_local_storage_and_callables() {
    for lazy in [false, true] {
        let fixture = Fixture::complete_surface(Options {
            lazy,
            ..Options::default()
        });
        let producer = fixture.foundation.producer();
        let provider = Fixture::with_source(Options::default(), ConeIdentity::CORE, "provider");
        let reference = dependency(producer, fixture.unit, &provider);
        let plans = build(&fixture, vec![reference]).unwrap();
        let initialization = Semantics::from_artifact(
            fixture.semantics.static_storages().clone(),
            vec![plans.registrations()[0].semantic().clone()],
        );
        let original = crate::StrongRegistrationProductionSurfaceV2::from_semantics(
            LirTargetProfile::DARWIN_AARCH64,
            &fixture.foundation,
            &fixture.digests,
            fixture.identities.clone(),
            crate::StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(
                &fixture.foundation,
            )
            .unwrap(),
            crate::StrongTypeDescriptorSemanticPlanSetV2::from_artifact(
                producer,
                LirTargetProfile::DARWIN_AARCH64.wire_id(),
                Vec::new(),
            ),
            crate::StrongSafepointSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            crate::StrongImmortalObjectSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            initialization,
        )
        .unwrap();
        let decoded: crate::DecodedStrongRegistrationProductionSurfaceV2 =
            decode_canonical(&encode(&original).unwrap(), DecodeLimits::default()).unwrap();
        let replayed = decoded
            .replay(
                LirTargetProfile::DARWIN_AARCH64,
                &fixture.foundation,
                &fixture.digests,
                &crate::StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
                &crate::StrongTypeReferenceDefinitionsV2::new(producer, &[], &mut meter()).unwrap(),
                &Catalog::new(producer, &[definition(&provider)], &mut meter()).unwrap(),
                &mut meter(),
            )
            .unwrap();
        assert_eq!(
            replayed.initialization_units(),
            original.initialization_units()
        );
        assert_eq!(replayed.static_storages(), original.static_storages());
        assert_eq!(replayed.callables(), original.callables());
        assert_eq!(
            replayed.initialization_units().registrations()[0]
                .semantic()
                .dependencies(),
            &[reference]
        );
    }
}
