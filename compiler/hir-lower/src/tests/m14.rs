//! M14 structured generic constraints and interface upper-bound validation.

use scoop_ast as ast;
use scoop_hir as hir;

use super::*;

#[test]
fn function_types_own_unique_canonical_types_in_both_hir_products() {
    let ordinary = ty_function(false, vec![ty_named("Int")], ty_named("String"));
    let suspended = ty_function(true, vec![ty_named("Int")], ty_named("String"));
    let output = lower_user_output(file(vec![
        fun_sig(
            "useFunctions",
            vec![],
            vec![
                ("first", ordinary.clone()),
                ("second", ordinary),
                ("suspended", suspended),
            ],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect("canonical function types must lower");

    for (function_type, signature) in output.export.function_types.iter() {
        assert!(matches!(
            output.export.types[signature.canonical_type],
            hir::Type::Function(found) if found == function_type
        ));
        assert_eq!(
            output
                .export
                .types
                .iter()
                .filter(
                    |(_, ty)| matches!(ty, hir::Type::Function(found) if *found == function_type)
                )
                .count(),
            1
        );
    }

    for (function_type, signature) in output.local.function_types.iter() {
        assert!(matches!(
            &output.local.types[signature.canonical_type].kind,
            hir::concrete::TypeKind::Function(found) if *found == function_type
        ));
        assert_eq!(
            output
                .local
                .types
                .iter()
                .filter(|(_, ty)| matches!(&ty.kind, hir::concrete::TypeKind::Function(found) if *found == function_type))
                .count(),
            1
        );
    }

    let export_use = output
        .export
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == "useFunctions")
        .expect("export function");
    assert_eq!(export_use.params[0].ty, export_use.params[1].ty);
    assert_ne!(export_use.params[0].ty, export_use.params[2].ty);

    let local_use = output
        .local
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == "useFunctions")
        .expect("local function");
    assert_eq!(local_use.params[0].ty, local_use.params[1].ty);
    assert_ne!(local_use.params[0].ty, local_use.params[2].ty);
}

#[test]
fn compiler_exception_core_is_complete_in_export_and_local_hir() {
    let output = lower_user_output(file(vec![fun("main", vec![])]))
        .expect("the canonical compiler exception core must lower");

    let export = output.export.exception_core;
    let export_names = [
        export.throwable,
        export.unwrap_exception,
        export.class_cast_exception,
        export.arithmetic_exception,
        export.index_out_of_bounds_exception,
        export.illegal_state_exception,
    ]
    .map(|exception| output.export.classes[exception.class()].name.as_str());
    assert_eq!(
        export_names,
        [
            "Throwable",
            "UnwrapException",
            "ClassCastException",
            "ArithmeticException",
            "IndexOutOfBoundsException",
            "IllegalStateException",
        ]
    );

    let local = output.local.exception_core;
    let local_names = [
        local.throwable,
        local.unwrap_exception,
        local.class_cast_exception,
        local.arithmetic_exception,
        local.index_out_of_bounds_exception,
        local.illegal_state_exception,
    ]
    .map(|exception| output.local.classes[exception.class()].name.as_str());
    assert_eq!(local_names, export_names);
}

#[test]
fn compiler_exception_core_rejects_missing_or_nonzero_arg_targets() {
    let mut missing = core_file();
    missing.declarations.retain(
        |declaration| !matches!(declaration, Decl::Class(class) if class.name.text == "UnwrapException"),
    );
    let errors = lower(&[missing, file(vec![fun("main", vec![])])])
        .expect_err("a missing compiler exception must reject the core");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "scoop.core must define class `UnwrapException`")
    );

    let mut malformed = core_file();
    let declaration = malformed
        .declarations
        .iter_mut()
        .find(|declaration| {
            matches!(declaration, Decl::Class(class) if class.name.text == "ArithmeticException")
        })
        .expect("ArithmeticException declaration");
    *declaration = class_decl(
        ast::ClassModifier::Final,
        "ArithmeticException",
        vec![(false, "code", ty_named("Int"))],
        Some(("Throwable", vec![])),
        vec![],
        vec![],
    );
    let errors = lower(&[malformed, file(vec![fun("main", vec![])])])
        .expect_err("a nonzero-argument compiler exception must reject the core");
    assert!(errors.iter().any(|error| {
        error.message
            == "class `ArithmeticException` in scoop.core must be a non-generic final subtype of `Throwable` with a zero-argument constructor"
    }));
}

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

