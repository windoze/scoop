//! Complete expected persistent symbols for every strong definition plan.

use std::fmt;

use scoop_identity::{
    DecodedPersistentId, DecodedPersistentSymbolRequest, DecodedStrongDefinitionEntity,
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionPlanId, ObjectDefinitionPlanOwner,
    ObjectDefinitionPlanRole, PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
};
pub use scoop_identity::{
    DefinitionAtomRole, PersistentDispatchTableId, PersistentLayoutId, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, encode};

use crate::{
    OdrFreeLirFoundation, StrongObjectDefinitionPlanBuildError, StrongObjectDefinitionPlanSurfaceV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongAtomBoundarySymbolsV1 {
    atom: ObjectDefinitionAtomId,
    atom_role: DefinitionAtomRole,
    start: PersistentSymbolRequest,
    end: PersistentSymbolRequest,
}

impl StrongAtomBoundarySymbolsV1 {
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

impl WireEncode for StrongAtomBoundarySymbolsV1 {
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
pub struct StrongDefinitionSymbolPlanV1 {
    definition_plan: ObjectDefinitionPlanId,
    owner: StrongDefinitionEntity,
    definition_role: StrongDefinitionRole,
    primary_atom: ObjectDefinitionAtomId,
    primary_symbol: PersistentSymbolRequest,
    atom_boundaries: Vec<StrongAtomBoundarySymbolsV1>,
}

impl StrongDefinitionSymbolPlanV1 {
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

    pub fn atom_boundaries(&self) -> &[StrongAtomBoundarySymbolsV1] {
        &self.atom_boundaries
    }
}

impl WireEncode for StrongDefinitionSymbolPlanV1 {
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
pub struct StrongObjectSymbolSurfaceV1 {
    plans: Vec<StrongDefinitionSymbolPlanV1>,
}

impl StrongObjectSymbolSurfaceV1 {
    pub fn from_odr_free_foundation(
        foundation: &OdrFreeLirFoundation,
    ) -> Result<Self, StrongObjectSymbolSurfaceBuildError> {
        let definitions = StrongObjectDefinitionPlanSurfaceV1::from_odr_free_foundation(foundation)
            .map_err(StrongObjectSymbolSurfaceBuildError::DefinitionSurface)?;
        Self::from_definition_plans(foundation, &definitions)
    }

    pub(crate) fn from_definition_plans(
        foundation: &OdrFreeLirFoundation,
        definitions: &StrongObjectDefinitionPlanSurfaceV1,
    ) -> Result<Self, StrongObjectSymbolSurfaceBuildError> {
        let mut plans = Vec::with_capacity(definitions.plans().len());
        for definition in definitions.plans() {
            let definition_plan = definition.plan();
            let key = foundation
                .definition_plan(definition_plan)
                .ok_or(StrongObjectSymbolSurfaceBuildError::MissingDefinitionKey(
                    definition_plan,
                ))?
                .key();
            let (
                ObjectDefinitionPlanOwner::Strong { producer, entity },
                ObjectDefinitionPlanRole::Strong(definition_role),
            ) = (key.owner(), key.definition_role())
            else {
                return Err(
                    StrongObjectSymbolSurfaceBuildError::InvalidStrongDefinition(definition_plan),
                );
            };
            if producer != foundation.producer() {
                return Err(
                    StrongObjectSymbolSurfaceBuildError::InvalidStrongDefinition(definition_plan),
                );
            }
            let primary_symbol = PersistentSymbolRequest::new(
                key.primary_symbol_key().ok_or(
                    StrongObjectSymbolSurfaceBuildError::InvalidStrongDefinition(definition_plan),
                )?,
                LinkageClass::ConeStrong,
            )
            .map_err(StrongObjectSymbolSurfaceBuildError::Symbol)?;
            if !foundation.contains_symbol_request(primary_symbol) {
                return Err(
                    StrongObjectSymbolSurfaceBuildError::MissingPrimarySymbolRequest {
                        definition_plan,
                        symbol: primary_symbol.key(),
                    },
                );
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
                        .ok_or(StrongObjectSymbolSurfaceBuildError::MissingDefinitionAtom(
                            atom,
                        ))?
                        .key()
                        .role();
                    boundary_symbols(atom, role)
                        .map_err(StrongObjectSymbolSurfaceBuildError::Symbol)
                })
                .collect::<Result<Vec<_>, _>>()?;
            plans.push(StrongDefinitionSymbolPlanV1 {
                definition_plan,
                owner: entity,
                definition_role,
                primary_atom: definition.primary_atom(),
                primary_symbol,
                atom_boundaries,
            });
        }
        Ok(Self { plans })
    }

    pub fn plans(&self) -> &[StrongDefinitionSymbolPlanV1] {
        &self.plans
    }

    pub fn plan(
        &self,
        definition_plan: ObjectDefinitionPlanId,
    ) -> Option<&StrongDefinitionSymbolPlanV1> {
        self.plans
            .binary_search_by_key(&definition_plan, |plan| plan.definition_plan)
            .ok()
            .map(|index| &self.plans[index])
    }
}

impl WireEncode for StrongObjectSymbolSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

#[derive(Debug)]
struct DecodedStrongAtomBoundarySymbolsV1 {
    atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    atom_role: DefinitionAtomRole,
    start: DecodedPersistentSymbolRequest,
    end: DecodedPersistentSymbolRequest,
}

impl WireEncode for DecodedStrongAtomBoundarySymbolsV1 {
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

impl WireDecode for DecodedStrongAtomBoundarySymbolsV1 {
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
struct DecodedStrongDefinitionSymbolPlanV1 {
    definition_plan: DecodedPersistentId<ObjectDefinitionPlanId>,
    owner: DecodedStrongDefinitionEntity,
    definition_role: StrongDefinitionRole,
    primary_atom: DecodedPersistentId<ObjectDefinitionAtomId>,
    primary_symbol: DecodedPersistentSymbolRequest,
    atom_boundaries: Vec<DecodedStrongAtomBoundarySymbolsV1>,
}

impl WireEncode for DecodedStrongDefinitionSymbolPlanV1 {
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

impl WireDecode for DecodedStrongDefinitionSymbolPlanV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            definition_plan: decoder.field(1, DecodedPersistentId::decode)?,
            owner: decoder.field(2, DecodedStrongDefinitionEntity::decode)?,
            definition_role: decoder.field(3, StrongDefinitionRole::decode)?,
            primary_atom: decoder.field(4, DecodedPersistentId::decode)?,
            primary_symbol: decoder.field(5, DecodedPersistentSymbolRequest::decode)?,
            atom_boundaries: decoder.field(6, |decoder| {
                decoder
                    .decode_array(|decoder, _| DecodedStrongAtomBoundarySymbolsV1::decode(decoder))
            })?,
        })
    }
}

#[derive(Debug)]
pub struct DecodedStrongObjectSymbolSurfaceV1 {
    plans: Vec<DecodedStrongDefinitionSymbolPlanV1>,
}

impl DecodedStrongObjectSymbolSurfaceV1 {
    pub fn validate(
        self,
        foundation: &OdrFreeLirFoundation,
    ) -> Result<StrongObjectSymbolSurfaceV1, StrongObjectSymbolSurfaceValidationError> {
        let actual = encode(&self).map_err(StrongObjectSymbolSurfaceValidationError::Encode)?;
        let expected = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(foundation)
            .map_err(StrongObjectSymbolSurfaceValidationError::Foundation)?;
        let expected_bytes =
            encode(&expected).map_err(StrongObjectSymbolSurfaceValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(StrongObjectSymbolSurfaceValidationError::SurfaceMismatch);
        }
        Ok(expected)
    }
}

impl WireEncode for DecodedStrongObjectSymbolSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
    }
}

