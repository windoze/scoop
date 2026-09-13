use std::fmt;

use scoop_identity::{
    ConeCoordinate, ConeCoordinateError, ConeIdentity, DecodedConeCoordinate, DecodedPersistentId,
    DecodedPersistentSymbolRequest, DefinitionAtomRole, DigestKind, DigestNodeId,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionPlanKey,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentIdMismatch,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentSafepointSiteId,
    PersistentStaticStorageId, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};
pub use scoop_identity::{DigestPatchIntentId, ObjectDefinitionPlanId};
use scoop_wire::{Decoder, Encoder, HashError, WireDecode, WireEncode, WireError, encode};

use crate::{
    DigestInputRefV1, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongRegistrationIdentitySurfaceV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeRecordV1 {
    coordinate: ConeCoordinate,
    identity: ConeIdentity,
}

impl ConeRecordV1 {
    pub fn new(coordinate: ConeCoordinate) -> Result<Self, HashError> {
        let identity = coordinate.identity()?;
        Ok(Self {
            coordinate,
            identity,
        })
    }

    pub const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.identity
    }
}

impl WireEncode for ConeRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.coordinate.encode(encoder)?;
        encoder.field(2)?;
        self.identity.encode(encoder)
    }
}

#[derive(Debug)]
struct DecodedConeRecordV1 {
    coordinate: DecodedConeCoordinate,
    identity: DecodedPersistentId<ConeIdentity>,
}

impl WireEncode for DecodedConeRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.coordinate.encode(encoder)?;
        encoder.field(2)?;
        self.identity.encode(encoder)
    }
}

impl WireDecode for DecodedConeRecordV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            coordinate: decoder.field(1, DecodedConeCoordinate::decode)?,
            identity: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeRegistrationTablesV1 {
    static_storages: Vec<PersistentStaticStorageId>,
    immortal_objects: Vec<PersistentImmortalObjectId>,
    initialization_units: Vec<PersistentInitializationUnitId>,
    type_registrations: Vec<PersistentExactTypeId>,
    safepoints: Vec<PersistentSafepointSiteId>,
    callables: Vec<PersistentCallableBodyId>,
}

impl ConeRegistrationTablesV1 {
    fn from_registrations(registrations: &StrongRegistrationIdentitySurfaceV1) -> Self {
        Self {
            static_storages: semantic_ids(registrations.static_storages()),
            immortal_objects: semantic_ids(registrations.immortal_objects()),
            initialization_units: semantic_ids(registrations.initialization_units()),
            type_registrations: semantic_ids(registrations.type_registrations()),
            safepoints: semantic_ids(registrations.safepoints()),
            callables: semantic_ids(registrations.callables()),
        }
    }

    pub fn static_storages(&self) -> &[PersistentStaticStorageId] {
        &self.static_storages
    }

    pub fn immortal_objects(&self) -> &[PersistentImmortalObjectId] {
        &self.immortal_objects
    }

    pub fn initialization_units(&self) -> &[PersistentInitializationUnitId] {
        &self.initialization_units
    }

    pub fn type_registrations(&self) -> &[PersistentExactTypeId] {
        &self.type_registrations
    }

    pub fn safepoints(&self) -> &[PersistentSafepointSiteId] {
        &self.safepoints
    }

    pub fn callables(&self) -> &[PersistentCallableBodyId] {
        &self.callables
    }

    pub fn static_storage_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.static_storages
            .iter()
            .copied()
            .map(PersistentSymbolKey::RootRegistration)
            .map(strong_symbol_request)
    }

    pub fn immortal_object_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.immortal_objects
            .iter()
            .copied()
            .map(PersistentSymbolKey::ImmortalRegistration)
            .map(strong_symbol_request)
    }

    pub fn initialization_unit_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.initialization_units
            .iter()
            .copied()
            .map(PersistentSymbolKey::InitializationRegistration)
            .map(strong_symbol_request)
    }

    pub fn type_registration_symbol_requests(
        &self,
    ) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.type_registrations
            .iter()
            .copied()
            .map(PersistentSymbolKey::TypeRegistration)
            .map(strong_symbol_request)
    }

    pub fn safepoint_symbol_requests(&self) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.safepoints
            .iter()
            .copied()
            .map(PersistentSymbolKey::SafepointRegistration)
            .map(strong_symbol_request)
    }

    pub fn callable_symbol_requests(&self) -> impl Iterator<Item = PersistentSymbolRequest> + '_ {
        self.callables
            .iter()
            .copied()
            .map(PersistentSymbolKey::CallableRegistration)
            .map(strong_symbol_request)
    }
}

