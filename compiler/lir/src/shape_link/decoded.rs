use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedPersistentSymbolRequest, ObjectDefinitionPlanId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::wire::{EncodeResult, field};
use super::{ExternalShapeLinkImportV1, ShapeLinkError};
use crate::DecodedExternalStrongShapeSubjectV1;

#[derive(Debug)]
pub struct DecodedExternalShapeLinkImportV1 {
    pub(super) provider: DecodedPersistentId<ConeIdentity>,
    pub(super) subject: DecodedExternalStrongShapeSubjectV1,
    expected_symbol: DecodedPersistentSymbolRequest,
    required_definition: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl DecodedExternalShapeLinkImportV1 {
    pub(super) fn read(
        self,
        provider: &super::ShapeLinkProviderV1<'_>,
        subject: crate::ExternalStrongShapeSubjectV1,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
    ) -> Result<ExternalShapeLinkImportV1, ShapeLinkError> {
        use scoop_identity::PersistentIdResolver;
        let expected_symbol = self.expected_symbol.resolve(identities)?;
        let required_definition = identities.resolve(self.required_definition)?;
        if !provider
            .canonical_definitions()
            .plan(required_definition)
            .is_some_and(|plan| plan.primary_symbol() == expected_symbol)
        {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        }
        Ok(ExternalShapeLinkImportV1 {
            provider: provider.provider(),
            subject,
            expected_symbol,
            symbol: expected_symbol.symbol(),
            required_definition,
            contract: provider.contract(subject)?,
        })
    }
}
impl WireDecode for DecodedExternalShapeLinkImportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            provider: decoder.field(1, DecodedPersistentId::decode)?,
            subject: decoder.field(2, DecodedExternalStrongShapeSubjectV1::decode)?,
            expected_symbol: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            required_definition: decoder.field(4, DecodedPersistentId::decode)?,
        })
    }
}
impl WireEncode for DecodedExternalShapeLinkImportV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(4)?;
        field(encoder, 1, &self.provider)?;
        field(encoder, 2, &self.subject)?;
        field(encoder, 3, &self.expected_symbol)?;
        field(encoder, 4, &self.required_definition)
    }
}
