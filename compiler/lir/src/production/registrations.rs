use std::fmt;

use scoop_identity::{
    DecodedPersistentId, LinkageClass, ObjectDefinitionPlanId, ObjectDefinitionPlanOwner,
    OdrGroupId, OdrMemberId, PersistentCallableBodyId, PersistentExactTypeId, PersistentId,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentSafepointSiteId,
    PersistentStaticStorageId, PersistentSymbolKey, PersistentSymbolRequest,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::ConeLirFoundation;

/// Semantic ownership of a physical runtime registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationDefinitionOwner {
    Strong,
    Odr {
        group: OdrGroupId,
        member: OdrMemberId,
    },
}

impl RegistrationDefinitionOwner {
    pub const fn linkage(self) -> LinkageClass {
        match self {
            Self::Strong => LinkageClass::ConeStrong,
            Self::Odr { .. } => LinkageClass::OdrWeak,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistrationIdentityV1<I: PersistentId> {
    semantic_id: I,
    definition_plan: ObjectDefinitionPlanId,
    owner: RegistrationDefinitionOwner,
}

impl<I: PersistentId> RegistrationIdentityV1<I> {
    pub const fn owner(&self) -> RegistrationDefinitionOwner {
        self.owner
    }

    pub const fn semantic_id(&self) -> I {
        self.semantic_id
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }
}

impl<I: PersistentId + WireEncode> WireEncode for RegistrationIdentityV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedRegistrationIdentityV1<I: PersistentId> {
    semantic_id: DecodedPersistentId<I>,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
}

impl<I: PersistentId> WireEncode for DecodedRegistrationIdentityV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)
    }
}

impl<I: PersistentId> WireDecode for DecodedRegistrationIdentityV1<I> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            semantic_id: decoder.field(1, DecodedPersistentId::decode)?,
            definition_plan: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationIdentitySurfaceV1 {
    static_storages: Vec<RegistrationIdentityV1<PersistentStaticStorageId>>,
    immortal_objects: Vec<RegistrationIdentityV1<PersistentImmortalObjectId>>,
    initialization_units: Vec<RegistrationIdentityV1<PersistentInitializationUnitId>>,
    type_registrations: Vec<RegistrationIdentityV1<PersistentExactTypeId>>,
    safepoints: Vec<RegistrationIdentityV1<PersistentSafepointSiteId>>,
    callables: Vec<RegistrationIdentityV1<PersistentCallableBodyId>>,
}

impl RegistrationIdentitySurfaceV1 {
    pub fn from_foundation(
        foundation: &ConeLirFoundation,
    ) -> Result<Self, RegistrationIdentityBuildError> {
        let mut surface = Self::empty();
        for record in foundation.definition_plans() {
            let (entity, role) = foundation.definition_subject(record).ok_or(
                RegistrationIdentityBuildError::UnknownDefinition(record.id()),
            )?;
            let Some(table) = registration_table(role) else {
                continue;
            };
            let owner = match record.key().owner() {
                ObjectDefinitionPlanOwner::Strong { .. } => RegistrationDefinitionOwner::Strong,
                ObjectDefinitionPlanOwner::Odr { member } => {
                    let key = foundation
                        .odr_member(member)
                        .ok_or(RegistrationIdentityBuildError::UnknownDefinition(
                            record.id(),
                        ))?
                        .key();
                    RegistrationDefinitionOwner::Odr {
                        group: key.group(),
                        member,
                    }
                }
            };
            let plan = record.id();
            match (table, entity.kind()) {
                (
                    RegistrationTableV1::StaticStorage,
                    StrongDefinitionEntityKind::StaticStorage(id),
                ) => {
                    surface.static_storages.push(entry(id, plan, owner));
                }
                (
                    RegistrationTableV1::ImmortalObject,
                    StrongDefinitionEntityKind::ImmortalObject(id),
                ) => {
                    surface.immortal_objects.push(entry(id, plan, owner));
                }
                (
                    RegistrationTableV1::InitializationUnit,
                    StrongDefinitionEntityKind::InitializationUnit(id),
                ) => surface.initialization_units.push(entry(id, plan, owner)),
                (RegistrationTableV1::Type, StrongDefinitionEntityKind::ExactType(id)) => {
                    surface.type_registrations.push(entry(id, plan, owner));
                }
                (RegistrationTableV1::Safepoint, StrongDefinitionEntityKind::SafepointSite(id)) => {
                    surface.safepoints.push(entry(id, plan, owner));
                }
                (RegistrationTableV1::Callable, StrongDefinitionEntityKind::CallableBody(id)) => {
                    surface.callables.push(entry(id, plan, owner));
                }
                _ => {
                    return Err(RegistrationIdentityBuildError::RoleEntityMismatch {
                        plan,
                        role,
                        entity: entity.kind(),
                    });
                }
            }
        }
        surface.sort_and_validate()?;
        Ok(surface)
    }

