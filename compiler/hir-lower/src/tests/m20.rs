use super::*;

fn upper(name: &str, bound: TypeRef) -> ast::TypeParamDecl {
    ast::TypeParamDecl {
        name: ident(name),
        inline_bound: Some(ast::TypeBound::Upper(bound)),
        span: sp(),
    }
}

fn where_clause(constraints: Vec<(&str, ast::TypeBound)>) -> ast::WhereClause {
    ast::WhereClause {
        constraints: constraints
            .into_iter()
            .map(|(parameter, bound)| ast::TypeConstraint {
                parameter: ident(parameter),
                bound,
                span: sp(),
            })
            .collect(),
        span: sp(),
    }
}

fn generic_class(
    name: &str,
    modifier: ast::ClassModifier,
    parameters: Vec<ast::TypeParamDecl>,
    constructor: Vec<(bool, &str, TypeRef)>,
    methods: Vec<ast::FunctionDecl>,
) -> Decl {
    let mut declaration = class_decl(modifier, name, constructor, None, Vec::new(), methods);
    let Decl::Class(class) = &mut declaration else {
        unreachable!()
    };
    class.type_params = parameters;
    declaration
}

fn class_bound_world() -> Vec<Decl> {
    let base = generic_class(
        "Base",
        ast::ClassModifier::Open,
        vec![type_param("T")],
        vec![(false, "value", ty_named("T"))],
        vec![method_expr(
            "get",
            Vec::new(),
            Some(ty_named("T")),
            var("value"),
        )],
    );
    let mut derived = class_decl(
        ast::ClassModifier::Final,
        "StringNode",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut derived else {
        unreachable!()
    };
    class.supertypes = vec![constructor_supertype(
        ty_generic("Base", vec![ty_named("String")]),
        call_arguments(vec![str_lit("node")]),
    )];
    vec![base, derived]
}

fn class_bounded_read(bound_argument: TypeRef) -> Decl {
    let mut declaration = fun_expr(
        "read",
        vec!["T"],
        vec![("node", ty_named("T"))],
        Some(ty_named("String")),
        method_call(var("node"), "get", Vec::new()),
    );
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.type_params[0] = upper("T", ty_generic("Base", vec![bound_argument]));
    declaration
}

#[test]
fn class_bound_member_resolves_to_a_concrete_class_method() {
    let mut declarations = class_bound_world();
    declarations.extend([
        class_bounded_read(ty_named("String")),
        fun(
            "main",
            vec![stmt(call("read", vec![call("StringNode", Vec::new())]))],
        ),
    ]);
    let output = lower_user_output(file(declarations))
        .expect("a subclass of the exact class bound must expose inherited members");

    let bound = output
        .export
        .bound_callable_refs
        .iter()
        .map(|(_, bound)| bound)
        .find(|bound| {
            matches!(
                bound.source,
                hir::BoundCallableSource::Class { callable, .. }
                    if output.export.functions[output.export.callable_function(callable)].name
                        == "Base.get"
            )
        })
        .expect("the generic template keeps a typed class-bound member");
    let hir::BoundCallableSource::Class { bound, .. } = bound.source else {
        unreachable!()
    };
    assert_eq!(
        hir::type_name(
            &output.export,
            output.export.class_applications[bound].canonical_type
        ),
        "Base<String>"
    );

    let read = output
        .local
        .functions
        .iter()
        .find(|(_, function)| {
            function.name == "read"
                && matches!(
                    function.origin,
                    hir::concrete::FunctionOrigin::Free(
                        hir::concrete::FreeFunctionOrigin::Generic { .. }
                    )
                )
        })
        .expect("read<StringNode> specialization")
        .1;
    let hir::concrete::FunctionKind::User(body) = &read.kind else {
        panic!("the specialization has a user body")
    };
    let result = body
        .statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::Return { value: Some(value) } => Some(value),
            _ => None,
        })
        .expect("the specialization returns its bound call");
    let hir::concrete::ExprKind::MethodCall { callee, .. } = &result.kind else {
        panic!("the bound call becomes an ordinary concrete method call")
    };
    let target = output.local.callable_function(*callee);
    assert_eq!(output.local.functions[target].name, "Base.get");
}

