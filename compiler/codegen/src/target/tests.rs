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
fn darwin_aarch64_aliases_resolve_to_one_complete_profile() {
    for triple in [
        "aarch64-apple-darwin",
        "arm64-apple-darwin",
        "arm64-apple-darwin25.6.0",
        "aarch64-apple-macosx14.0.0",
    ] {
        let profile = ResolvedTargetProfile::resolve(triple).expect(triple);
        assert_eq!(profile, ResolvedTargetProfile::DARWIN_AARCH64);
        assert_eq!(profile.id(), TargetProfileId::DarwinAarch64);
        assert_eq!(
            profile.lir_target(),
            scoop_lir::LirTargetProfile::DARWIN_AARCH64
        );
        assert_eq!(
            profile.lir_target_selection(),
            scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
        );
        assert_eq!(
            profile.backend().backend_profile(),
            scoop_lir::BackendProfile::LLVM_22_1
        );
        let backend = profile.backend();
        backend
            .validate_lir_target_profile(scoop_lir::LirTargetProfile::DARWIN_AARCH64)
            .expect("the selected codegen and LIR profiles agree");
        assert_eq!(backend.canonical_triple(), "aarch64-apple-darwin");
        assert_eq!(backend.managed_address_space(), 1);
        assert_eq!(backend.stack_map_version(), 3);
        let c_bridge = profile.c_bridge_toolchain();
        assert_eq!(c_bridge.canonical_triple(), "aarch64-apple-darwin");
        assert_eq!(c_bridge.compiler_driver(), "cc");
        assert_eq!(c_bridge.compiler_args(), ["-std=c11"]);
        let runtime = profile.runtime_build();
        assert_eq!(runtime.canonical_triple(), "aarch64-apple-darwin");
        assert_eq!(
            runtime.runtime_c_flags(),
            [
                "-pthread",
                "-fno-omit-frame-pointer",
                "-fno-optimize-sibling-calls",
            ]
        );
        let final_link = profile.final_link();
        assert_eq!(final_link.id(), TargetProfileId::DarwinAarch64);
        assert_eq!(final_link.canonical_triple(), "aarch64-apple-darwin");
        assert_eq!(final_link.linker_driver(), "cc");
        assert_eq!(final_link.linker_args(), ["-pthread"]);
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
        assert_eq!(
            runtime.runtime_sources(),
            [
                "runtime/src/rt.c",
                "runtime/src/eh.c",
                "runtime/src/eh_personality.c",
                "runtime/src/initialization.c",
                "runtime/src/gc.c",
                "runtime/src/gc/allocation.c",
                "runtime/src/gc/collector.c",
                "runtime/src/gc/evacuation.c",
                "runtime/src/gc/reclamation.c",
                "runtime/src/gc/heap.c",
                "runtime/src/gc/heap_objects.c",
                "runtime/src/gc/handles.c",
                "runtime/src/gc/root_frames.c",
                "runtime/src/gc/roots.c",
                "runtime/src/gc/stackmap.c",
                "runtime/src/gc/stack_roots.c",
                "runtime/src/thread.c",
                "runtime/src/thread/collection.c",
                "runtime/src/thread/debug.c",
                "runtime/src/thread/roots.c",
                "runtime/src/thread/transitions.c",
                "runtime/src/callback.c",
                "runtime/src/platform/profiles/darwin_aarch64.c",
                "runtime/src/platform/image/macho.c",
                "runtime/src/platform/arch/aarch64.c",
                "runtime/src/platform/arch/aarch64_anchor.S",
                "runtime/src/platform/os/darwin.c",
            ]
        );
    }
}

#[test]
fn unsupported_targets_are_rejected_before_codegen() {
    for triple in [
        "arm64e-apple-darwin",
        "x86_64-apple-darwin",
        "aarch64-unknown-linux-gnu",
        "aarch64-apple-ios",
    ] {
        let error = ResolvedTargetProfile::resolve(triple).expect_err(triple);
        assert!(
            error.0.contains("unsupported target"),
            "unexpected error for {triple}: {error}"
        );
    }
}

#[test]
fn profile_creates_the_canonical_aarch64_machine() {
    let profile = ResolvedTargetProfile::resolve("arm64-apple-darwin").expect("profile");
    let backend = profile.backend();
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
fn llvm_host_identity_resolves_through_the_target_registry() {
    assert_eq!(
        ResolvedTargetProfile::resolve_host().expect("supported host profile"),
        ResolvedTargetProfile::DARWIN_AARCH64
    );
}

#[test]
fn darwin_aarch64_c_pointer_representations_are_qualified() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("codegen crate is nested below the workspace root");

    let profile_source = workspace.join("runtime/src/platform/profiles/darwin_aarch64.c");
    let profile = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-fsyntax-only"])
        .arg(&profile_source)
        .output()
        .expect("compile Darwin/AArch64 runtime profile qualification");
    assert!(
        profile.status.success(),
        "Darwin/AArch64 runtime target assertions must compile cleanly:\n{}",
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
