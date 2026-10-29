//! Runtime composition for the implemented targets; no linker or LLVM discovery.
use scoop_lir::LirTargetProfile;

mod sources;
#[cfg(all(test, windows, target_arch = "x86_64"))]
mod windows_tests;
#[cfg(all(test, windows, target_arch = "x86_64"))]
mod windows_trace_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedRuntimeBuildProfile {
    target: LirTargetProfile,
    components: &'static RuntimeComponents,
}

#[derive(Debug, PartialEq, Eq)]
struct RuntimeComponents {
    profile: &'static str,
    os: &'static [&'static str],
    image: &'static [&'static str],
    arch: &'static [&'static str],
    os_flags: &'static [&'static str],
    image_flags: &'static [&'static str],
    arch_flags: &'static [&'static str],
    includes: &'static [&'static str],
}

impl ValidatedRuntimeBuildProfile {
    pub fn for_target(target: LirTargetProfile) -> Result<Self, crate::ToolchainError> {
        let components = match target.id() {
            scoop_target_data::DARWIN_AARCH64_ID => &RuntimeComponents {
                profile: "runtime/src/platform/profiles/darwin_aarch64.c",
                os: sources::DARWIN_OS,
                image: sources::MACHO_IMAGE,
                arch: sources::AARCH64,
                os_flags: &["-pthread"],
                image_flags: &[],
                arch_flags: &[],
                includes: &["include", "src"],
            },
            scoop_target_data::LINUX_X86_64_GNU_ID | scoop_target_data::LINUX_X86_64_MUSL_ID => {
                &RuntimeComponents {
                    profile: "runtime/src/platform/profiles/linux_x86_64.c",
                    os: sources::LINUX_OS,
                    image: sources::ELF_IMAGE,
                    arch: sources::X86_64,
                    os_flags: &["-pthread"],
                    image_flags: &["-fvisibility=hidden"],
                    arch_flags: &["-mno-red-zone"],
                    includes: &["include", "src"],
                }
            }
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID => &RuntimeComponents {
                profile: "runtime/src/platform/profiles/windows_x86_64.c",
                os: sources::WINDOWS_OS,
                image: sources::PE_IMAGE,
                arch: sources::WINDOWS_X86_64,
                os_flags: &["-D__SEH__", "-D_CRT_SECURE_NO_WARNINGS"],
                image_flags: &[],
                arch_flags: &["-mno-red-zone"],
                includes: &["include", "src"],
            },
            _ => {
                return Err(crate::ToolchainError(format!(
                    "unsupported runtime target {}/{}",
                    target.id().canonical_name(),
                    target.id().major_version()
                )));
            }
        };
        Ok(Self { target, components })
    }

    pub const fn canonical_triple(self) -> &'static str {
        self.target.contract().canonical_triple()
    }

    pub fn runtime_sources(self) -> Vec<&'static str> {
        let components = self.components;
        let trace = if self.target.native_object_format() == scoop_lir::NativeObjectFormat::Coff64 {
            sources::WINDOWS_TRACE
        } else {
            sources::UNIX_TRACE
        };
        sources::COMMON
            .iter()
            .copied()
            .chain(trace.iter().copied())
            .chain([components.profile])
            .chain(components.os.iter().copied())
            .chain(components.image.iter().copied())
            .chain(components.arch.iter().copied())
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
