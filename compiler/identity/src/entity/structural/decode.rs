use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    DeclarationName, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};
use crate::{
    CanonicalIdentifierError, ConeIdentity, DecodedCanonicalIdentifier, DecodedPersistentId,
    DecodedSourceIdentity, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
    SourceIdentityResolutionError,
};

impl WireDecode for StructuralDefinitionSiteRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::LocalDeclaration),
            2 => Ok(Self::Lambda),
            3 => Ok(Self::DefaultValue),
            4 => Ok(Self::AnonymousObject),
            5 => Ok(Self::CoroutineTransform),
            6 => Ok(Self::CallbackConversion),
            7 => Ok(Self::CallableConversion),
            8 => Ok(Self::DispatchAdapter),
            9 => Ok(Self::Initializer),
            10 => Ok(Self::SynthesizedBridge),
            11 => Ok(Self::SyntheticValue),
            12 => Ok(Self::StringConstant),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireDecode for StructuralPathSegment {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self::new(
            decoder.field(1, StructuralDefinitionSiteRole::decode)?,
            decoder.field(2, Decoder::u32)?,
        ))
    }
}

impl WireDecode for StructuralDefinitionPath {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let segments = decoder.decode_array(|decoder, _| StructuralPathSegment::decode(decoder))?;
        Self::new(segments).map_err(|_| {
            wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected: 1,
                    actual: 0,
                },
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDeclarationName {
    Named(DecodedCanonicalIdentifier),
    Constructor,
}

impl DecodedDeclarationName {
    pub fn validate(self) -> Result<DeclarationName, CanonicalIdentifierError> {
        match self {
            Self::Named(name) => name.validate().map(DeclarationName::Named),
            Self::Constructor => Ok(DeclarationName::Constructor),
        }
    }
}

impl WireEncode for DecodedDeclarationName {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Named(name) => encode_sum(encoder, 1, name),
            Self::Constructor => encode_empty_sum(encoder, 2),
        }
    }
}

impl WireDecode for DecodedDeclarationName {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedCanonicalIdentifier::decode)
                    .map(Self::Named)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Constructor)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedDefinitionOwnerAtom {
    Type(DecodedPersistentId<PersistentTypeId>),
    GenericType(DecodedPersistentId<PersistentGenericTypeId>),
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
    ExtensionProperty(DecodedPersistentId<crate::PersistentExtensionPropertyId>),
    GeneratedCallable(DecodedPersistentId<PersistentGeneratedCallableId>),
    PropertyAccessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    EnumVariant(DecodedPersistentId<crate::PersistentEnumVariantId>),
}

impl DecodedDefinitionOwnerAtom {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefinitionOwnerAtom, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>
            + PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<crate::PersistentExtensionPropertyId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>,
    {
        match self {
            Self::Type(id) => resolver.resolve(id).map(DefinitionOwnerAtom::Type),
            Self::GenericType(id) => resolver.resolve(id).map(DefinitionOwnerAtom::GenericType),
            Self::Function(id) => resolver.resolve(id).map(DefinitionOwnerAtom::Function),
            Self::GenericFunction(id) => resolver
                .resolve(id)
                .map(DefinitionOwnerAtom::GenericFunction),
            Self::Constructor(id) => resolver.resolve(id).map(DefinitionOwnerAtom::Constructor),
            Self::Property(id) => resolver.resolve(id).map(DefinitionOwnerAtom::Property),
            Self::ExtensionProperty(id) => resolver
                .resolve(id)
                .map(DefinitionOwnerAtom::ExtensionProperty),
            Self::GeneratedCallable(id) => resolver
                .resolve(id)
                .map(DefinitionOwnerAtom::GeneratedCallable),
            Self::PropertyAccessor(id) => resolver
                .resolve(id)
                .map(DefinitionOwnerAtom::PropertyAccessor),
            Self::EnumVariant(id) => resolver.resolve(id).map(DefinitionOwnerAtom::EnumVariant),
        }
    }
}

