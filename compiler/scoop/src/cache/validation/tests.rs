use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate, NormalizedSourcePath};
use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::CanonicalMirFoundation;
use scoop_protocol::{
    DiagnosticOriginV1, DiagnosticSeverityV1, ProtocolByteSpan, ProtocolConeIdentity,
    StructuredDiagnosticV1,
};
use scoop_slib::{
    ConeKind, ConeSourceForm, IdentityFoundationArtifact, IdentityFoundationArtifactInput,
    ProducerRecord, probe_prebuilt_manifest_summary,
};
use scoop_wire::{DecodeLimits, sha256};

use super::*;

fn cone(coordinate: ConeCoordinate) -> ConeRecord {
    ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap()
}

fn foundation(producer: &str) -> IdentityFoundationArtifact {
    let hir = CanonicalHirFoundation::empty();
    let mir = CanonicalMirFoundation::empty();
    let lir = CanonicalLirFoundation::empty();
    IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
        ProducerRecord::new(producer).unwrap(),
        cone(ConeCoordinate::reserved_core()),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        &hir,
        &mir,
        &lir,
    ))
    .unwrap()
}

fn compiler(seed: &[u8]) -> PairedCompilerFingerprintV1 {
    PairedCompilerFingerprintV1::from_parts(sha256(seed), sha256(b"distribution"), sha256(b"build"))
}

fn binding<'a>(
    key: ConeCompileCacheKeyV1,
    artifact: ArtifactFingerprint,
    cone: ConeRecord,
    dependencies: &'a [DependencyRecord],
    compiler: PairedCompilerFingerprintV1,
    profile: &'a ArtifactCapabilityProfileId,
) -> ActualCacheBinding<'a> {
    ActualCacheBinding {
        key,
        artifact,
        cone,
        target: ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        direct_dependencies: dependencies,
        compiler,
        profile,
    }
}

#[test]
fn receipt_binding_checks_every_independent_authority_dimension() {
    let first = foundation("cache-binding-first");
    let second = foundation("cache-binding-second");
    let key = ConeCompileCacheKeyV1::from_digest(sha256(b"key"));
    let current_compiler = compiler(b"compiler");
    let current_cone = cone(ConeCoordinate::reserved_core());
    let profile = ArtifactCapabilityProfileId::cross_cone_semantics_strong();
    let receipt = CacheReceiptBodyV1::new(
        key,
        first.artifact_fingerprint(),
        current_cone.clone(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        Vec::new(),
        current_compiler,
        profile.clone(),
        Vec::new(),
    )
    .unwrap();
    assert!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                first.artifact_fingerprint(),
                current_cone.clone(),
                &[],
                current_compiler,
                &profile,
            ),
        )
        .is_ok()
    );

    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                second.artifact_fingerprint(),
                current_cone.clone(),
                &[],
                current_compiler,
                &profile,
            ),
        ),
        Err(CacheReceiptBindingError::ArtifactFingerprint)
    );
    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                first.artifact_fingerprint(),
                cone(ConeCoordinate::new("test", "other", "1.0.0").unwrap()),
                &[],
                current_compiler,
                &profile,
            ),
        ),
        Err(CacheReceiptBindingError::Cone)
    );
    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                first.artifact_fingerprint(),
                current_cone.clone(),
                &[],
                compiler(b"other-compiler"),
                &profile,
            ),
        ),
        Err(CacheReceiptBindingError::Compiler)
    );
    let identity_profile = ArtifactCapabilityProfileId::identity_foundation();
    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                first.artifact_fingerprint(),
                current_cone,
                &[],
                current_compiler,
                &identity_profile,
            ),
        ),
        Err(CacheReceiptBindingError::ArtifactProfile)
    );
}

