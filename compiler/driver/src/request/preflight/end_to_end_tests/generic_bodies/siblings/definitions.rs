use std::collections::BTreeMap;

pub(super) fn check(
    closure: &scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure,
    left: scoop_identity::ConeIdentity,
    right: scoop_identity::ConeIdentity,
    case: &str,
) {
    let mut expected = BTreeMap::<_, Vec<_>>::new();
    for (_, artifact) in closure.dependency_first() {
        for group in artifact.production_projection().odr_members().groups() {
            for member in group.members() {
                expected
                    .entry((group.group(), member.member()))
                    .or_default()
                    .push(artifact.provider());
            }
        }
    }
    let merged = closure.odr_definitions();
    assert_eq!(merged.members().len(), expected.len());
    let reversed = scoop_slib::merge_cross_cone_odr_definitions(
        closure
            .dependency_first()
            .map(|(sections, artifact)| (sections.identity_graph(), artifact))
            .collect::<Vec<_>>()
            .into_iter()
            .rev(),
    )
    .unwrap();
    assert_eq!(reversed.members().len(), expected.len());
    let mut shared_bodies = 0;
    let mut shared_registrations = 0;
    let mut shared_immortals = 0;
    for ((group, member), providers) in expected {
        let definition = merged.get(group, member).unwrap();
        let reverse = reversed.get(group, member).unwrap();
        assert_eq!(reverse.group_key(), definition.group_key());
        assert_eq!(reverse.key(), definition.key());
        assert_eq!(reverse.abi(), definition.abi());
        assert_eq!(reverse.definition(), definition.definition());
        assert_eq!(
            reverse
                .candidates()
                .map(|candidate| candidate.provider())
                .collect::<Vec<_>>(),
            providers.iter().copied().rev().collect::<Vec<_>>(),
        );
        assert_eq!(definition.member(), member);
        assert_eq!(definition.key().group(), group);
        assert_eq!(
            definition
                .candidates()
                .map(|candidate| candidate.provider())
                .collect::<Vec<_>>(),
            providers
        );
        for candidate in definition.candidates() {
            assert_eq!(
                candidate.definition().owner(),
                scoop_slib::LinkDefinitionOwnerV1::OdrDefinition(member)
            );
            let (_, artifact) = closure.artifact(candidate.provider()).unwrap();
            assert!(
                artifact
                    .defined_symbols()
                    .owners()
                    .contains(candidate.definition())
            );
        }
        if providers.contains(&left) && providers.contains(&right) {
            shared_bodies +=
                usize::from(definition.key().role() == scoop_identity::OdrMemberRole::CallableBody);
            shared_registrations += usize::from(
                definition.key().role() == scoop_identity::OdrMemberRole::RegistrationRecord,
            );
            shared_immortals += usize::from(
                definition.key().role() == scoop_identity::OdrMemberRole::ImmortalObject,
            );
        }
    }
    assert!(shared_bodies >= 2, "{case}: {shared_bodies}");
    assert!(shared_registrations >= 2, "{case}: {shared_registrations}");
    if case.starts_with("strings") {
        assert!(shared_immortals >= 4, "{case}: {shared_immortals}");
    }
}
