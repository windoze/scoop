use super::*;
use crate::DecodedExactDispatchSemanticProjectionV1;

#[test]
fn dispatch_projection_keeps_slot_contract_and_target_but_drops_definition() {
    let fixture = DirectFixture::new(1);
    let mut resolver = fixture.local_resolver();
    let record = ExactDispatchExportV1::replay(
        TARGET,
        (&fixture.vtable).into(),
        &[fixture.identity_input()],
        &fixture.foundation,
        &mut resolver,
        &mut meter(),
    )
    .unwrap();
    let bytes = encode(&record.semantic_projection()).unwrap();
    assert_eq!(bytes[0], 0xa4);
    let decoded: DecodedExactDispatchSemanticProjectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    decoded.validate_against(&record, &mut meter()).unwrap();
    let mut complete = bytes.clone();
    complete[0] = 0xa5;
    complete.push(5);
    complete.extend(encode(&record.definition()).unwrap());
    assert_eq!(encode(&record).unwrap(), complete);
    assert!(
        decode_canonical::<DecodedExactDispatchSemanticProjectionV1>(
            &complete,
            DecodeLimits::default()
        )
        .is_err()
    );
    let mut changed = bytes.clone();
    *changed.last_mut().unwrap() ^= 1;
    let decoded: DecodedExactDispatchSemanticProjectionV1 =
        decode_canonical(&changed, DecodeLimits::default()).unwrap();
    assert!(decoded.validate_against(&record, &mut meter()).is_err());
    let decoded: DecodedExactDispatchSemanticProjectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(
        decoded
            .validate_against(
                &record,
                &mut BudgetMeter::new(DecodeLimits {
                    owned_bytes: 1,
                    ..DecodeLimits::default()
                })
            )
            .is_err()
    );
}
