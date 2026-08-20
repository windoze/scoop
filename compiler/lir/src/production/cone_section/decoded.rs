//! Versioned production section carriers, including canonical Function leaves.

use super::*;

pub type DecodedConeProductionSectionV1 =
    DecodedConeProductionSection<DecodedStrongRegistrationProductionSurfaceV1>;
pub type DecodedConeProductionSectionV2 =
    DecodedConeProductionSection<crate::DecodedStrongRegistrationProductionSurfaceV2>;

#[derive(Debug)]
pub struct DecodedConeProductionSection<R> {
    pub(super) canonical_definitions: DecodedObjectSymbolSurfaceV1,
    pub(super) object_definition_plans: DecodedObjectDefinitionPlanSurfaceV1,
    pub(super) digest_finalization_plan: DecodedDigestFinalizationPlanV1,
    pub(super) registration_production: R,
    pub(super) image_plan: DecodedConeImagePlanV1,
    pub(super) entry_plan: DecodedEntryProductionPlanV1,
    pub(super) shape_support_plan: DecodedParamFreeShapeSupportPlanSetV1,
    pub(super) generated_bridge_plan: DecodedGeneratedBridgePlanSetV1,
    pub(super) canonical_callables: crate::DecodedCanonicalCallableAbisV1,
    pub(super) canonical_shapes: crate::DecodedCanonicalShapeAbisV1,
}

impl<R: WireEncode> WireEncode for DecodedConeProductionSection<R> {
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

impl<R: WireDecode> WireDecode for DecodedConeProductionSection<R> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Ok(Self {
            canonical_definitions: decoder.field(2, DecodedObjectSymbolSurfaceV1::decode)?,
            object_definition_plans: decoder
                .field(3, DecodedObjectDefinitionPlanSurfaceV1::decode)?,
            digest_finalization_plan: decoder.field(4, DecodedDigestFinalizationPlanV1::decode)?,
            registration_production: decoder.field(5, R::decode)?,
            image_plan: decoder.field(6, DecodedConeImagePlanV1::decode)?,
            entry_plan: decoder.field(7, DecodedEntryProductionPlanV1::decode)?,
            shape_support_plan: decoder.field(8, DecodedParamFreeShapeSupportPlanSetV1::decode)?,
            generated_bridge_plan: decoder.field(9, DecodedGeneratedBridgePlanSetV1::decode)?,
            canonical_callables: decoder
                .field(13, crate::DecodedCanonicalCallableAbisV1::decode)?,
            canonical_shapes: decoder.field(14, crate::DecodedCanonicalShapeAbisV1::decode)?,
        })
    }
}
