use super::*;

// --- positive: struct / enum methods, `this`, bare member access ---

#[test]
fn struct_methods_and_bare_field_access() {
    let file = file(vec![
        struct_decl_methods(
            "S",
            vec![("v", ty_named("Int"))],
            vec![method_expr("get", vec![], Some(ty_named("Int")), var("v"))],
        ),
        fun_expr(
            "use_it",
            vec![],
            vec![("s", ty_named("S"))],
            Some(ty_named("Int")),
            method_call(var("s"), "get", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("struct methods must lower");

    // `get` is `S.get` with `this: S` as parameter 0; the bare `v` in
    // its body is `this.v`.
    let get = find_fn(&module, "S.get");
    let function = &module.functions[get];
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.params[0].name, "this");
    let struct_id = module
        .structs
        .iter()
        .find(|(_, s)| s.name == "S")
        .map(|(id, _)| id)
        .unwrap();
    match &returned(body_of(&module, "S.get")).kind {
        hir::ExprKind::FieldAccess { receiver, field } => {
            let expected = hir::AppliedStructFieldRef::checked(
                &module.structs,
                &module.struct_applications,
                module.structs[struct_id].self_application,
                0,
            )
            .expect("S.v is a checked applied struct field");
            assert_eq!(
                *field,
                hir::FieldRef::StructField {
                    owner: module.struct_applications[expected.application()].canonical_type,
                    field: module.field_identities[expected].id(),
                }
            );
            assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
        }
        other => panic!("expected `this.v`, found {other:?}"),
    }

    match &returned(body_of(&module, "use_it")).kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(module.callable_function(*callee), get);
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn enum_methods_resolve_and_this_is_the_value() {
    let file = file(vec![
        enum_decl_methods(
            "Color",
            vec![],
            vec![variant_unit("Red"), variant_unit("Blue")],
            vec![method_expr(
                "code",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            )],
        ),
        fun_expr(
            "f",
            vec![],
            vec![("c", ty_named("Color"))],
            Some(ty_named("Int")),
            method_call(var("c"), "code", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("enum methods must lower");
    let code = &module.functions[find_fn(&module, "Color.code")];
    assert!(matches!(
        module.types[code.method.expect("a method").owner],
        hir::Type::Enum(..)
    ));
    match &returned(body_of(&module, "f")).kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Color.code"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn bare_method_calls_inside_a_class_mean_this() {
    let file = file(vec![
        describable(),
        shape(),
        class_decl(
            Final,
            "Loud",
            vec![],
            Some(("Shape", vec![str_lit("loud")])),
            vec![],
            vec![method(
                "shout",
                vec![],
                Some(ty_named("String")),
                vec![ret(Some(call("describe", vec![])))],
            )],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("bare method calls must lower");
    match &returned(body_of(&module, "Loud.shout")).kind {
        hir::ExprKind::MethodCall {
            receiver, callee, ..
        } => {
            assert_eq!(
                module.functions[module.callable_function(*callee)].name,
                "Shape.describe"
            );
            assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
        }
        other => panic!("expected `this.describe()`, found {other:?}"),
    }
}
