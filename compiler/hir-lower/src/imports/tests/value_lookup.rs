use super::*;
mod regressions;
use crate::tests::{
    assign, enum_decl, fun, fun_expr, int_lit, stmt, ty_named, ty_nullable, val, val_ty, var,
    variant_unit,
};

fn object(name: &str, members: Vec<ast::ClassMember>) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members,
        span: sp(),
    })
}

fn stored(name: &str, ty: ast::TypeRef, expression: ast::Expr) -> ast::PropertyDecl {
    ast::PropertyDecl {
        mutable: true,
        ty,
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(expression),
            accessors: ast::AccessorSyntax::default(),
        },
        ..constant(name)
    }
}

fn read(name: &str) -> ast::SourceFile {
    file(vec![fun("main", vec![val("chosen", var(name))])])
}

fn first_init(output: &hir::Output) -> &hir::Expr {
    let function = output
        .export
        .functions
        .iter()
        .find_map(|(_, f)| (f.name == "main").then_some(f))
        .unwrap();
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("main has a body")
    };
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::ValDecl { init, .. } => Some(init),
            _ => None,
        })
        .expect("main initializes a value")
}

#[test]
fn exact_values_shadow_other_kinds_in_lower_layers_independently_of_source_order() {
    for object_wins in [true, false] {
        let library = package(
            file(vec![if object_wins {
                object("Chosen", Vec::new())
            } else {
                ast::Decl::Global(constant("Chosen"))
            }]),
            &["api"],
        );
        let mut user = read("Chosen");
        user.declarations.push(if object_wins {
            ast::Decl::Global(constant("Chosen"))
        } else {
            object("Chosen", Vec::new())
        });
        user.imports.push(exact(&["api", "Chosen"], None, false));
        for sources in [
            vec![library.clone(), user.clone()],
            vec![user.clone(), library.clone()],
        ] {
            let output = lower_sources(sources)
                .expect("exact value precedes current package regardless of kind");
            assert_eq!(
                matches!(first_init(&output).kind, hir::ExprKind::SingletonValue(_)),
                object_wins
            );
            assert_eq!(
                matches!(first_init(&output).kind, hir::ExprKind::IntegerLiteral(_)),
                !object_wins
            );
        }
    }
}

#[test]
fn value_layers_cover_current_star_and_core_and_deduplicate_origins() {
    let library = package(
        file(vec![enum_decl(
            "State",
            Vec::new(),
            vec![variant_unit("None")],
        )]),
        &["api"],
    );
    let mut user = read("None");
    user.imports = vec![
        star(&["api", "State"], false),
        star(&["api", "State"], false),
    ];
    let output =
        lower_sources(vec![library.clone(), user.clone()]).expect("star variant precedes core");
    let hir::ExprKind::VariantConstruct { variant, .. } = first_init(&output).kind else {
        panic!("variant value")
    };
    assert_eq!(
        output.export.enums[variant.declaration().enumeration()].name,
        "State"
    );
    user.imports.extend([
        exact(&["api", "State", "None"], None, false),
        exact(&["api", "State", "None"], None, false),
    ]);
    lower_sources(vec![user.clone(), library.clone()])
        .expect("duplicate exact and star origins remain unique");
    user.imports
        .retain(|import| matches!(import, ast::ImportSyntax::Star { .. }));
    user.declarations.push(ast::Decl::Global(constant("None")));
    let output =
        lower_sources(vec![library, user]).expect("current property precedes star variant");
    assert!(matches!(
        first_init(&output).kind,
        hir::ExprKind::IntegerLiteral(_)
    ));
    lower_sources(vec![file(vec![fun(
        "main",
        vec![val_ty(
            "chosen",
            Some(ty_nullable(ty_named("Int"))),
            var("None"),
        )],
    )])])
    .expect("existing core prelude is retained");
}

