use super::*;

fn ulong(magnitude: u64) -> Expr {
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Decimal,
        suffix: ast::IntegerSuffix::UnsignedLong,
        span: sp(),
    })
}

fn pointer_requirement_functions() -> Vec<Decl> {
    let g = with_kind(
        fun_sig(
            "g",
            vec!["T"],
            Vec::new(),
            None,
            vec![safety_block(
                ast::SafetyMode::Unsafe,
                vec![val_ty(
                    "pointer",
                    Some(ty_generic("Ptr", vec![ty_named("T")])),
                    typed_call("Ptr", vec![ty_named("T")], vec![ulong(1)]),
                )],
            )],
        ),
        ast::TypeParamKindBound::Value,
    );
    let f = with_kind(
        fun_sig(
            "f",
            vec!["T"],
            Vec::new(),
            None,
            vec![stmt(typed_call("g", vec![ty_named("T")], Vec::new()))],
        ),
        ast::TypeParamKindBound::Value,
    );
    vec![g, f]
}

fn generic_pointer_lambda() -> Expr {
    Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(Vec::new()),
        body: block(vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![val_ty(
                "pointer",
                Some(ty_generic("Ptr", vec![ty_named("T")])),
                typed_call("Ptr", vec![ty_named("T")], vec![ulong(1)]),
            )],
        )]),
        span: sp(),
    }
}

fn generic_pointer_anonymous() -> Expr {
    Expr::AnonymousFunction {
        id: ast::AnonymousFunctionId(0),
        is_suspend: false,
        params: Vec::new(),
        return_ty: Some(ty_named("Unit")),
        body: block(vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![val_ty(
                "pointer",
                Some(ty_generic("Ptr", vec![ty_named("T")])),
                typed_call("Ptr", vec![ty_named("T")], vec![ulong(1)]),
            )],
        )]),
        span: sp(),
    }
}

fn generic_closure_factory(name: &str, body: Expr) -> Decl {
    with_kind(
        fun_expr(
            name,
            vec!["T"],
            Vec::new(),
            Some(ty_function(false, Vec::new(), ty_named("Unit"))),
            body,
        ),
        ast::TypeParamKindBound::Value,
    )
}

fn typed_call_at(name: &str, type_args: Vec<TypeRef>, args: Vec<Expr>, span: Span) -> Expr {
    let mut expression = typed_call(name, type_args, args);
    let Expr::Call(call) = &mut expression else {
        unreachable!("the typed-call builder always produces a call")
    };
    call.callee.span = span;
    call.span = span;
    expression
}

fn generic_function<'module>(
    module: &'module hir::Module,
    name: &str,
) -> (&'module hir::Function, &'module hir::GenericFunction) {
    let function = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("missing generic function {name}"));
    let hir::FunctionGenericity::Generic { definition, .. } = function.genericity else {
        panic!("{name} must remain a generic function")
    };
    (function, &module.generic_functions[definition])
}

fn value_wrapper() -> Decl {
    with_kind(
        generic_struct_decl(
            "Wrapper",
            vec!["T"],
            vec![("pointer", ty_generic("Ptr", vec![ty_named("T")]))],
        ),
        ast::TypeParamKindBound::Value,
    )
}

fn pointer_to_wrapped(name: &str) -> TypeRef {
    ty_generic("Ptr", vec![ty_generic("Wrapper", vec![ty_named(name)])])
}

fn type_alias(name: &str, target: TypeRef) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target,
        span: sp(),
    })
}

fn omitted_optional_global(name: &str, inner: TypeRef) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_nullable(inner),
        body: ast::PropertyBodySyntax::OptionalOmitted,
        span: sp(),
    })
}

fn generic_supertype(name: &str, argument: &str, constructor: bool) -> ast::SupertypeSpec {
    ast::SupertypeSpec {
        ty: ty_generic(name, vec![ty_named(argument)]),
        constructor_arguments: constructor.then(Vec::new),
        span: sp(),
    }
}

