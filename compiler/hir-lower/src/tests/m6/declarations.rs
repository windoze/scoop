use super::*;

// --- positive: golden dump ---

#[test]
fn class_hierarchy_golden() {
    let file = file(vec![describable(), shape(), point(), fun("main", vec![])]);
    let module = lower_user(file).expect("the hierarchy must lower");
    let expected = include_str!("snapshots/class_hierarchy_golden.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

// --- positive: declaration structure ---

#[test]
fn class_and_interface_structure() {
    let file = file(vec![describable(), shape(), point(), fun("main", vec![])]);
    let module = lower_user(file).expect("the hierarchy must lower");

    let shape_id = class_id(&module, "Shape");
    let point_id = class_id(&module, "Point");
    let describable_id = interface_id(&module, "Describable");

    let shape = &module.classes[shape_id];
    assert_eq!(shape.modifier, hir::ClassModifier::Open);
    assert_eq!(
        module.class_constructors[shape.constructors[0]]
            .parameters
            .len(),
        1
    );
    assert_eq!(shape.interfaces, vec![interface_ty(&module, "Describable")]);

    // Base-class clause with the lowered delegation arguments.
    let point = &module.classes[point_id];
    let base = point.base_class.as_ref().expect("Point has a base");
    assert!(matches!(module.types[*base], hir::Type::Class(application)
        if module.class_applications[application].template == module.nominal_identities[shape_id].declaration_id()
            && module.class_applications[application].arguments.is_empty()));
    let hir::ClassConstructorKind::Primary {
        base:
            hir::BaseInitialization::Super {
                arguments: delegation,
                ..
            },
        ..
    } = &module.class_constructors[point.constructors[0]].kind
    else {
        panic!("Point primary constructor delegates to Shape")
    };
    assert_eq!(delegation.args.len(), 1);
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::StringLiteral { .. },
                ..
            },
            ..
        }
    )));

    // Methods carry owner/modality metadata and `this` as parameter 0.
    let describe = &module.functions[find_fn(&module, "Shape.describe")];
    assert_eq!(
        describe.method.map(|method| method.owner),
        Some(module.class_applications[class_application(&module, shape_id)].canonical_type)
    );
    assert_eq!(describe.params[0].name, "this");
    assert_eq!(describe.params.len(), 1); // only `this`

    let iface_method = &module.functions[find_fn(&module, "Describable.describe")];
    assert!(matches!(
        module.types[iface_method.method.expect("a method").owner],
        hir::Type::Interface(application)
            if module.interface_applications[application].template == module.nominal_identities[describable_id].declaration_id()
                && module.interface_applications[application].arguments.is_empty()
    ));

    // The interface method identity points directly at its complete function.
    let member = module.interfaces[describable_id].methods[0];
    let sig = &module.functions[module.interface_methods[member].function];
    assert_eq!(sig.name.rsplit('.').next(), Some("describe"));
    assert_eq!(sig.params.len(), 1); // only `this`
    assert_eq!(module.types[sig.return_ty], hir::Type::String);
}

// --- positive: method resolution and field layout ---

#[test]
fn method_calls_resolve_against_the_receiver_type() {
    let file = file(vec![
        describable(),
        shape(),
        point(),
        fun_expr(
            "show",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("String")),
            method_call(var("s"), "describe", vec![]),
        ),
        fun_expr(
            "show2",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_named("String")),
            method_call(var("d"), "describe", vec![]),
        ),
        // Inherited method: `moveTo` is declared on Point, `describe`
        // is inherited from Shape.
        fun_expr(
            "show3",
            vec![],
            vec![("p", ty_named("Point"))],
            Some(ty_named("String")),
            method_call(var("p"), "describe", vec![]),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("method calls must lower");

    let show = returned(body_of(&module, "show"));
    match &show.kind {
        hir::ExprKind::MethodCall { callee, args, .. } => {
            assert_eq!(
                module.functions[module
                    .callable_function(crate::tests::local_method_callable(&module, *callee))]
                .name,
                "Shape.describe"
            );
            assert!(args.is_empty());
        }
        other => panic!("expected a method call, found {other:?}"),
    }
    let show2 = returned(body_of(&module, "show2"));
    match &show2.kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module
                    .callable_function(crate::tests::local_method_callable(&module, *callee))]
                .name,
                "Describable.describe"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
    // `describe` on a Point receiver resolves to the inherited Shape method.
    let show3 = returned(body_of(&module, "show3"));
    match &show3.kind {
        hir::ExprKind::MethodCall { callee, .. } => {
            assert_eq!(
                module.functions[module
                    .callable_function(crate::tests::local_method_callable(&module, *callee))]
                .name,
                "Shape.describe"
            );
        }
        other => panic!("expected a method call, found {other:?}"),
    }
}

#[test]
fn class_field_layout_is_base_prefix_then_own() {
    let file = file(vec![
        describable(),
        shape(),
        point(),
        fun_expr(
            "get_y",
            vec![],
            vec![("p", ty_named("Point"))],
            Some(ty_named("Int")),
            field(var("p"), "y"),
        ),
        fun_expr(
            "get_name",
            vec![],
            vec![("p", ty_named("Point"))],
            Some(ty_named("String")),
            field(var("p"), "name"),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("field reads must lower");
    let point_id = class_id(&module, "Point");
    let shape_id = class_id(&module, "Shape");

    let y_field = module.classes[point_id].fields[1];
    let name_field = module.classes[shape_id].fields[0];
    // Export HIR preserves declaration identities; concrete layout assigns
    // the inherited prefix later.
    match &returned(body_of(&module, "get_y")).kind {
        hir::ExprKind::FieldAccess { field, .. } => assert_eq!(
            *field,
            hir::FieldRef::ClassField {
                owner: module.class_applications[class_application(&module, point_id)]
                    .canonical_type,
                field: module.field_identities[y_field].id()
            }
        ),
        other => panic!("expected a field access, found {other:?}"),
    }
    match &returned(body_of(&module, "get_name")).kind {
        hir::ExprKind::FieldAccess { field, .. } => assert_eq!(
            *field,
            hir::FieldRef::ClassField {
                owner: module.class_applications[class_application(&module, shape_id)]
                    .canonical_type,
                field: module.field_identities[name_field].id()
            }
        ),
        other => panic!("expected a field access, found {other:?}"),
    }
}
