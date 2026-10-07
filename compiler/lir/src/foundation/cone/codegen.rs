//! Remove private site and body records eliminated by ordinary code optimization.

use super::*;
use scoop_identity::{PersistentSafepointSiteId, PersistentSymbolRequestTable};
use std::collections::BTreeSet;

impl ConeLirFoundation {
    pub(crate) fn without_codegen_records(
        &self,
        sites: &BTreeSet<PersistentSafepointSiteId>,
        definitions: &BTreeSet<ObjectDefinitionPlanId>,
        atoms: &BTreeSet<ObjectDefinitionAtomId>,
    ) -> Self {
        if sites.is_empty() && atoms.is_empty() {
            return self.clone();
        }
        let mut result = self.clone();
        let canonical = Rc::make_mut(&mut result.canonical);
        let members = canonical
            .definition_plans
            .iter()
            .filter_map(|plan| {
                if !definitions.contains(&plan.id()) {
                    return None;
                }
                match plan.key().owner() {
                    ObjectDefinitionPlanOwner::Odr { member } => Some(member),
                    ObjectDefinitionPlanOwner::Strong { .. } => None,
                }
            })
            .collect::<BTreeSet<_>>();
        canonical
            .safepoint_sites
            .retain(|record| !sites.contains(&record.id()));
        canonical
            .safepoints
            .retain(|record| !sites.contains(&record.site()));
        canonical
            .odr_members
            .retain(|record| !members.contains(&record.id()));
        canonical
            .definition_plans
            .retain(|record| !definitions.contains(&record.id()));
        canonical
            .definition_atoms
            .retain(|record| !atoms.contains(&record.id()));
        let requests = canonical
            .symbol_requests
            .requests()
            .iter()
            .copied()
            .filter(|request| match request.key() {
                PersistentSymbolKey::SafepointRegistration(site) => !sites.contains(&site),
                PersistentSymbolKey::OdrMember(member) => !members.contains(&member),
                PersistentSymbolKey::DefinitionBoundaryStart(atom)
                | PersistentSymbolKey::DefinitionBoundaryEnd(atom) => !atoms.contains(&atom),
                _ => true,
            })
            .collect();
        canonical.symbol_requests = PersistentSymbolRequestTable::new(requests)
            .expect("retaining a subset preserves unique symbol requests");
        result
    }
}
