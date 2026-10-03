//! Complete expected persistent symbols for every strong definition plan.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DecodedPersistentSymbolRequest, DecodedStrongDefinitionEntity,
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionPlanId, ObjectDefinitionPlanOwner,
    PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
};
pub use scoop_identity::{
    DefinitionAtomRole, PersistentDispatchTableId, PersistentLayoutId, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use crate::{ConeLirFoundation, ObjectDefinitionPlanBuildError, ObjectDefinitionPlanSurfaceV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomBoundarySymbolsV1 {
    atom: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    start: PersistentSymbolRequest,
    end: PersistentSymbolRequest,
}

impl AtomBoundarySymbolsV1 {
    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn atom_role(self) -> DefinitionAtomRole {
        self.atom_role
    }

    pub const fn start(self) -> PersistentSymbolRequest {
        self.start
    }

    pub const fn end(self) -> PersistentSymbolRequest {
        self.end
    }
}

impl WireEncode for AtomBoundarySymbolsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.atom.encode(encoder)?;
        encoder.field(2)?;
        self.atom_role.encode(encoder)?;
        encoder.field(3)?;
        self.start.encode(encoder)?;
        encoder.field(4)?;
        self.end.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionSymbolPlanV1 {
    definition_plan: ObjectDefinitionPlanId,
    definition_owner: ObjectDefinitionPlanOwner,
    owner: StrongDefinitionEntity,
    definition_role: StrongDefinitionRole,
    primary_atom: ObjectDefinitionAtomId,
    primary_symbol: PersistentSymbolRequest,
    atom_boundaries: Vec<AtomBoundarySymbolsV1>,
}

impl DefinitionSymbolPlanV1 {
    pub const fn definition_owner(&self) -> ObjectDefinitionPlanOwner {
        self.definition_owner
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn owner(&self) -> StrongDefinitionEntity {
        self.owner
    }

    pub const fn definition_role(&self) -> StrongDefinitionRole {
        self.definition_role
    }

    pub const fn primary_atom(&self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn primary_symbol(&self) -> PersistentSymbolRequest {
        self.primary_symbol
    }

    pub fn atom_boundaries(&self) -> &[AtomBoundarySymbolsV1] {
        &self.atom_boundaries
    }
}

impl WireEncode for DefinitionSymbolPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        self.definition_role.encode(encoder)?;
        encoder.field(4)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(5)?;
        self.primary_symbol.encode(encoder)?;
        encoder.field(6)?;
        encode_array(encoder, &self.atom_boundaries)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectSymbolSurfaceV1 {
    plans: Vec<DefinitionSymbolPlanV1>,
}

impl ObjectSymbolSurfaceV1 {
    pub fn from_foundation(
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ObjectSymbolSurfaceBuildError> {
        let definitions = ObjectDefinitionPlanSurfaceV1::from_foundation(foundation)
            .map_err(ObjectSymbolSurfaceBuildError::DefinitionSurface)?;
        Self::from_definition_plans(foundation, &definitions)
    }

    pub(crate) fn from_definition_plans(
        foundation: &ConeLirFoundation,
        definitions: &ObjectDefinitionPlanSurfaceV1,
    ) -> Result<Self, ObjectSymbolSurfaceBuildError> {
        let mut plans = Vec::with_capacity(definitions.plans().len());
        let mut symbol_owners = BTreeMap::new();
        for definition in definitions.plans() {
            let definition_plan = definition.plan();
            let key = foundation
                .definition_plan(definition_plan)
                .ok_or(ObjectSymbolSurfaceBuildError::MissingDefinitionKey(
                    definition_plan,
                ))?
                .key();
            let record = foundation.definition_plan(definition_plan).ok_or(
                ObjectSymbolSurfaceBuildError::MissingDefinitionKey(definition_plan),
            )?;
            let (entity, definition_role) = foundation.definition_subject(record).ok_or(
                ObjectSymbolSurfaceBuildError::UnknownDefinitionSubject(definition_plan),
            )?;
            let linkage = match key.owner() {
                ObjectDefinitionPlanOwner::Strong { .. } => LinkageClass::ConeStrong,
                ObjectDefinitionPlanOwner::Odr { .. } => LinkageClass::OdrWeak,
            };
            let primary_symbol = PersistentSymbolRequest::new(
                entity.primary_symbol_key(definition_role).ok_or(
                    ObjectSymbolSurfaceBuildError::UnknownDefinitionSubject(definition_plan),
                )?,
                linkage,
            )
            .map_err(ObjectSymbolSurfaceBuildError::Symbol)?;
            if let Some(first) = symbol_owners.insert(primary_symbol.key(), definition_plan) {
                return Err(ObjectSymbolSurfaceBuildError::DuplicatePrimarySymbol {
                    symbol: primary_symbol.key(),
                    first,
                    second: definition_plan,
                });
            }
            if !foundation.contains_symbol_request(primary_symbol) {
                return Err(ObjectSymbolSurfaceBuildError::MissingPrimarySymbolRequest {
                    definition_plan,
                    symbol: primary_symbol.key(),
                });
            }

            let mut atoms = Vec::with_capacity(1 + definition.associated_atoms().len());
            atoms.push(definition.primary_atom());
            atoms.extend_from_slice(definition.associated_atoms());
            atoms.sort_unstable();
            let atom_boundaries = atoms
                .into_iter()
                .map(|atom| {
                    let role = foundation
                        .definition_atom(atom)
                        .ok_or(ObjectSymbolSurfaceBuildError::MissingDefinitionAtom(atom))?
                        .key()
                        .role();
                    boundary_symbols(atom, role, linkage)
                        .map_err(ObjectSymbolSurfaceBuildError::Symbol)
                })
                .collect::<Result<Vec<_>, _>>()?;
            plans.push(DefinitionSymbolPlanV1 {
                definition_plan,
                definition_owner: key.owner(),
                owner: entity,
                definition_role,
                primary_atom: definition.primary_atom(),
                primary_symbol,
                atom_boundaries,
            });
        }
        Ok(Self { plans })
    }

    pub fn plans(&self) -> &[DefinitionSymbolPlanV1] {
        &self.plans
    }

    pub fn plan(&self, definition_plan: ObjectDefinitionPlanId) -> Option<&DefinitionSymbolPlanV1> {
        self.plans
            .binary_search_by_key(&definition_plan, |plan| plan.definition_plan)
            .ok()
            .map(|index| &self.plans[index])
    }
}

impl WireEncode for ObjectSymbolSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

#[derive(Debug)]
struct DecodedAtomBoundarySymbolsV1 {
    atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    atom_role: DefinitionAtomRole,
    start: DecodedPersistentSymbolRequest,
    end: DecodedPersistentSymbolRequest,
}

impl WireEncode for DecodedAtomBoundarySymbolsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.atom.encode(encoder)?;
        encoder.field(2)?;
        self.atom_role.encode(encoder)?;
        encoder.field(3)?;
        self.start.encode(encoder)?;
        encoder.field(4)?;
        self.end.encode(encoder)
    }
}

impl WireDecode for DecodedAtomBoundarySymbolsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            atom: decoder.field(1, DecodedPersistentId::decode)?,
            atom_role: decoder.field(2, DefinitionAtomRole::decode)?,
            start: decoder.field(3, DecodedPersistentSymbolRequest::decode)?,
            end: decoder.field(4, DecodedPersistentSymbolRequest::decode)?,
        })
    }
}

