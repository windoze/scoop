use super::*;
use std::sync::Arc;

#[test]
fn projections_keep_control_flow_captures_and_ignore_failed_type_queries() {
    let mut observed = None;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let context = SemanticContext::Function(function);
        assert!(
            queries
                .resolve_type(
                    context,
                    &scoop_parser::parse_type("Storage<String>").unwrap()
                )
                .is_err()
        );
        let Some(Availability::Available(body)) = queries
            .projected_body(SemanticCallable::Current(function))
            .unwrap()
        else {
            panic!("ordinary checked function")
        };
        let Some(Availability::Available(repeated)) = queries
            .projected_body(SemanticCallable::Current(function))
            .unwrap()
        else {
            panic!("cached body")
        };
        assert!(Arc::ptr_eq(&body, &repeated));
        assert_eq!(body.parameters().len(), 2);
        assert!(body.statements().iter().any(|statement| matches!(
            statement.kind(),
            hir::DefaultStatementKindV1::While { .. }
        )));
        assert!(body.statements().iter().any(|statement| matches!(statement.kind(), hir::DefaultStatementKindV1::ValDecl { init, .. } if matches!(init.kind(), hir::DefaultExpressionKindV1::Lambda(_)))));
        let hir::DefaultStatementKindV1::Return(result) = body.statements().last().unwrap().kind()
        else {
            panic!("typed return");
        };
        assert_eq!(result.result_type(), body.result());
        let json =
            serde_json::to_value(scoop_generator_protocol::BodyJson::new(body.as_ref())).unwrap();
        assert_eq!(
            json["owner"],
            serde_json::json!(scoop_generator_protocol::CallableDeclaration::from(
                body.owner()
            ))
        );
        assert_eq!(
            json["result"],
            serde_json::json!(scoop_generator_protocol::QueryJson(body.result()))
        );
        let statements = json["statements"].as_array().unwrap();
        let loop_body = statements
            .iter()
            .find(|statement| statement["operation"]["kind"] == "while")
            .unwrap();
        assert_eq!(loop_body["operation"]["loopKind"], "while");
        let closure = statements
            .iter()
            .find_map(|statement| {
                let init = &statement["operation"]["init"]["operation"];
                (init["kind"] == "lambda").then_some(&init["callable"])
            })
            .unwrap();
        let capture = &closure["captures"][0];
        assert_eq!(capture["source"]["kind"], "local");
        assert!(
            json["locals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|local| local["selector"] == capture["source"]["local"])
        );
        assert_eq!(
            capture["firstUseOrigin"]["source"]["path"],
            "src/user.scoop"
        );
        assert_eq!(
            serde_json::to_value(scoop_generator_protocol::BodyJson::new(repeated.as_ref()))
                .unwrap(),
            json
        );
        observed = Some((function, body.owner()));
        Ok(Vec::new())
    });
    let module = compile(
        "annotation class Mark\n@NoGC struct Storage<T>(val value: T)\n@Mark fun workflow(first: Int, second: Int = first): Int { var result = second; while (result < first) { result += 1 }; val captured = result; val next = { captured + 1 }; return next() }",
        &mut driver,
    ).unwrap();
    let (function, owner) = observed.unwrap();
    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(identity)) =
        &module.function_identities[function]
    else {
        panic!("same final source identity");
    };
    assert_eq!(
        owner,
        hir::DefaultCallableDeclarationV1::Function(identity.id())
    );
}
