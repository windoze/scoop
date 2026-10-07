use super::*;
use crate::{
    CallableImplementationV1, CallableInfixV1, CallableOperatorRoleV1, CallableSafetyV1,
    CallableSourceEffectsBuildError, CallableSourceEffectsV1, DecodedCallableSourceEffectsV1,
};
use scoop_identity::{Effect, GcEffect};
use scoop_wire::{decode_canonical, encode};

fn effect(implementation: CallableImplementationV1, gc: GcEffect) -> CallableSourceEffectsV1 {
    CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Unsafe,
        gc,
        implementation,
        CallableOperatorRoleV1::None,
        CallableInfixV1::Ordinary,
    )
    .unwrap()
}

fn condition(requirements: &[(u32, u32)]) -> CallableReleaseCallabilityV1 {
    CallableReleaseCallabilityV1::NoTransition {
        requirements: requirements
            .iter()
            .map(|&(depth, index)| ReleaseValueBinderV1 { depth, index })
            .collect(),
    }
}

#[test]
fn release_conditions_have_canonical_wire_and_keep_both_binder_frames() {
    assert_eq!(
        encode(&CallableReleaseCallabilityV1::Unavailable).unwrap(),
        [0xa1, 0, 1]
    );
    let effects = effect(CallableImplementationV1::Scoop, GcEffect::NoGc)
        .with_release_callability(condition(&[(1, 0), (0, 1), (1, 0)]))
        .unwrap();
    assert_eq!(effects.release_callability(), &condition(&[(0, 1), (1, 0)]));
    assert_eq!(
        encode(effects.release_callability()).unwrap(),
        [0xa2, 0, 2, 1, 0x82, 0x82, 0, 1, 0x82, 1, 0]
    );
    let decoded: DecodedCallableSourceEffectsV1 =
        decode_canonical(&encode(&effects).unwrap()).unwrap();
    assert_eq!(decoded.validate().unwrap(), effects);
    assert_eq!(effects.provider_entry_gc_effect(), GcEffect::NoGc);
}

#[test]
fn release_effect_cannot_upgrade_extern_or_managed_callables() {
    for (implementation, gc) in [
        (CallableImplementationV1::Scoop, GcEffect::Managed),
        (
            CallableImplementationV1::SourceExternC(scoop_identity::CAbiCallMode::NativeSafe),
            GcEffect::NoGc,
        ),
        (CallableImplementationV1::SourceExternScoop, GcEffect::NoGc),
    ] {
        assert_eq!(
            effect(implementation, gc).with_release_callability(condition(&[])),
            Err(CallableSourceEffectsBuildError::InvalidReleaseEffect)
        );
    }
}

#[test]
fn release_reader_rejects_unsorted_duplicate_and_out_of_range_binders() {
    for requirements in [vec![(1, 0), (0, 0)], vec![(0, 0), (0, 0)]] {
        let error = decode_canonical::<CallableReleaseCallabilityV1>(
            &encode(&condition(&requirements)).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::NonCanonicalCbor);
    }
    let error = decode_canonical::<CallableReleaseCallabilityV1>(&[
        0xa2, 0, 2, 1, 0x81, 0x82, 0x1b, 0, 0, 0, 1, 0, 0, 0, 0, 0,
    ])
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::IntegerOutOfRange);
    let old_effects = [0xa6, 1, 1, 2, 1, 3, 2, 4, 0xa1, 0, 1, 5, 0xa1, 0, 1, 6, 1];
    assert!(matches!(
        decode_canonical::<DecodedCallableSourceEffectsV1>(&old_effects)
            .unwrap_err()
            .kind(),
        WireErrorKind::InvalidLength {
            expected: 7,
            actual: 6
        }
    ));
}
