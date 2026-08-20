//! A portable delegate reference retains its declaration and ordered binder arguments.

use scoop_identity::{
    DecodedPersistentId, DecodedSignatureTypeKey, NonEmptyVec, PersistentExtensionPropertyId,
    PersistentIdResolver, SignatureTypeKey,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultGenericDelegateReferenceV1 {
    property: PersistentExtensionPropertyId,
    arguments: NonEmptyVec<SignatureTypeKey>,
}

impl DefaultGenericDelegateReferenceV1 {
    pub fn new(
        property: PersistentExtensionPropertyId,
        arguments: NonEmptyVec<SignatureTypeKey>,
    ) -> Self {
        Self {
            property,
            arguments,
        }
    }

    pub const fn property(&self) -> PersistentExtensionPropertyId {
        self.property
    }

    pub fn arguments(&self) -> &[SignatureTypeKey] {
        self.arguments.as_slice()
    }
}

impl WireEncode for DefaultGenericDelegateReferenceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.property.encode(encoder)?;
        encoder.field(2)?;
        encode_arguments(encoder, self.arguments.as_slice())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultGenericDelegateReferenceV1 {
    property: DecodedPersistentId<PersistentExtensionPropertyId>,
    arguments: NonEmptyVec<DecodedSignatureTypeKey>,
}

impl DecodedDefaultGenericDelegateReferenceV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefaultGenericDelegateReferenceV1, E>
    where
        R: crate::SignatureTypeReferenceResolver<E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>,
    {
        let property = resolver.resolve(self.property)?;
        let arguments = self
            .arguments
            .into_vec()
            .into_iter()
            .map(|argument| argument.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DefaultGenericDelegateReferenceV1::new(
            property,
            NonEmptyVec::new(arguments).expect("resolution preserves non-empty arguments"),
        ))
    }
}

impl WireDecode for DecodedDefaultGenericDelegateReferenceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let property = decoder.field(1, DecodedPersistentId::decode)?;
        let arguments = decoder.field(2, |decoder| {
            let arguments =
                decoder.decode_array(|decoder, _| DecodedSignatureTypeKey::decode(decoder))?;
            NonEmptyVec::new(arguments).map_err(|_| {
                WireError::new(
                    WireErrorKind::InvalidLength {
                        expected: 1,
                        actual: 0,
                    },
                    decoder.path().clone(),
                    Some(decoder.position()),
                )
            })
        })?;
        Ok(Self {
            property,
            arguments,
        })
    }
}

impl WireEncode for DecodedDefaultGenericDelegateReferenceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.property.encode(encoder)?;
        encoder.field(2)?;
        encode_arguments(encoder, self.arguments.as_slice())
    }
}

fn encode_arguments<T: WireEncode>(
    encoder: &mut Encoder,
    arguments: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(arguments.len() as u64)?;
    for argument in arguments {
        argument.encode(encoder)?;
    }
    Ok(())
}
