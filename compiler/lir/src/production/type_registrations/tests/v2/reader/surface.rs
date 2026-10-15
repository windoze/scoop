//! Production fixtures retain foreign type references in their wire records.

use super::*;
use crate::StrongRegistrationProductionSurfaceV2;

mod section;

fn surface(fixture: &Fixture, foreign: bool) -> StrongRegistrationProductionSurfaceV2 {
    let producer = fixture.foundation.producer();
    StrongRegistrationProductionSurfaceV2::from_semantics(
        crate::target_fixtures::DARWIN_AARCH64,
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
