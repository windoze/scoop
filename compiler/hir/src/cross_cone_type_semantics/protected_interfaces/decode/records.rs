use super::*;
use crate::DecodedDeclarationAccessSourceV1;
use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, DecodedPersistentId,
    PersistentConstructorId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedCallableInterfaceV1 {
    declaration: DecodedCallableTemplateOrigin,
    declaration_access: DecodedDeclarationAccessSourceV1,
    payload: DecodedProtectedCallablePayloadV1,
}
impl DecodedProtectedCallableInterfaceV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedCallableInterfaceV1, ProtectedCallableInterfaceResolutionError<E>> {
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
        ProtectedCallableInterfaceV1::try_new(declaration, access, payload)
            .map_err(Error::Interface)
    }
}
impl DecodedProtectedCallableInterfaceV1 {
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
impl WireEncode for DecodedProtectedCallableInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        self.encode_fields(encoder)
    }
}
impl DecodedProtectedCallableInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn decode_fields(
        decoder: &mut Decoder<'_>,
    ) -> Result<Self, WireError> {
        Ok(Self {
            declaration: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedProtectedCallablePayloadV1::decode)?,
        })
    }
}
impl WireDecode for DecodedProtectedCallableInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Self::decode_fields(decoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedConstructorInterfaceV1 {
    declaration: DecodedPersistentId<PersistentConstructorId>,
    declaration_access: DecodedDeclarationAccessSourceV1,
    payload: DecodedProtectedCallablePayloadV1,
}
impl DecodedProtectedConstructorInterfaceV1 {
    pub fn resolve<R: ProtectedCallableInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedConstructorInterfaceV1, ProtectedCallableInterfaceResolutionError<E>> {
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
        ProtectedConstructorInterfaceV1::try_new(declaration, access, payload)
            .map_err(Error::Interface)
    }
}
impl DecodedProtectedConstructorInterfaceV1 {
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
impl WireEncode for DecodedProtectedConstructorInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        self.encode_fields(encoder)
    }
}
impl DecodedProtectedConstructorInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn decode_fields(
        decoder: &mut Decoder<'_>,
    ) -> Result<Self, WireError> {
        Ok(Self {
            declaration: decoder.field(1, DecodedPersistentId::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedProtectedCallablePayloadV1::decode)?,
        })
    }
}
impl WireDecode for DecodedProtectedConstructorInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Self::decode_fields(decoder)
    }
}
