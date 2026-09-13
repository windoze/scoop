use super::*;

fn source_identity(source_key: u32) -> scoop_identity::SourceIdentity {
    let path = match source_key {
        0 => "src/000-zero.scoop",
        1 => "src/001-one.scoop",
        3 => "src/003-three.scoop",
        5 => "src/005-five.scoop",
        9 => "src/009-nine.scoop",
        10 => "src/010-ten.scoop",
        20 => "src/020-twenty.scoop",
        25 => "src/025-twenty-five.scoop",
        30 => "src/030-thirty.scoop",
        40 => "src/040-forty.scoop",
        50 => "src/050-fifty.scoop",
        60 => "src/060-sixty.scoop",
        70 => "src/070-seventy.scoop",
        90 => "src/090-ninety.scoop",
        _ => panic!("test source key {source_key} needs an explicit logical path"),
    };
    test_source_identity(path)
}

fn in_package(mut source: ast::SourceFile, package: &str) -> ast::SourceFile {
    source.package = ast::PackageSyntax::QualifiedPackage {
        package_keyword_span: sp(),
        path: ast::QualifiedNameSyntax {
            first: ident(package),
            rest: Vec::new(),
            span: sp(),
        },
        span: sp(),
    };
    source
}

fn declaration_at(mut declaration: ast::Decl, span: Span) -> ast::Decl {
    let ast::Decl::Function(function) = &mut declaration else {
        panic!("entry test declarations are functions")
    };
    function.span = span;
    declaration
}

fn extern_main() -> ast::Decl {
    let mut declaration = fun("main", Vec::new());
    let ast::Decl::Function(function) = &mut declaration else {
        unreachable!("fun builds a function")
    };
    function.annotations = vec![ast::Annotation {
        name: ident("Extern"),
        args: [
            ("lib", ""),
            ("name", "legacy_native_main"),
            ("abi", "scoop"),
        ]
        .into_iter()
        .map(|(name, value)| ast::AnnotationArg {
            name: Some(ident(name)),
            value: ast::AnnotationLiteral::String(value.to_string()),
            span: sp(),
        })
        .collect(),
        span: sp(),
    }];
    function.body = ast::FunctionBody::None;
    declaration
}

fn parsed_sources(sources: Vec<(u32, ast::SourceFile)>) -> ast::AllParsedSources {
    let mut sources = sources.into_iter();
    let (first_key, first_source) = sources.next().expect("entry tests supply user sources");
    let parsed =
        |source_key, source| ast::IdentifiedParsedSource::new(source_identity(source_key), source);
    ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        parsed(first_key, first_source),
        sources
            .map(|(source_key, source)| parsed(source_key, source))
            .collect(),
    ))
    .expect("entry tests use unique source identities")
}