#[test]
fn class_bound_actual_must_reach_the_exact_application() {
    let mut declarations = class_bound_world();
    declarations.extend([
        class_bounded_read(ty_named("Int")),
        fun(
            "main",
            vec![stmt(call("read", vec![call("StringNode", Vec::new())]))],
        ),
    ]);
    let errors = lower_user(file(declarations))
        .expect_err("Base<String> must not satisfy the invariant Base<Int> bound");
    assert!(errors.iter().any(|error| {
        error.message.contains(
            "type argument `StringNode` for `T` must satisfy class upper bound `Base<Int>`",
        )
    }));
}

#[test]
fn class_and_interface_bounds_form_one_typed_nominal_set() {
    let mut declarations = class_bound_world();
    let mut constrained = generic_struct_decl("Constrained", vec!["T"], Vec::new());
    let Decl::Struct(structure) = &mut constrained else {
        unreachable!()
    };
    structure.type_params[0] = upper("T", ty_named("Named"));
    structure.where_clause = Some(where_clause(vec![(
        "T",
        ast::TypeBound::Upper(ty_generic("Base", vec![ty_named("String")])),
    )]));
    declarations.extend([
        interface_decl("Named", Vec::new()),
        constrained,
        fun("main", Vec::new()),
    ]);
    let module = lower_user(file(declarations))
        .expect("one class bound may be combined with interface bounds");
    let declaration = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Constrained")
        .unwrap()
        .1;
    let hir::TypeParamBounds::Nominal(bounds) = &declaration.type_params[0].bounds else {
        panic!("upper bounds use the nominal constraint branch")
    };
    let class = bounds.class.as_ref().expect("one typed class bound");
    assert_eq!(bounds.interfaces.len(), 1);
    assert_eq!(
        hir::type_name(
            &module,
            module.class_applications[class.application].canonical_type
        ),
        "Base<String>"
    );
}

#[test]
fn duplicate_class_and_nonnominal_bounds_are_rejected() {
    let first = generic_class(
        "First",
        ast::ClassModifier::Open,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let second = generic_class(
        "Second",
        ast::ClassModifier::Open,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut duplicate = generic_struct_decl("Duplicate", vec!["T"], Vec::new());
    let Decl::Struct(structure) = &mut duplicate else {
        unreachable!()
    };
    structure.type_params[0] = upper("T", ty_named("First"));
    structure.where_clause = Some(where_clause(vec![(
        "T",
        ast::TypeBound::Upper(ty_named("Second")),
    )]));

    let mut invalid = generic_struct_decl("Invalid", vec!["T"], Vec::new());
    let Decl::Struct(structure) = &mut invalid else {
        unreachable!()
    };
    structure.type_params[0] = upper("T", ty_named("Int"));

    let mut invalid_any = generic_struct_decl("InvalidAny", vec!["T"], Vec::new());
    let Decl::Struct(structure) = &mut invalid_any else {
        unreachable!()
    };
    structure.type_params[0] = upper("T", ty_named("Any"));

    let mut invalid_function = generic_struct_decl("InvalidFunction", vec!["T"], Vec::new());
    let Decl::Struct(structure) = &mut invalid_function else {
        unreachable!()
    };
    structure.type_params[0] = upper("T", ty_function(false, Vec::new(), ty_named("String")));

    let mut invalid_parameter = generic_struct_decl("InvalidParameter", vec!["T", "U"], Vec::new());
    let Decl::Struct(structure) = &mut invalid_parameter else {
        unreachable!()
    };
    structure.type_params[0] = upper("T", ty_named("U"));

    let errors = lower_user(file(vec![
        first,
        second,
        duplicate,
        invalid,
        invalid_any,
        invalid_function,
        invalid_parameter,
        fun("main", Vec::new()),
    ]))
    .expect_err("a second class bound and a value bound must fail");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("type parameter `T` of struct cannot have more than one class upper bound; found `Second`")
    }));
    assert!(errors.iter().any(|error| {
        error.message
            == "upper bound of type parameter `T` must be a class or interface, found `Int`"
    }));
    assert!(errors.iter().any(|error| {
        error.message
            == "upper bound of type parameter `T` must be a class or interface, found `Any`"
    }));
    assert!(errors.iter().any(|error| {
        error.message
            == "upper bound of type parameter `T` must be a class or interface, found `() -> String`"
    }));
    assert!(errors.iter().any(|error| {
        error.message == "upper bound of type parameter `T` must be a class or interface, found `U`"
    }));
}

