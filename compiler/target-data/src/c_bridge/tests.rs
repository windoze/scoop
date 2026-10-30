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
fn linux_c_profile_separates_libc_and_tracks_consumed_inputs() {
    let identity =
        |bytes: &[u8]| GccCompilerIdentityV1::new("15.2.0", scoop_wire::sha256(bytes)).unwrap();
    let gnu =
        crate::linux_c_bridge_profile(crate::LINUX_X86_64_GNU, identity(b"compiler and headers"))
            .unwrap();
    let musl =
        crate::linux_c_bridge_profile(crate::LINUX_X86_64_MUSL, identity(b"compiler and headers"))
            .unwrap();
    let changed = crate::linux_c_bridge_profile(
        crate::LINUX_X86_64_GNU,
        identity(b"changed specs or headers"),
    )
    .unwrap();
    assert_ne!(gnu.fingerprint(), musl.fingerprint());
    assert_ne!(gnu.fingerprint(), changed.fingerprint());
    assert!(gnu.contract().deployment().is_err());
    assert!(
        crate::linux_c_bridge_profile(crate::DARWIN_AARCH64, identity(b"compiler and headers"),)
            .is_err()
    );
    for version in ["0", "+15", "15.", "15.x", "15.2.0.1"] {
        assert!(GccCompilerIdentityV1::new(version, scoop_wire::sha256(b"")).is_err());
    }
}

#[test]
fn c_bridge_toolchain_contract_and_fingerprints_match_fixed_vectors() {
    let profile = fixture_profile();
    assert_eq!(
        hex(&encode(profile.contract()).unwrap()),
        "a801a301781d6f72672e73636f6f702d6c616e672e7461726765742d70726f66696c65026e64617277696e2d6161726368363403010258207902159fd4c00dcb4c11524752a684b911a1657e42e56fd734f808613c6152770374616172636836342d6170706c652d64617277696e04a40101021a000d0100031a000e02000481a20101021a1000020005a40115020003000472636c616e672d323130302e312e312e313031065820697290aeee34a7c9c2ef092c7a1fadcf6aaeabddd7f7026aa18a0efa78f8092d0758208f314440b379f395521bfef94a78be5baf19a3cc62add05363b83d3ea242b1fd08a301800261430363555443"
    );
    assert_eq!(
        CanonicalCBridgeFlagContractV1::CURRENT
            .fingerprint()
            .unwrap()
            .to_string(),
        "697290aeee34a7c9c2ef092c7a1fadcf6aaeabddd7f7026aa18a0efa78f8092d"
    );
    assert_eq!(
        GeneratedCSourceTemplateContractV1::CURRENT
            .fingerprint()
            .unwrap()
            .to_string(),
        "8f314440b379f395521bfef94a78be5baf19a3cc62add05363b83d3ea242b1fd"
    );
    assert_eq!(
        profile.fingerprint().to_string(),
        "0bdd7de63765be8cecd6b72d88a5d8cb3c12d2f937780c6677ae2ac38ad8e38d"
    );
}

#[test]
fn windows_c_profiles_separate_environments_and_preserve_seh_flags() {
    let compiler =
        WindowsClangCompilerIdentityV1::new([22, 1, 8], scoop_wire::sha256(b"selected headers"));
    let profiles = [
        crate::WINDOWS_X86_64_MSVC_UCRT,
        crate::WINDOWS_X86_64_MINGW_MSVCRT,
        crate::WINDOWS_X86_64_MINGW_UCRT,
    ]
    .map(|target| crate::windows_c_bridge_profile(target, compiler.clone()).unwrap());
    let fingerprints = profiles
        .iter()
        .map(|profile| profile.fingerprint().to_string())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(fingerprints.len(), 3);
    for profile in profiles {
        assert!(profile.contract().deployment().is_err());
    }
    assert!(crate::windows_c_bridge_profile(crate::LINUX_X86_64_GNU, compiler).is_err());
    let flags = CanonicalCBridgeFlagContractV1::WindowsClang.flags();
    assert!(flags.contains(&CanonicalCBridgeFlagV1::WindowsSeh));
    assert!(flags.contains(&CanonicalCBridgeFlagV1::ReproducibleCoff));
    assert!(flags.contains(&CanonicalCBridgeFlagV1::UnwindTables));
    assert!(flags.contains(&CanonicalCBridgeFlagV1::AsynchronousUnwindTables));
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
    crate::darwin_c_bridge_profile(crate::DARWIN_AARCH64, deployment, compiler).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
