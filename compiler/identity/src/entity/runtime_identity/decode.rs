use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{LayoutKey, RepresentationRole, ScanKey, ScanRole};
use crate::{
    CapabilityIdError, CapabilityRefinementError, DecodedCapabilityId, DecodedPersistentId,
    PersistentExactTypeId, PersistentIdResolver, PersistentLayoutId, TargetProfileWireId,
};

mod immortal;
mod storage;

pub use immortal::DecodedImmortalObjectKey;
pub use storage::{DecodedStaticStorageKey, StaticStorageResolutionError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedLayoutKey {
    exact_type: DecodedPersistentId<PersistentExactTypeId>,
    target_profile: DecodedCapabilityId,
    representation: RepresentationRole,
}

impl DecodedLayoutKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<LayoutKey, LayoutKeyResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        let exact_type = resolver
            .resolve(self.exact_type)
            .map_err(LayoutKeyResolutionError::Reference)?;
        let target_profile = self
            .target_profile
            .validate()
            .map_err(LayoutKeyResolutionError::TargetCapability)?;
        let target_profile = TargetProfileWireId::refine(target_profile)
            .map_err(LayoutKeyResolutionError::TargetProfile)?;
        Ok(LayoutKey::new(
            exact_type,
            target_profile,
            self.representation,
        ))
    }
}

impl WireEncode for DecodedLayoutKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.target_profile.encode(encoder)?;
        encoder.field(3)?;
        self.representation.encode(encoder)
    }
}

impl WireDecode for DecodedLayoutKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            target_profile: decoder.field(2, DecodedCapabilityId::decode)?,
            representation: decoder.field(3, RepresentationRole::decode)?,
        })
    }
}

impl WireDecode for RepresentationRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::ManagedValue),
            2 => Ok(Self::ManagedObject),
            3 => Ok(Self::CValue),
            4 => Ok(Self::NativeFunctionPointer),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedScanKey {
    layout: DecodedPersistentId<PersistentLayoutId>,
    role: ScanRole,
}

impl DecodedScanKey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<ScanKey, E>
    where
        R: PersistentIdResolver<PersistentLayoutId, Error = E>,
    {
        resolver
            .resolve(self.layout)
            .map(|layout| ScanKey::new(layout, self.role))
    }
}

impl WireEncode for DecodedScanKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.layout.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl WireDecode for DecodedScanKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            layout: decoder.field(1, DecodedPersistentId::decode)?,
            role: decoder.field(2, ScanRole::decode)?,
        })
    }
}

impl WireDecode for ScanRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::InlineValue),
            2 => Ok(Self::ManagedObject),
            3 => Ok(Self::ArrayElement),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutKeyResolutionError<E> {
    Reference(E),
    TargetCapability(CapabilityIdError),
    TargetProfile(CapabilityRefinementError),
}

impl<E: fmt::Display> fmt::Display for LayoutKeyResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::TargetCapability(error) => error.fmt(formatter),
            Self::TargetProfile(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for LayoutKeyResolutionError<E> {}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    decode_value_variant(decoder, fields, DecodedPersistentId::decode, build)
}

fn decode_value_variant<V, T>(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    decode: impl FnOnce(&mut Decoder<'_, '_>) -> Result<V, WireError>,
    build: impl FnOnce(V) -> T,
) -> Result<T, WireError> {
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, decode).map(build)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
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

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
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

#[cfg(test)]
mod tests;