    fn empty() -> Self {
        Self {
            static_storages: Vec::new(),
            immortal_objects: Vec::new(),
            initialization_units: Vec::new(),
            type_registrations: Vec::new(),
            safepoints: Vec::new(),
            callables: Vec::new(),
        }
    }

    fn sort_and_validate(&mut self) -> Result<(), RegistrationIdentityBuildError> {
        sort_unique(
            RegistrationTableV1::StaticStorage,
            &mut self.static_storages,
        )?;
        sort_unique(
            RegistrationTableV1::ImmortalObject,
            &mut self.immortal_objects,
        )?;
        sort_unique(
            RegistrationTableV1::InitializationUnit,
            &mut self.initialization_units,
        )?;
        sort_unique(RegistrationTableV1::Type, &mut self.type_registrations)?;
        sort_unique(RegistrationTableV1::Safepoint, &mut self.safepoints)?;
        sort_unique(RegistrationTableV1::Callable, &mut self.callables)
    }

    pub fn static_storages(&self) -> &[RegistrationIdentityV1<PersistentStaticStorageId>] {
        &self.static_storages
    }

    pub fn immortal_objects(&self) -> &[RegistrationIdentityV1<PersistentImmortalObjectId>] {
        &self.immortal_objects
    }

    pub fn initialization_units(
        &self,
    ) -> &[RegistrationIdentityV1<PersistentInitializationUnitId>] {
        &self.initialization_units
    }

    pub fn type_registrations(&self) -> &[RegistrationIdentityV1<PersistentExactTypeId>] {
        &self.type_registrations
    }

    pub fn safepoints(&self) -> &[RegistrationIdentityV1<PersistentSafepointSiteId>] {
        &self.safepoints
    }

    pub fn callables(&self) -> &[RegistrationIdentityV1<PersistentCallableBodyId>] {
        &self.callables
    }
    pub fn static_storage_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.static_storages.iter().map(|entry| {
            registration_symbol(
                PersistentSymbolKey::RootRegistration(entry.semantic_id()),
                entry.owner(),
            )
        })
    }

    pub fn immortal_object_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.immortal_objects.iter().map(|entry| {
            registration_symbol(
                PersistentSymbolKey::ImmortalRegistration(entry.semantic_id()),
                entry.owner(),
            )
        })
    }

    pub fn initialization_unit_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.initialization_units.iter().map(|entry| {
            registration_symbol(
                PersistentSymbolKey::InitializationRegistration(entry.semantic_id()),
                entry.owner(),
            )
        })
    }

    pub fn type_registration_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.type_registrations.iter().map(|entry| {
            registration_symbol(
                PersistentSymbolKey::TypeRegistration(entry.semantic_id()),
                entry.owner(),
            )
        })
    }

    pub fn safepoint_symbol_requests(&self) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.safepoints.iter().map(|entry| {
            registration_symbol(
                PersistentSymbolKey::SafepointRegistration(entry.semantic_id()),
                entry.owner(),
            )
        })
    }

    pub fn callable_symbol_requests(&self) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.callables.iter().map(|entry| {
            registration_symbol(
                PersistentSymbolKey::CallableRegistration(entry.semantic_id()),
                entry.owner(),
            )
        })
    }
}

