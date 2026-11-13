use std::collections::BTreeSet;

use scoop_lir::LirTargetProfile;

use super::ValidatedRuntimeBuildProfile;

struct ExpectedComponents {
    bundle: &'static str,
    os: &'static str,
    image: &'static str,
    loaded_image: Option<&'static str>,
    trace_loader: Option<&'static str>,
    arch: &'static str,
    frame: &'static str,
    managed_entry: &'static str,
    abi_shim: &'static str,
    metadata_image_ops: &'static str,
    thread_vm_ops: &'static str,
    managed_frame_ops: &'static str,
    c_long_size: &'static str,
    image_machine: Option<&'static str>,
    inline_gc_forwarding: bool,
    no_red_zone: bool,
}

fn assert_components(target: LirTargetProfile, expected: ExpectedComponents) {
    let profile = ValidatedRuntimeBuildProfile::for_target(target).unwrap();
    let sources = profile.runtime_sources(false);
    let unique_sources: BTreeSet<_> = sources.iter().copied().collect();
    assert_eq!(unique_sources.len(), sources.len());
    for expected_source in [
        expected.bundle,
        expected.os,
        expected.image,
        expected.arch,
        expected.frame,
        expected.managed_entry,
    ] {
        assert!(
            unique_sources.contains(expected_source),
            "{} does not select {expected_source}",
            profile.canonical_triple()
        );
    }
    assert!(
        expected
            .trace_loader
            .map(|source| unique_sources.contains(source))
            .unwrap_or_else(|| { !unique_sources.contains("runtime/src/stack_trace/linux.c") }),
        "{} selects the wrong trace loader",
        profile.canonical_triple()
    );
    assert!(
        expected
            .loaded_image
            .map(|source| unique_sources.contains(source))
            .unwrap_or_else(|| {
                !unique_sources.contains("runtime/src/platform/loader/linux_elf.c")
            }),
        "{} selects the wrong loaded-image provider",
        profile.canonical_triple()
    );
    assert!(
        unique_sources.contains(expected.abi_shim),
        "{} selects the wrong Scoop ABI shim",
        profile.canonical_triple()
    );
    assert_eq!(
        profile.runtime_c_flags().contains(&"-mno-red-zone"),
        expected.no_red_zone
    );
    let flags = profile.runtime_c_flags();
    for expected_flag in [
        expected.metadata_image_ops,
        expected.thread_vm_ops,
        expected.managed_frame_ops,
        expected.c_long_size,
    ] {
        assert!(
            flags.contains(&expected_flag),
            "{} does not select {expected_flag}",
            profile.canonical_triple()
        );
    }
    assert_eq!(
        flags.contains(&"-DSCOOP_PLATFORM_INLINE_GC_FORWARDING=1"),
        expected.inline_gc_forwarding
    );
    assert_eq!(
        flags.contains(&"-DSCOOP_PLATFORM_INLINE_GC_FORWARDING=0"),
        !expected.inline_gc_forwarding
    );
    assert_eq!(
        flags
            .iter()
            .copied()
            .find(|flag| flag.starts_with("-DSCOOP_PE_IMAGE_MACHINE=")),
        expected.image_machine
    );
}