fn operator_equals(is_override: bool, bodyless: bool, other: TypeRef) -> ast::FunctionDecl {
    let mut method = method_full(
        is_override,
        bodyless,
        "equals",
        vec![("other", other)],
        Some(ty_named("Boolean")),
        if bodyless {
            FunctionBody::None
        } else {
            FunctionBody::Expr(Box::new(bool_lit(true)))
        },
    );
    method.operator = Some(ast::OperatorModifier { span: sp() });
    method
}

#[test]
fn operator_equals_is_a_typed_hir_contract() {
    let output = lower_user_output(file(vec![
        interface_decl(
            "EqualTo",
            vec![operator_equals(false, true, ty_named("EqualTo"))],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Value",
            Vec::new(),
            None,
            vec!["EqualTo"],
            vec![operator_equals(true, false, ty_named("EqualTo"))],
        ),
        fun(
            "main",
            vec![
                val("left", call("Value", Vec::new())),
                val("right", call("Value", Vec::new())),
                val("same", binary(BinOp::Eq, var("left"), var("right"))),
                val("different", binary(BinOp::Ne, var("left"), var("right"))),
            ],
        ),
    ]))
    .expect("operator identity must participate in ordinary interface conformance");

    let methods = output
        .export
        .functions
        .iter()
        .filter(|(_, function)| matches!(function.name.as_str(), "EqualTo.equals" | "Value.equals"))
        .map(|(_, function)| {
            function
                .method
                .expect("equals declarations are methods")
                .operator
        })
        .collect::<Vec<_>>();
    assert_eq!(
        methods,
        vec![
            Some(hir::OperatorKind::Equals),
            Some(hir::OperatorKind::Equals)
        ]
    );
    assert!(hir::dump(&output.export).contains("operator fun equals(other: EqualTo): Boolean"));
    assert!(output.local.functions.iter().any(|(_, function)| {
        function.name == "Value.equals"
            && function
                .method
                .is_some_and(|method| method.operator == Some(hir::OperatorKind::Equals))
    }));
    let dump = hir::dump(&output.export);
    assert!(dump.contains("MethodCall Value.equals : Boolean"), "{dump}");
    assert!(
        dump.contains("Unary Not : Boolean\n        MethodCall Value.equals : Boolean"),
        "{dump}"
    );

    let plain_equals = method_full(
        true,
        false,
        "equals",
        vec![("other", ty_named("EqualTo"))],
        Some(ty_named("Boolean")),
        FunctionBody::Expr(Box::new(bool_lit(true))),
    );
    let errors = lower_user(file(vec![
        interface_decl(
            "EqualTo",
            vec![operator_equals(false, true, ty_named("EqualTo"))],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Plain",
            Vec::new(),
            None,
            vec!["EqualTo"],
            vec![plain_equals],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("the operator bit is part of the interface method contract");
    assert!(errors.iter().any(|error| {
        error.message == "`equals` must have the same `operator` modifier as `EqualTo.equals`"
    }));
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("does not implement interface method `EqualTo.equals`")
    }));
}

#[test]
fn generic_equality_resolves_the_exact_operator_bound_member() {
    let equality = generic_interface_decl(
        "Equality",
        vec![(ast::Variance::Invariant, "T")],
        vec![operator_equals(false, true, ty_named("T"))],
    );
    let mut value = struct_decl_full(
        "Value",
        Vec::new(),
        Vec::new(),
        vec![operator_equals(true, false, ty_named("Value"))],
    );
    let Decl::Struct(value_decl) = &mut value else {
        unreachable!()
    };
    value_decl.interfaces = vec![ty_generic("Equality", vec![ty_named("Value")])];

    let mut equal = fun_expr(
        "equal",
        vec!["T"],
        vec![("left", ty_named("T")), ("right", ty_named("T"))],
        Some(ty_named("Boolean")),
        binary(BinOp::Eq, var("left"), var("right")),
    );
    let Decl::Function(equal_decl) = &mut equal else {
        unreachable!()
    };
    equal_decl.type_params[0] = upper("T", ty_generic("Equality", vec![ty_named("T")]));

    let output = lower_user_output(file(vec![
        equality,
        value,
        equal,
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call(
                    "equal",
                    vec![
                        struct_init("Value", Vec::new()),
                        struct_init("Value", Vec::new()),
                    ],
                )],
            ))],
        ),
    ]))
    .expect("the F-bound exposes its exact operator member");

    let dump = hir::dump(&output.export);
    assert!(
        dump.contains("MethodCall bound T0 via Equality<T0> -> Equality.equals : Boolean"),
        "{dump}"
    );
    let concrete = output
        .local
        .functions
        .iter()
        .find(|(_, function)| {
            function.name == "equal"
                && matches!(
                    function.origin,
                    hir::concrete::FunctionOrigin::Free(
                        hir::concrete::FreeFunctionOrigin::Generic { .. }
                    )
                )
        })
        .expect("equal<Value> specialization")
        .1;
    let hir::concrete::FunctionKind::User(body) = &concrete.kind else {
        panic!("equal<Value> has a concrete body")
    };
    let hir::concrete::StatementKind::Return {
        value:
            Some(hir::concrete::Expr {
                kind: hir::concrete::ExprKind::MethodCall { callee, .. },
                ..
            }),
    } = &body.statements[0].kind
    else {
        panic!("the concrete body returns the resolved operator call")
    };
    let target = output.local.callable_function(*callee);
    assert_eq!(output.local.functions[target].name, "Value.equals");
}

