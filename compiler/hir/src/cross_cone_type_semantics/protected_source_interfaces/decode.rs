use super::*;
use crate::{DecodedExportDefinitionSourceV1, ProtectedCallableInterfaceResolver};
use scoop_identity::{
    DecodedCallableTemplateOrigin, DecodedCanonicalIdentifier, DecodedSignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

mod calling;
pub use calling::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedSourceParameterV1 {
    name: DecodedCanonicalIdentifier,
    value_type: DecodedSignatureTypeKey,
    calling: DecodedProtectedParameterCallingV1,
    definition_origin: DecodedExportDefinitionSourceV1,
}
impl DecodedProtectedSourceParameterV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        keys: &ProtectedDefaultKeyIndexV1,
    ) -> Result<ProtectedSourceParameterV1, ProtectedSourceResolutionError<E>> {
        use ProtectedSourceResolutionError as Error;

        let name = self.name.validate().map_err(Error::Name)?;
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(Error::Foundation)?;
        let calling = self.calling.resolve(resolver, keys)?;
        let origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(Error::Origin)?;
        Ok(ProtectedSourceParameterV1::new(
            name, value_type, calling, origin,
        ))
    }
}
impl WireDecode for DecodedProtectedSourceParameterV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            name: decoder.field(1, DecodedCanonicalIdentifier::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            calling: decoder.field(3, DecodedProtectedParameterCallingV1::decode)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
        })
    }
}
impl WireEncode for DecodedProtectedSourceParameterV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.name.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.calling.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedCallableSourceInterfaceV1 {
    owner: DecodedCallableTemplateOrigin,
    parameters: Vec<DecodedProtectedSourceParameterV1>,
}
impl DecodedProtectedCallableSourceInterfaceV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        keys: &ProtectedDefaultKeyIndexV1,
    ) -> Result<ProtectedCallableSourceInterfaceV1, ProtectedSourceResolutionError<E>> {
        use ProtectedSourceResolutionError as Error;
        let mut parameters = Vec::new();
        let path = WirePath::root();

        scoop_wire::allocation::try_reserve(&mut parameters, self.parameters.len(), &path)
            .map_err(Error::Resource)?;
        let owner = self.owner.resolve(resolver).map_err(Error::Foundation)?;
        for parameter in self.parameters {
            parameters.push(parameter.resolve(resolver, keys)?);
        }
        ProtectedCallableSourceInterfaceV1::try_new(
            owner,
            CanonicalProtectedSourceParametersV1::try_new(parameters).map_err(Error::Build)?,
        )
        .map_err(Error::Build)
    }
}
impl WireDecode for DecodedProtectedCallableSourceInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            parameters: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedProtectedSourceParameterV1::decode(d))
            })?,
        })
    }
}
impl WireEncode for DecodedProtectedCallableSourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.parameters)
    }
}
