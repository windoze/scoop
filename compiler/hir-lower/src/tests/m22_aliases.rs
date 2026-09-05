use super::*;

fn explicit_visibility(visibility: ast::DeclaredVisibility) -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility,
        span: sp(),
    }
}

fn type_alias(name: &str, target: TypeRef) -> Decl {
    type_alias_with_visibility(name, target, ast::VisibilitySyntax::Omitted)
}

fn type_alias_with_visibility(
    name: &str,
    target: TypeRef,
    visibility: ast::VisibilitySyntax,
) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility,
        name: ident(name),
        target,
        span: sp(),
    })
}

fn named_type_at(name: &str, span: Span) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Named(ident_at(name, span)),
        span,
    }
}

fn type_alias_at(name: &str, target: TypeRef, span: Span) -> Decl {
    Decl::TypeAlias(ast::TypeAliasDecl {
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        target,
        span,
    })
}

fn const_property(name: &str, ty: TypeRef, expression: Expr) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Const(Box::new(expression)),
        span: sp(),
    }
}

fn object_with_const(name: &str, property: ast::PropertyDecl) -> Decl {
    Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: vec![ast::ClassMember::StoredProperty(property)],
        span: sp(),
    })
}

fn set_nominal_visibility(mut declaration: Decl, visibility: ast::DeclaredVisibility) -> Decl {
    let syntax = explicit_visibility(visibility);
    match &mut declaration {
        Decl::Struct(declaration) => declaration.visibility = syntax,
        Decl::Enum(declaration) => declaration.visibility = syntax,
        Decl::Class(declaration) => declaration.visibility = syntax,
        Decl::Interface(declaration) => declaration.visibility = syntax,
        Decl::Object(declaration) => declaration.visibility = syntax,
        _ => panic!("test helper expects a nominal declaration"),
    }
    declaration
}

fn lowered_alias<'module>(
    module: &'module hir::Module,
    name: &str,
) -> (hir::ExportTypeAliasId, &'module hir::TypeAliasDecl) {
    module
        .type_aliases
        .iter()
        .find(|(_, declaration)| declaration.name == name)
        .unwrap_or_else(|| panic!("missing lowered typealias `{name}`"))
}

fn user_body(module: &hir::Module) -> &hir::Body {
    match &module.functions[module.entry].kind {
        hir::FunctionKind::User(body) => body,
        _ => panic!("main must have a user body"),
    }
}

