//! Wire-version checks do not replace the selected layout closure proof.

use super::*;

#[test]
fn layout_schema_retains_ten_fields_and_the_shared_initialization_payload() {
    let coordinate = ConeCoordinate::new("test", "strong-section", "0.0.0").unwrap();
    let (foundation, digests) = fixture(&coordinate);
    let producer = foundation.producer();
    let target = LirTargetProfile::DARWIN_AARCH64;
    let external = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let identities =
        crate::StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
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
    let current = StrongProductionSectionV2::from_parts(
        coordinate.clone(),
        &[],
        &foundation,
        external.clone(),
        digests.clone(),
        registrations,
        EntryProductionSourceV1::Library,
        &[],
        None,
    )
    .unwrap();
    let old = StrongProductionSectionV1::new(
        coordinate,
        &[],
        &foundation,
        external,
        digests.clone(),
        StrongRegistrationProductionSurfaceV1::empty(target, &foundation, &digests).unwrap(),
        EntryProductionSourceV1::Library,
        &[],
        None,
    )
    .unwrap();
    let bytes = encode(&current).unwrap();
    assert_eq!(bytes[0], 0xaa);
    assert_initialization_field(&bytes);
    assert_eq!(bytes, encode(&old).unwrap());
    let decoded: DecodedStrongProductionSectionV2 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let mut truncated = bytes.clone();
    truncated.pop();
    assert!(
        decode_canonical::<DecodedStrongProductionSectionV2>(&truncated, DecodeLimits::default())
            .is_err()
    );
    for field_count in [0xa9, 0xab] {
        let mut malformed = bytes.clone();
        malformed[0] = field_count;
        assert!(
            decode_canonical::<DecodedStrongProductionSectionV2>(
                &malformed,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
}
