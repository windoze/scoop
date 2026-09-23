use scoop_identity::{DecodedPersistentId, PersistentIdResolver, PersistentPropertyAccessorId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    PropertyCapabilityBuildError, PropertyCapabilityResolutionError, PropertySetterPublicAccessV1,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyCapabilityV1 {
    kind: PropertyCapabilityKindV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum PropertyCapabilityKindV1 {
    ReadOnly {
        getter: PersistentPropertyAccessorId,
    },
    ReadWrite {
        getter: PersistentPropertyAccessorId,
        setter: PersistentPropertyAccessorId,
        setter_access: PropertySetterPublicAccessV1,
    },
}

impl PropertyCapabilityV1 {
    pub(crate) const fn from_accessors(
        accessors: super::PropertyAccessorsV1,
        setter_access: PropertySetterPublicAccessV1,
    ) -> Self {
        let getter = accessors.getter();
        match accessors.setter() {
            None => Self::read_only(getter),
            Some(setter) => Self {
                kind: PropertyCapabilityKindV1::ReadWrite {
                    getter,
                    setter,
                    setter_access,
                },
            },
        }
    }

    pub const fn read_only(getter: PersistentPropertyAccessorId) -> Self {
        Self {
            kind: PropertyCapabilityKindV1::ReadOnly { getter },
        }
    }

    pub fn try_read_write(
        getter: PersistentPropertyAccessorId,
        setter: PersistentPropertyAccessorId,
        setter_access: PropertySetterPublicAccessV1,
    ) -> Result<Self, PropertyCapabilityBuildError> {
        if getter == setter {
            return Err(PropertyCapabilityBuildError::DuplicateAccessor(getter));
        }
        Ok(Self {
            kind: PropertyCapabilityKindV1::ReadWrite {
                getter,
                setter,
                setter_access,
            },
        })
    }

    pub const fn getter(self) -> PersistentPropertyAccessorId {
        match self.kind {
            PropertyCapabilityKindV1::ReadOnly { getter }
            | PropertyCapabilityKindV1::ReadWrite { getter, .. } => getter,
        }
    }

    pub const fn setter(self) -> Option<PersistentPropertyAccessorId> {
        match self.kind {
            PropertyCapabilityKindV1::ReadOnly { .. } => None,
            PropertyCapabilityKindV1::ReadWrite { setter, .. } => Some(setter),
        }
    }

    pub const fn setter_access(self) -> Option<PropertySetterPublicAccessV1> {
        match self.kind {
            PropertyCapabilityKindV1::ReadOnly { .. } => None,
            PropertyCapabilityKindV1::ReadWrite { setter_access, .. } => Some(setter_access),
        }
    }

    pub const fn is_read_only(self) -> bool {
        matches!(self.kind, PropertyCapabilityKindV1::ReadOnly { .. })
    }
}

impl WireEncode for PropertyCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.kind {
            PropertyCapabilityKindV1::ReadOnly { getter } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                getter.encode(encoder)
            }
            PropertyCapabilityKindV1::ReadWrite {
                getter,
                setter,
                setter_access,
            } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                getter.encode(encoder)?;
                encoder.field(2)?;
                setter.encode(encoder)?;
                encoder.field(3)?;
                setter_access.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedPropertyCapabilityV1 {
    kind: DecodedPropertyCapabilityKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodedPropertyCapabilityKindV1 {
    ReadOnly {
        getter: DecodedPersistentId<PersistentPropertyAccessorId>,
    },
    ReadWrite {
        getter: DecodedPersistentId<PersistentPropertyAccessorId>,
        setter: DecodedPersistentId<PersistentPropertyAccessorId>,
        setter_access: PropertySetterPublicAccessV1,
    },
}

impl DecodedPropertyCapabilityV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PropertyCapabilityV1, PropertyCapabilityResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        match self.kind {
            DecodedPropertyCapabilityKindV1::ReadOnly { getter } => resolver
                .resolve(getter)
                .map(PropertyCapabilityV1::read_only)
                .map_err(PropertyCapabilityResolutionError::Getter),
            DecodedPropertyCapabilityKindV1::ReadWrite {
                getter,
                setter,
                setter_access,
            } => {
                let getter = resolver
                    .resolve(getter)
                    .map_err(PropertyCapabilityResolutionError::Getter)?;
                let setter = resolver
                    .resolve(setter)
                    .map_err(PropertyCapabilityResolutionError::Setter)?;
                PropertyCapabilityV1::try_read_write(getter, setter, setter_access)
                    .map_err(PropertyCapabilityResolutionError::Capability)
            }
        }
    }
}

impl WireEncode for DecodedPropertyCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.kind {
            DecodedPropertyCapabilityKindV1::ReadOnly { getter } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                getter.encode(encoder)
            }
            DecodedPropertyCapabilityKindV1::ReadWrite {
                getter,
                setter,
                setter_access,
            } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                getter.encode(encoder)?;
                encoder.field(2)?;
                setter.encode(encoder)?;
                encoder.field(3)?;
                setter_access.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedPropertyCapabilityV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let kind = match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                DecodedPropertyCapabilityKindV1::ReadOnly {
                    getter: decoder.field(1, DecodedPersistentId::decode)?,
                }
            }
            2 => {
                expect_sum_length(decoder, fields, 4)?;
                DecodedPropertyCapabilityKindV1::ReadWrite {
                    getter: decoder.field(1, DecodedPersistentId::decode)?,
                    setter: decoder.field(2, DecodedPersistentId::decode)?,
                    setter_access: decoder.field(3, PropertySetterPublicAccessV1::decode)?,
                }
            }
            tag => return Err(unknown_tag(decoder, tag)),
        };
        Ok(Self { kind })
    }
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
