//! Select complete physical implementations before invoking the native linker.

use super::*;
use scoop_identity::{
    ObjectDefinitionPlanId, ObjectDefinitionPlanOwner, OdrMemberId, StrongDefinitionRole,
};

mod reachable;

#[derive(Default)]
pub(crate) struct SelectedMembers {
    members: BTreeSet<(ConeIdentity, SlibMemberId)>,
    definitions: BTreeMap<ConeIdentity, BTreeSet<ObjectDefinitionPlanId>>,
}

impl SelectedMembers {
    pub(super) fn new<'a>(
        closure: &ProgramLinkClosure,
        native_references: impl Iterator<Item = &'a str>,
    ) -> Result<Self, LinkError> {
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
            let surface = &artifact.production().definitions;
            for member in symbols.members() {
                let scoop_slib::ProgramLinkMember::Scoop { definitions, .. } = member else {
                    // Generated-C objects have ordinary provider ownership.
                    result.members.insert((cone, member.member()));
                    continue;
                };
                let plans = definitions.iter().map(|definition| {
                    surface.get(definition).map(|plan| (*definition, plan)).ok_or_else(|| error(format!(
                        "Cone {cone}: object member {} refers to missing definition {definition}", member.member()
                    )))
                }).collect::<Result<Vec<_>, _>>()?;
                let image = plans
                    .iter()
                    .any(|plan| plan.1.role == StrongDefinitionRole::ImageDescriptor);
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
                    .find(|plan| plan.1.role == StrongDefinitionRole::CallableBody);
                let owner = match body {
                    Some(plan) => Some(plan.1.owner),
                    None if plans.len() == 1 => Some(plans[0].1.owner),
                    None => {
                        if plans.iter().all(|plan| {
                            plan.1.role == StrongDefinitionRole::DispatchTable
                                && matches!(plan.1.owner, ObjectDefinitionPlanOwner::Odr { .. })
                        }) {
                            // Complete interface arrays and their parent aliases
                            // share bytes and must be selected as one physical unit.
                            let members = plans
                                .iter()
                                .map(|plan| {
                                    let ObjectDefinitionPlanOwner::Odr { member } = plan.1.owner
                                    else {
                                        unreachable!("ODR dispatch group")
                                    };
                                    member
                                })
                                .collect::<Vec<_>>();
                            let present = members
                                .iter()
                                .filter(|member| shared.contains(member))
                                .count();
                            if present == members.len() {
                                continue;
                            }
                            if present != 0 {
                                return Err(error(format!(
                                    "Cone {cone}: shared interface table storage has inconsistent physical grouping"
                                )));
                            }
                            shared.extend(members);
                            result.members.insert((cone, member.member()));
                            selected.extend(plans.iter().map(|plan| plan.0));
                            continue;
                        }
                        if plans.iter().any(|plan| {
                            matches!(plan.1.owner, ObjectDefinitionPlanOwner::Odr { .. })
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
                selected.extend(plans.iter().map(|plan| plan.0));
            }
        }
        result.retain_reachable(closure, native_references)?;
        Ok(result)
    }

    pub(crate) fn contains(&self, cone: ConeIdentity, member: SlibMemberId) -> bool {
        self.members.contains(&(cone, member))
    }

    pub(super) fn definitions(&self, cone: ConeIdentity) -> &BTreeSet<ObjectDefinitionPlanId> {
        &self.definitions[&cone]
    }
}
