use super::*;
use scoop_wire::{ResourceKind, WirePath};

#[test]
fn borrowed_materialization_replay_rejects_duplicate_missing_and_wrong_kind_units() {
    let (partition, plan) = materialization_plan();
    let bytes = encoded_link_identity_closure_for_member_plan_test(&plan);
    let original: DecodedLinkIdentityClosureSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut duplicate = original.clone();
    duplicate
        .materializations
        .push(duplicate.materializations[0].clone());
    assert!(matches!(
        duplicate.replay_materializations(&partition, &mut meter()),
        Err(LinkObjectMaterializationValidationError::MemberPlan(
            LinkObjectMemberSetPlanError::DuplicateScoopLirDefinition(_)
        ))
    ));
    let mut missing = original.clone();
    missing.materializations.clear();
    assert!(matches!(
        missing.replay_materializations(&partition, &mut meter()),
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
            .map(|unit| decode_canonical(&encode(unit).unwrap(), DecodeLimits::default()).unwrap())
            .collect(),
    };
    assert!(matches!(
        wrong_kind.replay_materializations(&partition, &mut meter()),
        Err(LinkObjectMaterializationValidationError::UnknownGeneratedBridgeUnit(_))
    ));
    assert_eq!(
        original
            .replay_materializations(&partition, &mut meter())
            .unwrap(),
        plan
    );
    assert_eq!(encode(&original).unwrap(), bytes);
}

#[test]
fn borrowed_materialization_replay_preserves_inclusive_and_cumulative_budgets() {
    let (partition, plan) = materialization_plan();
    let bytes = encoded_link_identity_closure_for_member_plan_test(&plan);
    let decoded: DecodedLinkIdentityClosureSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let mut measured = meter();
    decoded
        .replay_materializations(&partition, &mut measured)
        .unwrap();
    let cost = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: cost,
        ..DecodeLimits::default()
    });
    assert_eq!(
        decoded
            .replay_materializations(&partition, &mut exact)
            .unwrap(),
        plan
    );
    assert_resource(
        decoded.replay_materializations(&partition, &mut exact),
        ResourceKind::ValidationWorkUnits,
    );
    for (limits, resource) in [
        (
            DecodeLimits {
                validation_work_units: cost - 1,
                ..DecodeLimits::default()
            },
            ResourceKind::ValidationWorkUnits,
        ),
        (
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::DecodedEdges,
        ),
        (
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::OwnedBytes,
        ),
        (
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            ResourceKind::LogicalHeapBytes,
        ),
    ] {
        assert_resource(
            decoded.replay_materializations(&partition, &mut BudgetMeter::new(limits)),
            resource,
        );
    }
}

fn assert_resource(
    result: Result<PlannedLinkObjectMemberSetV1, LinkObjectMaterializationValidationError>,
    resource: ResourceKind,
) {
    let Err(LinkObjectMaterializationValidationError::Resource(error)) = result else {
        panic!("expected a resource failure");
    };
    assert!(
        matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource)
    );
    assert!(
        error
            .path()
            .segments()
            .starts_with(WirePath::root().field(1).segments())
    );
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
