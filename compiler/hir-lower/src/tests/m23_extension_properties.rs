use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SetterMode {
    None,
    Inherited,
    Private,
}

fn qualified(parts: &[&str]) -> ast::QualifiedNameSyntax {
    let (first, rest) = parts
        .split_first()
        .expect("test qualified names are nonempty");
    ast::QualifiedNameSyntax {
        first: ident(first),
        rest: rest
            .iter()
            .map(|part| ast::QualifiedNameTailSyntax {
                dot_span: sp(),
                identifier: ident(part),
            })
            .collect(),
        span: sp(),
    }
}

fn package(mut source: ast::SourceFile, name: &str) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: qualified(&[name]),
        span: sp(),
    };
    source
}

fn exact(package: &str, name: &str, alias: Option<&str>) -> ast::ImportSyntax {
    ast::ImportSyntax::Exact {
        exposure: ast::ImportExposureSyntax::Local,
        selector: qualified(&[package, name]),
        alias: alias.map(|name| ast::ImportAliasSyntax {
            as_keyword_span: sp(),
            name: ident(name),
            span: sp(),
        }),
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn star(package: &str) -> ast::ImportSyntax {
    ast::ImportSyntax::Star {
        exposure: ast::ImportExposureSyntax::Local,
        namespace: qualified(&[package]),
        import_keyword_span: sp(),
        terminal_dot_span: sp(),
        star_span: sp(),
        span: sp(),
    }
}

fn lower_sources(
    sources: Vec<ast::SourceFile>,
    core: ast::SourceFile,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let request = ast::Stage1RequestId::from_raw(723);
    let mut parsed = sources.into_iter().enumerate().map(|(index, source)| {
        ast::ParsedSource::new(
            ast::Stage1SourceHandle::new(
                request,
                u32::try_from(index).expect("test source index fits u32"),
            ),
            source,
        )
    });
    let parsed = ast::AllParsedSources::try_new(
        request,
        ast::NonEmptyVec::new(
            parsed.next().expect("test source list is nonempty"),
            parsed.collect(),
        ),
    )
    .expect("all test sources belong to one request");
    let input = Stage1CompilationInput::new(
        vec![ProviderSource {
            source: &core,
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core.scoop",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| Stage1SourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    );
    lower_stage1_compilation_input(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

fn core_with(declarations: Vec<Decl>) -> ast::SourceFile {
    let mut core = core_file();
    core.declarations.extend(declarations);
    make_core_public(&mut core);
    core
}

fn property(receiver: Option<&str>, name: &str, marker: i64, setter: SetterMode) -> Decl {
    let setter = match setter {
        SetterMode::None => None,
        SetterMode::Inherited | SetterMode::Private => Some(ast::SetterDecl {
            annotations: Vec::new(),
            visibility: if setter == SetterMode::Private {
                ast::SetterVisibilitySyntax::Explicit {
                    visibility: ast::DeclaredVisibility::Private,
                    span: sp(),
                }
            } else {
                ast::SetterVisibilitySyntax::Inherited
            },
            parameter: ast::SetterParameterSyntax::Default { span: sp() },
            body: ast::AccessorBodySyntax::Block(block(Vec::new())),
            span: sp(),
        }),
    };
    Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: setter.is_some(),
        receiver_ty: receiver.map(ty_named),
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Computed(ast::AccessorSyntax {
            getter: Some(ast::GetterDecl {
                annotations: Vec::new(),
                body: ast::AccessorBodySyntax::Expr(Box::new(int_lit(marker))),
                span: sp(),
            }),
            setter,
        }),
        span: sp(),
    })
}

fn private(mut declaration: Decl) -> Decl {
    let Decl::Global(property) = &mut declaration else {
        panic!("test visibility helper expects a property")
    };
    property.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    declaration
}

fn extension_block(name: &str, statements: Vec<Statement>) -> Decl {
    let Decl::Function(mut function) = fun_sig(name, Vec::new(), Vec::new(), None, statements)
    else {
        unreachable!("fun_sig creates a function")
    };
    function.receiver_ty = Some(ty_named("Owner"));
    Decl::Function(function)
}

fn extension_read(name: &str, value: Expr) -> Decl {
    extension_expr(
        ty_named("Owner"),
        name,
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        value,
    )
}

fn update_name(name: &str) -> Expr {
    Expr::Update {
        place: ast::PlaceExpr::Name(ident(name)),
        op: ast::UpdateOp::Increment,
        notation: ast::UpdateNotation::Postfix,
        span: sp(),
    }
}

fn body<'a>(module: &'a hir::Module, name: &str) -> &'a hir::Body {
    let function = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("test function `{name}` exists"));
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function `{name}` has a source body")
    };
    body
}

fn called_function(module: &hir::Module, expression: &hir::Expr) -> hir::FunctionId {
    let callable = match expression.kind {
        hir::ExprKind::Call { callee, .. } => callee,
        hir::ExprKind::MethodCall {
            callee: hir::MethodCallee::Callable(callee),
            ..
        } => callee,
        ref other => panic!("expected a callable property access, found {other:?}"),
    };
    module.callable_function(callable)
}

fn getter_property(module: &hir::Module, function: hir::FunctionId) -> hir::PropertyId {
    module
        .properties
        .iter()
        .find_map(|(id, property)| {
            matches!(
                module.property_getters[property.capability.getter()].implementation,
                hir::PropertyAccessorImplementation::Body(candidate) if candidate == function
            )
            .then_some(id)
        })
        .expect("the selected call is a property getter")
}

fn setter_property(module: &hir::Module, function: hir::FunctionId) -> Option<hir::PropertyId> {
    module.properties.iter().find_map(|(id, property)| {
        let setter = property.capability.setter()?;
        matches!(
            module.property_setters[setter].implementation,
            hir::PropertyAccessorImplementation::Body(candidate) if candidate == function
        )
        .then_some(id)
    })
}

fn property_marker(module: &hir::Module, property: hir::PropertyId) -> i64 {
    let getter = module.properties[property].capability.getter();
    let hir::PropertyAccessorImplementation::Body(function) =
        module.property_getters[getter].implementation
    else {
        panic!("a test property has a computed getter")
    };
    let hir::FunctionKind::User(body) = &module.functions[function].kind else {
        panic!("a test getter has a source body")
    };
    let hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(marker)) =
        return_value(&body.statements).kind
    else {
        panic!("a test getter returns its integer marker")
    };
    i64::from(marker)
}

