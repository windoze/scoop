use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedPersistentSymbolRequest, ObjectDefinitionPlanId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::wire::{EncodeResult, equal_fields, field};
use super::{DecodedShapeLinkContractV1, ExternalShapeLinkImportV1, ShapeLinkError};
use crate::DecodedExternalStrongShapeSubjectV1;

#[derive(Debug)]
pub struct DecodedExternalShapeLinkImportV1 {
    pub(super) provider: DecodedPersistentId<ConeIdentity>,
    pub(super) subject: DecodedExternalStrongShapeSubjectV1,
    expected_symbol: DecodedPersistentSymbolRequest,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
    contract: DecodedShapeLinkContractV1,
}

impl DecodedExternalShapeLinkImportV1 {
    pub fn validate_against(
        self,
        expected: &ExternalShapeLinkImportV1<'_>,
    ) -> Result<(), ShapeLinkError> {
        if self.provider.verify(expected.provider()).is_err()
            || self
                .required_definition
                .verify(expected.required_definition())
                .is_err()
            || !equal_fields(&self.subject, &expected.subject())?
            || !equal_fields(&self.expected_symbol, &expected.expected_symbol())?
            || !expected.contract().matches_subject(expected.subject())
        {
            return Err(ShapeLinkError::Header);
        }
        self.contract.validate_against(expected.contract())
    }
}
impl WireDecode for DecodedExternalShapeLinkImportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            subject: decoder.field(2, DecodedExternalStrongShapeSubjectV1::decode)?,
            expected_symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            required_definition: decoder.field(4, DecodedPersistentId::decode)?,
            contract: decoder.field(5, DecodedShapeLinkContractV1::decode)?,
        })
    }
}
impl WireEncode for DecodedExternalShapeLinkImportV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(5)?;
        field(encoder, 1, &self.provider)?;
        field(encoder, 2, &self.subject)?;
        field(encoder, 3, &self.expected_symbol)?;
        field(encoder, 4, &self.required_definition)?;
        field(encoder, 5, &self.contract)
    }
}
