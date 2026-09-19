use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedSelectedTypeConstructionV1 {
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    EnumVariant(DecodedPersistentId<PersistentEnumVariantId>),
}
impl DecodedSelectedTypeConstructionV1 {
    pub(super) fn resolve<R: SelectedTypeUseResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<SelectedTypeConstructionV1, E> {
        match self {
            Self::Constructor(id) => resolver
                .resolve(id)
                .map(SelectedTypeConstructionV1::Constructor),
            Self::EnumVariant(id) => resolver
                .resolve(id)
                .map(SelectedTypeConstructionV1::EnumVariant),
        }
    }
}
impl WireEncode for DecodedSelectedTypeConstructionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Constructor(id) => encoding::payload(encoder, 1, id, None),
            Self::EnumVariant(id) => encoding::payload(encoder, 2, id, None),
        }
    }
}
impl WireDecode for DecodedSelectedTypeConstructionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::EnumVariant),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedSelectedDirectInheritanceEdgeV1 {
    ClassBase {
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
    Interface {
        exact: DecodedPersistentId<PersistentExactTypeId>,
    },
}
impl DecodedSelectedDirectInheritanceEdgeV1 {
    pub(super) fn resolve<R: SelectedTypeUseResolver<E>, E>(
        self,
        resolver: &mut R,
    ) -> Result<SelectedDirectInheritanceEdgeV1, E> {
        match self {
            Self::ClassBase { exact } => resolver
                .resolve(exact)
                .map(|exact| SelectedDirectInheritanceEdgeV1::ClassBase { exact }),
            Self::Interface { exact } => resolver
                .resolve(exact)
                .map(|exact| SelectedDirectInheritanceEdgeV1::Interface { exact }),
        }
    }
}
impl WireEncode for DecodedSelectedDirectInheritanceEdgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ClassBase { exact } => encoding::payload(encoder, 1, exact, None),
            Self::Interface { exact } => encoding::payload(encoder, 2, exact, None),
        }
    }
}
impl WireDecode for DecodedSelectedDirectInheritanceEdgeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|exact| Self::ClassBase { exact }),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(|exact| Self::Interface { exact }),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
