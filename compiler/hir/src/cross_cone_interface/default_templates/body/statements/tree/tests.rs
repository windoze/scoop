use std::num::NonZeroU32;

use scoop_identity::{CallableTemplateOrigin, LocalValueSelector};
use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::super::super::expressions::test_support::{Fixture, LocalError, definition_path};
use super::*;
use crate::{
    CanonicalBooleanV1, DecodedDefaultBindingActionV1, DecodedDefaultWhenFallbackV1,
    DecodedOptionalDefaultStatementListV1, DecodedOptionalDefaultWhenGuardV1,
    DefaultAppliedOptionV1, DefaultAssignTargetV1, DefaultBindingActionV1, DefaultBindingLeafV1,
    DefaultBindingPlanV1, DefaultBindingProjectionV1, DefaultBindingShapeV1,
    DefaultBindingTemporaryV1, DefaultCatchV1, DefaultEnumVariantFieldRefV1,
    DefaultEnumVariantRefV1, DefaultExpressionKindV1, DefaultExpressionV1,
    DefaultForIterationPlanV1, DefaultIteratorConformanceV1, DefaultIteratorNextV1,
    DefaultLocalFunctionV1, DefaultPatternV1, DefaultTryV1, DefaultWhenArmV1,
    DefaultWhenFallbackV1, DefaultWhenGuardV1, DefaultWhenV1, OptionalDefaultExpressionV1,
    OptionalDefaultStatementListV1, OptionalDefaultWhenGuardV1,
};

#[test]
fn every_statement_variant_keeps_its_frozen_wire_tag_and_round_trips() {
    let fixture = Fixture::new();
    let local_function = DefaultLocalFunctionV1::try_new(
        CallableTemplateOrigin::Function(fixture.function),
        definition_path(),
        fixture.value_type(),
        Vec::new(),
        0,
    )
    .unwrap();

    let cases = vec![
        statement(
            DefaultStatementKindV1::Expr(Box::new(unit(&fixture))),
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::InitializationEnsure(fixture.initialization),
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::LocalFunction(local_function),
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::Return(OptionalDefaultExpressionV1::absent()),
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::ValDecl {
                pattern: DefaultPatternV1::wildcard(),
                init: Box::new(unit(&fixture)),
            },
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::Assign {
                target: Box::new(DefaultAssignTargetV1::Local {
                    local: fixture.local(),
                }),
                value: Box::new(unit(&fixture)),
            },
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::If {
                condition: Box::new(unit(&fixture)),
                then_body: vec![break_statement(&fixture)],
                else_body: OptionalDefaultStatementListV1::try_present(vec![continue_statement(
                    &fixture,
                )])
                .unwrap(),
            },
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::While {
                condition_setup: vec![expression_statement(&fixture)],
                condition: Box::new(unit(&fixture)),
                body: vec![break_statement(&fixture)],
            },
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::For(Box::new(for_plan(&fixture))),
            &fixture,
        ),
        break_statement(&fixture),
        continue_statement(&fixture),
        statement(
            DefaultStatementKindV1::When(Box::new(when_value(&fixture))),
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::Try(Box::new(try_value(&fixture))),
            &fixture,
        ),
        statement(
            DefaultStatementKindV1::Throw(Box::new(unit(&fixture))),
            &fixture,
        ),
    ];

    assert_eq!(cases.len(), 14);
    for (expected_tag, expected) in (1_u64..=14).zip(cases) {
        let bytes = encode(&expected.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        assert_eq!(statement_tag(&bytes), expected_tag);
        let decoded: DecodedDefaultStatementV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
            Ok(expected)
        );
    }
}