#[derive(Debug)]
struct DecodedDefinitionSymbolPlanV1 {
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    owner: DecodedStrongDefinitionEntity,
    definition_role: StrongDefinitionRole,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    primary_symbol: DecodedPersistentSymbolRequest,
    atom_boundaries: Vec<DecodedAtomBoundarySymbolsV1>,
}

impl WireEncode for DecodedDefinitionSymbolPlanV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.definition_plan.encode(encoder)?;
        encoder.field(2)?;
        self.owner.encode(encoder)?;
        encoder.field(3)?;
        self.definition_role.encode(encoder)?;
        encoder.field(4)?;
        self.primary_atom.encode(encoder)?;
        encoder.field(5)?;
        self.primary_symbol.encode(encoder)?;
        encoder.field(6)?;
        encode_array(encoder, &self.atom_boundaries)
    }
}

impl WireDecode for DecodedDefinitionSymbolPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            definition_plan: decoder.field(1, DecodedPersistentId::decode)?,
            owner: decoder.field(2, DecodedStrongDefinitionEntity::decode)?,
            definition_role: decoder.field(3, StrongDefinitionRole::decode)?,
            primary_atom: decoder.field(4, DecodedPersistentId::decode)?,
            primary_symbol: decoder.field(5, DecodedPersistentSymbolRequest::decode)?,
            atom_boundaries: decoder.field(6, |decoder| {
                decoder.decode_array(|decoder, _| DecodedAtomBoundarySymbolsV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedObjectSymbolSurfaceV1 {
    plans: Vec<DecodedDefinitionSymbolPlanV1>,
}

impl DecodedObjectSymbolSurfaceV1 {
    pub fn validate(
        self,
        foundation: &ConeLirFoundation,
    ) -> Result<ObjectSymbolSurfaceV1, ObjectSymbolSurfaceValidationError> {
        let actual = encode(&self).map_err(ObjectSymbolSurfaceValidationError::Encode)?;
        let expected = ObjectSymbolSurfaceV1::from_foundation(foundation)
            .map_err(ObjectSymbolSurfaceValidationError::Foundation)?;
        let expected_bytes =
            encode(&expected).map_err(ObjectSymbolSurfaceValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(ObjectSymbolSurfaceValidationError::SurfaceMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedObjectSymbolSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

impl WireDecode for DecodedObjectSymbolSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedDefinitionSymbolPlanV1::decode(decoder))
            .map(|plans| Self { plans })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSymbolSurfaceValidationError {
    Foundation(ObjectSymbolSurfaceBuildError),
    Encode(scoop_wire::cbor::EncodeError),
    SurfaceMismatch,
}

impl fmt::Display for ObjectSymbolSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong object symbol surface: {self:?}"
        )
    }
}

impl std::error::Error for ObjectSymbolSurfaceValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Foundation(source) => Some(source),
            Self::Encode(source) => Some(source),
            Self::SurfaceMismatch => None,
        }
    }
}

