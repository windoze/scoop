use super::*;
use scoop_lir::{
    AppleClangCompilerIdentityV1, DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
};

#[test]
fn invocation_builds_only_the_canonical_command() {
    let profile = profile();
    let invocation = ValidatedCBridgeToolchainInvocation::new(
        scoop_target_data::DARWIN_AARCH64,
        profile,
        PathBuf::from("/toolchain/bin/clang"),
        PathBuf::from("/toolchain/SDKs/MacOSX.sdk"),
    )
    .unwrap();
    let command = invocation.object_compilation_command(
        Path::new("/temporary/input.c"),
        Path::new("/temporary/output.o"),
    );
    assert_eq!(command.get_program(), "/toolchain/bin/clang");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "-target",
            "aarch64-apple-darwin",
            "-isysroot",
            "/toolchain/SDKs/MacOSX.sdk",
            "-mmacosx-version-min=15.6.2",
            "-std=c11",
            "-c",
            "/temporary/input.c",
            "-o",
            "/temporary/output.o",
            "-O0",
            "-g0",
            "-fno-common",
            "-fno-ident",
            "-fno-stack-protector",
            "-fno-unwind-tables",
            "-fno-asynchronous-unwind-tables",
            "-fno-builtin",
        ]
    );
    let release = invocation
        .clone()
        .with_optimization(scoop_lir::OptimizationMode::Release);
    assert_eq!(release.profile(), invocation.profile());
    let command = release.object_compilation_command(
        Path::new("/temporary/input.c"),
        Path::new("/temporary/output.o"),
    );
    let args = command.get_args().collect::<Vec<_>>();
    assert!(args.contains(&OsStr::new("-O2")));
    assert!(!args.contains(&OsStr::new("-O0")));
    assert!(args.contains(&OsStr::new("-fno-builtin")));
}

#[test]
fn invocation_rejects_relative_host_locators() {
    let error = ValidatedCBridgeToolchainInvocation::new(
        scoop_target_data::DARWIN_AARCH64,
        profile(),
        PathBuf::from("clang"),
        PathBuf::from("/toolchain/SDKs/MacOSX.sdk"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CBridgeToolchainInvocationError::RelativeCompilerDriver
    ));
}

fn profile() -> CBridgeToolchainProfileV1 {
    let deployment = DarwinCBridgeDeploymentContractV1::new(
        DarwinPackedVersionV1::from_components(15, 6, 2).unwrap(),
        DarwinPackedVersionV1::from_components(26, 5, 0).unwrap(),
        Vec::new(),
    )
    .unwrap();
    let compiler = AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap();
    scoop_target_data::darwin_c_bridge_profile(deployment, compiler).unwrap()
}

#[test]
fn producer_selection_rejects_unknown_version_and_mismatched_profile() {
    let original = profile();
    for (name, version) in [
        ("unknown-producer", 1),
        ("darwin-aarch64-apple-clang", 2),
        ("linux-x86-64-gnu-gcc", 1),
    ] {
        let profile = CBridgeToolchainProfileV1::new(
            scoop_identity::CBridgeToolchainProfileId::new(name, version).unwrap(),
            original.contract().clone(),
        )
        .unwrap();
        assert!(matches!(
            ValidatedCBridgeToolchainInvocation::new(
                scoop_target_data::DARWIN_AARCH64,
                profile,
                PathBuf::from("/toolchain/clang"),
                PathBuf::from("/toolchain/sdk"),
            ),
            Err(CBridgeToolchainInvocationError::UnsupportedProfile)
        ));
    }
}
