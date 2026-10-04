use crate::runtime::ValidatedRuntimeBuildProfile;
use std::path::PathBuf;

use scoop_lir::{
    BackendProfile, LirTargetProfile, TargetProfileId, ValidatedCBridgeToolchainInvocation,
    ValidatedLirTargetSelection,
};

use crate::{
    FinalLinkOptions, ToolchainError, ValidatedFinalLinkProfile,
    c_bridge::resolve_system_c_bridge_toolchain,
};

#[derive(Clone, Debug, Default)]
pub struct CToolchainOptions {
    pub compiler: Option<PathBuf>,
    pub native_sysroot: Option<PathBuf>,
}

/// Complete target selection resolved atomically by the shared registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTargetProfile {
    lir_target: ValidatedLirTargetSelection,
    backend: BackendProfile,
    c_bridge_toolchain: ValidatedCBridgeToolchainInvocation,
    runtime_build: ValidatedRuntimeBuildProfile,
}

impl ResolvedTargetProfile {
    /// Resolves all mutually compatible projections as one value.
    pub fn resolve(triple: &str) -> Result<Self, ToolchainError> {
        Self::resolve_with(triple, &CToolchainOptions::default())
    }

    pub fn resolve_with(triple: &str, options: &CToolchainOptions) -> Result<Self, ToolchainError> {
        let target = validate_target(triple)?;
        let c_bridge_toolchain = match target.id() {
            TargetProfileId::DarwinAarch64 => {
                if options.compiler.is_some() || options.native_sysroot.is_some() {
                    return Err(ToolchainError("--cc and --native-sysroot apply to Linux targets; Darwin uses the selected Xcode toolchain".into()));
                }
                resolve_system_c_bridge_toolchain()?
            }
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => {
                crate::resolve_linux_c_toolchain(
                    target,
                    options.compiler.as_deref(),
                    options.native_sysroot.as_deref(),
                )?
            }
        };
        let lir_target = ValidatedLirTargetSelection::from_id(target.id());
        Ok(Self {
            lir_target,
            backend: lir_target.backend(),
            c_bridge_toolchain,
            runtime_build: ValidatedRuntimeBuildProfile::for_target(target),
        })
    }

    /// Resolves the current host through the same closed registry.
    pub fn resolve_host() -> Result<Self, ToolchainError> {
        Self::resolve(host_target_triple()?)
    }

    pub const fn id(&self) -> TargetProfileId {
        self.lir_target.target().id()
    }

    /// Returns the unique target spelling transported to the paired compiler.
    pub const fn canonical_triple(&self) -> &'static str {
        self.runtime_build.canonical_triple()
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

    pub fn final_link(&self) -> Result<ValidatedFinalLinkProfile, ToolchainError> {
        self.final_link_with(&FinalLinkOptions::default())
    }

    pub fn final_link_with(
        &self,
        options: &FinalLinkOptions,
    ) -> Result<ValidatedFinalLinkProfile, ToolchainError> {
        ValidatedFinalLinkProfile::from_startup(self.c_bridge_toolchain.clone(), options)
    }
}

pub fn host_target_triple() -> Result<&'static str, ToolchainError> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => Ok("aarch64-apple-darwin"),
        ("x86_64", "linux") if cfg!(target_env = "musl") => Ok("x86_64-unknown-linux-musl"),
        ("x86_64", "linux") => Ok("x86_64-unknown-linux-gnu"),
        (arch, os) => Err(ToolchainError(format!(
            "unsupported host {arch}-{os}; M28 supports macOS/AArch64 and Linux/amd64"
        ))),
    }
}

pub(crate) fn validate_target(triple: &str) -> Result<LirTargetProfile, ToolchainError> {
    match triple {
        "x86_64-unknown-linux-gnu" | "x86_64-linux-gnu" => {
            return Ok(LirTargetProfile::LINUX_X86_64_GNU);
        }
        "x86_64-unknown-linux-musl" | "x86_64-linux-musl" => {
            return Ok(LirTargetProfile::LINUX_X86_64_MUSL);
        }
        _ => {}
    }
    let mut components = triple.split('-');
    let arch = components.next().unwrap_or_default();
    let vendor = components.next().unwrap_or_default();
    let os = components.next().unwrap_or_default();
    let has_extra_identity = components.any(|component| !component.is_empty());
    let supported_os = versioned_component(os, "darwin") || versioned_component(os, "macosx");
    if matches!(arch, "aarch64" | "arm64")
        && vendor == "apple"
        && supported_os
        && !has_extra_identity
    {
        Ok(LirTargetProfile::DARWIN_AARCH64)
    } else {
        Err(ToolchainError(format!(
            "unsupported target {triple:?}; M28 supports macOS/AArch64, Linux glibc/amd64 and Linux musl/amd64"
        )))
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

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
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
            assert!(profile.final_link().unwrap().linker_driver().is_absolute());
            assert!(
                profile
                    .final_link()
                    .unwrap()
                    .linker_args()
                    .contains(&"-no_deduplicate")
            );
            assert!(
                profile
                    .final_link()
                    .unwrap()
                    .system_provider()
                    .unwrap()
                    .exports()
                    .contains_key("_getpagesize")
            );
            assert_eq!(
                profile.final_link().unwrap().fingerprint().unwrap(),
                expected.final_link().unwrap().fingerprint().unwrap()
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
