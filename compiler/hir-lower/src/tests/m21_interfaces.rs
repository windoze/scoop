use super::*;

fn interface(
    name: &str,
    parents: Vec<&str>,
    methods: Vec<FunctionDecl>,
    properties: Vec<ast::PropertyDecl>,
) -> Decl {
    Decl::Interface(ast::InterfaceDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        type_params: Vec::new(),
        supertypes: parents
            .into_iter()
            .map(|parent| ast::SupertypeSpec {
                ty: ty_named(parent),
                constructor_arguments: None,
                span: sp(),
            })
            .collect(),
        where_clause: None,
        methods,
        properties,
        nested: Vec::new(),
        companion: None,
        span: sp(),
    })
}

fn getter(expression: Expr) -> ast::AccessorSyntax {
    ast::AccessorSyntax {
        getter: Some(ast::GetterDecl {
            annotations: Vec::new(),
            body: ast::AccessorBodySyntax::Expr(Box::new(expression)),
            span: sp(),
        }),
        setter: None,
    }
}

fn read_write_accessors(getter_expression: Expr) -> ast::AccessorSyntax {
    ast::AccessorSyntax {
        getter: getter(getter_expression).getter,
        setter: Some(ast::SetterDecl {
            annotations: Vec::new(),
            visibility: ast::SetterVisibilitySyntax::Inherited,
            parameter: ast::SetterParameterSyntax::Default { span: sp() },
            body: ast::AccessorBodySyntax::Block(block(Vec::new())),
            span: sp(),
        }),
    }
}

fn property(
    name: &str,
    mutable: bool,
    is_override: bool,
    body: ast::PropertyBodySyntax,
) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override,
        mutable,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body,
        span: sp(),
    }
}

fn qualified_super_call(interface: &str, name: &str) -> Expr {
    Expr::QualifiedInterfaceSuperMethodCall {
        super_span: sp(),
        qualifier: ty_named(interface),
        name: ident(name),
        type_args: Vec::new(),
        args: Vec::new(),
        span: sp(),
    }
}

fn qualified_super_read(interface: &str, name: &str) -> Expr {
    Expr::QualifiedInterfaceSuperAccess {
        super_span: sp(),
        qualifier: ty_named(interface),
        name: ident(name),
        span: sp(),
    }
}

fn qualified_super_write(interface: &str, name: &str, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ast::AssignTarget::QualifiedInterfaceSuperProperty {
                qualifier: ty_named(interface),
                name: ident(name),
                span: sp(),
            },
            op: ast::AssignmentOp::Assign,
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

fn find_function(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == name).then_some(id))
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

#[test]
fn class_and_value_conformances_select_interface_defaults() {
    let declaration = interface(
        "Defaulted",
        Vec::new(),
        vec![method_expr(
            "number",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(7),
        )],
        Vec::new(),
    );
    let module = lower_user(file(vec![
        declaration,
        class_decl(
            ast::ClassModifier::Final,
            "Boxed",
            Vec::new(),
            None,
            vec!["Defaulted"],
            Vec::new(),
        ),
        struct_decl_full("Value", Vec::new(), vec!["Defaulted"], Vec::new()),
        fun("main", Vec::new()),
    ]))
    .expect("a unique default satisfies class and value-type obligations");
    let default = find_function(&module, "Defaulted.number");
    let boxed = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Boxed")
        .expect("Boxed")
        .1;
    let value = module
        .structs
        .iter()
        .find(|(_, value)| value.name == "Value")
        .expect("Value")
        .1;
    for implementations in [
        &boxed.interface_implementations,
        &value.interface_implementations,
    ] {
        let hir::InterfaceImplementationTarget::Method(application) =
            implementations[0].methods[0].target
        else {
            panic!("the default is a concrete target")
        };
        assert_eq!(module.method_applications[application].function, default);
    }
}

