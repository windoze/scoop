use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CallableOwner, DispatchDeclarationOwner, NominalDeclarationOwner, NominalOwner, PropertyOwner,
};
use crate::{
    DecodedPersistentId, PersistentCallableApplicationId, PersistentConstructorId,
    PersistentExactTypeId, PersistentExtensionPropertyId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentIdResolver, PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedPropertyOwner {
    Property(DecodedPersistentId<PersistentPropertyId>),
    ExtensionProperty(DecodedPersistentId<PersistentExtensionPropertyId>),
}

impl DecodedPropertyOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<PropertyOwner, E>
    where
        R: PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>,
    {
        match self {
            Self::Property(id) => resolver.resolve(id).map(PropertyOwner::Property),
            Self::ExtensionProperty(id) => {
                resolver.resolve(id).map(PropertyOwner::ExtensionProperty)
            }
        }
    }
}

impl WireEncode for DecodedPropertyOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Property(id) => encode_id_sum(encoder, 1, id),
            Self::ExtensionProperty(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedPropertyOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let tag = decode_sum_tag(decoder)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ExtensionProperty),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedNominalDeclarationOwner {
    Concrete(DecodedPersistentId<PersistentTypeId>),
    GenericTemplate(DecodedPersistentId<PersistentGenericTypeId>),
}

impl DecodedNominalDeclarationOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<NominalDeclarationOwner, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>,
    {
        match self {
            Self::Concrete(id) => resolver.resolve(id).map(NominalDeclarationOwner::Concrete),
            Self::GenericTemplate(id) => resolver
                .resolve(id)
                .map(NominalDeclarationOwner::GenericTemplate),
        }
    }
}

impl WireEncode for DecodedNominalDeclarationOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Concrete(id) => encode_id_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedNominalDeclarationOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let tag = decode_sum_tag(decoder)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Concrete),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericTemplate),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedNominalOwner {
    Declaration(DecodedNominalDeclarationOwner),
    ExactApplication(DecodedPersistentId<PersistentExactTypeId>),
}

impl DecodedNominalOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<NominalOwner, E>
    where
        R: PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>
            + PersistentIdResolver<PersistentExactTypeId, Error = E>,
    {
        match self {
            Self::Declaration(owner) => owner.resolve(resolver).map(NominalOwner::Declaration),
            Self::ExactApplication(id) => resolver.resolve(id).map(NominalOwner::ExactApplication),
        }
    }
}

impl WireEncode for DecodedNominalOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Declaration(owner) => encode_id_sum(encoder, 1, owner),
            Self::ExactApplication(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedNominalOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let tag = decode_sum_tag(decoder)?;
        match tag {
            1 => decoder
                .field(1, DecodedNominalDeclarationOwner::decode)
                .map(Self::Declaration),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::ExactApplication),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedCallableOwner {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericTemplate(DecodedPersistentId<PersistentGenericFunctionId>),
    Application(DecodedPersistentId<PersistentCallableApplicationId>),
    Constructor(DecodedPersistentId<PersistentConstructorId>),
    Accessor(DecodedPersistentId<PersistentPropertyAccessorId>),
    Generated(DecodedPersistentId<PersistentGeneratedCallableId>),
}

impl DecodedCallableOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CallableOwner, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentCallableApplicationId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>,
    {
        match self {
            Self::Function(id) => resolver.resolve(id).map(CallableOwner::Function),
            Self::GenericTemplate(id) => resolver.resolve(id).map(CallableOwner::GenericTemplate),
            Self::Application(id) => resolver.resolve(id).map(CallableOwner::Application),
            Self::Constructor(id) => resolver.resolve(id).map(CallableOwner::Constructor),
            Self::Accessor(id) => resolver.resolve(id).map(CallableOwner::Accessor),
            Self::Generated(id) => resolver.resolve(id).map(CallableOwner::Generated),
        }
    }
}

impl WireEncode for DecodedCallableOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_id_sum(encoder, 1, id),
            Self::GenericTemplate(id) => encode_id_sum(encoder, 2, id),
            Self::Application(id) => encode_id_sum(encoder, 3, id),
            Self::Constructor(id) => encode_id_sum(encoder, 4, id),
            Self::Accessor(id) => encode_id_sum(encoder, 5, id),
            Self::Generated(id) => encode_id_sum(encoder, 6, id),
        }
    }
}

impl WireDecode for DecodedCallableOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let tag = decode_sum_tag(decoder)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericTemplate),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Application),
            4 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Constructor),
            5 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Accessor),
            6 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Generated),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedDispatchDeclarationOwner {
    Function(DecodedPersistentId<PersistentFunctionId>),
    Accessor(DecodedPersistentId<PersistentPropertyAccessorId>),
}

impl DecodedDispatchDeclarationOwner {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DispatchDeclarationOwner, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        match self {
            Self::Function(id) => resolver.resolve(id).map(DispatchDeclarationOwner::Function),
            Self::Accessor(id) => resolver.resolve(id).map(DispatchDeclarationOwner::Accessor),
        }
    }
}

impl WireEncode for DecodedDispatchDeclarationOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => encode_id_sum(encoder, 1, id),
            Self::Accessor(id) => encode_id_sum(encoder, 2, id),
        }
    }
}

impl WireDecode for DecodedDispatchDeclarationOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let tag = decode_sum_tag(decoder)?;
        match tag {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Accessor),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

fn decode_sum_tag(decoder: &mut Decoder<'_, '_>) -> Result<u64, WireError> {
    decoder.expect_map(2)?;
    decoder.field(0, Decoder::unsigned)
}

fn encode_id_sum(
    encoder: &mut Encoder,
    tag: u64,
    id: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    id.encode(encoder)
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

#[cfg(test)]
mod tests {
    use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

    use super::{
        DecodedCallableOwner, DecodedDispatchDeclarationOwner, DecodedNominalDeclarationOwner,
        DecodedNominalOwner, DecodedPropertyOwner,
    };
    use crate::{
        CallableOwner, DispatchDeclarationOwner, NominalDeclarationOwner, NominalOwner,
        PersistentCallableApplicationId, PersistentConstructorId, PersistentExactTypeId,
        PersistentExtensionPropertyId, PersistentFunctionId, PersistentGeneratedCallableId,
        PersistentGenericFunctionId, PersistentGenericTypeId, PersistentIdMismatch,
        PersistentIdResolver, PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId,
        PropertyOwner,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ResolutionError;

    struct Resolver;

    trait TestId {
        fn expected() -> Self;
    }

    macro_rules! test_identity {
        ($id:ty) => {
            impl TestId for $id {
                fn expected() -> Self {
                    Self([7; 32])
                }
            }

            impl PersistentIdResolver<$id> for Resolver {
                type Error = ResolutionError;

                fn resolve(
                    &mut self,
                    id: crate::DecodedPersistentId<$id>,
                ) -> Result<$id, Self::Error> {
                    id.verify(<$id>::expected())
                        .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
                }
            }
        };
    }

    test_identity!(PersistentPropertyId);
    test_identity!(PersistentExtensionPropertyId);
    test_identity!(PersistentTypeId);
    test_identity!(PersistentGenericTypeId);
    test_identity!(PersistentExactTypeId);
    test_identity!(PersistentFunctionId);
    test_identity!(PersistentGenericFunctionId);
    test_identity!(PersistentCallableApplicationId);
    test_identity!(PersistentConstructorId);
    test_identity!(PersistentPropertyAccessorId);
    test_identity!(PersistentGeneratedCallableId);

    #[test]
    fn every_owner_family_round_trips_and_resolves_its_typed_variants() {
        for owner in [
            PropertyOwner::Property(PersistentPropertyId::expected()),
            PropertyOwner::ExtensionProperty(PersistentExtensionPropertyId::expected()),
        ] {
            let decoded = decode_canonical::<DecodedPropertyOwner>(
                &encode(&owner).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            assert_eq!(decoded.resolve(&mut Resolver).unwrap(), owner);
        }

        for owner in [
            NominalDeclarationOwner::Concrete(PersistentTypeId::expected()),
            NominalDeclarationOwner::GenericTemplate(PersistentGenericTypeId::expected()),
        ] {
            let decoded = decode_canonical::<DecodedNominalDeclarationOwner>(
                &encode(&owner).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            assert_eq!(decoded.resolve(&mut Resolver).unwrap(), owner);
        }

        for owner in [
            NominalOwner::Declaration(NominalDeclarationOwner::Concrete(
                PersistentTypeId::expected(),
            )),
            NominalOwner::ExactApplication(PersistentExactTypeId::expected()),
        ] {
            let decoded = decode_canonical::<DecodedNominalOwner>(
                &encode(&owner).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            assert_eq!(decoded.resolve(&mut Resolver).unwrap(), owner);
        }

        for owner in [
            CallableOwner::Function(PersistentFunctionId::expected()),
            CallableOwner::GenericTemplate(PersistentGenericFunctionId::expected()),
            CallableOwner::Application(PersistentCallableApplicationId::expected()),
            CallableOwner::Constructor(PersistentConstructorId::expected()),
            CallableOwner::Accessor(PersistentPropertyAccessorId::expected()),
            CallableOwner::Generated(PersistentGeneratedCallableId::expected()),
        ] {
            let decoded = decode_canonical::<DecodedCallableOwner>(
                &encode(&owner).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            assert_eq!(decoded.resolve(&mut Resolver).unwrap(), owner);
        }

        for owner in [
            DispatchDeclarationOwner::Function(PersistentFunctionId::expected()),
            DispatchDeclarationOwner::Accessor(PersistentPropertyAccessorId::expected()),
        ] {
            let decoded = decode_canonical::<DecodedDispatchDeclarationOwner>(
                &encode(&owner).unwrap(),
                DecodeLimits::default(),
            )
            .unwrap();
            assert_eq!(decoded.resolve(&mut Resolver).unwrap(), owner);
        }
    }

    #[test]
    fn owner_decoders_reject_unknown_tags_without_kind_fallback() {
        let mut bytes = vec![0xa2, 0x00, 0x03, 0x01, 0x58, 0x20];
        bytes.extend_from_slice(&[0; 32]);
        let error =
            decode_canonical::<DecodedPropertyOwner>(&bytes, DecodeLimits::default()).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
    }
}
