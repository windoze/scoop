use super::*;
use crate::cross_cone_interface::default_templates::body::expressions::test_support::Fixture;
use crate::{
    CanonicalIntegerConstantV1, DefaultIntegerOperationV1, IntegerBinaryOperator as B,
    IntegerUnaryOperator as U,
};
use scoop_wire::{decode_canonical, encode};

#[test]
fn complete_integer_operations_preserve_wire_and_operand_order() {
    let fixture = Fixture::new();
    let operand = |value| {
        DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::IntegerLiteral(CanonicalIntegerConstantV1::Signed32(value)),
            fixture.value_type(),
            fixture.origin(),
            scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
        )
        .unwrap()
    };
    let unary = [U::UnaryPlus, U::UnaryMinus, U::Inc, U::Dec, U::Inv]
        .into_iter()
        .map(|operation| IntegerExpression::Unary {
            kind: crate::IntegerKind::SIGNED_32,
            operation,
            operand: Box::new(operand(1)),
        });
    let binary = [
        B::Add,
        B::Sub,
        B::Mul,
        B::CompareTo,
        B::Equals,
        B::And,
        B::Or,
        B::Xor,
        B::Shl,
        B::Shr,
        B::Ushr,
        B::Div,
        B::Rem,
    ]
    .into_iter()
    .map(|operation| IntegerExpression::Binary {
        kind: crate::IntegerKind::SIGNED_32,
        operation,
        lhs: Box::new(operand(1)),
        rhs: Box::new(operand(2)),
    });
    for operation in unary.chain(binary) {
        let original = DefaultExpressionV1::try_new(
            DefaultExpressionKindV1::IntegerOperation(operation),
            fixture.value_type(),
            fixture.origin(),
            scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
        )
        .unwrap();
        let bytes = encode(&original.index_locals(&mut fixture.locals()).unwrap()).unwrap();
        let decoded: DecodedDefaultExpressionV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let restored = decoded
            .resolve(&mut fixture.resolver(), &mut fixture.locals())
            .unwrap();
        assert_eq!(restored, original);
        assert_eq!(
            encode(&restored.index_locals(&mut fixture.locals()).unwrap()).unwrap(),
            bytes
        );
    }
}

#[test]
fn decoded_integer_operations_reject_mismatched_arity() {
    let fixture = Fixture::new();
    let operand = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::IntegerLiteral(CanonicalIntegerConstantV1::Signed32(7)),
        fixture.value_type(),
        fixture.origin(),
        scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
    )
    .unwrap();
    let bytes = encode(&operand.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    let operand: DecodedDefaultExpressionV1 = decode_canonical(&bytes).unwrap();
    let cases = [
        (
            IntegerOperation::NoGc {
                kind: crate::IntegerKind::SIGNED_32,
                operation: crate::NoGcIntegerOperation::Add,
            },
            DecodedDefaultIntegerArgumentsV1::Unary(Box::new(operand.clone())),
            IntegerOperationArity::Binary,
            IntegerOperationArity::Unary,
        ),
        (
            IntegerOperation::Managed {
                kind: crate::IntegerKind::SIGNED_32,
                operation: crate::IntegerDivRem::Div,
            },
            DecodedDefaultIntegerArgumentsV1::Unary(Box::new(operand.clone())),
            IntegerOperationArity::Binary,
            IntegerOperationArity::Unary,
        ),
        (
            IntegerOperation::NoGc {
                kind: crate::IntegerKind::SIGNED_32,
                operation: crate::NoGcIntegerOperation::Inv,
            },
            DecodedDefaultIntegerArgumentsV1::Binary {
                lhs: Box::new(operand.clone()),
                rhs: Box::new(operand.clone()),
            },
            IntegerOperationArity::Unary,
            IntegerOperationArity::Binary,
        ),
    ];
    for (operation, arguments, expected, actual) in cases {
        let mut malformed = operand.clone();
        malformed.kind = DecodedDefaultExpressionKindV1::IntegerOperation {
            operation: DefaultIntegerOperationV1::from(operation),
            arguments,
        };
        let decoded: DecodedDefaultExpressionV1 =
            decode_canonical(&encode(&malformed).unwrap()).unwrap();
        assert_eq!(
            decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
            Err(DefaultExpressionResolutionError::IntegerOperationArity { expected, actual })
        );
    }
}
