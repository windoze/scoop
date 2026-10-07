use std::num::NonZeroU32;

use scoop_identity::{CanonicalIdentifier, Effect, GcEffect, SignatureTypeKey};
use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn parameter_list_preserves_declaration_order_and_rejects_duplicate_names() {
    let second = parameter("zeta", 1);
    let first = parameter("alpha", 0);
    let parameters =
        CanonicalSourceParameterShapesV1::try_new(vec![second.clone(), first.clone()]).unwrap();

    assert_eq!(parameters.parameters(), &[second.clone(), first]);
    assert_eq!(parameters.len_u32(), 2);
    assert!(!parameters.is_empty());
    assert_eq!(
        CanonicalSourceParameterShapesV1::try_new(vec![second.clone(), second]),
        Err(SourceParameterListBuildError::DuplicateName(identifier(
            "zeta"
        )))
    );

    let expected = [
        b"\x82".as_slice(),
        encode(&parameters.parameters()[0]).unwrap().as_slice(),
        encode(&parameters.parameters()[1]).unwrap().as_slice(),
    ]
    .concat();
    assert_eq!(encode(&parameters).unwrap(), expected);
}

#[test]
fn decoded_parameter_list_validates_names_types_and_duplicates() {
    let expected = CanonicalSourceParameterShapesV1::try_new(vec![
        parameter("first", 0),
        parameter("second", 1),
    ])
    .unwrap();
    let decoded: DecodedCanonicalSourceParameterShapesV1 =
        decode_canonical(&encode(&expected).unwrap()).unwrap();
    let mut authority = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);

    let duplicate = ParameterSequence(vec![parameter("same", 0), parameter("same", 1)]);
    let decoded: DecodedCanonicalSourceParameterShapesV1 =
        decode_canonical(&encode(&duplicate).unwrap()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(SourceParameterListValidationError::DuplicateName { index: 1, .. })
    ));

    let decoded: DecodedCanonicalSourceParameterShapesV1 =
        decode_canonical(&encode(&InvalidNameParameter).unwrap()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(SourceParameterListValidationError::Parameter {
            index: 0,
            error: SourceParameterShapeResolutionError::Name(_),
        })
    ));
}

#[test]
fn callable_effects_and_closed_leaf_enums_have_fixed_wire() {
    let effects = CallableSourceEffectsV1::try_new(
        Effect::Ordinary,
        CallableSafetyV1::Unsafe,
        GcEffect::NoGc,
        CallableImplementationV1::SourceExternC(scoop_identity::CAbiCallMode::NativeSafe),
        CallableOperatorRoleV1::Language(CallableOperatorV1::Component {
            index: NonZeroU32::new(3).unwrap(),
        }),
        CallableInfixV1::Infix,
    )
    .unwrap();
    assert_eq!(
        encode(&effects).unwrap(),
        hex("a701010202030204a20004010105a2000201a20018180103060207a10001")
    );
    assert_eq!(
        decode_canonical::<DecodedCallableSourceEffectsV1>(&encode(&effects).unwrap())
            .unwrap()
            .validate()
            .unwrap(),
        effects
    );

    assert_eq!(encode(&CallableSafetyV1::Safe).unwrap(), vec![1]);
    assert_eq!(
        encode(&CallableImplementationV1::SourceExternScoop).unwrap(),
        vec![0xa1, 0, 3]
    );
    assert_eq!(encode(&CallableInfixV1::Infix).unwrap(), vec![2]);
    assert_eq!(
        encode(&CallableModalityV1::InterfaceDefault).unwrap(),
        vec![4]
    );
    assert_eq!(encode(&PublicLookupAccessV1::PublicSlot).unwrap(), vec![2]);
    assert_eq!(
        encode(&PropertyDelegateOperatorV1::GetValue).unwrap(),
        vec![2]
    );
}

