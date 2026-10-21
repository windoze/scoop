use std::fmt;

pub use scoop_identity::{
    ConeCoordinate, ConeImageSupportRole, DefinitionAtomSubkey, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey,
};
use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedPersistentSymbolRequest, DefinitionAtomRole,
    LinkageClass, ObjectDefinitionAtomId, PersistentCallableBodyId, PersistentExactTypeId,
    PersistentImmortalObjectId, PersistentInitializationUnitId, PersistentSafepointSiteId,
    PersistentStaticStorageId, PersistentSymbolError, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{
    Decoder, Encoder, HashError, RuntimeEncode, RuntimeEncodeError, RuntimeEncoder, WireDecode,
    WireEncode, WireError,
};

use crate::{ConeLirFoundation, DigestFinalizationPlanV1, RegistrationIdentitySurfaceV1};

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
    fn from_registrations(registrations: &RegistrationIdentitySurfaceV1) -> Self {
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
}

impl fmt::Display for ConeImagePlanBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Cone image plan: {self:?}")
    }
}

impl std::error::Error for ConeImagePlanBuildError {}

fn semantic_ids<I: scoop_identity::PersistentId>(
    entries: &[crate::RegistrationIdentityV1<I>],
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

mod atoms;
mod dependencies;
mod plan;
pub use atoms::ConeImageSupportAtomsV1;
use atoms::{DecodedConeImageSupportAtomsV1, require_image_atoms};
pub use plan::{ConeImagePlanV1, DecodedConeImagePlanV1};

#[cfg(test)]
mod tests;
