use super::*;

pub(crate) fn verified_code_link_object_members() -> crate::VerifiedCodeLinkObjectMemberSetV1 {
    let (_, final_objects, _, _) = finalized_link_object_fixture();
    let plan = link_object_plan();
    let member_plan = &plan.scoop_lir_members()[0];
    let member = SlibMember::new(
        cone().identity(),
        member_plan.stable_key().clone(),
        member_plan.role().clone(),
        final_objects.objects()[0].bytes().to_vec(),
    )
    .unwrap();

    crate::verify_code_link_object_members_v1(final_objects, &[member.record().clone()]).unwrap()
}
