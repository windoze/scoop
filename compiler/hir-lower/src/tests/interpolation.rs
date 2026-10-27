use super::*;

#[test]
fn short_interpolation_snapshots_bound_conversions_before_concat() {
    // The test core intentionally has no StringBuilder declaration.
    let user = scoop_parser::parse(
        r#"
        fun <T: ToString> render(value: T): String = f"<${value}:${42}>"
        fun main() { println(render(7)) }
        "#,
    )
    .unwrap();
    let module = lower(&[complete_core_file(), user]).unwrap().export;
    let render = function(&module, "render");
    let body = body_of(&module, render);
    let conversions = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init,
            } = &statement.kind
            else {
                return None;
            };
            let hir::ExprKind::MethodCall {
                receiver,
                callee: hir::MethodCallee::Bound(callee),
                args,
            } = &init.kind
            else {
                return None;
            };
            assert!(args.is_empty());
            let bound = &module.bound_callable_refs[*callee];
            assert_eq!(bound.receiver_type, receiver.ty);
            assert!(!matches!(
                module.types[receiver.ty],
                hir::Type::Interface(_)
            ));
            let hir::BoundCallableSource::Interface {
                member: hir::InterfaceMethodReference::Local(member),
                ..
            } = bound.source
            else {
                panic!("the conversion must retain the actual core slot");
            };
            assert_eq!(
                module.interfaces[module.interface_methods[member].owner].name,
                "ToString"
            );
            Some(*local)
        })
        .collect::<Vec<_>>();
    assert_eq!(conversions.len(), 2);
    let result = return_value(&body.statements);
    fn pieces(expr: &hir::Expr, output: &mut Vec<hir::LocalId>) -> usize {
        match &expr.kind {
            hir::ExprKind::PrimitiveBinary {
                kind: hir::PrimitiveBinaryKind::StringConcat,
                lhs,
                rhs,
            } => pieces(lhs, output) + pieces(rhs, output),
            hir::ExprKind::Local(local) => {
                output.push(*local);
                1
            }
            hir::ExprKind::StringLiteral { .. } => 1,
            _ => panic!("concat must only read captured strings or literals"),
        }
    }
    let mut reads = Vec::new();
    assert_eq!(pieces(result, &mut reads), 5);
    let snapshots = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local },
                init:
                    hir::Expr {
                        kind: hir::ExprKind::Local(source),
                        ..
                    },
            } = &statement.kind
            else {
                return None;
            };
            Some((*local, *source))
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(
        reads
            .iter()
            .map(|local| snapshots[local])
            .collect::<Vec<_>>(),
        conversions
    );
}

#[test]
fn empty_and_single_interpolation_need_no_builder_or_concat() {
    let user = scoop_parser::parse(
        r#"
        fun empty(): String = f""
        fun literal(): String = f"雪"
        fun single(value: Int): String = f"${value}"
        fun main() {}
    "#,
    )
    .unwrap();
    let module = lower(&[complete_core_file(), user]).unwrap().export;
    for name in ["empty", "literal"] {
        let function = function(&module, name);
        let body = body_of(&module, function);
        assert!(matches!(
            return_value(&body.statements).kind,
            hir::ExprKind::StringLiteral { .. }
        ));
    }
    let function = function(&module, "single");
    let body = body_of(&module, function);
    assert!(matches!(
        return_value(&body.statements).kind,
        hir::ExprKind::Local(_)
    ));
}

fn function(module: &hir::Module, name: &str) -> hir::FunctionId {
    *module
        .top_level
        .iter()
        .find(|id| module.functions[**id].name == name)
        .unwrap()
}

fn body_of(module: &hir::Module, function: hir::FunctionId) -> &hir::Body {
    let hir::FunctionKind::User(body) = &module.functions[function].kind else {
        panic!("expected an ordinary source body");
    };
    body
}
