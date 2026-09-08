use super::*;

const REQUEST: ast::Stage1RequestId = ast::Stage1RequestId::from_raw(23);

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
    let (first_handle, first_source) = sources.next().expect("entry tests supply user sources");
    let parsed = |handle, source| {
        ast::ParsedSource::new(ast::Stage1SourceHandle::new(REQUEST, handle), source)
    };
    ast::AllParsedSources::try_new(
        REQUEST,
        ast::NonEmptyVec::new(
            parsed(first_handle, first_source),
            sources
                .map(|(handle, source)| parsed(handle, source))
                .collect(),
        ),
    )
    .expect("entry tests use unique handles in one request")
}

fn stage1_input<'a>(
    core: &'a ast::SourceFile,
    user_sources: ast::AllParsedSources,
) -> Stage1CompilationInput<'a> {
    Stage1CompilationInput::new(
        vec![ProviderSource {
            source: core,
            provider: hir::IntrinsicProviderId::from_raw(17),
            name: "core.scoop",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(29),
        user_sources,
        |_| Stage1SourceDetails {
            display_locator: "user.scoop",
            source_text: "",
        },
    )
}

fn lower_sources(
    core: &ast::SourceFile,
    user_sources: Vec<(u32, ast::SourceFile)>,
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let input = stage1_input(core, parsed_sources(user_sources));
    lower_stage1_compilation_input(&input, IntrinsicDeclarationPolicy::CoreOnly)
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

    let input = stage1_input(
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
    let output = lower_stage1_compilation_input(
        &input,
        IntrinsicDeclarationPolicy::AllowListedForTesting {
            providers: std::collections::HashSet::from([hir::IntrinsicProviderId::from_raw(29)]),
        },
    )
    .expect("only the fully qualified current-unit main is an entry candidate");

    let entry = &output.export.functions[output.export.entry];
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
) -> (ast::Stage1SourceHandle, Option<Span>) {
    let input = stage1_input(core, parsed_sources(user_sources));
    let errors = lower_stage1_compilation_input(&input, IntrinsicDeclarationPolicy::CoreOnly)
        .expect_err("the test input has no unique legacy entry");
    let diagnostic = errors
        .iter()
        .find(|diagnostic| diagnostic.message == expected_message)
        .expect("the expected legacy entry diagnostic is present");
    let user_index = diagnostic
        .file
        .checked_sub(1)
        .expect("legacy entry diagnostics belong to a user source");
    let handle = input
        .user_sources()
        .sources()
        .iter()
        .nth(user_index)
        .expect("diagnostic file maps to a parsed user source")
        .source_handle();
    (handle, diagnostic.span)
}

#[test]
fn legacy_entry_anchor_uses_the_smallest_handle_across_container_permutations() {
    let core = core_file();
    let low_handle = ast::Stage1SourceHandle::new(REQUEST, 3);
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
            assert_eq!(owner, low_handle);
            assert_eq!(span, Some(low_span));
        }
    }
}
