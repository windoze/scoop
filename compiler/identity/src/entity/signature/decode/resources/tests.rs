use super::*;
use crate::{
    CallingConvention, ConeIdentity, Effect, NonEmptyVec, PersistentGenericTypeId,
    PersistentTypeId, SignatureTypeKey,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

#[test]
fn wide_tuples_check_work_and_depth_before_scheduling_their_elements() {
    let signature = DecodedSignatureTypeKey::Tuple(
        NonEmptyVec::new(vec![
            DecodedSignatureTypeKey::Binder { depth: 0, index: 0 };
            4096
        ])
        .unwrap(),
    );
    for limits in [
        DecodeLimits {
            validation_work_units: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
    ] {
        let mut limited = BudgetMeter::new(limits);
        assert!(signature.charge_resolution(&mut limited).is_err());
        assert_eq!(limited.usage().decoded_edges, 0);
        assert_eq!(limited.usage().decoded_nodes, 1);
    }
    let mut resources = BudgetMeter::new(DecodeLimits::default());
    signature.charge_resolution(&mut resources).unwrap();
    assert_eq!(resources.usage().decoded_nodes, 4097);
}

#[test]
fn aggregate_depth_continues_signature_recursion_and_rejects_overflow() {
    let signature =
        DecodedSignatureTypeKey::RawPointer(Box::new(DecodedSignatureTypeKey::Binder {
            depth: 0,
            index: 0,
        }));
    assert!(
        signature
            .charge_resolution_from_depth(
                &mut BudgetMeter::new(DecodeLimits {
                    semantic_recursion: 5,
                    ..DecodeLimits::default()
                }),
                5
            )
            .is_err()
    );
    signature
        .charge_resolution_from_depth(
            &mut BudgetMeter::new(DecodeLimits {
                semantic_recursion: 6,
                ..DecodeLimits::default()
            }),
            5,
        )
        .unwrap();
    assert_eq!(
        signature
            .charge_resolution_from_depth(
                &mut BudgetMeter::new(DecodeLimits {
                    semantic_recursion: u64::MAX,
                    ..DecodeLimits::default()
                }),
                u64::MAX
            )
            .unwrap_err()
            .kind(),
        &WireErrorKind::IntegerOutOfRange
    );
}

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
