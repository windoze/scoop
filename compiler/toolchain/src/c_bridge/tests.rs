use super::*;
use std::path::absolute;

#[cfg(target_os = "macos")]
#[test]
fn system_resolver_qualifies_the_actual_probe_object() {
    for target in [
        scoop_target_data::DARWIN_AARCH64,
        scoop_target_data::DARWIN_X86_64,
    ] {
        let (resolved, _) = probe_system_c_bridge_toolchain(target, None, None).unwrap();
        assert!(resolved.compiler_driver().is_file());
        assert!(resolved.sdk_root().expect("Darwin test toolchain").is_dir());
        assert_eq!(
            resolved.profile().contract().canonical_triple(),
            target.contract().canonical_triple()
        );
        let (explicit, _) = probe_system_c_bridge_toolchain(
            target,
            Some(resolved.compiler_driver()),
            Some(resolved.sdk_root().unwrap()),
        )
        .unwrap();
        assert_eq!(explicit.profile(), resolved.profile());
    }
}

#[test]
fn fake_host_facts_resolve_without_host_paths_entering_the_contract() {
    let resolved = resolve_c_bridge_toolchain(facts()).unwrap();
    assert_eq!(
        resolved.compiler_driver(),
        absolute("/toolchain/bin/clang").unwrap()
    );
    assert_eq!(
        resolved.sdk_root().expect("Darwin test toolchain"),
        absolute("/toolchain/SDKs/MacOSX.sdk").unwrap()
    );
    let contract = resolved.profile().contract();
    assert_eq!(contract.canonical_triple(), "aarch64-apple-darwin");
    assert_eq!(
        contract
            .deployment()
            .expect("Darwin test toolchain")
            .minimum_os()
            .components(),
        (15, 6, 2)
    );
    assert_eq!(
        contract
            .deployment()
            .expect("Darwin test toolchain")
            .sdk()
            .components(),
        (26, 5, 0)
    );
    assert!(
        contract
            .deployment()
            .expect("Darwin test toolchain")
            .tools()
            .is_empty()
    );
    assert_eq!(
        contract
            .compiler()
            .expect("Darwin test toolchain")
            .version_major(),
        21
    );
    assert_eq!(
        contract.compiler().expect("Darwin test toolchain").build(),
        "clang-2100.1.1.101"
    );
}

#[test]
fn validated_profile_builds_only_the_canonical_object_command() {
    let resolved = resolve_c_bridge_toolchain(facts()).unwrap();
    let sdk = absolute("/toolchain/SDKs/MacOSX.sdk").unwrap();
    let command = resolved.object_compilation_command(
        Path::new("/temporary/input.c"),
        Path::new("/temporary/output.o"),
    );
    assert_eq!(
        command.get_program(),
        absolute("/toolchain/bin/clang").unwrap()
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "-target",
            "aarch64-apple-darwin",
            "-isysroot",
            sdk.to_str().unwrap(),
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
            "-fno-omit-frame-pointer",
            "-mno-omit-leaf-frame-pointer",
            "-funwind-tables",
            "-fasynchronous-unwind-tables",
            "-fno-builtin",
        ]
    );
    assert_eq!(
        command.get_envs().collect::<Vec<_>>(),
        [
            (
                std::ffi::OsStr::new("LANG"),
                Some(std::ffi::OsStr::new("C"))
            ),
            (
                std::ffi::OsStr::new("LC_ALL"),
                Some(std::ffi::OsStr::new("C"))
            ),
            (
                std::ffi::OsStr::new("TMPDIR"),
                Some(std::ffi::OsStr::new("/temporary"))
            ),
            (
                std::ffi::OsStr::new("TZ"),
                Some(std::ffi::OsStr::new("UTC"))
            ),
        ]
    );
}

#[test]
fn resolver_rejects_relative_locators_and_non_apple_clang() {
    let mut relative = facts();
    relative.compiler_driver = PathBuf::from("clang");
    assert!(
        resolve_c_bridge_toolchain(relative)
            .unwrap_err()
            .0
            .contains("absolute")
    );

    let mut upstream = facts();
    upstream.compiler_version = "clang version 21.0.0".to_owned();
    assert!(
        resolve_c_bridge_toolchain(upstream)
            .unwrap_err()
            .0
            .contains("unsupported C bridge compiler")
    );
}

#[test]
fn version_parser_rejects_open_or_out_of_range_shapes() {
    assert_eq!(
        parse_darwin_version("26.5", "SDK").unwrap().components(),
        (26, 5, 0)
    );
    for spelling in ["26", "26.5.1.2", "26.x", "26.256"] {
        assert!(parse_darwin_version(spelling, "SDK").is_err(), "{spelling}");
    }
}

fn facts() -> CBridgeToolchainFacts {
    CBridgeToolchainFacts {
        target: scoop_target_data::DARWIN_AARCH64,
        compiler_driver: absolute("/toolchain/bin/clang").unwrap(),
        sdk_root: absolute("/toolchain/SDKs/MacOSX.sdk").unwrap(),
        sdk_version: "26.5".to_owned(),
        deployment_version: "15.6.2".to_owned(),
        compiler_version:
            "Apple clang version 21.0.0 (clang-2100.1.1.101)\nTarget: arm64-apple-darwin".to_owned(),
        tools: Vec::new(),
    }
}
