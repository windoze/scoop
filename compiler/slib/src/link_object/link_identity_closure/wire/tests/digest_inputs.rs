use super::*;
use scoop_wire::ResourceKind;

#[test]
fn borrowed_digest_inputs_keep_wire_and_reject_duplicate_or_missing_intents() {
    let (plan, digest, decoded) = fixture();
    let bytes = encode(&decoded).unwrap();
    let sites = decoded
        .replay_digest_patch_inputs(&plan, &digest, &mut meter())
        .unwrap();
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].checked_offset(), 144);
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let mut duplicate = decoded.clone();
    duplicate.patch_sites.push(decoded.patch_sites[0]);
    assert!(matches!(
        duplicate.replay_digest_patch_inputs(&plan, &digest, &mut meter()),
        Err(LinkDigestPatchInputValidationError::DuplicatePatchIntent(_))
    ));
    let mut missing = decoded;
    missing.patch_sites.clear();
    assert!(matches!(
        missing.replay_digest_patch_inputs(&plan, &digest, &mut meter()),
        Err(LinkDigestPatchInputValidationError::MissingPatchIntent(_))
    ));
}

#[test]
fn digest_input_replay_uses_the_same_inclusive_work_and_allocation_budget() {
    let (plan, digest, decoded) = fixture();
    let mut measured = meter();
    decoded
        .replay_digest_patch_inputs(&plan, &digest, &mut measured)
        .unwrap();
    let cost = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: cost,
        ..DecodeLimits::default()
    });
    decoded
        .replay_digest_patch_inputs(&plan, &digest, &mut exact)
        .unwrap();
    assert_resource(
        decoded.replay_digest_patch_inputs(&plan, &digest, &mut exact),
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
            decoded.replay_digest_patch_inputs(&plan, &digest, &mut BudgetMeter::new(limits)),
            resource,
        );
    }
}

fn fixture() -> (
    PlannedLinkObjectMemberSetV1,
    StrongDigestFinalizationPlanV1,
    DecodedLinkIdentityClosureSectionV1,
) {
    let coordinate = wire_fixture_coordinate();
    let producer = coordinate.identity().unwrap();
    let (canonical, production) =
        crate::link_decode::strong_production_fixture_for_test(coordinate, &[]);
    let foundation = scoop_lir::OdrFreeLirFoundation::try_new(producer, canonical).unwrap();
    let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let units =
        CanonicalScoopLirObjectUnitSetV1::new(partition.scoop_lir_definition_plans().to_vec())
            .unwrap();
    let plan = PlannedLinkObjectMemberSetV1::new(&partition, vec![units], Vec::new()).unwrap();
    let digest = production.digest_finalization_plan().clone();
    let patch = &digest.nodes()[0].patch_intents()[0];
    let member = plan
        .member_for_definition(patch.key().target_definition())
        .unwrap();
    let bytes = encoded_link_identity_closure_for_patch_test(&plan, None, patch.id(), member, 144);
    let decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    (plan, digest, decoded)
}

fn assert_resource(
    result: Result<Vec<ProvisionalDigestPatchSiteV1>, LinkDigestPatchInputValidationError>,
    resource: ResourceKind,
) {
    assert!(
        matches!(result, Err(LinkDigestPatchInputValidationError::Resource(error))
        if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: actual, .. } if *actual == resource))
    );
}

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
