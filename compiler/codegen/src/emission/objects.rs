use super::*;

mod emit;
mod finalization;
mod render;
use emit::emit_object_set_with_production;
pub use render::render_llvm_ir_members;

/// One emitted Scoop object and its exact producer units.
#[derive(Debug)]
pub struct EmittedConeObjectMemberV1 {
    units: ScoopLirObjectUnitSetV1,
    path: std::path::PathBuf,
    kind: EmittedConeObjectMemberKindV1,
}

/// The ordinary producer kind for one emitted object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmittedConeObjectMemberKindV1 {
    NonCallable,
    CallableBody {
        body: scoop_lir::PersistentCallableBodyId,
    },
}

impl EmittedConeObjectMemberV1 {
    pub const fn units(&self) -> &ScoopLirObjectUnitSetV1 {
        &self.units
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn kind(&self) -> EmittedConeObjectMemberKindV1 {
        self.kind
    }
}

/// Complete Scoop object set retained for `.slib`
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

/// Emits complete objects and retains their production records
/// for `.slib` packaging.
pub fn emit_object_set(
    input: &scoop_lir::ConeLirOutput,
    coordinate: &scoop_lir::ConeCoordinate,
    direct_dependencies: &[scoop_lir::ConeIdentity],
    entry_source: scoop_lir::EntryProductionSourceV1,
    temporary_parent: &Path,
    profile: ValidatedBackendProfile,
) -> Result<EmittedConeObjectSetV1, CodegenError> {
    profile.validate_lir_target_profile(input.module().meta.target_profile)?;
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
    profile.validate_lir_target_profile(input.module().meta.target_profile)?;
    emit_object_set_with_production(input, production, temporary_parent, profile)
}
