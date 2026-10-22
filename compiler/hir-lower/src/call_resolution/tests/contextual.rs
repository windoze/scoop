use super::*;
use crate::call_resolution::contextual::{
    ArgumentExpression, ArgumentInferenceFailureKind, ArgumentInferenceInput, ArgumentPattern,
};
use crate::declaration_inference::DeclarationResult;

#[test]
fn fixed_point_facts_keep_unbound_parameters_for_the_complete_boundary() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(90, 0);
    let parameters = [callable.id];
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], &[callable]);
    let result = DeclarationResult::Known(lowerer.boolean);
    let input = ArgumentInferenceInput {
        expressions: &[],
        patterns: &[],
        parameters: &parameters,
        session: &mut session,
        environment,
        result,
        expected_result: None,
        forced_hint: None,
    };
    let facts = lowerer
        .infer_contextual_argument_facts(ArgumentInferenceInput {
            session: &mut *input.session,
            ..input
        })
        .unwrap_or_else(|_| panic!("no given input has failed"));
    assert!(facts.values.is_empty());
    assert!(facts.sinks.is_empty());
    assert!(matches!(facts.result, DeclarationResult::Known(ty) if ty == lowerer.boolean));
    let Err(failure) = lowerer.infer_contextual_arguments(input) else {
        panic!("an unbound type parameter cannot complete a call");
    };
    assert!(matches!(
        failure.kind,
        ArgumentInferenceFailureKind::Constraint(_)
    ));
}

#[test]
fn fixed_point_hints_do_not_certify_kind_bounds() {
    let mut lowerer = nominals::lowerer();
    let callable = parameter(91, 0);
    let parameters = [callable.id];
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], std::slice::from_ref(&callable));
    let variable = session.callable_variables(environment)[0];
    session.push(
        Constraint::Equal(variable.into(), lowerer.string.into()),
        ConstraintOrigin::ExplicitTypeArgument(0),
    );
    session.push(
        Constraint::Kind(variable.into(), hir::TypeParamKind::Value),
        ConstraintOrigin::TypeParameterBound(callable.id),
    );
    let input = ArgumentInferenceInput {
        expressions: &[],
        patterns: &[],
        parameters: &parameters,
        session: &mut session,
        environment,
        result: DeclarationResult::Known(lowerer.boolean),
        expected_result: None,
        forced_hint: None,
    };
    assert!(
        lowerer
            .infer_contextual_argument_facts(ArgumentInferenceInput {
                session: &mut *input.session,
                ..input
            })
            .is_ok()
    );
    let Err(failure) = lowerer.infer_contextual_arguments(input) else {
        panic!("String still cannot satisfy a value bound");
    };
    assert!(matches!(
        failure.kind,
        ArgumentInferenceFailureKind::Constraint(ref failure)
            if matches!(failure.kind, ConstraintFailureKind::Kind { .. })
    ));
}

#[test]
fn failed_argument_keeps_the_preceding_checked_expression() {
    let mut lowerer = nominals::lowerer();
    let first = scoop_ast::Expr::BoolLiteral {
        value: true,
        span: Span::new(0, 4),
    };
    let second = scoop_ast::Expr::This {
        label: Some(scoop_ast::Ident {
            text: "missing".into(),
            span: Span::new(11, 18),
        }),
        span: Span::new(6, 18),
    };
    let expressions = [
        ArgumentExpression::Source(&first),
        ArgumentExpression::Source(&second),
    ];
    let patterns = [ArgumentPattern {
        ty: lowerer.boolean,
        exact: false,
    }; 2];
    let mut session = InferenceSession::new();
    let environment = session.add_environment(&[], &[]);
    let Err(failure) = lowerer.infer_contextual_argument_facts(ArgumentInferenceInput {
        expressions: &expressions,
        patterns: &patterns,
        parameters: &[],
        session: &mut session,
        environment,
        result: DeclarationResult::Known(lowerer.boolean),
        expected_result: None,
        forced_hint: None,
    }) else {
        panic!("an unknown receiver label must remain a failed input");
    };
    assert_eq!(failure.arguments.len(), 2);
    let first = failure.arguments[0].as_ref().expect("checked first input");
    assert_eq!(first.ty, lowerer.boolean);
    assert_eq!(first.span, Span::new(0, 4));
    assert!(failure.arguments[1].is_none());
    assert!(matches!(
        failure.kind,
        ArgumentInferenceFailureKind::Expression(ref failure)
            if failure.source_index == 1 && failure.reason.contains("missing")
    ));
}
