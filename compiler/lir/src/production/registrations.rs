use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DigestNodeId, DigestOwnerAndRoleKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentSafepointSiteId, PersistentStaticStorageId,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{OdrFreeLirFoundation, StrongDigestFinalizationPlanV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongRegistrationIdentityV1<I: PersistentId> {
    semantic_id: I,
    definition_plan: ObjectDefinitionPlanId,
    fingerprint_node: DigestNodeId,
}

impl<I: PersistentId> StrongRegistrationIdentityV1<I> {
    pub const fn semantic_id(&self) -> I {
        self.semantic_id
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn fingerprint_node(&self) -> DigestNodeId {
        self.fingerprint_node
    }
}

impl<I: PersistentId + WireEncode> WireEncode for StrongRegistrationIdentityV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(3)?;
        self.fingerprint_node.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedStrongRegistrationIdentityV1<I: PersistentId> {
    semantic_id: DecodedPersistentId<I>,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    fingerprint_node: DecodedPersistentId<DigestNodeId>,
}

impl<I: PersistentId> WireEncode for DecodedStrongRegistrationIdentityV1<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.semantic_id.encode(encoder)?;
        encoder.field(2)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(3)?;
        self.fingerprint_node.encode(encoder)
    }
}

impl<I: PersistentId> WireDecode for DecodedStrongRegistrationIdentityV1<I> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            semantic_id: decoder.field(1, DecodedPersistentId::decode)?,
            definition_plan: decoder.field(2, DecodedPersistentId::decode)?,
            fingerprint_node: decoder.field(3, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongRegistrationIdentitySurfaceV1 {
    static_storages: Vec<StrongRegistrationIdentityV1<PersistentStaticStorageId>>,
    immortal_objects: Vec<StrongRegistrationIdentityV1<PersistentImmortalObjectId>>,
    initialization_units: Vec<StrongRegistrationIdentityV1<PersistentInitializationUnitId>>,
    type_registrations: Vec<StrongRegistrationIdentityV1<PersistentExactTypeId>>,
    safepoints: Vec<StrongRegistrationIdentityV1<PersistentSafepointSiteId>>,
    callables: Vec<StrongRegistrationIdentityV1<PersistentCallableBodyId>>,
}

impl StrongRegistrationIdentitySurfaceV1 {
    pub fn from_foundation(
        foundation: &OdrFreeLirFoundation,
        digest_plan: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, StrongRegistrationIdentityBuildError> {
        let mut surface = Self::empty();
        for record in foundation.definition_plans() {
            let ObjectDefinitionPlanOwner::Strong { entity, .. } = record.key().owner() else {
                return Err(StrongRegistrationIdentityBuildError::OdrPlan(record.id()));
            };
            let ObjectDefinitionPlanRole::Strong(role) = record.key().definition_role() else {
                return Err(StrongRegistrationIdentityBuildError::OdrPlan(record.id()));
            };
            let Some(table) = registration_table(role) else {
                continue;
            };
            let fingerprint_node = digest_plan
                .nodes()
                .iter()
                .find(|node| {
                    node.key().owner_and_role()
                        == DigestOwnerAndRoleKey::StrongRegistration(record.id())
                })
                .map(|node| node.id())
                .ok_or(StrongRegistrationIdentityBuildError::MissingFingerprintNode(record.id()))?;
            let plan = record.id();
            match (table, entity.kind()) {
                (
                    RegistrationTableV1::StaticStorage,
                    StrongDefinitionEntityKind::StaticStorage(id),
                ) => {
                    surface
                        .static_storages
                        .push(entry(id, plan, fingerprint_node));
                }
                (
                    RegistrationTableV1::ImmortalObject,
                    StrongDefinitionEntityKind::ImmortalObject(id),
                ) => {
                    surface
                        .immortal_objects
                        .push(entry(id, plan, fingerprint_node));
                }
                (
                    RegistrationTableV1::InitializationUnit,
                    StrongDefinitionEntityKind::InitializationUnit(id),
                ) => surface
                    .initialization_units
                    .push(entry(id, plan, fingerprint_node)),
                (RegistrationTableV1::Type, StrongDefinitionEntityKind::ExactType(id)) => {
                    surface
                        .type_registrations
                        .push(entry(id, plan, fingerprint_node));
                }
                (RegistrationTableV1::Safepoint, StrongDefinitionEntityKind::SafepointSite(id)) => {
                    surface.safepoints.push(entry(id, plan, fingerprint_node));
                }
                (RegistrationTableV1::Callable, StrongDefinitionEntityKind::CallableBody(id)) => {
                    surface.callables.push(entry(id, plan, fingerprint_node));
                }
                _ => {
                    return Err(StrongRegistrationIdentityBuildError::RoleEntityMismatch {
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

    fn sort_and_validate(&mut self) -> Result<(), StrongRegistrationIdentityBuildError> {
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

    pub fn static_storages(&self) -> &[StrongRegistrationIdentityV1<PersistentStaticStorageId>] {
        &self.static_storages
    }

    pub fn immortal_objects(&self) -> &[StrongRegistrationIdentityV1<PersistentImmortalObjectId>] {
        &self.immortal_objects
    }

    pub fn initialization_units(
        &self,
    ) -> &[StrongRegistrationIdentityV1<PersistentInitializationUnitId>] {
        &self.initialization_units
    }

    pub fn type_registrations(&self) -> &[StrongRegistrationIdentityV1<PersistentExactTypeId>] {
        &self.type_registrations
    }

    pub fn safepoints(&self) -> &[StrongRegistrationIdentityV1<PersistentSafepointSiteId>] {
        &self.safepoints
    }

    pub fn callables(&self) -> &[StrongRegistrationIdentityV1<PersistentCallableBodyId>] {
        &self.callables
    }
}

impl WireEncode for StrongRegistrationIdentitySurfaceV1 {
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
pub struct DecodedStrongRegistrationIdentitySurfaceV1 {
    static_storages: Vec<DecodedStrongRegistrationIdentityV1<PersistentStaticStorageId>>,
    immortal_objects: Vec<DecodedStrongRegistrationIdentityV1<PersistentImmortalObjectId>>,
    initialization_units: Vec<DecodedStrongRegistrationIdentityV1<PersistentInitializationUnitId>>,
    type_registrations: Vec<DecodedStrongRegistrationIdentityV1<PersistentExactTypeId>>,
    safepoints: Vec<DecodedStrongRegistrationIdentityV1<PersistentSafepointSiteId>>,
    callables: Vec<DecodedStrongRegistrationIdentityV1<PersistentCallableBodyId>>,
}

impl DecodedStrongRegistrationIdentitySurfaceV1 {
    pub fn validate(
        self,
        foundation: &OdrFreeLirFoundation,
        digest_plan: &StrongDigestFinalizationPlanV1,
    ) -> Result<StrongRegistrationIdentitySurfaceV1, StrongRegistrationIdentityValidationError>
    {
        let expected =
            StrongRegistrationIdentitySurfaceV1::from_foundation(foundation, digest_plan)
                .map_err(StrongRegistrationIdentityValidationError::Foundation)?;
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

impl WireEncode for DecodedStrongRegistrationIdentitySurfaceV1 {
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

impl WireDecode for DecodedStrongRegistrationIdentitySurfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
pub enum StrongRegistrationIdentityBuildError {
    OdrPlan(ObjectDefinitionPlanId),
    MissingFingerprintNode(ObjectDefinitionPlanId),
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

impl fmt::Display for StrongRegistrationIdentityBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong registration identity surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationIdentityBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongRegistrationIdentityValidationError {
    Foundation(StrongRegistrationIdentityBuildError),
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

impl fmt::Display for StrongRegistrationIdentityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong registration identity surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationIdentityValidationError {}

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

fn entry<I: PersistentId>(
    semantic_id: I,
    definition_plan: ObjectDefinitionPlanId,
    fingerprint_node: DigestNodeId,
) -> StrongRegistrationIdentityV1<I> {
    StrongRegistrationIdentityV1 {
        semantic_id,
        definition_plan,
        fingerprint_node,
    }
}

fn sort_unique<I: PersistentId>(
    table: RegistrationTableV1,
    entries: &mut [StrongRegistrationIdentityV1<I>],
) -> Result<(), StrongRegistrationIdentityBuildError> {
    entries.sort_unstable_by_key(StrongRegistrationIdentityV1::semantic_id);
    for pair in entries.windows(2) {
        if pair[0].semantic_id == pair[1].semantic_id {
            return Err(StrongRegistrationIdentityBuildError::DuplicateSemanticId {
                table,
                bytes: *pair[0].semantic_id.as_array(),
            });
        }
    }
    Ok(())
}

fn validate_table<I: PersistentId>(
    table: RegistrationTableV1,
    decoded: Vec<DecodedStrongRegistrationIdentityV1<I>>,
    expected: &[StrongRegistrationIdentityV1<I>],
) -> Result<(), StrongRegistrationIdentityValidationError> {
    if decoded.len() != expected.len() {
        return Err(StrongRegistrationIdentityValidationError::TableLength {
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
            || decoded
                .fingerprint_node
                .verify(expected.fingerprint_node)
                .is_err()
        {
            return Err(StrongRegistrationIdentityValidationError::EntryMismatch { table, index });
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
    decoder: &mut Decoder<'_, '_>,
    field: u32,
) -> Result<Vec<DecodedStrongRegistrationIdentityV1<I>>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| DecodedStrongRegistrationIdentityV1::decode(decoder))
    })
}

#[cfg(test)]
mod tests;