#[test]
fn receipt_binding_rejects_dependency_and_key_drift() {
    let artifact = foundation("cache-binding-dependency");
    let summary = probe_prebuilt_manifest_summary(
        artifact.as_bytes(),
        DecodeLimits::M23_DEFAULT,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let dependency = DependencyRecord::new(
        ConeCoordinate::reserved_core(),
        summary.semantic_fingerprints().hir(),
        summary.semantic_fingerprints().mir(),
        summary.semantic_fingerprints().lir(),
    )
    .unwrap();
    let other_dependency = DependencyRecord::new(
        ConeCoordinate::new("test", "dependency", "1.0.0").unwrap(),
        summary.semantic_fingerprints().hir(),
        summary.semantic_fingerprints().mir(),
        summary.semantic_fingerprints().lir(),
    )
    .unwrap();
    let key = ConeCompileCacheKeyV1::from_digest(sha256(b"key"));
    let compiler = compiler(b"compiler");
    let cone = cone(ConeCoordinate::reserved_core());
    let profile = ArtifactCapabilityProfileId::cross_cone_semantics_strong();
    let receipt = CacheReceiptBodyV1::new(
        key,
        artifact.artifact_fingerprint(),
        cone.clone(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        vec![dependency.clone(), other_dependency.clone()],
        compiler,
        profile.clone(),
        Vec::new(),
    )
    .unwrap();

    let actual_dependencies = [other_dependency.clone(), dependency.clone()];
    assert_ne!(
        receipt.direct_dependencies(),
        actual_dependencies.as_slice()
    );
    assert!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                artifact.artifact_fingerprint(),
                cone.clone(),
                &actual_dependencies,
                compiler,
                &profile
            ),
        )
        .is_ok()
    );
    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                artifact.artifact_fingerprint(),
                cone.clone(),
                &[other_dependency.clone(), other_dependency],
                compiler,
                &profile
            ),
        ),
        Err(CacheReceiptBindingError::DirectDependencies)
    );

    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                artifact.artifact_fingerprint(),
                cone.clone(),
                &[],
                compiler,
                &profile,
            ),
        ),
        Err(CacheReceiptBindingError::DirectDependencies)
    );
    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                ConeCompileCacheKeyV1::from_digest(sha256(b"other-key")),
                artifact.artifact_fingerprint(),
                cone,
                receipt.direct_dependencies(),
                compiler,
                &profile,
            ),
        ),
        Err(CacheReceiptBindingError::CacheKey)
    );
}

#[test]
fn cached_warning_origins_must_belong_to_the_validated_closure() {
    let core_warning = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Warning,
        "SCOOPC_CORE_WARNING".to_owned(),
        "core warning".to_owned(),
        DiagnosticOriginV1::SemanticSourceSpan {
            cone: ProtocolConeIdentity::from_array(*ConeIdentity::CORE.as_array()),
            logical_path: NormalizedSourcePath::new("src/core.scoop").unwrap(),
            span: ProtocolByteSpan::new(0, 1).unwrap(),
        },
        Vec::new(),
    )
    .unwrap();
    assert!(
        validate_warning_origins(std::slice::from_ref(&core_warning), &[ConeIdentity::CORE])
            .is_ok()
    );

    let foreign = ConeCoordinate::new("test", "foreign", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let foreign_warning = StructuredDiagnosticV1::new(
        DiagnosticSeverityV1::Warning,
        "SCOOPC_FOREIGN_WARNING".to_owned(),
        "foreign warning".to_owned(),
        DiagnosticOriginV1::SemanticSourceSpan {
            cone: ProtocolConeIdentity::from_array(*foreign.as_array()),
            logical_path: NormalizedSourcePath::new("src/foreign.scoop").unwrap(),
            span: ProtocolByteSpan::new(0, 1).unwrap(),
        },
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        validate_warning_origins(&[foreign_warning], &[ConeIdentity::CORE]),
        Err(CacheCompletionError::WarningOriginOutsideClosure { cone, .. })
            if cone.as_array() == foreign.as_array()
    ));
}
