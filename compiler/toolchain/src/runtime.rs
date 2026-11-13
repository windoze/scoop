//! Runtime composition for the implemented targets; no linker or LLVM discovery.
use scoop_lir::{
    BackendRedZoneV1, LirTargetProfile, MachineFrameRecoveryV1, NativeObjectFormat,
    ObjectTargetProfileV1, ProcedureAbi, ScoopAbiClassifier, TargetArchitecture,
};
use scoop_target_data::TargetOs;

#[cfg(all(test, target_os = "macos"))]
mod darwin_tests;
#[cfg(test)]
mod selection_tests;
mod sources;
#[cfg(all(test, windows, target_arch = "x86_64"))]
mod windows_tests;
#[cfg(all(test, windows, target_arch = "x86_64"))]
mod windows_trace_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedRuntimeBuildProfile {
    target: LirTargetProfile,
    components: RuntimeComponents,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeComponents {
    os: &'static [&'static str],
    image: &'static [&'static str],
    loaded_image: &'static [&'static str],
    arch: &'static [&'static str],
    frame: &'static [&'static str],
    managed_entry: &'static [&'static str],
    abi_shim: &'static [&'static str],
    trace: &'static [&'static str],
    trace_loader: &'static [&'static str],
    metadata_image_ops_flag: &'static str,
    thread_vm_ops_flag: &'static str,
    managed_frame_ops_flag: &'static str,
    c_long_size_flag: &'static str,
    gc_forwarding_flag: &'static str,
    os_flags: &'static [&'static str],
    image_flags: &'static [&'static str],
    frame_flags: &'static [&'static str],
    includes: &'static [&'static str],
}

