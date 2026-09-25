use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CanonicalNativeGroupName, NativeExternalSymbolKey, NativeLibraryBinding, NativeLibraryGrouping,
    NativeLibraryKind, NativeLinkRequirementKey, NativeLinkSymbol, NativeLinkSymbolError,
};
use crate::{
    CanonicalNativeNameError, CapabilityIdError, CapabilityRefinementError,
    DecodedCanonicalNativeLibraryName, DecodedCapabilityId, DecodedPersistentId,
    NativeLinkRequirementId, PersistentIdResolver, SourceNativeSymbolError, TargetProfileWireId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNativeLinkSymbol(Vec<u8>);

impl DecodedNativeLinkSymbol {
    pub fn validate_darwin(self) -> Result<NativeLinkSymbol, NativeLinkValidationError> {
        NativeLinkSymbol::from_owned_darwin_macho_external(self.0)
    }
}

impl WireEncode for DecodedNativeLinkSymbol {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl WireDecode for DecodedNativeLinkSymbol {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.owned_bytes().map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNativeExternalSymbolKey {
    target_profile: DecodedCapabilityId,
    native_link_symbol: DecodedNativeLinkSymbol,
}

impl DecodedNativeExternalSymbolKey {
    pub fn validate(self) -> Result<NativeExternalSymbolKey, NativeLinkValidationError> {
        resolve_target_profile(self.target_profile)?;
        Ok(
            NativeExternalSymbolKey::from_validated_darwin_macho_external(
                self.native_link_symbol.validate_darwin()?,
            ),
        )
    }
}

impl WireEncode for DecodedNativeExternalSymbolKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target_profile.encode(encoder)?;
        encoder.field(2)?;
        self.native_link_symbol.encode(encoder)
    }
}

impl WireDecode for DecodedNativeExternalSymbolKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            target_profile: decoder.field(1, DecodedCapabilityId::decode)?,
            native_link_symbol: decoder.field(2, DecodedNativeLinkSymbol::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalNativeGroupName(String);

impl DecodedCanonicalNativeGroupName {
    pub fn validate(self) -> Result<CanonicalNativeGroupName, CanonicalNativeNameError> {
        CanonicalNativeGroupName::from_owned(self.0)
    }
}

impl WireEncode for DecodedCanonicalNativeGroupName {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.text(&self.0)
    }
}

impl WireDecode for DecodedCanonicalNativeGroupName {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.owned_text().map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNativeLibraryGrouping {
    Independent,
    OrderedGroup {
        name: DecodedCanonicalNativeGroupName,
        position: u32,
    },
}

impl DecodedNativeLibraryGrouping {
    pub fn validate(self) -> Result<NativeLibraryGrouping, NativeLinkValidationError> {
        match self {
            Self::Independent => Ok(NativeLibraryGrouping::Independent),
            Self::OrderedGroup { name, position } => Ok(NativeLibraryGrouping::OrderedGroup {
                name: name.validate().map_err(NativeLinkValidationError::Group)?,
                position,
            }),
        }
    }
}

impl WireEncode for DecodedNativeLibraryGrouping {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Independent => encode_empty_sum(encoder, 1),
            Self::OrderedGroup { name, position } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                name.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(*position))
            }
        }
    }
}

impl WireDecode for DecodedNativeLibraryGrouping {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Independent)
            }
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::OrderedGroup {
                    name: decoder.field(1, DecodedCanonicalNativeGroupName::decode)?,
                    position: decoder.field(2, Decoder::u32)?,
                })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedNativeLinkRequirementKey {
    target_profile: DecodedCapabilityId,
    library: DecodedCanonicalNativeLibraryName,
    kind: NativeLibraryKind,
    grouping: DecodedNativeLibraryGrouping,
}

impl DecodedNativeLinkRequirementKey {
    pub fn validate(self) -> Result<NativeLinkRequirementKey, NativeLinkValidationError> {
        resolve_target_profile(self.target_profile)?;
        let library = self
            .library
            .validate()
            .map_err(NativeLinkValidationError::Library)?;
        let grouping = self.grouping.validate()?;
        Ok(NativeLinkRequirementKey::for_darwin(
            library, self.kind, grouping,
        ))
    }
}

impl WireEncode for DecodedNativeLinkRequirementKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.target_profile.encode(encoder)?;
        encoder.field(2)?;
        self.library.encode(encoder)?;
        encoder.field(3)?;
        self.kind.encode(encoder)?;
        encoder.field(4)?;
        self.grouping.encode(encoder)
    }
}

impl WireDecode for DecodedNativeLinkRequirementKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            target_profile: decoder.field(1, DecodedCapabilityId::decode)?,
            library: decoder.field(2, DecodedCanonicalNativeLibraryName::decode)?,
            kind: decoder.field(3, NativeLibraryKind::decode)?,
            grouping: decoder.field(4, DecodedNativeLibraryGrouping::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedNativeLibraryBinding {
    DefaultNativeNamespace,
    Requirement(DecodedPersistentId<NativeLinkRequirementId>),
}

impl DecodedNativeLibraryBinding {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<NativeLibraryBinding, E>
    where
        R: PersistentIdResolver<NativeLinkRequirementId, Error = E>,
    {
        match self {
            Self::DefaultNativeNamespace => Ok(NativeLibraryBinding::DefaultNativeNamespace),
            Self::Requirement(id) => resolver.resolve(id).map(NativeLibraryBinding::Requirement),
        }
    }
}

impl WireEncode for DecodedNativeLibraryBinding {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::DefaultNativeNamespace => encode_empty_sum(encoder, 1),
            Self::Requirement(id) => encode_value_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedNativeLibraryBinding {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::DefaultNativeNamespace)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Requirement)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for NativeLibraryKind {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::TargetDefault),
            2 => Ok(Self::Dynamic),
            3 => Ok(Self::StaticArchive),
            4 => Ok(Self::Framework),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeLinkValidationError {
    TargetCapability(CapabilityIdError),
    TargetProfile(CapabilityRefinementError),
    Library(CanonicalNativeNameError),
    Group(CanonicalNativeNameError),
    MissingMachOExternalPrefix,
    SourceSymbol(SourceNativeSymbolError),
    LinkSymbol(NativeLinkSymbolError),
}

impl fmt::Display for NativeLinkValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetCapability(error) => error.fmt(formatter),
            Self::TargetProfile(error) => error.fmt(formatter),
            Self::Library(error) => error.fmt(formatter),
            Self::Group(error) => error.fmt(formatter),
            Self::MissingMachOExternalPrefix => {
                formatter.write_str("Mach-O external symbol must start with '_'")
            }
            Self::SourceSymbol(error) => error.fmt(formatter),
            Self::LinkSymbol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for NativeLinkValidationError {}

fn resolve_target_profile(
    target: DecodedCapabilityId,
) -> Result<TargetProfileWireId, NativeLinkValidationError> {
    let target = target
        .validate()
        .map_err(NativeLinkValidationError::TargetCapability)?;
    TargetProfileWireId::refine(target).map_err(NativeLinkValidationError::TargetProfile)
}

fn decode_sum_header(decoder: &mut Decoder<'_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests;