#[test]
fn same_layer_cross_kind_ambiguity_reports_use_and_every_origin() {
    let property = package(file(vec![ast::Decl::Global(constant("Chosen"))]), &["left"]);
    let singleton = package(file(vec![object("Chosen", Vec::new())]), &["right"]);
    for use_exact in [false, true] {
        let mut user = read("Chosen");
        let ast::Decl::Function(main) = &mut user.declarations[0] else {
            unreachable!()
        };
        let ast::FunctionBody::Block(body) = &mut main.body else {
            unreachable!()
        };
        let ast::StatementKind::ValDecl(value) = &mut body.statements[0].kind else {
            unreachable!()
        };
        value.init = ast::Expr::Var(ast::Ident {
            text: "Chosen".to_string(),
            span: ast::Span::new(100, 106),
        });
        user.imports = if use_exact {
            vec![
                exact(&["left", "Chosen"], None, false),
                exact(&["right", "Chosen"], None, false),
            ]
        } else {
            vec![star(&["left"], false), star(&["right"], false)]
        };
        for reverse in [false, true] {
            if reverse {
                user.imports.reverse();
            }
            let sources = if reverse {
                vec![user.clone(), singleton.clone(), property.clone()]
            } else {
                vec![property.clone(), singleton.clone(), user.clone()]
            };
            let errors = lower_sources(sources).expect_err("different value origins are ambiguous");
            let diagnostic = errors
                .iter()
                .find(|error| error.message.contains("value `Chosen` is ambiguous"))
                .expect("value ambiguity");
            assert_eq!(diagnostic.span, Some(ast::Span::new(100, 106)));
            assert_eq!(diagnostic.notes.len(), 2);
            assert_ne!(diagnostic.notes[0].file, diagnostic.notes[1].file);
        }
    }
}

#[test]
fn expected_enum_does_not_disambiguate_imported_variants() {
    let left = package(
        file(vec![enum_decl(
            "Left",
            Vec::new(),
            vec![variant_unit("Ready")],
        )]),
        &["left"],
    );
    let right = package(
        file(vec![enum_decl(
            "Right",
            Vec::new(),
            vec![variant_unit("Ready")],
        )]),
        &["right"],
    );
    let mut user = file(vec![fun(
        "main",
        vec![val_ty("chosen", Some(ty_named("Left")), var("Ready"))],
    )]);
    user.imports = vec![
        exact(&["left", "Left"], None, false),
        star(&["left", "Left"], false),
        star(&["right", "Right"], false),
    ];
    let errors =
        lower_sources(vec![left, right, user]).expect_err("expected type cannot choose an origin");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "value `Ready` is ambiguous in the star import layer")
    );
}

#[test]
fn imported_generic_unit_variant_is_postponed_until_expected_application() {
    let library = package(
        file(vec![enum_decl(
            "State",
            vec!["T"],
            vec![variant_unit("Ready")],
        )]),
        &["api"],
    );
    let mut user = file(vec![fun(
        "main",
        vec![val_ty(
            "chosen",
            Some(ast::TypeRef {
                kind: ast::TypeRefKind::Generic(ident("State"), vec![ty_named("Int")]),
                span: sp(),
            }),
            var("Chosen"),
        )],
    )]);
    user.imports = vec![
        exact(&["api", "State"], None, false),
        exact(&["api", "State", "Ready"], Some("Chosen"), false),
    ];
    lower_sources(vec![user, library])
        .expect("generic imported unit variant consumes its exact expected application");
}

#[test]
fn private_value_does_not_hide_visible_star_and_lexical_binding_still_wins() {
    let mut private = constant("Chosen");
    private.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let private = file(vec![ast::Decl::Global(private)]);
    let library = package(file(vec![object("Chosen", Vec::new())]), &["api"]);
    let mut user = read("Chosen");
    user.imports.push(star(&["api"], false));
    let output = lower_sources(vec![private.clone(), library.clone(), user.clone()])
        .expect("private current candidate is filtered");
    assert!(matches!(
        first_init(&output).kind,
        hir::ExprKind::SingletonValue(_)
    ));
    user.declarations = vec![fun(
        "main",
        vec![val("Chosen", int_lit(3)), val("chosen", var("Chosen"))],
    )];
    lower_sources(vec![private, library, user]).expect("lexical value keeps priority");
}

#[test]
fn real_member_value_still_precedes_exact_import() {
    let library = package(file(vec![object("Chosen", Vec::new())]), &["api"]);
    let ast::Decl::Function(read_member) = fun_expr(
        "read",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        var("Chosen"),
    ) else {
        unreachable!()
    };
    let mut user = file(vec![
        object(
            "Tools",
            vec![
                ast::ClassMember::StoredProperty(stored("Chosen", ty_named("Int"), int_lit(2))),
                ast::ClassMember::Function(read_member),
            ],
        ),
        fun("main", Vec::new()),
    ]);
    user.imports.push(exact(&["api", "Chosen"], None, false));
    lower_sources(vec![user, library])
        .expect("implicit real member property precedes the exact-imported singleton");
}

