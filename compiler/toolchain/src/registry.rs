use crate::runtime::ValidatedRuntimeBuildProfile;
use std::path::PathBuf;

use crate::ValidatedCBridgeToolchainInvocation;
use scoop_lir::{BackendProfile, LirTargetProfile, TargetProfileId, ValidatedLirTargetSelection};

use crate::{FinalLinkOptions, ToolchainError, ValidatedFinalLinkProfile};

#[derive(Clone, Debug, Default)]
pub struct CToolchainOptions {
    pub compiler: Option<PathBuf>,
    pub cxx_compiler: Option<PathBuf>,
    pub native_sysroot: Option<PathBuf>,
}

/// Complete target selection resolved atomically by the shared registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTargetProfile {
    lir_target: ValidatedLirTargetSelection,
    backend: BackendProfile,
    c_bridge_toolchain: ValidatedCBridgeToolchainInvocation,
    runtime_build: ValidatedRuntimeBuildProfile,
    native_tool_inputs: scoop_wire::Digest256,
    native_c: Option<PathBuf>,
    native_cxx: Option<PathBuf>,
}

impl ResolvedTargetProfile {
    /// Resolves all mutually compatible projections as one value.
    pub fn resolve(triple: &str) -> Result<Self, ToolchainError> {
        Self::resolve_with(triple, &CToolchainOptions::default())
    }

    pub fn resolve_with(triple: &str, options: &CToolchainOptions) -> Result<Self, ToolchainError> {
        crate::native_record::probe(triple, options)
    }

    pub(crate) fn from_c_toolchain(
        lir_target: ValidatedLirTargetSelection,
        c_bridge_toolchain: ValidatedCBridgeToolchainInvocation,
        native_tool_inputs: scoop_wire::Digest256,
    ) -> Result<Self, ToolchainError> {
        Ok(Self {
            lir_target,
            backend: lir_target.backend(),
            c_bridge_toolchain,
            runtime_build: ValidatedRuntimeBuildProfile::for_target(lir_target.target())?,
            native_tool_inputs,
            native_c: None,
            native_cxx: None,
        })
    }

    pub(crate) fn with_native_drivers(mut self, c: Option<PathBuf>, cxx: Option<PathBuf>) -> Self {
        self.native_c = c;
        self.native_cxx = cxx;
        self
    }

    pub(crate) fn native_c_driver(&self) -> Option<&std::path::Path> {
        self.native_c.as_deref()
    }
    pub(crate) fn native_cxx_driver(&self) -> Option<&std::path::Path> {
        self.native_cxx.as_deref()
    }

    pub const fn native_tool_inputs(&self) -> scoop_wire::Digest256 {
        self.native_tool_inputs
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
        self.final_link_with_cxx(options, false)
    }

    pub fn final_link_with_cxx_cached(
        &self,
        options: &FinalLinkOptions,
        cxx: bool,
        cache: &std::path::Path,
    ) -> Result<ValidatedFinalLinkProfile, ToolchainError> {
        ValidatedFinalLinkProfile::resolve_cached(self, options, cxx, cache)
    }

    pub fn final_link_with_cxx(
        &self,
        options: &FinalLinkOptions,
        cxx: bool,
    ) -> Result<ValidatedFinalLinkProfile, ToolchainError> {
        let cxx = cxx
            .then(|| {
                crate::ValidatedCxxToolchain::resolve_with(
                    &self.c_bridge_toolchain,
                    self.native_cxx_driver(),
                )
            })
            .transpose()?;
        ValidatedFinalLinkProfile::from_tools(self.c_bridge_toolchain.clone(), options, cxx)
    }
}

pub fn host_target_triple() -> Result<&'static str, ToolchainError> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("aarch64", "macos") => Ok("aarch64-apple-darwin"),
        ("x86_64", "linux") if cfg!(target_env = "musl") => Ok("x86_64-unknown-linux-musl"),
        ("x86_64", "linux") => Ok("x86_64-unknown-linux-gnu"),
        ("x86_64", "windows") => Ok("x86_64-pc-windows-msvc-ucrt"),
        (arch, os) => Err(ToolchainError(format!(
            "unsupported host {arch}-{os}; supported hosts are macOS/AArch64, Linux/amd64 and Windows/amd64"
        ))),
    }
}

pub fn select_target(triple: &str) -> Result<ValidatedLirTargetSelection, ToolchainError> {
    match triple {
        "x86_64-unknown-linux-gnu" | "x86_64-linux-gnu" => {
            return Ok(scoop_target_data::LINUX_X86_64_GNU_LLVM_22_1);
        }
        "x86_64-unknown-linux-musl" | "x86_64-linux-musl" => {
            return Ok(scoop_target_data::LINUX_X86_64_MUSL_LLVM_22_1);
        }
        "x86_64-pc-windows-msvc-ucrt" => {
            return Ok(scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1);
        }
        "x86_64-w64-windows-llvm-mingw-msvcrt" => {
            return Ok(scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1);
        }
        "x86_64-w64-windows-llvm-mingw-ucrt" => {
            return Ok(scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1);
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
        Ok(scoop_target_data::DARWIN_AARCH64_LLVM_22_1)
    } else {
        Err(ToolchainError(format!(
            "unsupported target {triple:?}; use a registered macOS/AArch64, Linux/amd64 or complete Windows/amd64 target"
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
            assert_eq!(profile.id(), scoop_target_data::DARWIN_AARCH64_ID);
            assert_eq!(profile.canonical_triple(), "aarch64-apple-darwin");
            assert_eq!(
                profile.lir_target_selection(),
                scoop_target_data::DARWIN_AARCH64_LLVM_22_1
            );
            assert_eq!(profile.backend(), scoop_target_data::LLVM_22_1);
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
            "x86_64-pc-windows-msvc",
            "x86_64-w64-windows-gnu",
            "windows-ucrt",
            "windows-msvcrt",
        ] {
            let error = ResolvedTargetProfile::resolve(triple).unwrap_err();
            assert!(error.0.contains("unsupported target"));
        }
    }

    #[test]
    fn windows_cli_names_preserve_each_native_environment() {
        for (triple, selection) in [
            (
                "x86_64-pc-windows-msvc-ucrt",
                scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1,
            ),
            (
                "x86_64-w64-windows-llvm-mingw-msvcrt",
                scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1,
            ),
            (
                "x86_64-w64-windows-llvm-mingw-ucrt",
                scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1,
            ),
        ] {
            assert_eq!(select_target(triple).unwrap(), selection);
            assert_eq!(
                crate::manifest::source_target(selection.target().id())
                    .unwrap()
                    .triple,
                triple
            );
        }
        if cfg!(all(windows, target_arch = "x86_64")) {
            assert_eq!(host_target_triple().unwrap(), "x86_64-pc-windows-msvc-ucrt");
        }
    }
}
