//! Select complete physical implementations before invoking the native linker.

use super::*;
use scoop_identity::{
    ObjectDefinitionPlanId, ObjectDefinitionPlanOwner, OdrMemberId, StrongDefinitionRole,
};

#[derive(Default)]
pub(crate) struct SelectedMembers {
    members: BTreeSet<(ConeIdentity, SlibMemberId)>,
    definitions: BTreeMap<ConeIdentity, BTreeSet<ObjectDefinitionPlanId>>,
}

impl SelectedMembers {
    pub(super) fn new(closure: &ProgramLinkClosure) -> Result<Self, LinkError> {
        let mut result = Self::default();
        let mut shared = BTreeSet::<OdrMemberId>::new();
        let mut artifacts: Vec<_> = closure.artifacts().collect();
        artifacts.sort_by(|(a, _), (b, _)| {
            let a = a.manifest().cone().coordinate();
            let b = b.manifest().cone().coordinate();
            (a.group(), a.name(), a.version()).cmp(&(b.group(), b.name(), b.version()))
        });
        for (artifact, symbols) in artifacts {
            let cone = artifact.identity();
            let selected = result.definitions.entry(cone).or_default();
            let surface = artifact.production().canonical_definitions();
            let builtins = symbols.object_contents().patch_sites().builtins();
            for member in builtins.strong_relocations().members() {
                let plans = member
                    .definitions()
                    .definitions()
                    .iter()
                    .map(|definition| {
                        surface
                            .plan(definition.definition())
                            .expect("verified physical definition")
                    })
                    .collect::<Vec<_>>();
                let image = plans
                    .iter()
                    .any(|plan| plan.definition_role() == StrongDefinitionRole::ImageDescriptor);
                if image {
                    if plans.len() != 1 {
                        return Err(error(format!(
                            "Cone {cone}: candidate image does not have an independent physical member"
                        )));
                    }
                    continue;
                }
                let body = plans
                    .iter()
                    .find(|plan| plan.definition_role() == StrongDefinitionRole::CallableBody);
                let owner = match body {
                    Some(plan) => Some(plan.definition_owner()),
                    None if plans.len() == 1 => Some(plans[0].definition_owner()),
                    None => {
                        if plans.iter().any(|plan| {
                            matches!(
                                plan.definition_owner(),
                                ObjectDefinitionPlanOwner::Odr { .. }
                            )
                        }) {
                            return Err(error(format!(
                                "Cone {cone}: independent ODR definitions share a physical member"
                            )));
                        }
                        None
                    }
                };
                if let Some(ObjectDefinitionPlanOwner::Odr { member }) = owner
                    && !shared.insert(member)
                {
                    continue;
                }
                result.members.insert((cone, member.member()));
                selected.extend(plans.iter().map(|plan| plan.definition_plan()));
            }
            // Generated-C units have ordinary provider ownership and no ODR candidates.
            for object in symbols.object_contents().generated_objects() {
                result.members.insert((cone, object.member()));
            }
        }
        Ok(result)
    }

    pub(crate) fn contains(&self, cone: ConeIdentity, member: SlibMemberId) -> bool {
        self.members.contains(&(cone, member))
    }

    pub(super) fn definitions(&self, cone: ConeIdentity) -> &BTreeSet<ObjectDefinitionPlanId> {
        &self.definitions[&cone]
    }
}
