//! Closed LIR production section for the M23-3 strong-only profile.

use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity, SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use crate::{
    ConeLirFoundation, DecodedConeImagePlanV1, DecodedEntryProductionPlanV1,
    DecodedGeneratedBridgePlanSetV1, DecodedParamFreeShapeSupportPlanSetV1,
    DecodedStrongDigestFinalizationPlanV1, DecodedStrongObjectDefinitionPlanSurfaceV1,
    DecodedStrongObjectSymbolSurfaceV1, DecodedStrongRegistrationProductionSurfaceV1,
    DigestPlanError, EntryProductionPlanBuildError, EntryProductionPlanV1, EntryProductionSourceV1,
    GeneratedBridgePlanBuildError, GeneratedBridgePlanSetV1, ParamFreeShapeSupportBuildError,
    ParamFreeShapeSupportPlanSetV1, StrongDigestFinalizationPlanV1,
    StrongDigestPlanValidationError, StrongObjectDefinitionPlanBuildError,
    StrongObjectDefinitionPlanSurfaceV1, StrongObjectSymbolSurfaceBuildError,
    StrongObjectSymbolSurfaceV1, StrongRegistrationProductionSurfaceV1,
    StrongRegistrationProductionValidationError,
};

use crate::{ConeImagePlanBuildError, ConeImagePlanV1};

pub type StrongProductionSectionV1 = StrongProductionSection<
    crate::StrongTypeDescriptorRefV1,
    crate::StrongTypeDispatchCallableRefV1,
    scoop_identity::PersistentInitializationUnitId,
>;
pub type StrongProductionSectionV2 = StrongProductionSection<
    crate::StrongTypeDescriptorRefV2,
    crate::StrongTypeDispatchCallableRefV2,
    crate::StrongInitializationDependencyRefV2,
>;

/// Every canonical LIR production input required by the strong-only profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongProductionSection<D, C, I> {
    canonical_definitions: StrongObjectSymbolSurfaceV1,
    object_definition_plans: StrongObjectDefinitionPlanSurfaceV1,
    digest_finalization_plan: StrongDigestFinalizationPlanV1,
    registration_production: crate::StrongRegistrationProductionSurface<D, C, I>,
    image_plan: ConeImagePlanV1,
    entry_plan: EntryProductionPlanV1,
    shape_support_plan: ParamFreeShapeSupportPlanSetV1,
    generated_bridge_plan: GeneratedBridgePlanSetV1,
}

impl StrongProductionSectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        digest_finalization_plan: StrongDigestFinalizationPlanV1,
        registration_production: StrongRegistrationProductionSurfaceV1,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
    ) -> Result<Self, StrongProductionSectionBuildError> {
        Self::from_parts(
            coordinate,
            direct_dependencies,
            foundation,
            digest_finalization_plan,
            registration_production,
            entry_source,
            shape_sources,
        )
    }
}

impl StrongProductionSectionV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        digest_finalization_plan: StrongDigestFinalizationPlanV1,
        registration_production: crate::StrongRegistrationProductionSurfaceV2,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
    ) -> Result<Self, StrongProductionSectionBuildError> {
        Self::from_parts(
            coordinate,
            direct_dependencies,
            foundation,
            digest_finalization_plan,
            registration_production,
            entry_source,
            shape_sources,
        )
    }
}

impl<D, C, I> StrongProductionSection<D, C, I> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        digest_finalization_plan: StrongDigestFinalizationPlanV1,
        registration_production: crate::StrongRegistrationProductionSurface<D, C, I>,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
    ) -> Result<Self, StrongProductionSectionBuildError> {
        digest_finalization_plan
            .validate_against(foundation)
            .map_err(StrongProductionSectionBuildError::DigestPlan)?;
        let object_definition_plans =
            StrongObjectDefinitionPlanSurfaceV1::from_foundation(foundation)
                .map_err(StrongProductionSectionBuildError::ObjectDefinitions)?;
        let canonical_definitions = StrongObjectSymbolSurfaceV1::from_definition_plans(
            foundation,
            &object_definition_plans,
        )
        .map_err(StrongProductionSectionBuildError::CanonicalDefinitions)?;
        let image_plan = ConeImagePlanV1::new(
            coordinate,
            direct_dependencies,
            foundation,
            registration_production.identities(),
            &digest_finalization_plan,
        )
        .map_err(StrongProductionSectionBuildError::Image)?;
        let entry_plan = EntryProductionPlanV1::new(
            entry_source,
            foundation,
            registration_production.identities(),
            &digest_finalization_plan,
        )
        .map_err(StrongProductionSectionBuildError::Entry)?;
        let shape_support_plan = ParamFreeShapeSupportPlanSetV1::from_sources(
            shape_sources.iter(),
            foundation,
            registration_production.identities(),
        )
        .map_err(StrongProductionSectionBuildError::ShapeSupport)?;
        let generated_bridge_plan = GeneratedBridgePlanSetV1::from_foundation(foundation)
            .map_err(StrongProductionSectionBuildError::GeneratedBridges)?;
        Ok(Self {
            canonical_definitions,
            object_definition_plans,
            digest_finalization_plan,
            registration_production,
            image_plan,
            entry_plan,
            shape_support_plan,
            generated_bridge_plan,
        })
    }

    pub const fn canonical_definitions(&self) -> &StrongObjectSymbolSurfaceV1 {
        &self.canonical_definitions
    }

    pub const fn object_definition_plans(&self) -> &StrongObjectDefinitionPlanSurfaceV1 {
        &self.object_definition_plans
    }

    pub const fn digest_finalization_plan(&self) -> &StrongDigestFinalizationPlanV1 {
        &self.digest_finalization_plan
    }

    pub const fn registration_production(
        &self,
    ) -> &crate::StrongRegistrationProductionSurface<D, C, I> {
        &self.registration_production
    }

    pub const fn image_plan(&self) -> &ConeImagePlanV1 {
        &self.image_plan
    }

    pub const fn entry_plan(&self) -> &EntryProductionPlanV1 {
        &self.entry_plan
    }

    pub const fn shape_support_plan(&self) -> &ParamFreeShapeSupportPlanSetV1 {
        &self.shape_support_plan
    }

    pub const fn generated_bridge_plan(&self) -> &GeneratedBridgePlanSetV1 {
        &self.generated_bridge_plan
    }
}

