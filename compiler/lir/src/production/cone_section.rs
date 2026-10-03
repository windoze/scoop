//! Complete per-Cone LIR production definitions, registrations and digest plans.

use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity, SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use crate::{
    ConeLirFoundation, DecodedConeImagePlanV1, DecodedDigestFinalizationPlanV1,
    DecodedEntryProductionPlanV1, DecodedGeneratedBridgePlanSetV1,
    DecodedObjectDefinitionPlanSurfaceV1, DecodedObjectSymbolSurfaceV1,
    DecodedParamFreeShapeSupportPlanSetV1, DecodedStrongRegistrationProductionSurfaceV1,
    DigestFinalizationPlanV1, DigestPlanError, DigestPlanValidationError,
    EntryProductionPlanBuildError, EntryProductionPlanV1, EntryProductionSourceV1,
    GeneratedBridgePlanBuildError, GeneratedBridgePlanSetV1, ObjectDefinitionPlanBuildError,
    ObjectDefinitionPlanSurfaceV1, ObjectSymbolSurfaceBuildError, ObjectSymbolSurfaceV1,
    ParamFreeShapeSupportBuildError, ParamFreeShapeSupportPlanSetV1,
    StrongRegistrationProductionSurfaceV1, StrongRegistrationProductionValidationError,
};

use crate::{ConeImagePlanBuildError, ConeImagePlanV1};

pub type ConeProductionSectionV1 = ConeProductionSection<
    crate::StrongTypeDescriptorRefV1,
    crate::StrongTypeDispatchCallableRefV1,
    scoop_identity::PersistentInitializationUnitId,
>;
pub type ConeProductionSectionV2 = ConeProductionSection<
    crate::StrongTypeDescriptorRefV2,
    crate::StrongTypeDispatchCallableRefV2,
    crate::StrongInitializationDependencyRefV2,
>;

/// Canonical production inputs shared by Strong and ODR materializations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeProductionSection<D, C, I> {
    canonical_definitions: ObjectSymbolSurfaceV1,
    object_definition_plans: ObjectDefinitionPlanSurfaceV1,
    digest_finalization_plan: DigestFinalizationPlanV1,
    registration_production: crate::StrongRegistrationProductionSurface<D, C, I>,
    image_plan: ConeImagePlanV1,
    entry_plan: EntryProductionPlanV1,
    shape_support_plan: ParamFreeShapeSupportPlanSetV1,
    generated_bridge_plan: GeneratedBridgePlanSetV1,
    canonical_callables: crate::CanonicalCallableLirDefinitionsV1,
    canonical_shapes: crate::CanonicalShapeLirDefinitionsV1,
}

impl ConeProductionSectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        digest_finalization_plan: DigestFinalizationPlanV1,
        registration_production: StrongRegistrationProductionSurfaceV1,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
        canonical_callables: crate::CanonicalCallableLirDefinitionsV1,
        canonical_shapes: crate::CanonicalShapeLirDefinitionsV1,
    ) -> Result<Self, ConeProductionSectionBuildError> {
        Self::from_parts(
            coordinate,
            direct_dependencies,
            foundation,
            digest_finalization_plan,
            registration_production,
            entry_source,
            shape_sources,
            canonical_callables,
            canonical_shapes,
        )
    }
}

impl ConeProductionSectionV2 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        digest_finalization_plan: DigestFinalizationPlanV1,
        registration_production: crate::StrongRegistrationProductionSurfaceV2,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
        canonical_callables: crate::CanonicalCallableLirDefinitionsV1,
        canonical_shapes: crate::CanonicalShapeLirDefinitionsV1,
    ) -> Result<Self, ConeProductionSectionBuildError> {
        Self::from_parts(
            coordinate,
            direct_dependencies,
            foundation,
            digest_finalization_plan,
            registration_production,
            entry_source,
            shape_sources,
            canonical_callables,
            canonical_shapes,
        )
    }
}

