//! Runtime composition for the implemented targets; no linker or LLVM discovery.
use scoop_lir::{BackendMachineFrameV1, LirTargetProfile, NativeObjectFormat, TargetArchitecture};
use scoop_target_data::{NativeEnvironment, RuntimeProfile};

#[cfg(all(test, target_os = "macos"))]
mod darwin_tests;
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
    profile: &'static str,
    os: &'static [&'static str],
    image: &'static [&'static str],
    arch: &'static [&'static str],
    frame: &'static [&'static str],
    trace: &'static [&'static str],
    os_flags: &'static [&'static str],
    image_flags: &'static [&'static str],
    arch_flags: &'static [&'static str],
    includes: &'static [&'static str],
}

impl ValidatedRuntimeBuildProfile {
    pub fn for_target(target: LirTargetProfile) -> Result<Self, crate::ToolchainError> {
        let definition = scoop_target_data::definition(target.id())
            .filter(|definition| definition.selection.target() == target)
            .ok_or_else(|| {
                crate::ToolchainError(format!("unsupported runtime target {target:?}"))
            })?;
        let (os, os_flags, trace): (_, &[_], _) = match definition.native_environment {
            NativeEnvironment::Darwin => (sources::DARWIN_OS, &["-pthread"], sources::UNIX_TRACE),
            NativeEnvironment::LinuxGnu | NativeEnvironment::LinuxMusl => {
                (sources::LINUX_OS, &["-pthread"], sources::UNIX_TRACE)
            }
            NativeEnvironment::MicrosoftUcrt
            | NativeEnvironment::LlvmMingwMsvcrt
            | NativeEnvironment::LlvmMingwUcrt => (
                sources::WINDOWS_OS,
                &["-D__SEH__", "-D_CRT_SECURE_NO_WARNINGS"],
                sources::WINDOWS_TRACE,
            ),
        };
        let (image, image_flags): (_, &[_]) = match target.native_object_format() {
            NativeObjectFormat::MachO64 => (sources::MACHO_IMAGE, &[]),
            NativeObjectFormat::Elf64 => (sources::ELF_IMAGE, &["-fvisibility=hidden"]),
            NativeObjectFormat::Coff64 => (sources::PE_IMAGE, &[]),
        };
        let (arch, arch_flags): (_, &[_]) = match target.architecture() {
            TargetArchitecture::Aarch64 => (sources::AARCH64, &[]),
            TargetArchitecture::X86_64 => (sources::X86_64, &["-mno-red-zone"]),
        };
        let frame = match (
            definition.selection.backend().contract().machine_frame,
            target.native_object_format(),
        ) {
            (BackendMachineFrameV1::DarwinAarch64, NativeObjectFormat::MachO64) => {
                sources::DARWIN_AARCH64_FRAME
            }
            (BackendMachineFrameV1::SysvAmd64, NativeObjectFormat::Elf64) => {
                sources::SYSV_AMD64_ELF_FRAME
            }
            (BackendMachineFrameV1::SysvAmd64, NativeObjectFormat::MachO64) => {
                sources::SYSV_AMD64_MACHO_FRAME
            }
            (BackendMachineFrameV1::WindowsSeh, NativeObjectFormat::Coff64) => {
                sources::WINDOWS_SEH_FRAME
            }
            _ => {
                return Err(crate::ToolchainError(
                    "no runtime anchor for the selected frame and object format".into(),
                ));
            }
        };
        let profile = match definition.runtime {
            RuntimeProfile::DarwinAarch64 => "runtime/src/platform/profiles/darwin_aarch64.c",
            RuntimeProfile::DarwinX86_64 => "runtime/src/platform/profiles/darwin_x86_64.c",
            RuntimeProfile::LinuxX86_64 => "runtime/src/platform/profiles/linux_x86_64.c",
            RuntimeProfile::WindowsX86_64 => "runtime/src/platform/profiles/windows_x86_64.c",
        };
        let components = RuntimeComponents {
            profile,
            os,
            image,
            arch,
            frame,
            trace,
            os_flags,
            image_flags,
            arch_flags,
            includes: &["include", "src"],
        };
        Ok(Self { target, components })
    }

    pub const fn canonical_triple(self) -> &'static str {
        self.target.contract().canonical_triple()
    }

    pub fn runtime_sources(self) -> Vec<&'static str> {
        let components = self.components;
        sources::COMMON
            .iter()
            .copied()
            .chain(components.trace.iter().copied())
            .chain([components.profile])
            .chain(components.os.iter().copied())
            .chain(components.image.iter().copied())
            .chain(components.arch.iter().copied())
            .chain(components.frame.iter().copied())
            .collect()
    }

    pub fn runtime_c_flags(self) -> Vec<&'static str> {
        let components = self.components;
        components
            .os_flags
            .iter()
            .copied()
            .chain(["-fno-omit-frame-pointer", "-fno-optimize-sibling-calls"])
            .chain(components.arch_flags.iter().copied())
            .chain(components.image_flags.iter().copied())
            .collect()
    }

    pub fn include_directories(self) -> &'static [&'static str] {
        self.components.includes
    }
}
