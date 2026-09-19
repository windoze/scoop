use super::*;
use scoop_wire::{ResourceKind, WireErrorKind};

#[test]
fn signature_reuses_exact_wire_and_rejects_parameter_allocation_over_budget() {
    let mut fixture = Fixture::new();
    let exact = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(fixture.value),
        vec![fixture.unit; 64],
        fixture.unit,
    );
    let signature = MirBridgeCallableSignatureV1::new(exact.clone(), crate::GcEffect::NoGc);
    let mut expected = vec![0xa2, 0x01];
    expected.extend(encode(&exact).unwrap());
    expected.extend([0x02, 0xa1, 0x00, 0x02]);
    assert_eq!(encode(&signature).unwrap(), expected);
    let decoded: DecodedMirBridgeCallableSignatureV1 =
        decode_canonical(&expected, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), expected);
    let mut limited = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: 100,
        ..DecodeLimits::default()
    });
    let error = decoded
        .resolve(&mut fixture.graph, &mut limited)
        .unwrap_err();
    let MirCallableBridgeError::Resource(error) = error else {
        panic!("signature must use the shared allocation budget");
    };
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::LogicalHeapBytes,
            ..
        }
    ));
}

#[test]
fn signature_shared_budget_is_monotonic_and_accepts_the_exact_limit() {
    let mut fixture = Fixture::new();
    let signature = MirBridgeCallableSignatureV1::new(
        sig(None, vec![fixture.unit; 3], fixture.unit),
        crate::GcEffect::Managed,
    );
    let bytes = encode(&signature).unwrap();
    let decode = || {
        decode_canonical::<DecodedMirBridgeCallableSignatureV1>(&bytes, DecodeLimits::default())
            .unwrap()
    };
    let mut initial = meter();
    assert_eq!(
        decode().resolve(&mut fixture.graph, &mut initial).unwrap(),
        signature
    );
    let required = initial.usage();
    let mut limited = BudgetMeter::new(DecodeLimits {
        logical_heap_bytes: required.logical_heap_bytes,
        validation_work_units: required.validation_work_units,
        ..DecodeLimits::default()
    });
    assert_eq!(
        decode().resolve(&mut fixture.graph, &mut limited).unwrap(),
        signature
    );
    assert!(matches!(
        decode().resolve(&mut fixture.graph, &mut limited),
        Err(MirCallableBridgeError::Resource(_))
    ));
    assert_eq!(
        limited.usage().logical_heap_bytes,
        required.logical_heap_bytes
    );
}