#[test]
fn selected_value_type_mismatch_does_not_fall_back_to_core_or_contextual_enum() {
    let library = package(file(vec![ast::Decl::Global(constant("None"))]), &["api"]);
    let mut user = file(vec![fun(
        "main",
        vec![val_ty(
            "chosen",
            Some(ty_nullable(ty_named("Int"))),
            var("None"),
        )],
    )]);
    user.imports.push(star(&["api"], false));
    let errors =
        lower_sources(vec![library, user]).expect_err("selected property hard-shadows core None");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("initializer of `chosen`")
                && error.message.contains("found Int"))
    );
}

#[test]
fn imported_singleton_properties_support_reads_writes_updates_and_constants() {
    let library = package(
        file(vec![object(
            "Tools",
            vec![
                ast::ClassMember::StoredProperty(stored("counter", ty_named("Int"), int_lit(0))),
                ast::ClassMember::StoredProperty(constant("version")),
            ],
        )]),
        &["api"],
    );
    let mut user = file(vec![fun(
        "main",
        vec![
            assign("counter", int_lit(2)),
            val(
                "previous",
                ast::Expr::Update {
                    place: ast::PlaceExpr::Name(ident("counter")),
                    op: ast::UpdateOp::Increment,
                    notation: ast::UpdateNotation::Postfix,
                    span: sp(),
                },
            ),
            val("chosen", var("counter")),
            val("revision", var("version")),
        ],
    )]);
    user.imports.push(star(&["api", "Tools"], false));
    lower_sources(vec![user, library])
        .expect("singleton receiver is carried through all property operations");
}

#[test]
fn companion_forwarding_and_direct_import_preserve_one_property_origin() {
    let ast::Decl::Class(mut host) = crate::tests::class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!()
    };
    host.members.push(ast::ClassMember::Companion(Box::new(
        ast::CompanionObjectDecl {
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            name: ast::CompanionNameSyntax::Named(ident("Factory")),
            supertypes: Vec::new(),
            members: vec![
                ast::ClassMember::StoredProperty(stored("counter", ty_named("Int"), int_lit(0))),
                ast::ClassMember::StoredProperty(constant("version")),
            ],
            span: sp(),
        },
    )));
    let library = package(file(vec![ast::Decl::Class(host)]), &["api"]);
    let mut user = file(vec![fun(
        "main",
        vec![
            assign("count", int_lit(9)),
            val("chosen", var("count")),
            val("revision", var("version")),
        ],
    )]);
    user.imports = vec![
        exact(&["api", "Host", "counter"], Some("count"), false),
        exact(&["api", "Host", "Factory", "counter"], Some("count"), false),
        star(&["api", "Host"], false),
    ];
    lower_sources(vec![user, library])
        .expect("forwarding retains the companion receiver and canonical property identity");
}

#[test]
fn imported_payload_variant_in_value_position_reports_its_actual_target() {
    let library = package(
        file(vec![enum_decl(
            "State",
            Vec::new(),
            vec![crate::tests::variant_positional(
                "Payload",
                vec![ty_named("Int")],
            )],
        )]),
        &["api"],
    );
    let mut user = read("Chosen");
    user.imports
        .push(exact(&["api", "State", "Payload"], Some("Chosen"), false));
    let errors = lower_sources(vec![library, user]).expect_err("payload variants require a call");
    assert!(errors.iter().any(|error| error.message
        == "variant `Chosen` of `State` takes arguments; use `Chosen(...)` to construct it"));
}

#[test]
fn assignment_and_update_do_not_skip_a_higher_object_for_a_lower_property() {
    let library = package(file(vec![object("Chosen", Vec::new())]), &["api"]);
    for statement in [
        assign("Chosen", int_lit(1)),
        stmt(ast::Expr::Update {
            place: ast::PlaceExpr::Name(ident("Chosen")),
            op: ast::UpdateOp::Increment,
            notation: ast::UpdateNotation::Postfix,
            span: sp(),
        }),
    ] {
        let mut user = file(vec![
            ast::Decl::Global(stored("Chosen", ty_named("Int"), int_lit(0))),
            fun("main", vec![statement]),
        ]);
        user.imports.push(exact(&["api", "Chosen"], None, false));
        let errors = lower_sources(vec![library.clone(), user])
            .expect_err("all place operations share value selection");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("immutable value")
                    || error.message.contains("not writable"))
        );
    }
}

