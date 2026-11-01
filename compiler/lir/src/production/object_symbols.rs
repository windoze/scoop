//! Complete expected persistent symbols for every strong definition plan.

mod link;
mod read;

use std::collections::BTreeMap;
use std::fmt;

pub use scoop_identity::{
    DefinitionAtomRole, PersistentDispatchTableId, PersistentLayoutId, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_identity::{
    LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionPlanId, ObjectDefinitionPlanOwner,
    PersistentSymbolError, PersistentSymbolKey, PersistentSymbolRequest,
};
use scoop_wire::{Encoder, WireEncode};

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
    symbol_owners: BTreeMap<PersistentSymbolKey, ObjectDefinitionPlanId>,
    atom_boundary_indices: BTreeMap<ObjectDefinitionAtomId, (usize, usize)>,
}

impl ObjectSymbolSurfaceV1 {
    pub(crate) fn remove_codegen_records(
        &mut self,
        definitions: &std::collections::BTreeSet<ObjectDefinitionPlanId>,
        atoms: &std::collections::BTreeSet<ObjectDefinitionAtomId>,
    ) {
        self.plans
            .retain(|plan| !definitions.contains(&plan.definition_plan));
        self.symbol_owners
            .retain(|_, definition| !definitions.contains(definition));
        for plan in &mut self.plans {
            plan.atom_boundaries
                .retain(|boundary| !atoms.contains(&boundary.atom));
        }
        self.atom_boundary_indices = atom_boundary_indices(&self.plans);
    }

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
            let record = foundation.definition_plan(definition_plan).ok_or(
                ObjectSymbolSurfaceBuildError::MissingDefinitionKey(definition_plan),
            )?;
            let key = record.key();
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
        let atom_boundary_indices = atom_boundary_indices(&plans);
        Ok(Self {
            plans,
            symbol_owners,
            atom_boundary_indices,
        })
    }

    pub fn plans(&self) -> &[DefinitionSymbolPlanV1] {
        &self.plans
    }

    pub fn atom_boundary(&self, atom: ObjectDefinitionAtomId) -> Option<AtomBoundarySymbolsV1> {
        self.atom_boundary_indices
            .get(&atom)
            .map(|&(plan, boundary)| self.plans[plan].atom_boundaries[boundary])
    }

    pub fn plan(&self, definition_plan: ObjectDefinitionPlanId) -> Option<&DefinitionSymbolPlanV1> {
        self.plans
            .binary_search_by_key(&definition_plan, |plan| plan.definition_plan)
            .ok()
            .map(|index| &self.plans[index])
    }

    pub fn definition(
        &self,
        owner: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    ) -> Option<&DefinitionSymbolPlanV1> {
        let symbol = owner.primary_symbol_key(role)?;
        let definition = *self.symbol_owners.get(&symbol)?;
        self.plan(definition)
            .filter(|plan| plan.owner() == owner && plan.definition_role() == role)
    }
}

fn atom_boundary_indices(
    plans: &[DefinitionSymbolPlanV1],
) -> BTreeMap<ObjectDefinitionAtomId, (usize, usize)> {
    plans
        .iter()
        .enumerate()
        .flat_map(|(plan_index, plan)| {
            plan.atom_boundaries
                .iter()
                .enumerate()
                .map(move |(boundary_index, boundary)| {
                    (boundary.atom, (plan_index, boundary_index))
                })
        })
        .collect()
}

impl WireEncode for ObjectSymbolSurfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.plans)
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
