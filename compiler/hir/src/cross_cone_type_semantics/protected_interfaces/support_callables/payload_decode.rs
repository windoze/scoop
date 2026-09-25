use super::super::*;
use crate::{
    CallableModalityV1, DecodedCallableSourceEffectsV1, DecodedCanonicalBinderListV1,
    DecodedCanonicalSourceParameterShapesV1, DecodedSourceNominalId,
};
use scoop_identity::{
    CallableTemplateOrigin, DecodedOptionalSignatureType, DecodedSignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSourceCallablePayloadV1 {
    owner: DecodedSourceNominalId,
    type_parameters: DecodedCanonicalBinderListV1,
    receiver: DecodedOptionalSignatureType,
    parameters: DecodedCanonicalSourceParameterShapesV1,
    result: DecodedSignatureTypeKey,
    effects: DecodedCallableSourceEffectsV1,
    modality: CallableModalityV1,
    source_interface: DecodedNominalSupportSourceInterfaceUseV1,
    slot_relations: DecodedCanonicalProtectedSlotRefsV1,
}
impl DecodedNominalSourceCallablePayloadV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        declaration: CallableTemplateOrigin,
        resolver: &mut R,
    ) -> Result<NominalSourceCallablePayloadV1, ProtectedCallableInterfaceResolutionError<E>> {
        use ProtectedCallableInterfaceResolutionError as Error;

        if self.receiver != DecodedOptionalSignatureType::Absent {
            return Err(Error::Interface(
                ProtectedCallableInterfaceBuildError::Receiver,
            ));
        }
        let source_interface = self
            .source_interface
            .resolve(resolver)
            .map_err(Error::Identity)?;
        if source_interface
            != NominalSupportSourceInterfaceUseV1::for_declaration(declaration)
                .map_err(Error::Interface)?
        {
            return Err(Error::Interface(
                ProtectedCallableInterfaceBuildError::SourceInterface,
            ));
        }
        let owner = self.owner.resolve(resolver).map_err(Error::Identity)?;
        let type_parameters = self
            .type_parameters
            .resolve(resolver)
            .map_err(Error::Binders)?;
        let parameters = self
            .parameters
            .resolve(resolver)
            .map_err(Error::Parameters)?;

        let result = self.result.resolve(resolver).map_err(Error::Identity)?;
        let effects = self.effects.validate().map_err(Error::Effects)?;
        let slots = self.slot_relations.resolve(resolver)?;
        NominalSourceCallablePayloadV1::try_new(
            declaration,
            owner,
            type_parameters,
            parameters,
            result,
            effects,
            self.modality,
            slots,
        )
        .map_err(Error::Interface)
    }
}
impl WireEncode for DecodedNominalSourceCallablePayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(3)?;
        self.receiver.encode(encoder)?;
        encoder.field(4)?;
        self.parameters.encode(encoder)?;
        encoder.field(5)?;
        self.result.encode(encoder)?;
        encoder.field(6)?;
        self.effects.encode(encoder)?;
        encoder.field(7)?;
        self.modality.encode(encoder)?;
        encoder.field(8)?;
        self.source_interface.encode(encoder)?;
        encoder.field(9)?;
        self.slot_relations.encode(encoder)
    }
}
impl WireDecode for DecodedNominalSourceCallablePayloadV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            owner: decoder.field(1, DecodedSourceNominalId::decode)?,
            type_parameters: decoder.field(2, DecodedCanonicalBinderListV1::decode)?,
            receiver: decoder.field(3, DecodedOptionalSignatureType::decode)?,
            parameters: decoder.field(4, DecodedCanonicalSourceParameterShapesV1::decode)?,
            result: decoder.field(5, DecodedSignatureTypeKey::decode)?,
            effects: decoder.field(6, DecodedCallableSourceEffectsV1::decode)?,
            modality: decoder.field(7, CallableModalityV1::decode)?,
            source_interface: decoder
                .field(8, DecodedNominalSupportSourceInterfaceUseV1::decode)?,
            slot_relations: decoder.field(9, DecodedCanonicalProtectedSlotRefsV1::decode)?,
        })
    }
}
