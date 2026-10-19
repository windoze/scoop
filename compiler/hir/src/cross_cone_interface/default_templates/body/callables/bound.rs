use super::*;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultBoundCallableSourceV1 {
    Class {
        bound: SignatureTypeKey,
        callable: DefaultCallableRefV1,
    },
    Interface {
        bound: SignatureTypeKey,
        member: CallableTemplateOrigin,
    },
}

impl WireEncode for DefaultBoundCallableSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Class { .. } => 1,
            Self::Interface { .. } => 2,
        })?;
        match self {
            Self::Class { bound, callable } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                callable.encode(encoder)
            }
            Self::Interface { bound, member } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                member.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultBoundCallableSourceV1 {
    Class {
        bound: DecodedSignatureTypeKey,
        callable: DecodedDefaultCallableRefV1,
    },
    Interface {
        bound: DecodedSignatureTypeKey,
        member: DecodedCallableTemplateOrigin,
    },
}

impl DecodedDefaultBoundCallableSourceV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultBoundCallableSourceV1, DefaultBoundCallableSourceResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::Class { bound, callable } => Ok(DefaultBoundCallableSourceV1::Class {
                bound: bound
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Bound)?,
                callable: callable
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Callable)?,
            }),
            Self::Interface { bound, member } => Ok(DefaultBoundCallableSourceV1::Interface {
                bound: bound
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Bound)?,
                member: member
                    .resolve(resolver)
                    .map_err(DefaultBoundCallableSourceResolutionError::Member)?,
            }),
        }
    }
}

impl WireEncode for DecodedDefaultBoundCallableSourceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Class { .. } => 1,
            Self::Interface { .. } => 2,
        })?;
        match self {
            Self::Class { bound, callable } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                callable.encode(encoder)
            }
            Self::Interface { bound, member } => {
                encoder.field(1)?;
                bound.encode(encoder)?;
                encoder.field(2)?;
                member.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDefaultBoundCallableSourceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 3)?;
        match tag {
            1 => Ok(Self::Class {
                bound: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                callable: decoder.field(2, DecodedDefaultCallableRefV1::decode)?,
            }),
            2 => Ok(Self::Interface {
                bound: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                member: decoder.field(2, DecodedCallableTemplateOrigin::decode)?,
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefaultBoundCallableRefV1 {
    receiver_type: SignatureTypeKey,
    source: DefaultBoundCallableSourceV1,
    instantiated_signature: SignatureTypeKey,
}

impl DefaultBoundCallableRefV1 {
    pub const fn new(
        receiver_type: SignatureTypeKey,
        source: DefaultBoundCallableSourceV1,
        instantiated_signature: SignatureTypeKey,
    ) -> Self {
        Self {
            receiver_type,
            source,
            instantiated_signature,
        }
    }

    pub const fn receiver_type(&self) -> &SignatureTypeKey {
        &self.receiver_type
    }

    pub const fn source(&self) -> &DefaultBoundCallableSourceV1 {
        &self.source
    }

    pub const fn instantiated_signature(&self) -> &SignatureTypeKey {
        &self.instantiated_signature
    }
}

impl WireEncode for DefaultBoundCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.receiver_type.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.instantiated_signature.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultBoundCallableRefV1 {
    receiver_type: DecodedSignatureTypeKey,
    source: DecodedDefaultBoundCallableSourceV1,
    instantiated_signature: DecodedSignatureTypeKey,
}

impl DecodedDefaultBoundCallableRefV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultBoundCallableRefV1, DefaultBoundCallableRefResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        Ok(DefaultBoundCallableRefV1 {
            receiver_type: self
                .receiver_type
                .resolve(resolver)
                .map_err(DefaultBoundCallableRefResolutionError::ReceiverType)?,
            source: self
                .source
                .resolve(resolver)
                .map_err(DefaultBoundCallableRefResolutionError::Source)?,
            instantiated_signature: self
                .instantiated_signature
                .resolve(resolver)
                .map_err(DefaultBoundCallableRefResolutionError::InstantiatedSignature)?,
        })
    }
}

impl WireEncode for DecodedDefaultBoundCallableRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.receiver_type.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.instantiated_signature.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultBoundCallableRefV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            receiver_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
            source: decoder.field(2, DecodedDefaultBoundCallableSourceV1::decode)?,
            instantiated_signature: decoder.field(3, DecodedSignatureTypeKey::decode)?,
        })
    }
}
