use scoop_identity::{DecodedPersistentId, PersistentIdResolver};
use scoop_wire::WireDecode;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedStrongTypeDescriptorRefV2 {
    Local(DecodedPersistentId<PersistentExactTypeId>),
    DependencyExternal {
        provider: DecodedPersistentId<ConeIdentity>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedStrongTypeDescriptorRefV2 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<StrongTypeDescriptorRefV2, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<ConeIdentity, Error = E>,
    {
        Ok(match self {
            Self::Local(exact) => StrongTypeDescriptorRefV2::Local(resolver.resolve(exact)?),
            Self::DependencyExternal { provider, exact } => {
                StrongTypeDescriptorRefV2::DependencyExternal {
                    provider: resolver.resolve(provider)?,
                    exact: resolver.resolve(exact)?,
                }
            }
        })
    }
}

impl WireEncode for DecodedStrongTypeDescriptorRefV2 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local(exact) => value(e, 1, exact),
            Self::DependencyExternal { provider, exact } => dependency(e, 3, provider, exact),
        }
    }
}

impl WireDecode for DecodedStrongTypeDescriptorRefV2 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = d.map()?;
        let kind = d.field(0, Decoder::unsigned)?;
        match kind {
            1 => {
                require_fields(d, fields, 2)?;
                Ok(Self::Local(d.field(1, DecodedPersistentId::decode)?))
            }
            3 => {
                require_fields(d, fields, 3)?;
                Ok(Self::DependencyExternal {
                    provider: d.field(1, DecodedPersistentId::decode)?,
                    exact: d.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(error(d, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedOptionalStrongTypeDescriptorRefV2 {
    Absent,
    Local(DecodedPersistentId<PersistentExactTypeId>),
    DependencyExternal {
        provider: DecodedPersistentId<ConeIdentity>,
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}

impl DecodedOptionalStrongTypeDescriptorRefV2 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<OptionalStrongTypeDescriptorRefV2, E>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<ConeIdentity, Error = E>,
    {
        Ok(match self {
            Self::Absent => OptionalStrongTypeDescriptorRefV2::Absent,
            Self::Local(exact) => {
                OptionalStrongTypeDescriptorRefV2::Local(resolver.resolve(exact)?)
            }
            Self::DependencyExternal { provider, exact } => {
                OptionalStrongTypeDescriptorRefV2::DependencyExternal {
                    provider: resolver.resolve(provider)?,
                    exact: resolver.resolve(exact)?,
                }
            }
        })
    }
}

impl WireEncode for DecodedOptionalStrongTypeDescriptorRefV2 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => OptionalStrongTypeDescriptorRefV2::Absent.encode(e),
            Self::Local(exact) => value(e, 2, exact),
            Self::DependencyExternal { provider, exact } => dependency(e, 4, provider, exact),
        }
    }
}

impl WireDecode for DecodedOptionalStrongTypeDescriptorRefV2 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = d.map()?;
        let kind = d.field(0, Decoder::unsigned)?;
        match kind {
            1 => {
                require_fields(d, fields, 2)?;
                let marker = d.field(1, Decoder::unsigned)?;
                if marker == 0 {
                    Ok(Self::Absent)
                } else {
                    Err(error(d, WireErrorKind::UnknownTag { tag: marker }))
                }
            }
            2 => {
                require_fields(d, fields, 2)?;
                Ok(Self::Local(d.field(1, DecodedPersistentId::decode)?))
            }
            4 => {
                require_fields(d, fields, 3)?;
                Ok(Self::DependencyExternal {
                    provider: d.field(1, DecodedPersistentId::decode)?,
                    exact: d.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(error(d, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedStrongTypeDispatchCallableRefV2 {
    Local(DecodedPersistentId<PersistentCallableBodyId>),
    Runtime(RuntimeFunction),
    DependencyExternal {
        provider: DecodedPersistentId<ConeIdentity>,
        body: DecodedPersistentId<PersistentCallableBodyId>,
    },
}

impl DecodedStrongTypeDispatchCallableRefV2 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<StrongTypeDispatchCallableRefV2, E>
    where
        R: PersistentIdResolver<PersistentCallableBodyId, Error = E>
            + PersistentIdResolver<ConeIdentity, Error = E>,
    {
        Ok(match self {
            Self::Local(body) => StrongTypeDispatchCallableRefV2::Local(resolver.resolve(body)?),
            Self::Runtime(function) => StrongTypeDispatchCallableRefV2::Runtime(function),
            Self::DependencyExternal { provider, body } => {
                StrongTypeDispatchCallableRefV2::DependencyExternal {
                    provider: resolver.resolve(provider)?,
                    body: resolver.resolve(body)?,
                }
            }
        })
    }
}

impl WireEncode for DecodedStrongTypeDispatchCallableRefV2 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Local(body) => value(e, 1, body),
            Self::Runtime(function) => runtime(e, *function),
            Self::DependencyExternal { provider, body } => dependency(e, 4, provider, body),
        }
    }
}

impl WireDecode for DecodedStrongTypeDispatchCallableRefV2 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = d.map()?;
        let kind = d.field(0, Decoder::unsigned)?;
        match kind {
            1 => {
                require_fields(d, fields, 2)?;
                Ok(Self::Local(d.field(1, DecodedPersistentId::decode)?))
            }
            3 => {
                require_fields(d, fields, 2)?;
                d.field(1, |d| {
                    d.expect_map(2)?;
                    let family = d.field(1, Decoder::unsigned)?;
                    let function = d.field(2, Decoder::unsigned)?;
                    RuntimeFunction::from_wire_tags(family, function)
                        .map(Self::Runtime)
                        .ok_or_else(|| error(d, WireErrorKind::UnknownTag { tag: function }))
                })
            }
            4 => {
                require_fields(d, fields, 3)?;
                Ok(Self::DependencyExternal {
                    provider: d.field(1, DecodedPersistentId::decode)?,
                    body: d.field(2, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(error(d, WireErrorKind::UnknownTag { tag })),
        }
    }
}
