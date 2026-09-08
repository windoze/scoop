use super::*;

fn source<'a>(
    source: &'a ast::SourceFile,
    provider: hir::IntrinsicProviderId,
    name: &'a str,
) -> ProviderSource<'a> {
    ProviderSource {
        source,
        provider,
        name,
        source_text: "",
    }
}

fn lower_user_sources(
    core: &ast::SourceFile,
    first: &ast::SourceFile,
    remaining: &[(&ast::SourceFile, &str)],
) -> Result<hir::Output, Vec<ast::Diagnostic>> {
    let core_provider = hir::IntrinsicProviderId::from_raw(17);
    let user_provider = hir::IntrinsicProviderId::from_raw(29);
    let input = Stage1CompilationInput::new(
        vec![source(core, core_provider, "core.scoop")],
        source(first, user_provider, "first.scoop"),
        remaining
            .iter()
            .map(|&(file, name)| source(file, user_provider, name))
            .collect(),
    )
    .expect("the test user sources share one provider");
    lower_stage1_compilation_input(&input, IntrinsicDeclarationPolicy::CoreOnly)
}

fn set_private(declaration: &mut ast::Decl) {
    let ast::Decl::Function(function) = declaration else {
        panic!("the test declaration is a function")
    };
    function.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
}

#[test]
fn multiple_user_sources_share_one_current_unit_and_keep_file_private_domains() {
    let core = core_file();
    let mut private = fun("privateHelper", Vec::new());
    set_private(&mut private);
    let first = file(vec![
        fun("sharedHelper", Vec::new()),
        private,
        fun("sameFile", vec![stmt(call("privateHelper", Vec::new()))]),
    ]);
    let second = file(vec![fun(
        "main",
        vec![
            stmt(call("sharedHelper", Vec::new())),
            stmt(call("sameFile", Vec::new())),
        ],
    )]);

    let output = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect("all user sources are one current-unit declaration side");
    let module = output.export;
    let user_provider = hir::IntrinsicProviderId::from_raw(29);
    let shared = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "sharedHelper").then_some(function))
        .expect("the cross-source helper is present");
    assert_eq!(
        shared.access.lookup.0.constraints(),
        &[hir::AccessConstraint::Cone(user_provider)]
    );
    let private = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "privateHelper").then_some(function))
        .expect("the private helper is present");
    assert_eq!(
        private.access.lookup.0.constraints(),
        &[
            hir::AccessConstraint::Cone(user_provider),
            hir::AccessConstraint::File(hir::VisibilityFile {
                provider: user_provider,
                index: 1,
            }),
        ]
    );
    assert_eq!(module.source_files[1].provider, user_provider);
    assert_eq!(module.source_files[2].provider, user_provider);
}

#[test]
fn private_declaration_does_not_cross_user_source_slots() {
    let core = core_file();
    let mut private = fun("privateHelper", Vec::new());
    set_private(&mut private);
    let first = file(vec![private]);
    let second = file(vec![fun(
        "main",
        vec![stmt(call("privateHelper", Vec::new()))],
    )]);

    let errors = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect_err("file-private lookup must use the exact user source slot");
    assert!(errors.iter().any(|error| {
        error.file == 2 && error.message == "function `privateHelper` is not accessible here"
    }));
}

#[test]
fn a_later_user_source_does_not_gain_core_intrinsic_authority() {
    let core = core_file();
    let first = file(Vec::new());
    let ast::Decl::Function(mut intrinsic) =
        intrinsic_fun("wipe", "rt_gc_collect", Vec::new(), None)
    else {
        panic!("the intrinsic builder returns a function")
    };
    intrinsic.body = ast::FunctionBody::None;
    let second = file(vec![
        ast::Decl::Function(intrinsic),
        fun("main", Vec::new()),
    ]);

    let errors = lower_user_sources(&core, &first, &[(&second, "second.scoop")])
        .expect_err("source position must not grant core intrinsic authority");
    assert!(errors.iter().any(|error| {
        error.file == 2 && error.message == "`@Intrinsic` is only allowed in the core library"
    }));
}

#[test]
fn stage1_input_rejects_mixed_user_providers() {
    let first = file(Vec::new());
    let second = file(vec![fun("main", Vec::new())]);
    let expected = hir::IntrinsicProviderId::from_raw(29);
    let actual = hir::IntrinsicProviderId::from_raw(31);

    let error = Stage1CompilationInput::new(
        Vec::new(),
        source(&first, expected, "first.scoop"),
        vec![source(&second, actual, "second.scoop")],
    )
    .err()
    .expect("mixed user providers cannot form a stage-1 input");
    assert_eq!(error.source_index, 1);
    assert_eq!(error.expected_provider, expected);
    assert_eq!(error.actual_provider, actual);
}