impl WireEncode for DecodedDefinitionOwnerAtom {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
            Self::Type(id) => (1, id),
            Self::GenericType(id) => (2, id),
            Self::Function(id) => (3, id),
            Self::GenericFunction(id) => (4, id),
            Self::Constructor(id) => (5, id),
            Self::Property(id) => (6, id),
            Self::ExtensionProperty(id) => (7, id),
            Self::GeneratedCallable(id) => (8, id),
            Self::PropertyAccessor(id) => (9, id),
            Self::EnumVariant(id) => (10, id),
        };
        encode_sum(encoder, tag, id)
    }
}

impl WireDecode for DecodedDefinitionOwnerAtom {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Type),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericType),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            7 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ExtensionProperty),
            8 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GeneratedCallable),
            9 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::PropertyAccessor),
            10 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::EnumVariant),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DecodedDefinitionOwnerChain(Vec<DecodedDefinitionOwnerAtom>);

impl DecodedDefinitionOwnerChain {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefinitionOwnerChain, DefinitionOwnerResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>
            + PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<crate::PersistentExtensionPropertyId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>,
    {
        let mut owners = Vec::new();
        owners
            .try_reserve_exact(self.0.len())
            .map_err(|_| DefinitionOwnerResolutionError::Allocation)?;
        for owner in self.0 {
            owners.push(
                owner
                    .resolve(resolver)
                    .map_err(DefinitionOwnerResolutionError::Reference)?,
            );
        }
        Ok(DefinitionOwnerChain::from_outer_to_inner(owners))
    }
}