#[test]
fn generic_interface_default_keeps_the_exact_owner_application() {
    let contract = generic_interface_decl(
        "Echo",
        vec!["T"],
        vec![method_expr(
            "echo",
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        )],
    );
    let mut implementation = class_decl(
        ast::ClassModifier::Final,
        "IntEcho",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut implementation else {
        unreachable!()
    };
    class.supertypes.push(ast::SupertypeSpec {
        ty: ty_generic("Echo", vec![ty_named("Int")]),
        constructor_arguments: None,
        span: sp(),
    });
    let module = lower_user(file(vec![
        contract,
        implementation,
        fun("main", Vec::new()),
    ]))
    .expect("the generic default is selected for one exact application");
    let class = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "IntEcho")
        .expect("IntEcho")
        .1;
    let hir::InterfaceImplementationTarget::Method(target) =
        class.interface_implementations[0].methods[0].target
    else {
        panic!("the default is concrete")
    };
    let hir::MethodOwnerApplication::Interface(owner) = module.method_applications[target].owner
    else {
        panic!("the default retains its interface application")
    };
    assert_eq!(module.interface_applications[owner].arguments, [module.int]);
}

#[test]
fn generic_default_conflicts_keep_distinct_exact_applications() {
    let contract = generic_interface_decl(
        "Phantom",
        vec!["T"],
        vec![method_expr(
            "number",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )],
    );
    let mut implementation = class_decl(
        ast::ClassModifier::Final,
        "Both",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut implementation else {
        unreachable!()
    };
    for argument in ["Int", "String"] {
        class.supertypes.push(ast::SupertypeSpec {
            ty: ty_generic("Phantom", vec![ty_named(argument)]),
            constructor_arguments: None,
            span: sp(),
        });
    }
    let errors = lower_user(file(vec![
        contract,
        implementation,
        fun("main", Vec::new()),
    ]))
    .expect_err("different exact applications are distinct default candidates");
    assert!(errors.iter().any(|error| {
        error.message.contains("Phantom<Int>.number")
            && error.message.contains("Phantom<String>.number")
    }));
}

#[test]
fn most_specific_default_wins_and_abstract_override_suppresses_parent() {
    let parent = interface(
        "Parent",
        Vec::new(),
        vec![method_expr(
            "number",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )],
        Vec::new(),
    );
    let child = interface(
        "Child",
        vec!["Parent"],
        vec![override_method_expr(
            "number",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(2),
        )],
        Vec::new(),
    );
    let module = lower_user(file(vec![
        parent.clone(),
        child,
        class_decl(
            ast::ClassModifier::Final,
            "Chosen",
            Vec::new(),
            None,
            vec!["Child"],
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("the most-specific child default wins");
    let child_default = find_function(&module, "Child.number");
    let chosen = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Chosen")
        .expect("Chosen")
        .1;
    assert!(
        chosen
            .interface_implementations
            .iter()
            .flat_map(|implementation| &implementation.methods)
            .all(|implementation| {
                let hir::InterfaceImplementationTarget::Method(application) = implementation.target
                else {
                    return false;
                };
                module.method_applications[application].function == child_default
            })
    );

    let abstract_child = interface(
        "AbstractChild",
        vec!["Parent"],
        vec![method_full(
            true,
            false,
            "number",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::None,
        )],
        Vec::new(),
    );
    let errors = lower_user(file(vec![
        parent,
        abstract_child,
        class_decl(
            ast::ClassModifier::Final,
            "Missing",
            Vec::new(),
            None,
            vec!["AbstractChild"],
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("a child abstract slot suppresses the parent default");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("does not implement interface method `AbstractChild.number`")
    }));
}

#[test]
fn unrelated_defaults_require_an_explicit_override() {
    let default = |name| {
        interface(
            name,
            Vec::new(),
            vec![method_expr(
                "number",
                Vec::new(),
                Some(ty_named("Int")),
                int_lit(1),
            )],
            Vec::new(),
        )
    };
    let errors = lower_user(file(vec![
        default("Left"),
        default("Right"),
        class_decl(
            ast::ClassModifier::Final,
            "Conflict",
            Vec::new(),
            None,
            vec!["Left", "Right"],
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("unrelated defaults conflict");
    assert_eq!(
        errors
            .iter()
            .filter(|error| error.message.contains("inherits conflicting defaults"))
            .count(),
        1,
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn interface_property_accessors_are_independent_slots() {
    let module = lower_user(file(vec![
        interface(
            "HasValue",
            Vec::new(),
            Vec::new(),
            vec![property(
                "value",
                true,
                false,
                ast::PropertyBodySyntax::Computed(read_write_accessors(int_lit(3))),
            )],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Inherited",
            Vec::new(),
            None,
            vec!["HasValue"],
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("default getter and setter satisfy their own slots");
    let interface_decl = module
        .interfaces
        .iter()
        .find(|(_, interface)| interface.name == "HasValue")
        .expect("HasValue")
        .1;
    assert_eq!(interface_decl.properties.len(), 1);
    assert_eq!(interface_decl.methods.len(), 2);
    assert!(interface_decl.methods.iter().all(|member| {
        module.interface_methods[*member].implementation == hir::InterfaceMemberImplementation::Body
    }));

    let mixed_module = lower_user(file(vec![
        interface(
            "Mixed",
            Vec::new(),
            Vec::new(),
            vec![property(
                "value",
                true,
                false,
                ast::PropertyBodySyntax::Computed(getter(int_lit(1))),
            )],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("an omitted interface setter stays an abstract slot");
    let mixed = mixed_module
        .interfaces
        .iter()
        .find(|(_, interface)| interface.name == "Mixed")
        .expect("Mixed")
        .1;
    assert_eq!(
        mixed
            .methods
            .iter()
            .map(|member| mixed_module.interface_methods[*member].implementation)
            .collect::<Vec<_>>(),
        vec![
            hir::InterfaceMemberImplementation::Body,
            hir::InterfaceMemberImplementation::AbstractSlot,
        ]
    );

    lower_user(file(vec![
        interface(
            "ReadOnly",
            Vec::new(),
            Vec::new(),
            vec![property(
                "value",
                false,
                false,
                ast::PropertyBodySyntax::Abstract,
            )],
        ),
        {
            let mut declaration = class_decl(
                ast::ClassModifier::Final,
                "MutableImplementation",
                Vec::new(),
                None,
                vec!["ReadOnly"],
                Vec::new(),
            );
            let Decl::Class(class) = &mut declaration else {
                unreachable!()
            };
            class
                .members
                .push(ast::ClassMember::StoredProperty(property(
                    "value",
                    true,
                    true,
                    ast::PropertyBodySyntax::Computed(read_write_accessors(int_lit(1))),
                )));
            declaration
        },
        fun("main", Vec::new()),
    ]))
    .expect("a var may implement a read-only interface property");

    let errors = lower_user(file(vec![
        interface(
            "Mutable",
            Vec::new(),
            Vec::new(),
            vec![property(
                "value",
                true,
                false,
                ast::PropertyBodySyntax::Abstract,
            )],
        ),
        {
            let mut declaration = class_decl(
                ast::ClassModifier::Final,
                "Immutable",
                Vec::new(),
                None,
                vec!["Mutable"],
                Vec::new(),
            );
            let Decl::Class(class) = &mut declaration else {
                unreachable!()
            };
            class
                .members
                .push(ast::ClassMember::StoredProperty(property(
                    "value",
                    false,
                    true,
                    ast::PropertyBodySyntax::Computed(getter(int_lit(1))),
                )));
            declaration
        },
        fun("main", Vec::new()),
    ]))
    .expect_err("a val cannot override an interface var");
    assert!(
        errors.iter().any(|error| error.message
            == "immutable property `value` cannot override a mutable property")
    );

    let errors = lower_user(file(vec![
        interface(
            "DefaultMutable",
            Vec::new(),
            Vec::new(),
            vec![property(
                "value",
                true,
                false,
                ast::PropertyBodySyntax::Computed(read_write_accessors(int_lit(1))),
            )],
        ),
        struct_decl_full(
            "ImmutableValue",
            Vec::new(),
            vec!["DefaultMutable"],
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("value types cannot acquire mutable-this semantics from a default setter");
    assert!(errors.iter().any(|error| error.message
        == "struct `ImmutableValue` cannot implement mutable interface property `value`"));
}

#[test]
fn qualified_interface_super_calls_default_function_getter_and_setter() {
    let contract = interface(
        "Contract",
        Vec::new(),
        vec![method_expr(
            "number",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )],
        vec![property(
            "value",
            true,
            false,
            ast::PropertyBodySyntax::Computed(read_write_accessors(int_lit(2))),
        )],
    );
    let read = method_expr(
        "read",
        Vec::new(),
        Some(ty_named("Int")),
        qualified_super_read("Contract", "value"),
    );
    let write = method(
        "write",
        vec![("value", ty_named("Int"))],
        None,
        vec![qualified_super_write("Contract", "value", var("value"))],
    );
    let number = override_method_expr(
        "number",
        Vec::new(),
        Some(ty_named("Int")),
        qualified_super_call("Contract", "number"),
    );
    let module = lower_user(file(vec![
        contract,
        class_decl(
            ast::ClassModifier::Final,
            "Implementation",
            Vec::new(),
            None,
            vec!["Contract"],
            vec![read, write, number],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("all qualified interface defaults lower as direct calls");
    for function in [
        "Implementation.read",
        "Implementation.write",
        "Implementation.number",
    ] {
        let hir::FunctionKind::User(body) =
            &module.functions[find_function(&module, function)].kind
        else {
            panic!("{function} body")
        };
        assert!(body.statements.iter().any(|statement| matches!(
            &statement.kind,
            hir::StatementKind::Expr(hir::Expr {
                kind: hir::ExprKind::DirectSuperMethodCall { .. },
                ..
            }) | hir::StatementKind::Return {
                value: Some(hir::Expr {
                    kind: hir::ExprKind::DirectSuperMethodCall { .. },
                    ..
                }),
            }
        )));
    }
}

#[test]
fn qualified_interface_super_requires_a_direct_concrete_target() {
    let parent = interface(
        "Parent",
        Vec::new(),
        vec![method_expr(
            "number",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )],
        Vec::new(),
    );
    let child = interface("Child", vec!["Parent"], Vec::new(), Vec::new());
    let errors = lower_user(file(vec![
        parent,
        child,
        class_decl(
            ast::ClassModifier::Final,
            "Implementation",
            Vec::new(),
            None,
            vec!["Child"],
            vec![method_expr(
                "read",
                Vec::new(),
                Some(ty_named("Int")),
                qualified_super_call("Parent", "number"),
            )],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("the qualifier must name an exact direct superinterface");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("is not a direct superinterface of class `Implementation`")
    }));
}

#[test]
fn private_interface_helpers_have_bodies_and_no_slots() {
    let mut helper = method_expr("helper", Vec::new(), Some(ty_named("Int")), int_lit(5));
    helper.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let default = method_expr(
        "number",
        Vec::new(),
        Some(ty_named("Int")),
        method_call(this_expr(), "helper", Vec::new()),
    );
    let module = lower_user(file(vec![
        interface("WithHelper", Vec::new(), vec![helper, default], Vec::new()),
        fun("main", Vec::new()),
    ]))
    .expect("a private interface helper is callable by its declaring interface");
    let interface_decl = module
        .interfaces
        .iter()
        .find(|(_, interface)| interface.name == "WithHelper")
        .expect("WithHelper")
        .1;
    assert_eq!(interface_decl.private_methods.len(), 1);
    assert_eq!(interface_decl.methods.len(), 1);

    let mut bodyless = bodyless_method(false, "helper", Vec::new(), None);
    bodyless.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let errors = lower_user(file(vec![
        interface("Invalid", Vec::new(), vec![bodyless], Vec::new()),
        fun("main", Vec::new()),
    ]))
    .expect_err("private interface helpers require executable bodies");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "private interface method `helper` must have a body")
    );
}
