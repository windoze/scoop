use scoop_identity::{DecodedPersistentId, PersistentIdResolver, PersistentPropertyAccessorId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    PropertyCapabilityBuildError, PropertyCapabilityResolutionError, PropertyCapabilityV1,
};

/// Complete logical accessor identities, independent of public lookup.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyAccessorsV1 {
    getter: PersistentPropertyAccessorId,
    setter: Option<PersistentPropertyAccessorId>,
}

impl PropertyAccessorsV1 {
    pub const fn read_only(getter: PersistentPropertyAccessorId) -> Self {
        Self {
            getter,
            setter: None,
        }
    }

    pub fn try_read_write(
        getter: PersistentPropertyAccessorId,
        setter: PersistentPropertyAccessorId,
    ) -> Result<Self, PropertyCapabilityBuildError> {
        if getter == setter {
            return Err(PropertyCapabilityBuildError::DuplicateAccessor(getter));
        }
        Ok(Self {
            getter,
            setter: Some(setter),
        })
    }

    pub const fn getter(self) -> PersistentPropertyAccessorId {
        self.getter
    }
    pub const fn setter(self) -> Option<PersistentPropertyAccessorId> {
        self.setter
    }
    pub const fn is_read_only(self) -> bool {
        self.setter.is_none()
    }

    pub(crate) const fn from_capability(capability: PropertyCapabilityV1) -> Self {
        Self {
            getter: capability.getter(),
            setter: capability.setter(),
        }
    }
}

impl WireEncode for PropertyAccessorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_accessors(&self.getter, self.setter.as_ref(), encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedPropertyAccessorsV1 {
    getter: DecodedPersistentId<PersistentPropertyAccessorId>,
    setter: Option<DecodedPersistentId<PersistentPropertyAccessorId>>,
}

impl DecodedPropertyAccessorsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PropertyAccessorsV1, PropertyCapabilityResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        let getter = resolver
            .resolve(self.getter)
            .map_err(PropertyCapabilityResolutionError::Getter)?;
        match self.setter {
            None => Ok(PropertyAccessorsV1::read_only(getter)),
            Some(setter) => {
                let setter = resolver
                    .resolve(setter)
                    .map_err(PropertyCapabilityResolutionError::Setter)?;
                PropertyAccessorsV1::try_read_write(getter, setter)
                    .map_err(PropertyCapabilityResolutionError::Capability)
            }
        }
    }
}

impl WireEncode for DecodedPropertyAccessorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_accessors(&self.getter, self.setter.as_ref(), encoder)
    }
}

fn encode_accessors<T: WireEncode>(
    getter: &T,
    setter: Option<&T>,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(if setter.is_some() { 3 } else { 2 })?;
    encoder.field(0)?;
    encoder.unsigned(if setter.is_some() { 3 } else { 1 })?;
    encoder.field(1)?;
    getter.encode(encoder)?;
    if let Some(setter) = setter {
        encoder.field(2)?;
        setter.encode(encoder)?;
    }
    Ok(())
}

impl WireDecode for DecodedPropertyAccessorsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 2,
            3 => 3,
            tag => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        if fields != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        let getter = decoder.field(1, DecodedPersistentId::decode)?;
        let setter = if tag == 3 {
            Some(decoder.field(2, DecodedPersistentId::decode)?)
        } else {
            None
        };
        Ok(Self { getter, setter })
    }
}
