use super::*;

#[test]
fn generic_class_constructor_members_and_concrete_instances_are_complete() {
    let output = lower_user_output(file(vec![
        generic_class(
            "Box",
            vec![type_param("T")],
            vec![(false, "value", ty_named("T"))],
            vec![method_expr(
                "get",
                Vec::new(),
                Some(ty_named("T")),
                field(this_expr(), "value"),
            )],
        ),
        fun(
            "main",
            vec![
                val("ints", call("Box", vec![int_lit(42)])),
                val(
                    "strings",
                    typed_call("Box", vec![ty_named("String")], vec![str_lit("ok")]),
                ),
                stmt(method_call(var("ints"), "get", Vec::new())),
                val("text", field(var("strings"), "value")),
            ],
        ),
    ]))
    .expect("generic class construction and member substitution must lower");

    let (export_box_id, export_box) = output
        .export
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "Box")
        .expect("Box declaration");
    assert_eq!(export_box.type_params.len(), 1);
    assert_eq!(export_box.methods.len(), 1);
    let applications = output
        .export
        .class_applications
        .iter()
        .filter(|(_, application)| application.template == export_box_id)
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 3, "Box<T>, Box<Int>, Box<String>");
    assert!(applications.iter().all(|(id, application)| {
        matches!(output.export.types[application.canonical_type], hir::Type::Class(found) if found == *id)
    }));
    assert_ne!(applications[1].0, applications[2].0);

    let mut instances = output
        .local
        .classes
        .iter()
        .filter(|(_, declaration)| declaration.name.starts_with("Box$"))
        .map(|(_, declaration)| declaration)
        .collect::<Vec<_>>();
    instances.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(instances.len(), 2);
    let concrete_origin =
        hir::concrete::ClassOriginId::from_raw(export_box_id.into_raw().into_u32());
    assert!(instances.iter().all(|instance| {
        instance.origin == concrete_origin
            && instance.type_arguments.len() == 1
            && instance.declared_constructor().len() == 1
            && instance.declared_constructor()[0].ty == instance.type_arguments[0]
            && instance.methods.len() == 1
    }));
    assert!(instances.iter().any(|instance| {
        matches!(
            output.local.types[instance.type_arguments[0]].kind,
            hir::concrete::TypeKind::Int
        )
    }));
    assert!(instances.iter().any(|instance| {
        matches!(
            output.local.types[instance.type_arguments[0]].kind,
            hir::concrete::TypeKind::String
        )
    }));
}

#[test]
fn generic_class_constructor_requires_complete_unique_arguments_and_bounds() {
    let mut marker_bound = type_param("T");
    marker_bound.inline_bound = Some(ast::TypeBound::Upper(ty_named("Marker")));
    let declarations = vec![
        interface_decl("Marker", Vec::new()),
        generic_class("Bounded", vec![marker_bound], Vec::new(), Vec::new()),
        fun(
            "main",
            vec![stmt(typed_call(
                "Bounded",
                vec![ty_named("Int")],
                Vec::new(),
            ))],
        ),
    ];
    let errors = lower_user(file(declarations)).expect_err("Int cannot satisfy Marker");
    assert!(errors.iter().any(|error| {
        error.message
            == "type argument `Int` for `T` of class `Bounded` must satisfy interface upper bound `Marker`"
    }));

    let errors = lower_user(file(vec![
        generic_class("Empty", vec![type_param("T")], Vec::new(), Vec::new()),
        fun("main", vec![stmt(call("Empty", Vec::new()))]),
    ]))
    .expect_err("an unconstrained constructor cannot invent a type argument");
    assert!(
        errors
            .iter()
            .any(|error| { error.message == "cannot infer type argument `T` for class `Empty`" })
    );
}

#[test]
fn generic_class_base_application_and_delegation_keep_typed_sources() {
    let mut base = generic_class(
        "Base",
        vec![type_param("T")],
        vec![(false, "value", ty_named("T"))],
        vec![method_expr(
            "get",
            Vec::new(),
            Some(ty_named("T")),
            field(this_expr(), "value"),
        )],
    );
    let Decl::Class(base_class) = &mut base else {
        unreachable!()
    };
    base_class.modifier = ast::ClassModifier::Open;

    let mut derived = generic_class(
        "Derived",
        vec![type_param("T")],
        vec![(false, "item", ty_named("T"))],
        Vec::new(),
    );
    let Decl::Class(derived_class) = &mut derived else {
        unreachable!()
    };
    derived_class.base_class = Some((ty_generic("Base", vec![ty_named("T")]), vec![var("item")]));

    let output = lower_user_output(file(vec![
        base,
        derived,
        fun(
            "main",
            vec![
                val("derived", call("Derived", vec![int_lit(7)])),
                val("result", method_call(var("derived"), "get", Vec::new())),
            ],
        ),
    ]))
    .expect("generic base application and constructor delegation must lower");

    let derived = output
        .local
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name.starts_with("Derived$"))
        .expect("Derived<Int> specialization")
        .1;
    let (base, arguments) = derived.base_class().expect("typed concrete base");
    assert!(output.local.classes[*base].name.starts_with("Base$"));
    assert!(matches!(
        arguments.as_slice(),
        [hir::concrete::Expr {
            kind: hir::concrete::ExprKind::ConstructorParam(parameter),
            ..
        }] if parameter.into_raw() == 0
    ));
}

#[test]
fn generic_base_substitution_preserves_nested_application_identity() {
    let wrapper = generic_struct_decl("Wrapper", vec!["T"], vec![("value", ty_named("T"))]);
    let mut base = generic_class(
        "Base",
        vec![type_param("T")],
        vec![(false, "value", ty_named("T"))],
        vec![method_expr(
            "get",
            Vec::new(),
            Some(ty_named("T")),
            field(this_expr(), "value"),
        )],
    );
    let Decl::Class(base_class) = &mut base else {
        unreachable!()
    };
    base_class.modifier = ast::ClassModifier::Open;

    let mut derived = generic_class(
        "Derived",
        vec![type_param("T")],
        vec![(false, "item", ty_named("T"))],
        Vec::new(),
    );
    let Decl::Class(derived_class) = &mut derived else {
        unreachable!()
    };
    derived_class.base_class = Some((
        ty_generic("Base", vec![ty_generic("Wrapper", vec![ty_named("T")])]),
        vec![call("Wrapper", vec![var("item")])],
    ));

    let output = lower_user_output(file(vec![
        wrapper,
        base,
        derived,
        fun(
            "main",
            vec![
                val("derived", call("Derived", vec![int_lit(7)])),
                val("wrapped", method_call(var("derived"), "get", Vec::new())),
                val("result", field(var("wrapped"), "value")),
            ],
        ),
    ]))
    .expect("nested generic base applications must substitute without reconstruction");

    let derived = output
        .local
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name.starts_with("Derived$"))
        .expect("Derived<Int> specialization")
        .1;
    let (base, _) = derived.base_class().expect("specialized base");
    let base_argument = output.local.classes[*base].type_arguments[0];
    let hir::concrete::TypeKind::Struct(wrapper) = output.local.types[base_argument].kind else {
        panic!("Base argument must be the concrete Wrapper<Int> identity")
    };
    assert!(output.local.structs[wrapper].name.starts_with("Wrapper$"));
    assert!(matches!(
        output.local.types[output.local.structs[wrapper].type_arguments[0]].kind,
        hir::concrete::TypeKind::Int
    ));
}