#[test]
fn current_targets_compose_runtime_from_independent_facts() {
    assert_components(
        scoop_target_data::DARWIN_AARCH64,
        ExpectedComponents {
            bundle: "runtime/src/platform/bundle.c",
            os: "runtime/src/platform/os/darwin.c",
            image: "runtime/src/platform/image/macho.c",
            loaded_image: None,
            trace_loader: None,
            arch: "runtime/src/platform/arch/aarch64_float_environment.c",
            frame: "runtime/src/platform/frame/darwin_aarch64.c",
            managed_entry: "runtime/src/platform/frame/darwin_aarch64_macho.S",
            abi_shim: "runtime/src/platform/abi/darwin_aarch64_macho_floating.S",
            metadata_image_ops: "-DSCOOP_PLATFORM_METADATA_IMAGE_OPS=scoop_macho_metadata_image_ops",
            thread_vm_ops: "-DSCOOP_PLATFORM_THREAD_VM_OPS=scoop_darwin_thread_vm_ops",
            managed_frame_ops: "-DSCOOP_PLATFORM_MANAGED_FRAME_OPS=scoop_darwin_aarch64_managed_frame_ops",
            c_long_size: "-DSCOOP_TARGET_LONG_SIZE=8",
            image_machine: None,
            inline_gc_forwarding: false,
            no_red_zone: false,
        },
    );
    assert_components(
        scoop_target_data::DARWIN_X86_64,
        ExpectedComponents {
            bundle: "runtime/src/platform/bundle.c",
            os: "runtime/src/platform/os/darwin.c",
            image: "runtime/src/platform/image/macho.c",
            loaded_image: None,
            trace_loader: None,
            arch: "runtime/src/platform/arch/x86_64_float_environment.c",
            frame: "runtime/src/platform/frame/sysv_amd64.c",
            managed_entry: "runtime/src/platform/frame/sysv_amd64_macho.S",
            abi_shim: "runtime/src/platform/abi/x86_64_tagged_floating.c",
            metadata_image_ops: "-DSCOOP_PLATFORM_METADATA_IMAGE_OPS=scoop_macho_metadata_image_ops",
            thread_vm_ops: "-DSCOOP_PLATFORM_THREAD_VM_OPS=scoop_darwin_thread_vm_ops",
            managed_frame_ops: "-DSCOOP_PLATFORM_MANAGED_FRAME_OPS=scoop_sysv_amd64_managed_frame_ops",
            c_long_size: "-DSCOOP_TARGET_LONG_SIZE=8",
            image_machine: None,
            inline_gc_forwarding: true,
            no_red_zone: true,
        },
    );
    for target in [
        scoop_target_data::LINUX_X86_64_GNU,
        scoop_target_data::LINUX_X86_64_MUSL,
    ] {
        assert_components(
            target,
            ExpectedComponents {
                bundle: "runtime/src/platform/bundle.c",
                os: "runtime/src/platform/os/linux.c",
                image: "runtime/src/platform/image/elf.c",
                loaded_image: Some("runtime/src/platform/loader/linux_elf.c"),
                trace_loader: Some("runtime/src/stack_trace/linux.c"),
                arch: "runtime/src/platform/arch/x86_64_float_environment.c",
                frame: "runtime/src/platform/frame/sysv_amd64.c",
                managed_entry: "runtime/src/platform/frame/sysv_amd64_elf.S",
                abi_shim: "runtime/src/platform/abi/x86_64_tagged_floating.c",
                metadata_image_ops: "-DSCOOP_PLATFORM_METADATA_IMAGE_OPS=scoop_linux_elf_metadata_image_ops",
                thread_vm_ops: "-DSCOOP_PLATFORM_THREAD_VM_OPS=scoop_linux_thread_vm_ops",
                managed_frame_ops: "-DSCOOP_PLATFORM_MANAGED_FRAME_OPS=scoop_sysv_amd64_managed_frame_ops",
                c_long_size: "-DSCOOP_TARGET_LONG_SIZE=8",
                image_machine: None,
                inline_gc_forwarding: true,
                no_red_zone: true,
            },
        );
    }
    for target in [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
    ] {
        assert_components(
            target,
            ExpectedComponents {
                bundle: "runtime/src/platform/bundle.c",
                os: "runtime/src/platform/os/windows.c",
                image: "runtime/src/platform/image/pe.c",
                loaded_image: None,
                trace_loader: None,
                arch: "runtime/src/platform/arch/x86_64_float_environment.c",
                frame: "runtime/src/platform/frame/windows_amd64_virtual_unwind.c",
                managed_entry: "runtime/src/platform/frame/windows_amd64_coff_managed_entry.S",
                abi_shim: "runtime/src/platform/abi/x86_64_tagged_floating.c",
                metadata_image_ops: "-DSCOOP_PLATFORM_METADATA_IMAGE_OPS=scoop_pe_metadata_image_ops",
                thread_vm_ops: "-DSCOOP_PLATFORM_THREAD_VM_OPS=scoop_windows_thread_vm_ops",
                managed_frame_ops: "-DSCOOP_PLATFORM_MANAGED_FRAME_OPS=scoop_windows_amd64_managed_frame_ops",
                c_long_size: "-DSCOOP_TARGET_LONG_SIZE=4",
                image_machine: Some("-DSCOOP_PE_IMAGE_MACHINE=IMAGE_FILE_MACHINE_AMD64"),
                inline_gc_forwarding: true,
                no_red_zone: true,
            },
        );
    }
}

#[test]
fn libc_environment_does_not_change_runtime_component_selection() {
    let linux_gnu =
        ValidatedRuntimeBuildProfile::for_target(scoop_target_data::LINUX_X86_64_GNU).unwrap();
    let linux_musl =
        ValidatedRuntimeBuildProfile::for_target(scoop_target_data::LINUX_X86_64_MUSL).unwrap();
    assert_eq!(
        linux_gnu.runtime_sources(false),
        linux_musl.runtime_sources(false)
    );
    assert_eq!(linux_gnu.runtime_c_flags(), linux_musl.runtime_c_flags());

    let windows_targets = [
        scoop_target_data::WINDOWS_X86_64_MSVC_UCRT,
        scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT,
        scoop_target_data::WINDOWS_X86_64_MINGW_UCRT,
    ];
    let baseline = ValidatedRuntimeBuildProfile::for_target(windows_targets[0]).unwrap();
    for target in windows_targets.into_iter().skip(1) {
        let candidate = ValidatedRuntimeBuildProfile::for_target(target).unwrap();
        assert_eq!(
            baseline.runtime_sources(false),
            candidate.runtime_sources(false)
        );
        assert_eq!(baseline.runtime_c_flags(), candidate.runtime_c_flags());
    }
}
