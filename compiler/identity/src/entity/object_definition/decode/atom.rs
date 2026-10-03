use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{DefinitionAtomResolver, ObjectDefinitionResolutionError};
use crate::{
    ConeImageSupportRole, DecodedPersistentId, DefinitionAtomRole, DefinitionAtomSubkey,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentSafepointSiteId, PersistentStaticStorageId, StructuralDefinitionPath,
};

impl WireDecode for ConeImageSupportRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::CoordinateGroup),
            2 => Ok(Self::CoordinateName),
            3 => Ok(Self::CoordinateVersion),
            4 => Ok(Self::Dependencies),
            5 => Ok(Self::StaticStorages),
            6 => Ok(Self::ImmortalObjects),
            7 => Ok(Self::InitializationUnits),
            8 => Ok(Self::TypeRegistrations),
            9 => Ok(Self::Safepoints),
            10 => Ok(Self::Callables),
            11 => Ok(Self::ArrayBoundsMessage),
            12 => Ok(Self::ArraySizeOverflowMessage),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for DefinitionAtomRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Primary),
            2 => Ok(Self::Lsda),
            3 => Ok(Self::EhFrame),
            4 => Ok(Self::CompactUnwind),
            5 => Ok(Self::Stackmap),
            6 => Ok(Self::RuntimeRecord),
            7 => Ok(Self::AddressTakenConstant),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefinitionAtomSubkey {
    Singleton,
    CallableBody(DecodedPersistentId<PersistentCallableBodyId>),
    StaticStorage(DecodedPersistentId<PersistentStaticStorageId>),
    ImmortalObject(DecodedPersistentId<PersistentImmortalObjectId>),
    InitializationUnit(DecodedPersistentId<PersistentInitializationUnitId>),
    ExactType(DecodedPersistentId<PersistentExactTypeId>),
    SafepointSite(DecodedPersistentId<PersistentSafepointSiteId>),
    StructuralPath(StructuralDefinitionPath),
    ConeImageSupport(ConeImageSupportRole),
}

impl DecodedDefinitionAtomSubkey {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<DefinitionAtomSubkey, E>
    where
        R: DefinitionAtomResolver<E>,
    {
        match self {
            Self::Singleton => Ok(DefinitionAtomSubkey::Singleton),
            Self::CallableBody(id) => resolver.resolve(id).map(DefinitionAtomSubkey::CallableBody),
            Self::StaticStorage(id) => resolver
                .resolve(id)
                .map(DefinitionAtomSubkey::StaticStorage),
            Self::ImmortalObject(id) => resolver
                .resolve(id)
                .map(DefinitionAtomSubkey::ImmortalObject),
            Self::InitializationUnit(id) => resolver
                .resolve(id)
                .map(DefinitionAtomSubkey::InitializationUnit),
            Self::ExactType(id) => resolver.resolve(id).map(DefinitionAtomSubkey::ExactType),
            Self::SafepointSite(id) => resolver
                .resolve(id)
                .map(DefinitionAtomSubkey::SafepointSite),
            Self::StructuralPath(path) => Ok(DefinitionAtomSubkey::StructuralPath(path)),
            Self::ConeImageSupport(role) => Ok(DefinitionAtomSubkey::ConeImageSupport(role)),
        }
    }
}

impl WireEncode for DecodedDefinitionAtomSubkey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Singleton => encode_empty_sum(encoder, 1),
            Self::CallableBody(id) => encode_value_sum(encoder, 2, id),
            Self::StaticStorage(id) => encode_value_sum(encoder, 3, id),
            Self::ImmortalObject(id) => encode_value_sum(encoder, 4, id),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 5, id),
            Self::ExactType(id) => encode_value_sum(encoder, 6, id),
            Self::SafepointSite(id) => encode_value_sum(encoder, 7, id),
            Self::StructuralPath(path) => encode_value_sum(encoder, 8, path),
            Self::ConeImageSupport(role) => encode_value_sum(encoder, 9, role),
        }
    }
}

impl WireDecode for DecodedDefinitionAtomSubkey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Singleton)
            }
            2 => decode_id_variant(decoder, fields, Self::CallableBody),
            3 => decode_id_variant(decoder, fields, Self::StaticStorage),
            4 => decode_id_variant(decoder, fields, Self::ImmortalObject),
            5 => decode_id_variant(decoder, fields, Self::InitializationUnit),
            6 => decode_id_variant(decoder, fields, Self::ExactType),
            7 => decode_id_variant(decoder, fields, Self::SafepointSite),
            8 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, StructuralDefinitionPath::decode)
                    .map(Self::StructuralPath)
            }
            9 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, ConeImageSupportRole::decode)
                    .map(Self::ConeImageSupport)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedObjectDefinitionAtomKey {
    plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    role: DefinitionAtomRole,
    subkey: DecodedDefinitionAtomSubkey,
}

impl DecodedObjectDefinitionAtomKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ObjectDefinitionAtomKey, ObjectDefinitionResolutionError<E>>
    where
        R: DefinitionAtomResolver<E>,
    {
        let plan = resolver
            .resolve(self.plan)
            .map_err(ObjectDefinitionResolutionError::Reference)?;
        let subkey = self
            .subkey
            .resolve(resolver)
            .map_err(ObjectDefinitionResolutionError::Reference)?;
        Ok(ObjectDefinitionAtomKey::new(plan, self.role, subkey))
    }
}

impl WireEncode for DecodedObjectDefinitionAtomKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.plan.encode(encoder)?;
        encoder.field(2)?;
        self.role.encode(encoder)?;
        encoder.field(3)?;
        self.subkey.encode(encoder)
    }
}

impl WireDecode for DecodedObjectDefinitionAtomKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            plan: decoder.field(1, DecodedPersistentId::decode)?,
            role: decoder.field(2, DefinitionAtomRole::decode)?,
            subkey: decoder.field(3, DecodedDefinitionAtomSubkey::decode)?,
        })
    }
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}
