//! Closed target/backend profiles accepted by Scoop codegen.
//!
//! M15 deliberately supports one profile.  Keeping every backend choice in
//! this module prevents target details and LLVM defaults from leaking through
//! the mechanical LIR-to-LLVM lowering.

use std::fmt;

use inkwell::OptimizationLevel;
use inkwell::llvm_sys::core::LLVMGetVersion;
use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};

use crate::CodegenError;

const REQUIRED_LLVM_MAJOR: u32 = 22;
const REQUIRED_LLVM_MINOR: u32 = 1;

/// The LLVM library linked into this compiler process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LlvmVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for LlvmVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Globally typed identity of a complete target profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetProfileId {
    DarwinAarch64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObjectFormat {
    MachO64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstructionSelector {
    SelectionDag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatepointRootPolicy {
    StackIndirectOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FramePointerPolicy {
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TailCallPolicy {
    Disabled,
}

/// A complete, immutable target/backend profile selected by the driver.
///
/// Fields are private so callers cannot construct a contradictory partial
/// profile.  New targets must be added through [`TargetProfile::resolve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetProfile {
    id: TargetProfileId,
    canonical_triple: &'static str,
    cpu: &'static str,
    features: &'static str,
    optimization: OptimizationLevel,
    relocation: RelocMode,
    code_model: CodeModel,
    object_format: ObjectFormat,
    instruction_selector: InstructionSelector,
    managed_address_space: u32,
    stack_map_version: u8,
    statepoint_roots: StatepointRootPolicy,
    frame_pointers: FramePointerPolicy,
    tail_calls: TailCallPolicy,
    runtime_sources: &'static [&'static str],
}

impl TargetProfile {
    const DARWIN_AARCH64: Self = Self {
        id: TargetProfileId::DarwinAarch64,
        canonical_triple: "aarch64-apple-darwin",
        cpu: "generic",
        features: "",
        optimization: OptimizationLevel::None,
        relocation: RelocMode::PIC,
        code_model: CodeModel::Default,
        object_format: ObjectFormat::MachO64,
        instruction_selector: InstructionSelector::SelectionDag,
        managed_address_space: 1,
        stack_map_version: 3,
        statepoint_roots: StatepointRootPolicy::StackIndirectOnly,
        frame_pointers: FramePointerPolicy::All,
        tail_calls: TailCallPolicy::Disabled,
        runtime_sources: &[
            "runtime/src/rt.c",
            "runtime/src/gc.c",
            "runtime/src/gc/collector.c",
            "runtime/src/gc/roots.c",
            "runtime/src/gc/stackmap.c",
            "runtime/src/gc/stack_roots.c",
            "runtime/src/thread.c",
            "runtime/src/callback.c",
            "runtime/src/platform/profiles/darwin_aarch64.c",
            "runtime/src/platform/image/macho.c",
            "runtime/src/platform/arch/aarch64.c",
            "runtime/src/platform/arch/aarch64_anchor.S",
            "runtime/src/platform/os/darwin.c",
        ],
    };

    /// Resolve a user/host triple to the one target profile supported by M15.
    /// LLVM compatibility is checked before a profile can escape this API.
    pub fn resolve(triple: &str) -> Result<Self, CodegenError> {
        validate_linked_llvm()?;
        let mut components = triple.split('-');
        let arch = components.next().unwrap_or_default();
        let vendor = components.next().unwrap_or_default();
        let os = components.next().unwrap_or_default();
        let has_extra_identity = components.any(|component| !component.is_empty());
        let supported_arch = matches!(arch, "aarch64" | "arm64");
        let supported_os = versioned_component(os, "darwin") || versioned_component(os, "macosx");

        if supported_arch && vendor == "apple" && supported_os && !has_extra_identity {
            Ok(Self::DARWIN_AARCH64)
        } else {
            Err(CodegenError(format!(
                "unsupported target {triple:?}; M15 supports only macOS/AArch64 \
                 (`aarch64-apple-darwin`, with `arm64` accepted as an alias)"
            )))
        }
    }

    pub fn id(self) -> TargetProfileId {
        self.id
    }

    pub fn canonical_triple(self) -> &'static str {
        self.canonical_triple
    }

    pub fn managed_address_space(self) -> u32 {
        self.managed_address_space
    }

    pub fn stack_map_version(self) -> u8 {
        self.stack_map_version
    }

    /// Runtime implementation files selected by this target profile.
    pub fn runtime_sources(self) -> &'static [&'static str] {
        self.runtime_sources
    }

    pub(crate) fn create_target_machine(self) -> Result<TargetMachine, CodegenError> {
        debug_assert_eq!(self.id, TargetProfileId::DarwinAarch64);
        debug_assert_eq!(self.object_format, ObjectFormat::MachO64);
        debug_assert_eq!(self.instruction_selector, InstructionSelector::SelectionDag);
        debug_assert_eq!(
            self.statepoint_roots,
            StatepointRootPolicy::StackIndirectOnly
        );
        debug_assert_eq!(self.frame_pointers, FramePointerPolicy::All);
        debug_assert_eq!(self.tail_calls, TailCallPolicy::Disabled);

        Target::initialize_aarch64(&InitializationConfig::default());
        let triple = TargetTriple::create(self.canonical_triple);
        let target = Target::from_triple(&triple).map_err(|error| {
            CodegenError(format!("no target for {}: {error}", self.canonical_triple))
        })?;
        target
            .create_target_machine(
                &triple,
                self.cpu,
                self.features,
                self.optimization,
                self.relocation,
                self.code_model,
            )
            .ok_or_else(|| {
                CodegenError(format!(
                    "failed to create target machine for {}",
                    self.canonical_triple
                ))
            })
    }
}

/// Read and validate the LLVM version linked into this compiler process.
pub fn linked_llvm_version() -> LlvmVersion {
    let mut major = 0;
    let mut minor = 0;
    let mut patch = 0;
    // SAFETY: all three pointers refer to live `u32` values for the duration
    // of the call, exactly as required by LLVM's C API.
    unsafe { LLVMGetVersion(&mut major, &mut minor, &mut patch) };
    LlvmVersion {
        major,
        minor,
        patch,
    }
}

fn validate_linked_llvm() -> Result<LlvmVersion, CodegenError> {
    validate_llvm_version(linked_llvm_version())
}

fn validate_llvm_version(version: LlvmVersion) -> Result<LlvmVersion, CodegenError> {
    if version.major == REQUIRED_LLVM_MAJOR && version.minor == REQUIRED_LLVM_MINOR {
        Ok(version)
    } else {
        Err(CodegenError(format!(
            "unsupported LLVM backend {version}; Scoop requires LLVM \
             {REQUIRED_LLVM_MAJOR}.{REQUIRED_LLVM_MINOR}"
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
                profile.runtime_sources(),
                [
                    "runtime/src/rt.c",
                    "runtime/src/gc.c",
                    "runtime/src/gc/collector.c",
                    "runtime/src/gc/roots.c",
                    "runtime/src/gc/stackmap.c",
                    "runtime/src/gc/stack_roots.c",
                    "runtime/src/thread.c",
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
}
