use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{ObjectDefinitionResolutionError, StrongDefinitionResolver};
use crate::{
    ConeIdentity, DecodedPersistentId, GeneratedBridgeAtomId, ObjectDefinitionIdentityError,
    ObjectDefinitionPlanKey, ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole, OdrMemberId,
    PersistentCallableBodyId, PersistentDispatchSlotId, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentImmortalObjectId, PersistentInitializationUnitId,
    PersistentLayoutId, PersistentSafepointSiteId, PersistentScanId, PersistentStaticStorageId,
    StrongDefinitionEntity, StrongDefinitionRole,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedStrongDefinitionEntity {
    CallableBody(DecodedPersistentId<PersistentCallableBodyId>),
    StaticStorage(DecodedPersistentId<PersistentStaticStorageId>),
    ImmortalObject(DecodedPersistentId<PersistentImmortalObjectId>),
    ExactType(DecodedPersistentId<PersistentExactTypeId>),
    Layout(DecodedPersistentId<PersistentLayoutId>),
    Scan(DecodedPersistentId<PersistentScanId>),
    DispatchTable(DecodedPersistentId<PersistentDispatchTableId>),
    DispatchSlot(DecodedPersistentId<PersistentDispatchSlotId>),
    InitializationUnit(DecodedPersistentId<PersistentInitializationUnitId>),
    SafepointSite(DecodedPersistentId<PersistentSafepointSiteId>),
    ConeImage(DecodedPersistentId<ConeIdentity>),
    GeneratedBridgeAtom(DecodedPersistentId<GeneratedBridgeAtomId>),
    RootEntry(DecodedPersistentId<ConeIdentity>),
}

impl DecodedStrongDefinitionEntity {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<StrongDefinitionEntity, ObjectDefinitionResolutionError<E>>
    where
        R: StrongDefinitionResolver<E>,
    {
        match self {
            Self::CallableBody(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::callable_body)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::StaticStorage(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::static_storage)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::ImmortalObject(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::immortal_object)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::ExactType(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::exact_type)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::Layout(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::layout)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::Scan(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::scan)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::DispatchTable(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::dispatch_table)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::DispatchSlot(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::dispatch_slot)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::InitializationUnit(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::initialization_unit)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::SafepointSite(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::safepoint_site)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::ConeImage(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::cone_image)
                .map_err(ObjectDefinitionResolutionError::Reference),
            Self::GeneratedBridgeAtom(id) => {
                let key = resolver
                    .resolve_key(id)
                    .map_err(ObjectDefinitionResolutionError::Reference)?;
                StrongDefinitionEntity::generated_bridge_atom(&key)
                    .map_err(ObjectDefinitionResolutionError::Definition)
            }
            Self::RootEntry(id) => resolver
                .resolve(id)
                .map(StrongDefinitionEntity::root_entry)
                .map_err(ObjectDefinitionResolutionError::Reference),
        }
    }
}

impl WireEncode for DecodedStrongDefinitionEntity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::CallableBody(id) => encode_value_sum(encoder, 1, id),
            Self::StaticStorage(id) => encode_value_sum(encoder, 2, id),
            Self::ImmortalObject(id) => encode_value_sum(encoder, 3, id),
            Self::ExactType(id) => encode_value_sum(encoder, 4, id),
            Self::Layout(id) => encode_value_sum(encoder, 5, id),
            Self::Scan(id) => encode_value_sum(encoder, 6, id),
            Self::DispatchTable(id) => encode_value_sum(encoder, 7, id),
            Self::DispatchSlot(id) => encode_value_sum(encoder, 8, id),
            Self::InitializationUnit(id) => encode_value_sum(encoder, 9, id),
            Self::SafepointSite(id) => encode_value_sum(encoder, 10, id),
            Self::ConeImage(id) => encode_value_sum(encoder, 11, id),
            Self::GeneratedBridgeAtom(id) => encode_value_sum(encoder, 12, id),
            Self::RootEntry(id) => encode_value_sum(encoder, 13, id),
        }
    }
}

