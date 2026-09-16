use scoop_hir::CanonicalHirFoundation;
use scoop_identity::ArtifactCapabilityProfileId;
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::CanonicalMirFoundation;
use scoop_slib::{
    ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
    IdentityFoundationArtifactInput, ProducerRecord,
};
use scoop_wire::{DecodeLimits, encode, sha256};

use super::*;

fn artifact() -> IdentityFoundationArtifact {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new("core-receipt-test").unwrap(),
        ConeRecord::new(
            scoop_identity::ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap()
}

fn compiler() -> PairedCompilerFingerprintV1 {
    PairedCompilerFingerprintV1::from_parts(
        sha256(b"executable"),
        sha256(b"distribution"),
        sha256(b"build"),
    )
}

fn receipt() -> TrustedCoreSlotReceiptV1 {
    let artifact = artifact();
    let source_key = CoreSourceSnapshotKeyV1::derive(ConeCompileCacheKeyV1::from_digest(sha256(
        b"core compile key",
    )))
    .unwrap();
    TrustedCoreSlotReceiptV1::new(
        TrustedCoreSlotReceiptBodyV1::new(
            source_key,
            artifact.artifact_fingerprint(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            compiler(),
            ArtifactCapabilityProfileId::single_cone_strong(),
            Vec::new(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn core_receipt_round_trips_with_independent_fingerprint() {
    let receipt = receipt();
    let bytes = encode(&receipt).unwrap();
    let (decoded, usage) =
        decode_trusted_core_slot_receipt_v1(&bytes, DecodeLimits::M23_DEFAULT).unwrap();

    assert_eq!(decoded, receipt);
    assert!(usage.validation_work_units > 0);
    assert_eq!(
        receipt.fingerprint().to_string(),
        "cd936abb3fc10ca191bbd188f35fae2f44c9f8b7d34a3e77e00c401225ce733d"
    );
}

#[test]
fn core_receipt_rejects_tampering_and_bounded_decode() {
    let mut bytes = encode(&receipt()).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    assert!(matches!(
        decode_trusted_core_slot_receipt_v1(&bytes, DecodeLimits::M23_DEFAULT),
        Err(TrustedCoreSlotReceiptDecodeError::FingerprintMismatch { .. })
    ));

    let bytes = encode(&receipt()).unwrap();
    let limits = DecodeLimits {
        owned_bytes: 8,
        ..DecodeLimits::M23_DEFAULT
    };
    assert!(matches!(
        decode_trusted_core_slot_receipt_v1(&bytes, limits),
        Err(TrustedCoreSlotReceiptDecodeError::Wire(_))
    ));
}
