use scoop_lir::{
    BackendProfile, LirTargetProfile, TargetProfileId, ValidatedCBridgeToolchainInvocation,
    ValidatedLirTargetSelection,
};

use crate::{ToolchainError, c_bridge::resolve_system_c_bridge_toolchain};

/// Complete target selection resolved atomically by the shared registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTargetProfile {
    lir_target: ValidatedLirTargetSelection,
    backend: BackendProfile,
    c_bridge_toolchain: ValidatedCBridgeToolchainInvocation,
    runtime_build: ValidatedRuntimeBuildProfile,
    final_link: ValidatedFinalLinkProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedRuntimeBuildProfile {
    canonical_triple: &'static str,
    runtime_sources: &'static [&'static str],
    runtime_c_flags: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedFinalLinkProfile {
    target: LirTargetProfile,
    canonical_triple: &'static str,
    linker_driver: &'static str,
    linker_args: &'static [&'static str],
}

impl ResolvedTargetProfile {
    fn darwin_aarch64(c_bridge_toolchain: ValidatedCBridgeToolchainInvocation) -> Self {
        let lir_target = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        Self {
            lir_target,
            backend: lir_target.backend(),
            c_bridge_toolchain,
            runtime_build: ValidatedRuntimeBuildProfile {
                canonical_triple: "aarch64-apple-darwin",
                runtime_sources: &[
                    "runtime/src/rt.c",
                    "runtime/src/boxing.c",
                    "runtime/src/arrays.c",
                    "runtime/src/value_shape.c",
                    "runtime/src/value_scan.c",
                    "runtime/src/eh.c",
                    "runtime/src/eh_personality.c",
                    "runtime/src/initialization.c",
                    "runtime/src/image/ranges.c",
                    "runtime/src/image/checks.c",
                    "runtime/src/image/dependencies.c",
                    "runtime/src/image/records.c",
                    "runtime/src/image/registry.c",
                    "runtime/src/image/scan_ranges.c",
                    "runtime/src/image/types.c",
                    "runtime/src/image/type_relations.c",
                    "runtime/src/image/storage.c",
                    "runtime/src/image/immortals.c",
                    "runtime/src/image/static_values.c",
                    "runtime/src/image/units.c",
                    "runtime/src/image/allocation_ranges.c",
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
                ],
                runtime_c_flags: &[
                    "-pthread",
                    "-fno-omit-frame-pointer",
                    "-fno-optimize-sibling-calls",
                ],
            },
            final_link: ValidatedFinalLinkProfile {
                target: LirTargetProfile::DARWIN_AARCH64,
                canonical_triple: "aarch64-apple-darwin",
                linker_driver: "cc",
                linker_args: &[
                    "-pthread",
                    "-Wl,-rename_section,__LLVM_STACKMAPS,__llvm_stackmaps,__DATA_CONST,__llvm_stackmaps",
                ],
            },
        }
    }

    /// Resolves all mutually compatible projections as one value.
    pub fn resolve(triple: &str) -> Result<Self, ToolchainError> {
        let mut components = triple.split('-');
        let arch = components.next().unwrap_or_default();
        let vendor = components.next().unwrap_or_default();
        let os = components.next().unwrap_or_default();
        let has_extra_identity = components.any(|component| !component.is_empty());
        let supported_arch = matches!(arch, "aarch64" | "arm64");
        let supported_os = versioned_component(os, "darwin") || versioned_component(os, "macosx");

        if supported_arch && vendor == "apple" && supported_os && !has_extra_identity {
            Ok(Self::darwin_aarch64(resolve_system_c_bridge_toolchain()?))
        } else {
            Err(ToolchainError(format!(
                "unsupported target {triple:?}; M23 supports only macOS/AArch64 \
                 (`aarch64-apple-darwin`, with `arm64` accepted as an alias)"
            )))
        }
    }

