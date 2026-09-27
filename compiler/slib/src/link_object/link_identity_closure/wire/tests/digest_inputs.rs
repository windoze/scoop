use super::*;

#[test]
fn borrowed_digest_inputs_keep_wire_and_reject_duplicate_or_missing_intents() {
    let (plan, digest, decoded) = fixture();
    let bytes = encode(&decoded).unwrap();
    let sites = decoded.replay_digest_patch_inputs(&plan, &digest).unwrap();
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].checked_offset(), 144);
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let mut duplicate = decoded.clone();
    duplicate.patch_sites.push(decoded.patch_sites[0]);
    assert!(matches!(
        duplicate.replay_digest_patch_inputs(&plan, &digest),
        Err(LinkDigestPatchInputValidationError::DuplicatePatchIntent(_))
    ));
    let mut missing = decoded;
    missing.patch_sites.clear();
    assert!(matches!(
        missing.replay_digest_patch_inputs(&plan, &digest),
        Err(LinkDigestPatchInputValidationError::MissingPatchIntent(_))
    ));
}

fn fixture() -> (
    PlannedLinkObjectMemberSetV1,
    DigestFinalizationPlanV1,
    DecodedLinkIdentityClosureSectionV1,
) {
    let coordinate = wire_fixture_coordinate();
    let producer = coordinate.identity().unwrap();
    let (canonical, production) =
        crate::link_decode::strong_production_fixture_for_test(coordinate, &[]);
    let foundation = scoop_lir::ConeLirFoundation::try_new(producer, canonical).unwrap();
    let partition = ProducerUnitPartitionV1::from_foundation(&foundation).unwrap();
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
    let decoded = decode_canonical(&bytes).unwrap();
    (plan, digest, decoded)
}