impl WireDecode for DecodedStrongObjectSymbolSurfaceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedStrongDefinitionSymbolPlanV1::decode(decoder))
            .map(|plans| Self { plans })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongObjectSymbolSurfaceValidationError {
    Foundation(StrongObjectSymbolSurfaceBuildError),
    Encode(scoop_wire::cbor::EncodeError),
    SurfaceMismatch,
}

impl fmt::Display for StrongObjectSymbolSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong object symbol surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongObjectSymbolSurfaceValidationError {
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
) -> Result<StrongAtomBoundarySymbolsV1, PersistentSymbolError> {
    Ok(StrongAtomBoundarySymbolsV1 {
        atom,
        atom_role,
        start: PersistentSymbolRequest::new(
            PersistentSymbolKey::DefinitionBoundaryStart(atom),
            LinkageClass::ConeStrong,
        )?,
        end: PersistentSymbolRequest::new(
            PersistentSymbolKey::DefinitionBoundaryEnd(atom),
            LinkageClass::ConeStrong,
        )?,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongObjectSymbolSurfaceBuildError {
    DefinitionSurface(StrongObjectDefinitionPlanBuildError),
    MissingDefinitionKey(ObjectDefinitionPlanId),
    MissingDefinitionAtom(ObjectDefinitionAtomId),
    InvalidStrongDefinition(ObjectDefinitionPlanId),
    Symbol(PersistentSymbolError),
    MissingPrimarySymbolRequest {
        definition_plan: ObjectDefinitionPlanId,
        symbol: PersistentSymbolKey,
    },
}

impl fmt::Display for StrongObjectSymbolSurfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong object symbol surface: {self:?}")
    }
}

impl std::error::Error for StrongObjectSymbolSurfaceBuildError {}

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
