use super::*;
use crate::DefaultIntegerOperationV1;

fn operand(fixture: &Fixture) -> DefaultExpressionV1 {
    expression(
        fixture,
        DefaultExpressionKindV1::IntegerLiteral(CanonicalIntegerConstantV1::Signed32(1)),
        core(DefaultOperationCoreTypeV1::Integer(
            DefaultIntegerKindV1::Signed32,
        )),
    )
}

fn integer_operation(
    fixture: &Fixture,
    arguments: DefaultIntegerArgumentsV1,
) -> DefaultExpressionV1 {
    expression(
        fixture,
        DefaultExpressionKindV1::IntegerOperation {
            operation: DefaultIntegerOperationV1::NoGc {
                kind: DefaultIntegerKindV1::Signed32,
                operation: DefaultNoGcIntegerOperationV1::Add,
            },
            arguments,
        },
        core(DefaultOperationCoreTypeV1::Integer(
            DefaultIntegerKindV1::Signed32,
        )),
    )
}

#[test]
fn validates_normalized_integer_operation_without_callable_authority() {
    let fixture = Fixture::new();
    let value = integer_operation(
        &fixture,
        DefaultIntegerArgumentsV1::binary(operand(&fixture), operand(&fixture)),
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    let mut authority = Authority::new();
    assert_eq!(validate(&template, &mut authority), Ok(()));
    assert_eq!(authority.intrinsic_validations, 0);
}

#[test]
fn normalized_integer_operation_rejects_wrong_arity() {
    let fixture = Fixture::new();
    let value = integer_operation(
        &fixture,
        DefaultIntegerArgumentsV1::Unary(Box::new(operand(&fixture))),
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    assert!(matches!(
        validate(&template, &mut Authority::new()),
        Err(ExportDefaultOperationTypingValidationError::Arity {
            expected: 2,
            actual: 1,
            ..
        })
    ));
}

#[test]
fn normalized_integer_operation_rejects_wrong_result() {
    let fixture = Fixture::new();
    let value = integer_operation(
        &fixture,
        DefaultIntegerArgumentsV1::binary(operand(&fixture), operand(&fixture)),
    );
    let value = expression(
        &fixture,
        value.kind().clone(),
        core(DefaultOperationCoreTypeV1::Boolean),
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    assert!(matches!(
        validate(&template, &mut Authority::new()),
        Err(ExportDefaultOperationTypingValidationError::Type { .. })
    ));
}

#[test]
fn normalized_unsigned_integer_rejects_logical_shift() {
    let fixture = Fixture::new();
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::IntegerOperation {
            operation: DefaultIntegerOperationV1::NoGc {
                kind: DefaultIntegerKindV1::Unsigned32,
                operation: DefaultNoGcIntegerOperationV1::Ushr,
            },
            arguments: DefaultIntegerArgumentsV1::binary(operand(&fixture), operand(&fixture)),
        },
        core(DefaultOperationCoreTypeV1::Integer(
            DefaultIntegerKindV1::Unsigned32,
        )),
    );
    let template = template(&fixture, value, Vec::new(), Vec::new(), false);
    assert!(matches!(
        validate(&template, &mut Authority::new()),
        Err(ExportDefaultOperationTypingValidationError::Problem {
            problem: DefaultOperationTypingProblemV1::InvalidIntegerOperation { .. },
            ..
        })
    ));
}
