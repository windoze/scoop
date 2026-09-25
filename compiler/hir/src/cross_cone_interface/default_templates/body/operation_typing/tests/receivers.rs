use super::*;
use crate::SourceCallReceiver;

#[test]
fn default_source_receiver_is_checked_before_the_adapted_logical_arguments() {
    let fixture = Fixture::new();
    let original = ty(40);
    let adapted = ty(41);
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    let local = fixture.local();
    let value = expression(
        &fixture,
        DefaultExpressionKindV1::Call {
            callee: fixture.callable(),
            arguments: vec![expression(
                &fixture,
                DefaultExpressionKindV1::Local(local.clone()),
                adapted.clone(),
            )],
            receiver: SourceCallReceiver::Receiver {
                static_type: original.clone(),
            },
        },
        unit.clone(),
    );
    let template = template_with_parameter_local(&fixture, local, adapted.clone(), value);
    let mut authority = Authority::new();
    authority.callable = DefaultCallableOperationShapeV1::new(
        Effect::Ordinary,
        Some(adapted.clone()),
        vec![],
        vec![],
        unit,
    );
    assert!(
        matches!(validate(&template, &mut authority), Err(ExportDefaultOperationTypingValidationError::Relation {
        relation: DefaultOperationTypeRelationV1::MemberReceiver, source, target, ..
    }) if *source == original && *target == adapted)
    );
    authority.relation_queries.clear();
    authority
        .relations
        .push(DefaultOperationTypeRelationV1::MemberReceiver);
    assert_eq!(validate(&template, &mut authority), Ok(()));
    assert_eq!(
        authority.relation_queries,
        vec![(
            DefaultOperationTypeRelationV1::MemberReceiver,
            original,
            adapted
        )]
    );
}

#[test]
fn default_source_receiver_presence_matches_the_provider_callable_shape() {
    let fixture = Fixture::new();
    let unit = core(DefaultOperationCoreTypeV1::Unit);
    for expected in [false, true] {
        let value = expression(
            &fixture,
            DefaultExpressionKindV1::Call {
                callee: fixture.callable(),
                arguments: vec![expression(
                    &fixture,
                    DefaultExpressionKindV1::UnitLiteral,
                    unit.clone(),
                )],
                receiver: if expected {
                    SourceCallReceiver::NoReceiver
                } else {
                    SourceCallReceiver::Receiver {
                        static_type: unit.clone(),
                    }
                },
            },
            unit.clone(),
        );
        let template = template(&fixture, value, vec![], vec![], false);
        let mut authority = Authority::new();
        authority.callable = DefaultCallableOperationShapeV1::new(
            Effect::Ordinary,
            expected.then(|| unit.clone()),
            vec![],
            if expected { vec![] } else { vec![unit.clone()] },
            unit.clone(),
        );
        let problem = if expected {
            DefaultOperationTypingProblemV1::MissingCallableReceiver
        } else {
            DefaultOperationTypingProblemV1::UnexpectedCallableReceiver
        };
        assert!(
            matches!(validate(&template, &mut authority), Err(ExportDefaultOperationTypingValidationError::Problem { problem: actual, .. }) if actual == problem)
        );
    }
}
