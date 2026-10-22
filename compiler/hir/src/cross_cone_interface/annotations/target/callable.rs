use super::*;
use scoop_identity::{
    CallableTemplateOrigin, DecodedCallableTemplateOrigin, PersistentGeneratedCallableId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AnnotationCallableV1 {
    Declaration(CallableTemplateOrigin),
    Local(PersistentGeneratedCallableId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum DecodedAnnotationCallableV1 {
    Declaration(DecodedCallableTemplateOrigin),
    Local(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl DecodedAnnotationCallableV1 {
    pub(super) fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotationCallableV1, E> {
        match self {
            Self::Declaration(id) => id.resolve(resolver).map(AnnotationCallableV1::Declaration),
            Self::Local(id) => resolver.resolve(id).map(AnnotationCallableV1::Local),
        }
    }
}

macro_rules! encode_callable {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(match self {
                    Self::Declaration(_) => 1,
                    Self::Local(_) => 2,
                })?;
                encoder.field(1)?;
                match self {
                    Self::Declaration(id) => id.encode(encoder),
                    Self::Local(id) => id.encode(encoder),
                }
            }
        }
    };
}
encode_callable!(AnnotationCallableV1);
encode_callable!(DecodedAnnotationCallableV1);

impl WireDecode for DecodedAnnotationCallableV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedCallableTemplateOrigin::decode)
                .map(Self::Declaration),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Local),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
