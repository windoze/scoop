use std::path::Path;

use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate};
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::CanonicalMirFoundation;
use scoop_protocol::{
    DiagnosticOriginV1, DiagnosticSeverityV1, HostPathCarrier, ProtocolByteSpan,
    StructuredDiagnosticV1,
};
use scoop_slib::{
    ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
    IdentityFoundationArtifactInput, ProducerRecord,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode, sha256};

use super::*;

fn artifact_fingerprint() -> ArtifactFingerprint {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("cache-receipt-test").unwrap(),
        core_cone(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap()
    .artifact_fingerprint()
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
            ArtifactCapabilityProfileId::cross_cone_semantics_strong(),
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
    let decoded = decode_cache_receipt_v1(&bytes, DecodeLimits::M23_DEFAULT).unwrap();

    assert_eq!(decoded, receipt);
    assert_eq!(
        decoded.body().structured_warnings()[0].code(),
        "SCOOPC_A_WARNING"
    );

    assert_eq!(
        receipt.fingerprint().to_string(),
        "ea4e639a635125aa4f5ade5dacb5ea506f075607d3bb68be42311a0dbcdfc156"
    );
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
    let mut decoded: DecodedCacheReceiptV1 =
        decode_canonical(&bytes, DecodeLimits::M23_DEFAULT).unwrap();
    decoded.fingerprint = sha256(b"tampered");

    assert!(matches!(
        decode_cache_receipt_v1(&encode(&decoded).unwrap(), DecodeLimits::M23_DEFAULT),
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
    let mut decoded: DecodedCacheReceiptV1 =
        decode_canonical(&bytes, DecodeLimits::M23_DEFAULT).unwrap();
    decoded.body.structured_warnings.swap(0, 1);

    assert!(matches!(
        decode_cache_receipt_v1(&encode(&decoded).unwrap(), DecodeLimits::M23_DEFAULT),
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
            ArtifactCapabilityProfileId::cross_cone_semantics_strong(),
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
            ArtifactCapabilityProfileId::cross_cone_semantics_strong(),
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

#[test]
fn receipt_decode_is_budgeted() {
    let bytes = encode(&receipt_with_warnings(vec![warning(
        "SCOOPC_WARNING",
        "warning",
    )]))
    .unwrap();
    let limits = DecodeLimits {
        owned_bytes: 0,
        ..DecodeLimits::M23_DEFAULT
    };

    assert!(matches!(
        decode_cache_receipt_v1(&bytes, limits),
        Err(CacheReceiptDecodeError::Wire(_))
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
        ArtifactCapabilityProfileId::cross_cone_semantics_strong(),
        vec![warning],
    )
}
