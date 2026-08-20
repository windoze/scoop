use std::path::Path;

use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_protocol::{
    DiagnosticOriginV1, DiagnosticSeverityV1, HostPathCarrier, ProtocolByteSpan,
    StructuredDiagnosticV1,
};
use scoop_slib::{ConeKind, ConeRecord, ConeSourceForm};
use scoop_wire::{decode_canonical, encode, sha256};

use super::*;

fn artifact_fingerprint() -> ArtifactFingerprint {
    ArtifactFingerprint::from_array(*sha256(b"cache-receipt-test").as_array())
}

fn core_cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn compiler() -> PairedCompilerFingerprintV1 {
    PairedCompilerFingerprintV1::from_parts(
        sha256(b"paired-scoopc"),
        sha256(b"distribution"),
        sha256(b"build"),
    )
}

fn warning(code: &str, message: &str) -> StructuredDiagnosticV1 {
    StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Warning,
        code.to_owned(),
        message.to_owned(),
        DiagnosticOriginV1::None,
        Vec::new(),
    )
    .unwrap()
}

fn receipt_with_warnings(warnings: Vec<StructuredDiagnosticV1>) -> CacheReceiptV1 {
    CacheReceiptV1::new(
        CacheReceiptBodyV1::new(
            ConeCompileCacheKeyV1::from_digest(sha256(b"cache-key")),
            artifact_fingerprint(),
            core_cone(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            Vec::new(),
            compiler(),
            ArtifactCapabilityProfileId::cross_cone_generic(),
            warnings,
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn receipt_round_trips_with_a_fixed_fingerprint() {
    let receipt = receipt_with_warnings(vec![
        warning("SCOOPC_Z_WARNING", "z warning"),
        warning("SCOOPC_A_WARNING", "a warning"),
    ]);
    let bytes = encode(&receipt).unwrap();
    let decoded = decode_cache_receipt_v1(&bytes).unwrap();

    assert_eq!(decoded, receipt);
    assert_eq!(
        decoded.body().structured_warnings()[0].code(),
        "SCOOPC_A_WARNING"
    );

    assert_eq!(
        receipt.fingerprint().to_string(),
        "a3a62d4d378ff2f263adf73a39c382d8a5da059280734e7c804a43f16eb4e9ee"
    );
}

#[test]
fn receipt_preserves_each_target_and_rejects_mixed_backend_tags() {
    let mut fingerprints = std::collections::BTreeSet::new();
    for id in [
        TargetProfileId::DarwinAarch64,
        TargetProfileId::LinuxX86_64Gnu,
        TargetProfileId::LinuxX86_64Musl,
    ] {
        let mut body = receipt_with_warnings(Vec::new()).body().clone();
        body.target_selection =
            CacheTargetSelectionV1::new(ValidatedLirTargetSelection::from_id(id));
        let receipt = CacheReceiptV1::new(body).unwrap();
        assert_eq!(
            decode_cache_receipt_v1(&encode(&receipt).unwrap()).unwrap(),
            receipt
        );
        assert!(fingerprints.insert(receipt.fingerprint()));
    }
    for (target, backend) in [(1, 2), (2, 1), (3, 1), (4, 2)] {
        assert!(
            DecodedCacheTargetSelectionV1 { target, backend }
                .validate()
                .is_err()
        );
    }
}

#[test]
fn receipt_rejects_retired_layout_profile_with_its_original_fingerprint() {
    let receipt = receipt_with_warnings(vec![
        warning("SCOOPC_Z_WARNING", "z warning"),
        warning("SCOOPC_A_WARNING", "a warning"),
    ]);
    let mut decoded: DecodedCacheReceiptV1 = decode_canonical(&encode(&receipt).unwrap()).unwrap();
    let profile = scoop_identity::CapabilityId::new(
        "org.scoop-lang.slib-profile",
        "cross-cone-layout-strong",
        3,
    )
    .unwrap();
    decoded.body.artifact_profile = decode_canonical(&encode(&profile).unwrap()).unwrap();
    decoded.fingerprint =
        domain_separated_cbor_hash(RECEIPT_FINGERPRINT_DOMAIN, &decoded.body).unwrap();
    assert_eq!(
        decoded.fingerprint.to_string(),
        "0107ee21dd581bbfabec042263a0f0d11719376e5ae398d8f91b7588b92b2c60"
    );
    assert!(matches!(
        decode_cache_receipt_v1(&encode(&decoded).unwrap()),
        Err(CacheReceiptDecodeError::UnknownArtifactProfile)
    ));
}

#[test]
fn receipt_rejects_the_legacy_single_cone_profile() {
    assert!(matches!(
        CacheReceiptBodyV1::new(
            ConeCompileCacheKeyV1::from_digest(sha256(b"cache-key")),
            artifact_fingerprint(),
            core_cone(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            Vec::new(),
            compiler(),
            ArtifactCapabilityProfileId::single_cone_strong(),
            Vec::new(),
        ),
        Err(CacheReceiptValidationError::UnsupportedArtifactProfile)
    ));
}

#[test]
fn receipt_rejects_fingerprint_tampering() {
    let receipt = receipt_with_warnings(vec![warning("SCOOPC_WARNING", "warning")]);
    let bytes = encode(&receipt).unwrap();
    let mut decoded: DecodedCacheReceiptV1 = decode_canonical(&bytes).unwrap();
    decoded.fingerprint = sha256(b"tampered");

    assert!(matches!(
        decode_cache_receipt_v1(&encode(&decoded).unwrap()),
        Err(CacheReceiptDecodeError::FingerprintMismatch { .. })
    ));
}

#[test]
fn receipt_rejects_noncanonical_warning_order() {
    let receipt = receipt_with_warnings(vec![
        warning("SCOOPC_A_WARNING", "a warning"),
        warning("SCOOPC_Z_WARNING", "z warning"),
    ]);
    let bytes = encode(&receipt).unwrap();
    let mut decoded: DecodedCacheReceiptV1 = decode_canonical(&bytes).unwrap();
    decoded.body.structured_warnings.swap(0, 1);

    assert!(matches!(
        decode_cache_receipt_v1(&encode(&decoded).unwrap()),
        Err(CacheReceiptDecodeError::Validation(
            CacheReceiptValidationError::WarningOrder
        ))
    ));
}

#[test]
fn receipt_rejects_duplicate_and_conflicting_warning_keys() {
    let duplicate = warning("SCOOPC_WARNING", "same");
    assert!(matches!(
        CacheReceiptBodyV1::new(
            ConeCompileCacheKeyV1::from_digest(sha256(b"cache-key")),
            artifact_fingerprint(),
            core_cone(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            Vec::new(),
            compiler(),
            ArtifactCapabilityProfileId::cross_cone_generic(),
            vec![duplicate.clone(), duplicate],
        ),
        Err(CacheReceiptValidationError::DuplicateWarningKey(_))
    ));
    assert!(matches!(
        CacheReceiptBodyV1::new(
            ConeCompileCacheKeyV1::from_digest(sha256(b"cache-key")),
            artifact_fingerprint(),
            core_cone(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            Vec::new(),
            compiler(),
            ArtifactCapabilityProfileId::cross_cone_generic(),
            vec![
                warning("SCOOPC_WARNING", "first"),
                warning("SCOOPC_WARNING", "second"),
            ],
        ),
        Err(CacheReceiptValidationError::ConflictingWarningKey(_))
    ));
}

#[test]
fn receipt_rejects_non_warning_and_host_path_diagnostics() {
    let error = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Error,
        "SCOOPC_ERROR".to_owned(),
        "error".to_owned(),
        DiagnosticOriginV1::None,
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        receipt_body_with_warning(error),
        Err(CacheReceiptValidationError::NonWarningDiagnostic(_))
    ));

    let host_warning = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Warning,
        "SCOOPC_HOST_WARNING".to_owned(),
        "host warning".to_owned(),
        DiagnosticOriginV1::HostPathSpan {
            path: HostPathCarrier::from_path(Path::new("/tmp/input.scoop")).unwrap(),
            span: ProtocolByteSpan::new(0, 1).unwrap(),
        },
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        receipt_body_with_warning(host_warning),
        Err(CacheReceiptValidationError::HostPathDiagnosticOrigin)
    ));
}

fn receipt_body_with_warning(
    warning: StructuredDiagnosticV1,
) -> Result<CacheReceiptBodyV1, CacheReceiptValidationError> {
    CacheReceiptBodyV1::new(
        ConeCompileCacheKeyV1::from_digest(sha256(b"cache-key")),
        artifact_fingerprint(),
        core_cone(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        Vec::new(),
        compiler(),
        ArtifactCapabilityProfileId::cross_cone_generic(),
        vec![warning],
    )
}
