//! Library or executable entry plans and their physical member assignment.

use super::{MissingObjectDefinitionMemberV1, VerifiedBuiltinObjectStrongRelocationSetV1};
use crate::SlibMemberId;
use scoop_lir::{EntryProductionPlanV1, ExecutableEntryPlanV1};

mod finalization;
pub use finalization::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableEntryDataV1 {
    plan: ExecutableEntryPlanV1,
    member: SlibMemberId,
}
impl ExecutableEntryDataV1 {
    pub const fn plan(&self) -> &ExecutableEntryPlanV1 {
        &self.plan
    }
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryProductionBranchV1 {
    Library,
    Executable(Box<ExecutableEntryDataV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryProductionDataV1 {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    branch: EntryProductionBranchV1,
}
impl EntryProductionDataV1 {
    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }
    pub const fn branch(&self) -> &EntryProductionBranchV1 {
        &self.branch
    }
}

pub fn bind_entry_production_v1(
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: EntryProductionPlanV1,
) -> Result<EntryProductionDataV1, MissingObjectDefinitionMemberV1> {
    let branch = match plan {
        EntryProductionPlanV1::Library => EntryProductionBranchV1::Library,
        EntryProductionPlanV1::Executable(plan) => {
            let definition = plan.root_descriptor_definition();
            let member = builtins
                .member_plan()
                .member_for_definition(definition)
                .ok_or(MissingObjectDefinitionMemberV1(definition))?;
            EntryProductionBranchV1::Executable(Box::new(ExecutableEntryDataV1 {
                plan: *plan,
                member,
            }))
        }
    };
    Ok(EntryProductionDataV1 { builtins, branch })
}