fn assert_owner_requires_its_parameter(module: &hir::Module, owner_name: &str) {
    let requirement = module
        .structs
        .iter()
        .find_map(|(_, declaration)| {
            (declaration.name == owner_name).then_some((
                &declaration.type_params,
                &declaration.gc_free_pointee_requirements,
            ))
        })
        .or_else(|| {
            module.enums.iter().find_map(|(_, declaration)| {
                (declaration.name == owner_name).then_some((
                    &declaration.type_params,
                    &declaration.gc_free_pointee_requirements,
                ))
            })
        })
        .or_else(|| {
            module.classes.iter().find_map(|(_, declaration)| {
                (declaration.name == owner_name).then_some((
                    &declaration.type_params,
                    &declaration.gc_free_pointee_requirements,
                ))
            })
        })
        .or_else(|| {
            module.interfaces.iter().find_map(|(_, declaration)| {
                (declaration.name == owner_name).then_some((
                    &declaration.type_params,
                    &declaration.gc_free_pointee_requirements,
                ))
            })
        })
        .unwrap_or_else(|| panic!("missing nominal `{owner_name}`"));
    assert_eq!(requirement.0.len(), 1, "{owner_name}");
    assert_eq!(requirement.1.len(), 1, "{owner_name}");
    assert_eq!(requirement.1[0].type_param, requirement.0[0].id);
}

fn named_pointer_wrapper(name: &str) -> Decl {
    let mut declaration = value_wrapper();
    let Decl::Struct(wrapper) = &mut declaration else {
        unreachable!()
    };
    wrapper.name = ident(name);
    declaration
}

fn closed_wrapper(name: &str) -> TypeRef {
    ty_generic(name, vec![ty_named("ManagedValue")])
}

fn closed_wrapper_occurrence(name: &str) -> Expr {
    typed_call("observe", vec![closed_wrapper(name)], Vec::new())
}

fn closed_wrapper_boolean_occurrence(name: &str) -> Expr {
    typed_call(
        "observeBoolean",
        vec![closed_wrapper(name)],
        vec![bool_lit(false)],
    )
}

fn secondary_struct_constructor(
    parameter_name: &str,
    parameter_type: TypeRef,
    syntax: ast::ParameterSyntax,
    delegation_value: Expr,
    body: Vec<Statement>,
) -> ast::StructMember {
    ast::StructMember::SecondaryConstructor(ast::SecondaryConstructorDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        params: vec![ast::Param {
            name: ident(parameter_name),
            ty: parameter_type,
            syntax,
            span: sp(),
        }],
        delegation: Some(ast::ConstructorDelegation::This {
            target_span: sp(),
            arguments: call_arguments(vec![delegation_value]),
            span: sp(),
        }),
        body: block(body),
        span: sp(),
    })
}

#[test]
fn generic_pointer_requirements_propagate_through_calls() {
    let mut declarations = pointer_requirement_functions();
    declarations.push(fun(
        "main",
        vec![stmt(typed_call("f", vec![ty_named("Int")], Vec::new()))],
    ));
    let module = lower_user(file(declarations))
        .expect("a GC-free concrete argument must satisfy propagated pointer requirements");

    for name in ["g", "f"] {
        let (function, generic) = generic_function(&module, name);
        let hir::FunctionGenericity::Generic { parameters, .. } = &function.genericity else {
            unreachable!("the helper selected a generic function")
        };
        assert_eq!(generic.gc_free_pointee_requirements.len(), 1);
        assert_eq!(
            generic.gc_free_pointee_requirements[0].type_param,
            parameters[0].id
        );
        assert!(generic.no_gc_type_params.is_empty());
    }
}

#[test]
fn propagated_pointer_requirement_rejects_a_managed_value_argument() {
    let mut declarations = vec![struct_decl(
        "ManagedValue",
        vec![("text", ty_named("String"))],
    )];
    declarations.extend(pointer_requirement_functions());
    declarations.push(fun(
        "main",
        vec![stmt(typed_call(
            "f",
            vec![ty_named("ManagedValue")],
            Vec::new(),
        ))],
    ));
    let errors = messages(declarations);
    assert!(errors.iter().any(|message| {
        message.contains("generic function `f` requires type argument ManagedValue")
            && message.contains("GC-free `Ptr` pointee")
    }));
}

