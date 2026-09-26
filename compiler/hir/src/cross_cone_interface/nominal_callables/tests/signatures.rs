use super::*;
use scoop_identity::{CallingConvention, NonEmptyVec, PersistentExactTypeId};

#[test]
fn structural_signatures_retain_order_effects_and_pointer_kinds() {
    let (classifier, unit, unit_exact) = classifier();
    let any = CoreBuiltinNominal::Any.identity_record().id();
    let any_exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(any)).unwrap();
    let unit = SignatureTypeKey::Nominal(unit);
    let any = SignatureTypeKey::Nominal(any);
    let cases = [
        (
            SignatureTypeKey::Tuple(NonEmptyVec::from_first(unit.clone(), [any.clone()])),
            ExactTypeKey::Tuple(NonEmptyVec::from_first(unit_exact, [any_exact])),
        ),
        (
            SignatureTypeKey::Tuple(NonEmptyVec::from_first(any.clone(), [unit.clone()])),
            ExactTypeKey::Tuple(NonEmptyVec::from_first(any_exact, [unit_exact])),
        ),
        (
            SignatureTypeKey::Function {
                effect: Effect::Ordinary,
                parameters: vec![unit.clone()],
                result: Box::new(any.clone()),
            },
            ExactTypeKey::Function {
                effect: Effect::Ordinary,
                parameters: vec![unit_exact],
                result: any_exact,
            },
        ),
        (
            SignatureTypeKey::Function {
                effect: Effect::Suspend,
                parameters: vec![unit.clone()],
                result: Box::new(any.clone()),
            },
            ExactTypeKey::Function {
                effect: Effect::Suspend,
                parameters: vec![unit_exact],
                result: any_exact,
            },
        ),
        (
            SignatureTypeKey::RawPointer(Box::new(unit.clone())),
            ExactTypeKey::RawPointer(unit_exact),
        ),
        (
            SignatureTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: vec![unit],
                result: Box::new(any),
            },
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: vec![unit_exact],
                result: any_exact,
            },
        ),
    ];
    let mut actual = Vec::new();
    for (signature, key) in cases {
        let exact = classifier.classify(&signature).unwrap().unwrap();
        assert_eq!(exact, PersistentExactTypeId::from_key(&key).unwrap());
        actual.push(exact);
    }
    actual.sort_unstable();
    actual.dedup();
    assert_eq!(actual.len(), 6);
}

#[test]
fn structural_signatures_require_every_nominal_leaf() {
    let (classifier, unit, _) = classifier();
    let signature = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        parameters: vec![SignatureTypeKey::Tuple(NonEmptyVec::from_first(
            SignatureTypeKey::Nominal(unit),
            [SignatureTypeKey::RawPointer(Box::new(
                SignatureTypeKey::Nominal(foreign_type()),
            ))],
        ))],
        result: Box::new(SignatureTypeKey::Nominal(unit)),
    };
    assert_eq!(classifier.classify(&signature).unwrap(), None);
}
