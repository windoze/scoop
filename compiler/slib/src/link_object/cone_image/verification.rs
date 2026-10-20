//! The image plan and its existing object member assignment.

use crate::SlibMemberId;
use crate::link_object::{
    MissingObjectDefinitionMemberV1, VerifiedBuiltinObjectStrongRelocationSetV1,
};
use scoop_lir::ConeImagePlanV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeImageDataV1 {
    plan: ConeImagePlanV1,
    member: SlibMemberId,
}
impl ConeImageDataV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.plan.cone().identity()
    }
    pub const fn plan(&self) -> &ConeImagePlanV1 {
        &self.plan
    }
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }
}

pub fn bind_cone_image_v1(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    plan: ConeImagePlanV1,
) -> Result<ConeImageDataV1, MissingObjectDefinitionMemberV1> {
    let member = builtins
        .member_plan()
        .member_for_definition(plan.definition_plan())
        .ok_or(MissingObjectDefinitionMemberV1(plan.definition_plan()))?;
    Ok(ConeImageDataV1 { plan, member })
}
