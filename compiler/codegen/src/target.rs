//! Closed target/backend profiles accepted by Scoop codegen.
//!
//! Target choices stay in this projection; mechanical LIR lowering does not
//! discover a host platform or silently inherit LLVM defaults.

use std::fmt;
use std::path::Path;

use inkwell::llvm_sys::core::LLVMGetVersion;
use inkwell::targets::{CodeModel, RelocMode, Target, TargetMachine, TargetTriple};
use inkwell::{AddressSpace, OptimizationLevel};
pub use scoop_lir::TargetProfileId;
use scoop_lir::{BackendProfile, LirTargetProfile, ValidatedLirTargetSelection};

use crate::statepoint::ExpectedSafepoints;
use crate::{CodegenError, artifact};

mod eh;
mod machine;
mod qualification;
#[cfg(test)]
use eh::{EhArtifactInspection, PersonalityAbi, UnwindModel, UnwindProvider};
pub(crate) use eh::{EhProfile, LsdaEncodingProfile};

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

use scoop_lir::NativeObjectFormat as ObjectFormat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CodeArchitecture {
    Aarch64,
    X86_64,
}

impl CodeArchitecture {
    fn initialize(self) {
        match self {
            Self::Aarch64 => Target::initialize_aarch64(&Default::default()),
            Self::X86_64 => Target::initialize_x86(&Default::default()),
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
        profile: ValidatedBackendProfile,
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
    pub(crate) const MOVING_GC: Self = Self(1);

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

/// A complete, immutable LIR-to-LLVM backend projection.
///
/// Fields are private so callers cannot construct a contradictory partial
/// profile. New targets are admitted only by the shared toolchain registry and
/// this module's closed selection-to-backend refinement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedBackendProfile {
    lir_target_selection: ValidatedLirTargetSelection,
    canonical_triple: &'static str,
    cpu: &'static str,
    features: &'static str,
    optimization: scoop_lir::OptimizationMode,
    relocation: RelocMode,
    code_model: CodeModel,
    object_format: ObjectFormat,
    writable_storage_section: &'static str,
    zero_fill_storage_section: &'static str,
    c_string_section: &'static str,
    eh: EhProfile,
    llvm_target_backend: CodeArchitecture,
    machine_pipeline: MachinePipeline,
    managed_address_space: ManagedAddressSpace,
    stack_map_version: u8,
    statepoint_roots: StatepointRootPolicy,
    frame_pointers: FramePointerPolicy,
    tail_calls: TailCallPolicy,
}

impl ValidatedBackendProfile {
    const DARWIN_AARCH64: Self = Self {
        lir_target_selection: ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        canonical_triple: "aarch64-apple-darwin",
        cpu: "generic",
        features: "",
        optimization: scoop_lir::OptimizationMode::Debug,
        relocation: RelocMode::PIC,
        code_model: CodeModel::Default,
        object_format: ObjectFormat::MachO64,
        writable_storage_section: "__DATA,__data",
        zero_fill_storage_section: "__DATA,__bss",
        c_string_section: "__TEXT,__cstring,cstring_literals",
        eh: EhProfile::DARWIN_AARCH64,
        llvm_target_backend: CodeArchitecture::Aarch64,
        machine_pipeline: MachinePipeline::Llvm22SelectionDagStandard,
        managed_address_space: ManagedAddressSpace::MOVING_GC,
        stack_map_version: 3,
        statepoint_roots: StatepointRootPolicy::StackIndirectOnly,
        frame_pointers: FramePointerPolicy::All,
        tail_calls: TailCallPolicy::Disabled,
    };

    pub fn from_selection(selection: ValidatedLirTargetSelection) -> Result<Self, CodegenError> {
        validate_linked_llvm()?;
        Ok(match selection.target().id() {
            TargetProfileId::DarwinAarch64 => Self::DARWIN_AARCH64,
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => Self {
                lir_target_selection: selection,
                canonical_triple: selection.target().id().canonical_triple(),
                cpu: "x86-64",
                object_format: ObjectFormat::Elf64,
                writable_storage_section: ".data",
                zero_fill_storage_section: ".bss",
                c_string_section: ".rodata.str1.1",
                eh: EhProfile::LINUX_X86_64,
                llvm_target_backend: CodeArchitecture::X86_64,
                ..Self::DARWIN_AARCH64
            },
        })
    }

    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.optimization = mode;
        self
    }

    pub const fn optimization(self) -> scoop_lir::OptimizationMode {
        self.optimization
    }

    pub fn id(self) -> TargetProfileId {
        self.lir_target_selection.target().id()
    }

    /// Complete LIR-facing target capabilities selected by this backend
    /// profile. The driver passes this exact value into LIR lowering; codegen
    /// later requires the finished module to carry the same value.
    pub const fn lir_target_profile(self) -> LirTargetProfile {
        self.lir_target_selection.target()
    }

    pub const fn backend_profile(self) -> BackendProfile {
        self.lir_target_selection.backend()
    }

    /// The exact closed LIR/backend selection represented by this backend
    /// projection.
    pub const fn lir_target_selection(self) -> ValidatedLirTargetSelection {
        self.lir_target_selection
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

    pub(crate) const fn writable_storage_section(self) -> &'static str {
        self.writable_storage_section
    }

    pub(crate) const fn zero_fill_storage_section(self) -> &'static str {
        self.zero_fill_storage_section
    }

    pub(crate) const fn read_only_metadata_section(self) -> &'static str {
        match self.object_format {
            ObjectFormat::MachO64 => "__DATA_CONST,__const",
            ObjectFormat::Elf64 => ".data.rel.ro.scoop.metadata",
        }
    }

    pub(crate) const fn c_string_section(self) -> &'static str {
        self.c_string_section
    }

    /// Closed backend projection used by codegen tests that deliberately do
    /// not require C-bridge, runtime-build, or final-link capabilities.
    #[cfg(test)]
    pub(crate) const fn darwin_aarch64_for_test() -> Self {
        Self::DARWIN_AARCH64
    }

    pub(crate) fn eh_profile(self) -> EhProfile {
        self.eh
    }

    pub(crate) fn architecture(self) -> CodeArchitecture {
        self.llvm_target_backend
    }

    pub(crate) fn disable_red_zone(self) -> bool {
        self.llvm_target_backend == CodeArchitecture::X86_64
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

    pub(crate) fn create_target_machine(self) -> Result<TargetMachine, CodegenError> {
        self.create_target_machine_with_optimization(match self.optimization {
            scoop_lir::OptimizationMode::Debug => OptimizationLevel::None,
            scoop_lir::OptimizationMode::Release => OptimizationLevel::Default,
        })
    }

    pub(crate) fn validate_lir_target_profile(
        self,
        actual: LirTargetProfile,
    ) -> Result<(), CodegenError> {
        if actual == self.lir_target_selection.target() {
            return Ok(());
        }
        Err(CodegenError(format!(
            "LIR target profile `{}` does not match codegen target profile `{}`",
            actual.id().canonical_name(),
            self.id().canonical_name(),
        )))
    }

    /// Validate the emitted object through the artifact contract selected by
    /// this backend projection. Generic codegen never chooses an object format
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
            (ObjectFormat::Elf64, StatepointRootPolicy::StackIndirectOnly) => {
                artifact::verify_elf_artifact(path, expected_safepoints, expected_eh, self)
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
        machine::initialize_options();
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

#[cfg(test)]
mod tests;