fn strong_symbol_request(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, LinkageClass::ConeStrong)
        .expect("registration symbol kinds accept strong Cone linkage")
}

impl WireEncode for ConeRegistrationTablesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_registration_tables(
            encoder,
            &self.static_storages,
            &self.immortal_objects,
            &self.initialization_units,
            &self.type_registrations,
            &self.safepoints,
            &self.callables,
        )
    }
}

#[derive(Debug)]
struct DecodedConeRegistrationTablesV1 {
    static_storages: Vec<DecodedPersistentId<PersistentStaticStorageId>>,
    immortal_objects: Vec<DecodedPersistentId<PersistentImmortalObjectId>>,
    initialization_units: Vec<DecodedPersistentId<PersistentInitializationUnitId>>,
    type_registrations: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    safepoints: Vec<DecodedPersistentId<PersistentSafepointSiteId>>,
    callables: Vec<DecodedPersistentId<PersistentCallableBodyId>>,
}

impl WireEncode for DecodedConeRegistrationTablesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_registration_tables(
            encoder,
            &self.static_storages,
            &self.immortal_objects,
            &self.initialization_units,
            &self.type_registrations,
            &self.safepoints,
            &self.callables,
        )
    }
}

impl WireDecode for DecodedConeRegistrationTablesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            static_storages: decode_ids(decoder, 1)?,
            immortal_objects: decode_ids(decoder, 2)?,
            initialization_units: decode_ids(decoder, 3)?,
            type_registrations: decode_ids(decoder, 4)?,
            safepoints: decode_ids(decoder, 5)?,
            callables: decode_ids(decoder, 6)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeImagePlanV1 {
    cone: ConeRecordV1,
    dependencies: Vec<ConeIdentity>,
    tables: ConeRegistrationTablesV1,
    symbol: PersistentSymbolRequest,
    definition_plan: ObjectDefinitionPlanId,
    fingerprint_patch: DigestPatchIntentId,
}

impl ConeImagePlanV1 {
    pub fn new(
        coordinate: ConeCoordinate,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        digest_plan: &StrongDigestFinalizationPlanV1,
    ) -> Result<Self, ConeImagePlanBuildError> {
        let cone = ConeRecordV1::new(coordinate).map_err(ConeImagePlanBuildError::Hash)?;
        if cone.identity() != foundation.producer() {
            return Err(ConeImagePlanBuildError::ProducerMismatch {
                coordinate: cone.identity(),
                foundation: foundation.producer(),
            });
        }
        let dependencies = if cone.identity() == ConeIdentity::CORE {
            Vec::new()
        } else {
            vec![ConeIdentity::CORE]
        };
        let tables = ConeRegistrationTablesV1::from_registrations(registrations);
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::ImageDescriptor(cone.identity()),
            LinkageClass::ConeStrong,
        )
        .map_err(ConeImagePlanBuildError::Symbol)?;
        if !foundation.contains_symbol_request(symbol) {
            return Err(ConeImagePlanBuildError::MissingSymbol(symbol));
        }
        let definition_key = ObjectDefinitionPlanKey::strong(
            cone.identity(),
            StrongDefinitionEntity::cone_image(cone.identity()),
            StrongDefinitionRole::ImageDescriptor,
        )
        .map_err(ConeImagePlanBuildError::Definition)?;
        let definition_plan = ObjectDefinitionPlanId::from_key(&definition_key)
            .map_err(ConeImagePlanBuildError::Hash)?;
        if !foundation
            .definition_plans()
            .iter()
            .any(|record| record.id() == definition_plan && record.key() == &definition_key)
        {
            return Err(ConeImagePlanBuildError::MissingDefinition(definition_plan));
        }

