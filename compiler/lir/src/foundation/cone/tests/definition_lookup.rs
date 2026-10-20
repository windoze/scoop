use super::*;

#[test]
fn resolves_odr_callable_descriptor_and_release_subjects_through_typed_records() {
    let callable = callable_odr_member();
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::odr(callable)).unwrap();
    let (group, descriptor) = odr_records();
    let exact = nominal_exact("Shape");
    let release = CborIdentityRecord::from_key(
        OdrMemberKey::new(
            group.id(),
            OdrMemberRole::ReleaseHook,
            OdrMemberDiscriminator::ExactType(exact),
        )
        .unwrap(),
    )
    .unwrap();
    let release_body =
        RuntimeIdentityRecord::from_key(&CallableBodyKey::release_hook(exact)).unwrap();
    let subjects = [
        (
            callable.member(),
            StrongDefinitionEntity::callable_body(body.id()),
            StrongDefinitionRole::CallableBody,
        ),
        (
            descriptor.id(),
            StrongDefinitionEntity::exact_type(exact),
            StrongDefinitionRole::TypeDescriptor,
        ),
        (
            release.id(),
            StrongDefinitionEntity::callable_body(release_body.id()),
            StrongDefinitionRole::CallableBody,
        ),
    ];
    let plans = subjects
        .iter()
        .map(|(member, _, _)| {
            CborIdentityRecord::from_key(ObjectDefinitionPlanKey::odr(*member)).unwrap()
        })
        .collect::<Vec<_>>();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(vec![release_body, strong_body("unrelated"), body])
        .unwrap();
    canonical.set_odr_groups(vec![group]).unwrap();
    canonical
        .set_odr_members(vec![descriptor, release])
        .unwrap();
    canonical.set_definition_plans(plans.clone()).unwrap();
    let expected_wire = encode(&canonical).unwrap();
    let foundation = ConeLirFoundation::try_new(ConeIdentity::CORE, canonical).unwrap();
    assert_eq!(encode(&foundation).unwrap(), expected_wire);
    for (plan, (_, entity, role)) in plans.iter().zip(subjects) {
        assert_eq!(foundation.definition_subject(plan), Some((entity, role)));
        assert_eq!(
            foundation.clone().definition_subject(plan),
            Some((entity, role))
        );
        assert_eq!(foundation.definition_for(entity, role), Some(plan));
    }
}
