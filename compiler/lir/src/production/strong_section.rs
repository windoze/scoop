//! Closed LIR production section for the M23-3 strong-only profile.

use std::fmt;

use scoop_identity::{ConeCoordinate, SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use crate::{
    CoreShapeSupportPlanBuildError, CoreShapeSupportPlanV1, DecodedConeImagePlanV1,
    DecodedCoreShapeSupportPlanV1, DecodedEntryProductionPlanV1, DecodedGeneratedBridgePlanSetV1,
    DecodedStrongDigestFinalizationPlanV1, DecodedStrongExternalLirBridgeSurfaceV1,
    DecodedStrongObjectDefinitionPlanSurfaceV1, DecodedStrongObjectSymbolSurfaceV1,
    DecodedStrongRegistrationIdentitySurfaceV1, DigestPlanError, EntryProductionPlanBuildError,
    EntryProductionPlanV1, EntryProductionSourceV1, GeneratedBridgePlanBuildError,
    GeneratedBridgePlanSetV1, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongDigestPlanValidationError, StrongExternalLirBridgeSurfaceV1,
    StrongObjectDefinitionPlanBuildError, StrongObjectDefinitionPlanSurfaceV1,
    StrongObjectSymbolSurfaceBuildError, StrongObjectSymbolSurfaceV1,
    StrongRegistrationIdentityBuildError, StrongRegistrationIdentitySurfaceV1,
};

use crate::{ConeImagePlanBuildError, ConeImagePlanV1};

/// Every canonical LIR production input required by the strong-only profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongProductionSectionV1 {
    external_bridges: StrongExternalLirBridgeSurfaceV1,
    canonical_definitions: StrongObjectSymbolSurfaceV1,
    object_definition_plans: StrongObjectDefinitionPlanSurfaceV1,
    digest_finalization_plan: StrongDigestFinalizationPlanV1,
    registration_plans: StrongRegistrationIdentitySurfaceV1,
    image_plan: ConeImagePlanV1,
    entry_plan: EntryProductionPlanV1,
    core_shape_support: CoreShapeSupportPlanV1,
    generated_bridge_plan: GeneratedBridgePlanSetV1,
}

impl StrongProductionSectionV1 {
    pub fn new(
        coordinate: ConeCoordinate,
        foundation: &OdrFreeLirFoundation,
        external_bridges: StrongExternalLirBridgeSurfaceV1,
        digest_finalization_plan: StrongDigestFinalizationPlanV1,
        entry_source: EntryProductionSourceV1,
        core_shape_sources: &[SourceDeclarationKey],
    ) -> Result<Self, StrongProductionSectionBuildError> {
        if external_bridges.producer() != foundation.producer() {
            return Err(StrongProductionSectionBuildError::ExternalBridgeProducer);
        }
        digest_finalization_plan
            .validate_against(foundation)
            .map_err(StrongProductionSectionBuildError::DigestPlan)?;
        let canonical_definitions =
            StrongObjectSymbolSurfaceV1::from_odr_free_foundation(foundation)
                .map_err(StrongProductionSectionBuildError::CanonicalDefinitions)?;
        let object_definition_plans =
            StrongObjectDefinitionPlanSurfaceV1::from_odr_free_foundation(foundation)
                .map_err(StrongProductionSectionBuildError::ObjectDefinitions)?;
        let registration_plans = StrongRegistrationIdentitySurfaceV1::from_foundation(
            foundation,
            &digest_finalization_plan,
        )
        .map_err(StrongProductionSectionBuildError::Registrations)?;
        let image_plan = ConeImagePlanV1::new(
            coordinate,
            foundation,
            &registration_plans,
            &digest_finalization_plan,
        )
        .map_err(StrongProductionSectionBuildError::Image)?;
        let entry_plan = EntryProductionPlanV1::new(
            entry_source,
            foundation,
            &registration_plans,
            &digest_finalization_plan,
        )
        .map_err(StrongProductionSectionBuildError::Entry)?;
        let core_shape_support =
            CoreShapeSupportPlanV1::new(core_shape_sources.iter(), foundation, &registration_plans)
                .map_err(StrongProductionSectionBuildError::CoreShapeSupport)?;
        let generated_bridge_plan = GeneratedBridgePlanSetV1::from_odr_free_foundation(foundation)
            .map_err(StrongProductionSectionBuildError::GeneratedBridges)?;
        Ok(Self {
            external_bridges,
            canonical_definitions,
            object_definition_plans,
            digest_finalization_plan,
            registration_plans,
            image_plan,
            entry_plan,
            core_shape_support,
            generated_bridge_plan,
        })
    }