#[test]
fn operator_equals_legality_is_checked_at_its_declaration() {
    let mut wrong_name = operator_equals(false, false, ty_named("Bad"));
    wrong_name.name = ident("compare");
    let mut wrong_arity = operator_equals(false, false, ty_named("Bad"));
    wrong_arity.params.push(ast::Param {
        name: ident("extra"),
        ty: ty_named("Bad"),
        span: sp(),
    });
    let mut wrong_result = operator_equals(false, false, ty_named("Bad"));
    wrong_result.return_ty = Some(ty_named("Int"));
    let mut generic = operator_equals(false, false, ty_named("T"));
    generic.type_params = vec![type_param("T")];
    let mut suspended = operator_equals(false, false, ty_named("Bad"));
    suspended.is_suspend = true;

    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Bad",
            Vec::new(),
            None,
            Vec::new(),
            vec![wrong_name, wrong_arity, wrong_result, generic, suspended],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("every invalid operator shape must be rejected before HIR output");
    for expected in [
        "operator member `compare` is not supported; M14 only defines `equals`",
        "operator `equals` must have exactly one parameter, found 2",
        "operator `equals` must return Boolean, found Int",
        "operator `equals` must not declare type parameters",
        "operator `equals` must not be suspend",
    ] {
        assert!(
            errors.iter().any(|error| error.message == expected),
            "missing diagnostic: {expected}"
        );
    }
}

#[test]
fn operator_modifier_is_rejected_outside_members() {
    let mut top = fun("top", Vec::new());
    let Decl::Function(function) = &mut top else {
        unreachable!()
    };
    function.operator = Some(ast::OperatorModifier { span: sp() });
    let errors = lower_user(file(vec![top, fun("main", Vec::new())]))
        .expect_err("top-level operator functions are not member capabilities");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "`operator` is only allowed on member functions")
    );
}

