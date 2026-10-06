use super::*;
use scoop_identity::{DecodedPersistentId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedClassPrimaryConstructorV1 {
    constructor: DecodedPersistentId<PersistentConstructorId>,
    properties: Vec<Option<DecodedPersistentId<PersistentPropertyId>>>,
}

impl DecodedClassPrimaryConstructorV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ClassPrimaryConstructorV1, ClassPrimaryConstructorResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>,
    {
        use ClassPrimaryConstructorResolutionError as Error;
        let constructor = resolver
            .resolve(self.constructor)
            .map_err(Error::Reference)?;
        let properties = self
            .properties
            .into_iter()
            .map(|property| {
                property
                    .map(|id| resolver.resolve(id))
                    .transpose()
                    .map_err(Error::Reference)
            })
            .collect::<Result<Vec<_>, _>>()?;
        ClassPrimaryConstructorV1::try_new(constructor, properties).map_err(Error::Mapping)
    }
}

impl WireDecode for DecodedClassPrimaryConstructorV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            constructor: decoder.field(1, DecodedPersistentId::decode)?,
            properties: decoder.field(2, |d| d.decode_array(|d, _| optional(d)))?,
        })
    }
}

impl WireEncode for ClassPrimaryConstructorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(self.constructor, &self.properties, encoder)
    }
}

impl WireEncode for DecodedClassPrimaryConstructorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode(self.constructor, &self.properties, encoder)
    }
}

fn encode<C: WireEncode, P: WireEncode>(
    constructor: C,
    properties: &[Option<P>],
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(1)?;
    constructor.encode(encoder)?;
    encoder.field(2)?;
    encoder.array(properties.len() as u64)?;
    for property in properties {
        encoder.array(u64::from(property.is_some()))?;
        if let Some(property) = property {
            property.encode(encoder)?;
        }
    }
    Ok(())
}

pub(in crate::cross_cone_interface::nominal_interfaces::declaration) fn optional<T: WireDecode>(
    decoder: &mut Decoder<'_>,
) -> Result<Option<T>, WireError> {
    match decoder.array()? {
        0 => Ok(None),
        1 => T::decode(decoder).map(Some),
        actual => Err(WireError::new(
            WireErrorKind::InvalidLength {
                expected: 1,
                actual,
            },
            decoder.path().clone(),
            Some(decoder.position()),
        )),
    }
}

#[derive(Debug)]
pub enum ClassPrimaryConstructorResolutionError<E> {
    Reference(E),
    Mapping(ClassPrimaryConstructorBuildError),
}

impl<E: fmt::Display> fmt::Display for ClassPrimaryConstructorResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(e) => e.fmt(f),
            Self::Mapping(e) => e.fmt(f),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ClassPrimaryConstructorResolutionError<E>
{
}
