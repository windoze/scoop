//! Both registration versions share the closed provider-bearing wire codec.

use super::*;
use crate::{
    DecodedOptionalStrongTypeDescriptorRefV2 as Optional,
    DecodedStrongTypeDescriptorRefV2 as Descriptor,
    DecodedStrongTypeDispatchCallableRefV2 as Callable,
};
use scoop_identity::ConeIdentity;

#[derive(Debug)]
pub enum DecodedStrongTypeDescriptorRefV1 {
    Local(DecodedPersistentId<PersistentExactTypeId>),
    DependencyExternal {
        provider: DecodedPersistentId<ConeIdentity>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl WireEncode for DecodedStrongTypeDescriptorRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match *self {
            Self::Local(exact) => Descriptor::Local(exact),
            Self::DependencyExternal { provider, exact } => {
                Descriptor::DependencyExternal { provider, exact }
            }
        }
        .encode(encoder)
    }
}

impl WireDecode for DecodedStrongTypeDescriptorRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        Ok(match Descriptor::decode(decoder)? {
            Descriptor::Local(exact) => Self::Local(exact),
            Descriptor::DependencyExternal { provider, exact } => {
                Self::DependencyExternal { provider, exact }
            }
        })
    }
}

#[derive(Debug)]
pub enum DecodedOptionalStrongTypeDescriptorRefV1 {
    Absent,
    Local(DecodedPersistentId<PersistentExactTypeId>),
    DependencyExternal {
        provider: DecodedPersistentId<ConeIdentity>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl WireEncode for DecodedOptionalStrongTypeDescriptorRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match *self {
            Self::Absent => Optional::Absent,
            Self::Local(exact) => Optional::Local(exact),
            Self::DependencyExternal { provider, exact } => {
                Optional::DependencyExternal { provider, exact }
            }
        }
        .encode(encoder)
    }
}

impl WireDecode for DecodedOptionalStrongTypeDescriptorRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        Ok(match Optional::decode(decoder)? {
            Optional::Absent => Self::Absent,
            Optional::Local(exact) => Self::Local(exact),
            Optional::DependencyExternal { provider, exact } => {
                Self::DependencyExternal { provider, exact }
            }
        })
    }
}

#[derive(Debug)]
pub enum DecodedStrongTypeDispatchCallableRefV1 {
    Local(DecodedPersistentId<PersistentCallableBodyId>),
    DependencyExternal {
        provider: DecodedPersistentId<ConeIdentity>,
        body: DecodedPersistentId<PersistentCallableBodyId>,
    },
    Runtime(crate::RuntimeFunction),
}

impl WireEncode for DecodedStrongTypeDispatchCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match *self {
            Self::Local(body) => Callable::Local(body),
            Self::DependencyExternal { provider, body } => {
                Callable::DependencyExternal { provider, body }
            }
            Self::Runtime(function) => Callable::Runtime(function),
        }
        .encode(encoder)
    }
}

impl WireDecode for DecodedStrongTypeDispatchCallableRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        Ok(match Callable::decode(decoder)? {
            Callable::Local(body) => Self::Local(body),
            Callable::DependencyExternal { provider, body } => {
                Self::DependencyExternal { provider, body }
            }
            Callable::Runtime(function) => Self::Runtime(function),
        })
    }
}