#[test]
fn closure_construction_propagates_generated_invoke_pointee_requirements() {
    let declarations = vec![
        struct_decl("ManagedValue", vec![("text", ty_named("String"))]),
        generic_closure_factory("makeLambda", generic_pointer_lambda()),
        generic_closure_factory("makeAnonymous", generic_pointer_anonymous()),
        fun(
            "main",
            vec![
                stmt(typed_call(
                    "makeLambda",
                    vec![ty_named("ManagedValue")],
                    Vec::new(),
                )),
                stmt(typed_call(
                    "makeAnonymous",
                    vec![ty_named("ManagedValue")],
                    Vec::new(),
                )),
            ],
        ),
    ];
    let errors = messages(declarations);
    for name in ["makeLambda", "makeAnonymous"] {
        assert!(
            errors.iter().any(|message| {
                message.contains(&format!(
                    "generic function `{name}` requires type argument ManagedValue"
                )) && message.contains("GC-free `Ptr` pointee")
            }),
            "missing generated-invoke predicate for {name}: {errors:#?}"
        );
    }
}

#[test]
fn closure_generated_invoke_predicates_accept_gc_free_arguments() {
    let module = lower_user(file(vec![
        generic_closure_factory("makeLambda", generic_pointer_lambda()),
        generic_closure_factory("makeAnonymous", generic_pointer_anonymous()),
        fun(
            "main",
            vec![
                stmt(typed_call("makeLambda", vec![ty_named("Int")], Vec::new())),
                stmt(typed_call(
                    "makeAnonymous",
                    vec![ty_named("Int")],
                    Vec::new(),
                )),
            ],
        ),
    ]))
    .expect("generated invoke predicates must retain GC-free concrete arguments");

    for name in ["makeLambda", "makeAnonymous"] {
        let (_, generic) = generic_function(&module, name);
        assert_eq!(generic.gc_free_pointee_requirements.len(), 1, "{name}");
    }
}

#[test]
fn constructor_regions_and_default_templates_propagate_callable_pointee_requirements() {
    let class_application_span = Span::new(20, 30);
    let struct_application_span = Span::new(40, 51);
    let mut class_host = class_decl(
        ast::ClassModifier::Final,
        "ClassHost",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut class_host else {
        unreachable!()
    };
    class.type_params = vec![type_param("T")];
    class.members = vec![ast::ClassMember::InitBlock(ast::InitBlockDecl {
        body: block(vec![stmt(typed_call("g", vec![ty_named("T")], Vec::new()))]),
        span: sp(),
    })];
    class_host = with_kind(class_host, ast::TypeParamKindBound::Value);

    let mut struct_host = generic_struct_decl(
        "StructHost",
        vec!["T"],
        vec![("token", ty_named("Boolean"))],
    );
    let Decl::Struct(structure) = &mut struct_host else {
        unreachable!()
    };
    structure.members = vec![secondary_struct_constructor(
        "unit",
        ty_named("Unit"),
        ast::ParameterSyntax::Required,
        bool_lit(false),
        vec![stmt(typed_call("g", vec![ty_named("T")], Vec::new()))],
    )];
    struct_host = with_kind(struct_host, ast::TypeParamKindBound::Value);

    let mut with_default = with_kind(
        fun_sig(
            "withDefault",
            vec!["T"],
            vec![("unit", ty_named("Unit"))],
            None,
            Vec::new(),
        ),
        ast::TypeParamKindBound::Value,
    );
    let Decl::Function(function) = &mut with_default else {
        unreachable!()
    };
    function.params[0].syntax = ast::ParameterSyntax::Default {
        expression: typed_call("g", vec![ty_named("T")], Vec::new()),
        equals_span: sp(),
    };

    let mut declarations = vec![struct_decl(
        "ManagedValue",
        vec![("text", ty_named("String"))],
    )];
    declarations.extend(pointer_requirement_functions());
    declarations.extend([class_host, struct_host, with_default]);
    declarations.push(fun(
        "main",
        vec![
            val(
                "classHost",
                typed_call_at(
                    "ClassHost",
                    vec![ty_named("ManagedValue")],
                    Vec::new(),
                    class_application_span,
                ),
            ),
            val(
                "structHost",
                typed_call_at(
                    "StructHost",
                    vec![ty_named("ManagedValue")],
                    vec![unit_lit()],
                    struct_application_span,
                ),
            ),
            stmt(typed_call(
                "withDefault",
                vec![ty_named("ManagedValue")],
                Vec::new(),
            )),
        ],
    ));

    let errors = lower_user(file(declarations)).expect_err("managed applications must fail");
    for name in ["ClassHost", "StructHost", "withDefault"] {
        assert!(
            errors.iter().any(|error| {
                error.message.contains(name)
                    && error.message.contains("ManagedValue")
                    && error.message.contains("GC-free `Ptr` pointee")
            }),
            "missing predicate propagation through {name}: {errors:#?}"
        );
    }

    for (name, expected_span) in [
        ("ClassHost", class_application_span),
        ("StructHost", struct_application_span),
    ] {
        let application_errors = errors
            .iter()
            .filter(|error| {
                error
                    .message
                    .starts_with(&format!("type {name}<ManagedValue> requires"))
            })
            .collect::<Vec<_>>();
        assert_eq!(application_errors.len(), 1, "{name}: {errors:#?}");
        assert_eq!(application_errors[0].file, 1, "{name}");
        assert_eq!(application_errors[0].span, Some(expected_span), "{name}");
    }
}

#[test]
fn runtime_global_initializer_validates_callable_pointee_requirements() {
    let mut declarations = vec![struct_decl(
        "ManagedValue",
        vec![("text", ty_named("String"))],
    )];
    declarations.extend(pointer_requirement_functions());
    declarations.push(Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident("badGlobal"),
        ty: ty_named("Unit"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(typed_call("g", vec![ty_named("ManagedValue")], Vec::new())),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    }));
    declarations.push(fun("main", Vec::new()));

    let errors = messages(declarations);
    assert!(
        errors.iter().any(|message| {
            message.contains("generic function `g` requires type argument ManagedValue")
                && message.contains("GC-free `Ptr` pointee")
        }),
        "runtime initializer call must retain its predicate edge: {errors:#?}"
    );
}