fn getter_marker(module: &hir::Module, expression: &hir::Expr) -> i64 {
    property_marker(
        module,
        getter_property(module, called_function(module, expression)),
    )
}

fn setter_markers(module: &hir::Module, body: &hir::Body) -> Vec<i64> {
    body.statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::Expr(expression) = &statement.kind else {
                return None;
            };
            let hir::ExprKind::Call { callee, .. } = expression.kind else {
                return None;
            };
            let function = module.callable_function(callee);
            setter_property(module, function).map(|property| property_marker(module, property))
        })
        .collect()
}

fn assert_candidate_notes(diagnostic: &ast::Diagnostic, count: usize) {
    assert_eq!(diagnostic.notes.len(), count, "{diagnostic:?}");
    assert!(
        diagnostic
            .notes
            .iter()
            .all(|note| note.message == "candidate declared here"),
        "{diagnostic:?}"
    );
    assert!(
        diagnostic.notes.windows(2).all(|notes| {
            (notes[0].file, notes[0].span.start, notes[0].span.end)
                <= (notes[1].file, notes[1].span.start, notes[1].span.end)
        }),
        "candidate notes must have stable source order: {diagnostic:?}"
    );
}

#[test]
fn explicit_read_and_write_fall_through_exact_current_star_then_core() {
    for winner in 0..4 {
        let receiver = |layer| if layer < winner { "String" } else { "Owner" };
        let exact_source = package(
            file(vec![property(
                Some(receiver(0)),
                "route",
                1,
                SetterMode::Inherited,
            )]),
            "exactlib",
        );
        let star_source = package(
            file(vec![property(
                Some(receiver(2)),
                "route",
                3,
                SetterMode::Inherited,
            )]),
            "starlib",
        );
        let core = core_with(vec![
            struct_decl("Owner", Vec::new()),
            property(Some(receiver(3)), "route", 4, SetterMode::Inherited),
        ]);
        let mut user = file(vec![
            property(Some(receiver(1)), "route", 2, SetterMode::Inherited),
            fun(
                "main",
                vec![
                    val("owner", call("Owner", Vec::new())),
                    val("chosen", field(var("owner"), "route")),
                    assign_field(var("owner"), "route", int_lit(9)),
                ],
            ),
        ]);
        user.imports = vec![exact("exactlib", "route", None), star("starlib")];

        let output = lower_sources(vec![exact_source, star_source, user], core)
            .expect("a receiver-inapplicable property layer falls through");
        let main = body(&output.export, "main");
        assert_eq!(
            getter_marker(&output.export, local_init(main, "chosen")),
            i64::from(winner + 1)
        );
        assert_eq!(
            setter_markers(&output.export, main),
            [i64::from(winner + 1)]
        );
    }
}