        let image_node = digest_plan
            .nodes()
            .iter()
            .find(|node| {
                node.key().owner_and_role()
                    == scoop_identity::DigestOwnerAndRoleKey::RuntimeImage(cone.identity())
            })
            .ok_or(ConeImagePlanBuildError::MissingDigestNode)?;
        validate_registration_inputs(image_node.direct_inputs(), registrations)?;
        let patch_key = DigestPatchIntentKey::new(
            image_node.id(),
            definition_plan,
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RuntimeImage,
        );
        let fingerprint_patch =
            DigestPatchIntentId::from_key(&patch_key).map_err(ConeImagePlanBuildError::Hash)?;
        if !image_node
            .patch_intents()
            .iter()
            .any(|record| record.id() == fingerprint_patch && record.key() == &patch_key)
        {
            return Err(ConeImagePlanBuildError::MissingFingerprintPatch(
                fingerprint_patch,
            ));
        }

        Ok(Self {
            cone,
            dependencies,
            tables,
            symbol,
            definition_plan,
            fingerprint_patch,
        })
    }

    pub const fn cone(&self) -> &ConeRecordV1 {
        &self.cone
    }

    pub fn dependencies(&self) -> &[ConeIdentity] {
        &self.dependencies
    }

    pub const fn tables(&self) -> &ConeRegistrationTablesV1 {
        &self.tables
    }

    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn fingerprint_patch(&self) -> DigestPatchIntentId {
        self.fingerprint_patch
    }
}

impl WireEncode for ConeImagePlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.cone.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.dependencies)?;
        encoder.field(3)?;
        self.tables.encode(encoder)?;
        encoder.field(4)?;
        self.symbol.encode(encoder)?;
        encoder.field(5)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(6)?;
        self.fingerprint_patch.encode(encoder)
    }
}

#[derive(Debug)]
pub struct DecodedConeImagePlanV1 {
    cone: DecodedConeRecordV1,
    dependencies: Vec<DecodedPersistentId<ConeIdentity>>,
    tables: DecodedConeRegistrationTablesV1,
    symbol: DecodedPersistentSymbolRequest,
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    fingerprint_patch: DecodedPersistentId<DigestPatchIntentId>,
}

impl DecodedConeImagePlanV1 {
    pub fn validate(
        self,
        coordinate: &ConeCoordinate,
        foundation: &OdrFreeLirFoundation,
        registrations: &StrongRegistrationIdentitySurfaceV1,
        digest_plan: &StrongDigestFinalizationPlanV1,
    ) -> Result<ConeImagePlanV1, ConeImagePlanValidationError> {
        let actual = encode(&self).map_err(ConeImagePlanValidationError::Encode)?;
        let decoded_coordinate = self
            .cone
            .coordinate
            .validate()
            .map_err(ConeImagePlanValidationError::Coordinate)?;
        if &decoded_coordinate != coordinate {
            return Err(ConeImagePlanValidationError::CoordinateMismatch);
        }
        let expected_identity = coordinate
            .identity()
            .map_err(ConeImagePlanValidationError::Hash)?;
        self.cone
            .identity
            .verify(expected_identity)
            .map_err(ConeImagePlanValidationError::Identity)?;
        let expected =
            ConeImagePlanV1::new(coordinate.clone(), foundation, registrations, digest_plan)
                .map_err(ConeImagePlanValidationError::Expected)?;
        let expected_bytes = encode(&expected).map_err(ConeImagePlanValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(ConeImagePlanValidationError::PlanMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedConeImagePlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.cone.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.dependencies)?;
        encoder.field(3)?;
        self.tables.encode(encoder)?;
        encoder.field(4)?;
        self.symbol.encode(encoder)?;
        encoder.field(5)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(6)?;
        self.fingerprint_patch.encode(encoder)
    }
}

impl WireDecode for DecodedConeImagePlanV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            cone: decoder.field(1, DecodedConeRecordV1::decode)?,
            dependencies: decode_ids(decoder, 2)?,
            tables: decoder.field(3, DecodedConeRegistrationTablesV1::decode)?,
            symbol: decoder.field(4, DecodedPersistentSymbolRequest::decode)?,
            definition_plan: decoder.field(5, DecodedPersistentId::decode)?,
            fingerprint_patch: decoder.field(6, DecodedPersistentId::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConeImagePlanBuildError {
    Hash(HashError),
    Symbol(PersistentSymbolError),
    Definition(scoop_identity::ObjectDefinitionIdentityError),
    ProducerMismatch {
        coordinate: ConeIdentity,
        foundation: ConeIdentity,
    },
    MissingSymbol(PersistentSymbolRequest),
    MissingDefinition(ObjectDefinitionPlanId),
    MissingDigestNode,
    RegistrationInputs {
        expected: Vec<DigestNodeId>,
        actual: Vec<DigestNodeId>,
    },
    MissingFingerprintPatch(DigestPatchIntentId),
}

impl fmt::Display for ConeImagePlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Cone image plan: {self:?}")
    }
}

