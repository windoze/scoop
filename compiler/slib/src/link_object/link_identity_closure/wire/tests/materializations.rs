use super::*;

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
        duplicate.replay_materializations(&partition),
        Err(LinkObjectMaterializationValidationError::MemberPlan(
            LinkObjectMemberSetPlanError::DuplicateScoopLirDefinition(_)
        ))
    ));
    let mut missing = original.clone();
    missing.materializations.clear();
    assert!(matches!(
        missing.replay_materializations(&partition),
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
        wrong_kind.replay_materializations(&partition),
        Err(LinkObjectMaterializationValidationError::UnknownGeneratedBridgeUnit(_))
    ));
    assert_eq!(original.replay_materializations(&partition).unwrap(), plan);
    assert_eq!(encode(&original).unwrap(), bytes);
}
