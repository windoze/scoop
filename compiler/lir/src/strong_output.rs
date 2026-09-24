//! Sealed LIR output for the single-Cone strong production path.

use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity, SourceDeclarationKey};
use scoop_wire::BudgetMeter;

use crate::{
    CallableAbiRecordV1, CallableAbiValidationError, EntryProductionSourceV1, Module,
    OdrFreeLirFoundation, OdrFreeLirFoundationProjectionError, StrongDigestProjectionError,
    StrongExternalLirBridgeBuildError, StrongExternalLirBridgeSurfaceV1,
    StrongProductionSectionBuildError, StrongProductionSectionV1, StrongProductionSectionV2,
    StrongRegistrationProductionBuildError, StrongRegistrationProductionSurfaceV1,
    project_strong_digest_finalization_plan, project_strong_digest_finalization_plan_v2,
};

mod shape_support;
pub use shape_support::{
    StrongLirBoxedValueMaterialization, StrongLirExactShapeMaterialization,
    StrongLirShapeSupportError, StrongLirShapeSupportPlan, StrongLirShapeSupportRoot,
};

/// One LIR graph paired with the exact ODR-free foundation projected from it.
///
/// The fields are private so production orchestration cannot replace either
/// half after the strong lowering gate has succeeded.
pub struct SingleConeStrongLirOutput {
    module: Module,
    foundation: OdrFreeLirFoundation,
    shape_support: StrongLirShapeSupportPlan,
    initialization_cycle_abi: Option<Box<CallableAbiRecordV1>>,
}

/// A freshly projected V2 section awaiting the complete local and selected
/// layout/ABI join. It exposes the registration plans needed to construct
/// local layout exports, but it cannot be encoded or published as a wire
/// section.
pub struct PendingStrongProductionSectionV2 {
    section: StrongProductionSectionV2,
}

impl PendingStrongProductionSectionV2 {
    pub const fn registration_production(&self) -> &crate::StrongRegistrationProductionSurfaceV2 {
        self.section.registration_production()
    }

    pub fn validate_layout_abi(
        self,
        layout_abi: &crate::CrossConeLayoutAbiSectionV1<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<crate::ValidatedStrongProductionSectionV2, crate::StrongProductionLayoutJoinError>
    {
        self.section.validate_layout_abi(layout_abi, meter)
    }
}

impl SingleConeStrongLirOutput {
    pub fn try_new(
        module: Module,
        shape_sources: Vec<SourceDeclarationKey>,
        initialization_cycle_abi: Option<Box<CallableAbiRecordV1>>,
    ) -> Result<Self, SingleConeStrongLirOutputError> {
        let foundation = OdrFreeLirFoundation::from_module(&module)
            .map_err(SingleConeStrongLirOutputError::Foundation)?;
        let shape_support = StrongLirShapeSupportPlan::from_module(&module, shape_sources)
            .map_err(SingleConeStrongLirOutputError::ShapeSupport)?;
        crate::validate_initialization_abi(
            initialization_cycle_abi.as_deref(),
            &foundation,
            &crate::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation)
                .map_err(SingleConeStrongLirOutputError::CallableDefinitions)?,
        )
        .map_err(SingleConeStrongLirOutputError::InitializationAbi)?;
        Ok(Self {
            module,
            foundation,
            shape_support,
            initialization_cycle_abi,
        })
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundation
    }

    pub const fn shape_support(&self) -> &StrongLirShapeSupportPlan {
        &self.shape_support
    }

    pub fn initialization_cycle_abi(&self) -> Option<&CallableAbiRecordV1> {
        self.initialization_cycle_abi.as_deref()
    }

    /// Builds the complete member-independent production section from this
    /// exact sealed graph/foundation pair.
    pub fn build_production_section(
        &self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
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
            direct_dependencies,
            &self.foundation,
            external_bridges,
            digests,
            registrations,
            entry_source,
            &self.shape_support.source_declarations(),
            self.initialization_cycle_abi.clone(),
        )
        .map_err(StrongProductionWriterError::Section)
    }

    /// Projects `strong-production/4` from the final LIR graph. The returned
    /// pending section supplies local registration inputs to the layout/ABI
    /// producer and becomes publishable only after `validate_layout_abi`.
    pub fn build_production_section_v2(
        &self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        entry_source: EntryProductionSourceV1,
        selected: &crate::StrongProductionDependencySelectionV2<'_>,
        external_initialization_uses: &[crate::StrongExternalInitializationUseV2],
        meter: &mut BudgetMeter,
    ) -> Result<PendingStrongProductionSectionV2, StrongProductionWriterError> {
        let external_bridges = StrongExternalLirBridgeSurfaceV1::from_module(&self.module)
            .map_err(StrongProductionWriterError::ExternalBridges)?;
        let digests = project_strong_digest_finalization_plan_v2(
            &self.module,
            &self.foundation,
            &entry_source,
            selected,
            meter,
        )
        .map_err(StrongProductionWriterError::Digests)?;
        let registrations = crate::StrongRegistrationProductionSurfaceV2::from_module(
            &self.module,
            &self.foundation,
            &digests,
            selected,
            external_initialization_uses,
            meter,
        )
        .map_err(StrongProductionWriterError::Registrations)?;
        StrongProductionSectionV2::new(
            coordinate,
            direct_dependencies,
            &self.foundation,
            external_bridges,
            digests,
            registrations,
            entry_source,
            &self.shape_support.source_declarations(),
            self.initialization_cycle_abi.clone(),
        )
        .map(|section| PendingStrongProductionSectionV2 { section })
        .map_err(StrongProductionWriterError::Section)
    }

    pub fn into_module(self) -> Module {
        self.module
    }
}

#[derive(Debug)]
pub enum SingleConeStrongLirOutputError {
    Foundation(OdrFreeLirFoundationProjectionError),
    ShapeSupport(StrongLirShapeSupportError),
    CallableDefinitions(crate::StrongObjectSymbolSurfaceBuildError),
    InitializationAbi(CallableAbiValidationError),
}

impl fmt::Display for SingleConeStrongLirOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Foundation(source) => source.fmt(formatter),
            Self::ShapeSupport(source) => source.fmt(formatter),
            Self::CallableDefinitions(source) => source.fmt(formatter),
            Self::InitializationAbi(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConeStrongLirOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Foundation(source) => source,
            Self::ShapeSupport(source) => source,
            Self::CallableDefinitions(source) => source,
            Self::InitializationAbi(source) => source,
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
