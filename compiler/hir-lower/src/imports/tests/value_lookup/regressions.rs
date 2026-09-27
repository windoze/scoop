use super::*;
use crate::namespace::TopLevelLookupLayer;
use crate::tests::{block, bool_lit, class_decl, fun_sig, struct_decl};

fn conditional(value: ast::Expr) -> ast::Expr {
    ast::Expr::If(Box::new(ast::If {
        cond: bool_lit(true),
        then_block: block(vec![stmt(value.clone())]),
        else_block: Some(block(vec![stmt(value)])),
        span: sp(),
    }))
}

#[test]
fn non_value_bindings_occupy_exact_current_and_star_before_lower_values() {
    let blockers = [
        fun("Chosen", Vec::new()),
        class_decl(
            ast::ClassModifier::Final,
            "Chosen",
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        ),
        ast::Decl::TypeAlias(ast::TypeAliasDecl {
            visibility: ast::VisibilitySyntax::Omitted,
            name: ident("Chosen"),
            target: ty_named("Int"),
            span: sp(),
        }),
    ];
    for blocker in blockers {
        for exact_layer in [true, false] {
            let library = package(file(vec![blocker.clone()]), &["api"]);
            let mut user = read("Chosen");
            user.imports.push(if exact_layer {
                exact(&["api", "Chosen"], None, false)
            } else {
                star(&["api"], false)
            });
            if exact_layer {
                user.declarations
                    .push(ast::Decl::Global(constant("Chosen")));
            }
            let errors = lower_sources(vec![library, user])
                .expect_err("a non-value name does not expose a lower value");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("not a value")),
                "{errors:#?}"
            );
            assert!(
                !errors
                    .iter()
                    .any(|error| error.message.contains("unknown variable"))
            );
        }
        let library = package(file(vec![ast::Decl::Global(constant("Chosen"))]), &["api"]);
        let mut user = read("Chosen");
        user.declarations.push(blocker);
        user.imports.push(star(&["api"], false));
        let errors =
            lower_sources(vec![library, user]).expect_err("current non-value precedes star value");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("not a value"))
        );
    }
}

#[test]
fn same_layer_values_win_over_legal_function_and_type_names() {
    let library = package(
        file(vec![
            ast::Decl::Global(constant("Chosen")),
            fun("Chosen", Vec::new()),
            fun_sig(
                "Chosen",
                Vec::new(),
                vec![("x", ty_named("Int"))],
                None,
                Vec::new(),
            ),
        ]),
        &["api"],
    );
    for exact_layer in [false, true] {
        let mut user = read("Chosen");
        user.imports.push(if exact_layer {
            exact(&["api", "Chosen"], None, false)
        } else {
            star(&["api"], false)
        });
        let output = lower_sources(vec![user, library.clone()])
            .expect("one value and overloaded functions legally share a name");
        assert!(matches!(
            first_init(&output).kind,
            hir::ExprKind::IntegerLiteral(_)
        ));
    }
}

#[test]
fn overloaded_function_blocker_reports_non_value_without_selecting_an_overload() {
    let library = package(
        file(vec![
            fun("Chosen", Vec::new()),
            fun_sig(
                "Chosen",
                Vec::new(),
                vec![("x", ty_named("Int"))],
                None,
                Vec::new(),
            ),
        ]),
        &["api"],
    );
    let mut user = read("Chosen");
    user.imports.push(star(&["api"], false));
    let errors =
        lower_sources(vec![library, user]).expect_err("overload group is not an ordinary value");
    let error = errors
        .iter()
        .find(|error| error.message.contains("function `Chosen` is not a value"))
        .expect("non-value diagnostic");
    assert_eq!(error.notes.len(), 2);
}

#[test]
fn blockers_prevent_assignment_place_const_and_static_initializer_fallback() {
    let library = package(file(vec![fun("Chosen", Vec::new())]), &["api"]);
    let operations = [
        vec![fun("main", vec![assign("Chosen", int_lit(9))])],
        vec![fun(
            "main",
            vec![val(
                "old",
                ast::Expr::Update {
                    place: ast::PlaceExpr::Name(ident("Chosen")),
                    op: ast::UpdateOp::Increment,
                    notation: ast::UpdateNotation::Postfix,
                    span: sp(),
                },
            )],
        )],
        vec![
            ast::Decl::Global(stored("result", ty_named("Int"), var("Chosen"))),
            fun("main", Vec::new()),
        ],
        vec![
            ast::Decl::Global(ast::PropertyDecl {
                body: ast::PropertyBodySyntax::Const(Box::new(var("Chosen"))),
                ..constant("result")
            }),
            fun("main", Vec::new()),
        ],
    ];
    for mut declarations in operations {
        declarations.push(ast::Decl::Global(constant("Chosen")));
        let mut user = file(declarations);
        user.imports.push(exact(&["api", "Chosen"], None, false));
        let errors = lower_sources(vec![user, library.clone()])
            .expect_err("all non-call uses share the occupied layer");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("not a value")
                    || error.message.contains("not const")),
            "{errors:#?}"
        );
    }
}

