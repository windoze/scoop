use super::*;

#[test]
fn shared_extension_function_receivers_check_variance_effect_and_arity() {
    for case in 0..6 {
        let (core, mut provider, base, derived) = provider();
        let wide = function(vec![base.clone()], derived.clone());
        let narrow = function(vec![derived.clone()], base.clone());
        let (source, target, accepted) = match case {
            0 => (wide, narrow, true),
            1 => (narrow, wide, false),
            2 => (
                function(vec![base.clone()], base.clone()),
                function(vec![base], derived),
                false,
            ),
            3 => (
                SignatureTypeKey::Function {
                    effect: Effect::Suspend,
                    parameters: vec![base],
                    result: Box::new(derived),
                },
                narrow,
                false,
            ),
            4 => (function(vec![base.clone(), base], derived), narrow, false),
            5 => (
                function(vec![narrow.clone()], wide.clone()),
                function(vec![wide], narrow),
                true,
            ),
            _ => unreachable!(),
        };
        check_relation(&core, &mut provider, &source, &target, accepted);
    }
}

#[test]
fn shared_extension_tuple_and_native_pointer_receivers_remain_invariant() {
    for case in 0..3 {
        for same in [false, true] {
            let (core, mut provider, base, derived) = provider();
            let source = if same { base.clone() } else { derived.clone() };
            let (source, target) = match case {
                0 => (
                    SignatureTypeKey::Tuple(NonEmptyVec::new(vec![source]).unwrap()),
                    SignatureTypeKey::Tuple(NonEmptyVec::new(vec![base]).unwrap()),
                ),
                1 => (
                    SignatureTypeKey::RawPointer(Box::new(source)),
                    SignatureTypeKey::RawPointer(Box::new(base)),
                ),
                2 => (
                    native_function(vec![source], derived.clone()),
                    native_function(vec![base], derived),
                ),
                _ => unreachable!(),
            };
            check_relation(&core, &mut provider, &source, &target, same);
        }
    }
}
