//! Complete expected persistent symbols for every strong definition plan.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::{
    DefinitionAtomRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    ObjectDefinitionPlanOwner, ObjectDefinitionPlanRole, PersistentSymbolError,
    PersistentSymbolKey, PersistentSymbolRequest, StrongDefinitionEntity, StrongDefinitionRole,
};

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
        let keys = foundation
            .definition_plans()
            .iter()
            .map(|record| (record.id(), *record.key()))
            .collect::<BTreeMap<_, _>>();
        let atom_roles = foundation
            .definition_atoms()
            .iter()
            .map(|record| (record.id(), record.key().role()))
            .collect::<BTreeMap<_, _>>();
        let mut plans = Vec::with_capacity(definitions.plans().len());
        for definition in definitions.plans() {
            let definition_plan = definition.plan();
            let key = keys.get(&definition_plan).ok_or(
                StrongObjectSymbolSurfaceBuildError::MissingDefinitionKey(definition_plan),
            )?;
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
                    let role = atom_roles.get(&atom).copied().ok_or(
                        StrongObjectSymbolSurfaceBuildError::MissingDefinitionAtom(atom),
                    )?;
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

#[cfg(test)]
mod tests;
