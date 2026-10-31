//! Versioned production section carriers, including canonical Function leaves.

use super::*;

pub type DecodedConeProductionSectionV1 =
    DecodedConeProductionSection<DecodedStrongRegistrationProductionSurfaceV1>;
pub type DecodedConeProductionSectionV2 =
    DecodedConeProductionSection<crate::DecodedStrongRegistrationProductionSurfaceV2>;

#[derive(Debug)]
pub struct DecodedConeProductionSection<R> {
    pub(super) registration_production: R,
    pub(super) image_plan: DecodedConeImagePlanV1,
    pub(super) entry_plan: DecodedEntryProductionPlanV1,
}

impl<R: WireEncode> WireEncode for DecodedConeProductionSection<R> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(5)?;
        self.registration_production.encode(encoder)?;
        encoder.field(6)?;
        self.image_plan.encode(encoder)?;
        encoder.field(7)?;
        self.entry_plan.encode(encoder)
    }
}

impl<R: WireDecode> WireDecode for DecodedConeProductionSection<R> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            registration_production: decoder.field(5, R::decode)?,
            image_plan: decoder.field(6, DecodedConeImagePlanV1::decode)?,
            entry_plan: decoder.field(7, DecodedEntryProductionPlanV1::decode)?,
        })
    }
}