impl<D, C, I> ConeProductionSection<D, C, I> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        foundation: &ConeLirFoundation,
        digest_finalization_plan: DigestFinalizationPlanV1,
        registration_production: crate::StrongRegistrationProductionSurface<D, C, I>,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
        canonical_callables: crate::CanonicalCallableLirDefinitionsV1,
        canonical_shapes: crate::CanonicalShapeLirDefinitionsV1,
    ) -> Result<Self, ConeProductionSectionBuildError> {
        digest_finalization_plan
            .validate_against(foundation)
            .map_err(ConeProductionSectionBuildError::DigestPlan)?;
        let object_definition_plans = ObjectDefinitionPlanSurfaceV1::from_foundation(foundation)
            .map_err(ConeProductionSectionBuildError::ObjectDefinitions)?;
        let canonical_definitions =
            ObjectSymbolSurfaceV1::from_definition_plans(foundation, &object_definition_plans)
                .map_err(ConeProductionSectionBuildError::CanonicalDefinitions)?;
        let image_plan = ConeImagePlanV1::new(
            coordinate,
            direct_dependencies,
            foundation,
            registration_production.identities(),
            &digest_finalization_plan,
        )
        .map_err(ConeProductionSectionBuildError::Image)?;
        let entry_plan = EntryProductionPlanV1::new(
            entry_source,
            foundation,
            registration_production.identities(),
            &digest_finalization_plan,
        )
        .map_err(ConeProductionSectionBuildError::Entry)?;
        let shape_support_plan = ParamFreeShapeSupportPlanSetV1::from_sources(
            shape_sources.iter(),
            foundation,
            registration_production.identities(),
        )
        .map_err(ConeProductionSectionBuildError::ShapeSupport)?;
        let generated_bridge_plan = GeneratedBridgePlanSetV1::from_foundation(foundation)
            .map_err(ConeProductionSectionBuildError::GeneratedBridges)?;
        Ok(Self {
            canonical_definitions,
            object_definition_plans,
            digest_finalization_plan,
            registration_production,
            image_plan,
            entry_plan,
            shape_support_plan,
            generated_bridge_plan,
            canonical_callables,
            canonical_shapes,
        })
    }

    pub const fn canonical_definitions(&self) -> &ObjectSymbolSurfaceV1 {
        &self.canonical_definitions
    }

    pub const fn object_definition_plans(&self) -> &ObjectDefinitionPlanSurfaceV1 {
        &self.object_definition_plans
    }

    pub const fn digest_finalization_plan(&self) -> &DigestFinalizationPlanV1 {
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

    pub const fn canonical_callable_definitions(
        &self,
    ) -> &crate::CanonicalCallableLirDefinitionsV1 {
        &self.canonical_callables
    }

    pub const fn canonical_shape_definitions(&self) -> &crate::CanonicalShapeLirDefinitionsV1 {
        &self.canonical_shapes
    }

    pub const fn generated_bridge_plan(&self) -> &GeneratedBridgePlanSetV1 {
        &self.generated_bridge_plan
    }
}

impl<D: crate::StrongDescriptorReference, C: Clone + WireEncode, I: WireEncode> WireEncode
    for ConeProductionSection<D, C, I>
{
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
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
        self.generated_bridge_plan.encode(encoder)?;
        encoder.field(13)?;
        self.canonical_callables.encode(encoder)?;
        encoder.field(14)?;
        self.canonical_shapes.encode(encoder)
    }
}

mod decoded;
mod link;
pub use decoded::*;

mod registrations;
mod replay;
pub use replay::StrongProductionLayoutJoinError;

impl DecodedConeProductionSectionV1 {
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
    ) -> Result<ConeProductionSectionV1, ConeProductionSectionValidationError> {
        let actual = encode(&self).map_err(ConeProductionSectionValidationError::Encode)?;
        let digest_finalization_plan = self
            .digest_finalization_plan
            .validate(identities, foundation)
            .map_err(ConeProductionSectionValidationError::DigestPlan)?;
        let registration_production = self
            .registration_production
            .validate(target, foundation, &digest_finalization_plan)
            .map_err(ConeProductionSectionValidationError::Registrations)?;
        let canonical_callables = self
            .canonical_callables
            .validate(foundation)
            .map_err(ConeProductionSectionValidationError::CanonicalCallables)?;
        let canonical_shapes = self
            .canonical_shapes
            .validate(foundation)
            .map_err(ConeProductionSectionValidationError::CanonicalShapes)?;
        let expected = ConeProductionSectionV1::new(
            coordinate,
            direct_dependencies,
            foundation,
            digest_finalization_plan,
            registration_production,
            entry_source,
            shape_sources,
            canonical_callables,
            canonical_shapes,
        )
        .map_err(ConeProductionSectionValidationError::Expected)?;
        let expected_bytes =
            encode(&expected).map_err(ConeProductionSectionValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(ConeProductionSectionValidationError::SectionMismatch);
        }
        Ok(expected)
    }
}

#[derive(Debug)]
pub enum ConeProductionSectionBuildError {
    CanonicalDefinitions(ObjectSymbolSurfaceBuildError),
    ObjectDefinitions(ObjectDefinitionPlanBuildError),
    DigestPlan(DigestPlanError),
    Image(ConeImagePlanBuildError),
    Entry(EntryProductionPlanBuildError),
    ShapeSupport(ParamFreeShapeSupportBuildError),
    GeneratedBridges(GeneratedBridgePlanBuildError),
}

impl fmt::Display for ConeProductionSectionBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong production section: {self:?}")
    }
}

impl std::error::Error for ConeProductionSectionBuildError {}

#[derive(Debug)]
pub enum ConeProductionSectionValidationError {
    CanonicalCallables(crate::CanonicalCallableLirError),
    CanonicalShapes(crate::CanonicalShapeLirError),
    DigestReplay(Box<crate::DigestPlanReplayError>),
    DigestProjection(Box<crate::DigestProjectionError>),
    DigestMismatch,
    Encode(scoop_wire::cbor::EncodeError),
    Resource(WireError),
    DigestPlan(DigestPlanValidationError),
    Registrations(StrongRegistrationProductionValidationError),
    Expected(ConeProductionSectionBuildError),
    SectionMismatch,
}

impl fmt::Display for ConeProductionSectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong production section: {self:?}"
        )
    }
}

impl std::error::Error for ConeProductionSectionValidationError {}

#[cfg(test)]
pub(in crate::production) mod tests;