#[test]
fn implicit_read_and_write_fall_through_exact_current_star_then_core() {
    for winner in 0..4 {
        let receiver = |layer| if layer < winner { "String" } else { "Owner" };
        let exact_source = package(
            file(vec![property(
                Some(receiver(0)),
                "route",
                1,
                SetterMode::Inherited,
            )]),
            "exactlib",
        );
        let star_source = package(
            file(vec![property(
                Some(receiver(2)),
                "route",
                3,
                SetterMode::Inherited,
            )]),
            "starlib",
        );
        let core = core_with(vec![
            struct_decl("Owner", Vec::new()),
            property(Some(receiver(3)), "route", 4, SetterMode::Inherited),
        ]);
        let mut user = file(vec![
            property(Some(receiver(1)), "route", 2, SetterMode::Inherited),
            extension_read("readProbe", var("route")),
            extension_block("writeProbe", vec![assign("route", int_lit(9))]),
            fun("main", Vec::new()),
        ]);
        user.imports = vec![exact("exactlib", "route", None), star("starlib")];

        let output = lower_sources(vec![exact_source, star_source, user], core)
            .expect("a receiver-inapplicable implicit property layer falls through");
        assert_eq!(
            getter_marker(
                &output.export,
                return_value(&body(&output.export, "readProbe").statements),
            ),
            i64::from(winner + 1)
        );
        assert_eq!(
            setter_markers(&output.export, body(&output.export, "writeProbe")),
            [i64::from(winner + 1)]
        );
    }
}

