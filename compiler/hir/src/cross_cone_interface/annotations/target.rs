use super::wire::AnnotationDataResolver;
use scoop_identity::{
    DecodedNominalDeclarationOwner, DecodedPersistentId, NominalDeclarationOwner,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId, PersistentPropertyId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AnnotationTargetV1 {
    Nominal(NominalDeclarationOwner),
    Variant(PersistentEnumVariantId),
    Field(PersistentFieldId),
    VariantField(PersistentEnumVariantFieldId),
    Property(PersistentPropertyId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedAnnotationTargetV1 {
    Nominal(DecodedNominalDeclarationOwner),
    Variant(DecodedPersistentId<PersistentEnumVariantId>),
    Field(DecodedPersistentId<PersistentFieldId>),
    VariantField(DecodedPersistentId<PersistentEnumVariantFieldId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
}

impl DecodedAnnotationTargetV1 {
    pub fn resolve<R: AnnotationDataResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<AnnotationTargetV1, E> {
        match self {
            Self::Nominal(id) => id.resolve(resolver).map(AnnotationTargetV1::Nominal),
            Self::Variant(id) => resolver.resolve(id).map(AnnotationTargetV1::Variant),
            Self::Field(id) => resolver.resolve(id).map(AnnotationTargetV1::Field),
            Self::VariantField(id) => resolver.resolve(id).map(AnnotationTargetV1::VariantField),
            Self::Property(id) => resolver.resolve(id).map(AnnotationTargetV1::Property),
        }
    }
}

macro_rules! encode_target {
    ($type:ty) => {
        impl WireEncode for $type {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(match self {
                    Self::Nominal(_) => 1,
                    Self::Variant(_) => 2,
                    Self::Field(_) => 3,
                    Self::VariantField(_) => 4,
                    Self::Property(_) => 5,
                })?;
                encoder.field(1)?;
                match self {
                    Self::Nominal(id) => id.encode(encoder),
                    Self::Variant(id) => id.encode(encoder),
                    Self::Field(id) => id.encode(encoder),
                    Self::VariantField(id) => id.encode(encoder),
                    Self::Property(id) => id.encode(encoder),
                }
            }
        }
    };
}
encode_target!(AnnotationTargetV1);
encode_target!(DecodedAnnotationTargetV1);

impl WireDecode for DecodedAnnotationTargetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedNominalDeclarationOwner::decode)
                .map(Self::Nominal),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Variant),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Field),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::VariantField),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
