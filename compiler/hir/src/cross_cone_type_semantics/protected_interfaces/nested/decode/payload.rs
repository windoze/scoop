use super::*;
use crate::{DeclaredVisibilityV1, DecodedDeclarationAccessSourceV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedNestedNominalPayloadV1 {
    source_nominal: DecodedSourceNominalId,
    source_interface: DecodedProtectedNestedSourceInterfaceV1,
}
impl DecodedProtectedNestedNominalPayloadV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedNestedNominalPayloadV1, NestedSourceResolutionError<E>> {
        self.resolve_at(resolver)
    }
    fn resolve_at<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedNestedNominalPayloadV1, NestedSourceResolutionError<E>> {
        use NestedSourceResolutionError as Error;

        let source_nominal = self
            .source_nominal
            .resolve(resolver)
            .map_err(Error::Foundation)?;
        let source_interface = self.source_interface.resolve_at(resolver)?;
        ProtectedNestedNominalPayloadV1::try_new(source_nominal, source_interface)
            .map_err(Error::Build)
    }
}
impl WireDecode for DecodedProtectedNestedNominalPayloadV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            source_nominal: decoder.field(1, DecodedSourceNominalId::decode)?,
            source_interface: decoder.field(2, DecodedProtectedNestedSourceInterfaceV1::decode)?,
        })
    }
}
impl WireEncode for DecodedProtectedNestedNominalPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.source_nominal.encode(encoder)?;
        encoder.field(2)?;
        self.source_interface.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSupportNestedInterfaceV1 {
    declaration: DecodedSourceNominalId,
    declaration_access: DecodedDeclarationAccessSourceV1,
    payload: DecodedProtectedNestedNominalPayloadV1,
}
impl DecodedNominalSupportNestedInterfaceV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSupportNestedInterfaceV1, NestedSourceResolutionError<E>> {
        self.resolve_at(resolver)
    }
    pub(super) fn resolve_at<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<NominalSupportNestedInterfaceV1, NestedSourceResolutionError<E>> {
        use NestedSourceResolutionError as Error;

        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(Error::Foundation)?;
        let access = self
            .declaration_access
            .resolve(resolver)
            .map_err(Error::Access)?;
        let payload = self.payload.resolve_at(resolver)?;
        NominalSupportNestedInterfaceV1::try_new(declaration, access, payload).map_err(Error::Build)
    }
    pub(super) fn decode_fields(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        Ok(Self {
            declaration: decoder.field(1, DecodedSourceNominalId::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedProtectedNestedNominalPayloadV1::decode)?,
        })
    }
    pub(super) fn encode_fields(
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
impl WireDecode for DecodedNominalSupportNestedInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Self::decode_fields(decoder)
    }
}
impl WireEncode for DecodedNominalSupportNestedInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        self.encode_fields(encoder)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedNestedNominalInterfaceV1(DecodedNominalSupportNestedInterfaceV1);
impl DecodedProtectedNestedNominalInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn encode_fields(
        &self,
        encoder: &mut Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode_fields(encoder)
    }
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn decode_fields(
        decoder: &mut Decoder<'_>,
    ) -> Result<Self, WireError> {
        DecodedNominalSupportNestedInterfaceV1::decode_fields(decoder).map(Self)
    }
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<ProtectedNestedNominalInterfaceV1, NestedSourceResolutionError<E>> {
        let source = self.0.resolve(resolver)?;
        if source.declaration_access().declared_visibility() != DeclaredVisibilityV1::Protected {
            return Err(NestedSourceResolutionError::Build(
                NestedSourceBuildError::Access,
            ));
        }
        Ok(ProtectedNestedNominalInterfaceV1 { source })
    }
}
impl WireDecode for DecodedProtectedNestedNominalInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        DecodedNominalSupportNestedInterfaceV1::decode(decoder).map(Self)
    }
}
impl WireEncode for DecodedProtectedNestedNominalInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}
