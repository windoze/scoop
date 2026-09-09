//! Closed target/backend profiles accepted by Scoop codegen.
//!
//! M15 deliberately supports one profile.  Keeping every backend choice in
//! this module prevents target details and LLVM defaults from leaking through
//! the mechanical LIR-to-LLVM lowering.

use std::fmt;
use std::path::Path;

use inkwell::llvm_sys::core::LLVMGetVersion;
use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use inkwell::{AddressSpace, OptimizationLevel};
use scoop_lir::LirTargetProfile;
pub use scoop_lir::TargetProfileId;

use crate::statepoint::ExpectedSafepoints;
use crate::{CodegenError, artifact};

mod qualification;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObjectFormat {
    MachO64,
}

/// Complete exception-handling contract qualified for one target/backend
/// profile.  These are capabilities rather than loosely related flags: a
/// target cannot reach codegen without selecting every part of its unwind,
/// LSDA and artifact-inspection ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EhProfile {
    unwind_model: UnwindModel,
    personality_abi: PersonalityAbi,
    exception_data_registers: u8,
    encodings: LsdaEncodingProfile,
    unwind_provider: UnwindProvider,
    artifact_inspection: EhArtifactInspection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnwindModel {
    ItaniumDwarf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PersonalityAbi {
    ScoopLsdaSubset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LsdaEncodingProfile {
    pub(crate) lp_start: u8,
    pub(crate) type_table: u8,
    pub(crate) call_site: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnwindProvider {
    DarwinLibSystem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EhArtifactInspection {
    MachO,
}

impl EhProfile {
    const DARWIN_AARCH64: Self = Self {
        unwind_model: UnwindModel::ItaniumDwarf,
        personality_abi: PersonalityAbi::ScoopLsdaSubset,
        exception_data_registers: 2,
        encodings: LsdaEncodingProfile {
            lp_start: 0xff,
            type_table: 0x9b,
            call_site: 0x01,
        },
        unwind_provider: UnwindProvider::DarwinLibSystem,
        artifact_inspection: EhArtifactInspection::MachO,
    };

    pub(crate) fn encodings(self) -> LsdaEncodingProfile {
        self.encodings
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LlvmTargetBackend {
    Aarch64,
}

impl LlvmTargetBackend {
    fn initialize(self) {
        match self {
            Self::Aarch64 => Target::initialize_aarch64(&InitializationConfig::default()),
        }
    }
}

/// Closed LLVM machine pipeline whose statepoint behavior was qualified for
/// this compiler.  The ordinary inkwell `Target::create_target_machine` API
/// selects LLVM 22.1's standard SelectionDAG pipeline; keeping that operation
/// behind this capability prevents callers from silently substituting another
/// instruction selector or a custom machine-pass pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MachinePipeline {
    Llvm22SelectionDagStandard,
}

impl MachinePipeline {
    fn create_target_machine(
        self,
        target: &Target,
        triple: &TargetTriple,
        profile: TargetProfile,
        optimization: OptimizationLevel,
    ) -> Option<TargetMachine> {
        match self {
            Self::Llvm22SelectionDagStandard => target.create_target_machine(
                triple,
                profile.cpu,
                profile.features,
                optimization,
                profile.relocation,
                profile.code_model,
            ),
        }
    }
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

/// LLVM address space used for moving managed references.
///
/// Keeping the value in a dedicated type makes it impossible for mechanical
/// lowering to substitute an unrelated integer or silently truncate a wider
/// profile field when constructing an inkwell pointer type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ManagedAddressSpace(u16);

impl ManagedAddressSpace {
    const MOVING_GC: Self = Self(1);

    #[cfg(test)]
    pub(crate) const fn for_test(value: u16) -> Self {
        Self(value)
    }

    pub(crate) fn inkwell(self) -> AddressSpace {
        AddressSpace::from(self.0)
    }

    pub(crate) fn llvm(self) -> u32 {
        u32::from(self.0)
    }
}

/// A complete, immutable target/backend profile selected by the driver.
///
/// Fields are private so callers cannot construct a contradictory partial
/// profile.  New targets must be added through [`TargetProfile::resolve`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetProfile {
    lir_target_profile: LirTargetProfile,
    canonical_triple: &'static str,
    cpu: &'static str,
    features: &'static str,
    optimization: OptimizationLevel,
    relocation: RelocMode,
    code_model: CodeModel,
    object_format: ObjectFormat,
    eh: EhProfile,
    llvm_target_backend: LlvmTargetBackend,
    machine_pipeline: MachinePipeline,
    managed_address_space: ManagedAddressSpace,
    stack_map_version: u8,
    statepoint_roots: StatepointRootPolicy,
    frame_pointers: FramePointerPolicy,
    tail_calls: TailCallPolicy,
    runtime_sources: &'static [&'static str],
    runtime_c_flags: &'static [&'static str],
    linker_args: &'static [&'static str],
}

impl TargetProfile {
    const DARWIN_AARCH64: Self = Self {
        lir_target_profile: LirTargetProfile::DARWIN_AARCH64,
        canonical_triple: "aarch64-apple-darwin",
        cpu: "generic",
        features: "",
        optimization: OptimizationLevel::None,
        relocation: RelocMode::PIC,
        code_model: CodeModel::Default,
        object_format: ObjectFormat::MachO64,
        eh: EhProfile::DARWIN_AARCH64,
        llvm_target_backend: LlvmTargetBackend::Aarch64,
        machine_pipeline: MachinePipeline::Llvm22SelectionDagStandard,
        managed_address_space: ManagedAddressSpace::MOVING_GC,
        stack_map_version: 3,
        statepoint_roots: StatepointRootPolicy::StackIndirectOnly,
        frame_pointers: FramePointerPolicy::All,
        tail_calls: TailCallPolicy::Disabled,
        runtime_sources: &[
            "runtime/src/rt.c",
            "runtime/src/eh.c",
            "runtime/src/eh_personality.c",
            "runtime/src/initialization.c",
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
        linker_args: &["-pthread"],
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

    /// Resolve LLVM's canonical host triple through the same closed registry
    /// used for explicit triples. The driver does not reconstruct target
    /// identity from Rust host constants.
    pub fn resolve_host() -> Result<Self, CodegenError> {
        let triple = TargetMachine::get_default_triple();
        let triple = triple
            .as_str()
            .to_str()
            .map_err(|error| CodegenError(format!("host target triple is not UTF-8: {error}")))?;
        Self::resolve(triple)
    }

    pub fn id(self) -> TargetProfileId {
        self.lir_target_profile.id()
    }

    /// Complete LIR-facing target capabilities selected by this backend
    /// profile. The driver passes this exact value into LIR lowering; codegen
    /// later requires the finished module to carry the same value.
    pub const fn lir_target_profile(self) -> LirTargetProfile {
        self.lir_target_profile
    }

    pub fn canonical_triple(self) -> &'static str {
        self.canonical_triple
    }

    pub fn managed_address_space(self) -> u32 {
        self.managed_address_space.llvm()
    }

    pub(crate) fn managed_address_space_contract(self) -> ManagedAddressSpace {
        self.managed_address_space
    }

    #[cfg(test)]
    pub(crate) fn eh_profile(self) -> EhProfile {
        self.eh
    }

    pub(crate) fn frame_pointer_attribute(self) -> &'static str {
        match self.frame_pointers {
            FramePointerPolicy::All => "all",
        }
    }

    pub(crate) fn disable_tail_calls_attribute(self) -> &'static str {
        match self.tail_calls {
            TailCallPolicy::Disabled => "true",
        }
    }

    #[cfg(test)]
    pub(crate) fn with_managed_address_space_for_test(mut self, value: u16) -> Self {
        self.managed_address_space = ManagedAddressSpace::for_test(value);
        self
    }

    pub fn stack_map_version(self) -> u8 {
        self.stack_map_version
    }

    /// Runtime implementation files selected by this target profile.
    pub fn runtime_sources(self) -> &'static [&'static str] {
        self.runtime_sources
    }

    /// Mandatory C compiler flags for this profile's runtime source bundle.
    pub fn runtime_c_flags(self) -> &'static [&'static str] {
        self.runtime_c_flags
    }

    /// Mandatory target/platform arguments for the final system link.
    pub fn linker_args(self) -> &'static [&'static str] {
        self.linker_args
    }

    pub(crate) fn create_target_machine(self) -> Result<TargetMachine, CodegenError> {
        self.create_target_machine_with_optimization(self.optimization)
    }

    pub(crate) fn validate_lir_target_profile(
        self,
        actual: LirTargetProfile,
    ) -> Result<(), CodegenError> {
        if actual == self.lir_target_profile {
            return Ok(());
        }
        Err(CodegenError(format!(
            "LIR target profile `{}` does not match codegen target profile `{}`",
            actual.id().canonical_name(),
            self.id().canonical_name(),
        )))
    }

    /// Validate the emitted object through the artifact contract selected by
    /// this complete profile. Generic codegen never chooses an object format
    /// or stack-map decoder on its own.
    pub(crate) fn verify_object(
        self,
        path: &Path,
        expected_safepoints: &ExpectedSafepoints,
        expected_eh: &artifact::ExpectedEh,
    ) -> Result<(), CodegenError> {
        match (self.object_format, self.statepoint_roots) {
            (ObjectFormat::MachO64, StatepointRootPolicy::StackIndirectOnly) => {
                artifact::verify_macho_artifact(
                    path,
                    expected_safepoints,
                    expected_eh,
                    self.stack_map_version,
                    self.eh.encodings(),
                )
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn create_qualification_target_machine(
        self,
        optimization: OptimizationLevel,
    ) -> Result<TargetMachine, CodegenError> {
        self.create_target_machine_with_optimization(optimization)
    }

    fn create_target_machine_with_optimization(
        self,
        optimization: OptimizationLevel,
    ) -> Result<TargetMachine, CodegenError> {
        self.llvm_target_backend.initialize();
        let triple = TargetTriple::create(self.canonical_triple);
        let target = Target::from_triple(&triple).map_err(|error| {
            CodegenError(format!("no target for {}: {error}", self.canonical_triple))
        })?;
        let machine = self
            .machine_pipeline
            .create_target_machine(&target, &triple, self, optimization)
            .ok_or_else(|| {
                CodegenError(format!(
                    "failed to create target machine for {}",
                    self.canonical_triple
                ))
            })?;
        qualification::validate_target_machine(self, &machine)?;
        Ok(machine)
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
mod tests;