    /// Resolves the current host through the same closed registry.
    pub fn resolve_host() -> Result<Self, ToolchainError> {
        let host = match (std::env::consts::ARCH, std::env::consts::OS) {
            ("aarch64", "macos") => "aarch64-apple-darwin",
            (arch, os) => {
                return Err(ToolchainError(format!(
                    "unsupported host {arch}-{os}; M23 supports only macOS/AArch64"
                )));
            }
        };
        Self::resolve(host)
    }

    pub const fn id(&self) -> TargetProfileId {
        self.lir_target.target().id()
    }

    /// Returns the unique target spelling transported to the paired compiler.
    pub const fn canonical_triple(&self) -> &'static str {
        self.runtime_build.canonical_triple
    }

    pub const fn lir_target(&self) -> LirTargetProfile {
        self.lir_target.target()
    }

    pub const fn lir_target_selection(&self) -> ValidatedLirTargetSelection {
        self.lir_target
    }

    pub const fn backend(&self) -> BackendProfile {
        self.backend
    }

    pub const fn c_bridge_toolchain(&self) -> &ValidatedCBridgeToolchainInvocation {
        &self.c_bridge_toolchain
    }

    pub const fn runtime_build(&self) -> ValidatedRuntimeBuildProfile {
        self.runtime_build
    }

    pub const fn final_link(&self) -> ValidatedFinalLinkProfile {
        self.final_link
    }
}

impl ValidatedRuntimeBuildProfile {
    pub const fn canonical_triple(self) -> &'static str {
        self.canonical_triple
    }

    pub const fn runtime_sources(self) -> &'static [&'static str] {
        self.runtime_sources
    }

    pub const fn runtime_c_flags(self) -> &'static [&'static str] {
        self.runtime_c_flags
    }
}

impl ValidatedFinalLinkProfile {
    pub const fn id(self) -> TargetProfileId {
        self.target.id()
    }

    pub const fn canonical_triple(self) -> &'static str {
        self.canonical_triple
    }

    pub const fn linker_driver(self) -> &'static str {
        self.linker_driver
    }

    pub const fn linker_args(self) -> &'static [&'static str] {
        self.linker_args
    }
}

fn versioned_component(component: &str, prefix: &str) -> bool {
    let Some(suffix) = component.strip_prefix(prefix) else {
        return false;
    };
    suffix.is_empty()
        || suffix.starts_with(|character: char| character.is_ascii_digit())
        || suffix.strip_prefix('.').is_some_and(|version| {
            version.starts_with(|character: char| character.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_resolve_to_one_complete_profile() {
        let expected = ResolvedTargetProfile::resolve("aarch64-apple-darwin").unwrap();
        for triple in [
            "aarch64-apple-darwin",
            "arm64-apple-darwin",
            "arm64-apple-darwin25.6.0",
            "aarch64-apple-macosx14.0.0",
        ] {
            let profile = ResolvedTargetProfile::resolve(triple).unwrap();
            assert_eq!(profile, expected);
            assert_eq!(profile.id(), TargetProfileId::DarwinAarch64);
            assert_eq!(profile.canonical_triple(), "aarch64-apple-darwin");
            assert_eq!(
                profile.lir_target_selection(),
                ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
            );
            assert_eq!(profile.backend(), BackendProfile::LLVM_22_1);
            assert_eq!(
                profile
                    .c_bridge_toolchain()
                    .profile()
                    .contract()
                    .canonical_triple(),
                "aarch64-apple-darwin"
            );
            assert_eq!(
                profile.runtime_build().runtime_c_flags(),
                [
                    "-pthread",
                    "-fno-omit-frame-pointer",
                    "-fno-optimize-sibling-calls",
                ]
            );
            assert_eq!(
                profile.final_link().linker_args(),
                [
                    "-pthread",
                    "-Wl,-rename_section,__LLVM_STACKMAPS,__llvm_stackmaps,__DATA_CONST,__llvm_stackmaps"
                ]
            );
        }
    }

    #[test]
    fn unsupported_targets_are_rejected() {
        for triple in [
            "arm64e-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-gnu",
            "aarch64-apple-ios",
        ] {
            let error = ResolvedTargetProfile::resolve(triple).unwrap_err();
            assert!(error.0.contains("unsupported target"));
        }
    }
}
