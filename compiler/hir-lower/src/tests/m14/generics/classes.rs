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
    let concrete_origin = &output.export.nominal_identities[export_box_id];
    assert!(instances.iter().all(|instance| {
        &instance.origin == concrete_origin
            && instance.type_arguments.len() == 1
            && instance.declared_fields().len() == 1
            && instance.declared_fields()[0].ty == instance.type_arguments[0]
            && instance.methods.len() == 1
    }));
    assert!(instances.iter().any(|instance| {
        matches!(
            output.local.types[instance.type_arguments[0]].kind,
            hir::concrete::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
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
        error.message.contains("class Bounded<T : Marker>()")
            && error
                .message
                .contains("type argument `Int` for `T` must satisfy interface upper bound `Marker`")
    }));

    let errors = lower_user(file(vec![
        generic_class("Empty", vec![type_param("T")], Vec::new(), Vec::new()),
        fun("main", vec![stmt(call("Empty", Vec::new()))]),
    ]))
    .expect_err("an unconstrained constructor cannot invent a type argument");
    assert!(errors.iter().any(|error| {
        error.message.contains("class Empty<T>()")
            && error
                .message
                .contains("cannot infer a unique type argument for `T`")
    }));
}

#[test]
fn nominal_constructors_infer_the_unique_common_supertype() {
    let base = class_decl(
        ast::ClassModifier::Open,
        "Base",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let left = class_decl(
        ast::ClassModifier::Final,
        "Left",
        Vec::new(),
        Some(("Base", Vec::new())),
        Vec::new(),
        Vec::new(),
    );
    let right = class_decl(
        ast::ClassModifier::Final,
        "Right",
        Vec::new(),
        Some(("Base", Vec::new())),
        Vec::new(),
        Vec::new(),
    );
    let pair_class = generic_class(
        "PairClass",
        vec![type_param("T")],
        vec![
            (false, "left", ty_named("T")),
            (false, "right", ty_named("T")),
        ],
        Vec::new(),
    );
    let pair_struct = generic_struct_decl(
        "PairStruct",
        vec!["T"],
        vec![("left", ty_named("T")), ("right", ty_named("T"))],
    );
    let pair_enum = enum_decl(
        "PairEnum",
        vec!["T"],
        vec![variant_positional(
            "Both",
            vec![ty_named("T"), ty_named("T")],
        )],
    );
    let output = lower_user(file(vec![
        base,
        left,
        right,
        pair_class,
        pair_struct,
        pair_enum,
        fun(
            "main",
            vec![
                val(
                    "classPair",
                    call(
                        "PairClass",
                        vec![call("Left", Vec::new()), call("Right", Vec::new())],
                    ),
                ),
                val(
                    "structPair",
                    struct_init(
                        "PairStruct",
                        vec![call("Left", Vec::new()), call("Right", Vec::new())],
                    ),
                ),
                val(
                    "enumPair",
                    method_call(
                        var("PairEnum"),
                        "Both",
                        vec![call("Left", Vec::new()), call("Right", Vec::new())],
                    ),
                ),
            ],
        ),
    ]))
    .expect("all nominal constructors must use subtype constraints");

    let hir::FunctionKind::User(main) = &output.functions[output.entry()].kind else {
        panic!("main is a user function")
    };
    let local_type = |name: &str| {
        let local = main
            .locals
            .iter()
            .map(|(_, local)| local)
            .find(|local| local.name == name)
            .unwrap_or_else(|| panic!("missing local `{name}`"));
        hir::type_name(&output, local.ty)
    };
    assert_eq!(local_type("classPair"), "PairClass<Base>");
    assert_eq!(local_type("structPair"), "PairStruct<Base>");
    assert_eq!(local_type("enumPair"), "PairEnum<Base>");
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
    derived_class.supertypes = vec![constructor_supertype(
        ty_generic("Base", vec![ty_named("T")]),
        call_arguments(vec![var("item")]),
    )];

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

    let (derived_id, derived) = output
        .local
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name.starts_with("Derived$"))
        .expect("Derived<Int> specialization");
    let base = derived.base_class().expect("typed concrete base");
    assert!(output.local.classes[base].name.starts_with("Base$"));
    let constructor = output
        .local
        .class_constructors
        .iter()
        .find_map(|(_, constructor)| (constructor.class == derived_id).then_some(constructor))
        .expect("derived initializer");
    let arguments = constructor
        .body()
        .statements
        .iter()
        .find_map(|statement| {
            let hir::concrete::StatementKind::Expr(hir::concrete::Expr {
                kind: hir::concrete::ExprKind::ClassInitializerCall { args, .. },
                ..
            }) = &statement.kind
            else {
                return None;
            };
            Some(args)
        })
        .expect("derived initializer calls its base initializer");
    assert!(matches!(
        arguments.as_slice(),
        [hir::concrete::Expr {
            kind: hir::concrete::ExprKind::Local(_),
            ..
        }]
    ));
    assert!(
        constructor
            .body()
            .statements
            .iter()
            .any(|statement| matches!(
                statement.kind,
                hir::concrete::StatementKind::ValDecl {
                    init: hir::concrete::Expr {
                        kind: hir::concrete::ExprKind::ConstructorParam(_),
                        ..
                    },
                    ..
                }
            ))
    );
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
    derived_class.supertypes = vec![constructor_supertype(
        ty_generic("Base", vec![ty_generic("Wrapper", vec![ty_named("T")])]),
        call_arguments(vec![call("Wrapper", vec![var("item")])]),
    )];

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
    let base = derived.base_class().expect("specialized base");
    let base_argument = output.local.classes[base].type_arguments[0];
    let hir::concrete::TypeKind::Struct(wrapper) = output.local.types[base_argument].kind else {
        panic!("Base argument must be the concrete Wrapper<Int> identity")
    };
    assert!(output.local.structs[wrapper].name.starts_with("Wrapper$"));
    assert!(matches!(
        output.local.types[output.local.structs[wrapper].type_arguments[0]].kind,
        hir::concrete::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
    ));
}
