//! M14 structured generic constraints and interface upper-bound validation.

use scoop_ast as ast;
use scoop_hir as hir;

use super::*;

fn upper(name: &str, interface: TypeRef) -> ast::TypeParamDecl {
    ast::TypeParamDecl {
        name: ident(name),
        variance: ast::Variance::Invariant,
        inline_bound: Some(ast::TypeBound::Upper(interface)),
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

fn marker_world() -> Vec<Decl> {
    vec![
        interface_decl("Marker", vec![]),
        struct_decl_full("Marked", vec![], vec!["Marker"], vec![]),
    ]
}

fn bounded_identity() -> Decl {
    let mut declaration = fun_expr(
        "boundedIdentity",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.type_params[0] = upper("T", ty_named("Marker"));
    declaration
}

fn generic_class(
    name: &str,
    parameters: Vec<ast::TypeParamDecl>,
    constructor: Vec<(bool, &str, TypeRef)>,
    methods: Vec<ast::FunctionDecl>,
) -> Decl {
    let mut declaration = class_decl(
        ast::ClassModifier::Final,
        name,
        constructor,
        None,
        Vec::new(),
        methods,
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!()
    };
    class.type_params = parameters;
    declaration
}

#[test]
fn interface_bounds_are_complete_ordered_hir_constraints() {
    let mut renderable =
        generic_interface_decl("Renderable", vec![(ast::Variance::Invariant, "T")], vec![]);
    let Decl::Interface(renderable_decl) = &mut renderable else {
        unreachable!()
    };
    renderable_decl.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_generic(
        "Comparable",
        vec![ty_named("T")],
    )));

    let mut constrained = generic_struct_decl("Constrained", vec!["T"], vec![]);
    let Decl::Struct(constrained_decl) = &mut constrained else {
        unreachable!()
    };
    constrained_decl.where_clause = Some(where_clause(vec![
        ("T", ast::TypeBound::Upper(ty_named("Marker"))),
        (
            "T",
            ast::TypeBound::Upper(ty_generic("Comparable", vec![ty_named("T")])),
        ),
    ]));

    let module = lower_user(file(vec![
        interface_decl("Marker", vec![]),
        generic_interface_decl("Comparable", vec![(ast::Variance::Invariant, "T")], vec![]),
        renderable,
        constrained,
        fun("main", vec![]),
    ]))
    .expect("multiple interface bounds and F-bounds must lower");

    let declaration = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Constrained")
        .unwrap()
        .1;
    let hir::TypeParamBounds::Interfaces(bounds) = &declaration.type_params[0].bounds else {
        panic!("interface upper bounds must use the typed constraint branch")
    };
    assert_eq!(bounds.len(), 2);
    assert_eq!(
        hir::type_name(
            &module,
            module.interface_applications[bounds[0].application].canonical_type
        ),
        "Marker"
    );
    assert_eq!(
        hir::type_name(
            &module,
            module.interface_applications[bounds[1].application].canonical_type
        ),
        "Comparable<T0>"
    );
    assert!(
        hir::dump(&module).contains("struct Constrained<T : Marker & Comparable<T>>"),
        "HIR dump must preserve bound order"
    );
}

#[test]
fn concrete_and_generic_arguments_must_prove_interface_bounds() {
    let mut declarations = marker_world();
    declarations.extend([
        bounded_identity(),
        fun(
            "main",
            vec![stmt(call(
                "boundedIdentity",
                vec![struct_init("Marked", vec![])],
            ))],
        ),
    ]);
    lower_user(file(declarations)).expect("an implementing value type satisfies the bound");

    let mut declarations = marker_world();
    declarations.extend([
        bounded_identity(),
        fun(
            "main",
            vec![stmt(call("boundedIdentity", vec![int_lit(1)]))],
        ),
    ]);
    let errors = lower_user(file(declarations)).expect_err("Int does not implement Marker");
    assert!(errors.iter().any(|error| {
        error.message
            == "type argument `Int` for `T` of function `boundedIdentity` must satisfy interface upper bound `Marker`"
    }));
}

#[test]
fn bound_declaration_errors_are_diagnosed_at_hir() {
    let mut declaration = fun_expr(
        "invalid",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.type_params[0].inline_bound =
        Some(ast::TypeBound::Kind(ast::TypeParamKindBound::Value));
    function.where_clause = Some(where_clause(vec![
        ("T", ast::TypeBound::Upper(ty_named("Marker"))),
        ("U", ast::TypeBound::Upper(ty_named("Marker"))),
    ]));

    let errors = lower_user(file(vec![
        interface_decl("Marker", vec![]),
        declaration,
        fun("main", vec![]),
    ]))
    .expect_err("kind/interface mixing and unknown where parameters must fail");
    assert!(errors.iter().any(|error| {
        error.message
            == "type parameter `T` of function cannot combine interface upper bounds with a kind bound"
    }));
    assert!(errors.iter().any(|error| {
        error.message == "unknown type parameter `U` in where clause of function"
    }));
}

#[test]
fn upper_bound_must_be_a_complete_interface_application() {
    let mut declaration = bounded_identity();
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.type_params[0] = upper("T", ty_named("Int"));
    let errors = lower_user(file(vec![declaration, fun("main", vec![])]))
        .expect_err("a value type cannot be an upper bound");
    assert!(errors.iter().any(|error| {
        error.message == "upper bound of type parameter `T` must be an interface, found `Int`"
    }));
}

#[test]
fn duplicate_interface_bounds_are_rejected_by_application_identity() {
    let mut declaration = fun_expr(
        "duplicate",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.where_clause = Some(where_clause(vec![
        ("T", ast::TypeBound::Upper(ty_named("Marker"))),
        ("T", ast::TypeBound::Upper(ty_named("Marker"))),
    ]));
    let errors = lower_user(file(vec![
        interface_decl("Marker", vec![]),
        declaration,
        fun("main", vec![]),
    ]))
    .expect_err("the same normalized bound cannot appear twice");
    assert!(errors.iter().any(|error| {
        error.message
            == "duplicate interface upper bound `Marker` for type parameter `T` of function"
    }));
}

#[test]
fn generic_interface_methods_are_rejected_at_definition() {
    let mut method = method_full(
        false,
        true,
        "map",
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        FunctionBody::None,
    );
    method.type_params = vec![type_param("T")];
    let errors = lower_user(file(vec![
        interface_decl("Mapper", vec![method]),
        fun("main", vec![]),
    ]))
    .expect_err("interface method-level generics have no dispatch ABI");
    assert!(errors.iter().any(|error| {
        error.message == "interface method `map` cannot declare method type parameters"
    }));
}

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
    assert!(instances.iter().all(|instance| {
        instance.type_arguments.len() == 1
            && instance.constructor.len() == 1
            && instance.constructor[0].ty == instance.type_arguments[0]
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
fn generic_method_applications_keep_owner_and_method_arguments_separate() {
    let mut choose = method_expr(
        "choose",
        vec![("value", ty_named("U"))],
        Some(ty_named("U")),
        var("value"),
    );
    choose.type_params = vec![type_param("U")];
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(var("box"))),
        name: ident("choose"),
        span: sp(),
    };
    let output = lower_user_output(file(vec![
        generic_class(
            "Box",
            vec![type_param("T")],
            vec![(false, "value", ty_named("T"))],
            vec![choose],
        ),
        fun(
            "main",
            vec![
                val("box", call("Box", vec![str_lit("owner")])),
                val(
                    "number",
                    typed_method_call(
                        var("box"),
                        "choose",
                        vec![ty_named("Int")],
                        vec![int_lit(42)],
                    ),
                ),
                val_ty(
                    "stringChoice",
                    Some(ty_function(
                        false,
                        vec![ty_named("String")],
                        ty_named("String"),
                    )),
                    reference,
                ),
            ],
        ),
    ]))
    .expect("generic method calls and references must have exact applications");

    let method = output
        .export
        .functions
        .iter()
        .find(|(_, function)| function.name == "Box.choose")
        .expect("generic method declaration")
        .0;
    let hir::FunctionGenericity::GenericMethod {
        definition,
        owner_parameters,
        method_parameters,
    } = &output.export.functions[method].genericity
    else {
        panic!("Box.choose must have a distinct generic-method identity")
    };
    assert_eq!(owner_parameters.len(), 1);
    assert_eq!(method_parameters.len(), 1);
    assert_ne!(
        owner_parameters[0].id.identity_raw(),
        method_parameters.iter().next().unwrap().id.identity_raw(),
        "owner and method parameters have distinct semantic identities"
    );
    assert_eq!(owner_parameters[0].id.into_raw(), 0);
    assert_eq!(method_parameters.iter().next().unwrap().id.into_raw(), 1);

    let applications = output
        .export
        .generic_method_applications
        .iter()
        .filter(|(_, application)| application.method == *definition)
        .map(|(_, application)| application)
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 2);
    for application in &applications {
        let hir::GenericMethodOwner::Class(owner) = application.owner else {
            panic!("Box.choose must retain its exact class application")
        };
        assert_eq!(
            output.export.class_applications[owner].arguments,
            vec![output.export.string]
        );
    }
    assert!(
        applications
            .iter()
            .any(|application| application.method_arguments.to_vec() == [output.export.int])
    );
    assert!(
        applications
            .iter()
            .any(|application| application.method_arguments.to_vec() == [output.export.string])
    );

    let concrete_methods = output
        .local
        .functions
        .iter()
        .filter_map(|(_, function)| match &function.origin {
            hir::concrete::FunctionOrigin::Method(hir::concrete::MethodOrigin {
                owner: hir::concrete::MethodOwner::Class(owner),
                specialization:
                    hir::concrete::MethodSpecialization::Generic {
                        definition: found,
                        method_arguments,
                        ..
                    },
            }) if found == definition => Some((*owner, method_arguments.to_vec())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(concrete_methods.len(), 2);
    for (owner, _) in &concrete_methods {
        assert_eq!(
            output.local.classes[*owner].type_arguments,
            vec![output.local.string]
        );
        assert!(
            output.local.classes[*owner]
                .methods
                .iter()
                .all(|method| output.local.functions[*method].name != "Box.choose"),
            "generic methods are direct applications and never dispatch-table members"
        );
    }
    assert!(
        concrete_methods
            .iter()
            .any(|(_, arguments)| arguments == &[output.local.int])
    );
    assert!(
        concrete_methods
            .iter()
            .any(|(_, arguments)| arguments == &[output.local.string])
    );
}

#[test]
fn parameterized_method_families_use_one_source_identity_domain_for_symbols() {
    let ordinary = method_expr(
        "keep",
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let mut generic = method_expr(
        "keep",
        vec![("value", ty_named("U")), ("marker", ty_named("Int"))],
        Some(ty_named("U")),
        var("value"),
    );
    generic.type_params = vec![type_param("U")];
    let output = lower_user_output(file(vec![
        generic_class(
            "Host",
            vec![type_param("T")],
            vec![(false, "value", ty_named("T"))],
            vec![ordinary, generic],
        ),
        fun(
            "main",
            vec![
                val("host", call("Host", vec![str_lit("owner")])),
                stmt(method_call(var("host"), "keep", vec![str_lit("ordinary")])),
                stmt(method_call(
                    var("host"),
                    "keep",
                    vec![int_lit(42), int_lit(0)],
                )),
            ],
        ),
    ]))
    .expect("both parameterized method families have exact concrete identities");

    let symbols = output
        .local
        .functions
        .iter()
        .filter_map(|(_, function)| {
            if function.name != "Host.keep" {
                return None;
            }
            let hir::concrete::FunctionOrigin::Method(origin) = &function.origin else {
                return None;
            };
            match &origin.specialization {
                hir::concrete::MethodSpecialization::OwnerParameterized { symbol }
                | hir::concrete::MethodSpecialization::Generic { symbol, .. } => Some(*symbol),
                hir::concrete::MethodSpecialization::Plain => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 2);
    let discriminators = symbols
        .iter()
        .map(|symbol| match symbol {
            hir::concrete::InstanceSymbol::Overloaded { discriminator } => *discriminator,
            hir::concrete::InstanceSymbol::Unique => {
                panic!("same-name parameterized declarations require explicit symbol identities")
            }
        })
        .collect::<Vec<_>>();
    assert_ne!(discriminators[0], discriminators[1]);
}

#[test]
fn generic_method_bounds_are_checked_with_the_method_argument_group() {
    let mut accept = method_expr(
        "accept",
        vec![("value", ty_named("U"))],
        Some(ty_named("U")),
        var("value"),
    );
    accept.type_params = vec![upper("U", ty_named("Marker"))];
    let host = generic_class(
        "Host",
        vec![type_param("T")],
        vec![(false, "value", ty_named("T"))],
        vec![accept],
    );

    lower_user(file(vec![
        interface_decl("Marker", vec![]),
        struct_decl_full("Marked", vec![], vec!["Marker"], vec![]),
        host.clone(),
        fun(
            "main",
            vec![stmt(method_call(
                call("Host", vec![int_lit(1)]),
                "accept",
                vec![struct_init("Marked", vec![])],
            ))],
        ),
    ]))
    .expect("the method argument satisfies its exact upper bound");

    let errors = lower_user(file(vec![
        interface_decl("Marker", vec![]),
        host,
        fun(
            "main",
            vec![stmt(method_call(
                call("Host", vec![int_lit(1)]),
                "accept",
                vec![int_lit(2)],
            ))],
        ),
    ]))
    .expect_err("the owner argument must not be used in place of the method argument");
    assert!(errors.iter().any(|error| {
        error.message
            == "type argument `Int` for `U` of function `Host.accept` must satisfy interface upper bound `Marker`"
    }));
}

#[test]
fn generic_callable_recursion_requires_an_identity_argument_mapping() {
    let direct = fun_sig(
        "direct",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        vec![stmt(typed_call(
            "direct",
            vec![ty_named("T")],
            vec![var("value")],
        ))],
    );
    let first = fun_sig(
        "first",
        vec!["A", "B"],
        vec![("a", ty_named("A")), ("b", ty_named("B"))],
        None,
        vec![stmt(typed_call(
            "second",
            vec![ty_named("B"), ty_named("A")],
            vec![var("b"), var("a")],
        ))],
    );
    let second = fun_sig(
        "second",
        vec!["X", "Y"],
        vec![("x", ty_named("X")), ("y", ty_named("Y"))],
        None,
        vec![stmt(typed_call(
            "first",
            vec![ty_named("Y"), ty_named("X")],
            vec![var("y"), var("x")],
        ))],
    );
    lower_user(file(vec![direct, first, second, fun("main", vec![])]))
        .expect("identity recursion and an identity-composing parameter permutation terminate");

    let growing = fun_sig(
        "growing",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        vec![stmt(typed_call(
            "growing",
            vec![ty_nullable(ty_named("T"))],
            vec![some(var("value"))],
        ))],
    );
    let errors = lower_user(file(vec![growing, fun("main", vec![])]))
        .expect_err("a recursive application that grows its argument is polymorphic recursion");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("polymorphic recursion is not supported")
            && error.message.contains("growing")
            && error
                .message
                .contains("changes its complete type-argument mapping")
    }));
}

#[test]
fn generic_method_polymorphic_recursion_checks_both_argument_groups() {
    let mut grow = method(
        "grow",
        vec![("value", ty_named("U"))],
        None,
        vec![stmt(typed_method_call(
            this_expr(),
            "grow",
            vec![ty_nullable(ty_named("U"))],
            vec![some(var("value"))],
        ))],
    );
    grow.type_params = vec![type_param("U")];
    let host = generic_struct_decl_full(
        "Host",
        vec!["T"],
        vec![("owner", ty_named("T"))],
        vec![],
        vec![grow],
    );
    let errors = lower_user(file(vec![host, fun("main", vec![])]))
        .expect_err("changing the method suffix in a recursive cycle must fail");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("polymorphic recursion is not supported")
            && error.message.contains("Host.grow")
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
    let (base, arguments) = derived.base_class.as_ref().expect("typed concrete base");
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
    let (base, _) = derived.base_class.as_ref().expect("specialized base");
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

#[test]
fn bounded_receiver_call_records_exact_interface_member_identity() {
    let show = interface_decl(
        "Show",
        vec![method_full(
            false,
            true,
            "show",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::None,
        )],
    );
    let shown = struct_decl_full(
        "Shown",
        Vec::new(),
        vec!["Show"],
        vec![method_full(
            true,
            false,
            "show",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::Expr(Box::new(int_lit(7))),
        )],
    );
    let mut read = fun_expr(
        "read",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("Int")),
        method_call(var("value"), "show", Vec::new()),
    );
    let Decl::Function(read_decl) = &mut read else {
        unreachable!()
    };
    read_decl.type_params[0] = upper("T", ty_named("Show"));

    let output = lower_user_output(file(vec![
        show,
        shown,
        read,
        fun(
            "main",
            vec![stmt(call("read", vec![struct_init("Shown", Vec::new())]))],
        ),
    ]))
    .expect("a bound method must resolve from the declared interface capability");

    assert_eq!(output.export.bound_callable_refs.len(), 1);
    let bound = &output.export.bound_callable_refs.iter().next().unwrap().1;
    let member = output.export.interface_methods[bound.member];
    assert_eq!(
        output.export.functions[member.function]
            .name
            .rsplit('.')
            .next(),
        Some("show")
    );
    let shown = output
        .export
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Shown")
        .unwrap()
        .1;
    let hir::InterfaceImplementationTarget::Method(application) =
        shown.interface_implementations[0].methods[0].target
    else {
        panic!("Shown conformance has an exact method application")
    };
    assert_eq!(
        output.export.functions[output.export.method_applications[application].function].name,
        "Shown.show"
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
        .expect("read<Shown> specialization")
        .1;
    let hir::concrete::FunctionKind::User(body) = &read.kind else {
        panic!("read specialization must have a user body")
    };
    let hir::concrete::StatementKind::Return {
        value:
            Some(hir::concrete::Expr {
                kind: hir::concrete::ExprKind::MethodCall { callee, .. },
                ..
            }),
    } = &body.statements[0].kind
    else {
        panic!("read specialization must return its bound call")
    };
    let target = output.local.callable_function(*callee);
    assert_eq!(output.local.functions[target].name, "Shown.show");
}

#[test]
fn bound_member_inherits_through_exact_parent_application() {
    let parent = interface_decl(
        "Parent",
        vec![method_full(
            false,
            true,
            "value",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::None,
        )],
    );
    let mut child = interface_decl("Child", Vec::new());
    let Decl::Interface(child_decl) = &mut child else {
        unreachable!()
    };
    child_decl.parents = vec![ty_named("Parent")];
    let implementation = struct_decl_full(
        "Implementation",
        Vec::new(),
        vec!["Child"],
        vec![method_full(
            true,
            false,
            "value",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::Expr(Box::new(int_lit(11))),
        )],
    );
    let mut read = fun_expr(
        "readParent",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("Int")),
        method_call(var("value"), "value", Vec::new()),
    );
    let Decl::Function(read_decl) = &mut read else {
        unreachable!()
    };
    read_decl.type_params[0] = upper("T", ty_named("Child"));

    let output = lower_user_output(file(vec![
        parent,
        child,
        implementation,
        read,
        fun(
            "main",
            vec![stmt(call(
                "readParent",
                vec![struct_init("Implementation", Vec::new())],
            ))],
        ),
    ]))
    .expect("a bound exposes members inherited from its exact parent application");

    let bound = output.export.bound_callable_refs.iter().next().unwrap().1;
    assert_eq!(
        output.export.interfaces[output.export.interface_applications[bound.bound].template].name,
        "Child"
    );
    assert_eq!(
        output.export.interfaces[output.export.interface_methods[bound.member].owner].name,
        "Parent"
    );
    let implementation = output
        .export
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Implementation")
        .unwrap()
        .1;
    let child_conformance = implementation
        .interface_implementations
        .iter()
        .find(|conformance| {
            output.export.interfaces
                [output.export.interface_applications[conformance.interface].template]
                .name
                == "Child"
        })
        .expect("Child conformance");
    assert_eq!(child_conformance.methods[0].member, bound.member);
}

#[test]
fn unrelated_bounds_with_the_same_member_are_ambiguous() {
    let method = || {
        method_full(
            false,
            true,
            "value",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::None,
        )
    };
    let mut read = fun_expr(
        "ambiguous",
        vec!["T"],
        vec![("receiver", ty_named("T"))],
        Some(ty_named("Int")),
        method_call(var("receiver"), "value", Vec::new()),
    );
    let Decl::Function(read_decl) = &mut read else {
        unreachable!()
    };
    read_decl.type_params[0] = upper("T", ty_named("Left"));
    read_decl.where_clause = Some(where_clause(vec![(
        "T",
        ast::TypeBound::Upper(ty_named("Right")),
    )]));
    let errors = lower_user(file(vec![
        interface_decl("Left", vec![method()]),
        interface_decl("Right", vec![method()]),
        read,
        fun("main", Vec::new()),
    ]))
    .expect_err("unrelated bounds retain distinct member identities");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "call to `value` is ambiguous")
    );
}

#[test]
fn interface_inheritance_cycle_is_rejected_at_hir() {
    let mut left = interface_decl("Left", Vec::new());
    let Decl::Interface(left_decl) = &mut left else {
        unreachable!()
    };
    left_decl.parents = vec![ty_named("Right")];
    let mut right = interface_decl("Right", Vec::new());
    let Decl::Interface(right_decl) = &mut right else {
        unreachable!()
    };
    right_decl.parents = vec![ty_named("Left")];
    let errors = lower_user(file(vec![left, right, fun("main", Vec::new())]))
        .expect_err("interface inheritance cycles must be diagnosed");
    assert!(errors.iter().any(|error| {
        error.message == "interface `Left` directly or indirectly inherits from itself"
    }));
    assert!(errors.iter().any(|error| {
        error.message == "interface `Right` directly or indirectly inherits from itself"
    }));
}