#[test]
fn control_flow_optional_sums_are_explicit_and_round_trip() {
    let fixture = Fixture::new();

    let absent = OptionalDefaultStatementListV1::absent();
    let bytes = encode(&absent.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    assert_eq!(bytes, [0xa1, 0x00, 0x01]);
    let decoded: DecodedOptionalDefaultStatementListV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
        Ok(absent)
    );

    let present =
        OptionalDefaultStatementListV1::try_present(vec![break_statement(&fixture)]).unwrap();
    let bytes = encode(&present.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    assert_eq!(bytes[2], 2);
    let decoded: DecodedOptionalDefaultStatementListV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
        Ok(present)
    );

    let absent_guard = OptionalDefaultWhenGuardV1::absent();
    let bytes = encode(&absent_guard.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    assert_eq!(bytes, [0xa1, 0x00, 0x01]);
    let decoded: DecodedOptionalDefaultWhenGuardV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);

    let present_guard = OptionalDefaultWhenGuardV1::present(
        DefaultWhenGuardV1::try_new(vec![break_statement(&fixture)], unit(&fixture)).unwrap(),
    );
    let bytes = encode(&present_guard.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    assert_eq!(bytes[2], 2);
    let decoded: DecodedOptionalDefaultWhenGuardV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn every_when_fallback_variant_keeps_its_frozen_wire_tag() {
    let fixture = Fixture::new();
    let fallbacks = vec![
        DefaultWhenFallbackV1::try_else(vec![break_statement(&fixture)]).unwrap(),
        DefaultWhenFallbackV1::irrefutable_arm(fixture.value_type()),
        DefaultWhenFallbackV1::pattern_matrix(fixture.value_type()),
        DefaultWhenFallbackV1::enum_pattern_matrix(fixture.value_type(), fixture.value_type()),
    ];

    for (expected_tag, fallback) in (1_u8..=4).zip(fallbacks) {
        let bytes = encode(&fallback.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultWhenFallbackV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
    }
}

#[test]
fn every_binding_action_variant_keeps_its_frozen_wire_tag() {
    let fixture = Fixture::new();
    let source = temporary(&fixture);
    let result = temporary(&fixture);
    let actions = vec![
        DefaultBindingActionV1::project(
            source.clone(),
            result.clone(),
            DefaultBindingProjectionV1::tuple_index(0),
            fixture.origin(),
        ),
        DefaultBindingActionV1::try_component(
            source.clone(),
            NonZeroU32::new(1).unwrap(),
            result.clone(),
            vec![break_statement(&fixture)],
            unit(&fixture),
            fixture.origin(),
        )
        .unwrap(),
        DefaultBindingActionV1::bind(
            source,
            DefaultBindingLeafV1::new(
                fixture.local(),
                fixture.value_type(),
                CanonicalBooleanV1::False,
            ),
            fixture.origin(),
        ),
    ];

    for (expected_tag, action) in (1_u8..=3).zip(actions) {
        let bytes = encode(&action.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultBindingActionV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
    }
}

#[test]
fn nested_statement_index_errors_keep_the_full_path() {
    let fixture = Fixture::new();
    let missing_local = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::Local(LocalValueSelector::Parameter {
            declaration_index: 1,
        }),
        fixture.value_type(),
        fixture.origin(),
    )
    .unwrap();
    let nested = statement(
        DefaultStatementKindV1::Expr(Box::new(missing_local)),
        &fixture,
    );
    let outer = statement(
        DefaultStatementKindV1::If {
            condition: Box::new(unit(&fixture)),
            then_body: vec![nested],
            else_body: OptionalDefaultStatementListV1::absent(),
        },
        &fixture,
    );

    assert_eq!(
        outer.index_locals(&mut fixture.locals()).unwrap_err(),
        DefaultStatementIndexError::NestedStatement {
            variant_tag: 7,
            field: 2,
            index: 0,
            error: Box::new(DefaultStatementIndexError::Expression {
                variant_tag: 1,
                field: 1,
                error: crate::DefaultExpressionIndexError::Local(LocalError),
            }),
        }
    );
}

#[test]
fn statement_decoder_rejects_unknown_tags_and_non_exact_sums() {
    let fixture = Fixture::new();
    let bytes = encode(
        &break_statement(&fixture)
            .index_locals(&mut fixture.locals())
            .unwrap(),
    )
    .unwrap();

    let mut unknown = bytes.clone();
    unknown[4] = 15;
    let error = decode_canonical::<DecodedDefaultStatementV1>(&unknown).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 15 });

    let mut non_exact = bytes;
    non_exact[2] = 0xa2;
    non_exact.splice(5..5, [0x01, 0x00]);
    let error = decode_canonical::<DecodedDefaultStatementV1>(&non_exact).unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );

    let error =
        decode_canonical::<DecodedOptionalDefaultStatementListV1>(&[0xa1, 0x00, 0x03]).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

fn for_plan(fixture: &Fixture) -> DefaultForIterationPlanV1 {
    let source = temporary(fixture);
    let iterator = temporary(fixture);
    let result = temporary(fixture);
    let element = temporary(fixture);
    let actions = vec![
        DefaultBindingActionV1::project(
            source.clone(),
            result.clone(),
            DefaultBindingProjectionV1::tuple_index(0),
            fixture.origin(),
        ),
        DefaultBindingActionV1::try_component(
            source.clone(),
            NonZeroU32::new(1).unwrap(),
            result.clone(),
            vec![expression_statement(fixture)],
            unit(fixture),
            fixture.origin(),
        )
        .unwrap(),
        DefaultBindingActionV1::bind(
            result,
            DefaultBindingLeafV1::new(
                fixture.local(),
                fixture.value_type(),
                CanonicalBooleanV1::False,
            ),
            fixture.origin(),
        ),
    ];
    let binding =
        DefaultBindingPlanV1::try_new(source.clone(), DefaultBindingShapeV1::wildcard(), actions)
            .unwrap();
    let conformance = DefaultIteratorConformanceV1::new(
        source.clone(),
        iterator,
        fixture.value_type(),
        fixture.origin(),
    );
    let option = DefaultAppliedOptionV1::new(
        DefaultEnumVariantFieldRefV1::new(fixture.variant_field, fixture.value_type()),
        DefaultEnumVariantRefV1::new(fixture.variant, fixture.value_type()),
    );
    let next = DefaultIteratorNextV1::new(
        fixture.callable(),
        element.clone(),
        option,
        element,
        fixture.origin(),
    );
    DefaultForIterationPlanV1::try_new(
        vec![expression_statement(fixture)],
        source,
        unit(fixture),
        vec![expression_statement(fixture)],
        unit(fixture),
        conformance,
        next,
        binding,
        vec![break_statement(fixture)],
    )
    .unwrap()
}

fn when_value(fixture: &Fixture) -> DefaultWhenV1 {
    let guard =
        DefaultWhenGuardV1::try_new(vec![expression_statement(fixture)], unit(fixture)).unwrap();
    let arm = DefaultWhenArmV1::try_new(
        DefaultPatternV1::wildcard(),
        OptionalDefaultWhenGuardV1::present(guard),
        vec![break_statement(fixture)],
        fixture.origin(),
    )
    .unwrap();
    DefaultWhenV1::try_new(
        unit(fixture),
        vec![arm],
        DefaultWhenFallbackV1::pattern_matrix(fixture.value_type()),
    )
    .unwrap()
}

fn try_value(fixture: &Fixture) -> DefaultTryV1 {
    let catch = DefaultCatchV1::try_new(
        fixture.local(),
        fixture.value_type(),
        vec![continue_statement(fixture)],
        fixture.origin(),
    )
    .unwrap();
    DefaultTryV1::try_new(
        vec![expression_statement(fixture)],
        vec![catch],
        OptionalDefaultStatementListV1::try_present(vec![expression_statement(fixture)]).unwrap(),
    )
    .unwrap()
}

fn temporary(fixture: &Fixture) -> DefaultBindingTemporaryV1 {
    DefaultBindingTemporaryV1::new(fixture.local(), fixture.value_type())
}

fn statement(kind: DefaultStatementKindV1, fixture: &Fixture) -> DefaultStatementV1 {
    DefaultStatementV1::try_new(kind, fixture.origin()).unwrap()
}

fn expression_statement(fixture: &Fixture) -> DefaultStatementV1 {
    statement(
        DefaultStatementKindV1::Expr(Box::new(unit(fixture))),
        fixture,
    )
}

fn break_statement(fixture: &Fixture) -> DefaultStatementV1 {
    statement(DefaultStatementKindV1::Break, fixture)
}

fn continue_statement(fixture: &Fixture) -> DefaultStatementV1 {
    statement(DefaultStatementKindV1::Continue, fixture)
}

fn unit(fixture: &Fixture) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::UnitLiteral,
        fixture.value_type(),
        fixture.origin(),
    )
    .unwrap()
}

fn statement_tag(bytes: &[u8]) -> u64 {
    match bytes[4] {
        value @ 0..=23 => u64::from(value),
        0x18 => u64::from(bytes[5]),
        _ => u64::MAX,
    }
}
