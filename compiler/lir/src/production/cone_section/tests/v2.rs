//! Wire-version checks do not replace the selected layout closure proof.

use super::*;

#[test]
fn layout_schema_retains_eight_shared_production_fields() {
    let coordinate = ConeCoordinate::new("test", "strong-section", "0.0.0").unwrap();
    let (foundation, digests) = fixture(&coordinate);
    let producer = foundation.producer();
    let target = LirTargetProfile::DARWIN_AARCH64;

    let identities =
        crate::RegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    let registrations = crate::StrongRegistrationProductionSurfaceV2::from_semantics(
        target,
        &foundation,
        &digests,
        identities,
        crate::StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(&foundation)
            .unwrap(),
        crate::StrongTypeDescriptorSemanticPlanSetV2::from_artifact(
            producer,
            target.wire_id(),
            Vec::new(),
        ),
        crate::StrongSafepointSemanticPlanSetV1::from_artifact(producer, Vec::new()),
        crate::StrongImmortalObjectSemanticPlanSetV1::from_artifact(producer, Vec::new()),
        crate::StrongInitializationUnitSemanticPlanSetV2::from_artifact(
            crate::StrongStaticStorageSemanticPlanSetV1::from_artifact(producer, Vec::new()),
            Vec::new(),
        ),
    )
    .unwrap();
    let current = ConeProductionSectionV2::from_parts(
        coordinate.clone(),
        &[],
        &foundation,
        digests.clone(),
        registrations,
        EntryProductionSourceV1::Library,
        &[],
    )
    .unwrap();
    let old = ConeProductionSectionV1::new(
        coordinate,
        &[],
        &foundation,
        digests.clone(),
        StrongRegistrationProductionSurfaceV1::empty(target, &foundation, &digests).unwrap(),
        EntryProductionSourceV1::Library,
        &[],
    )
    .unwrap();
    let bytes = encode(&current).unwrap();
    super::digests::check(&current, &foundation);
    assert_eq!(bytes[0], 0xa8);
    assert_eq!(bytes, encode(&old).unwrap());
    let decoded: DecodedConeProductionSectionV2 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(decode_canonical::<DecodedConeProductionSectionV2>(&truncated).is_err());
    for field_count in [0xa7, 0xa9] {
        let mut malformed = bytes.clone();
        malformed[0] = field_count;
        assert!(decode_canonical::<DecodedConeProductionSectionV2>(&malformed).is_err());
    }
}