#[test]
fn static_initializer_cannot_fold_a_lower_core_value_past_an_import() {
    let library = package(file(vec![ast::Decl::Global(constant("None"))]), &["api"]);
    let mut user = file(vec![
        ast::Decl::Global(stored("chosen", ty_nullable(ty_named("Int")), var("None"))),
        fun("main", Vec::new()),
    ]);
    user.imports.push(star(&["api"], false));
    for sources in [vec![user.clone(), library.clone()], vec![library, user]] {
        let errors = lower_sources(sources)
            .expect_err("early static image cannot hide imported type mismatch");
        assert!(errors.iter().any(|error| error.message.contains("initializer") && error.message.contains("Int")), "{errors:#?}");
    }
}

#[test]
fn static_initializer_preserves_import_ambiguity_and_constant_origin() {
    let library = package(file(vec![ast::Decl::Global(constant("Chosen"))]), &["api"]);
    let other = package(file(vec![object("Chosen", Vec::new())]), &["other"]);
    let mut user = file(vec![
        ast::Decl::Global(stored("result", ty_named("Int"), var("Chosen"))),
        fun("main", Vec::new()),
    ]);
    user.imports.push(star(&["api"], false));
    lower_sources(vec![user.clone(), library.clone()])
        .expect("imported const is usable in an initializer");
    user.imports.push(star(&["other"], false));
    let errors = lower_sources(vec![user, library, other])
        .expect_err("static folding cannot select one ambiguous origin");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("value `Chosen` is ambiguous"))
    );
}

#[test]
fn const_initializer_uses_import_identity_and_dependency_cycles_across_sources() {
    let mut chosen = constant("Chosen");
    chosen.body = ast::PropertyBodySyntax::Const(Box::new(var("Imported")));
    let mut user = file(vec![
        ast::Decl::Global(chosen),
        fun("main", vec![val("chosen", var("Chosen"))]),
    ]);
    user.imports
        .push(exact(&["api", "Seed"], Some("Imported"), false));
    let library = package(file(vec![ast::Decl::Global(constant("Seed"))]), &["api"]);
    for sources in [
        vec![user.clone(), library.clone()],
        vec![library.clone(), user.clone()],
    ] {
        let output =
            lower_sources(sources).expect("const evaluation follows the selected SourcePropertyId");
        assert!(matches!(
            first_init(&output).kind,
            hir::ExprKind::IntegerLiteral(_)
        ));
    }
    let mut cyclic = library;
    cyclic.imports.push(exact(&["Chosen"], Some("Back"), false));
    let ast::Decl::Global(seed) = &mut cyclic.declarations[0] else {
        unreachable!()
    };
    seed.body = ast::PropertyBodySyntax::Const(Box::new(var("Back")));
    let errors = lower_sources(vec![user, cyclic])
        .expect_err("import identity also drives the dependency graph");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("const dependency cycle"))
    );
}

#[test]
fn const_initializer_cannot_bypass_cross_kind_shadowing_or_ambiguity() {
    let library = package(file(vec![object("Chosen", Vec::new())]), &["api"]);
    let mut result = constant("result");
    result.body = ast::PropertyBodySyntax::Const(Box::new(var("Chosen")));
    let mut user = file(vec![
        ast::Decl::Global(constant("Chosen")),
        ast::Decl::Global(result),
        fun("main", Vec::new()),
    ]);
    user.imports.push(exact(&["api", "Chosen"], None, false));
    let errors = lower_sources(vec![library.clone(), user.clone()])
        .expect_err("selected object cannot fall back to current const");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("not const")),
        "{errors:#?}"
    );
    let other = package(
        file(vec![ast::Decl::Global(constant("Chosen"))]),
        &["other"],
    );
    user.imports.push(exact(&["other", "Chosen"], None, false));
    let errors = lower_sources(vec![user, library, other])
        .expect_err("const evaluator preserves value ambiguity");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("value `Chosen` is ambiguous"))
    );
}

#[test]
fn core_value_body_cannot_read_current_unit_property() {
    let mut core = crate::tests::core_file();
    core.declarations.push(fun_expr(
        "readUser",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        var("UserValue"),
    ));
    let errors = lower_sources_with_core(
        vec![file(vec![
            ast::Decl::Global(constant("UserValue")),
            fun("main", Vec::new()),
        ])],
        core,
    )
    .expect_err("core has no current unit value layer");
    assert!(
        errors
            .iter()
            .any(|error| error.file == 0 && error.message.contains("unknown variable `UserValue`"))
    );
}
