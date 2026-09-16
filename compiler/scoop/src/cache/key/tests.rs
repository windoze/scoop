use scoop_identity::{ConeIdentity, NormalizedSourcePath};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeToolchainProfileV1, DarwinCBridgeDeploymentContractV1,
    DarwinPackedVersionV1, ValidatedLirTargetSelection,
};
use scoop_protocol::ScoopcProtocolCapabilityV1;
use scoop_slib::{ConeKind, ConeRecord, ConeSourceForm, IdentityAbiDescriptor};
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
        CurrentConeSemanticProjectionV1::TrustedCoreBootstrap,
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

#[test]
fn compile_cache_key_has_a_fixed_canonical_vector() {
    let input = input("class Any\n", "paired-scoopc-v1");

    assert_eq!(
        input.key().unwrap().to_string(),
        "687a9db05ac5486a7eee880f9e5545e82ded339d71c2d22e520e7ddb3070cea8"
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