impl WireEncode for RegistrationIdentitySurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        encode_array(encoder, &self.static_storages)?;
        encoder.field(2)?;
        encode_array(encoder, &self.immortal_objects)?;
        encoder.field(3)?;
        encode_array(encoder, &self.initialization_units)?;
        encoder.field(4)?;
        encode_array(encoder, &self.type_registrations)?;
        encoder.field(5)?;
        encode_array(encoder, &self.safepoints)?;
        encoder.field(6)?;
        encode_array(encoder, &self.callables)
    }
}

#[derive(Debug)]
pub struct DecodedRegistrationIdentitySurfaceV1 {
    static_storages: Vec<DecodedRegistrationIdentityV1<PersistentStaticStorageId>>,
    immortal_objects: Vec<DecodedRegistrationIdentityV1<PersistentImmortalObjectId>>,
    initialization_units: Vec<DecodedRegistrationIdentityV1<PersistentInitializationUnitId>>,
    type_registrations: Vec<DecodedRegistrationIdentityV1<PersistentExactTypeId>>,
    safepoints: Vec<DecodedRegistrationIdentityV1<PersistentSafepointSiteId>>,
    callables: Vec<DecodedRegistrationIdentityV1<PersistentCallableBodyId>>,
}

impl DecodedRegistrationIdentitySurfaceV1 {
    pub fn validate(
        self,
        foundation: &ConeLirFoundation,
    ) -> Result<RegistrationIdentitySurfaceV1, RegistrationIdentityValidationError> {
        let expected = RegistrationIdentitySurfaceV1::from_foundation(foundation)
            .map_err(RegistrationIdentityValidationError::Foundation)?;
        validate_table(
            RegistrationTableV1::StaticStorage,
            self.static_storages,
            &expected.static_storages,
        )?;
        validate_table(
            RegistrationTableV1::ImmortalObject,
            self.immortal_objects,
            &expected.immortal_objects,
        )?;
        validate_table(
            RegistrationTableV1::InitializationUnit,
            self.initialization_units,
            &expected.initialization_units,
        )?;
        validate_table(
            RegistrationTableV1::Type,
            self.type_registrations,
            &expected.type_registrations,
        )?;
        validate_table(
            RegistrationTableV1::Safepoint,
            self.safepoints,
            &expected.safepoints,
        )?;
        validate_table(
            RegistrationTableV1::Callable,
            self.callables,
            &expected.callables,
        )?;
        Ok(expected)
    }
}

impl WireEncode for DecodedRegistrationIdentitySurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        encode_array(encoder, &self.static_storages)?;
        encoder.field(2)?;
        encode_array(encoder, &self.immortal_objects)?;
        encoder.field(3)?;
        encode_array(encoder, &self.initialization_units)?;
        encoder.field(4)?;
        encode_array(encoder, &self.type_registrations)?;
        encoder.field(5)?;
        encode_array(encoder, &self.safepoints)?;
        encoder.field(6)?;
        encode_array(encoder, &self.callables)
    }
}

impl WireDecode for DecodedRegistrationIdentitySurfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            static_storages: decode_table(decoder, 1)?,
            immortal_objects: decode_table(decoder, 2)?,
            initialization_units: decode_table(decoder, 3)?,
            type_registrations: decode_table(decoder, 4)?,
            safepoints: decode_table(decoder, 5)?,
            callables: decode_table(decoder, 6)?,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RegistrationTableV1 {
    StaticStorage,
    ImmortalObject,
    InitializationUnit,
    Type,
    Safepoint,
    Callable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationIdentityBuildError {
    UnknownDefinition(ObjectDefinitionPlanId),
    RoleEntityMismatch {
        plan: ObjectDefinitionPlanId,
        role: StrongDefinitionRole,
        entity: StrongDefinitionEntityKind,
    },
    DuplicateSemanticId {
        table: RegistrationTableV1,
        bytes: [u8; 32],
    },
}

impl fmt::Display for RegistrationIdentityBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong registration identity surface: {self:?}"
        )
    }
}

