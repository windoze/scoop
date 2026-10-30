use scoop_lir::NativeSymbolNormalization;

#[test]
fn spelling_and_host_selection_use_the_complete_catalog() {
    for target in crate::TARGETS {
        assert_eq!(crate::named_target(target.canonical()), Some(target));
        for alias in target.aliases {
            assert_eq!(crate::named_target(alias), Some(target));
        }
    }
    for alias in ["arm64-apple-darwin25.6.0", "aarch64-apple-macosx14.0.0"] {
        assert_eq!(
            crate::named_target(alias).unwrap().selection,
            crate::DARWIN_AARCH64_LLVM_22_1
        );
    }
    for invalid in [
        "arm64-apple-darwin25.invalid",
        "arm64-apple-ios",
        "x86_64-pc-windows-msvc",
        "x86_64-w64-windows-gnu",
    ] {
        assert!(crate::named_target(invalid).is_none());
    }
    let windows = crate::host("windows", "x86_64", "").unwrap();
    assert_eq!(windows.rust_target, "x86_64-pc-windows-gnullvm");
    assert_eq!(
        windows.default_target,
        crate::WINDOWS_X86_64_MSVC_UCRT
            .contract()
            .canonical_triple()
    );
    assert_eq!(
        crate::host("linux", "x86_64", "musl")
            .unwrap()
            .default_target,
        crate::LINUX_X86_64_MUSL.contract().canonical_triple()
    );
    assert!(crate::host("linux", "aarch64", "").is_none());
}

#[test]
fn windows_environments_keep_distinct_identity_and_seh_frame_recovery() {
    let profiles = [
        crate::WINDOWS_X86_64_MSVC_UCRT,
        crate::WINDOWS_X86_64_MINGW_MSVCRT,
        crate::WINDOWS_X86_64_MINGW_UCRT,
    ];
    let fingerprints = profiles.map(|profile| profile.fingerprint().unwrap().to_string());
    assert_eq!(
        fingerprints
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    for profile in profiles {
        let selection = crate::selection(profile.id()).unwrap();
        assert_eq!(selection.backend(), crate::LLVM_22_1_WINDOWS_X86_64);
        assert_eq!(
            profile.native_object_format(),
            scoop_lir::NativeObjectFormat::Coff64
        );
        assert_eq!(
            profile.contract().native_symbol_normalization(),
            NativeSymbolNormalization::CoffIdentity
        );
        assert_eq!(
            profile.contract().scoop_abi_classifier(),
            scoop_lir::ScoopAbiClassifier::Win64CallerCopyAndIndirectInterfaceResult
        );
        assert!(matches!(
            selection.backend().contract().machine_frame,
            Some(scoop_lir::BackendMachineFrameV1::WindowsSeh {
                stack_pointer_register: 7,
                frame_pointer_register: 6,
                root_slot_bytes: 8
            })
        ));
        assert_eq!(
            scoop_lir::TargetEhSupportV1::ScoopPersonality.logical_symbol_for_target(profile),
            "scoop_eh_personality_seh"
        );
    }
}

#[test]
fn linux_libcs_have_distinct_targets_and_share_the_x86_backend() {
    let gnu = crate::selection(crate::LINUX_X86_64_GNU_ID)
        .expect("implemented target selected by this consumer");
    let musl = crate::selection(crate::LINUX_X86_64_MUSL_ID)
        .expect("implemented target selected by this consumer");
    assert_eq!(gnu.backend(), musl.backend());
    assert_ne!(gnu.backend().wire_id(), crate::LLVM_22_1.wire_id());
    assert_ne!(
        gnu.backend().fingerprint().unwrap(),
        crate::LLVM_22_1.fingerprint().unwrap()
    );
    assert_ne!(gnu.target().wire_id(), musl.target().wire_id());
    assert_ne!(
        gnu.target().fingerprint().unwrap(),
        musl.target().fingerprint().unwrap()
    );
    for profile in [gnu.target(), musl.target()] {
        assert_eq!(
            profile.contract().native_symbol_normalization(),
            NativeSymbolNormalization::ElfIdentity
        );
        assert_eq!(profile.contract().stack_alignment_bytes(), 16);
        assert_eq!(profile.managed_pointer_layout().size_bytes(), 8);
        assert_eq!(
            profile.native_object_format().wire_id(),
            scoop_identity::ObjectFormatId::elf_relocatable()
        );
        assert_eq!(
            profile
                .contract()
                .native_symbol_normalization()
                .compiler_generated_object_symbol("_entry"),
            "_entry"
        );
    }
}
