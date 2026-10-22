use super::*;

#[test]
fn control_flow_preserves_guards_empty_branches_loop_targets_and_origins() {
    let condition = expression(
        hir::DefaultExpressionKindV1::BooleanLiteral(hir::CanonicalBooleanV1::True),
        core_type("Boolean"),
    );
    let guard = hir::DefaultWhenGuardV1::try_new(
        vec![statement(hir::DefaultStatementKindV1::Expr(Box::new(
            unit(),
        )))],
        condition.clone(),
    )
    .unwrap();
    let arm = hir::DefaultWhenArmV1::try_new(
        hir::DefaultWhenConditionV1::Predicate(Box::new(guard)),
        hir::OptionalDefaultWhenGuardV1::absent(),
        vec![statement(hir::DefaultStatementKindV1::Break(
            hir::DefaultLoopTargetV1::new(1),
        ))],
        origin(),
    )
    .unwrap();
    let when = hir::DefaultWhenV1::try_conditions(
        vec![arm],
        hir::DefaultWhenFallbackV1::try_else(vec![]).unwrap(),
    )
    .unwrap();
    let when = serde_json::to_value(BodyJson::new(&when)).unwrap();
    assert_eq!(when["subject"], Value::Null);
    assert_eq!(when["arms"][0]["guard"], Value::Null);
    assert_eq!(
        when["arms"][0]["condition"]["guard"]["setup"][0]["operation"]["kind"],
        "expr"
    );
    assert_eq!(
        when["arms"][0]["body"][0]["operation"],
        json!({"kind": "break", "target": 1})
    );
    assert_eq!(when["fallback"], json!({"kind": "else", "body": []}));
    assert_eq!(when["arms"][0]["origin"], json!(QueryJson(&origin())));

    let loop_body = hir::DefaultStatementKindV1::While {
        loop_kind: hir::LoopKind::DoWhile,
        condition_setup: vec![statement(hir::DefaultStatementKindV1::Expr(Box::new(
            unit(),
        )))],
        condition: Box::new(condition),
        body: vec![statement(hir::DefaultStatementKindV1::Continue(
            hir::DefaultLoopTargetV1::INNERMOST,
        ))],
    };
    let loop_body = serde_json::to_value(BodyJson::new(&loop_body)).unwrap();
    assert_eq!(loop_body["loopKind"], "doWhile");
    assert_eq!(loop_body["conditionSetup"].as_array().unwrap().len(), 1);
    assert_eq!(
        loop_body["body"][0]["operation"],
        json!({"kind": "continue", "target": 0})
    );

    let absent = hir::DefaultTryV1::try_new(
        vec![],
        vec![],
        hir::OptionalDefaultStatementListV1::absent(),
    )
    .unwrap();
    let empty = hir::DefaultTryV1::try_new(
        vec![],
        vec![],
        hir::OptionalDefaultStatementListV1::try_present(vec![]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(BodyJson::new(&absent)).unwrap()["finallyBody"],
        Value::Null
    );
    assert_eq!(
        serde_json::to_value(BodyJson::new(&empty)).unwrap()["finallyBody"],
        json!([])
    );
}
