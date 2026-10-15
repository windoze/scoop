use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedObjectSymbolSurfaceV1 {
    pub(crate) fn read(
        self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ObjectSymbolSurfaceV1, LinkDataError> {
        if self
            .plans
            .windows(2)
            .any(|pair| pair[0].definition_plan.as_array() >= pair[1].definition_plan.as_array())
        {
            return Err(link_error("noncanonical definition symbol order"));
        }
        let plans = self
            .plans
            .into_iter()
            .map(|plan| {
                let definition_plan = identities
                    .resolve(plan.definition_plan)
                    .map_err(link_error)?;
                let definition_owner = foundation
                    .definition_plan(definition_plan)
                    .ok_or_else(|| link_error(format!("missing definition {definition_plan}")))?
                    .key()
                    .owner();
                Ok(DefinitionSymbolPlanV1 {
                    definition_plan,
                    definition_owner,
                    owner: plan.owner.resolve(identities).map_err(link_error)?,
                    definition_role: plan.definition_role,
                    primary_atom: identities.resolve(plan.primary_atom).map_err(link_error)?,
                    primary_symbol: plan
                        .primary_symbol
                        .resolve(identities)
                        .map_err(link_error)?,
                    atom_boundaries: plan
                        .atom_boundaries
                        .into_iter()
                        .map(|atom| {
                            Ok(AtomBoundarySymbolsV1 {
                                atom: identities.resolve(atom.atom).map_err(link_error)?,
                                atom_role: atom.atom_role,
                                start: atom.start.resolve(identities).map_err(link_error)?,
                                end: atom.end.resolve(identities).map_err(link_error)?,
                            })
                        })
                        .collect::<Result<_, LinkDataError>>()?,
                })
            })
            .collect::<Result<Vec<_>, LinkDataError>>()?;
        Ok(ObjectSymbolSurfaceV1 {
            atom_boundary_indices: atom_boundary_indices(&plans),
            plans,
        })
    }
}
