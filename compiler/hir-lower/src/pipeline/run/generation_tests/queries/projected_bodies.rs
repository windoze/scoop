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
