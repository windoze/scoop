use super::*;

#[test]
fn materialization_replay_uses_the_artifact_target_for_member_roles() {
    let (partition, plan) = materialization_plan();
    let bytes = encoded_link_identity_closure_for_member_plan_test(&plan);
    let decoded: DecodedLinkIdentityClosureSectionV1 = decode_canonical(&bytes).unwrap();
    for target in [
        scoop_lir::LirTargetProfile::LINUX_X86_64_GNU,
        scoop_lir::LirTargetProfile::LINUX_X86_64_MUSL,
    ] {
        let replayed = decoded.replay_materializations(target, &partition).unwrap();
        assert_eq!(replayed.target(), target);
        assert_eq!(
            replayed.definition_assignments(),
            plan.definition_assignments()
        );
        for member in replayed.scoop_lir_members() {
            let crate::SlibMemberRole::LinkObject {
                target_profile,
                object_format,
                ..
            } = member.role()
            else {
                panic!("link object role");
            };
            assert_eq!(*target_profile, target.wire_id());
            assert_eq!(
                *object_format,
                scoop_identity::ObjectFormatId::elf_relocatable()
            );
        }
    }
}

#[test]
fn borrowed_materialization_replay_rejects_duplicate_missing_and_wrong_kind_units() {
    let (partition, plan) = materialization_plan();
    let bytes = encoded_link_identity_closure_for_member_plan_test(&plan);
    let original: DecodedLinkIdentityClosureSectionV1 = decode_canonical(&bytes).unwrap();
    let mut duplicate = original.clone();
    duplicate
        .materializations
        .push(duplicate.materializations[0].clone());
    assert!(matches!(
        duplicate.replay_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition),
        Err(LinkObjectMaterializationValidationError::MemberPlan(
            LinkObjectMemberSetPlanError::DuplicateScoopLirDefinition(_)
        ))
    ));
    let mut missing = original.clone();
    missing.materializations.clear();
    assert!(matches!(
        missing.replay_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition),
        Err(LinkObjectMaterializationValidationError::MemberPlan(
            LinkObjectMemberSetPlanError::MissingScoopLirDefinition(_)
        ))
    ));
    let mut wrong_kind = original.clone();
    let DecodedLinkObjectMaterializationV1::ScoopLir { member, units } =
        &original.materializations[0]
    else {
        panic!("fixture has Scoop units");
    };
    wrong_kind.materializations[0] = DecodedLinkObjectMaterializationV1::GeneratedCBridge {
        member: *member,
        units: units
            .iter()
            .map(|unit| decode_canonical(&encode(unit).unwrap()).unwrap())
            .collect(),
    };
    assert!(matches!(
        wrong_kind.replay_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition),
        Err(LinkObjectMaterializationValidationError::UnknownGeneratedBridgeUnit(_))
    ));
    assert_eq!(
        original
            .replay_materializations(scoop_lir::LirTargetProfile::DARWIN_AARCH64, &partition)
            .unwrap(),
        plan
    );
    assert_eq!(encode(&original).unwrap(), bytes);
}
