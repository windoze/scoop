use std::fmt;

use scoop_identity::SourceDeclarationKind;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{DecodedSourceNominalId, SourceNominalId, SourceNominalIdResolver};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PublicNominalKindV1 {
    Class,
    Interface,
    Struct,
    Enum,
    Object,
}

impl PublicNominalKindV1 {
    pub const fn source_kind(self) -> SourceDeclarationKind {
        match self {
            Self::Class => SourceDeclarationKind::Class,
            Self::Interface => SourceDeclarationKind::Interface,
            Self::Struct => SourceDeclarationKind::Struct,
            Self::Enum => SourceDeclarationKind::Enum,
            Self::Object => SourceDeclarationKind::Object,
        }
    }
}

impl TryFrom<SourceDeclarationKind> for PublicNominalKindV1 {
    type Error = UnsupportedPublicNominalKind;

    fn try_from(kind: SourceDeclarationKind) -> Result<Self, Self::Error> {
        match kind {
            SourceDeclarationKind::Class => Ok(Self::Class),
            SourceDeclarationKind::Interface => Ok(Self::Interface),
            SourceDeclarationKind::Struct => Ok(Self::Struct),
            SourceDeclarationKind::Enum => Ok(Self::Enum),
            SourceDeclarationKind::Object => Ok(Self::Object),
            kind @ (SourceDeclarationKind::AnnotationClass
            | SourceDeclarationKind::Function
            | SourceDeclarationKind::Constructor
            | SourceDeclarationKind::Property
            | SourceDeclarationKind::ExtensionProperty
            | SourceDeclarationKind::TypeAlias) => Err(UnsupportedPublicNominalKind(kind)),
        }
    }
}

impl WireEncode for PublicNominalKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Class => 1,
            Self::Interface => 2,
            Self::Struct => 3,
            Self::Enum => 4,
            Self::Object => 5,
        })
    }
}

impl WireDecode for PublicNominalKindV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Class),
            2 => Ok(Self::Interface),
            3 => Ok(Self::Struct),
            4 => Ok(Self::Enum),
            5 => Ok(Self::Object),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PublicDeclarationOwnerV1 {
    TopLevel,
    Nominal(SourceNominalId),
    Extension,
}

impl PublicDeclarationOwnerV1 {
    pub const fn nominal_owner(self) -> Option<SourceNominalId> {
        match self {
            Self::Nominal(owner) => Some(owner),
            Self::TopLevel | Self::Extension => None,
        }
    }
}

impl WireEncode for PublicDeclarationOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::TopLevel => encode_empty_sum(encoder, 1),
            Self::Nominal(owner) => encode_value_sum(encoder, 2, owner),
            Self::Extension => encode_empty_sum(encoder, 3),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedPublicDeclarationOwnerV1 {
    TopLevel,
    Nominal(DecodedSourceNominalId),
    Extension,
}

impl DecodedPublicDeclarationOwnerV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<PublicDeclarationOwnerV1, E>
    where
        R: SourceNominalIdResolver<E>,
    {
        match self {
            Self::TopLevel => Ok(PublicDeclarationOwnerV1::TopLevel),
            Self::Nominal(owner) => owner
                .resolve(resolver)
                .map(PublicDeclarationOwnerV1::Nominal),
            Self::Extension => Ok(PublicDeclarationOwnerV1::Extension),
        }
    }
}

impl WireEncode for DecodedPublicDeclarationOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::TopLevel => encode_empty_sum(encoder, 1),
            Self::Nominal(owner) => encode_value_sum(encoder, 2, owner),
            Self::Extension => encode_empty_sum(encoder, 3),
        }
    }
}

impl WireDecode for DecodedPublicDeclarationOwnerV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::TopLevel)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSourceNominalId::decode)
                    .map(Self::Nominal)
            }
            3 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Extension)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedPublicNominalKind(SourceDeclarationKind);

impl UnsupportedPublicNominalKind {
    pub const fn kind(self) -> SourceDeclarationKind {
        self.0
    }
}

impl fmt::Display for UnsupportedPublicNominalKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "source declaration kind {:?} is not a public nominal kind",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedPublicNominalKind {}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
