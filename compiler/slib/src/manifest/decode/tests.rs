use scoop_identity::{CapabilityId, ConeCoordinate};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::{decode_canonical, encode};

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
        CompatibilityRecord::new(
            selection(),
            crate::ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        )
        .unwrap(),
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        Vec::new(),
        &members(),
        SemanticFingerprintRecord::from_digests(
            HirFingerprint::from_array([1; 32]),
            MirFingerprint::from_array([2; 32]),
            LirFingerprint::from_array([3; 32]),
            crate::FingerprintAvailability::Available(crate::CodeFingerprint::from_array([4; 32])),
            crate::FingerprintAvailability::Available(crate::RuntimeImageFingerprint::from_array(
                [5; 32],
            )),
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
    decoded.validate(selection())
}

#[test]
fn manifest_round_trips_through_untrusted_validation() {
    let expected = manifest(Vec::new());
    let decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encode(&expected).unwrap()).unwrap();

    assert_eq!(validate(decoded), Ok(expected));
}

#[test]
fn manifest_and_compatibility_schema_stay_at_the_initial_value() {
    let encoded = encode(&manifest(Vec::new())).unwrap();
    let mut decoded = decode_canonical::<DecodedBootstrapManifest>(&encoded).unwrap();
    decoded.manifest_schema = 2;
    assert_eq!(
        validate(decoded),
        Err(BootstrapManifestValidationError::UnsupportedSchema {
            kind: SchemaKind::Manifest,
            actual: 2,
        })
    );

    let mut decoded = decode_canonical::<DecodedBootstrapManifest>(&encoded).unwrap();
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
    let mut decoded = decode_canonical::<DecodedBootstrapManifest>(&encoded).unwrap();
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

    let mut decoded = decode_canonical::<DecodedBootstrapManifest>(&encoded).unwrap();
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
    let mut decoded = decode_canonical::<DecodedBootstrapManifest>(&encoded).unwrap();
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

    let decoded =
        decode_canonical::<DecodedBootstrapManifest>(&encode(&expected).unwrap()).unwrap();

    assert_eq!(validate(decoded), Ok(expected));
}