#[test]
fn applicable_exact_property_ambiguity_is_terminal() {
    let left = package(
        file(vec![property(
            Some("Owner"),
            "route",
            1,
            SetterMode::Inherited,
        )]),
        "left",
    );
    let right = package(
        file(vec![property(
            Some("Owner"),
            "route",
            2,
            SetterMode::Inherited,
        )]),
        "right",
    );
    let mut user = file(vec![
        property(Some("Owner"), "route", 3, SetterMode::Inherited),
        fun(
            "main",
            vec![val("chosen", field(call("Owner", Vec::new()), "route"))],
        ),
    ]);
    user.imports = vec![exact("left", "route", None), exact("right", "route", None)];
    let errors = lower_sources(
        vec![left, right, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("an applicable ambiguous property layer cannot fall through");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ambiguous")),
        "{errors:?}"
    );
}

#[test]
fn aliases_and_repeated_imports_preserve_and_deduplicate_property_identity() {
    let library = package(
        file(vec![property(
            Some("Owner"),
            "route",
            7,
            SetterMode::Inherited,
        )]),
        "api",
    );
    let mut user = file(vec![fun(
        "main",
        vec![
            val("owner", call("Owner", Vec::new())),
            val("aliased", field(var("owner"), "path")),
            assign_field(var("owner"), "path", int_lit(1)),
            val("starred", field(var("owner"), "route")),
            assign_field(var("owner"), "route", int_lit(2)),
        ],
    )]);
    user.imports = vec![
        exact("api", "route", Some("path")),
        exact("api", "route", Some("path")),
        star("api"),
        star("api"),
    ];
    let output = lower_sources(
        vec![library, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect("aliases retain the extension role and repeated typed origins deduplicate");
    let main = body(&output.export, "main");
    assert_eq!(
        getter_marker(&output.export, local_init(main, "aliased")),
        7
    );
    assert_eq!(
        getter_marker(&output.export, local_init(main, "starred")),
        7
    );
    assert_eq!(setter_markers(&output.export, main), [7, 7]);
}

#[test]
fn inaccessible_current_property_is_filtered_before_star_selection() {
    let hidden = file(vec![private(property(
        Some("Owner"),
        "route",
        2,
        SetterMode::Inherited,
    ))]);
    let library = package(
        file(vec![property(
            Some("Owner"),
            "route",
            3,
            SetterMode::Inherited,
        )]),
        "api",
    );
    let mut user = file(vec![fun(
        "main",
        vec![
            val("owner", call("Owner", Vec::new())),
            val("chosen", field(var("owner"), "route")),
            assign_field(var("owner"), "route", int_lit(1)),
        ],
    )]);
    user.imports = vec![star("api"), star("api")];
    let output = lower_sources(
        vec![hidden, library, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect("an inaccessible current candidate does not hide a visible star candidate");
    let main = body(&output.export, "main");
    assert_eq!(getter_marker(&output.export, local_init(main, "chosen")), 3);
    assert_eq!(setter_markers(&output.export, main), [3]);
}

#[test]
fn selected_property_setter_failure_does_not_fall_through() {
    let library = package(
        file(vec![property(
            Some("Owner"),
            "route",
            1,
            SetterMode::Private,
        )]),
        "api",
    );
    let mut user = file(vec![
        property(Some("Owner"), "route", 2, SetterMode::Inherited),
        fun(
            "main",
            vec![assign_field(call("Owner", Vec::new()), "route", int_lit(1))],
        ),
    ]);
    user.imports = vec![exact("api", "route", None)];
    let errors = lower_sources(
        vec![library, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("a selected private setter is terminal");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message == "setter of property `route` is not accessible"),
        "{errors:?}"
    );
}

#[test]
fn real_member_locking_precedes_an_exact_extension_property() {
    let library = package(
        file(vec![property(
            Some("Owner"),
            "route",
            9,
            SetterMode::Inherited,
        )]),
        "api",
    );
    let mut user = file(vec![fun(
        "main",
        vec![assign_field(
            call("Owner", vec![int_lit(0)]),
            "route",
            int_lit(1),
        )],
    )]);
    user.imports = vec![exact("api", "route", None)];
    let errors = lower_sources(
        vec![library, user],
        core_with(vec![class_decl(
            ast::ClassModifier::Final,
            "Owner",
            vec![(false, "route", ty_named("Int"))],
            None,
            Vec::new(),
            Vec::new(),
        )]),
    )
    .expect_err("an immutable real member locks selection before extensions");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message == "cannot assign to immutable property `route`"),
        "{errors:?}"
    );
}

#[test]
fn bare_names_compare_ordinary_and_implicit_extension_values_per_layer() {
    let exact_value = package(
        file(vec![property(None, "route", 1, SetterMode::None)]),
        "values",
    );
    let mut exact_value_user = file(vec![
        property(Some("Owner"), "route", 2, SetterMode::None),
        extension_read("probe", var("route")),
        fun("main", Vec::new()),
    ]);
    exact_value_user.imports = vec![exact("values", "route", None)];
    let output = lower_sources(
        vec![exact_value, exact_value_user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect("a higher ordinary value beats a lower applicable implicit extension");
    assert_eq!(
        getter_marker(
            &output.export,
            return_value(&body(&output.export, "probe").statements)
        ),
        1
    );

    let exact_inapplicable = package(
        file(vec![property(Some("String"), "route", 1, SetterMode::None)]),
        "extensions",
    );
    let mut ordinary_user = file(vec![
        property(None, "route", 2, SetterMode::None),
        extension_read("probe", var("route")),
        fun("main", Vec::new()),
    ]);
    ordinary_user.imports = vec![exact("extensions", "route", None)];
    let output = lower_sources(
        vec![exact_inapplicable, ordinary_user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect("an inapplicable implicit extension falls through to an ordinary value");
    assert_eq!(
        getter_marker(
            &output.export,
            return_value(&body(&output.export, "probe").statements)
        ),
        2
    );
}

#[test]
fn same_layer_ordinary_and_implicit_extension_ambiguity_lists_both_origins() {
    let value = package(
        file(vec![property(None, "ordinary", 1, SetterMode::None)]),
        "values",
    );
    let extension = package(
        file(vec![property(
            Some("Owner"),
            "extension",
            2,
            SetterMode::None,
        )]),
        "extensions",
    );
    let mut user = file(vec![
        extension_read("probe", var("route")),
        fun("main", Vec::new()),
    ]);
    user.imports = vec![
        exact("values", "ordinary", Some("route")),
        exact("extensions", "extension", Some("route")),
    ];
    let errors = lower_sources(
        vec![value, extension, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("ordinary and applicable extension values tie in one layer");
    let diagnostic = errors
        .iter()
        .find(|diagnostic| {
            diagnostic.message == "value `route` is ambiguous in the exact import layer"
        })
        .unwrap_or_else(|| panic!("missing exact-layer ambiguity: {errors:?}"));
    assert_candidate_notes(diagnostic, 2);
}

#[test]
fn same_layer_ordinary_value_ambiguity_lists_every_typed_origin() {
    let left = package(
        file(vec![property(None, "left", 1, SetterMode::None)]),
        "left",
    );
    let right = package(
        file(vec![property(None, "right", 2, SetterMode::None)]),
        "right",
    );
    let mut user = file(vec![
        extension_read("probe", var("route")),
        fun("main", Vec::new()),
    ]);
    user.imports = vec![
        exact("left", "left", Some("route")),
        exact("right", "right", Some("route")),
    ];
    let errors = lower_sources(
        vec![left, right, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("distinct ordinary values tie in one exact layer");
    let diagnostic = errors
        .iter()
        .find(|diagnostic| {
            diagnostic.message == "value `route` is ambiguous in the exact import layer"
        })
        .unwrap_or_else(|| panic!("missing exact-layer value ambiguity: {errors:?}"));
    assert_candidate_notes(diagnostic, 2);
}

#[test]
fn exact_function_blocker_stops_before_a_lower_implicit_extension() {
    let library = package(
        file(vec![
            fun("route", Vec::new()),
            property(Some("String"), "route", 1, SetterMode::None),
        ]),
        "api",
    );
    let mut user = file(vec![
        property(Some("Owner"), "route", 2, SetterMode::None),
        extension_read("probe", var("route")),
        fun("main", Vec::new()),
    ]);
    user.imports = vec![exact("api", "route", None)];
    let errors = lower_sources(
        vec![library, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("an exact function blocks a lower applicable implicit extension");
    let diagnostic = errors
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .starts_with("function `route` is not a value")
        })
        .unwrap_or_else(|| panic!("missing exact function blocker: {errors:?}"));
    assert_candidate_notes(diagnostic, 1);
}

#[test]
fn exact_type_blocker_stops_before_a_lower_implicit_extension() {
    let library = package(file(vec![struct_decl("Blocker", Vec::new())]), "api");
    let mut user = file(vec![
        property(Some("Owner"), "route", 2, SetterMode::None),
        extension_read("probe", var("route")),
        fun("main", Vec::new()),
    ]);
    user.imports = vec![exact("api", "Blocker", Some("route"))];
    let errors = lower_sources(
        vec![library, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect_err("an exact type blocks a lower applicable implicit extension");
    let diagnostic = errors
        .iter()
        .find(|diagnostic| diagnostic.message == "type `route` is a type, not a value")
        .unwrap_or_else(|| panic!("missing exact type blocker: {errors:?}"));
    assert_candidate_notes(diagnostic, 1);
}

#[test]
fn bare_assignment_and_place_update_share_layered_property_selection() {
    let library = package(
        file(vec![property(
            Some("Owner"),
            "route",
            7,
            SetterMode::Inherited,
        )]),
        "api",
    );
    let mut user = file(vec![
        extension_block("assignProbe", vec![assign("path", int_lit(1))]),
        extension_block("updateProbe", vec![val("updated", update_name("path"))]),
        fun("main", Vec::new()),
    ]);
    user.imports = vec![
        exact("api", "route", Some("path")),
        exact("api", "route", Some("path")),
    ];
    let output = lower_sources(
        vec![library, user],
        core_with(vec![struct_decl("Owner", Vec::new())]),
    )
    .expect("bare assignment and place update use the shared typed selector");
    assert_eq!(
        setter_markers(&output.export, body(&output.export, "assignProbe")),
        [7]
    );
    assert_eq!(
        setter_markers(&output.export, body(&output.export, "updateProbe")),
        [7]
    );
}
