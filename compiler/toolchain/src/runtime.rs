//! Runtime composition for the implemented targets; no linker or LLVM discovery.
use scoop_lir::{LirTargetProfile, TargetProfileId};

mod sources;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedRuntimeBuildProfile {
    target: LirTargetProfile,
}

struct RuntimeComponents {
    profile: &'static str,
    os: &'static [&'static str],
    image: &'static [&'static str],
    arch: &'static [&'static str],
    image_flags: &'static [&'static str],
    arch_flags: &'static [&'static str],
    includes: &'static [&'static str],
}

impl ValidatedRuntimeBuildProfile {
    pub const fn for_target(target: LirTargetProfile) -> Self {
        Self { target }
    }

    pub const fn canonical_triple(self) -> &'static str {
        self.target.id().canonical_triple()
    }

    fn components(self) -> RuntimeComponents {
        match self.target.id() {
            TargetProfileId::DarwinAarch64 => RuntimeComponents {
                profile: "runtime/src/platform/profiles/darwin_aarch64.c",
                os: sources::DARWIN_OS,
                image: sources::MACHO_IMAGE,
                arch: sources::AARCH64,
                image_flags: &[],
                arch_flags: &[],
                includes: &["include", "src", "third_party/ryu"],
            },
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => {
                RuntimeComponents {
                    profile: "runtime/src/platform/profiles/linux_x86_64.c",
                    os: sources::LINUX_OS,
                    image: sources::ELF_IMAGE,
                    arch: sources::X86_64,
                    image_flags: &[
                        "-fvisibility=hidden",
                        "-DMBEDTLS_CONFIG_FILE=\"scoop_sha256_config.h\"",
                    ],
                    arch_flags: &["-mno-red-zone"],
                    includes: &[
                        "include",
                        "src",
                        "third_party/ryu",
                        "third_party/mbedtls",
                        "third_party/mbedtls/include",
                    ],
                }
            }
        }
    }

    pub fn runtime_sources(self) -> Vec<&'static str> {
        let components = self.components();
        sources::COMMON
            .iter()
            .copied()
            .chain([components.profile])
            .chain(components.os.iter().copied())
            .chain(components.image.iter().copied())
            .chain(components.arch.iter().copied())
            .collect()
    }

    pub fn runtime_c_flags(self) -> Vec<&'static str> {
        let components = self.components();
        [
            "-pthread",
            "-fno-omit-frame-pointer",
            "-fno-optimize-sibling-calls",
        ]
        .into_iter()
        .chain(components.arch_flags.iter().copied())
        .chain(components.image_flags.iter().copied())
        .collect()
    }

    pub fn include_directories(self) -> &'static [&'static str] {
        self.components().includes
    }
}
