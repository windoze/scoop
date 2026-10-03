use scoop_identity::{ConeIdentity, NormalizedSourcePath};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeToolchainFingerprint, CBridgeToolchainProfileV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, ValidatedLirTargetSelection,
};
use scoop_protocol::ScoopcProtocolCapabilityV1;
use scoop_slib::{
    ArtifactManifestSummaryV1, ConeKind, ConeRecord, ConeSourceForm, DependencyRecord,
    IdentityAbiDescriptor, read_artifact_manifest_summary,
};
use scoop_wire::{encode, sha256};

use super::*;

fn input(source_text: &str, compiler_executable: &str) -> ConeCompileCacheInputV1 {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let c_bridge = CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
        DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
            DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
            Vec::new(),
        )
        .unwrap(),
        AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap(),
    )
    .unwrap();
    let source = SourceIdentity::new(
        ConeIdentity::CORE,
        NormalizedSourcePath::new("src/core.scoop").unwrap(),
    )
    .unwrap();
    ConeCompileCacheInputV1::new(
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        CurrentConeSemanticProjectionV1::Manifest {
            coordinate: ConeCoordinate::reserved_core(),
            requested_kind: RequestedConeKind::Library,
            dependencies: Vec::new(),
        },
        vec![SourceCacheInputV1::new(
            source,
            SourceContentDigest::from_utf8(source_text),
        )],
        Vec::new(),
        PairedCompilerFingerprintV1::from_parts(
            sha256(compiler_executable.as_bytes()),
            sha256(b"distribution"),
            sha256(b"build"),
        ),
        IdentityAbiDescriptor::current().unwrap(),
        strong_profile_id(),
        ScoopcProtocolCapabilityV1::current(),
        selection.target().fingerprint().unwrap(),
        selection.backend().fingerprint().unwrap(),
        c_bridge.fingerprint(),
    )
}

fn c_bridge_fingerprint(minimum: u32, sdk: u32, compiler: &str) -> CBridgeToolchainFingerprint {
    CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
        DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::new(minimum).unwrap(),
            DarwinPackedVersionV1::new(sdk).unwrap(),
            Vec::new(),
        )
        .unwrap(),
        AppleClangCompilerIdentityV1::new(21, 0, 0, compiler).unwrap(),
    )
    .unwrap()
    .fingerprint()
}

