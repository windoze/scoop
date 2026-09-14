//! Sealed LIR output for the single-Cone strong production path.

use std::fmt;

use scoop_identity::{ConeCoordinate, SourceDeclarationKey};

use crate::{
    EntryProductionSourceV1, Module, OdrFreeLirFoundation, OdrFreeLirFoundationProjectionError,
    StrongDigestProjectionError, StrongExternalLirBridgeBuildError,
    StrongExternalLirBridgeSurfaceV1, StrongProductionSectionBuildError, StrongProductionSectionV1,
    StrongRegistrationProductionBuildError, StrongRegistrationProductionSurfaceV1,
    project_strong_digest_finalization_plan,
};

mod core_shape_support;
pub use core_shape_support::{
    StrongLirBoxedValueMaterialization, StrongLirCoreShapeSupportError,
    StrongLirCoreShapeSupportPlan, StrongLirCoreShapeSupportRoot,
    StrongLirExactShapeMaterialization,
};

/// One LIR graph paired with the exact ODR-free foundation projected from it.
///
/// The fields are private so production orchestration cannot replace either
/// half after the strong lowering gate has succeeded.
pub struct SingleConeStrongLirOutput {
    module: Module,
    foundation: OdrFreeLirFoundation,
    core_shape_support: StrongLirCoreShapeSupportPlan,
}

impl SingleConeStrongLirOutput {
    pub fn try_new(
        module: Module,
        core_shape_sources: Vec<SourceDeclarationKey>,
    ) -> Result<Self, SingleConeStrongLirOutputError> {
        let foundation = OdrFreeLirFoundation::from_module(&module)
            .map_err(SingleConeStrongLirOutputError::Foundation)?;
        let core_shape_support =
            StrongLirCoreShapeSupportPlan::from_module(&module, core_shape_sources)
                .map_err(SingleConeStrongLirOutputError::CoreShapeSupport)?;
        Ok(Self {
            module,
            foundation,
            core_shape_support,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundation
    }

    pub const fn core_shape_support(&self) -> &StrongLirCoreShapeSupportPlan {
        &self.core_shape_support
    }

    /// Builds the complete member-independent production section from this
    /// exact sealed graph/foundation pair.
    pub fn build_production_section(
        &self,
        coordinate: ConeCoordinate,
        entry_source: EntryProductionSourceV1,
    ) -> Result<StrongProductionSectionV1, StrongProductionWriterError> {
        let external_bridges = StrongExternalLirBridgeSurfaceV1::from_module(&self.module)
            .map_err(StrongProductionWriterError::ExternalBridges)?;
        let digests =
            project_strong_digest_finalization_plan(&self.module, &self.foundation, &entry_source)
                .map_err(StrongProductionWriterError::Digests)?;
        let registrations = StrongRegistrationProductionSurfaceV1::from_module(
            &self.module,
            &self.foundation,
            &digests,
        )
        .map_err(StrongProductionWriterError::Registrations)?;
        StrongProductionSectionV1::new(
            coordinate,
            &self.foundation,
            external_bridges,
            digests,
            registrations,
            entry_source,
            &self.core_shape_support.source_declarations(),
        )
        .map_err(StrongProductionWriterError::Section)
    }

    pub fn into_module(self) -> Module {
        self.module
    }
}

#[derive(Debug)]
pub enum SingleConeStrongLirOutputError {
    Foundation(OdrFreeLirFoundationProjectionError),
    CoreShapeSupport(StrongLirCoreShapeSupportError),
}

impl fmt::Display for SingleConeStrongLirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(source) => source.fmt(formatter),
            Self::CoreShapeSupport(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConeStrongLirOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Foundation(source) => source,
            Self::CoreShapeSupport(source) => source,
        })
    }
}

#[derive(Debug)]
pub enum StrongProductionWriterError {
    ExternalBridges(StrongExternalLirBridgeBuildError),
    Digests(StrongDigestProjectionError),
    Registrations(StrongRegistrationProductionBuildError),
    Section(StrongProductionSectionBuildError),
}

impl fmt::Display for StrongProductionWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot build strong production section: {self:?}"
        )
    }
}

impl std::error::Error for StrongProductionWriterError {}
