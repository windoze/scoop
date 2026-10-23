//! Worker-owned metadata keeps the ordinary borrowed provider view reusable.

use std::rc::Rc;

use super::*;

#[derive(Clone)]
pub struct OwnedImportedProvider {
    pub foundation: Rc<ImportedHirFoundation>,
    pub interface: Rc<CrossConeHirInterfaceSectionV1>,
    pub alias_expansions: Rc<CanonicalTypeAliasExpansionsV1>,
    pub core: Rc<crate::CoreBootstrapInterfaceSectionV1>,
}

impl OwnedImportedProvider {
    pub fn input(&self) -> ImportedProviderInput<'_> {
        ImportedProviderInput {
            foundation: &self.foundation,
            interface: &self.interface,
            alias_expansions: &self.alias_expansions,
        }
    }
}
