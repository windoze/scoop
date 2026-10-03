use scoop_identity::{DecodedPersistentId, PersistentIdResolver, PersistentPropertyAccessorId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{PropertyAccessorImplementationV1, PropertyAccessorSourceV1, PropertyAccessorsV1};
use crate::PropertyCapabilityResolutionError;

impl WireEncode for PropertyAccessorsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_accessors(&self.getter, self.setter.as_ref(), encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedAccessor {
    accessor: DecodedPersistentId<PersistentPropertyAccessorId>,
    implementation: PropertyAccessorImplementationV1,
}

impl WireEncode for DecodedAccessor {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.accessor.encode(encoder)?;
        encoder.field(2)?;
        self.implementation.encode(encoder)
    }
}

impl WireDecode for DecodedAccessor {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            accessor: decoder.field(1, DecodedPersistentId::decode)?,
            implementation: decoder.field(2, PropertyAccessorImplementationV1::decode)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedPropertyAccessorsV1 {
    getter: DecodedAccessor,
    setter: Option<DecodedAccessor>,
}

impl DecodedPropertyAccessorsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<PropertyAccessorsV1, PropertyCapabilityResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        let getter = PropertyAccessorSourceV1::new(
            resolver
                .resolve(self.getter.accessor)
                .map_err(PropertyCapabilityResolutionError::Getter)?,
            self.getter.implementation,
        );
        match self.setter {
            None => Ok(PropertyAccessorsV1::read_only(getter)),
            Some(setter) => {
                let setter = PropertyAccessorSourceV1::new(
                    resolver
                        .resolve(setter.accessor)
                        .map_err(PropertyCapabilityResolutionError::Setter)?,
                    setter.implementation,
                );
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
    encoder.unsigned(if setter.is_some() { 5 } else { 4 })?;
    encoder.field(1)?;
    getter.encode(encoder)?;
    if let Some(setter) = setter {
        encoder.field(2)?;
        setter.encode(encoder)?;
    }
    Ok(())
}

impl WireDecode for DecodedPropertyAccessorsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            4 => 2,
            5 => 3,
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
        let getter = decoder.field(1, DecodedAccessor::decode)?;
        let setter = if tag == 5 {
            Some(decoder.field(2, DecodedAccessor::decode)?)
        } else {
            None
        };
        Ok(Self { getter, setter })
    }
}
