//! V2 dependency references survive the complete eight-field surface reader.

use super::*;
use crate::{
    DecodedStrongRegistrationProductionSurfaceV2, ReplayedStrongRegistrationProductionV2,
    StrongInitializationDefinitionCatalogV2, StrongRegistrationProductionSurfaceV2,
    StrongTypeReferenceDefinitionsV2,
};

fn surface(fixture: &Fixture, foreign: bool) -> StrongRegistrationProductionSurfaceV2 {
    let producer = fixture.foundation.producer();
    StrongRegistrationProductionSurfaceV2::from_semantics(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.digests,
        fixture.identities.clone(),
        crate::StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(
            &fixture.foundation,
        )
        .unwrap(),
        semantics(fixture, foreign.then_some(ConeIdentity::CORE)),
        crate::StrongSafepointSemanticPlanSetV1::from_artifact(producer, Vec::new()),
        crate::StrongImmortalObjectSemanticPlanSetV1::from_artifact(producer, Vec::new()),
        crate::StrongInitializationUnitSemanticPlanSetV2::from_artifact(
            crate::StrongStaticStorageSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            Vec::new(),
        ),
    )
    .unwrap()
}

fn replay(
    fixture: &Fixture,
    bytes: &[u8],
    definitions: &StrongTypeReferenceDefinitionsV2,
    meter: &mut BudgetMeter,
) -> Result<ReplayedStrongRegistrationProductionV2, Error> {
    let decoded: DecodedStrongRegistrationProductionSurfaceV2 =
        decode_canonical(bytes, DecodeLimits::default()).unwrap();
    let producer = fixture.foundation.producer();
    decoded.replay(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.digests,
        &StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap(),
        definitions,
        &StrongInitializationDefinitionCatalogV2::new(producer, &[], &mut super::meter()).unwrap(),
        meter,
    )
}

#[test]
fn complete_surface_replays_legacy_and_dependency_type_registrations() {
    for foreign in [false, true] {
        let fixture = if foreign {
            foreign_fixture()
        } else {
            Fixture::new(Options {
                first_type_has_itable: true,
                ..Options::default()
            })
        };
        let original = surface(&fixture, foreign);
        let references = catalog(&semantics(&fixture, foreign.then_some(ConeIdentity::CORE)));
        let replayed = replay(
            &fixture,
            &encode(&original).unwrap(),
            &references,
            &mut meter(),
        )
        .unwrap();
        assert_eq!(replayed.identities(), original.identities());
        assert_eq!(replayed.types(), original.types());
        assert_eq!(replayed.safepoints(), original.safepoints());
        assert_eq!(replayed.callables(), original.callables());
        assert_eq!(replayed.static_storages(), original.static_storages());
        assert_eq!(replayed.immortal_objects(), original.immortal_objects());
        assert_eq!(
            replayed.initialization_units(),
            original.initialization_units()
        );
    }
}

#[test]
fn complete_surface_rejects_missing_or_wrong_consumer_definitions() {
    let fixture = foreign_fixture();
    let bytes = encode(&surface(&fixture, true)).unwrap();
    let empty = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &[], &mut meter())
        .unwrap();
    assert!(matches!(
        replay(&fixture, &bytes, &empty, &mut meter()),
        Err(Error::TypeReference(_))
    ));
    let wrong =
        StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[], &mut meter()).unwrap();
    assert!(matches!(
        replay(&fixture, &bytes, &wrong, &mut meter()),
        Err(Error::ProducerMismatch)
    ));
}

#[test]
fn full_surface_reader_shares_inclusive_heap_and_work_limits() {
    let fixture = foreign_fixture();
    let bytes = encode(&surface(&fixture, true)).unwrap();
    let definitions = catalog(&semantics(&fixture, Some(ConeIdentity::CORE)));
    let mut baseline = meter();
    replay(&fixture, &bytes, &definitions, &mut baseline).unwrap();
    let usage = baseline.usage();
    for limits in [
        DecodeLimits {
            logical_heap_bytes: usage.logical_heap_bytes - 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: usage.validation_work_units - 1,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            replay(
                &fixture,
                &bytes,
                &definitions,
                &mut BudgetMeter::new(limits)
            ),
            Err(Error::Resource(_))
        ));
    }
    let mut shared = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: usage.logical_heap_bytes,
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    replay(&fixture, &bytes, &definitions, &mut shared).unwrap();
    assert!(matches!(
        replay(&fixture, &bytes, &definitions, &mut shared),
        Err(Error::Resource(_))
    ));
}
