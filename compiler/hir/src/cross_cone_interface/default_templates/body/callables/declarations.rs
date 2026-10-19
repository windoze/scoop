use super::*;

/// Stable declaration identity accepted by a callable use in a default body.
///
/// Constructors remain a distinct reference domain. Generated callables are
/// admitted here because lexical closure bodies have no source declaration id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultCallableDeclarationV1 {
    Function(scoop_identity::PersistentFunctionId),
    GenericFunction(scoop_identity::PersistentGenericFunctionId),
    PropertyAccessor(scoop_identity::PersistentPropertyAccessorId),
    Generated(PersistentGeneratedCallableId),
}

impl DefaultCallableDeclarationV1 {
    pub const fn template_owner(self) -> scoop_identity::CallableTemplateOwner {
        use scoop_identity::CallableTemplateOwner as Owner;
        match self {
            Self::Function(id) => Owner::Function(id),
            Self::GenericFunction(id) => Owner::GenericFunction(id),
            Self::PropertyAccessor(id) => Owner::Accessor(id),
            Self::Generated(id) => Owner::Generated(id),
        }
    }
}

impl WireEncode for DefaultCallableDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Function(_) => 1,
            Self::GenericFunction(_) => 2,
            Self::PropertyAccessor(_) => 3,
            Self::Generated(_) => 4,
        })?;
        encoder.field(1)?;
        match self {
            Self::Function(id) => id.encode(encoder),
            Self::GenericFunction(id) => id.encode(encoder),
            Self::PropertyAccessor(id) => id.encode(encoder),
            Self::Generated(id) => id.encode(encoder),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedDefaultCallableDeclarationV1 {
    Function(DecodedPersistentId<scoop_identity::PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<scoop_identity::PersistentGenericFunctionId>),
    PropertyAccessor(DecodedPersistentId<scoop_identity::PersistentPropertyAccessorId>),
    Generated(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl DecodedDefaultCallableDeclarationV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefaultCallableDeclarationV1, E>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::Function(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::Function),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::GenericFunction),
            Self::PropertyAccessor(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::PropertyAccessor),
            Self::Generated(id) => resolver
                .resolve(id)
                .map(DefaultCallableDeclarationV1::Generated),
        }
    }
}

impl WireEncode for DecodedDefaultCallableDeclarationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Function(_) => 1,
            Self::GenericFunction(_) => 2,
            Self::PropertyAccessor(_) => 3,
            Self::Generated(_) => 4,
        })?;
        encoder.field(1)?;
        match self {
            Self::Function(id) => id.encode(encoder),
            Self::GenericFunction(id) => id.encode(encoder),
            Self::PropertyAccessor(id) => id.encode(encoder),
            Self::Generated(id) => id.encode(encoder),
        }
    }
}

impl WireDecode for DecodedDefaultCallableDeclarationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::PropertyAccessor),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Generated),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
