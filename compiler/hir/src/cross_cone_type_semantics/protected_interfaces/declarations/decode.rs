use super::*;
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedProtectedDeclarationInterfaceV1 {
    Callable(Box<DecodedProtectedCallableInterfaceV1>),
    Constructor(Box<DecodedProtectedConstructorInterfaceV1>),
    Property(Box<DecodedProtectedPropertyInterfaceV1>),
    NestedNominal(Box<DecodedProtectedNestedNominalInterfaceV1>),
}
impl DecodedProtectedDeclarationInterfaceV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedDeclarationInterfaceV1, ProtectedDeclarationResolutionError<E>> {
        use ProtectedDeclarationResolutionError as Error;
        Ok(match self {
            Self::Callable(value) => ProtectedDeclarationInterfaceV1::Callable(Box::new(
                value.resolve(resolver, meter).map_err(Error::Callable)?,
            )),
            Self::Constructor(value) => ProtectedDeclarationInterfaceV1::Constructor(Box::new(
                value.resolve(resolver, meter).map_err(Error::Callable)?,
            )),
            Self::Property(value) => ProtectedDeclarationInterfaceV1::Property(Box::new(
                value.resolve(resolver, meter).map_err(Error::Property)?,
            )),
            Self::NestedNominal(value) => ProtectedDeclarationInterfaceV1::NestedNominal(Box::new(
                value.resolve(resolver, meter).map_err(Error::Nested)?,
            )),
        })
    }
}
impl WireDecode for DecodedProtectedDeclarationInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => DecodedProtectedCallableInterfaceV1::decode_fields(decoder)
                .map(|value| Self::Callable(Box::new(value))),
            2 => DecodedProtectedConstructorInterfaceV1::decode_fields(decoder)
                .map(|value| Self::Constructor(Box::new(value))),
            3 => DecodedProtectedPropertyInterfaceV1::decode_fields(decoder)
                .map(|value| Self::Property(Box::new(value))),
            4 => DecodedProtectedNestedNominalInterfaceV1::decode_fields(decoder)
                .map(|value| Self::NestedNominal(Box::new(value))),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
impl WireEncode for DecodedProtectedDeclarationInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(value) => {
                wire::tag(encoder, 4, 1)?;
                value.encode_fields(encoder)
            }
            Self::Constructor(value) => {
                wire::tag(encoder, 4, 2)?;
                value.encode_fields(encoder)
            }
            Self::Property(value) => {
                wire::tag(encoder, 4, 3)?;
                value.encode_fields(encoder)
            }
            Self::NestedNominal(value) => {
                wire::tag(encoder, 4, 4)?;
                value.encode_fields(encoder)
            }
        }
    }
}
