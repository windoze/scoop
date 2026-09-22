use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::super::test_support::{Fixture, LocalError};
use super::*;
use crate::{
    CanonicalBooleanV1, CanonicalIntegerConstantV1, DefaultAnonymousFunctionV1,
    DefaultArrayAccessKindV1, DefaultBinaryOperatorV1, DefaultCallableBodyTypeArgumentsV1,
    DefaultCallableDeclarationV1, DefaultCallableReferenceTargetV1, DefaultCallableReferenceV1,
    DefaultClassConstructorIdV1, DefaultConstructorRefV1, DefaultEnumVariantFieldRefV1,
    DefaultEnumVariantRefV1, DefaultFieldRefV1, DefaultForeignCallbackOperationV1,
    DefaultIntegerKindV1, DefaultIntegerOperationV1, DefaultLambdaV1, DefaultMethodCalleeV1,
    DefaultNoGcIntegerOperationV1, DefaultPlaceV1, DefaultPrimitiveBinaryKindV1,
    DefaultPrimitiveUnaryKindV1, DefaultStringOwnerV1, DefaultUnaryOperatorV1,
};

#[test]
fn every_expression_variant_keeps_its_frozen_wire_tag() {
    let fixture = Fixture::new();
    let value_type = fixture.value_type();
    let struct_constructor = DefaultConstructorRefV1::Struct {
        declaration: fixture.constructor,
        owner_type: value_type.clone(),
    };
    let class_constructor = DefaultConstructorRefV1::Class {
        declaration: DefaultClassConstructorIdV1::Generated(fixture.generated),
        owner_type: value_type.clone(),
    };
    let variant = DefaultEnumVariantRefV1::new(fixture.variant, value_type.clone());
    let variant_field =
        DefaultEnumVariantFieldRefV1::new(fixture.variant_field, value_type.clone());
    let callable_reference = || {
        DefaultCallableReferenceV1::try_new(
            fixture.generated,
            super::super::test_support::definition_path(),
            DefaultCallableReferenceTargetV1::Named(fixture.callable()),
            value_type.clone(),
            Vec::new(),
            0,
        )
        .unwrap()
    };
    let lambda = || {
        DefaultLambdaV1::try_new(
            fixture.generated,
            super::super::test_support::definition_path(),
            value_type.clone(),
            DefaultCallableBodyTypeArgumentsV1::lexical(),
            Vec::new(),
            0,
        )
        .unwrap()
    };
    let anonymous = || {
        DefaultAnonymousFunctionV1::try_new(
            fixture.generated,
            super::super::test_support::definition_path(),
            value_type.clone(),
            DefaultCallableBodyTypeArgumentsV1::lexical(),
            Vec::new(),
            0,
        )
        .unwrap()
    };
    let assembly = || {
        DefaultArrayAssemblyV1::try_new(
            value_type.clone(),
            vec![DefaultArrayAssemblyPartV1::Element(unit(&fixture))],
            value_type.clone(),
        )
        .unwrap()
    };

    let cases = vec![
        DefaultExpressionKindV1::StringLiteral {
            value: "text".to_owned(),
            owner: DefaultStringOwnerV1::CurrentInstantiation,
        },
        DefaultExpressionKindV1::IntegerLiteral(CanonicalIntegerConstantV1::Signed32(7)),
        DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
        DefaultExpressionKindV1::UnitLiteral,
        DefaultExpressionKindV1::TupleLiteral(vec![unit(&fixture)]),
        DefaultExpressionKindV1::StructInit {
            constructor: struct_constructor,
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::StructConstruct {
            owner_type: value_type.clone(),
            fields: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::ClassInit {
            constructor: class_constructor,
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::VariantConstruct {
            variant: variant.clone(),
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::VariantTest {
            operand: Box::new(unit(&fixture)),
            variant,
        },
        DefaultExpressionKindV1::VariantPayloadProject {
            operand: Box::new(unit(&fixture)),
            field: variant_field,
        },
        DefaultExpressionKindV1::Local(fixture.local()),
        DefaultExpressionKindV1::GlobalRead(fixture.property),
        DefaultExpressionKindV1::SingletonValue(fixture.object),
        DefaultExpressionKindV1::Lambda(lambda()),
        DefaultExpressionKindV1::AnonymousFunction(anonymous()),
        DefaultExpressionKindV1::CallableReference(callable_reference()),
        DefaultExpressionKindV1::FunctionCoercion {
            source: Box::new(unit(&fixture)),
            source_function_type: value_type.clone(),
            target_function_type: value_type.clone(),
        },
        DefaultExpressionKindV1::PtrFromNonZeroULong(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::PtrToULong(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::PtrCast(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::PtrLoad {
            pointer: Box::new(unit(&fixture)),
            offset: OptionalDefaultExpressionV1::Absent,
        },
        DefaultExpressionKindV1::PtrStore {
            pointer: Box::new(unit(&fixture)),
            offset: OptionalDefaultExpressionV1::present(unit(&fixture)),
            value: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::PtrOffset {
            pointer: Box::new(unit(&fixture)),
            offset: Box::new(unit(&fixture)),
            subtract: CanonicalBooleanV1::False,
        },
        DefaultExpressionKindV1::AddressOf(DefaultPlaceV1::Local {
            local: fixture.local(),
        }),
        DefaultExpressionKindV1::SizeOf(value_type.clone()),
        DefaultExpressionKindV1::AlignOf(value_type.clone()),
        DefaultExpressionKindV1::FunctionAddress(DefaultCallableDeclarationV1::Function(
            fixture.function,
        )),
        DefaultExpressionKindV1::ForeignCallbackRegister {
            registration: fixture.callback,
            closure: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::ForeignCallbackOperation {
            operation: DefaultForeignCallbackOperationV1::Retain,
            callback: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::FieldAccess {
            receiver: Box::new(unit(&fixture)),
            field: DefaultFieldRefV1::Struct {
                declaration: fixture.field,
                owner_type: value_type.clone(),
            },
        },
        DefaultExpressionKindV1::MethodCall {
            receiver: Box::new(unit(&fixture)),
            callee: DefaultMethodCalleeV1::Callable(fixture.callable()),
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::DirectSuperMethodCall {
            receiver: Box::new(unit(&fixture)),
            callee: DefaultMethodCalleeV1::Callable(fixture.callable()),
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::Box(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::Unbox(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::IsInstance {
            operand: Box::new(unit(&fixture)),
            checked_type: value_type.clone(),
        },
        DefaultExpressionKindV1::Cast {
            operand: Box::new(unit(&fixture)),
            optional: CanonicalBooleanV1::True,
        },
        DefaultExpressionKindV1::ArrayLiteral(vec![unit(&fixture)]),
        DefaultExpressionKindV1::ArrayAssembly(assembly()),
        DefaultExpressionKindV1::Index {
            access: DefaultArrayAccessKindV1::ImmutableGet,
            receiver: Box::new(unit(&fixture)),
            index: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::ArraySet {
            access: DefaultArrayAccessKindV1::MutableSet,
            receiver: Box::new(unit(&fixture)),
            index: Box::new(unit(&fixture)),
            value: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::ArrayLen(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::ArrayClone(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::Call {
            callee: fixture.callable(),
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::LocalFunctionCall {
            declaration: scoop_identity::CallableTemplateOrigin::Function(fixture.function),
            callee: fixture.callable(),
            captures: vec![unit(&fixture)],
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::CallableCall {
            callee: Box::new(unit(&fixture)),
            function_type: value_type.clone(),
            arguments: vec![unit(&fixture)],
        },
        DefaultExpressionKindV1::PrimitiveBinary {
            kind: DefaultPrimitiveBinaryKindV1::StringConcat,
            lhs: Box::new(unit(&fixture)),
            rhs: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::PrimitiveUnary {
            kind: DefaultPrimitiveUnaryKindV1::BooleanNot,
            operand: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::IntegerOperation {
            operation: DefaultIntegerOperationV1::NoGc {
                kind: DefaultIntegerKindV1::Signed32,
                operation: DefaultNoGcIntegerOperationV1::Add,
            },
            arguments: DefaultIntegerArgumentsV1::binary(unit(&fixture), unit(&fixture)),
        },
        DefaultExpressionKindV1::IntegerConversion {
            source_kind: DefaultIntegerKindV1::Signed32,
            target_kind: DefaultIntegerKindV1::Unsigned64,
            operand: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::Binary {
            operator: DefaultBinaryOperatorV1::RefEq,
            lhs: Box::new(unit(&fixture)),
            rhs: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::Unary {
            operator: DefaultUnaryOperatorV1::Not,
            operand: Box::new(unit(&fixture)),
        },
        DefaultExpressionKindV1::SomeWrap(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::NoneLiteral,
        DefaultExpressionKindV1::IsSome(Box::new(unit(&fixture))),
        DefaultExpressionKindV1::Unwrap {
            operand: Box::new(unit(&fixture)),
            trap_on_none: CanonicalBooleanV1::True,
        },
    ];

    assert_eq!(cases.len(), 56);
    for (index, kind) in cases.into_iter().enumerate() {
        let expected_tag = u64::try_from(index + 1).unwrap();
        let expression = expression(kind, &fixture);
        let bytes = encode(&expression.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        assert_eq!(expression_tag(&bytes), expected_tag);
        let decoded: DecodedDefaultExpressionV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
    }
}

#[test]
fn leaf_and_recursive_expressions_have_fixed_tags_and_round_trip() {
    let fixture = Fixture::new();
    let unit = expression(DefaultExpressionKindV1::UnitLiteral, &fixture);
    let local = expression(DefaultExpressionKindV1::Local(fixture.local()), &fixture);
    let binary = expression(
        DefaultExpressionKindV1::Binary {
            operator: DefaultBinaryOperatorV1::RefEq,
            lhs: Box::new(local),
            rhs: Box::new(unit.clone()),
        },
        &fixture,
    );
    let pointer_load = expression(
        DefaultExpressionKindV1::PtrLoad {
            pointer: Box::new(unit.clone()),
            offset: OptionalDefaultExpressionV1::present(unit.clone()),
        },
        &fixture,
    );

    let cases = [
        (4, unit),
        (51, binary),
        (22, pointer_load),
        (
            54,
            expression(DefaultExpressionKindV1::NoneLiteral, &fixture),
        ),
    ];
    for (expected_tag, expected) in cases {
        let mut locals = fixture.locals();
        let bytes = encode(&expected.index_locals(&mut locals).unwrap()).unwrap();
        assert_eq!(expression_tag(&bytes), expected_tag);

        let decoded: DecodedDefaultExpressionV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
            Ok(expected)
        );
    }
}

#[test]
fn expression_index_errors_preserve_nested_location() {
    let fixture = Fixture::new();
    let missing_local = expression(
        DefaultExpressionKindV1::Local(scoop_identity::LocalValueSelector::Parameter {
            declaration_index: 1,
        }),
        &fixture,
    );
    let expression = expression(
        DefaultExpressionKindV1::Binary {
            operator: DefaultBinaryOperatorV1::And,
            lhs: Box::new(missing_local),
            rhs: Box::new(expression(DefaultExpressionKindV1::UnitLiteral, &fixture)),
        },
        &fixture,
    );

    assert_eq!(
        expression.index_locals(&mut fixture.locals()).unwrap_err(),
        DefaultExpressionIndexError::Nested {
            variant_tag: 51,
            field: 2,
            index: None,
            error: Box::new(DefaultExpressionIndexError::Local(LocalError)),
        }
    );
}

#[test]
fn explicit_optional_expression_sum_round_trips() {
    let absent: DecodedOptionalDefaultExpressionV1 =
        decode_canonical(&[0xa1, 0x00, 0x01], DecodeLimits::default()).unwrap();
    assert_eq!(absent, DecodedOptionalDefaultExpressionV1::Absent);
    assert_eq!(encode(&absent).unwrap(), [0xa1, 0x00, 0x01]);

    let error = decode_canonical::<DecodedOptionalDefaultExpressionV1>(
        &[0xa1, 0x00, 0x03],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

#[test]
fn expression_decoder_rejects_unknown_tags_and_non_exact_sums() {
    let error = decode_canonical::<DecodedDefaultExpressionV1>(
        &[0xa3, 0x01, 0xa1, 0x00, 0x18, 0x39],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 57 });

    let error = decode_canonical::<DecodedDefaultExpressionV1>(
        &[0xa3, 0x01, 0xa2, 0x00, 0x04, 0x01, 0x00],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn expression_builders_enforce_constructor_and_declaration_domains() {
    let fixture = Fixture::new();
    let class_constructor = crate::DefaultConstructorRefV1::Class {
        declaration: crate::DefaultClassConstructorIdV1::Generated(fixture.generated),
        owner_type: fixture.value_type(),
    };
    assert_eq!(
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::StructInit {
                constructor: class_constructor,
                arguments: Vec::new(),
            },
            fixture.value_type(),
            fixture.origin(),
        ),
        Err(DefaultExpressionBuildError::StructInitRequiresStructConstructor)
    );

    let local_declaration = scoop_identity::CallableTemplateOrigin::Constructor(
        scoop_identity::PersistentConstructorId::from_source_declaration(
            &scoop_identity::SourceDeclarationKey::constructor(
                scoop_identity::SourceDeclarationSite::new(
                    scoop_identity::ConeIdentity::SINGLE_FILE,
                    scoop_identity::PackagePath::root(),
                    scoop_identity::DefinitionOwnerChain::top_level(),
                    scoop_identity::DeclarationScope::ConeWide,
                )
                .unwrap(),
                Vec::new(),
            ),
        )
        .unwrap(),
    );
    assert_eq!(
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::LocalFunctionCall {
                declaration: local_declaration,
                callee: fixture.callable(),
                captures: Vec::new(),
                arguments: Vec::new(),
            },
            fixture.value_type(),
            fixture.origin(),
        ),
        Err(DefaultExpressionBuildError::UnsupportedLocalFunctionDeclaration(local_declaration))
    );
}

#[test]
fn canonical_boolean_payload_is_not_cbor_boolean() {
    let fixture = Fixture::new();
    let expression = expression(
        DefaultExpressionKindV1::BooleanLiteral(CanonicalBooleanV1::True),
        &fixture,
    );
    let bytes = encode(&expression.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    assert_eq!(expression_tag(&bytes), 3);
    assert!(bytes.windows(3).any(|window| window == [0x00, 0x03, 0x01]));
}

fn expression(kind: DefaultExpressionKindV1, fixture: &Fixture) -> DefaultExpressionV1 {
    DefaultExpressionV1::try_new(kind, fixture.value_type(), fixture.origin()).unwrap()
}

fn unit(fixture: &Fixture) -> DefaultExpressionV1 {
    expression(DefaultExpressionKindV1::UnitLiteral, fixture)
}

fn expression_tag(bytes: &[u8]) -> u64 {
    match bytes[4] {
        value @ 0..=23 => u64::from(value),
        0x18 => u64::from(bytes[5]),
        _ => u64::MAX,
    }
}
