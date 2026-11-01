use super::*;

mod callables;
mod emit;
mod finalization;
mod render;
use emit::emit_object_set_with_production;
pub use render::render_llvm_ir_members;

/// One emitted Scoop object and its exact producer units.
#[derive(Debug)]
pub struct EmittedConeObjectMemberV1 {
    pub units: ScoopLirObjectUnitSetV1,
    pub bytes: Vec<u8>,
    pub kind: EmittedConeObjectMemberKindV1,
}

/// The ordinary producer kind for one emitted object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmittedConeObjectMemberKindV1 {
    NonCallable,
    CallableBody {
        body: scoop_lir::PersistentCallableBodyId,
    },
}

/// Complete owned Scoop objects and production records for `.slib` packaging.
#[derive(Debug)]
pub struct EmittedConeObjectSet<P> {
    pub optimization: scoop_lir::OptimizationMode,
    pub target_selection: scoop_lir::ValidatedLirTargetSelection,
    pub foundation: scoop_lir::ConeLirFoundation,
    pub production: P,
    pub partition: ScoopLirObjectPartitionV1,
    pub members: Vec<EmittedConeObjectMemberV1>,
}

pub type EmittedConeObjectSetV1 = EmittedConeObjectSet<scoop_lir::ConeProductionSectionV1>;
pub type EmittedConeObjectSetV2 = EmittedConeObjectSet<scoop_lir::ConeProductionSectionV2>;

impl<P> EmittedConeObjectSet<P> {
    pub const fn target(&self) -> scoop_lir::LirTargetProfile {
        self.target_selection.target()
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
    profile: ValidatedBackendProfile,
    jobs: std::num::NonZeroUsize,
) -> Result<EmittedConeObjectSetV1, CodegenError> {
    profile.validate_lir_target_profile(input.module().meta.target_profile)?;
    let production = input
        .build_production_section(coordinate.clone(), direct_dependencies, entry_source)
        .map_err(|error| {
            CodegenError(format!("cannot build strong production section: {error}"))
        })?;
    emit_object_set_with_production(input, production, profile, jobs)
}

/// Emits the complete LIR and Strong V2 production records for one Cone.
pub fn emit_object_set_v2(
    input: &scoop_lir::ConeLirOutput,
    production: scoop_lir::ConeProductionSectionV2,
    profile: ValidatedBackendProfile,
    jobs: std::num::NonZeroUsize,
) -> Result<EmittedConeObjectSetV2, CodegenError> {
    profile.validate_lir_target_profile(input.module().meta.target_profile)?;
    emit_object_set_with_production(input, production, profile, jobs)
}
