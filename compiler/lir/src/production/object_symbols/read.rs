//! Project consumer definitions from the identity records already read for this Cone.

use super::*;
use crate::{LinkDataError, link_data::link_error};

impl ObjectSymbolSurfaceV1 {
    pub(crate) fn read_foundation(foundation: &ConeLirFoundation) -> Result<Self, LinkDataError> {
        let plans = foundation
            .definition_plans()
            .iter()
            .map(|record| {
                let definition_plan = record.id();
                let (owner, definition_role) =
                    foundation.definition_subject(record).ok_or_else(|| {
                        link_error(format!("missing definition subject {definition_plan}"))
                    })?;
                let definition_owner = record.key().owner();
                let linkage = definition_linkage(definition_owner);
                let primary_symbol = PersistentSymbolRequest::new(
                    owner.primary_symbol_key(definition_role).ok_or_else(|| {
                        link_error(format!("invalid definition subject {definition_plan}"))
                    })?,
                    linkage,
                )
                .map_err(link_error)?;
                let (_, primary_atom) = foundation
                    .resolve_definition_atom(definition_plan, DefinitionAtomRole::Primary)
                    .map_err(link_error)?;
                let atom_boundaries = foundation
                    .definition_atoms_for_plan(definition_plan)
                    .map(|atom| {
                        boundary_symbols(atom.id(), atom.key().role(), linkage).map_err(link_error)
                    })
                    .collect::<Result<_, _>>()?;
                Ok(DefinitionSymbolPlanV1 {
                    definition_plan,
                    definition_owner,
                    owner,
                    definition_role,
                    primary_atom,
                    primary_symbol,
                    atom_boundaries,
                })
            })
            .collect::<Result<Vec<_>, LinkDataError>>()?;
        Ok(Self {
            symbol_owners: plans
                .iter()
                .map(|plan| (plan.primary_symbol.key(), plan.definition_plan))
                .collect(),
            atom_boundary_indices: atom_boundary_indices(&plans),
            plans,
        })
    }
}

pub(super) fn definition_linkage(owner: ObjectDefinitionPlanOwner) -> LinkageClass {
    match owner {
        ObjectDefinitionPlanOwner::Strong { .. } => LinkageClass::ConeStrong,
        ObjectDefinitionPlanOwner::Odr { .. } => LinkageClass::OdrWeak,
    }
}