fn boundary_symbols(
    atom: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    linkage: LinkageClass,
) -> Result<AtomBoundarySymbolsV1, PersistentSymbolError> {
    Ok(AtomBoundarySymbolsV1 {
        atom,
        atom_role,
        start: PersistentSymbolRequest::new(
            PersistentSymbolKey::DefinitionBoundaryStart(atom),
            linkage,
        )?,
        end: PersistentSymbolRequest::new(
            PersistentSymbolKey::DefinitionBoundaryEnd(atom),
            linkage,
        )?,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectSymbolSurfaceBuildError {
    DefinitionSurface(ObjectDefinitionPlanBuildError),
    MissingDefinitionKey(ObjectDefinitionPlanId),
    MissingDefinitionAtom(ObjectDefinitionAtomId),
    UnknownDefinitionSubject(ObjectDefinitionPlanId),
    Symbol(PersistentSymbolError),
    DuplicatePrimarySymbol {
        symbol: PersistentSymbolKey,
        first: ObjectDefinitionPlanId,
        second: ObjectDefinitionPlanId,
    },
    MissingPrimarySymbolRequest {
        definition_plan: ObjectDefinitionPlanId,
        symbol: PersistentSymbolKey,
    },
}

impl fmt::Display for ObjectSymbolSurfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong object symbol surface: {self:?}")
    }
}

impl std::error::Error for ObjectSymbolSurfaceBuildError {}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
