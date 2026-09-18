use super::*;
use crate::tests::type_param;

#[test]
fn getter_setter_and_update_support_explicit_and_implicit_receivers() {
    let fixture = DependencyPropertyFixture::new(vec![computed_extension_property(
        "score",
        ty_named("Int"),
        int_lit(1),
        Some(Vec::new()),
    )]);
    let explicit_update = Expr::Update {
        place: scoop_ast::PlaceExpr::Field {
            receiver: Box::new(int_lit(3)),
            name: ident("score"),
            span: sp(),
        },
        op: scoop_ast::UpdateOp::Increment,
        notation: scoop_ast::UpdateNotation::Postfix,
        span: sp(),
    };
    let mut consumer = file(vec![
        integer_increment_extension(),
        fun_expr(
            "readExplicit",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            field(int_lit(1), "score"),
        ),
        fun(
            "writeExplicit",
            vec![assign_field(int_lit(2), "score", int_lit(9))],
        ),
        fun_expr(
            "updateExplicit",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            explicit_update,
        ),
        integer_extension_expr("readImplicit", var("score")),
        integer_extension_block("writeImplicit", vec![assign("score", int_lit(7))]),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "score"]));

    fixture.inspect(consumer, |output| {
        let output = output.expect("dependency extension properties must lower on both paths");
        assert_eq!(output.imported_dependencies().callable_count(), 2);
        let dump = scoop_hir::dump(&output.output().export);
        assert_eq!(dump.matches("ImportedDependencyCall").count(), 6, "{dump}");
        assert_eq!(
            output
                .imported_dependencies()
                .callables()
                .filter(|callable| {
                    matches!(
                        callable.capability().declaration(),
                        scoop_identity::DependencyCallableDeclarationId::PropertyAccessor(_)
                    )
                })
                .count(),
            2
        );
    });
}

#[test]
fn dependency_extension_property_and_invoke_compose_in_one_call() {
    let invoke = extension_expr(
        ty_named("Int"),
        "invoke",
        Vec::new(),
        vec![("flag", ty_named("Boolean"))],
        Some(ty_named("Int")),
        this_expr(),
    );
    let fixture = DependencyPropertyFixture::with_core_types(
        vec![
            computed_extension_property("route", ty_named("Int"), this_expr(), None),
            as_operator(invoke),
        ],
        &["Boolean", "Int"],
    );
    let mut consumer = file(vec![fun_expr(
        "callRoute",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        method_call(int_lit(3), "route", vec![bool_lit(true)]),
    )]);
    consumer.imports.extend([
        exact_import(&["dependency", "api", "route"]),
        exact_import(&["dependency", "api", "invoke"]),
    ]);

    fixture.inspect(consumer, |output| {
        let output = output.expect("dependency property and extension invoke must compose");
        assert_eq!(output.imported_dependencies().callable_count(), 2);
        let dump = scoop_hir::dump(&output.output().export);
        assert_eq!(dump.matches("ImportedDependencyCall").count(), 2, "{dump}");
    });
}

#[test]
fn direct_assignment_selects_only_the_dependency_setter() {
    let fixture = DependencyPropertyFixture::new(vec![computed_extension_property(
        "score",
        ty_named("Int"),
        int_lit(1),
        Some(Vec::new()),
    )]);
    let mut consumer = file(vec![fun(
        "write",
        vec![assign_field(int_lit(2), "score", int_lit(9))],
    )]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "score"]));

    fixture.inspect(consumer, |output| {
        let output = output.expect("a dependency extension setter must lower");
        assert_eq!(output.imported_dependencies().callable_count(), 1);
        let dump = scoop_hir::dump(&output.output().export);
        assert_eq!(dump.matches("ImportedDependencyCall").count(), 1, "{dump}");
    });
}

#[test]
fn read_only_extension_property_rejects_explicit_assignment() {
    let fixture = DependencyPropertyFixture::new(vec![computed_extension_property(
        "score",
        ty_named("Int"),
        int_lit(1),
        None,
    )]);
    let mut consumer = file(vec![fun(
        "write",
        vec![assign_field(int_lit(2), "score", int_lit(9))],
    )]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "score"]));

    fixture.inspect(consumer, |output| {
        let diagnostics = match output {
            Ok(_) => panic!("a read-only dependency extension property cannot be assigned"),
            Err(diagnostics) => diagnostics,
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot assign to immutable property `score`")
        }));
    });
}

#[test]
fn generic_extension_property_reports_the_generic_capability_gate() {
    let fixture = DependencyPropertyFixture::new(vec![generic_extension_property("identity")]);
    let mut consumer = file(vec![fun_expr(
        "read",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        field(int_lit(1), "identity"),
    )]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "identity"]));

    fixture.inspect(consumer, |output| {
        let diagnostics = match output {
            Ok(_) => panic!("a generic dependency extension property needs M23-7"),
            Err(diagnostics) => diagnostics,
        };
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("SCOOP_HIR_CROSS_CONE_GENERIC_REQUIRED")
        }));
    });
}

#[test]
fn inapplicable_exact_dependency_extension_falls_through_to_current_package() {
    let fixture = DependencyPropertyFixture::with_core_types(
        vec![computed_extension_property_on(
            "String",
            "score",
            ty_named("Int"),
            int_lit(1),
            None,
        )],
        &["Int", "String"],
    );
    let mut consumer = file(vec![
        computed_extension_property("score", ty_named("Int"), int_lit(2), None),
        fun_expr(
            "read",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            field(int_lit(1), "score"),
        ),
    ]);
    consumer
        .imports
        .push(exact_import(&["dependency", "api", "score"]));

    fixture.inspect(consumer, |output| {
        let output = output.expect("an inapplicable dependency layer must fall through");
        assert_eq!(output.imported_dependencies().callable_count(), 0);
        let dump = scoop_hir::dump(&output.output().export);
        assert!(!dump.contains("ImportedDependencyCall"), "{dump}");
    });
}

fn integer_extension_expr(name: &str, body: Expr) -> Decl {
    extension_expr(
        ty_named("Int"),
        name,
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        body,
    )
}

fn integer_extension_block(name: &str, statements: Vec<Statement>) -> Decl {
    let Decl::Function(mut function) = fun(name, statements) else {
        unreachable!("fun builds a function")
    };
    function.receiver_ty = Some(ty_named("Int"));
    Decl::Function(function)
}

fn computed_extension_property(
    name: &str,
    ty: TypeRef,
    getter: Expr,
    setter: Option<Vec<Statement>>,
) -> Decl {
    computed_extension_property_on("Int", name, ty, getter, setter)
}

fn computed_extension_property_on(
    receiver: &str,
    name: &str,
    ty: TypeRef,
    getter: Expr,
    setter: Option<Vec<Statement>>,
) -> Decl {
    let Decl::Global(mut property) = computed_property(name, ty, getter, setter) else {
        unreachable!("computed_property builds a global property")
    };
    property.receiver_ty = Some(ty_named(receiver));
    Decl::Global(property)
}

fn generic_extension_property(name: &str) -> Decl {
    let Decl::Global(mut property) = computed_property(name, ty_named("T"), this_expr(), None)
    else {
        unreachable!("computed_property builds a global property")
    };
    property.receiver_ty = Some(ty_named("T"));
    property.type_params = vec![type_param("T")];
    Decl::Global(property)
}

fn as_operator(mut declaration: Decl) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("the test operator declaration is a function")
    };
    function.operator = Some(scoop_ast::OperatorModifier { span: sp() });
    declaration
}
