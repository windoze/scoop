//! Only the saved production data used by the native program linker.
use super::*;
use crate::LinkDataError;
use scoop_identity::{
    ObjectDefinitionPlanId, ObjectDefinitionPlanOwner, PersistentSymbolRequest,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub struct ProgramLinkDefinition {
    pub owner: ObjectDefinitionPlanOwner,
    pub entity: StrongDefinitionEntity,
    pub role: StrongDefinitionRole,
}

pub struct ProgramLinkProduction {
    pub definitions: BTreeMap<ObjectDefinitionPlanId, ProgramLinkDefinition>,
    pub image_symbol: PersistentSymbolRequest,
    pub trap_symbols: [PersistentSymbolRequest; 2],
    pub entry_symbol: Option<PersistentSymbolRequest>,
    pub registrations: [Vec<(ObjectDefinitionPlanId, PersistentSymbolRequest)>; 6],
}

impl DecodedConeProductionSectionV2 {
    pub fn read_link(
        self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ProgramLinkProduction, LinkDataError> {
        let (image_symbol, trap_atoms) = self.image_plan.link_symbols(identities)?;
        let trap_symbols = self
            .canonical_definitions
            .link_trap_symbols(trap_atoms, identities)?;
        Ok(ProgramLinkProduction {
            definitions: self
                .canonical_definitions
                .link_definitions(foundation, identities)?,
            image_symbol,
            trap_symbols,
            entry_symbol: self.entry_plan.link_symbol(identities)?,
            registrations: self
                .registration_production
                .link_registrations(foundation, identities)?,
        })
    }
}
