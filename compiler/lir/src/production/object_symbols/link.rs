//! Link needs definition subjects and two trap symbols, not a complete symbol surface.

use super::*;
use crate::{LinkDataError, link_data::link_error};

impl ConeLirFoundation {
    pub(crate) fn link_definitions(
        &self,
    ) -> Result<BTreeMap<ObjectDefinitionPlanId, crate::ProgramLinkDefinition>, LinkDataError> {
        self.definition_plans()
            .iter()
            .map(|record| {
                let (entity, role) = self.definition_subject(record).ok_or_else(|| {
                    link_error(format!("missing definition subject {}", record.id()))
                })?;
                Ok((
                    record.id(),
                    crate::ProgramLinkDefinition {
                        owner: record.key().owner(),
                        entity,
                        role,
                    },
                ))
            })
            .collect()
    }

    pub(crate) fn link_trap_symbols(
        &self,
        atoms: [ObjectDefinitionAtomId; 2],
    ) -> Result<[PersistentSymbolRequest; 2], LinkDataError> {
        let symbol = |atom| {
            let atom = self
                .definition_atom(atom)
                .ok_or_else(|| link_error(format!("missing image trap atom {atom}")))?;
            let plan = self.definition_plan(atom.key().plan()).ok_or_else(|| {
                link_error(format!(
                    "missing image trap definition {}",
                    atom.key().plan()
                ))
            })?;
            PersistentSymbolRequest::new(
                PersistentSymbolKey::DefinitionBoundaryStart(atom.id()),
                super::read::definition_linkage(plan.key().owner()),
            )
            .map_err(link_error)
        };
        Ok([symbol(atoms[0])?, symbol(atoms[1])?])
    }
}
