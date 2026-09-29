use super::*;

#[test]
fn interface_bounds_are_complete_ordered_hir_constraints() {
    let mut renderable = generic_interface_decl("Renderable", vec!["T"], vec![]);
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
        generic_interface_decl("Comparable", vec!["T"], vec![]),
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
    let hir::TypeParamBounds::Nominal(bounds) = &declaration.type_params[0].bounds else {
        panic!("interface upper bounds must use the typed constraint branch")
    };
    assert!(bounds.class.is_none());
    assert_eq!(bounds.interfaces.len(), 2);
    let [
        hir::InterfaceUpperBound::Local(marker),
        hir::InterfaceUpperBound::Local(comparable),
    ] = bounds.interfaces.as_slice()
    else {
        panic!("both interface bounds are declared in this source module")
    };
    assert_eq!(
        hir::type_name(
            &module,
            module.interface_applications[marker.application].canonical_type
        ),
        "Marker"
    );
    assert_eq!(
        hir::type_name(
            &module,
            module.interface_applications[comparable.application].canonical_type
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
        error
            .message
            .contains("fun boundedIdentity<T : Marker>(value: T): T")
            && error
                .message
                .contains("type argument `Int` for `T` must satisfy interface upper bound `Marker`")
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
            == "type parameter `T` of function cannot combine nominal upper bounds with a kind bound"
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
        error.message
            == "upper bound of type parameter `T` must be a class or interface, found `Int`"
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