#[test]
fn core_non_value_bindings_block_contextual_variant_fallback() {
    let extension = ast::Decl::Global(ast::PropertyDecl {
        receiver_ty: Some(ty_named("Int")),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(int_lit(3))),
                span: sp(),
            }),
            setter: None,
        }),
        ..constant("Ready")
    });
    for blocker in [
        fun("Ready", Vec::new()),
        struct_decl("Ready", Vec::new()),
        extension,
    ] {
        let mut core = crate::tests::core_file();
        let mut blocker = blocker;
        let public = ast::VisibilitySyntax::Explicit {
            visibility: ast::DeclaredVisibility::Public,
            span: sp(),
        };
        match &mut blocker {
            ast::Decl::Function(value) => value.visibility = public,
            ast::Decl::Struct(value) => value.visibility = public,
            ast::Decl::Global(value) => value.visibility = public,
            _ => unreachable!(),
        }
        core.declarations.push(blocker);
        let user = file(vec![
            enum_decl("State", Vec::new(), vec![variant_unit("Ready")]),
            fun(
                "main",
                vec![val_ty("chosen", Some(ty_named("State")), var("Ready"))],
            ),
        ]);
        let errors = lower_sources_with_core(vec![user], core)
            .expect_err("prelude occupancy precedes contextual enum");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("not a value")
                    || error.message.contains("requires a receiver")),
            "{errors:#?}"
        );
    }
}

#[test]
fn singleton_assignment_materializes_receiver_before_rhs_control_flow() {
    let library = package(
        file(vec![object(
            "Tools",
            vec![ast::ClassMember::StoredProperty(stored(
                "counter",
                ty_named("Int"),
                int_lit(0),
            ))],
        )]),
        &["api"],
    );
    let mut user = file(vec![fun(
        "main",
        vec![assign("counter", conditional(int_lit(4)))],
    )]);
    user.imports.push(star(&["api", "Tools"], false));
    let output = lower_sources(vec![user, library]).expect("imported property assignment");
    let main = output
        .export
        .functions
        .iter()
        .find_map(|(_, f)| (f.name == "main").then_some(f))
        .unwrap();
    let hir::FunctionKind::User(body) = &main.kind else {
        unreachable!()
    };
    let receiver = body.statements.iter().position(|statement| matches!(&statement.kind, hir::StatementKind::ValDecl { init, .. } if matches!(init.kind, hir::ExprKind::SingletonValue(_)))).expect("receiver is materialized");
    let rhs = body
        .statements
        .iter()
        .position(|statement| matches!(statement.kind, hir::StatementKind::If { .. }))
        .expect("RHS has control flow");
    assert!(
        receiver < rhs,
        "singleton initialization must occur before RHS evaluation"
    );
}

#[test]
fn failed_named_property_write_does_not_commit_rhs_sink() {
    let (mut state, _, _) = lowerer();
    let access = state.top_level_access(ast::VisibilitySyntax::Omitted, sp(), "property", 0);
    let capability = state.allocate_const_capability(access.clone(), sp());
    let property = state.properties.alloc(hir::Property {
        owner: hir::PropertyOwner::TopLevel,
        name: "flag".to_string(),
        access: access.clone(),
        modifier: hir::MethodModifier::Final,
        is_override: false,
        overrides: Vec::new(),
        ty: state.boolean,
        capability,
        representation: hir::PropertyRepresentation::Const {
            value: hir::ConstPropertyValue::Boolean(false),
        },
        span: sp(),
    });
    state.property_files.insert(property, 0);
    let TopLevelLookupLayer::CurrentPackage(package) =
        state.top_level_namespaces.source_namespace(0)
    else {
        unreachable!()
    };
    let source = state.visibility_file(0);
    state.imports.insert(
        ResolvedNamespace::Package(package),
        CurrentUnitBinding {
            target: CurrentUnitTarget::Property(property),
            source,
            file: 0,
            span: sp(),
            access: access.lookup,
            name: "flag".to_string(),
        },
    );
    state.imports.files = vec![FrozenFileImports::default(); 3];
    let statements = state.lower_block(&block(vec![assign("flag", conditional(bool_lit(true)))]));
    assert!(state.diagnostics.iter().any(|error| {
        error
            .message
            .contains("cannot assign to immutable property")
    }));
    assert!(
        statements.is_empty(),
        "failed writes cannot leave RHS control flow in the caller's sink: {statements:#?}"
    );
}

#[test]
fn core_const_fallback_enforces_complete_access_domain() {
    for public in [false, true] {
        let mut core = crate::tests::core_file();
        let mut secret = constant("CoreSecret");
        if public {
            secret.visibility = ast::VisibilitySyntax::Explicit {
                visibility: ast::DeclaredVisibility::Public,
                span: sp(),
            };
        }
        core.declarations.push(ast::Decl::Global(secret));
        core.declarations.push(ast::Decl::Global(ast::PropertyDecl {
            body: ast::PropertyBodySyntax::Const(Box::new(var("CoreSecret"))),
            ..constant("CoreOwnUse")
        }));
        let user = file(vec![
            ast::Decl::Global(ast::PropertyDecl {
                body: ast::PropertyBodySyntax::Const(Box::new(var("CoreSecret"))),
                ..constant("UserUse")
            }),
            fun("main", Vec::new()),
        ]);
        let result = lower_sources_with_core(vec![user], core);
        if public {
            result.expect("public core const is visible");
        } else {
            let errors = result.expect_err("internal core const is not current-unit internal");
            assert!(
                errors
                    .iter()
                    .any(|error| error.file == 1 && error.message.contains("not accessible")),
                "{errors:#?}"
            );
            assert!(
                !errors.iter().any(|error| error.file == 0),
                "same-provider core uses remain valid"
            );
        }
    }
}