impl WireDecode for DecodedStrongDefinitionEntity {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => decode_id_variant(decoder, fields, Self::CallableBody),
            2 => decode_id_variant(decoder, fields, Self::StaticStorage),
            3 => decode_id_variant(decoder, fields, Self::ImmortalObject),
            4 => decode_id_variant(decoder, fields, Self::ExactType),
            5 => decode_id_variant(decoder, fields, Self::Layout),
            6 => decode_id_variant(decoder, fields, Self::Scan),
            7 => decode_id_variant(decoder, fields, Self::DispatchTable),
            8 => decode_id_variant(decoder, fields, Self::DispatchSlot),
            9 => decode_id_variant(decoder, fields, Self::InitializationUnit),
            10 => decode_id_variant(decoder, fields, Self::SafepointSite),
            11 => decode_id_variant(decoder, fields, Self::ConeImage),
            12 => decode_id_variant(decoder, fields, Self::GeneratedBridgeAtom),
            13 => decode_id_variant(decoder, fields, Self::RootEntry),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedObjectDefinitionPlanOwner {
    Strong {
        producer: DecodedPersistentId<ConeIdentity>,
        entity: DecodedStrongDefinitionEntity,
    },
    Odr {
        member: DecodedPersistentId<OdrMemberId>,
    },
}

impl DecodedObjectDefinitionPlanOwner {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ObjectDefinitionPlanOwner, ObjectDefinitionResolutionError<E>>
    where
        R: StrongDefinitionResolver<E>,
    {
        match self {
            Self::Strong { producer, entity } => Ok(ObjectDefinitionPlanOwner::Strong {
                producer: resolver
                    .resolve(producer)
                    .map_err(ObjectDefinitionResolutionError::Reference)?,
                entity: entity.resolve(resolver)?,
            }),
            Self::Odr { member } => resolver
                .resolve(member)
                .map(|member| ObjectDefinitionPlanOwner::Odr { member })
                .map_err(ObjectDefinitionResolutionError::Reference),
        }
    }
}

impl WireEncode for DecodedObjectDefinitionPlanOwner {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Strong { producer, entity } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                producer.encode(encoder)?;
                encoder.field(2)?;
                entity.encode(encoder)
            }
            Self::Odr { member } => encode_value_sum(encoder, 2, member),
        }
    }
}

impl WireDecode for DecodedObjectDefinitionPlanOwner {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Strong {
                    producer: decoder.field(1, DecodedPersistentId::decode)?,
                    entity: decoder.field(2, DecodedStrongDefinitionEntity::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(|member| Self::Odr { member })
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for StrongDefinitionRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::CallableBody),
            2 => Ok(Self::StaticStorage),
            3 => Ok(Self::ImmortalObject),
            4 => Ok(Self::TypeDescriptor),
            5 => Ok(Self::Layout),
            6 => Ok(Self::ScanProgram),
            7 => Ok(Self::DispatchTable),
            8 => Ok(Self::DispatchSlot),
            9 => Ok(Self::InitializationCell),
            10 => Ok(Self::InitializationDescriptor),
            11 => Ok(Self::RootRegistration),
            12 => Ok(Self::ImmortalRegistration),
            13 => Ok(Self::InitializationRegistration),
            14 => Ok(Self::TypeRegistration),
            15 => Ok(Self::SafepointRegistration),
            16 => Ok(Self::CallableRegistration),
            17 => Ok(Self::ImageDescriptor),
            18 => Ok(Self::GeneratedBridge),
            19 => Ok(Self::RootEntryDescriptor),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for ObjectDefinitionPlanRole {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, StrongDefinitionRole::decode)
                    .map(Self::Strong)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::OdrMemberPrimary)
            }
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedObjectDefinitionPlanKey {
    owner: DecodedObjectDefinitionPlanOwner,
    definition_role: ObjectDefinitionPlanRole,
}

impl DecodedObjectDefinitionPlanKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ObjectDefinitionPlanKey, ObjectDefinitionResolutionError<E>>
    where
        R: StrongDefinitionResolver<E>,
    {
        let owner = self.owner.resolve(resolver)?;
        match (owner, self.definition_role) {
            (
                ObjectDefinitionPlanOwner::Strong { producer, entity },
                ObjectDefinitionPlanRole::Strong(role),
            ) => ObjectDefinitionPlanKey::strong(producer, entity, role)
                .map_err(ObjectDefinitionResolutionError::Definition),
            (
                ObjectDefinitionPlanOwner::Odr { member },
                ObjectDefinitionPlanRole::OdrMemberPrimary,
            ) => Ok(ObjectDefinitionPlanKey::odr(member)),
            _ => Err(ObjectDefinitionResolutionError::Definition(
                ObjectDefinitionIdentityError::PlanOwnerRoleMismatch,
            )),
        }
    }
}

impl WireEncode for DecodedObjectDefinitionPlanKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.definition_role.encode(encoder)
    }
}

impl WireDecode for DecodedObjectDefinitionPlanKey {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            owner: decoder.field(1, DecodedObjectDefinitionPlanOwner::decode)?,
            definition_role: decoder.field(2, ObjectDefinitionPlanRole::decode)?,
        })
    }
}

fn decode_sum_header(decoder: &mut Decoder<'_, '_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_, '_>,
    fields: u64,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
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

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}
