use super::*;

pub(super) fn inspect(proof: &slib::ReplayedLayoutLinkSymbolUsesV1) {
    let objects = proof.object_contents();
    let mut members = objects
        .objects()
        .objects()
        .iter()
        .map(|object| object.member())
        .chain(objects.generated_objects().map(|object| object.member()))
        .collect::<Vec<_>>();
    members.sort_unstable();
    assert_eq!(
        members,
        proof
            .link_objects()
            .members()
            .iter()
            .map(|member| member.member())
            .collect::<Vec<_>>()
    );
}