#[test]
fn forward_and_chained_aliases_expand_to_one_canonical_generic_target() {
    let module = lower_user(file(vec![
        type_alias("Head", ty_named("Middle")),
        type_alias("Middle", ty_named("IntBox")),
        type_alias("IntBox", ty_generic("Box", vec![ty_named("Int")])),
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        // Type and callable namespaces remain separate.
        fun("Head", Vec::new()),
        fun_sig(
            "consume",
            Vec::new(),
            vec![("value", ty_named("Head"))],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("forward and chained aliases must lower");

    let (_, head) = lowered_alias(&module, "Head");
    let (_, middle) = lowered_alias(&module, "Middle");
    let (_, int_box) = lowered_alias(&module, "IntBox");
    assert_eq!(head.target, middle.target);
    assert_eq!(middle.target, int_box.target);
    assert_eq!(hir::type_name(&module, head.target), "Box<Int>");

    let consume = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "consume")
        .map(|(_, function)| function)
        .expect("consume declaration");
    assert_eq!(consume.params[0].ty, head.target);

    let dump = hir::dump(&module);
    for alias in ["Head", "Middle", "IntBox"] {
        assert!(
            dump.contains(&format!("typealias {alias} = Box<Int>")),
            "{dump}"
        );
    }
}

#[test]
fn tuple_and_function_alias_targets_are_structurally_complete() {
    let module = lower_user(file(vec![
        type_alias("Pair", ty_tuple(vec![ty_named("Int"), ty_named("String")])),
        type_alias(
            "Callback",
            ty_function(false, vec![ty_named("Pair")], ty_named("String")),
        ),
        fun_sig(
            "accept",
            Vec::new(),
            vec![
                ("pair", ty_named("Pair")),
                ("callback", ty_named("Callback")),
            ],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("tuple and function aliases must lower");

    let (_, pair) = lowered_alias(&module, "Pair");
    let (_, callback) = lowered_alias(&module, "Callback");
    let hir::Type::Tuple(elements) = &module.types[pair.target] else {
        panic!("Pair must expand to a tuple")
    };
    assert_eq!(elements, &[int_type(&module), module.string]);
    let hir::Type::Function(function_type) = module.types[callback.target] else {
        panic!("Callback must expand to a function type")
    };
    let function_type = &module.function_types[function_type];
    assert_eq!(function_type.parameter_types, vec![pair.target]);
    assert_eq!(function_type.return_type, module.string);

    let dump = hir::dump(&module);
    assert!(dump.contains("typealias Pair = (Int, String)"), "{dump}");
    assert!(
        dump.contains("typealias Callback = ((Int, String)) -> String"),
        "{dump}"
    );
}

#[test]
fn alias_constructor_and_variant_qualifier_use_fixed_target_arguments() {
    let module = lower_user(file(vec![
        generic_struct_decl("Marker", vec!["T"], Vec::new()),
        enum_decl(
            "Choice",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        ),
        type_alias("IntMarker", ty_generic("Marker", vec![ty_named("Int")])),
        type_alias("IntChoice", ty_generic("Choice", vec![ty_named("Int")])),
        fun(
            "main",
            vec![
                // Marker<T> has no value from which T could otherwise be inferred.
                val("marker", call("IntMarker", Vec::new())),
                val(
                    "some",
                    method_call(var("IntChoice"), "Some", vec![int_lit(1)]),
                ),
                // A generic unit variant has no inference input; the alias fixes T.
                val("none", field(var("IntChoice"), "None")),
            ],
        ),
    ]))
    .expect("alias qualifiers must expand before constructor resolution");

    let body = user_body(&module);
    assert_eq!(
        hir::type_name(&module, local_init(body, "marker").ty),
        "Marker<Int>"
    );
    assert_eq!(
        hir::type_name(&module, local_init(body, "some").ty),
        "Choice<Int>"
    );
    assert_eq!(
        hir::type_name(&module, local_init(body, "none").ty),
        "Choice<Int>"
    );

    let dump = hir::dump(&module);
    assert!(dump.contains("StructInit Marker : Marker<Int>"), "{dump}");
    assert!(
        dump.contains("VariantConstruct Choice.Some<Int> : Choice<Int>"),
        "{dump}"
    );
    assert!(
        dump.contains("VariantConstruct Choice.None<Int> : Choice<Int>"),
        "{dump}"
    );
}

#[test]
fn lexical_nested_types_shadow_aliases_in_types_and_constructors() {
    let Decl::Struct(inner) = struct_decl("Inner", Vec::new()) else {
        unreachable!()
    };
    let Decl::Struct(mut shadow) = struct_decl("Shadow", Vec::new()) else {
        unreachable!()
    };
    shadow.members.push(ast::StructMember::Nested(Box::new(
        ast::NestedNominalDecl::Struct(Box::new(inner)),
    )));
    let qualified_inner = TypeRef {
        kind: TypeRefKind::Qualified {
            path: vec![ident("Shadow"), ident("Inner")],
            arguments: Vec::new(),
        },
        span: sp(),
    };
    let make = method_expr(
        "make",
        Vec::new(),
        Some(ty_named("Shadow")),
        call("Shadow", Vec::new()),
    );
    let keep = method_expr(
        "keep",
        vec![("value", qualified_inner.clone())],
        Some(qualified_inner),
        var("value"),
    );
    let Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        vec![make, keep],
    ) else {
        unreachable!()
    };
    host.members.push(ast::ClassMember::Nested(Box::new(
        ast::NestedNominalDecl::Struct(Box::new(shadow)),
    )));

    lower_user(file(vec![
        type_alias("Shadow", ty_named("AliasTarget")),
        struct_decl("AliasTarget", Vec::new()),
        Decl::Class(host),
        fun("main", Vec::new()),
    ]))
    .expect("a lexical nested type must consistently shadow a top-level alias");
}

#[test]
fn lexical_nested_types_also_shadow_aliases_in_patterns() {
    let Decl::Struct(shadow) = struct_decl("Shadow", Vec::new()) else {
        unreachable!()
    };
    let reject = method(
        "reject",
        vec![("value", ty_named("AliasTarget"))],
        None,
        vec![val_pat(
            false,
            pat_pos(&["Shadow"], Vec::new(), None),
            None,
            var("value"),
        )],
    );
    let Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        vec![reject],
    ) else {
        unreachable!()
    };
    host.members.push(ast::ClassMember::Nested(Box::new(
        ast::NestedNominalDecl::Struct(Box::new(shadow)),
    )));

    let errors = lower_user(file(vec![
        type_alias("Shadow", ty_named("AliasTarget")),
        struct_decl("AliasTarget", Vec::new()),
        Decl::Class(host),
        fun("main", Vec::new()),
    ]))
    .expect_err("a shadowed top-level alias must not remain available to a pattern");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern `Shadow` does not match a subject of type AliasTarget"
    );
}

#[test]
fn alias_qualified_patterns_preserve_the_exact_generic_application() {
    let module = lower_user(file(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        enum_decl(
            "Choice",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        ),
        type_alias("StringBox", ty_generic("Box", vec![ty_named("String")])),
        type_alias("IntChoice", ty_generic("Choice", vec![ty_named("Int")])),
        fun(
            "main",
            vec![
                val_pat(
                    false,
                    pat_pos(&["StringBox"], vec![pat_bind("text")], None),
                    None,
                    call("Box", vec![str_lit("value")]),
                ),
                val(
                    "choice",
                    method_call(var("Choice"), "Some", vec![int_lit(1)]),
                ),
                when_stmt(
                    var("choice"),
                    vec![
                        arm(
                            pat_pos(&["IntChoice", "Some"], vec![pat_bind("number")], None),
                            None,
                            Vec::new(),
                        ),
                        arm(
                            pat_pos(&["IntChoice", "None"], Vec::new(), None),
                            None,
                            Vec::new(),
                        ),
                    ],
                    None,
                ),
            ],
        ),
    ]))
    .expect("alias-qualified patterns must match their exact target application");

    let body = user_body(&module);
    let struct_application = body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Struct { application, .. },
                ..
            } => Some(*application),
            _ => None,
        })
        .expect("alias-qualified struct pattern");
    assert_eq!(
        module.struct_applications[struct_application].arguments,
        vec![module.string]
    );

    let when = body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(when) => Some(when),
            _ => None,
        })
        .expect("alias-qualified enum patterns");
    assert_eq!(when.arms.len(), 2);
    for arm in &when.arms {
        let hir::Pattern::Variant { application, .. } = &arm.pattern else {
            panic!("each IntChoice arm must be a variant pattern")
        };
        assert_eq!(
            module.enum_applications[*application].arguments,
            vec![int_type(&module)]
        );
    }
}