#[test]
fn class_bound_exposes_an_abstract_interface_capability() {
    let named = interface_decl(
        "Named",
        vec![bodyless_method(
            false,
            "name",
            Vec::new(),
            Some(ty_named("String")),
        )],
    );
    let base = class_decl(
        ast::ClassModifier::Abstract,
        "NamedBase",
        Vec::new(),
        None,
        vec!["Named"],
        Vec::new(),
    );
    let concrete = class_decl(
        ast::ClassModifier::Final,
        "ConcreteName",
        Vec::new(),
        Some(("NamedBase", Vec::new())),
        Vec::new(),
        vec![override_method_expr(
            "name",
            Vec::new(),
            Some(ty_named("String")),
            str_lit("concrete"),
        )],
    );
    let mut render = fun_expr(
        "render",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("String")),
        method_call(var("value"), "name", Vec::new()),
    );
    let Decl::Function(function) = &mut render else {
        unreachable!()
    };
    function.type_params[0] = upper("T", ty_named("NamedBase"));

    let output = lower_user_output(file(vec![
        named,
        base,
        concrete,
        render,
        fun(
            "main",
            vec![stmt(call("render", vec![call("ConcreteName", Vec::new())]))],
        ),
    ]))
    .expect("a class bound includes interface capabilities inherited from the class");
    assert!(output.export.bound_callable_refs.iter().any(|(_, bound)| {
        matches!(
            bound.source,
            hir::BoundCallableSource::Interface { member, .. }
                if output.export.functions[output.export.interface_methods[member].function].name
                    == "Named.name"
        )
    }));
    let render = output
        .local
        .functions
        .iter()
        .find(|(_, function)| function.name == "render")
        .expect("render<ConcreteName> specialization")
        .1;
    let hir::concrete::FunctionKind::User(body) = &render.kind else {
        panic!("render specialization has a user body")
    };
    let result = body
        .statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::Return { value: Some(value) } => Some(value),
            _ => None,
        })
        .expect("render returns the interface capability call");
    let hir::concrete::ExprKind::MethodCall { callee, .. } = &result.kind else {
        panic!("the interface capability becomes a concrete method call")
    };
    let target = output.local.callable_function(*callee);
    assert_eq!(output.local.functions[target].name, "ConcreteName.name");
}

#[test]
fn recursive_exact_class_bound_is_legal() {
    let base = generic_class(
        "RecursiveBase",
        ast::ClassModifier::Open,
        vec![type_param("T")],
        Vec::new(),
        Vec::new(),
    );

    let mut node = class_decl(
        ast::ClassModifier::Final,
        "RecursiveNode",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut node else {
        unreachable!()
    };
    class.supertypes = vec![constructor_supertype(
        ty_generic("RecursiveBase", vec![ty_named("RecursiveNode")]),
        Vec::new(),
    )];

    let mut identity = fun_expr(
        "recursiveIdentity",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let Decl::Function(function) = &mut identity else {
        unreachable!()
    };
    function.type_params[0] = upper("T", ty_generic("RecursiveBase", vec![ty_named("T")]));

    lower_user(file(vec![
        base,
        node,
        identity,
        fun(
            "main",
            vec![stmt(call(
                "recursiveIdentity",
                vec![call("RecursiveNode", Vec::new())],
            ))],
        ),
    ]))
    .expect("an exact F-bound must terminate by typed application identity");
}