impl ValidatedRuntimeBuildProfile {
    pub fn for_target(target: LirTargetProfile) -> Result<Self, crate::ToolchainError> {
        let definition = scoop_target_data::definition(target.id())
            .filter(|definition| definition.selection.target() == target)
            .ok_or_else(|| {
                crate::ToolchainError(format!("unsupported runtime target {target:?}"))
            })?;
        let (os, os_flags, trace, thread_vm_ops_flag): (_, &[_], _, _) = match definition.os {
            TargetOs::Darwin => (
                sources::DARWIN_OS,
                &["-pthread"],
                sources::UNIX_TRACE,
                sources::DARWIN_THREAD_VM_OPS,
            ),
            TargetOs::Linux => (
                sources::LINUX_OS,
                &["-pthread"],
                sources::UNIX_TRACE,
                sources::LINUX_THREAD_VM_OPS,
            ),
            TargetOs::Windows => (
                sources::WINDOWS_OS,
                &["-D__SEH__", "-D_CRT_SECURE_NO_WARNINGS"],
                sources::WINDOWS_TRACE,
                sources::WINDOWS_THREAD_VM_OPS,
            ),
        };
        let c_long_size_flag = match definition.native_environment.c_long_size_bytes() {
            4 => "-DSCOOP_TARGET_LONG_SIZE=4",
            8 => "-DSCOOP_TARGET_LONG_SIZE=8",
            size => {
                return Err(crate::ToolchainError(format!(
                    "unsupported target C long size {size}"
                )));
            }
        };
        let gc_forwarding_flag = match definition.runtime_tuning {
            scoop_target_data::RuntimeTuning::OutOfLineGcForwarding => {
                sources::OUT_OF_LINE_GC_FORWARDING
            }
            scoop_target_data::RuntimeTuning::InlineGcForwarding => sources::INLINE_GC_FORWARDING,
        };
        let (image, image_flags): (_, &[_]) = match target.object_target() {
            ObjectTargetProfileV1::MachOAarch64 | ObjectTargetProfileV1::MachOX86_64 => {
                (sources::MACHO_IMAGE, &[])
            }
            ObjectTargetProfileV1::ElfX86_64 => (sources::ELF_IMAGE, &["-fvisibility=hidden"]),
            ObjectTargetProfileV1::CoffAmd64 => {
                (sources::PE_IMAGE, &[sources::PE_AMD64_IMAGE_MACHINE])
            }
        };
        let (loaded_image, trace_loader, metadata_image_ops_flag): (&[&str], &[&str], &str) =
            match (definition.os, target.native_object_format()) {
                (TargetOs::Linux, NativeObjectFormat::Elf64) => (
                    sources::LINUX_ELF_LOADED_IMAGE,
                    sources::LINUX_ELF_TRACE_LOADER,
                    sources::LINUX_ELF_METADATA_IMAGE_OPS,
                ),
                (TargetOs::Darwin, NativeObjectFormat::MachO64) => {
                    (&[], &[], sources::MACHO_METADATA_IMAGE_OPS)
                }
                (TargetOs::Windows, NativeObjectFormat::Coff64) => {
                    (&[], &[], sources::PE_METADATA_IMAGE_OPS)
                }
                _ => {
                    return Err(crate::ToolchainError(
                        "no runtime loader adapters for the selected OS and object format".into(),
                    ));
                }
            };
        let arch = match target.architecture() {
            TargetArchitecture::Aarch64 => sources::AARCH64,
            TargetArchitecture::X86_64 => sources::X86_64,
        };
        let backend = definition.selection.backend().contract();
        let (frame, managed_frame_ops_flag) = match backend.machine_frame_recovery {
            MachineFrameRecoveryV1::DarwinAarch64Fixed => (
                sources::DARWIN_AARCH64_FRAME,
                sources::DARWIN_AARCH64_FRAME_OPS,
            ),
            MachineFrameRecoveryV1::SysvAmd64Fixed => {
                (sources::SYSV_AMD64_FRAME, sources::SYSV_AMD64_FRAME_OPS)
            }
            MachineFrameRecoveryV1::WindowsAmd64VirtualUnwind => (
                sources::WINDOWS_AMD64_VIRTUAL_UNWIND,
                sources::WINDOWS_AMD64_FRAME_OPS,
            ),
        };
        let frame_flags: &[_] = match backend.red_zone {
            BackendRedZoneV1::Enabled => &[],
            BackendRedZoneV1::Disabled => &["-mno-red-zone"],
        };
        let managed_entry = match (target.procedure_abi(), target.object_target()) {
            (ProcedureAbi::DarwinAarch64, ObjectTargetProfileV1::MachOAarch64) => {
                sources::DARWIN_AARCH64_MACHO_MANAGED_ENTRY
            }
            (ProcedureAbi::SysvAmd64, ObjectTargetProfileV1::MachOX86_64) => {
                sources::SYSV_AMD64_MACHO_MANAGED_ENTRY
            }
            (ProcedureAbi::SysvAmd64, ObjectTargetProfileV1::ElfX86_64) => {
                sources::SYSV_AMD64_ELF_MANAGED_ENTRY
            }
            (ProcedureAbi::Win64, ObjectTargetProfileV1::CoffAmd64) => {
                sources::WIN64_COFF_MANAGED_ENTRY
            }
            _ => {
                return Err(crate::ToolchainError(
                    "no runtime ManagedEntry for the selected procedure ABI and object target"
                        .into(),
                ));
            }
        };
        let abi_shim = match (
            target.procedure_abi(),
            target.contract().scoop_abi_classifier(),
            target.object_target(),
        ) {
            (
                ProcedureAbi::DarwinAarch64,
                ScoopAbiClassifier::ScalarInterfaceAndSmallValueParts,
                ObjectTargetProfileV1::MachOAarch64,
            ) => sources::DARWIN_AARCH64_MACHO_ABI_SHIM,
            (
                ProcedureAbi::SysvAmd64,
                ScoopAbiClassifier::ScalarInterfaceAndSmallValueParts,
                ObjectTargetProfileV1::MachOX86_64 | ObjectTargetProfileV1::ElfX86_64,
            )
            | (
                ProcedureAbi::Win64,
                ScoopAbiClassifier::Win64CallerCopyAndIndirectInterfaceResult,
                ObjectTargetProfileV1::CoffAmd64,
            ) => sources::X86_64_TAGGED_FLOATING_ABI_SHIM,
            _ => {
                return Err(crate::ToolchainError(
                    "no runtime Scoop ABI shim for the selected procedure ABI, Scoop ABI and object target"
                        .into(),
                ));
            }
        };
        let components = RuntimeComponents {
            os,
            image,
            loaded_image,
            arch,
            frame,
            managed_entry,
            abi_shim,
            trace,
            trace_loader,
            metadata_image_ops_flag,
            thread_vm_ops_flag,
            managed_frame_ops_flag,
            c_long_size_flag,
            gc_forwarding_flag,
            os_flags,
            image_flags,
            frame_flags,
            includes: &["include", "src"],
        };
        Ok(Self { target, components })
    }

    pub const fn canonical_triple(self) -> &'static str {
        self.target.contract().canonical_triple()
    }

    pub fn runtime_sources(self, enable_profiling: bool) -> Vec<&'static str> {
        let components = self.components;
        sources::COMMON
            .iter()
            .chain(if enable_profiling {
                sources::PROFILING
            } else {
                &[]
            })
            .copied()
            .chain(components.trace.iter().copied())
            .chain(components.trace_loader.iter().copied())
            .chain([sources::PLATFORM_BUNDLE])
            .chain(components.os.iter().copied())
            .chain(components.image.iter().copied())
            .chain(components.loaded_image.iter().copied())
            .chain(components.arch.iter().copied())
            .chain(components.frame.iter().copied())
            .chain(components.managed_entry.iter().copied())
            .chain(components.abi_shim.iter().copied())
            .collect()
    }

    pub fn runtime_c_flags(self) -> Vec<&'static str> {
        let components = self.components;
        components
            .os_flags
            .iter()
            .copied()
            .chain(["-fno-omit-frame-pointer", "-fno-optimize-sibling-calls"])
            .chain(components.frame_flags.iter().copied())
            .chain(components.image_flags.iter().copied())
            .chain([
                components.metadata_image_ops_flag,
                components.thread_vm_ops_flag,
                components.managed_frame_ops_flag,
                components.c_long_size_flag,
                components.gc_forwarding_flag,
            ])
            .collect()
    }

    pub fn include_directories(self) -> &'static [&'static str] {
        self.components.includes
    }
}
