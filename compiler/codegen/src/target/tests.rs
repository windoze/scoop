use super::*;

#[test]
fn linked_backend_is_llvm_22_1() {
    let version = validate_linked_llvm().expect("LLVM 22.1");
    assert_eq!((version.major, version.minor), (22, 1));
}

#[test]
fn another_llvm_minor_is_rejected_structurally() {
    let error = validate_llvm_version(LlvmVersion {
        major: 23,
        minor: 1,
        patch: 0,
    })
    .expect_err("LLVM 23 must be rejected");
    assert_eq!(
        error.0,
        "unsupported LLVM backend 23.1.0; Scoop requires LLVM 22.1"
    );
}

#[test]
fn selected_profile_creates_the_canonical_aarch64_machine() {
    let backend = ValidatedBackendProfile::from_selection(
        scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
    )
    .expect("qualified backend selection");
    assert_eq!(
        backend.backend_profile(),
        scoop_lir::BackendProfile::LLVM_22_1
    );
    assert_eq!(backend.managed_address_space(), 1);
    assert_eq!(backend.stack_map_version(), 3);
    assert_eq!(
        backend.eh_profile(),
        EhProfile {
            unwind_model: UnwindModel::ItaniumDwarf,
            personality_abi: PersonalityAbi::ScoopLsdaSubset,
            exception_data_registers: 2,
            encodings: LsdaEncodingProfile {
                lp_start: 0xff,
                type_table: 0x9b,
                call_site: 0x01,
            },
            unwind_provider: UnwindProvider::DarwinLibSystem,
            artifact_inspection: EhArtifactInspection::MachO,
        }
    );
    let machine = backend.create_target_machine().expect("target machine");
    assert_eq!(
        machine
            .get_triple()
            .as_str()
            .to_str()
            .expect("UTF-8 triple"),
        backend.canonical_triple()
    );
}

#[test]
fn native_c_pointer_representations_are_qualified() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("codegen crate is nested below the workspace root");

    let profile_source = workspace.join(crate::tests::platform_support::native_profile_source());
    let profile = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg(&profile_source)
        .output()
        .expect("compile native runtime profile qualification");
    assert!(
        profile.status.success(),
        "native runtime target assertions must compile cleanly:\n{}",
        String::from_utf8_lossy(&profile.stderr)
    );

    let binary =
        std::env::temp_dir().join(format!("scoop_target_qualification_{}", std::process::id()));
    let compile = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(workspace.join("runtime/tests/target_qualification_test.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .expect("compile target pointer qualification test");
    assert!(
        compile.status.success(),
        "target pointer qualification test must compile cleanly:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = std::process::Command::new(&binary)
        .output()
        .expect("run target pointer qualification test");
    std::fs::remove_file(&binary).ok();
    assert!(
        output.status.success(),
        "target pointer qualification test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(
        output.stdout,
        b"target pointer qualification tests passed\n"
    );
}