fn combined_input<'a>(
    core: &'a ast::SourceFile,
    user_sources: ast::AllParsedSources,
) -> LegacyCombinedSources<'a> {
    LegacyCombinedSources::try_new(
        vec![ProviderSource {
            source: core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(17),
            name: "core.scoop",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(29),
        user_sources,
        |_| CurrentSourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    )
    .expect("explicit entry-test source identities are valid")
}

fn lower_sources(
    core: &ast::SourceFile,
    user_sources: Vec<(u32, ast::SourceFile)>,
) -> Result<hir::LegacyExecutableOutput, Vec<ast::Diagnostic>> {
    let input = combined_input(core, parsed_sources(user_sources));
    lower_legacy_combined_executable(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

fn lower_frontend_sources(
    core: &ast::SourceFile,
    user_sources: Vec<(u32, ast::SourceFile)>,
) -> hir::Output {
    let input = combined_input(core, parsed_sources(user_sources));
    lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
        .expect("entry-selection fixtures have otherwise valid frontend HIR")
}

#[test]
fn frontend_lowering_does_not_require_main() {
    let core = core_file();
    let input = combined_input(
        &core,
        parsed_sources(vec![(0, file(vec![fun("libraryFunction", Vec::new())]))]),
    );
    let output = lower_legacy_combined_sources(&input, IntrinsicDeclarationPolicy::CoreOnly)
        .expect("frontend HIR accepts a current unit without an executable entry");
    assert!(
        output
            .export
            .functions
            .iter()
            .any(|(_, function)| function.name == "libraryFunction")
    );
}

#[test]
fn hir_dump_dispatch_preserves_only_a_legacy_executable_entry() {
    let output = lower_sources(&core_file(), vec![(0, file(vec![fun("main", Vec::new())]))])
        .expect("the legacy executable has one valid entry");
    let legacy_dump = hir::dump(&output.export);
    assert_eq!(legacy_dump, hir::dump_legacy_executable(&output.export));
    assert!(legacy_dump.contains("\n  entry main\n"));

    let base_dump = hir::dump(output.export.module());
    assert!(!base_dump.contains("\n  entry "));
}

#[test]
fn unique_valid_main_ignores_every_ineligible_main_shape() {
    let valid_span = Span::new(101, 111);
    let mut core = core_file();
    core.declarations
        .push(declaration_at(fun("main", Vec::new()), Span::new(1, 2)));

    let valid = file(vec![declaration_at(fun("main", Vec::new()), valid_span)]);
    let generic = in_package(
        file(vec![fun_sig(
            "main",
            vec!["T"],
            Vec::new(),
            None,
            Vec::new(),
        )]),
        "generic",
    );
    let suspend = in_package(file(vec![suspend_fun("main", Vec::new())]), "suspending");
    let parameterized = in_package(
        file(vec![fun_sig(
            "main",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            None,
            Vec::new(),
        )]),
        "parameterized",
    );
    let non_unit = in_package(
        file(vec![fun_expr(
            "main",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        )]),
        "nonunit",
    );
    let external = in_package(file(vec![extern_main()]), "external");
    let intrinsic = in_package(
        file(vec![intrinsic_fun(
            "main",
            "rt_gc_collect",
            Vec::new(),
            None,
        )]),
        "intrinsic",
    );
    let extension = in_package(
        file(vec![extension_expr(
            ty_named("Int"),
            "main",
            Vec::new(),
            Vec::new(),
            None,
            unit_lit(),
        )]),
        "extension",
    );

    let input = combined_input(
        &core,
        parsed_sources(vec![
            (70, generic),
            (60, suspend),
            (50, parameterized),
            (40, non_unit),
            (30, external),
            (25, intrinsic),
            (20, extension),
            (10, valid),
        ]),
    );
    let output = lower_legacy_combined_executable(
        &input,
        IntrinsicDeclarationPolicy::AllowListedForTesting {
            providers: std::collections::HashSet::from([hir::IntrinsicProviderId::from_raw(29)]),
        },
    )
    .expect("only the fully qualified current-unit main is an entry candidate");

    let entry = &output.export.functions[output.export.entry()];
    assert_eq!(entry.span, valid_span);
    assert!(matches!(&entry.kind, hir::FunctionKind::User(_)));
    assert!(matches!(&entry.genericity, hir::FunctionGenericity::Plain));
    assert!(!entry.is_suspend);
    assert!(entry.params.is_empty());
    assert_eq!(entry.return_ty, output.export.unit);
}

#[test]
fn non_unit_main_has_a_focused_legacy_entry_diagnostic() {
    let span = Span::new(41, 57);
    let declaration = declaration_at(
        fun_expr(
            "main",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        span,
    );
    let errors = lower_sources(&core_file(), vec![(0, file(vec![declaration]))])
        .expect_err("a legacy executable main must return Unit");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`main` must return `Unit`");
    assert_eq!(errors[0].span, Some(span));
}

#[test]
fn invalid_main_diagnostic_belongs_to_its_declaring_source() {
    let span = Span::new(51, 64);
    let generic = in_package(
        file(vec![declaration_at(
            fun_sig("main", vec!["T"], Vec::new(), None, Vec::new()),
            span,
        )]),
        "generic",
    );
    let errors = lower_sources(&core_file(), vec![(1, file(Vec::new())), (9, generic)])
        .expect_err("a generic main cannot be a legacy entry");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`main` must not be generic");
    assert_eq!(errors[0].file, 2);
    assert_eq!(errors[0].span, Some(span));
}

#[test]
fn valid_mains_in_different_packages_are_multiple_entries() {
    let first = in_package(file(vec![fun("main", Vec::new())]), "first");
    let second = in_package(file(vec![fun("main", Vec::new())]), "second");
    let errors = lower_sources(&core_file(), vec![(5, first), (9, second)])
        .expect_err("package qualification does not distinguish executable entries");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "multiple entry points: declare exactly one `fun main()`"
    );
}

fn entry_error_owner(
    core: &ast::SourceFile,
    user_sources: Vec<(u32, ast::SourceFile)>,
    expected_message: &str,
) -> (scoop_identity::SourceIdentity, Option<Span>) {
    let input = combined_input(core, parsed_sources(user_sources));
    let errors = lower_legacy_combined_executable(&input, IntrinsicDeclarationPolicy::CoreOnly)
        .expect_err("the test input has no unique legacy entry");
    let diagnostic = errors
        .iter()
        .find(|diagnostic| diagnostic.message == expected_message)
        .expect("the expected legacy entry diagnostic is present");
    let user_index = diagnostic
        .file
        .checked_sub(1)
        .expect("legacy entry diagnostics belong to a user source");
    let identity = input
        .user_sources()
        .sources()
        .iter()
        .nth(user_index)
        .expect("diagnostic file maps to a parsed user source")
        .identity()
        .clone();
    (identity, diagnostic.span)
}

#[test]
fn legacy_entry_anchor_uses_the_smallest_source_identity_across_container_permutations() {
    let core = core_file();
    let low_identity = source_identity(3);
    let low_span = Span::new(30, 39);
    let high_span = Span::new(80, 99);

    for (low_source, high_source, message) in [
        (
            file(Vec::new()),
            file(Vec::new()),
            "missing entry point: declare `fun main()`",
        ),
        (
            in_package(file(vec![fun("main", Vec::new())]), "low"),
            in_package(file(vec![fun("main", Vec::new())]), "high"),
            "multiple entry points: declare exactly one `fun main()`",
        ),
    ] {
        let mut low_source = low_source;
        low_source.span = low_span;
        let mut high_source = high_source;
        high_source.span = high_span;

        for sources in [
            vec![(90, high_source.clone()), (3, low_source.clone())],
            vec![(3, low_source.clone()), (90, high_source.clone())],
        ] {
            let (owner, span) = entry_error_owner(&core, sources, message);
            assert_eq!(owner, low_identity);
            assert_eq!(span, Some(low_span));
        }
    }
}

#[test]
fn library_output_skips_entry_selection_entirely() {
    let first = in_package(file(vec![fun("main", Vec::new())]), "first");
    let second = in_package(file(vec![fun("main", Vec::new())]), "second");
    let output = lower_frontend_sources(&core_file(), vec![(9, second), (1, first)]);

    assert_eq!(
        select_cone_output_kind(&output.export, scoop_identity::RequestedConeKind::Library)
            .expect("library output never requires an executable entry"),
        hir::ConeOutputKind::Library
    );
}

#[test]
fn executable_output_seals_private_current_main_and_exact_signature() {
    let span = Span::new(31, 47);
    let mut declaration = declaration_at(fun("main", Vec::new()), span);
    let ast::Decl::Function(function) = &mut declaration else {
        unreachable!("the fixture declaration is a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span,
    };
    let mut core = core_file();
    core.declarations.push(fun("main", Vec::new()));
    let output = lower_frontend_sources(&core, vec![(10, file(vec![declaration]))]);

    let selected = select_cone_output_kind(
        &output.export,
        scoop_identity::RequestedConeKind::Executable,
    )
    .expect("the private current-Cone main is a valid entry");
    let hir::ConeOutputKind::Executable { local_entry } = selected else {
        panic!("the requested executable has a sealed entry")
    };
    let function = local_entry.local_function().function();
    assert_eq!(output.export.functions[function].span, span);
    let hir::HirSourceFunctionIdentity::Plain(record) = output.export.function_identities[function]
        .source_identity()
        .expect("the selected declaration is a source function")
    else {
        panic!("the selected source function is non-generic")
    };
    assert_eq!(local_entry.declaration(), record.id());
    let exact_unit = output.export.type_identities[output.export.unit]
        .exact()
        .expect("Unit has an exact identity")
        .id();
    assert_eq!(local_entry.source_signature().unit(), exact_unit);
    assert_eq!(
        local_entry.source_signature().as_exact().effect(),
        scoop_identity::Effect::Ordinary
    );
    assert_eq!(
        local_entry.source_signature().as_exact().receiver(),
        scoop_identity::OptionalExactOwner::Absent
    );
    assert!(
        local_entry
            .source_signature()
            .as_exact()
            .parameters()
            .is_empty()
    );
    assert_eq!(
        local_entry.source_signature_fingerprint(),
        scoop_identity::SourceSignatureFingerprint::from_signature(local_entry.source_signature())
            .unwrap()
    );
}

#[test]
fn missing_executable_entry_lists_invalid_mains_in_persistent_location_order() {
    let generic_span = Span::new(11, 19);
    let suspend_span = Span::new(21, 29);
    let non_unit_span = Span::new(31, 39);
    let generic = in_package(
        file(vec![declaration_at(
            fun_sig("main", vec!["T"], Vec::new(), None, Vec::new()),
            generic_span,
        )]),
        "generic",
    );
    let suspending = in_package(
        file(vec![declaration_at(
            suspend_fun("main", Vec::new()),
            suspend_span,
        )]),
        "suspending",
    );
    let non_unit = in_package(
        file(vec![declaration_at(
            fun_expr(
                "main",
                Vec::new(),
                Vec::new(),
                Some(ty_named("Int")),
                int_lit(1),
            ),
            non_unit_span,
        )]),
        "nonunit",
    );
    let output = lower_frontend_sources(
        &core_file(),
        vec![(9, non_unit), (1, generic), (5, suspending)],
    );

    let errors = select_cone_output_kind(
        &output.export,
        scoop_identity::RequestedConeKind::Executable,
    )
    .expect_err("no invalid main can fill an executable entry");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "missing executable entry: declare exactly one ordinary `fun main(): Unit`"
    );
    assert_eq!(errors[0].file, 2);
    assert_eq!(errors[0].span, Some(Span::new(0, 0)));
    assert_eq!(errors[0].notes.len(), 3);
    assert_eq!(
        errors[0]
            .notes
            .iter()
            .map(|note| (note.file, note.span, note.message.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (
                2,
                generic_span,
                "`main` is not eligible because it is generic"
            ),
            (
                3,
                suspend_span,
                "`main` is not eligible because it is suspend"
            ),
            (
                1,
                non_unit_span,
                "`main` is not eligible because it does not return `Unit`"
            ),
        ]
    );
}

#[test]
fn valid_entry_ignores_invalid_named_overloads_and_extensions() {
    let valid_span = Span::new(51, 59);
    let valid = in_package(
        file(vec![declaration_at(fun("main", Vec::new()), valid_span)]),
        "valid",
    );
    let generic = in_package(
        file(vec![fun_sig(
            "main",
            vec!["T"],
            Vec::new(),
            None,
            Vec::new(),
        )]),
        "generic",
    );
    let extension = in_package(
        file(vec![extension_expr(
            ty_named("Int"),
            "main",
            Vec::new(),
            Vec::new(),
            None,
            unit_lit(),
        )]),
        "extension",
    );
    let output = lower_frontend_sources(
        &core_file(),
        vec![(70, generic), (20, extension), (10, valid)],
    );

    let selected = select_cone_output_kind(
        &output.export,
        scoop_identity::RequestedConeKind::Executable,
    )
    .expect("only the ordinary valid declaration is selected");
    let hir::ConeOutputKind::Executable { local_entry } = selected else {
        panic!("the requested executable has a sealed entry")
    };
    assert_eq!(
        output.export.functions[local_entry.local_function().function()].span,
        valid_span
    );
}

#[test]
fn multiple_entries_are_reported_in_persistent_source_location_order() {
    let low_span = Span::new(41, 49);
    let high_span = Span::new(81, 89);
    let low = in_package(
        file(vec![declaration_at(fun("main", Vec::new()), low_span)]),
        "low",
    );
    let high = in_package(
        file(vec![declaration_at(fun("main", Vec::new()), high_span)]),
        "high",
    );
    let output = lower_frontend_sources(&core_file(), vec![(90, high), (3, low)]);

    let errors = select_cone_output_kind(
        &output.export,
        scoop_identity::RequestedConeKind::Executable,
    )
    .expect_err("two valid mains are ambiguous");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "multiple executable entries: declare exactly one ordinary `fun main(): Unit`"
    );
    assert_eq!((errors[0].file, errors[0].span), (2, Some(low_span)));
    assert_eq!(errors[0].notes.len(), 1);
    assert_eq!(
        (errors[0].notes[0].file, errors[0].notes[0].span),
        (1, high_span)
    );
}