#[test]
fn nested_pointer_requirements_are_replayed_for_aliases_and_static_storage() {
    let declarations = vec![
        struct_decl("ManagedValue", vec![("text", ty_named("String"))]),
        value_wrapper(),
        type_alias("BadAlias", pointer_to_wrapped("ManagedValue")),
        omitted_optional_global("badStorage", pointer_to_wrapped("ManagedValue")),
        fun("main", Vec::new()),
    ];
    let errors = messages(declarations);
    let nested_errors = errors
        .iter()
        .filter(|message| {
            message.contains("Wrapper<ManagedValue>")
                && message.contains("requires a GC-free `Ptr` pointee argument")
        })
        .count();
    assert_eq!(nested_errors, 2, "{errors:#?}");

    lower_user(file(vec![
        value_wrapper(),
        type_alias("GoodAlias", pointer_to_wrapped("Int")),
        omitted_optional_global("goodStorage", pointer_to_wrapped("Int")),
        fun("main", Vec::new()),
    ]))
    .expect("nested GC-free pointer applications must remain valid");
}

#[test]
fn direct_pointer_predicates_are_reported_once_through_nested_storage() {
    let errors = messages(vec![
        struct_decl("ManagedValue", vec![("text", ty_named("String"))]),
        omitted_optional_global(
            "badStorage",
            ty_generic("Ptr", vec![ty_named("ManagedValue")]),
        ),
        fun("main", Vec::new()),
    ]);
    let pointer_errors = errors
        .iter()
        .filter(|message| message.contains("`Ptr` pointee must be GC-free"))
        .count();
    assert_eq!(pointer_errors, 1, "{errors:#?}");
    assert!(errors.iter().all(|message| {
        !message.contains("type Option<Ptr<ManagedValue>> requires a GC-free `Ptr` pointee")
    }));
}