#[test]
fn callable_effects_reject_impossible_semantic_combinations() {
    let ordinary_role = CallableOperatorRoleV1::None;
    assert_eq!(
        CallableSourceEffectsV1::try_new(
            Effect::Suspend,
            CallableSafetyV1::Safe,
            GcEffect::NoGc,
            CallableImplementationV1::Scoop,
            ordinary_role,
            CallableInfixV1::Ordinary,
        ),
        Err(CallableSourceEffectsBuildError::NoGcSuspend)
    );
    assert_eq!(
        CallableSourceEffectsV1::try_new(
            Effect::Suspend,
            CallableSafetyV1::Safe,
            GcEffect::Managed,
            CallableImplementationV1::SourceExternScoop,
            ordinary_role,
            CallableInfixV1::Ordinary,
        ),
        Err(CallableSourceEffectsBuildError::SuspendExtern(
            CallableImplementationV1::SourceExternScoop
        ))
    );
    assert_eq!(
        CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            CallableSafetyV1::Safe,
            GcEffect::NoGc,
            CallableImplementationV1::SourceExternC(scoop_identity::CAbiCallMode::NativeSafe),
            ordinary_role,
            CallableInfixV1::Ordinary,
        ),
        Err(CallableSourceEffectsBuildError::SafeCExtern)
    );
    assert_eq!(
        CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            CallableSafetyV1::Unsafe,
            GcEffect::Managed,
            CallableImplementationV1::SourceExternC(scoop_identity::CAbiCallMode::NativeSafe),
            ordinary_role,
            CallableInfixV1::Ordinary,
        ),
        Err(CallableSourceEffectsBuildError::ManagedCExtern)
    );

    let decoded = decode_canonical::<DecodedCallableSourceEffectsV1>(&hex(
        "a701020201030204a1000105a10001060107a10001",
    ))
    .unwrap();
    assert_eq!(
        decoded.validate(),
        Err(CallableSourceEffectsBuildError::NoGcSuspend)
    );
}

#[test]
fn every_language_operator_tag_is_stable() {
    let operators = [
        CallableOperatorV1::UnaryPlus,
        CallableOperatorV1::UnaryMinus,
        CallableOperatorV1::Not,
        CallableOperatorV1::Inc,
        CallableOperatorV1::Dec,
        CallableOperatorV1::Plus,
        CallableOperatorV1::Minus,
        CallableOperatorV1::Times,
        CallableOperatorV1::Div,
        CallableOperatorV1::Rem,
        CallableOperatorV1::RangeTo,
        CallableOperatorV1::RangeUntil,
        CallableOperatorV1::Contains,
        CallableOperatorV1::Get,
        CallableOperatorV1::Set,
        CallableOperatorV1::Invoke,
        CallableOperatorV1::PlusAssign,
        CallableOperatorV1::MinusAssign,
        CallableOperatorV1::TimesAssign,
        CallableOperatorV1::DivAssign,
        CallableOperatorV1::RemAssign,
        CallableOperatorV1::CompareTo,
        CallableOperatorV1::Equals,
        CallableOperatorV1::Component {
            index: NonZeroU32::new(7).unwrap(),
        },
        CallableOperatorV1::Iterator,
    ];

    for (offset, operator) in operators.into_iter().enumerate() {
        let bytes = encode(&operator).unwrap();
        let decoded = decode_canonical::<CallableOperatorV1>(&bytes).unwrap();
        assert_eq!(decoded, operator);
        let tag = u64::try_from(offset + 1).unwrap();
        let expected = if tag <= 23 {
            vec![0xa1, 0x00, u8::try_from(tag).unwrap()]
        } else if tag == 24 {
            vec![0xa2, 0x00, 0x18, 0x18, 0x01, 0x07]
        } else {
            assert_eq!(tag, 25);
            vec![0xa1, 0x00, 0x18, 0x19]
        };
        assert_eq!(bytes, expected);
    }
}

#[test]
fn reader_rejects_unknown_tags_bad_sum_lengths_and_zero_component_index() {
    let unknown = decode_canonical::<CallableSafetyV1>(&[3]).unwrap_err();
    assert!(matches!(
        unknown.kind(),
        WireErrorKind::UnknownTag { tag: 3 }
    ));

    let missing_operator = decode_canonical::<CallableOperatorRoleV1>(&hex("a10002")).unwrap_err();
    assert!(matches!(
        missing_operator.kind(),
        WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    ));

    let zero_component = decode_canonical::<CallableOperatorV1>(&hex("a20018180100")).unwrap_err();
    assert_eq!(zero_component.kind(), &WireErrorKind::IntegerOutOfRange);
}

fn parameter(name: &str, index: u32) -> SourceParameterShapeV1 {
    SourceParameterShapeV1::new(
        identifier(name),
        SignatureTypeKey::Binder { depth: 0, index },
    )
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

struct ParameterSequence(Vec<SourceParameterShapeV1>);

impl WireEncode for ParameterSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for parameter in &self.0 {
            parameter.encode(encoder)?;
        }
        Ok(())
    }
}

struct InvalidNameParameter;

impl WireEncode for InvalidNameParameter {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(1)?;
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.text("not-canonical")?;
        encoder.field(2)?;
        SignatureTypeKey::Binder { depth: 0, index: 0 }.encode(encoder)
    }
}

fn hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
