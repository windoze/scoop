use super::*;

#[test]
fn do_condition_sees_only_definitions_on_every_incoming_path() {
    let fixture = Fixture::new();
    let local = local_selector(0);
    for skip in [false, true] {
        let mut body = Vec::new();
        if skip {
            body.push(statement(
                &fixture,
                DefaultStatementKindV1::If {
                    condition: Box::new(unit(&fixture)),
                    then_body: vec![statement(
                        &fixture,
                        DefaultStatementKindV1::Continue(crate::DefaultLoopTargetV1::INNERMOST),
                    )],
                    else_body: OptionalDefaultStatementListV1::absent(),
                },
            ));
        }
        body.push(statement(
            &fixture,
            DefaultStatementKindV1::ValDecl {
                pattern: DefaultPatternV1::binding(local.clone()),
                init: Box::new(unit(&fixture)),
            },
        ));
        let value = template(
            &fixture,
            vec![source_record(
                &fixture,
                local.clone(),
                CanonicalBooleanV1::False,
            )],
            vec![],
            vec![statement(
                &fixture,
                DefaultStatementKindV1::While {
                    loop_kind: crate::LoopKind::DoWhile,
                    condition_setup: vec![],
                    condition: Box::new(local_expression(&fixture, local.clone(), binder())),
                    body,
                },
            )],
            unit(&fixture),
        );
        if skip {
            assert!(matches!(
                validate(&value),
                Err(ExportDefaultLocalDataFlowValidationError::Local {
                    error: DefaultLocalDataFlowLocalError::UseBeforeDefinition,
                    ..
                })
            ));
        } else {
            assert_eq!(validate(&value), Ok(()));
        }
    }
}

#[test]
fn do_body_return_cannot_reach_the_condition() {
    let fixture = Fixture::new();
    let local = local_selector(0);
    let value = template(
        &fixture,
        vec![source_record(
            &fixture,
            local.clone(),
            CanonicalBooleanV1::False,
        )],
        vec![],
        vec![statement(
            &fixture,
            DefaultStatementKindV1::While {
                loop_kind: crate::LoopKind::DoWhile,
                condition_setup: vec![],
                condition: Box::new(local_expression(&fixture, local, binder())),
                body: vec![statement(
                    &fixture,
                    DefaultStatementKindV1::Return(Box::new(unit(&fixture))),
                )],
            },
        )],
        unit(&fixture),
    );
    assert_eq!(validate(&value), Ok(()));
}