impl std::error::Error for ConeImagePlanBuildError {}

#[derive(Debug)]
pub enum ConeImagePlanValidationError {
    Coordinate(ConeCoordinateError),
    CoordinateMismatch,
    Identity(PersistentIdMismatch<ConeIdentity>),
    Hash(HashError),
    Expected(ConeImagePlanBuildError),
    Encode(scoop_wire::cbor::EncodeError),
    PlanMismatch,
}

impl fmt::Display for ConeImagePlanValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid decoded Cone image plan: {self:?}")
    }
}

impl std::error::Error for ConeImagePlanValidationError {}

fn validate_registration_inputs(
    inputs: &[DigestInputRefV1],
    registrations: &StrongRegistrationIdentitySurfaceV1,
) -> Result<(), ConeImagePlanBuildError> {
    let mut expected = registrations
        .static_storages()
        .iter()
        .map(|entry| entry.fingerprint_node())
        .chain(
            registrations
                .immortal_objects()
                .iter()
                .map(|entry| entry.fingerprint_node()),
        )
        .chain(
            registrations
                .initialization_units()
                .iter()
                .map(|entry| entry.fingerprint_node()),
        )
        .chain(
            registrations
                .type_registrations()
                .iter()
                .map(|entry| entry.fingerprint_node()),
        )
        .chain(
            registrations
                .safepoints()
                .iter()
                .map(|entry| entry.fingerprint_node()),
        )
        .chain(
            registrations
                .callables()
                .iter()
                .map(|entry| entry.fingerprint_node()),
        )
        .collect::<Vec<_>>();
    expected.sort_unstable();
    let mut actual = inputs
        .iter()
        .filter(|input| input.kind() == DigestKind::StrongRegistration)
        .map(|input| input.node())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    if actual == expected {
        Ok(())
    } else {
        Err(ConeImagePlanBuildError::RegistrationInputs { expected, actual })
    }
}

fn semantic_ids<I: scoop_identity::PersistentId>(
    entries: &[crate::StrongRegistrationIdentityV1<I>],
) -> Vec<I> {
    entries.iter().map(|entry| entry.semantic_id()).collect()
}

fn encode_registration_tables<A, B, C, D, E, F>(
    encoder: &mut Encoder,
    static_storages: &[A],
    immortal_objects: &[B],
    initialization_units: &[C],
    type_registrations: &[D],
    safepoints: &[E],
    callables: &[F],
) -> Result<(), scoop_wire::cbor::EncodeError>
where
    A: WireEncode,
    B: WireEncode,
    C: WireEncode,
    D: WireEncode,
    E: WireEncode,
    F: WireEncode,
{
    encoder.map(6)?;
    encoder.field(1)?;
    encode_array(encoder, static_storages)?;
    encoder.field(2)?;
    encode_array(encoder, immortal_objects)?;
    encoder.field(3)?;
    encode_array(encoder, initialization_units)?;
    encoder.field(4)?;
    encode_array(encoder, type_registrations)?;
    encoder.field(5)?;
    encode_array(encoder, safepoints)?;
    encoder.field(6)?;
    encode_array(encoder, callables)
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

fn decode_ids<I: scoop_identity::PersistentId>(
    decoder: &mut Decoder<'_, '_>,
    field: u32,
) -> Result<Vec<DecodedPersistentId<I>>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
    })
}

#[cfg(test)]
mod tests;
