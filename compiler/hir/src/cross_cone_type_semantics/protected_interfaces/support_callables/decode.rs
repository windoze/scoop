use super::*;
use crate::{
    DecodedDeclarationAccessSourceV1, DecodedNominalSourceCallablePayloadV1,
    ProtectedCallableInterfaceResolutionError, ProtectedCallableInterfaceResolver,
};
use scoop_identity::{DecodedCallableTemplateOrigin, DecodedPersistentId};
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSupportCallableInterfaceV1 {
    declaration: DecodedCallableTemplateOrigin,
    declaration_access: DecodedDeclarationAccessSourceV1,
    payload: DecodedNominalSourceCallablePayloadV1,
}
impl DecodedNominalSupportCallableInterfaceV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSupportCallableInterfaceV1, ProtectedCallableInterfaceResolutionError<E>>
    {
        use ProtectedCallableInterfaceResolutionError as Error;

        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(Error::Identity)?;
        let access = self
            .declaration_access
            .resolve(resolver)
            .map_err(Error::Source)?;
        let payload = self.payload.resolve(declaration, resolver)?;
        NominalSupportCallableInterfaceV1::try_new(declaration, access, payload)
            .map_err(Error::Interface)
    }
}
impl WireEncode for DecodedNominalSupportCallableInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        self.encode_fields(encoder)
    }
}
impl DecodedNominalSupportCallableInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_access.encode(encoder)?;
        encoder.field(3)?;
        self.payload.encode(encoder)
    }
}
impl WireDecode for DecodedNominalSupportCallableInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Self::decode_fields(decoder)
    }
}
impl DecodedNominalSupportCallableInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn decode_fields(
        decoder: &mut Decoder<'_>,
    ) -> Result<Self, WireError> {
        Ok(Self {
            declaration: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedNominalSourceCallablePayloadV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSupportConstructorInterfaceV1 {
    declaration: DecodedPersistentId<PersistentConstructorId>,
    declaration_access: DecodedDeclarationAccessSourceV1,
    payload: DecodedNominalSourceCallablePayloadV1,
}
impl DecodedNominalSupportConstructorInterfaceV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSupportConstructorInterfaceV1, ProtectedCallableInterfaceResolutionError<E>>
    {
        use ProtectedCallableInterfaceResolutionError as Error;

        let declaration = resolver
            .resolve(self.declaration)
            .map_err(Error::Identity)?;
        let access = self
            .declaration_access
            .resolve(resolver)
            .map_err(Error::Source)?;
        let payload = self
            .payload
            .resolve(CallableTemplateOrigin::Constructor(declaration), resolver)?;
        NominalSupportConstructorInterfaceV1::try_new(declaration, access, payload)
            .map_err(Error::Interface)
    }
}
impl WireEncode for DecodedNominalSupportConstructorInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        self.encode_fields(encoder)
    }
}
impl DecodedNominalSupportConstructorInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_access.encode(encoder)?;
        encoder.field(3)?;
        self.payload.encode(encoder)
    }
}
impl WireDecode for DecodedNominalSupportConstructorInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Self::decode_fields(decoder)
    }
}
impl DecodedNominalSupportConstructorInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn decode_fields(
        decoder: &mut Decoder<'_>,
    ) -> Result<Self, WireError> {
        Ok(Self {
            declaration: decoder.field(1, DecodedPersistentId::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedNominalSourceCallablePayloadV1::decode)?,
        })
    }
}
