use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    DispatchRole, DispatchSlotKey, DispatchTableKey, DispatchTableRole, OptionalExactInterface,
};
use crate::{
    DecodedDispatchDeclarationOwner, DecodedPersistentId, PersistentExactTypeId,
    PersistentFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedDispatchSlotKey {
    owner: DecodedDispatchDeclarationOwner,
    role: DispatchRole,
}

impl DecodedDispatchSlotKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DispatchSlotKey, DispatchIdentityResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        match (self.owner, self.role) {
            (DecodedDispatchDeclarationOwner::Function(owner), DispatchRole::VirtualMethod) => {
                resolver
                    .resolve(owner)
                    .map(DispatchSlotKey::virtual_method)
                    .map_err(DispatchIdentityResolutionError::Reference)
            }
            (DecodedDispatchDeclarationOwner::Function(owner), DispatchRole::InterfaceMethod) => {
                resolver
                    .resolve(owner)
                    .map(DispatchSlotKey::interface_method)
                    .map_err(DispatchIdentityResolutionError::Reference)
            }
            (DecodedDispatchDeclarationOwner::Accessor(owner), DispatchRole::PropertyGetter) => {
                resolver
                    .resolve(owner)
                    .map(DispatchSlotKey::property_getter)
                    .map_err(DispatchIdentityResolutionError::Reference)
            }
            (DecodedDispatchDeclarationOwner::Accessor(owner), DispatchRole::PropertySetter) => {
                resolver
                    .resolve(owner)
                    .map(DispatchSlotKey::property_setter)
                    .map_err(DispatchIdentityResolutionError::Reference)
            }
            (DecodedDispatchDeclarationOwner::Function(_), role) => {
                Err(DispatchIdentityResolutionError::InvalidFunctionRole(role))
            }
            (DecodedDispatchDeclarationOwner::Accessor(_), role) => {
                Err(DispatchIdentityResolutionError::InvalidAccessorRole(role))
            }
        }
    }
}

impl WireEncode for DecodedDispatchSlotKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)
    }
}

impl WireDecode for DecodedDispatchSlotKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedDispatchDeclarationOwner::decode)?,
            role: decoder.field(2, DispatchRole::decode)?,
        })
    }
}

impl WireDecode for DispatchRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::VirtualMethod),
            2 => Ok(Self::InterfaceMethod),
            3 => Ok(Self::PropertyGetter),
            4 => Ok(Self::PropertySetter),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for DispatchTableRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::VTable),
            2 => Ok(Self::ITable),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedOptionalExactInterface {
    Absent,
    Present(DecodedPersistentId<PersistentExactTypeId>),
}

impl DecodedOptionalExactInterface {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<OptionalExactInterface, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::Absent => Ok(OptionalExactInterface::Absent),
            Self::Present(interface) => resolver
                .resolve(interface)
                .map(OptionalExactInterface::Present),
        }
    }
}

impl WireEncode for DecodedOptionalExactInterface {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty_sum(encoder, 1),
            Self::Present(interface) => encode_value_sum(encoder, 2, interface),
        }
    }
}

impl WireDecode for DecodedOptionalExactInterface {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Absent)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Present)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecodedDispatchTableKey {
    exact_type: DecodedPersistentId<PersistentExactTypeId>,
    role: DispatchTableRole,
    interface: DecodedOptionalExactInterface,
}

impl DecodedDispatchTableKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DispatchTableKey, DispatchIdentityResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        let exact_type = resolver
            .resolve(self.exact_type)
            .map_err(DispatchIdentityResolutionError::Reference)?;
        match (self.role, self.interface) {
            (DispatchTableRole::VTable, DecodedOptionalExactInterface::Absent) => {
                Ok(DispatchTableKey::vtable(exact_type))
            }
            (DispatchTableRole::ITable, DecodedOptionalExactInterface::Present(interface)) => {
                let interface = resolver
                    .resolve(interface)
                    .map_err(DispatchIdentityResolutionError::Reference)?;
                Ok(DispatchTableKey::itable(exact_type, interface))
            }
            (DispatchTableRole::VTable, DecodedOptionalExactInterface::Present(_)) => {
                Err(DispatchIdentityResolutionError::VTableInterfacePresent)
            }
            (DispatchTableRole::ITable, DecodedOptionalExactInterface::Absent) => {
                Err(DispatchIdentityResolutionError::ITableInterfaceAbsent)
            }
        }
    }
}

impl WireEncode for DecodedDispatchTableKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.exact_type.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.interface.encode(encoder)
    }
}

impl WireDecode for DecodedDispatchTableKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            exact_type: decoder.field(1, DecodedPersistentId::decode)?,
            role: decoder.field(2, DispatchTableRole::decode)?,
            interface: decoder.field(3, DecodedOptionalExactInterface::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchIdentityResolutionError<E> {
    Reference(E),
    InvalidFunctionRole(DispatchRole),
    InvalidAccessorRole(DispatchRole),
    VTableInterfacePresent,
    ITableInterfaceAbsent,
}

impl<E: fmt::Display> fmt::Display for DispatchIdentityResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::InvalidFunctionRole(role) => {
                write!(
                    formatter,
                    "dispatch role {role:?} is invalid for a function"
                )
            }
            Self::InvalidAccessorRole(role) => {
                write!(
                    formatter,
                    "dispatch role {role:?} is invalid for an accessor"
                )
            }
            Self::VTableInterfacePresent => {
                formatter.write_str("a vtable must not carry an interface identity")
            }
            Self::ITableInterfaceAbsent => {
                formatter.write_str("an itable must carry an interface identity")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DispatchIdentityResolutionError<E> {}

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