impl std::error::Error for RegistrationIdentityBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationIdentityValidationError {
    Foundation(RegistrationIdentityBuildError),
    TableLength {
        table: RegistrationTableV1,
        expected: usize,
        actual: usize,
    },
    EntryMismatch {
        table: RegistrationTableV1,
        index: usize,
    },
}

impl fmt::Display for RegistrationIdentityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong registration identity surface: {self:?}"
        )
    }
}

impl std::error::Error for RegistrationIdentityValidationError {}

fn registration_table(role: StrongDefinitionRole) -> Option<RegistrationTableV1> {
    match role {
        StrongDefinitionRole::RootRegistration => Some(RegistrationTableV1::StaticStorage),
        StrongDefinitionRole::ImmortalRegistration => Some(RegistrationTableV1::ImmortalObject),
        StrongDefinitionRole::InitializationRegistration => {
            Some(RegistrationTableV1::InitializationUnit)
        }
        StrongDefinitionRole::TypeRegistration => Some(RegistrationTableV1::Type),
        StrongDefinitionRole::SafepointRegistration => Some(RegistrationTableV1::Safepoint),
        StrongDefinitionRole::CallableRegistration => Some(RegistrationTableV1::Callable),
        _ => None,
    }
}

fn registration_symbol(
    key: PersistentSymbolKey,
    owner: RegistrationDefinitionOwner,
) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, owner.linkage())
        .expect("registration symbol kinds accept their definition linkage")
}

fn entry<I: PersistentId>(
    semantic_id: I,
    definition_plan: ObjectDefinitionPlanId,
    owner: RegistrationDefinitionOwner,
) -> RegistrationIdentityV1<I> {
    RegistrationIdentityV1 {
        semantic_id,
        definition_plan,
        owner,
    }
}

fn sort_unique<I: PersistentId>(
    table: RegistrationTableV1,
    entries: &mut [RegistrationIdentityV1<I>],
) -> Result<(), RegistrationIdentityBuildError> {
    entries.sort_unstable_by_key(RegistrationIdentityV1::semantic_id);
    for pair in entries.windows(2) {
        if pair[0].semantic_id == pair[1].semantic_id {
            return Err(RegistrationIdentityBuildError::DuplicateSemanticId {
                table,
                bytes: *pair[0].semantic_id.as_array(),
            });
        }
    }
    Ok(())
}

fn validate_table<I: PersistentId>(
    table: RegistrationTableV1,
    decoded: Vec<DecodedRegistrationIdentityV1<I>>,
    expected: &[RegistrationIdentityV1<I>],
) -> Result<(), RegistrationIdentityValidationError> {
    if decoded.len() != expected.len() {
        return Err(RegistrationIdentityValidationError::TableLength {
            table,
            expected: expected.len(),
            actual: decoded.len(),
        });
    }
    for (index, (decoded, expected)) in decoded.into_iter().zip(expected).enumerate() {
        if decoded.semantic_id.verify(expected.semantic_id).is_err()
            || decoded
                .definition_plan
                .verify(expected.definition_plan)
                .is_err()
        {
            return Err(RegistrationIdentityValidationError::EntryMismatch { table, index });
        }
    }
    Ok(())
}

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn decode_table<I: PersistentId>(
    decoder: &mut Decoder<'_>,
    field: u32,
) -> Result<Vec<DecodedRegistrationIdentityV1<I>>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| DecodedRegistrationIdentityV1::decode(decoder))
    })
}

#[cfg(test)]
mod tests;
