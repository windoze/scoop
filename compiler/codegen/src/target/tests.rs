use super::*;
use std::path::Path;

mod loop_alignment;

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
    let backend =
        ValidatedBackendProfile::for_selection(scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
            .expect("qualified backend selection");
    assert_eq!(backend.backend_profile(), scoop_target_data::LLVM_22_1);
    assert_eq!(backend.managed_address_space(), 1);
    let machine = backend.create_target_machine().expect("target machine");
    assert_eq!(
        machine
            .get_triple()
            .as_str()
            .to_str()
            .expect("UTF-8 triple"),
        "aarch64-apple-macosx11.0.0"
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

#[test]
fn unregistered_backend_identity_or_machine_configuration_is_rejected() {
    use scoop_identity::BackendProfileId;
    use scoop_lir::BackendProfileContract;
    const UNKNOWN_ID: BackendProfileId = match BackendProfileId::new("unknown-backend", 1) {
        Ok(id) => id,
        Err(_) => panic!("canonical test identity"),
    };
    const KNOWN_ID: BackendProfileId = match BackendProfileId::new("llvm-22-1", 1) {
        Ok(id) => id,
        Err(_) => panic!("canonical test identity"),
    };
    const DIFFERENT_CPU: BackendProfileContract = BackendProfileContract {
        cpu: "other-cpu",
        ..scoop_target_data::LLVM_22_1.contract()
    };
    const ORIGINAL: BackendProfileContract = scoop_target_data::LLVM_22_1.contract();
    for backend in [
        BackendProfile::new(&UNKNOWN_ID, &ORIGINAL),
        BackendProfile::new(&KNOWN_ID, &DIFFERENT_CPU),
        scoop_target_data::LLVM_22_1_LINUX_X86_64,
    ] {
        let selection =
            ValidatedLirTargetSelection::new(scoop_target_data::DARWIN_AARCH64, backend);
        assert!(
            ValidatedBackendProfile::for_selection(selection)
                .unwrap_err()
                .0
                .contains("unsupported target/backend contract")
        );
    }
}

#[test]
fn canonical_unknown_target_is_not_a_codegen_registration() {
    const UNKNOWN: scoop_lir::TargetProfileId =
        match scoop_lir::TargetProfileId::new("unknown-target", 1) {
            Ok(id) => id,
            Err(_) => panic!("canonical test identity"),
        };
    let target =
        scoop_lir::LirTargetProfile::new(&UNKNOWN, scoop_target_data::DARWIN_AARCH64.description());
    let selection = ValidatedLirTargetSelection::new(target, scoop_target_data::LLVM_22_1);
    assert!(
        ValidatedBackendProfile::for_selection(selection)
            .unwrap_err()
            .0
            .contains("unsupported target/backend")
    );
}