#[test]
fn direct_and_indirect_alias_cycles_report_the_closing_reference() {
    let direct_reference = Span::new(12, 18);
    let right_to_left_reference = Span::new(52, 56);
    let errors = lower_user(file(vec![
        type_alias_at(
            "Direct",
            named_type_at("Direct", direct_reference),
            Span::new(10, 18),
        ),
        type_alias_at(
            "Left",
            named_type_at("Right", Span::new(32, 37)),
            Span::new(30, 37),
        ),
        type_alias_at(
            "Right",
            named_type_at("Left", right_to_left_reference),
            Span::new(50, 56),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("recursive aliases must be rejected");

    assert_eq!(errors.len(), 2);
    assert!(errors.iter().all(|diagnostic| diagnostic.file == 1));
    assert_eq!(errors[0].span, Some(direct_reference));
    assert_eq!(errors[0].message, "typealias cycle: Direct -> Direct");
    assert_eq!(errors[1].span, Some(right_to_left_reference));
    assert_eq!(errors[1].message, "typealias cycle: Left -> Right -> Left");
}

#[test]
fn aliases_share_the_type_namespace_with_aliases_and_nominals() {
    let errors = lower_user(file(vec![
        type_alias("Same", ty_named("Int")),
        type_alias("Same", ty_named("Int")),
        struct_decl("Taken", Vec::new()),
        type_alias("Taken", ty_named("Int")),
        type_alias("Reserved", ty_named("Int")),
        enum_decl("Reserved", Vec::new(), vec![variant_unit("Only")]),
        type_alias("Unit", ty_named("Int")),
        type_alias("Any", ty_named("Int")),
        fun("main", Vec::new()),
    ]))
    .expect_err("duplicate type-namespace declarations must fail");

    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        vec![
            "duplicate typealias `Same`",
            "duplicate type `Taken` (already declared as a struct)",
            "duplicate type `Reserved` (already declared as a typealias)",
            "duplicate type `Unit` (already declared as a built-in type)",
            "duplicate type `Any` (already declared as a built-in type)",
        ]
    );
}

#[test]
fn alias_visibility_and_public_export_identity_are_preserved() {
    let module = lower_user(file(vec![
        type_alias_with_visibility(
            "Published",
            ty_named("Int"),
            explicit_visibility(ast::DeclaredVisibility::Public),
        ),
        type_alias("Defaulted", ty_named("Int")),
        type_alias_with_visibility(
            "FileLocal",
            ty_named("Int"),
            explicit_visibility(ast::DeclaredVisibility::Private),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("valid alias visibilities must lower");

    let (published_id, published) = lowered_alias(&module, "Published");
    let (defaulted_id, defaulted) = lowered_alias(&module, "Defaulted");
    let (file_local_id, file_local) = lowered_alias(&module, "FileLocal");
    assert_eq!(published.access.declared, hir::DeclaredVisibility::Public);
    assert_eq!(defaulted.access.declared, hir::DeclaredVisibility::Internal);
    assert_eq!(file_local.access.declared, hir::DeclaredVisibility::Private);
    assert!(published.access.lookup.0.is_universal());
    assert_eq!(published.target, int_type(&module));
    assert_eq!(defaulted.target, int_type(&module));
    assert_eq!(file_local.target, int_type(&module));
    assert!(module.public_surface.type_aliases.contains(&published_id));
    assert!(!module.public_surface.type_aliases.contains(&defaulted_id));
    assert!(!module.public_surface.type_aliases.contains(&file_local_id));
}

#[test]
fn private_alias_is_only_visible_in_its_declaration_file() {
    let source = file(vec![
        type_alias_with_visibility(
            "Secret",
            ty_named("Int"),
            explicit_visibility(ast::DeclaredVisibility::Private),
        ),
        // This same-file signature proves that the alias itself is valid here.
        fun_sig(
            "sameFile",
            Vec::new(),
            vec![("value", ty_named("Secret"))],
            None,
            Vec::new(),
        ),
    ]);
    let consumer = file(vec![
        fun_sig(
            "otherFile",
            Vec::new(),
            vec![("value", ty_named("Secret"))],
            None,
            Vec::new(),
        ),
        fun("main", Vec::new()),
    ]);

    let errors = lower(&[core_file(), source, consumer])
        .expect_err("a private alias must not be visible in another file");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file, 2);
    assert_eq!(
        errors[0].message,
        "typealias `Secret` is not accessible from this source location"
    );
}

#[test]
fn private_alias_is_rejected_in_cross_file_expression_position() {
    let source = file(vec![
        struct_decl("Target", Vec::new()),
        type_alias_with_visibility(
            "Secret",
            ty_named("Target"),
            explicit_visibility(ast::DeclaredVisibility::Private),
        ),
    ]);
    let consumer = file(vec![fun(
        "main",
        vec![val("value", call("Secret", Vec::new()))],
    )]);

    let errors = lower(&[core_file(), source, consumer])
        .expect_err("a private alias cannot qualify an expression from another file");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file, 2);
    assert_eq!(
        errors[0].message,
        "typealias `Secret` is not accessible from this source location"
    );
}

#[test]
fn alias_qualified_const_reads_preserve_access_diagnostics() {
    let target = object_with_const(
        "Constants",
        const_property("answer", ty_named("Int"), int_lit(42)),
    );
    let source = file(vec![
        target,
        type_alias_with_visibility(
            "SecretConstants",
            ty_named("Constants"),
            explicit_visibility(ast::DeclaredVisibility::Private),
        ),
    ]);
    let consumer = file(vec![
        Decl::Global(const_property(
            "leaked",
            ty_named("Int"),
            field(var("SecretConstants"), "answer"),
        )),
        fun("main", Vec::new()),
    ]);

    let errors = lower(&[core_file(), source, consumer])
        .expect_err("an inaccessible alias must not qualify a const initializer");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].file, 2);
    assert_eq!(
        errors[0].message,
        "typealias `SecretConstants` is not accessible from this source location"
    );

    let module = lower_user(file(vec![
        object_with_const(
            "VisibleConstants",
            const_property("answer", ty_named("Int"), int_lit(42)),
        ),
        type_alias("ConstantsAlias", ty_named("VisibleConstants")),
        Decl::Global(const_property(
            "copied",
            ty_named("Int"),
            field(var("ConstantsAlias"), "answer"),
        )),
        fun("main", Vec::new()),
    ]))
    .expect("a visible object alias may qualify a const initializer");
    let (_, copied) = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "copied")
        .expect("copied const property");
    assert!(matches!(
        copied.representation,
        hir::PropertyRepresentation::Const {
            value: hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed32(42))
        }
    ));
}

#[test]
fn alias_exposure_checks_the_fully_expanded_target_tree() {
    let errors = lower_user(file(vec![
        set_nominal_visibility(
            struct_decl("PrivateHidden", Vec::new()),
            ast::DeclaredVisibility::Private,
        ),
        struct_decl("InternalOnly", Vec::new()),
        type_alias(
            "InternalLeak",
            ty_tuple(vec![ty_named("PrivateHidden"), ty_named("Int")]),
        ),
        type_alias_with_visibility(
            "PublicLeak",
            ty_function(false, vec![ty_named("Int")], ty_named("InternalOnly")),
            explicit_visibility(ast::DeclaredVisibility::Public),
        ),
        type_alias_with_visibility(
            "ProtectedAlias",
            ty_named("Int"),
            explicit_visibility(ast::DeclaredVisibility::Protected),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("an alias cannot expose a narrower target declaration");

    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.contains(
        &"signature of typealias `InternalLeak` exposes type `PrivateHidden` outside its access domain"
    ));
    assert!(messages.contains(
        &"signature of typealias `PublicLeak` exposes type `InternalOnly` outside its access domain"
    ));
    assert!(messages.contains(&"top-level typealias cannot be protected"));
    assert_eq!(messages.len(), 3, "unexpected diagnostics: {messages:?}");
}

#[test]
fn alias_target_bounds_are_checked_after_inheritance_is_complete() {
    let mut bounded = generic_struct_decl("Bounded", vec!["T"], Vec::new());
    let Decl::Struct(declaration) = &mut bounded else {
        unreachable!()
    };
    declaration.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("Marker")));

    let module = lower_user(file(vec![
        // Keep the alias and implementation before their dependencies to
        // exercise both forward lookup and deferred conformance checking.
        type_alias(
            "MarkedBounded",
            ty_generic("Bounded", vec![ty_named("Marked")]),
        ),
        struct_decl_full("Marked", Vec::new(), vec!["Marker"], Vec::new()),
        interface_decl("Marker", Vec::new()),
        bounded,
        fun("main", Vec::new()),
    ]))
    .expect("an alias argument may satisfy a bound through a later inheritance edge");

    let (_, alias) = lowered_alias(&module, "MarkedBounded");
    assert_eq!(hir::type_name(&module, alias.target), "Bounded<Marked>");
}

#[test]
fn alias_target_bound_violations_are_rejected_after_expansion() {
    let mut bounded = generic_struct_decl("Bounded", vec!["T"], Vec::new());
    let Decl::Struct(declaration) = &mut bounded else {
        unreachable!()
    };
    declaration.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("Marker")));

    let errors = lower_user(file(vec![
        type_alias(
            "UnmarkedBounded",
            ty_generic("Bounded", vec![ty_named("Unmarked")]),
        ),
        struct_decl("Unmarked", Vec::new()),
        interface_decl("Marker", Vec::new()),
        bounded,
        fun("main", Vec::new()),
    ]))
    .expect_err("an expanded alias target must still satisfy nominal bounds");

    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message
            == "type argument `Unmarked` for `T` of target of typealias `UnmarkedBounded` must satisfy interface upper bound `Marker`"
    }));
}

#[test]
fn aliases_do_not_distinguish_overload_signatures() {
    let errors = lower_user(file(vec![
        type_alias("Value", ty_named("Int")),
        fun_expr(
            "pick",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun_expr(
            "pick",
            Vec::new(),
            vec![("value", ty_named("Value"))],
            Some(ty_named("Value")),
            var("value"),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("alias spelling must not create a distinct overload");

    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `pick` is already declared with the same signature"
    );
}
