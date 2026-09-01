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