#[test]
fn nominal_pointer_requirements_propagate_over_every_inheritance_edge() {
    let pointer = ty_generic("Ptr", vec![ty_named("T")]);
    let carrier_method = method(
        "accept",
        vec![("pointer", pointer.clone())],
        None,
        Vec::new(),
    );
    let mut carrier = with_kind(
        generic_interface_decl("Carrier", vec!["T"], vec![carrier_method]),
        ast::TypeParamKindBound::Value,
    );
    let Decl::Interface(carrier_decl) = &mut carrier else {
        unreachable!()
    };
    carrier_decl.supertypes = Vec::new();

    let mut parent = with_kind(
        generic_interface_decl("Parent", vec!["T"], Vec::new()),
        ast::TypeParamKindBound::Value,
    );
    let Decl::Interface(parent_decl) = &mut parent else {
        unreachable!()
    };
    parent_decl.supertypes = vec![generic_supertype("Carrier", "T", false)];

    let base_method = method("basePointer", vec![("pointer", pointer)], None, Vec::new());
    let mut base = class_decl(
        ast::ClassModifier::Open,
        "Base",
        Vec::new(),
        None,
        Vec::new(),
        vec![base_method],
    );
    let Decl::Class(base_decl) = &mut base else {
        unreachable!()
    };
    base_decl.type_params = vec![ast::TypeParamDecl {
        name: ident("T"),
        inline_bound: Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value)),
        span: sp(),
    }];

    let mut derived = class_decl(
        ast::ClassModifier::Final,
        "Derived",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(derived_decl) = &mut derived else {
        unreachable!()
    };
    derived_decl.type_params = vec![ast::TypeParamDecl {
        name: ident("T"),
        inline_bound: Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value)),
        span: sp(),
    }];
    derived_decl.supertypes = vec![generic_supertype("Base", "T", true)];

    let mut class_implementation = class_decl(
        ast::ClassModifier::Final,
        "ClassImplementation",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class_decl) = &mut class_implementation else {
        unreachable!()
    };
    class_decl.type_params = vec![ast::TypeParamDecl {
        name: ident("T"),
        inline_bound: Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value)),
        span: sp(),
    }];
    class_decl.supertypes = vec![generic_supertype("Parent", "T", false)];

    let mut structure = with_kind(
        generic_struct_decl("Structure", vec!["T"], Vec::new()),
        ast::TypeParamKindBound::Value,
    );
    let Decl::Struct(structure_decl) = &mut structure else {
        unreachable!()
    };
    structure_decl.supertypes = vec![generic_supertype("Parent", "T", false)];

    let mut enumeration = enum_decl_full(
        "Enumeration",
        vec!["T"],
        vec![variant_unit("Only")],
        Vec::new(),
        Vec::new(),
    );
    enumeration = with_kind(enumeration, ast::TypeParamKindBound::Value);
    let Decl::Enum(enumeration_decl) = &mut enumeration else {
        unreachable!()
    };
    enumeration_decl.interfaces = vec![ty_generic("Parent", vec![ty_named("T")])];

    let module = lower_user(file(vec![
        carrier,
        parent,
        base,
        derived,
        class_implementation,
        structure,
        enumeration,
        fun("main", Vec::new()),
    ]))
    .expect("symbolic pointee requirements remain valid on generic nominal declarations");

    for name in [
        "Carrier",
        "Parent",
        "Base",
        "Derived",
        "ClassImplementation",
        "Structure",
        "Enumeration",
    ] {
        assert_owner_requires_its_parameter(&module, name);
    }
}

