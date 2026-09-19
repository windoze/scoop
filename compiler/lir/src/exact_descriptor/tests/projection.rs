use super::*;

#[test]
fn descriptor_projection_preserves_complete_semantics_and_excludes_both_post_slots() {
    let fixture = Fixture::new();
    let descriptor = fixture.replay(fixture.semantic()).unwrap();
    let bytes = encode(&descriptor.semantic_projection()).unwrap();
    assert_eq!(bytes[0], 0xa8);
    let decoded: DecodedExactDescriptorSemanticProjectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    decoded
        .validate_against(&descriptor, &mut fixture.meter())
        .unwrap();
    let mut complete = bytes;
    complete[0] = 0xaa;
    complete.push(9);
    complete.extend(encode(&descriptor.definition()).unwrap());
    complete.push(10);
    complete.extend(encode(&descriptor.registration()).unwrap());
    assert_eq!(encode(&descriptor).unwrap(), complete);
    assert!(
        decode_canonical::<DecodedExactDescriptorSemanticProjectionV1>(
            &complete,
            DecodeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn descriptor_projection_rejects_diagnostic_tampering_and_shared_budget_exhaustion() {
    let fixture = Fixture::new();
    let descriptor = fixture.replay(fixture.semantic()).unwrap();
    let bytes = encode(&descriptor.semantic_projection()).unwrap();
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    let decoded: DecodedExactDescriptorSemanticProjectionV1 =
        decode_canonical(&changed, DecodeLimits::default()).unwrap();
    assert!(
        decoded
            .validate_against(&descriptor, &mut fixture.meter())
            .is_err()
    );
    let decoded: DecodedExactDescriptorSemanticProjectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(
        decoded
            .validate_against(
                &descriptor,
                &mut scoop_wire::BudgetMeter::new(DecodeLimits {
                    validation_work_units: 7,
                    ..DecodeLimits::default()
                })
            )
            .is_err()
    );
}