impl<D: crate::StrongDescriptorReference, C: Clone + WireEncode, I: WireEncode> WireEncode
    for StrongProductionSection<D, C, I>
{
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(2)?;
        self.canonical_definitions.encode(encoder)?;
        encoder.field(3)?;
        self.object_definition_plans.encode(encoder)?;
        encoder.field(4)?;
        self.digest_finalization_plan.encode(encoder)?;
        encoder.field(5)?;
        self.registration_production.encode(encoder)?;
        encoder.field(6)?;
        self.image_plan.encode(encoder)?;
        encoder.field(7)?;
        self.entry_plan.encode(encoder)?;
        encoder.field(8)?;
        self.shape_support_plan.encode(encoder)?;
        encoder.field(9)?;
        self.generated_bridge_plan.encode(encoder)
    }
}

mod decoded;
pub use decoded::*;

mod registrations;
mod replay;
pub use replay::StrongProductionLayoutJoinError;

impl DecodedStrongProductionSectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        target: crate::LirTargetProfile,
        foundation: &ConeLirFoundation,

        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<StrongProductionSectionV1, StrongProductionSectionValidationError> {
        let actual = encode(&self).map_err(StrongProductionSectionValidationError::Encode)?;
        let digest_finalization_plan = self
            .digest_finalization_plan
            .validate(identities, foundation)
            .map_err(StrongProductionSectionValidationError::DigestPlan)?;
        let registration_production = self
            .registration_production
            .validate(target, foundation, &digest_finalization_plan)
            .map_err(StrongProductionSectionValidationError::Registrations)?;
        let expected = StrongProductionSectionV1::new(
            coordinate,
            direct_dependencies,
            foundation,
            digest_finalization_plan,
            registration_production,
            entry_source,
            shape_sources,
        )
        .map_err(StrongProductionSectionValidationError::Expected)?;
        let expected_bytes =
            encode(&expected).map_err(StrongProductionSectionValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(StrongProductionSectionValidationError::SectionMismatch);
        }
        Ok(expected)
    }
}

#[derive(Debug)]
pub enum StrongProductionSectionBuildError {
    CanonicalDefinitions(StrongObjectSymbolSurfaceBuildError),
    ObjectDefinitions(StrongObjectDefinitionPlanBuildError),
    DigestPlan(DigestPlanError),
    Image(ConeImagePlanBuildError),
    Entry(EntryProductionPlanBuildError),
    ShapeSupport(ParamFreeShapeSupportBuildError),
    GeneratedBridges(GeneratedBridgePlanBuildError),
}

impl fmt::Display for StrongProductionSectionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong production section: {self:?}")
    }
}

impl std::error::Error for StrongProductionSectionBuildError {}

#[derive(Debug)]
pub enum StrongProductionSectionValidationError {
    DigestReplay(Box<crate::StrongDigestPlanReplayError>),
    DigestProjection(Box<crate::StrongDigestProjectionError>),
    DigestMismatch,
    Encode(scoop_wire::cbor::EncodeError),
    Resource(WireError),
    DigestPlan(StrongDigestPlanValidationError),
    Registrations(StrongRegistrationProductionValidationError),
    Expected(StrongProductionSectionBuildError),
    SectionMismatch,
}

impl fmt::Display for StrongProductionSectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong production section: {self:?}"
        )
    }
}

impl std::error::Error for StrongProductionSectionValidationError {}

#[cfg(test)]
pub(in crate::production) mod tests;
