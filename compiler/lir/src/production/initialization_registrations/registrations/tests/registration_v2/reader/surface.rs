//! Initialization dependencies are retained across complete surface replay.

use super::*;

#[test]
fn full_surface_replays_foreign_initialization_and_local_storage_and_callables() {
    for lazy in [false, true] {
        let mut fixture = Fixture::complete_surface(Options {
            lazy,
            ..Options::default()
        });
        let coordinate = scoop_identity::ConeCoordinate::reserved_single_file();
        crate::production::strong_section::tests::attach_image(
            &coordinate,
            &mut fixture.foundation,
            &mut fixture.digests,
        );
        let producer = fixture.foundation.producer();
        let provider_id =
            scoop_identity::ConeCoordinate::new("test", "initialization-provider", "1.0.0")
                .unwrap()
                .identity()
                .unwrap();
        let provider = Fixture::with_source(Options::default(), provider_id, "provider");
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
        let external =
            crate::StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
        let section = crate::StrongProductionSectionV2::from_parts(
            coordinate.clone(),
            &[],
            &fixture.foundation,
            external.clone(),
            fixture.digests.clone(),
            original,
            crate::EntryProductionSourceV1::Library,
            &[],
            None,
        )
        .unwrap();
        let decoded: crate::DecodedStrongProductionSectionV2 =
            decode_canonical(&encode(&section).unwrap(), DecodeLimits::default()).unwrap();
        let complete = decoded
            .replay(
                coordinate,
                &[],
                LirTargetProfile::DARWIN_AARCH64,
                &fixture.foundation,
                external,
                crate::EntryProductionSourceV1::Library,
                &[],
                None,
                &crate::StrongTypeReferenceDefinitionsV2::new(producer, &[], &mut meter()).unwrap(),
                &Catalog::new(producer, &[definition(&provider)], &mut meter()).unwrap(),
                &mut meter(),
            )
            .unwrap();
        assert_eq!(
            complete.initialization_registrations(),
            replayed.initialization_units()
        );
        assert_eq!(
            complete.static_storage_registrations(),
            replayed.static_storages()
        );
        assert_eq!(complete.callable_registrations(), replayed.callables());
    }
}
