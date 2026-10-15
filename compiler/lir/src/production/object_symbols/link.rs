use super::*;
use crate::{LinkDataError, link_data::link_error};
use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

impl DecodedObjectSymbolSurfaceV1 {
    pub(crate) fn link_definitions(
        &self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<BTreeMap<ObjectDefinitionPlanId, crate::ProgramLinkDefinition>, LinkDataError> {
        self.plans
            .iter()
            .map(|plan| {
                let id = identities
                    .resolve(plan.definition_plan)
                    .map_err(link_error)?;
                let owner = foundation
                    .definition_plan(id)
                    .ok_or_else(|| link_error(format!("missing definition {id}")))?
                    .key()
                    .owner();
                Ok((
                    id,
                    crate::ProgramLinkDefinition {
                        owner,
                        role: plan.definition_role,
                    },
                ))
            })
            .collect()
    }

    pub(crate) fn link_trap_symbols(
        &self,
        atoms: [ObjectDefinitionAtomId; 2],
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<[PersistentSymbolRequest; 2], LinkDataError> {
        let symbol = |atom: ObjectDefinitionAtomId, identities: &mut ValidatedIdentityGraph| {
            self.plans
                .iter()
                .flat_map(|plan| &plan.atom_boundaries)
                .find(|boundary| boundary.atom.as_array() == atom.as_array())
                .ok_or_else(|| link_error(format!("missing image trap atom {atom}")))?
                .start
                .resolve(identities)
                .map_err(link_error)
        };
        Ok([symbol(atoms[0], identities)?, symbol(atoms[1], identities)?])
    }
}