impl WireEncode for DecodedDefinitionOwnerChain {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for owner in &self.0 {
            owner.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedDefinitionOwnerChain {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedDefinitionOwnerAtom::decode(decoder))
            .map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionOwnerResolutionError<E> {
    Reference(E),
    Allocation,
}

impl<E: fmt::Display> fmt::Display for DefinitionOwnerResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Allocation => formatter.write_str("failed to allocate definition owner chain"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefinitionOwnerResolutionError<E> {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDeclarationScope {
    ConeWide,
    SourceScoped(DecodedSourceIdentity),
    LexicalScoped {
        source: DecodedSourceIdentity,
        path: StructuralDefinitionPath,
    },
}

impl DecodedDeclarationScope {
    pub fn resolve<R>(
        self,
        resolver: &mut R,
    ) -> Result<DeclarationScope, SourceIdentityResolutionError<R::Error>>
    where
        R: PersistentIdResolver<ConeIdentity>,
    {
        match self {
            Self::ConeWide => Ok(DeclarationScope::ConeWide),
            Self::SourceScoped(source) => {
                source.resolve(resolver).map(DeclarationScope::SourceScoped)
            }
            Self::LexicalScoped { source, path } => Ok(DeclarationScope::LexicalScoped {
                source: source.resolve(resolver)?,
                path,
            }),
        }
    }
}

impl WireEncode for DecodedDeclarationScope {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ConeWide => encode_empty_sum(encoder, 1),
            Self::SourceScoped(source) => encode_sum(encoder, 2, source),
            Self::LexicalScoped { source, path } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                path.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDeclarationScope {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::ConeWide)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedSourceIdentity::decode)
                    .map(Self::SourceScoped)
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::LexicalScoped {
                    source: decoder.field(1, DecodedSourceIdentity::decode)?,
                    path: decoder.field(2, StructuralDefinitionPath::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_sum<T: WireEncode + ?Sized>(
    encoder: &mut Encoder,
    tag: u64,
    value: &T,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests {
    use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

    use super::{DecodedDeclarationName, DecodedDeclarationScope, DecodedDefinitionOwnerChain};
    use crate::{
        ConeIdentity, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain,
        PersistentConstructorId, PersistentEnumVariantId, PersistentExtensionPropertyId,
        PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
        PersistentGenericTypeId, PersistentIdMismatch, PersistentIdResolver,
        PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, SourceIdentity,
        StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ResolutionError;

    struct Resolver;

    macro_rules! resolve_same_bytes {
        ($id:ty) => {
            impl PersistentIdResolver<$id> for Resolver {
                type Error = ResolutionError;

                fn resolve(
                    &mut self,
                    id: crate::DecodedPersistentId<$id>,
                ) -> Result<$id, Self::Error> {
                    id.verify(<$id>::from_test_bytes())
                        .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
                }
            }
        };
    }

    trait TestId {
        fn from_test_bytes() -> Self;
    }

    macro_rules! test_id {
        ($id:ty) => {
            impl TestId for $id {
                fn from_test_bytes() -> Self {
                    Self([7; 32])
                }
            }
            resolve_same_bytes!($id);
        };
    }

    impl PersistentIdResolver<ConeIdentity> for Resolver {
        type Error = ResolutionError;

        fn resolve(
            &mut self,
            id: crate::DecodedPersistentId<ConeIdentity>,
        ) -> Result<ConeIdentity, Self::Error> {
            id.verify(ConeIdentity::SINGLE_FILE)
                .map_err(|_: PersistentIdMismatch<ConeIdentity>| ResolutionError)
        }
    }

    test_id!(PersistentTypeId);
    test_id!(PersistentGenericTypeId);
    test_id!(PersistentFunctionId);
    test_id!(PersistentGenericFunctionId);
    test_id!(PersistentConstructorId);
    test_id!(PersistentPropertyId);
    test_id!(PersistentExtensionPropertyId);
    test_id!(PersistentGeneratedCallableId);
    test_id!(PersistentPropertyAccessorId);
    test_id!(PersistentEnumVariantId);

    #[test]
    fn owner_chain_round_trips_all_typed_variants() {
        let owners = DefinitionOwnerChain::from_outer_to_inner(vec![
            DefinitionOwnerAtom::Type(PersistentTypeId::from_test_bytes()),
            DefinitionOwnerAtom::GenericType(PersistentGenericTypeId::from_test_bytes()),
            DefinitionOwnerAtom::Function(PersistentFunctionId::from_test_bytes()),
            DefinitionOwnerAtom::GenericFunction(PersistentGenericFunctionId::from_test_bytes()),
            DefinitionOwnerAtom::Constructor(PersistentConstructorId::from_test_bytes()),
            DefinitionOwnerAtom::Property(PersistentPropertyId::from_test_bytes()),
            DefinitionOwnerAtom::ExtensionProperty(PersistentExtensionPropertyId::from_test_bytes()),
            DefinitionOwnerAtom::GeneratedCallable(PersistentGeneratedCallableId::from_test_bytes()),
            DefinitionOwnerAtom::PropertyAccessor(PersistentPropertyAccessorId::from_test_bytes()),
            DefinitionOwnerAtom::EnumVariant(PersistentEnumVariantId::from_test_bytes()),
        ]);
        let bytes = encode(&owners).unwrap();
        let decoded =
            decode_canonical::<DecodedDefinitionOwnerChain>(&bytes, DecodeLimits::default())
                .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), owners);
    }

    #[test]
    fn lexical_scope_round_trips_with_a_validated_structural_path() {
        let scope = DeclarationScope::LexicalScoped {
            source: SourceIdentity::single_file(),
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 2),
                [StructuralPathSegment::new(
                    StructuralDefinitionSiteRole::LocalDeclaration,
                    1,
                )],
            ),
        };
        let bytes = encode(&scope).unwrap();
        let decoded =
            decode_canonical::<DecodedDeclarationScope>(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), scope);
    }

    #[test]
    fn structural_decoder_rejects_empty_paths_unknown_roles_and_bad_names() {
        let empty_path =
            decode_canonical::<StructuralDefinitionPath>(b"\x80", DecodeLimits::default())
                .unwrap_err();
        assert_eq!(
            empty_path.kind(),
            &WireErrorKind::InvalidLength {
                expected: 1,
                actual: 0,
            }
        );

        let unknown_role = decode_canonical::<StructuralPathSegment>(
            b"\xa2\x01\x0d\x02\x00",
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(unknown_role.kind(), &WireErrorKind::UnknownTag { tag: 13 });

        let bad_name = decode_canonical::<DecodedDeclarationName>(
            b"\xa2\x00\x01\x01\x69not-valid",
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(bad_name.validate().is_err());
    }
}
