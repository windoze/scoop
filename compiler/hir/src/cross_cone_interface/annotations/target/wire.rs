use super::*;

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
                    Self::ExtensionProperty(_) => 6,
                    Self::Annotation(_) => 7,
                    Self::Callable(_) => 8,
                    Self::Parameter { .. } => 9,
                    Self::CallableReceiver(_) => 10,
                    Self::PropertyReceiver(_) => 11,
                    Self::TypeAlias(_) => 12,
                    Self::PropertyStorage { .. } => 13,
                })?;
                encoder.field(1)?;
                match self {
                    Self::Nominal(id) => id.encode(encoder),
                    Self::Variant(id) => id.encode(encoder),
                    Self::Field(id) => id.encode(encoder),
                    Self::VariantField(id) => id.encode(encoder),
                    Self::Property(id) => id.encode(encoder),
                    Self::ExtensionProperty(id) => id.encode(encoder),
                    Self::Annotation(id) => id.encode(encoder),
                    Self::Callable(id) | Self::CallableReceiver(id) => id.encode(encoder),
                    Self::PropertyReceiver(id) => id.encode(encoder),
                    Self::TypeAlias(id) => id.encode(encoder),
                    Self::Parameter { callable, index } => {
                        encoder.map(2)?;
                        encoder.field(1)?;
                        callable.encode(encoder)?;
                        encoder.field(2)?;
                        encoder.unsigned(u64::from(*index))
                    }
                    Self::PropertyStorage { property, role } => {
                        encoder.map(2)?;
                        encoder.field(1)?;
                        property.encode(encoder)?;
                        encoder.field(2)?;
                        role.encode(encoder)
                    }
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
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ExtensionProperty),
            7 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Annotation),
            8 => decoder
                .field(1, DecodedAnnotationCallableV1::decode)
                .map(Self::Callable),
            9 => decoder.field(1, |decoder| {
                decoder.expect_map(2)?;
                let callable = decoder.field(1, DecodedAnnotationCallableV1::decode)?;
                let index = u32::try_from(decoder.field(2, Decoder::unsigned)?).map_err(|_| {
                    WireError::new(
                        WireErrorKind::IntegerOutOfRange,
                        decoder.path().clone(),
                        Some(decoder.position()),
                    )
                })?;
                Ok(Self::Parameter { callable, index })
            }),
            10 => decoder
                .field(1, DecodedAnnotationCallableV1::decode)
                .map(Self::CallableReceiver),
            11 => decoder
                .field(1, DecodedPropertyOwner::decode)
                .map(Self::PropertyReceiver),
            12 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::TypeAlias),
            13 => decoder.field(1, |decoder| {
                decoder.expect_map(2)?;
                Ok(Self::PropertyStorage {
                    property: decoder.field(1, DecodedPropertyOwner::decode)?,
                    role: decoder.field(2, crate::AnnotationStorageRole::decode)?,
                })
            }),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}
