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
        let profile = TargetProfile::resolve(triple).expect(triple);
        assert_eq!(profile, TargetProfile::DARWIN_AARCH64);
        assert_eq!(profile.id(), TargetProfileId::DarwinAarch64);
        assert_eq!(profile.canonical_triple(), "aarch64-apple-darwin");
        assert_eq!(profile.managed_address_space(), 1);
        assert_eq!(profile.stack_map_version(), 3);
        assert_eq!(
            profile.runtime_c_flags(),
            [
                "-pthread",
                "-fno-omit-frame-pointer",
                "-fno-optimize-sibling-calls",
            ]
        );
        assert_eq!(profile.linker_args(), ["-pthread", "-lc++abi"]);
        assert_eq!(
            profile.eh_profile(),
            EhProfile {
                unwind_model: UnwindModel::ItaniumDwarf,
                personality_abi: PersonalityAbi::ScoopLsdaSubsetV1,
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
            profile.runtime_sources(),
            [
                "runtime/src/rt.c",
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
        let error = TargetProfile::resolve(triple).expect_err(triple);
        assert!(
            error.0.contains("unsupported target"),
            "unexpected error for {triple}: {error}"
        );
    }
}

#[test]
fn profile_creates_the_canonical_aarch64_machine() {
    let profile = TargetProfile::resolve("arm64-apple-darwin").expect("profile");
    let machine = profile.create_target_machine().expect("target machine");
    assert_eq!(
        machine
            .get_triple()
            .as_str()
            .to_str()
            .expect("UTF-8 triple"),
        profile.canonical_triple()
    );
}

#[test]
fn llvm_host_identity_resolves_through_the_target_registry() {
    assert_eq!(
        TargetProfile::resolve_host().expect("supported host profile"),
        TargetProfile::DARWIN_AARCH64
    );
}
