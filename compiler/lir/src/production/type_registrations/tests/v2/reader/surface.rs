//! V2 dependency references survive the complete eight-field surface reader.

use super::*;
use crate::{
    DecodedStrongRegistrationProductionSurfaceV2, StrongInitializationDefinitionCatalogV2,
    StrongRegistrationProductionSurfaceV2, StrongTypeReferenceDefinitionsV2,
};

mod section;

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
) -> Result<StrongRegistrationProductionSurfaceV2, Error> {
    let decoded: DecodedStrongRegistrationProductionSurfaceV2 = decode_canonical(bytes).unwrap();
    let producer = fixture.foundation.producer();
    decoded.replay(
        crate::LirTargetProfile::DARWIN_AARCH64,
        &fixture.foundation,
        &fixture.digests,
        definitions,
        &StrongInitializationDefinitionCatalogV2::new(producer, &[]).unwrap(),
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
        let replayed = replay(&fixture, &encode(&original).unwrap(), &references).unwrap();
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
    let empty = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::SINGLE_FILE, &[]).unwrap();
    assert!(matches!(
        replay(&fixture, &bytes, &empty),
        Err(Error::TypeReference(_))
    ));
    let wrong = StrongTypeReferenceDefinitionsV2::new(ConeIdentity::CORE, &[]).unwrap();
    assert!(matches!(
        replay(&fixture, &bytes, &wrong),
        Err(Error::ProducerMismatch)
    ));
}
