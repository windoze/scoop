use super::*;
use scoop_wire::{
    BudgetMeter, DecodeLimits, ResourceKind, WireErrorKind, decode_canonical, encode,
};

#[test]
fn normalization_preserves_final_bytes_and_accepts_adjacent_slots_in_either_order() {
    let fixture = patch_fixture([0x5a; 64], &[]);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.bytes,
    )];
    let sites = [
        ProvisionalDigestPatchSiteV1::new(
            fixture.intent,
            fixture.member,
            fixture.section_offset + 32,
            32,
        ),
        ProvisionalDigestPatchSiteV1::new(
            fixture.intent,
            fixture.member,
            fixture.section_offset,
            32,
        ),
    ];
    let normalized =
        normalize_final_scoop_lir_objects_v1(fixture.builtins.member_plan(), &objects, &sites)
            .unwrap();
    let object = &normalized.objects()[0];
    assert_eq!(object.final_bytes(), fixture.bytes);
    let start = fixture.section_offset as usize;
    assert_eq!(
        &object.provisional_bytes()[..start],
        &fixture.bytes[..start]
    );
    assert_eq!(&object.provisional_bytes()[start..start + 64], &[0; 64]);
    assert_eq!(
        &object.provisional_bytes()[start + 64..],
        &fixture.bytes[start + 64..]
    );
}

#[test]
fn normalization_rejects_duplicate_overlapping_outside_and_wrong_width_slots() {
    let fixture = patch_fixture([0; 64], &[]);
    let objects = [ScoopLirObjectCandidateV1::new(
        fixture.member,
        &fixture.bytes,
    )];
    let site = ProvisionalDigestPatchSiteV1::new(
        fixture.intent,
        fixture.member,
        fixture.section_offset + 16,
        32,
    );
    for delta in [0, 1, 31] {
        let overlap = ProvisionalDigestPatchSiteV1::new(
            fixture.intent,
            fixture.member,
            site.checked_offset() + delta,
            32,
        );
        for sites in [[site, overlap], [overlap, site]] {
            assert!(matches!(
                normalize_final_scoop_lir_objects_v1(
                    fixture.builtins.member_plan(),
                    &objects,
                    &sites
                ),
                Err(FinalObjectNormalizationError::OverlappingPatch { .. })
            ));
        }
    }
    for site in [
        ProvisionalDigestPatchSiteV1::new(
            fixture.intent,
            fixture.member,
            fixture.bytes.len() as u64 - 31,
            32,
        ),
        ProvisionalDigestPatchSiteV1::new(fixture.intent, fixture.member, u64::MAX, 32),
    ] {
        assert!(matches!(
            normalize_final_scoop_lir_objects_v1(fixture.builtins.member_plan(), &objects, &[site]),
            Err(FinalObjectNormalizationError::PatchRange { .. })
        ));
    }
    let wrong = ProvisionalDigestPatchSiteV1::new(
        fixture.intent,
        fixture.member,
        fixture.section_offset,
        31,
    );
    assert!(matches!(
        normalize_final_scoop_lir_objects_v1(fixture.builtins.member_plan(), &objects, &[wrong]),
        Err(FinalObjectNormalizationError::PatchWidth { .. })
    ));
}

#[test]
fn borrowed_object_projection_preserves_wire_and_requires_actual_definition_ranges() {
    let fixture = patch_fixture([0; 64], &[]);
    let offset = fixture.section_offset + SLOT_IN_ATOM;
    let bytes = crate::link_object::encoded_link_identity_closure_for_patch_test(
        fixture.builtins.member_plan(),
        &fixture.builtins,
        fixture.intent,
        fixture.member,
        offset,
    );
    let decoded: crate::DecodedLinkIdentityClosureSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let patch = ProvisionalDigestPatchSiteV1::new(fixture.intent, fixture.member, offset, 32);
    let patches = verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins,
        &fixture.foundation,
        fixture.digest_plan,
        &[ScoopLirObjectCandidateV1::new(
            fixture.member,
            &fixture.bytes,
        )],
        &[patch],
    )
    .unwrap();
    let plan = patches.builtins().member_plan();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    decoded
        .replay_object_projections(plan, &patches, &mut meter)
        .unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    let usage = meter.usage();
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    decoded
        .replay_object_projections(plan, &patches, &mut exact)
        .unwrap();
    assert!(
        matches!(decoded.replay_object_projections(plan, &patches, &mut exact),
        Err(crate::LinkObjectProjectionValidationError::Resource(error))
            if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
    );
    let stale =
        crate::link_object::encoded_link_identity_closure_without_object_projection_for_test(
            plan,
            fixture.intent,
            fixture.member,
            offset,
        );
    let stale: crate::DecodedLinkIdentityClosureSectionV1 =
        decode_canonical(&stale, DecodeLimits::default()).unwrap();
    assert_eq!(
        stale.replay_object_projections(
            plan,
            &patches,
            &mut BudgetMeter::new(DecodeLimits::default())
        ),
        Err(crate::LinkObjectProjectionValidationError::ProjectionMismatch)
    );
    for limits in [
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            decoded.replay_object_projections(plan, &patches, &mut BudgetMeter::new(limits)),
            Err(crate::LinkObjectProjectionValidationError::Resource(_))
        ));
    }
}
