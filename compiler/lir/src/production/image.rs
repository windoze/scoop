use std::fmt;

pub use scoop_identity::{
    ConeCoordinate, ConeImageSupportRole, DefinitionAtomSubkey, DigestPatchIntentId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey,
};
use scoop_identity::{
    ConeCoordinateError, ConeIdentity, DecodedConeCoordinate, DecodedPersistentId,
    DecodedPersistentSymbolRequest, DefinitionAtomRole, DigestKind, DigestNodeId,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId,
    PersistentCallableBodyId, PersistentExactTypeId, PersistentIdMismatch,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentSafepointSiteId,
    PersistentStaticStorageId, PersistentSymbolError, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{
    Decoder, Encoder, HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireDecode,
    WireEncode, WireError, encode,
};

use crate::{
    ConeLirFoundation, DigestInputRefV1, StrongDigestFinalizationPlanV1,
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

impl RuntimeEncode for ConeRecordV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.byte_span(self.coordinate.group().as_bytes())?;
        encoder.byte_span(self.coordinate.name().as_bytes())?;
        encoder.byte_span(self.coordinate.version().as_bytes())?;
        encoder.fixed(self.identity.as_array())
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
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
pub enum ConeImagePlanBuildError {
    SelfDependency(ConeIdentity),
    DuplicateDependency(ConeIdentity),
    Hash(HashError),
    Symbol(PersistentSymbolError),
    Definition(scoop_identity::ObjectDefinitionIdentityError),
    ProducerMismatch {
        coordinate: ConeIdentity,
        foundation: ConeIdentity,
    },
    MissingSymbol(PersistentSymbolRequest),
    MissingDefinition(ObjectDefinitionPlanId),
    AtomSet {
        expected: Vec<ObjectDefinitionAtomKey>,
        actual: Vec<ObjectDefinitionAtomKey>,
    },
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
    if inputs
        .iter()
        .any(|input| input.kind() != DigestKind::StrongRegistration)
    {
        return Err(ConeImagePlanBuildError::RegistrationInputs {
            expected,
            actual: inputs.iter().map(|input| input.node()).collect(),
        });
    }
    let mut actual = inputs.iter().map(|input| input.node()).collect::<Vec<_>>();
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

#[allow(clippy::too_many_arguments)]
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
    decoder: &mut Decoder<'_>,
    field: u32,
) -> Result<Vec<DecodedPersistentId<I>>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
    })
}

mod atoms;
mod dependencies;
mod plan;
pub use atoms::ConeImageSupportAtomsV1;
use atoms::{DecodedConeImageSupportAtomsV1, require_image_atoms};
pub use plan::{ConeImagePlanV1, DecodedConeImagePlanV1};

#[cfg(test)]
mod tests;
