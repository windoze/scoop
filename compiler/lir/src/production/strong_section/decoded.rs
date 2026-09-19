//! Versioned ten-field Strong section carriers.

use super::*;

pub type DecodedStrongProductionSectionV1 =
    DecodedStrongProductionSection<DecodedStrongRegistrationProductionSurfaceV1>;
pub type DecodedStrongProductionSectionV2 =
    DecodedStrongProductionSection<crate::DecodedStrongRegistrationProductionSurfaceV2>;

#[derive(Debug)]
pub struct DecodedStrongProductionSection<R> {
    pub(super) external_bridges: DecodedStrongExternalLirBridgeSurfaceV1,
    pub(super) canonical_definitions: DecodedStrongObjectSymbolSurfaceV1,
    pub(super) object_definition_plans: DecodedStrongObjectDefinitionPlanSurfaceV1,
    pub(super) digest_finalization_plan: DecodedStrongDigestFinalizationPlanV1,
    pub(super) registration_production: R,
    pub(super) image_plan: DecodedConeImagePlanV1,
    pub(super) entry_plan: DecodedEntryProductionPlanV1,
    pub(super) core_shape_support: DecodedCoreShapeSupportPlanV1,
    pub(super) generated_bridge_plan: DecodedGeneratedBridgePlanSetV1,
    pub(super) core_lir_bridge: DecodedCoreLirBridgeBranchV1,
}

impl<R> DecodedStrongProductionSection<R> {
    pub fn reconstruct_external_bridges(
        &self,
        producer: scoop_identity::ConeIdentity,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeReconstructionError> {
        self.external_bridges.reconstruct(producer, identities)
    }
}

impl<R: WireEncode> WireEncode for DecodedStrongProductionSection<R> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.external_bridges.encode(encoder)?;
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
        self.core_shape_support.encode(encoder)?;
        encoder.field(9)?;
        self.generated_bridge_plan.encode(encoder)?;
        encoder.field(10)?;
        self.core_lir_bridge.encode(encoder)
    }
}

impl<R: WireDecode> WireDecode for DecodedStrongProductionSection<R> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Ok(Self {
            external_bridges: decoder.field(1, DecodedStrongExternalLirBridgeSurfaceV1::decode)?,
            canonical_definitions: decoder.field(2, DecodedStrongObjectSymbolSurfaceV1::decode)?,
            object_definition_plans: decoder
                .field(3, DecodedStrongObjectDefinitionPlanSurfaceV1::decode)?,
            digest_finalization_plan: decoder
                .field(4, DecodedStrongDigestFinalizationPlanV1::decode)?,
            registration_production: decoder.field(5, R::decode)?,
            image_plan: decoder.field(6, DecodedConeImagePlanV1::decode)?,
            entry_plan: decoder.field(7, DecodedEntryProductionPlanV1::decode)?,
            core_shape_support: decoder.field(8, DecodedCoreShapeSupportPlanV1::decode)?,
            generated_bridge_plan: decoder.field(9, DecodedGeneratedBridgePlanSetV1::decode)?,
            core_lir_bridge: decoder.field(10, DecodedCoreLirBridgeBranchV1::decode)?,
        })
    }
}
