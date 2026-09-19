use super::*;
use crate::{
    DecodedDeclarationAccessSourceV1, DecodedExportConstValueV1,
    DecodedNominalSourcePropertyPayloadV1, ProtectedPropertyInterfaceResolver,
};
use scoop_identity::DecodedPersistentId;
use scoop_wire::{BudgetMeter, Decoder, WireDecode, WireError, WireErrorKind, WirePath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNominalSupportPropertyPayloadV1 {
    Runtime {
        interface: DecodedNominalSourcePropertyPayloadV1,
    },
    Const {
        value: DecodedExportConstValueV1,
    },
}
impl DecodedNominalSupportPropertyPayloadV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<NominalSupportPropertyPayloadV1, NominalSupportPropertyResolutionError<E>> {
        use NominalSupportPropertyResolutionError as Error;
        Ok(match self {
            Self::Runtime { interface } => NominalSupportPropertyPayloadV1::Runtime {
                interface: interface.resolve(resolver, meter).map_err(Error::Runtime)?,
            },
            Self::Const { value } => NominalSupportPropertyPayloadV1::Const {
                value: value
                    .resolve_metered(resolver, meter)
                    .map_err(Error::Const)?,
            },
        })
    }
}
impl WireEncode for DecodedNominalSupportPropertyPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Runtime { interface } => {
                wire::tag(encoder, 2, 1)?;
                encoder.field(1)?;
                interface.encode(encoder)
            }
            Self::Const { value } => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                value.encode(encoder)
            }
        }
    }
}
impl WireDecode for DecodedNominalSupportPropertyPayloadV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedNominalSourcePropertyPayloadV1::decode)
                .map(|interface| Self::Runtime { interface }),
            2 => decoder
                .field(1, DecodedExportConstValueV1::decode)
                .map(|value| Self::Const { value }),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNominalSupportPropertyInterfaceV1 {
    declaration: DecodedPersistentId<PersistentPropertyId>,
    declaration_access: DecodedDeclarationAccessSourceV1,
    payload: DecodedNominalSupportPropertyPayloadV1,
}
impl DecodedNominalSupportPropertyInterfaceV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<NominalSupportPropertyInterfaceV1, NominalSupportPropertyResolutionError<E>> {
        use NominalSupportPropertyResolutionError as Error;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        let declaration = resolver
            .resolve(self.declaration)
            .map_err(Error::Identity)?;
        let access = self
            .declaration_access
            .resolve_metered(resolver, meter)
            .map_err(Error::Source)?;
        let payload = self.payload.resolve(resolver, meter)?;
        NominalSupportPropertyInterfaceV1::try_new(declaration, access, payload)
            .map_err(Error::Property)
    }
}
impl WireEncode for DecodedNominalSupportPropertyInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        self.encode_fields(encoder)
    }
}
impl DecodedNominalSupportPropertyInterfaceV1 {
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
impl WireDecode for DecodedNominalSupportPropertyInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Self::decode_fields(decoder)
    }
}
impl DecodedNominalSupportPropertyInterfaceV1 {
    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn decode_fields(
        decoder: &mut Decoder<'_, '_>,
    ) -> Result<Self, WireError> {
        Ok(Self {
            declaration: decoder.field(1, DecodedPersistentId::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedNominalSupportPropertyPayloadV1::decode)?,
        })
    }
}
