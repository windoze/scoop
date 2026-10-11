use super::*;
use crate::cross_cone_interface::default_templates::body::expressions::test_support::Fixture;
use scoop_wire::{decode_canonical, encode};

fn decoded_fixture(fixture: &Fixture) -> DecodedDefaultWhenV1 {
    let subject = crate::DefaultExpressionV1::try_new(
        crate::DefaultExpressionKindV1::UnitLiteral,
        fixture.value_type(),
        fixture.origin(),
        scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
    )
    .unwrap();
    let arm = DefaultWhenArmV1::try_new(
        DefaultWhenConditionV1::Case(crate::DefaultPatternV1::wildcard()),
        OptionalDefaultWhenGuardV1::absent(),
        Vec::new(),
        fixture.origin(),
    )
    .unwrap();
    let when = DefaultWhenV1::try_subject(
        subject,
        vec![arm],
        DefaultWhenFallbackV1::Impossible(ExhaustivenessProof::PatternMatrix),
    )
    .unwrap();
    let bytes = encode(&when.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    decode_canonical(&bytes).unwrap()
}

#[test]
fn case_requires_a_subject_at_the_format_boundary() {
    let fixture = Fixture::new();
    let mut decoded = decoded_fixture(&fixture);
    decoded.subject = DecodedOptionalDefaultExpressionV1::Absent;
    decoded.fallback = DecodedDefaultWhenFallbackV1::Fallthrough;
    let bytes = encode(&decoded).unwrap();
    let decoded: DecodedDefaultWhenV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
        Err(DefaultControlFlowResolutionError::Shape(
            DefaultControlFlowBuildError::MissingWhenSubject
        ))
    ));
}

#[test]
fn coverage_requires_a_subject_at_the_format_boundary() {
    let fixture = Fixture::new();
    let mut decoded = decoded_fixture(&fixture);
    decoded.subject = DecodedOptionalDefaultExpressionV1::Absent;
    decoded.arms.clear();
    let bytes = encode(&decoded).unwrap();
    let decoded: DecodedDefaultWhenV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
        Err(DefaultControlFlowResolutionError::Shape(
            DefaultControlFlowBuildError::MissingWhenSubject
        ))
    ));
}

#[test]
fn coverage_types_must_match_the_actual_subject_when_binding() {
    let fixture = Fixture::new();
    let wrong = SignatureTypeKey::RawPointer(Box::new(fixture.value_type()));
    let wrong: DecodedSignatureTypeKey = decode_canonical(&encode(&wrong).unwrap()).unwrap();
    let actual: DecodedSignatureTypeKey =
        decode_canonical(&encode(&fixture.value_type()).unwrap()).unwrap();
    for fallback in [
        DecodedDefaultWhenFallbackV1::IrrefutableArm {
            subject_type: wrong.clone(),
        },
        DecodedDefaultWhenFallbackV1::PatternMatrix {
            subject_type: wrong.clone(),
        },
        DecodedDefaultWhenFallbackV1::EnumPatternMatrix {
            subject_type: actual,
            owner_type: wrong,
        },
    ] {
        let mut decoded = decoded_fixture(&fixture);
        decoded.fallback = fallback;
        let bytes = encode(&decoded).unwrap();
        let decoded: DecodedDefaultWhenV1 = decode_canonical(&bytes).unwrap();
        assert!(matches!(
            decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
            Err(DefaultControlFlowResolutionError::Shape(
                DefaultControlFlowBuildError::WhenSubjectTypeMismatch
            ))
        ));
    }
}
