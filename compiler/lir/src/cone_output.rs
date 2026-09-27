//! Complete LIR output for the shared physical lowering path.

use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity, SourceDeclarationKey};

use crate::{
    ConeLirFoundation, ConeLirFoundationProjectionError, EntryProductionSourceV1, Module,
    StrongProductionSectionBuildError, StrongProductionSectionV1, StrongProductionSectionV2,
    StrongRegistrationProductionBuildError, StrongRegistrationProductionSurfaceV1,
};

mod shape_support;
pub use shape_support::{
    StrongLirBoxedValueMaterialization, StrongLirExactShapeMaterialization,
    StrongLirShapeSupportError, StrongLirShapeSupportPlan, StrongLirShapeSupportRoot,
};

/// A complete LIR graph with its identity foundation and shape metadata.
pub struct ConeLirOutput {
    module: Module,
    foundation: ConeLirFoundation,
    shape_support: StrongLirShapeSupportPlan,
}

impl ConeLirOutput {
    pub fn try_new(
        module: Module,
        shape_sources: Vec<SourceDeclarationKey>,
    ) -> Result<Self, ConeLirOutputError> {
        let foundation =
            ConeLirFoundation::from_module(&module).map_err(ConeLirOutputError::Foundation)?;
        let shape_support = StrongLirShapeSupportPlan::from_module(&module, shape_sources)
            .map_err(ConeLirOutputError::ShapeSupport)?;
        Ok(Self {
            module,
            foundation,
            shape_support,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn foundation(&self) -> &ConeLirFoundation {
        &self.foundation
    }

    pub const fn shape_support(&self) -> &StrongLirShapeSupportPlan {
        &self.shape_support
    }

    /// Builds the complete member-independent production section from this
    /// exact sealed graph/foundation pair.
    pub fn build_production_section(
        &self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        entry_source: EntryProductionSourceV1,
    ) -> Result<StrongProductionSectionV1, StrongProductionWriterError> {
        let (digests, registrations) = StrongRegistrationProductionSurfaceV1::from_module(
            &self.module,
            &self.foundation,
            &entry_source,
        )
        .map_err(StrongProductionWriterError::Registrations)?;
        StrongProductionSectionV1::new(
            coordinate,
            direct_dependencies,
            &self.foundation,
            digests,
            registrations,
            entry_source,
            &self.shape_support.source_declarations(),
        )
        .map_err(StrongProductionWriterError::Section)
    }

    /// Projects the complete V2 registration and object plans from LIR.
    /// The layout producer joins these records with its actual exports.
    pub fn build_production_section_v2(
        &self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        entry_source: EntryProductionSourceV1,
        external_initialization_uses: &[crate::StrongExternalInitializationUseV2],
    ) -> Result<StrongProductionSectionV2, StrongProductionWriterError> {
        let (digests, registrations) = crate::StrongRegistrationProductionSurfaceV2::from_module(
            &self.module,
            &self.foundation,
            &entry_source,
            external_initialization_uses,
        )
        .map_err(StrongProductionWriterError::Registrations)?;
        StrongProductionSectionV2::new(
            coordinate,
            direct_dependencies,
            &self.foundation,
            digests,
            registrations,
            entry_source,
            &self.shape_support.source_declarations(),
        )
        .map_err(StrongProductionWriterError::Section)
    }

    pub fn into_module(self) -> Module {
        self.module
    }
}

#[derive(Debug)]
pub enum ConeLirOutputError {
    Foundation(ConeLirFoundationProjectionError),
    ShapeSupport(StrongLirShapeSupportError),
}

impl fmt::Display for ConeLirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(source) => source.fmt(formatter),
            Self::ShapeSupport(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ConeLirOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Foundation(source) => source,
            Self::ShapeSupport(source) => source,
        })
    }
}

#[derive(Debug)]
pub enum StrongProductionWriterError {
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
