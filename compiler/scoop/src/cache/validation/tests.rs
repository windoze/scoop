use scoop_identity::{ArtifactCapabilityProfileId, ConeCoordinate, NormalizedSourcePath};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_protocol::{
    DiagnosticOriginV1, DiagnosticSeverityV1, ProtocolByteSpan, ProtocolConeIdentity,
    StructuredDiagnosticV1,
};
use scoop_slib::{ConeKind, ConeSourceForm, read_artifact_manifest_summary};
use scoop_wire::sha256;

use super::*;

fn cone(coordinate: ConeCoordinate) -> ConeRecord {
    ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap()
}

fn artifact_summary(producer: &str) -> scoop_slib::ArtifactManifestSummaryV1 {
    let archive = crate::test_artifacts::manifest_archive(
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        cone(ConeCoordinate::reserved_core()),
        producer,
        Vec::new(),
    );
    read_artifact_manifest_summary(
        archive.as_bytes(),
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap()
}

fn compiler(seed: &[u8]) -> PairedCompilerFingerprintV1 {
    PairedCompilerFingerprintV1::from_parts(sha256(seed), sha256(b"distribution"), sha256(b"build"))
}

fn binding<'a>(
    key: ConeCompileCacheKeyV1,
    artifact: ArtifactFingerprint,
    cone: &'a ConeRecord,
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
fn receipt_binding_checks_cache_metadata() {
    let first = artifact_summary("cache-binding-first");
    let second = artifact_summary("cache-binding-second");
    let key = ConeCompileCacheKeyV1::from_digest(sha256(b"key"));
    let current_compiler = compiler(b"compiler");
    let current_cone = cone(ConeCoordinate::reserved_core());
    let profile = ArtifactCapabilityProfileId::cross_cone_generic();
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
                &current_cone,
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
                &current_cone,
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
                &cone(ConeCoordinate::new("test", "other", "1.0.0").unwrap()),
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
                &current_cone,
                &[],
                compiler(b"other-compiler"),
                &profile,
            ),
        ),
        Err(CacheReceiptBindingError::Compiler)
    );
    let other_profile = ArtifactCapabilityProfileId::single_cone_strong();
    assert_eq!(
        validate_receipt_binding(
            &receipt,
            &binding(
                key,
                first.artifact_fingerprint(),
                &current_cone,
                &[],
                current_compiler,
                &other_profile,
            ),
        ),
        Err(CacheReceiptBindingError::ArtifactProfile)
    );
}

#[test]
fn receipt_binding_rejects_dependency_and_key_drift() {
    let artifact = artifact_summary("cache-binding-dependency");
    let dependency = DependencyRecord::new(
        ConeCoordinate::reserved_core(),
        artifact.semantic_fingerprints().hir(),
        artifact.semantic_fingerprints().mir(),
        artifact.semantic_fingerprints().lir(),
    )
    .unwrap();
    let other_dependency = DependencyRecord::new(
        ConeCoordinate::new("test", "dependency", "1.0.0").unwrap(),
        artifact.semantic_fingerprints().hir(),
        artifact.semantic_fingerprints().mir(),
        artifact.semantic_fingerprints().lir(),
    )
    .unwrap();
    let key = ConeCompileCacheKeyV1::from_digest(sha256(b"key"));
    let compiler = compiler(b"compiler");
    let cone = cone(ConeCoordinate::reserved_core());
    let profile = ArtifactCapabilityProfileId::cross_cone_generic();
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
                &cone,
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
                &cone,
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
                &cone,
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
                &cone,
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
