use scoop_wire::encode;

use super::*;

#[test]
fn darwin_versions_pack_and_render_canonically() {
    let version = DarwinPackedVersionV1::from_components(26, 5, 2).unwrap();
    assert_eq!(version.packed(), 0x001a_0502);
    assert_eq!(version.components(), (26, 5, 2));
    assert_eq!(version.to_string(), "26.5.2");
    assert_eq!(
        DarwinPackedVersionV1::from_components(1, 256, 0),
        Err(DarwinPackedVersionError::MinorOutOfRange(256))
    );
}

#[test]
fn deployment_accepts_empty_tools_and_rejects_missing_or_noncanonical_clang() {
    let version = DarwinPackedVersionV1::new(1).unwrap();
    assert!(DarwinCBridgeDeploymentContractV1::new(version, version, Vec::new()).is_ok());
    assert_eq!(
        DarwinCBridgeDeploymentContractV1::new(
            version,
            version,
            vec![DarwinBuildToolVersionContractV1::new(
                DarwinBuildToolIdV1::Ld,
                version,
            )],
        ),
        Err(DarwinCBridgeDeploymentContractError::MissingClang)
    );
    assert_eq!(
        DarwinCBridgeDeploymentContractV1::new(
            version,
            version,
            vec![
                DarwinBuildToolVersionContractV1::new(DarwinBuildToolIdV1::Ld, version),
                DarwinBuildToolVersionContractV1::new(DarwinBuildToolIdV1::Clang, version),
            ],
        ),
        Err(DarwinCBridgeDeploymentContractError::NonCanonicalToolOrder { index: 1 })
    );
}

#[test]
fn compiler_identity_rejects_ambiguous_build_spelling() {
    assert_eq!(
        AppleClangCompilerIdentityV1::new(0, 1, 2, "clang-1"),
        Err(AppleClangCompilerIdentityError::ZeroMajorVersion)
    );
    assert_eq!(
        AppleClangCompilerIdentityV1::new(21, 0, 0, "clang build"),
        Err(AppleClangCompilerIdentityError::InvalidBuildCharacter)
    );
}

#[test]
fn c_bridge_toolchain_contract_and_fingerprints_match_fixed_vectors() {
    let profile = fixture_profile();
    assert_eq!(
        hex(&encode(profile.contract()).unwrap()),
        "a801a301781d6f72672e73636f6f702d6c616e672e7461726765742d70726f66696c65026e64617277696e2d61617263683634030102582042697b4e4e2102ef19d81bd624f7e428065d2ddff90279f1fc458bfcfdb136710374616172636836342d6170706c652d64617277696e04a40101021a000d0100031a000e02000481a20101021a1000020005a40115020003000472636c616e672d323130302e312e312e3130310658209df6ab7082de152b84b4e7ec57a4b5fc3979f821e8cac0ea533d671a8b8964a2075820368cd9a023dad9b6f085cb8ebbd5f44d8e20ecbb24c98e45d6244fa63f3512a908a301800261430363555443"
    );
    assert_eq!(
        CanonicalCBridgeFlagContractV1::CURRENT
            .fingerprint()
            .unwrap()
            .to_string(),
        "9df6ab7082de152b84b4e7ec57a4b5fc3979f821e8cac0ea533d671a8b8964a2"
    );
    assert_eq!(
        GeneratedCSourceTemplateContractV1::CURRENT
            .fingerprint()
            .unwrap()
            .to_string(),
        "368cd9a023dad9b6f085cb8ebbd5f44d8e20ecbb24c98e45d6244fa63f3512a9"
    );
    assert_eq!(
        profile.fingerprint().to_string(),
        "90ee0173d0bb4f75c63d04a95c59b800fa005906e545fc245f0f373a546555d5"
    );
}

#[test]
fn profile_fingerprint_commits_deployment_sdk_tool_and_compiler_identity() {
    let baseline = fixture_profile().fingerprint();
    for changed in [
        fixture_profile_with(
            0x000d_0200,
            0x000e_0200,
            0x1000_0200,
            21,
            "clang-2100.1.1.101",
        ),
        fixture_profile_with(
            0x000d_0100,
            0x000e_0300,
            0x1000_0200,
            21,
            "clang-2100.1.1.101",
        ),
        fixture_profile_with(
            0x000d_0100,
            0x000e_0200,
            0x1000_0300,
            21,
            "clang-2100.1.1.101",
        ),
        fixture_profile_with(
            0x000d_0100,
            0x000e_0200,
            0x1000_0200,
            22,
            "clang-2200.1.1.101",
        ),
    ] {
        assert_ne!(changed.fingerprint(), baseline);
    }
}

fn fixture_profile() -> CBridgeToolchainProfileV1 {
    fixture_profile_with(
        0x000d_0100,
        0x000e_0200,
        0x1000_0200,
        21,
        "clang-2100.1.1.101",
    )
}

fn fixture_profile_with(
    minimum_os: u32,
    sdk: u32,
    tool_version: u32,
    compiler_major: u32,
    compiler_build: &str,
) -> CBridgeToolchainProfileV1 {
    let deployment = DarwinCBridgeDeploymentContractV1::new(
        DarwinPackedVersionV1::new(minimum_os).unwrap(),
        DarwinPackedVersionV1::new(sdk).unwrap(),
        vec![DarwinBuildToolVersionContractV1::new(
            DarwinBuildToolIdV1::Clang,
            DarwinPackedVersionV1::new(tool_version).unwrap(),
        )],
    )
    .unwrap();
    let compiler = AppleClangCompilerIdentityV1::new(compiler_major, 0, 0, compiler_build).unwrap();
    CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(deployment, compiler).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
