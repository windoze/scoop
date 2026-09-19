use super::*;
use crate::{
    CallingConvention, ConeIdentity, Effect, NonEmptyVec, PersistentGenericTypeId,
    PersistentTypeId, SignatureTypeKey,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

#[test]
fn source_signature_resolution_preflight_walks_every_shape_and_rejects_depth_before_resolution() {
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };
    let shapes = vec![
        SignatureTypeKey::Nominal(PersistentTypeId(ConeIdentity::CORE.0)),
        SignatureTypeKey::NominalApplication {
            origin: PersistentGenericTypeId(ConeIdentity::CORE.0),
            arguments: NonEmptyVec::from_first(binder.clone(), []),
        },
        SignatureTypeKey::Function {
            effect: Effect::Suspend,
            parameters: vec![binder.clone()],
            result: Box::new(binder.clone()),
        },
        SignatureTypeKey::RawPointer(Box::new(binder.clone())),
        SignatureTypeKey::NativeFunctionPointer {
            calling_convention: CallingConvention::C,
            parameters: vec![binder.clone()],
            result: Box::new(binder),
        },
    ];
    let signature = SignatureTypeKey::Tuple(NonEmptyVec::new(shapes).unwrap());
    let decoded: DecodedSignatureTypeKey =
        decode_canonical(&encode(&signature).unwrap(), DecodeLimits::default()).unwrap();
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    decoded.charge_resolution(&mut meter).unwrap();
    assert_eq!(meter.usage().decoded_nodes, 12);
    for limits in [
        DecodeLimits {
            semantic_recursion: 2,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(
            decoded
                .charge_resolution(&mut BudgetMeter::new(limits))
                .is_err()
        );
    }
}