    pub const fn external_bridges(&self) -> &StrongExternalLirBridgeSurfaceV1 {
        &self.external_bridges
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

    pub const fn registration_plans(&self) -> &StrongRegistrationIdentitySurfaceV1 {
        &self.registration_plans
    }

    pub const fn image_plan(&self) -> &ConeImagePlanV1 {
        &self.image_plan
    }

    pub const fn entry_plan(&self) -> &EntryProductionPlanV1 {
        &self.entry_plan
    }

    pub const fn core_shape_support(&self) -> &CoreShapeSupportPlanV1 {
        &self.core_shape_support
    }

    pub const fn generated_bridge_plan(&self) -> &GeneratedBridgePlanSetV1 {
        &self.generated_bridge_plan
    }
}

impl WireEncode for StrongProductionSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.external_bridges.encode(encoder)?;
        encoder.field(2)?;
        self.canonical_definitions.encode(encoder)?;
        encoder.field(3)?;
        self.object_definition_plans.encode(encoder)?;
        encoder.field(4)?;
        self.digest_finalization_plan.encode(encoder)?;
        encoder.field(5)?;
        self.registration_plans.encode(encoder)?;
        encoder.field(6)?;
        self.image_plan.encode(encoder)?;
        encoder.field(7)?;
        self.entry_plan.encode(encoder)?;
        encoder.field(8)?;
        self.core_shape_support.encode(encoder)?;
        encoder.field(9)?;
        self.generated_bridge_plan.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedStrongProductionSectionV1 {
    external_bridges: DecodedStrongExternalLirBridgeSurfaceV1,
    canonical_definitions: DecodedStrongObjectSymbolSurfaceV1,
    object_definition_plans: DecodedStrongObjectDefinitionPlanSurfaceV1,
    digest_finalization_plan: DecodedStrongDigestFinalizationPlanV1,
    registration_plans: DecodedStrongRegistrationIdentitySurfaceV1,
    image_plan: DecodedConeImagePlanV1,
    entry_plan: DecodedEntryProductionPlanV1,
    core_shape_support: DecodedCoreShapeSupportPlanV1,
    generated_bridge_plan: DecodedGeneratedBridgePlanSetV1,
}

impl DecodedStrongProductionSectionV1 {
    pub fn validate(
        self,
        coordinate: ConeCoordinate,
        foundation: &OdrFreeLirFoundation,
        expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
        entry_source: EntryProductionSourceV1,
        core_shape_sources: &[SourceDeclarationKey],
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<StrongProductionSectionV1, StrongProductionSectionValidationError> {
        let actual = encode(&self).map_err(StrongProductionSectionValidationError::Encode)?;
        let digest_finalization_plan = self
            .digest_finalization_plan
            .validate(identities, foundation)
            .map_err(StrongProductionSectionValidationError::DigestPlan)?;
        let expected = StrongProductionSectionV1::new(
            coordinate,
            foundation,
            expected_external_bridges.clone(),
            digest_finalization_plan,
            entry_source,
            core_shape_sources,
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

impl WireEncode for DecodedStrongProductionSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.external_bridges.encode(encoder)?;
        encoder.field(2)?;
        self.canonical_definitions.encode(encoder)?;
        encoder.field(3)?;
        self.object_definition_plans.encode(encoder)?;
        encoder.field(4)?;
        self.digest_finalization_plan.encode(encoder)?;
        encoder.field(5)?;
        self.registration_plans.encode(encoder)?;
        encoder.field(6)?;
        self.image_plan.encode(encoder)?;
        encoder.field(7)?;
        self.entry_plan.encode(encoder)?;
        encoder.field(8)?;
        self.core_shape_support.encode(encoder)?;
        encoder.field(9)?;
        self.generated_bridge_plan.encode(encoder)
    }
}

impl WireDecode for DecodedStrongProductionSectionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            external_bridges: decoder.field(1, DecodedStrongExternalLirBridgeSurfaceV1::decode)?,
            canonical_definitions: decoder.field(2, DecodedStrongObjectSymbolSurfaceV1::decode)?,
            object_definition_plans: decoder
                .field(3, DecodedStrongObjectDefinitionPlanSurfaceV1::decode)?,
            digest_finalization_plan: decoder
                .field(4, DecodedStrongDigestFinalizationPlanV1::decode)?,
            registration_plans: decoder
                .field(5, DecodedStrongRegistrationIdentitySurfaceV1::decode)?,
            image_plan: decoder.field(6, DecodedConeImagePlanV1::decode)?,
            entry_plan: decoder.field(7, DecodedEntryProductionPlanV1::decode)?,
            core_shape_support: decoder.field(8, DecodedCoreShapeSupportPlanV1::decode)?,
            generated_bridge_plan: decoder.field(9, DecodedGeneratedBridgePlanSetV1::decode)?,
        })
    }
}

#[derive(Debug)]
pub enum StrongProductionSectionBuildError {
    ExternalBridgeProducer,
    CanonicalDefinitions(StrongObjectSymbolSurfaceBuildError),
    ObjectDefinitions(StrongObjectDefinitionPlanBuildError),
    DigestPlan(DigestPlanError),
    Registrations(StrongRegistrationIdentityBuildError),
    Image(ConeImagePlanBuildError),
    Entry(EntryProductionPlanBuildError),
    CoreShapeSupport(CoreShapeSupportPlanBuildError),
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
    Encode(scoop_wire::cbor::EncodeError),
    DigestPlan(StrongDigestPlanValidationError),
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
mod tests;