fn dependency_summaries() -> (ArtifactManifestSummaryV1, ArtifactManifestSummaryV1) {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let core = crate::test_artifacts::manifest_archive(
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "cache-key-core",
        Vec::new(),
    );
    let core = read_artifact_manifest_summary(core.as_bytes(), selection).unwrap();
    let semantic = core.semantic_fingerprints();
    let dependency = crate::test_artifacts::manifest_archive(
        scoop_slib::ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(
            ConeCoordinate::new("dev.example", "dependency", "1.0.0").unwrap(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        "cache-key-dependency",
        vec![
            DependencyRecord::new(
                core.cone().coordinate().clone(),
                semantic.hir(),
                semantic.mir(),
                semantic.lir(),
            )
            .unwrap(),
        ],
    );
    let dependency = read_artifact_manifest_summary(dependency.as_bytes(), selection).unwrap();
    (core, dependency)
}

fn rich_input() -> ConeCompileCacheInputV1 {
    let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
    let (core, dependency) = dependency_summaries();
    let current_coordinate = ConeCoordinate::new("dev.example", "current", "1.0.0").unwrap();
    let current_identity = current_coordinate.identity().unwrap();
    let source = SourceIdentity::new(
        current_identity,
        NormalizedSourcePath::new("src/main.scoop").unwrap(),
    )
    .unwrap();
    let semantic = core.semantic_fingerprints();
    ConeCompileCacheInputV1::new(
        ConeRecord::new(
            current_coordinate.clone(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        CurrentConeSemanticProjectionV1::Manifest {
            coordinate: current_coordinate,
            requested_kind: RequestedConeKind::Library,
            dependencies: vec![dependency.cone().coordinate().clone()],
        },
        vec![SourceCacheInputV1::new(
            source,
            SourceContentDigest::from_utf8("fun answer(): Long = 42\n"),
        )],
        vec![CompileDependencyInputV1::new(
            core.cone().clone(),
            semantic.hir(),
            semantic.mir(),
            semantic.lir(),
            OptionalCoreCodeFingerprintV1::None,
        )],
        PairedCompilerFingerprintV1::from_parts(
            sha256(b"paired-scoopc-v1"),
            sha256(b"distribution-v1"),
            sha256(b"build-v1"),
        ),
        IdentityAbiDescriptor::current().unwrap(),
        strong_profile_id(),
        ScoopcProtocolCapabilityV1::current(),
        selection.target().fingerprint().unwrap(),
        selection.backend().fingerprint().unwrap(),
        c_bridge_fingerprint(0x000d_0100, 0x000e_0200, "clang-2100.1.1.101"),
    )
}

fn assert_mutation_misses(
    baseline: &ConeCompileCacheInputV1,
    mutate: impl FnOnce(&mut ConeCompileCacheInputV1),
) {
    let baseline_key = baseline.key().unwrap();
    let mut changed = baseline.clone();
    mutate(&mut changed);
    assert_ne!(baseline_key, changed.key().unwrap());
}

#[test]
fn compile_cache_key_has_a_fixed_canonical_vector() {
    let input = input("class Any\n", "paired-scoopc-v1");

    assert_eq!(
        input.key().unwrap().to_string(),
        "1ec9749482dcafbb89ec09eb1116a962b21dc230f7c4c10c19a0cacae80ef523"
    );
    assert_eq!(encode(&input).unwrap().first(), Some(&0xac));
}

#[test]
fn source_and_compiler_content_are_independent_key_dimensions() {
    let baseline = input("class Any\n", "paired-scoopc-v1").key().unwrap();
    let source_changed = input("class Any { }\n", "paired-scoopc-v1").key().unwrap();
    let compiler_changed = input("class Any\n", "paired-scoopc-v2").key().unwrap();

    assert_ne!(baseline, source_changed);
    assert_ne!(baseline, compiler_changed);
    assert_ne!(source_changed, compiler_changed);
}

#[test]
fn absent_optional_core_code_has_the_frozen_wire_shape() {
    assert_eq!(
        encode(&OptionalCoreCodeFingerprintV1::None).unwrap(),
        [0xa1, 0x00, 0x01]
    );
}

#[test]
fn every_currently_variable_cache_key_dimension_misses_independently() {
    let baseline = rich_input();
    let (_, alternate_dependency) = dependency_summaries();
    let alternate_semantic = alternate_dependency.semantic_fingerprints();

    assert_mutation_misses(&baseline, |input| {
        input.cone = ConeRecord::new(
            ConeCoordinate::new("dev.example", "other", "1.0.0").unwrap(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap();
    });
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { coordinate, .. } =
            &mut input.current_semantic
        else {
            unreachable!()
        };
        *coordinate = ConeCoordinate::new("dev.example", "current", "2.0.0").unwrap();
    });
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { requested_kind, .. } =
            &mut input.current_semantic
        else {
            unreachable!()
        };
        *requested_kind = RequestedConeKind::Executable;
    });
    assert_mutation_misses(&baseline, |input| {
        let CurrentConeSemanticProjectionV1::Manifest { dependencies, .. } =
            &mut input.current_semantic
        else {
            unreachable!()
        };
        dependencies.clear();
    });
    assert_mutation_misses(&baseline, |input| {
        input.current_semantic = CurrentConeSemanticProjectionV1::SingleFile;
    });
    assert_mutation_misses(&baseline, |input| {
        input.sources[0].source = SourceIdentity::new(
            input.cone.identity(),
            NormalizedSourcePath::new("src/other.scoop").unwrap(),
        )
        .unwrap();
    });
    assert_mutation_misses(&baseline, |input| {
        input.sources[0].content = SourceContentDigest::from_utf8("fun answer(): Long = 43\n");
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].cone = alternate_dependency.cone().clone();
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].hir = alternate_semantic.hir();
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].mir = alternate_semantic.mir();
    });
    assert_mutation_misses(&baseline, |input| {
        input.dependencies[0].lir = alternate_semantic.lir();
    });
    assert_mutation_misses(&baseline, |input| {
        input.compiler = PairedCompilerFingerprintV1::from_parts(
            sha256(b"paired-scoopc-v2"),
            sha256(b"distribution-v1"),
            sha256(b"build-v1"),
        );
    });
    assert_mutation_misses(&baseline, |input| {
        input.artifact_profile = ArtifactCapabilityProfileId::single_cone_strong();
    });
    assert_mutation_misses(&baseline, |input| {
        input.c_bridge_toolchain =
            c_bridge_fingerprint(0x000d_0200, 0x000e_0200, "clang-2100.1.1.101");
    });
}