#[test]
fn closed_pointee_applications_are_replayed_in_every_struct_constructor_region() {
    let wrapper_names = [
        "StructParameterOccurrence",
        "StructDefaultOccurrence",
        "StructDelegationOccurrence",
        "StructBodyOccurrence",
    ];
    let mut host = generic_struct_decl(
        "ConstructorHost",
        Vec::new(),
        vec![("token", ty_named("Boolean"))],
    );
    let Decl::Struct(host_decl) = &mut host else {
        unreachable!()
    };
    host_decl.members = vec![
        secondary_struct_constructor(
            "parameter",
            closed_wrapper(wrapper_names[0]),
            ast::ParameterSyntax::Required,
            bool_lit(false),
            Vec::new(),
        ),
        secondary_struct_constructor(
            "withDefault",
            ty_named("Unit"),
            ast::ParameterSyntax::Default {
                expression: closed_wrapper_occurrence(wrapper_names[1]),
                equals_span: sp(),
            },
            bool_lit(false),
            Vec::new(),
        ),
        secondary_struct_constructor(
            "delegated",
            ty_named("Int"),
            ast::ParameterSyntax::Required,
            closed_wrapper_boolean_occurrence(wrapper_names[2]),
            Vec::new(),
        ),
        secondary_struct_constructor(
            "body",
            ty_named("Long"),
            ast::ParameterSyntax::Required,
            bool_lit(false),
            vec![stmt(closed_wrapper_occurrence(wrapper_names[3]))],
        ),
    ];

    let mut declarations = vec![struct_decl(
        "ManagedValue",
        vec![("text", ty_named("String"))],
    )];
    declarations.extend(wrapper_names.into_iter().map(named_pointer_wrapper));
    declarations.push(with_kind(
        fun_sig("observe", vec!["T"], Vec::new(), None, Vec::new()),
        ast::TypeParamKindBound::Value,
    ));
    declarations.push(with_kind(
        fun_expr(
            "observeBoolean",
            vec!["T"],
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("Boolean")),
            var("value"),
        ),
        ast::TypeParamKindBound::Value,
    ));
    declarations.push(host);
    declarations.push(fun("main", Vec::new()));

    let errors = messages(declarations);
    for name in wrapper_names {
        assert!(
            errors.iter().any(|message| {
                message.contains(&format!("{name}<ManagedValue>"))
                    && message.contains("requires a GC-free `Ptr` pointee argument")
            }),
            "missing replay for {name}: {errors:#?}"
        );
    }
}

#[test]
fn closed_pointee_applications_are_replayed_in_class_initialization_regions() {
    let wrapper_names = [
        "ClassParameterOccurrence",
        "ClassBaseDelegationOccurrence",
        "ClassPropertyInitializerOccurrence",
        "ClassInitBlockOccurrence",
    ];

    let mut parameter_host = class_decl(
        ast::ClassModifier::Final,
        "ParameterHost",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(parameter_host_decl) = &mut parameter_host else {
        unreachable!()
    };
    parameter_host_decl.constructor = vec![ast::PrimaryClassParameter {
        property: ast::PrimaryParameterProperty::Plain,
        member_visibility: None,
        is_override: false,
        name: ident("parameter"),
        ty: closed_wrapper(wrapper_names[0]),
        syntax: ast::ParameterSyntax::Required,
        span: sp(),
    }]
    .into_iter()
    .collect();

    let base = class_decl(
        ast::ClassModifier::Open,
        "InitializationBase",
        vec![(false, "token", ty_named("Unit"))],
        None,
        Vec::new(),
        Vec::new(),
    );
    let mut derived = class_decl(
        ast::ClassModifier::Final,
        "InitializationHost",
        Vec::new(),
        Some((
            "InitializationBase",
            vec![closed_wrapper_occurrence(wrapper_names[1])],
        )),
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(derived_decl) = &mut derived else {
        unreachable!()
    };
    derived_decl.members = vec![
        ast::ClassMember::StoredProperty(ast::PropertyDecl {
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            modifier: ast::MethodModifier::Final,
            is_override: false,
            mutable: false,
            receiver_ty: None,
            type_params: Vec::new(),
            where_clause: None,
            name: ident("initialized"),
            ty: ty_named("Unit"),
            body: ast::PropertyBodySyntax::Initializer {
                expression: Box::new(closed_wrapper_occurrence(wrapper_names[2])),
                accessors: ast::AccessorSyntax::default(),
            },
            span: sp(),
        }),
        ast::ClassMember::InitBlock(ast::InitBlockDecl {
            body: block(vec![stmt(closed_wrapper_occurrence(wrapper_names[3]))]),
            span: sp(),
        }),
    ];

    let mut declarations = vec![struct_decl(
        "ManagedValue",
        vec![("text", ty_named("String"))],
    )];
    declarations.extend(wrapper_names.into_iter().map(named_pointer_wrapper));
    declarations.push(with_kind(
        fun_sig("observe", vec!["T"], Vec::new(), None, Vec::new()),
        ast::TypeParamKindBound::Value,
    ));
    declarations.extend([parameter_host, base, derived, fun("main", Vec::new())]);

    let errors = messages(declarations);
    for name in wrapper_names {
        assert!(
            errors.iter().any(|message| {
                message.contains(&format!("{name}<ManagedValue>"))
                    && message.contains("requires a GC-free `Ptr` pointee argument")
            }),
            "missing replay for {name}: {errors:#?}"
        );
    }
}
