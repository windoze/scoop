use super::*;

mod emit;
mod finalization;
mod render;
use emit::emit_object_set_with_production;
pub use render::render_llvm_ir_members;

/// One verified provisional Scoop object and its exact producer units.
#[derive(Debug)]
pub struct EmittedConeObjectMemberV1 {
    units: ScoopLirObjectUnitSetV1,
    path: std::path::PathBuf,
    kind: EmittedConeObjectMemberKind,
}

#[derive(Debug)]
enum EmittedConeObjectMemberKind {
    NonCallable {
        runtime_metadata: EmittedStrongRuntimeMetadataV1,
        digest_patches: Vec<EmittedStrongDigestPatchMaterializationV1>,
    },
    CallableBody {
        body: scoop_lir::PersistentCallableBodyId,
        runtime_metadata: EmittedStrongRuntimeMetadataV1,
        digest_patches: Vec<EmittedStrongDigestPatchMaterializationV1>,
    },
}

/// Borrowed typed contents of one verified provisional member.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmittedConeObjectMemberKindV1<'a> {
    NonCallable {
        runtime_metadata: &'a EmittedStrongRuntimeMetadataV1,
        digest_patches: &'a [EmittedStrongDigestPatchMaterializationV1],
    },
    CallableBody {
        body: scoop_lir::PersistentCallableBodyId,
        runtime_metadata: &'a EmittedStrongRuntimeMetadataV1,
        digest_patches: &'a [EmittedStrongDigestPatchMaterializationV1],
    },
}

impl EmittedConeObjectMemberV1 {
    pub const fn units(&self) -> &ScoopLirObjectUnitSetV1 {
        &self.units
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> EmittedConeObjectMemberKindV1<'_> {
        match &self.kind {
            EmittedConeObjectMemberKind::NonCallable {
                runtime_metadata,
                digest_patches,
            } => EmittedConeObjectMemberKindV1::NonCallable {
                runtime_metadata,
                digest_patches,
            },
            EmittedConeObjectMemberKind::CallableBody {
                body,
                runtime_metadata,
                digest_patches,
            } => EmittedConeObjectMemberKindV1::CallableBody {
                body: *body,
                runtime_metadata,
                digest_patches,
            },
        }
    }

    pub fn digest_patches(&self) -> &[EmittedStrongDigestPatchMaterializationV1] {
        match &self.kind {
            EmittedConeObjectMemberKind::NonCallable { digest_patches, .. }
            | EmittedConeObjectMemberKind::CallableBody { digest_patches, .. } => digest_patches,
        }
    }
}

/// Complete verified provisional Scoop object set retained for `.slib`
/// packaging. The owned temporary directory keeps every member immutable and
/// alive for exactly as long as this result.
#[derive(Debug)]
pub struct EmittedConeObjectSet<P> {
    optimization: scoop_lir::OptimizationMode,
    target_selection: scoop_lir::ValidatedLirTargetSelection,
    foundation: scoop_lir::ConeLirFoundation,
    production: P,
    partition: ScoopLirObjectPartitionV1,
    members: Vec<EmittedConeObjectMemberV1>,
    backing: tempfile::TempDir,
}

pub type EmittedConeObjectSetV1 = EmittedConeObjectSet<scoop_lir::ConeProductionSectionV1>;
pub type EmittedConeObjectSetV2 = EmittedConeObjectSet<scoop_lir::ConeProductionSectionV2>;

impl<P> EmittedConeObjectSet<P> {
    pub const fn optimization(&self) -> scoop_lir::OptimizationMode {
        self.optimization
    }

    pub const fn target(&self) -> scoop_lir::LirTargetProfile {
        self.target_selection.target()
    }

    pub const fn target_selection(&self) -> scoop_lir::ValidatedLirTargetSelection {
        self.target_selection
    }

    pub const fn foundation(&self) -> &scoop_lir::ConeLirFoundation {
        &self.foundation
    }

    pub const fn production(&self) -> &P {
        &self.production
    }

    /// Returns the production records after the caller has consumed the
    /// temporary object members.
    pub fn into_production(self) -> P {
        self.production
    }

    pub fn members(&self) -> &[EmittedConeObjectMemberV1] {
        &self.members
    }

    pub const fn partition(&self) -> &ScoopLirObjectPartitionV1 {
        &self.partition
    }

    pub fn temporary_directory(&self) -> &Path {
        self.backing.path()
    }
}

/// One rendered physical member used by diagnostics and golden tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedConeObjectModuleV1 {
    units: ScoopLirObjectUnitSetV1,
    llvm_ir: String,
}

impl RenderedConeObjectModuleV1 {
    pub const fn units(&self) -> &ScoopLirObjectUnitSetV1 {
        &self.units
    }

    pub fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }
}

/// Emits provisional objects and retains their production and patch records
/// for `.slib` packaging.
pub fn emit_object_set(
    input: &scoop_lir::ConeLirOutput,
    coordinate: &scoop_lir::ConeCoordinate,
    direct_dependencies: &[scoop_lir::ConeIdentity],
    entry_source: scoop_lir::EntryProductionSourceV1,
    temporary_parent: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSetV1, CodegenError> {
    validate_object_set_input(input, profile)?;
    let production = input
        .build_production_section(coordinate.clone(), direct_dependencies, entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    emit_object_set_with_production(input, production, temporary_parent, profile)
}

/// Emits the complete LIR and Strong V2 production records for one Cone.
pub fn emit_object_set_v2(
    input: &scoop_lir::ConeLirOutput,
    production: scoop_lir::ConeProductionSectionV2,
    temporary_parent: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSetV2, CodegenError> {
    validate_object_set_input(input, profile)?;
    if production.image_plan().cone().identity() != input.module().cone {
        return Err(CodegenError(
            "strong production belongs to a different Cone than the emitted LIR".to_owned(),
        ));
    }
    emit_object_set_with_production(input, production, temporary_parent, profile)
}

fn validate_object_set_input(
    input: &scoop_lir::ConeLirOutput,
    profile: ValidatedBackendProfile,
) -> Result<(), CodegenError> {
    validation::validate_module(input.module())?;
    profile.validate_lir_target_profile(input.module().meta.target_profile)?;
    if let scoop_lir::LirOutput::Executable { entry } = input.module().output {
        validation::validate_executable_entry(input.module(), entry)?;
    }
    Ok(())
}
