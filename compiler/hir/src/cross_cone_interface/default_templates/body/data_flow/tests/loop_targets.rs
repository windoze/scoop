use super::*;

#[test]
fn an_inner_loop_does_not_consume_a_jump_to_an_outer_loop() {
    let fixture = Fixture::new();
    let local = local_selector(0);
    for jump in [
        DefaultStatementKindV1::Break as fn(_) -> _,
        DefaultStatementKindV1::Continue,
    ] {
        let inner = statement(
            &fixture,
            DefaultStatementKindV1::While {
                loop_kind: crate::LoopKind::While,
                condition_setup: vec![statement(
                    &fixture,
                    jump(crate::DefaultLoopTargetV1::new(1)),
                )],
                condition: Box::new(unit(&fixture)),
                body: vec![],
            },
        );
        let outer = statement(
            &fixture,
            DefaultStatementKindV1::While {
                loop_kind: crate::LoopKind::While,
                condition_setup: vec![],
                condition: Box::new(unit(&fixture)),
                body: vec![
                    inner,
                    statement(
                        &fixture,
                        DefaultStatementKindV1::Expr(Box::new(local_expression(
                            &fixture,
                            local.clone(),
                            binder(),
                        ))),
                    ),
                ],
            },
        );
        let body = template(
            &fixture,
            vec![source_record(
                &fixture,
                local.clone(),
                CanonicalBooleanV1::False,
            )],
            vec![],
            vec![outer],
            unit(&fixture),
        );
        assert_eq!(validate(&body), Ok(()));
    }
}

#[test]
fn a_loop_distance_must_name_an_enclosing_loop_even_when_unreachable() {
    let fixture = Fixture::new();
    let body = template(
        &fixture,
        vec![],
        vec![],
        vec![statement(
            &fixture,
            DefaultStatementKindV1::While {
                loop_kind: crate::LoopKind::While,
                condition_setup: vec![],
                condition: Box::new(unit(&fixture)),
                body: vec![
                    statement(
                        &fixture,
                        DefaultStatementKindV1::Return(Box::new(unit(&fixture))),
                    ),
                    statement(
                        &fixture,
                        DefaultStatementKindV1::Continue(crate::DefaultLoopTargetV1::new(1)),
                    ),
                ],
            },
        )],
        unit(&fixture),
    );
    assert_eq!(
        validate(&body),
        Err(
            ExportDefaultLocalDataFlowValidationError::LoopControlOutsideLoop {
                control: DefaultLoopControlV1::Continue,
            }
        )
    );
}
