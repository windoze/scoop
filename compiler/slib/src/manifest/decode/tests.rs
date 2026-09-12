use scoop_identity::{CapabilityId, ConeCoordinate};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{
    BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, decode_canonical, encode,
};

use super::*;
use crate::{
    CompatibilityFingerprintKind, CompatibilityRecord, CompatibilitySchemaKind, ConeKind,
    ConeRecord, ConeSourceForm, HirFingerprint, LirFingerprint, ManifestSection, MemberPurposeSet,
    MemberStableKey, MirFingerprint, ProducerRecord, SemanticFingerprintRecord, SlibMember,
    SlibMemberRole,
};

fn members() -> Vec<SlibMember> {
    let cone = ConeCoordinate::reserved_core().identity().unwrap();
    [
        (
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            b"hir".as_slice(),
        ),
        (
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            b"mir".as_slice(),
        ),
        (
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            b"lir".as_slice(),
        ),
    ]
    .into_iter()
    .map(|(key, role, payload)| SlibMember::new(cone, key, role, payload.to_vec()).unwrap())
    .collect()
}

fn manifest(sections: Vec<ManifestSection>) -> BootstrapManifest {
    BootstrapManifest::new(
        ProducerRecord::new("dev").unwrap(),
        CompatibilityRecord::identity_foundation(selection()).unwrap(),
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        Vec::new(),
        &members(),
        SemanticFingerprintRecord::from_foundation_digests(
            HirFingerprint::from_array([1; 32]),
            MirFingerprint::from_array([2; 32]),
            LirFingerprint::from_array([3; 32]),
        ),
        sections,
    )
    .unwrap()
}

fn selection() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

fn validate(
    decoded: DecodedBootstrapManifest,
) -> Result<BootstrapManifest, BootstrapManifestValidationError> {
    decoded.validate(selection(), &mut BudgetMeter::new(DecodeLimits::default()))
}

fn resource_error(error: &BootstrapManifestValidationError) -> Option<&scoop_wire::WireError> {
    match error {
        BootstrapManifestValidationError::Compatibility(
            CompatibilityValidationError::Resource(error),
        )
        | BootstrapManifestValidationError::Cone(ConeRecordValidationError::Resource(error))
        | BootstrapManifestValidationError::Dependency {
            error: DependencyRecordValidationError::Resource(error),
            ..
        }
        | BootstrapManifestValidationError::Budget(error)
        | BootstrapManifestValidationError::Manifest(BootstrapManifestError::Resource(error)) => {
            Some(error)
        }
        BootstrapManifestValidationError::Member { error, .. } => match error.as_ref() {
            crate::SlibMemberRecordValidationError::Resource(error) => Some(error),
            _ => None,
        },
        _ => None,
    }
}

#[test]
fn manifest_round_trips_through_untrusted_validation() {
    let expected = manifest(Vec::new());
    let decoded = decode_canonical::<DecodedBootstrapManifest>(
        &encode(&expected).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(validate(decoded), Ok(expected));
}

#[test]
fn manifest_validation_budget_has_inclusive_boundaries() {
    let encoded = encode(&manifest(Vec::new())).unwrap();
    let usage = {
        let decoded =
            decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default())
                .unwrap();
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        decoded.validate(selection(), &mut meter).unwrap();
        meter.usage()
    };
    assert_eq!(usage.logical_heap_bytes, 3 * 32);
    assert_eq!(usage.owned_bytes, 0);
    assert!(usage.validation_work_units > 0);

    for (limit, accepted) in [
        (usage.validation_work_units - 1, false),
        (usage.validation_work_units, true),
        (usage.validation_work_units + 1, true),
    ] {
        let decoded =
            decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default())
                .unwrap();
        let mut meter = BudgetMeter::new(DecodeLimits {
            validation_work_units: limit,
            ..DecodeLimits::default()
        });
        let result = decoded.validate(selection(), &mut meter);
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            let error = result.unwrap_err();
            assert!(matches!(
                resource_error(&error).map(scoop_wire::WireError::kind),
                Some(WireErrorKind::LimitExceeded {
                    resource: ResourceKind::ValidationWorkUnits,
                    ..
                })
            ));
        }
    }

    for (limit, accepted) in [(95, false), (96, true), (97, true)] {
        let decoded =
            decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default())
                .unwrap();
        let mut meter = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: limit,
            ..DecodeLimits::default()
        });
        let result = decoded.validate(selection(), &mut meter);
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(matches!(
                result,
                Err(BootstrapManifestValidationError::Budget(ref error))
                    if error.kind() == &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::LogicalHeapBytes,
                        limit: 95,
                        observed: 96,
                    }
            ));
        }
    }
}

#[test]
fn manifest_and_compatibility_schema_stay_at_the_initial_value() {
    let encoded = encode(&manifest(Vec::new())).unwrap();
    let mut decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default()).unwrap();
    decoded.manifest_schema = 2;
    assert_eq!(
        validate(decoded),
        Err(BootstrapManifestValidationError::UnsupportedSchema {
            kind: SchemaKind::Manifest,
            actual: 2,
        })
    );

    let mut decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default()).unwrap();
    decoded.compatibility.set_identity_schema(2);
    assert_eq!(
        validate(decoded),
        Err(BootstrapManifestValidationError::Compatibility(
            CompatibilityValidationError::UnsupportedSchema {
                kind: CompatibilitySchemaKind::Identity,
                actual: 2,
            }
        ))
    );
}

#[test]
fn decoded_manifest_recomputes_cone_and_fingerprints() {
    let encoded = encode(&manifest(Vec::new())).unwrap();
    let mut decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default()).unwrap();
    decoded.compatibility.corrupt_language_fingerprint();
    assert!(matches!(
        validate(decoded),
        Err(BootstrapManifestValidationError::Compatibility(
            CompatibilityValidationError::FingerprintMismatch {
                kind: CompatibilityFingerprintKind::LanguageAbi,
                ..
            }
        ))
    ));

    let mut decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default()).unwrap();
    let mut bytes = *decoded.artifact_fingerprint.as_array();
    bytes[0] ^= 1;
    decoded.artifact_fingerprint = Digest256::from_array(bytes);
    assert!(matches!(
        validate(decoded),
        Err(BootstrapManifestValidationError::ArtifactFingerprintMismatch { .. })
    ));
}

#[test]
fn reader_rejects_wire_order_instead_of_sorting_it() {
    let encoded = encode(&manifest(Vec::new())).unwrap();
    let mut decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encoded, DecodeLimits::default()).unwrap();
    decoded.members.swap(0, 1);

    assert!(matches!(
        validate(decoded),
        Err(BootstrapManifestValidationError::NonIncreasingMember { .. })
    ));
}

#[test]
fn manifest_section_payload_uses_the_carrier_limit() {
    let section = ManifestSection::new(
        CapabilityId::new("org.scoop-lang.test", "opaque", 1).unwrap(),
        MemberPurposeSet::NONE,
        vec![0x55; 65],
    )
    .unwrap();
    let expected = manifest(vec![section]);
    let limits = DecodeLimits {
        semantic_leaf_bytes: 64,
        ..DecodeLimits::default()
    };
    let decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encode(&expected).unwrap(), limits).unwrap();

    assert_eq!(validate(decoded), Ok(expected));
}
