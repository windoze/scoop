use super::super::super::*;
use scoop_identity::{Effect, GcEffect};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

#[test]
fn integer_intrinsic_effects_are_checked_by_both_builder_and_reader() {
    for kind in crate::intrinsic_function_kinds() {
        let Some(effect) = kind.integer_gc_effect() else {
            continue;
        };
        let expected = match effect {
            crate::GcEffect::NoGc => GcEffect::NoGc,
            crate::GcEffect::Managed => GcEffect::Managed,
        };
        for (execution, gc_effect) in [
            (Effect::Ordinary, GcEffect::NoGc),
            (Effect::Ordinary, GcEffect::Managed),
            (Effect::Suspend, GcEffect::Managed),
        ] {
            let actual = CallableSourceEffectsV1::try_new(
                execution,
                CallableSafetyV1::Safe,
                gc_effect,
                CallableImplementationV1::Intrinsic(kind),
                CallableOperatorRoleV1::None,
                CallableInfixV1::Ordinary,
            );
            let ordinary = CallableSourceEffectsV1::try_new(
                execution,
                CallableSafetyV1::Safe,
                gc_effect,
                CallableImplementationV1::Scoop,
                CallableOperatorRoleV1::None,
                CallableInfixV1::Ordinary,
            )
            .unwrap();
            // Replace only the implementation field in the canonical six-field product.
            let mut bytes = encode(&ordinary).unwrap();
            assert_eq!(&bytes[7..11], &[4, 0xa1, 0, 1]);
            bytes.splice(
                8..11,
                encode(&CallableImplementationV1::Intrinsic(kind)).unwrap(),
            );
            let decoded: DecodedCallableSourceEffectsV1 =
                decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let read = decoded.validate();
            assert_eq!(actual, read, "{kind:?}");
            if execution == Effect::Ordinary && gc_effect == expected {
                assert!(actual.is_ok());
            } else {
                assert_eq!(
                    actual,
                    Err(CallableSourceEffectsBuildError::IntegerIntrinsicEffect {
                        kind,
                        execution,
                        gc_effect,
                    })
                );
            }
        }
    }
}