#[test]
fn intrinsic_type_contract_is_complete_in_export_and_local_hir() {
    let output = lower_user_output(file(vec![fun("main", vec![])]))
        .expect("the core intrinsic type contract must lower");
    let export = &output.export;
    let core = export.intrinsic_type_core;
    for (id, kind, representation) in [
        (
            core.int,
            hir::IntrinsicTypeKind::Int,
            hir::IntrinsicTypeRepresentation::Int,
        ),
        (
            core.uint,
            hir::IntrinsicTypeKind::UInt,
            hir::IntrinsicTypeRepresentation::UInt,
        ),
        (
            core.boolean,
            hir::IntrinsicTypeKind::Boolean,
            hir::IntrinsicTypeRepresentation::Boolean,
        ),
    ] {
        let hir::StructRepresentation::Intrinsic(declaration) = export.structs[id].representation
        else {
            panic!("fixed intrinsic struct must not masquerade as an empty declaration")
        };
        assert_eq!(declaration.kind, kind);
        assert_eq!(declaration.provider, hir::IntrinsicProviderId::from_raw(0));
        assert_eq!(
            export.struct_applications[export.structs[id].self_application].representation,
            hir::StructApplicationRepresentation::Intrinsic(representation)
        );
    }

    let hir::ClassRepresentation::Intrinsic(string_declaration) =
        export.classes[core.string].representation
    else {
        panic!("String must have an explicit intrinsic representation")
    };
    assert_eq!(string_declaration.kind, hir::IntrinsicTypeKind::String);
    assert!(matches!(
        export.class_applications[export.classes[core.string].self_application].representation,
        hir::ClassApplicationRepresentation::Intrinsic(hir::IntrinsicTypeRepresentation::String)
    ));
    for (id, mutable) in [(core.array, false), (core.mutable_array, true)] {
        let application = &export.class_applications[export.classes[id].self_application];
        let element = match &application.representation {
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Array { element },
            ) if !mutable => element,
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::MutableArray { element },
            ) if mutable => element,
            _ => panic!("generic intrinsic family must carry its element type"),
        };
        assert!(matches!(export.types[*element], hir::Type::Param(_)));
    }

    let local = &output.local;
    for (id, expected) in [
        (
            local.intrinsic_type_core.int,
            hir::concrete::IntrinsicTypeRepresentation::Int,
        ),
        (
            local.intrinsic_type_core.uint,
            hir::concrete::IntrinsicTypeRepresentation::UInt,
        ),
        (
            local.intrinsic_type_core.boolean,
            hir::concrete::IntrinsicTypeRepresentation::Boolean,
        ),
    ] {
        assert!(matches!(
            &local.structs[id].representation,
            hir::concrete::StructRepresentation::Intrinsic { application, .. }
                if application == &expected
        ));
    }
    assert!(matches!(
        local.classes[local.intrinsic_type_core.string].representation,
        hir::concrete::ClassRepresentation::Intrinsic {
            application: hir::concrete::IntrinsicTypeRepresentation::String,
            ..
        }
    ));
}

#[test]
fn intrinsic_type_shape_is_validated_at_its_source() {
    let mut core = core_file();
    let int = core
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Struct(declaration) if declaration.name.text == "Int" => Some(declaration),
            _ => None,
        })
        .expect("core Int declaration");
    int.fields = ast::StructRepresentationDecl::Declared(Vec::new());
    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("an intrinsic type must omit its compiler representation");
    assert!(errors.iter().any(|error| {
        error.file == 0
            && error.message
                == "intrinsic type `core_int` must omit its fields or primary constructor"
    }));
}

#[test]
fn allowlisted_type_provider_preserves_provenance_without_relaxing_shape() {
    let mut core = core_file();
    let int_index = core
        .declarations
        .iter()
        .position(
            |declaration| matches!(declaration, Decl::Struct(declaration) if declaration.name.text == "Int"),
        )
        .expect("core Int declaration");
    let int = core.declarations.remove(int_index);
    let user = file(vec![int, fun("main", vec![])]);
    let core_provider = hir::IntrinsicProviderId::from_raw(3);
    let test_provider = hir::IntrinsicProviderId::from_raw(7);
    let unit = CompilationUnit {
        core: vec![ProviderSource {
            source: &core,
            provider: core_provider,
        }],
        user: ProviderSource {
            source: &user,
            provider: test_provider,
        },
    };
    let output = lower_compilation_unit(
        &unit,
        IntrinsicDeclarationPolicy::AllowListedForTesting {
            providers: std::collections::HashSet::from([test_provider]),
        },
    )
    .expect("the internal allowlist grants only declaration authority");
    let hir::StructRepresentation::Intrinsic(declaration) =
        output.export.structs[output.export.intrinsic_type_core.int].representation
    else {
        panic!("the allowlisted declaration remains typed")
    };
    assert_eq!(declaration.provider, test_provider);
}

#[test]
fn intrinsic_types_do_not_acquire_zero_argument_source_constructors() {
    let errors = lower_user_output(file(vec![fun(
        "main",
        vec![stmt(call("Int", vec![])), stmt(call("String", vec![]))],
    )]))
    .expect_err("hidden intrinsic construction is not a source constructor");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "intrinsic struct `Int` has no source constructor")
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message == "intrinsic class `String` has no source constructor")
    );
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

    let bound = output
        .export
        .bound_callable_refs
        .iter()
        .map(|(_, bound)| bound)
        .find(|bound| {
            let member = output.export.interface_methods[bound.member];
            output.export.interfaces[member.owner].name == "Show"
        })
        .expect("read<T> has one non-core bound call");
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

    let bound = output
        .export
        .bound_callable_refs
        .iter()
        .map(|(_, bound)| bound)
        .find(|bound| {
            let member = output.export.interface_methods[bound.member];
            output.export.interfaces[member.owner].name == "Parent"
        })
        .expect("readParent<T> has one non-core bound call");
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
