use super::*;

impl WireEncode for DecodedSelectedTypeUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        use encoding::payload;
        match self {
            Self::Signature { exact } => payload(encoder, 1, exact, None),
            Self::Representation { exact } => payload(encoder, 2, exact, None),
            Self::Construct { exact, declaration } => payload(encoder, 3, exact, Some(declaration)),
            Self::MemberCall {
                receiver,
                declaration,
            } => payload(encoder, 4, receiver, Some(declaration)),
            Self::SlotCall { receiver, slot } => payload(encoder, 5, receiver, Some(slot)),
            Self::TypeTest { exact } => payload(encoder, 6, exact, None),
            Self::SingletonValue { exact, value } => payload(encoder, 7, exact, Some(value)),
            Self::Inheritance { derived, edge } => payload(encoder, 8, derived, Some(edge)),
            Self::ShapeSupport { exact } => payload(encoder, 9, exact, None),
        }
    }
}
impl WireDecode for DecodedSelectedTypeUseV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        wire::expect_fields(
            decoder,
            fields,
            match tag {
                1 | 2 | 6 | 9 => 2,
                3 | 4 | 5 | 7 | 8 => 3,
                tag => return Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
            },
        )?;
        let exact = decoder.field(1, DecodedPersistentId::decode)?;
        Ok(match tag {
            1 => Self::Signature { exact },
            2 => Self::Representation { exact },
            3 => Self::Construct {
                exact,
                declaration: decoder.field(2, DecodedSelectedTypeConstructionV1::decode)?,
            },
            4 => Self::MemberCall {
                receiver: exact,
                declaration: decoder.field(2, DecodedInheritanceCallableDeclarationV1::decode)?,
            },
            5 => Self::SlotCall {
                receiver: exact,
                slot: decoder.field(2, DecodedPersistentId::decode)?,
            },
            6 => Self::TypeTest { exact },
            7 => Self::SingletonValue {
                exact,
                value: decoder.field(2, DecodedPersistentId::decode)?,
            },
            8 => Self::Inheritance {
                derived: exact,
                edge: decoder.field(2, DecodedSelectedDirectInheritanceEdgeV1::decode)?,
            },
            9 => Self::ShapeSupport { exact },
            tag => return Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        })
    }
}
