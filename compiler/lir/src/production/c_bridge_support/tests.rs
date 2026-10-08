use scoop_wire::encode;

use super::*;
use crate::{
    AppleClangCompilerIdentityV1, DarwinBuildToolIdV1, DarwinBuildToolVersionContractV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
};

#[test]
fn support_contract_binds_target_and_complete_toolchain_profile() {
    let target = LirTargetProfile::DARWIN_AARCH64;
    let first_profile = profile("clang-2100.1.1.101");
    let second_profile = profile("clang-2100.1.1.102");
    let first = CBridgeTargetSupportRequirementV1::current(
        target,
        &first_profile,
        CBridgeTargetSupportV1::Memcpy,
    )
    .unwrap();
    let second = CBridgeTargetSupportRequirementV1::current(
        target,
        &second_profile,
        CBridgeTargetSupportV1::Memcpy,
    )
    .unwrap();

    assert_eq!(first.target(), &target.wire_id());
    assert_eq!(first.target_fingerprint(), target.fingerprint().unwrap());
    assert_eq!(first.profile_id(), first_profile.id());
    assert_eq!(first.profile_fingerprint(), first_profile.fingerprint());
    assert_eq!(first.support(), CBridgeTargetSupportV1::Memcpy);
    assert_eq!(first.object_symbol(target), b"_memcpy");
    assert_ne!(first.id(), second.id());
    assert_eq!(
        hex(&encode(&first).unwrap()),
        "a501a301781d6f72672e73636f6f702d6c616e672e7461726765742d70726f66696c65026e64617277696e2d61617263683634030102582079afcacae94bf05c0f1b38db6f43eb4f5668065903de37d1c2abf93551166c4203a30178296f72672e73636f6f702d6c616e672e632d6272696467652d746f6f6c636861696e2d70726f66696c6502781a64617277696e2d616172636836342d6170706c652d636c616e670301045820f2eb3baacb8d5a60ac01780667b81322134d07cb34773cb6d39e47a62a922d170501"
    );
}

#[test]
fn registry_is_a_closed_typed_lookup() {
    let profile = profile("clang-2100.1.1.101");
    let registry =
        CBridgeTargetSupportRegistryV1::current(LirTargetProfile::DARWIN_AARCH64, &profile)
            .unwrap();

    assert_eq!(registry.requirements().len(), 5);
    assert_eq!(
        registry
            .requirement_for_object_symbol(b"___error")
            .unwrap()
            .support(),
        CBridgeTargetSupportV1::DarwinErrno
    );
    assert_eq!(
        registry
            .requirement_for_object_symbol(b"__tlv_bootstrap")
            .unwrap()
            .support(),
        CBridgeTargetSupportV1::TlvBootstrap
    );
    assert_eq!(
        registry
            .requirement_for_object_symbol(b"_memcpy")
            .unwrap()
            .support(),
        CBridgeTargetSupportV1::Memcpy
    );
    assert!(
        registry
            .requirement_for_object_symbol(b"_memmove")
            .is_none()
    );
}

#[test]
fn linux_support_uses_its_tls_resolver_and_preserves_darwin_tags() {
    for target in [
        LirTargetProfile::LINUX_X86_64_GNU,
        LirTargetProfile::LINUX_X86_64_MUSL,
    ] {
        let compiler =
            crate::GccCompilerIdentityV1::new("15.2.0", scoop_wire::sha256(b"gcc inputs")).unwrap();
        let profile = CBridgeToolchainProfileV1::new_linux_gcc(target, compiler).unwrap();
        let registry = CBridgeTargetSupportRegistryV1::current(target, &profile).unwrap();
        assert_eq!(registry.requirements().len(), 5);
        assert_eq!(
            registry
                .requirement_for_object_symbol(b"__errno_location")
                .unwrap()
                .support(),
            CBridgeTargetSupportV1::LinuxErrno
        );
        assert_eq!(
            registry
                .requirement_for_object_symbol(b"__tls_get_addr")
                .unwrap()
                .support(),
            CBridgeTargetSupportV1::TlsGetAddr
        );
        assert!(
            registry
                .requirement_for_object_symbol(b"_tlv_bootstrap")
                .is_none()
        );
        assert!(matches!(
            CBridgeTargetSupportRequirementV1::current(
                target,
                &profile,
                CBridgeTargetSupportV1::TlvBootstrap
            ),
            Err(CBridgeTargetSupportRegistryError::UnsupportedTargetSupport { .. })
        ));
    }
    for (support, tag) in [
        (CBridgeTargetSupportV1::Memcpy, 1),
        (CBridgeTargetSupportV1::TlvBootstrap, 2),
        (CBridgeTargetSupportV1::TlsGetAddr, 3),
        (CBridgeTargetSupportV1::Fmodf, 4),
        (CBridgeTargetSupportV1::Fmod, 5),
        (CBridgeTargetSupportV1::DarwinErrno, 6),
        (CBridgeTargetSupportV1::LinuxErrno, 7),
    ] {
        assert_eq!(encode(&support).unwrap(), vec![tag]);
    }
}

fn profile(build: &str) -> CBridgeToolchainProfileV1 {
    let deployment = DarwinCBridgeDeploymentContractV1::new(
        DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
        DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
        vec![DarwinBuildToolVersionContractV1::new(
            DarwinBuildToolIdV1::Clang,
            DarwinPackedVersionV1::new(0x1000_0200).unwrap(),
        )],
    )
    .unwrap();
    CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
        deployment,
        AppleClangCompilerIdentityV1::new(21, 0, 0, build).unwrap(),
    )
    .unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
