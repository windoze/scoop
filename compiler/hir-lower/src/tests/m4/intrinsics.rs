use super::*;

// --- positive: intrinsics and the sysroot frame ---

#[test]
fn intrinsic_functions_are_marked() {
    let module = lower_user(file(vec![fun("main", vec![])])).expect("must lower");
    let start_coroutine = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|f| f.name == "startCoroutine")
        .expect("core declares startCoroutine");
    assert!(matches!(
        start_coroutine.kind,
        FunctionKind::Intrinsic(intrinsic)
            if intrinsic.kind == hir::IntrinsicFunctionKind::CoroutineStart
    ));
}

/// Multiple core files share the declaration scope (sysroot frame):
/// an enum declared in one core file is visible in the user file.
#[test]
fn multiple_core_files_share_scope() {
    let core_extra = file(vec![color_decl()]);
    let user = file(vec![fun(
        "main",
        vec![val("c", field(var("Color"), "Green"))],
    )]);
    let output =
        lower(&[core_file(), core_extra, user]).expect("multi-core-file program must lower");
    let dump = hir::dump(&output.export);
    assert!(
        dump.contains("VariantConstruct Color.Green : Color"),
        "{dump}"
    );
}

/// A diagnostic in a core file carries that file's index.
#[test]
fn core_file_diagnostic_carries_file_index() {
    let bad_core = file(vec![
        enum_decl(
            "Option",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        ),
        struct_decl("Point", vec![]),
        struct_decl("Point", vec![]),
    ]);
    let errors = lower(&[bad_core, file(vec![fun("main", vec![])])])
        .expect_err("duplicate struct in core must fail");
    // The same run also reports the missing core `Throwable` (M8);
    // this test is about the core file index of the duplicate.
    let duplicate = errors
        .iter()
        .find(|e| e.message == "duplicate struct `Point`")
        .expect("the duplicate struct must be diagnosed");
    assert_eq!(duplicate.file, 0);
}

// --- negative: intrinsics ---

#[test]
fn unknown_intrinsic_is_an_error() {
    let mut core = core_file();
    core.declarations
        .push(intrinsic_fun("reset", "rt_reset", vec![], None));
    let errors =
        lower(&[core, file(vec![fun("main", vec![])])]).expect_err("unknown intrinsic must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown intrinsic `rt_reset`");
    assert_eq!(errors[0].file, 0);
}

#[test]
fn intrinsic_in_user_file_is_an_error() {
    let Decl::Function(mut wipe) = intrinsic_fun("wipe", "rt_gc_collect", vec![], None) else {
        unreachable!()
    };
    wipe.body = FunctionBody::None;
    let user = file(vec![Decl::Function(wipe), fun("main", vec![])]);
    let errors = lower(&[core_file(), user]).expect_err("user `@Intrinsic` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`@Intrinsic` is only allowed in the core library"
    );
    assert_eq!(errors[0].file, 1);
}

#[test]
fn allowlisted_test_provider_carries_typed_intrinsic_provenance() {
    let mut core = core_file();
    core.declarations.retain(
        |decl| !matches!(decl, Decl::Function(function) if function.name.text == "gcCollect"),
    );
    let user = file(vec![
        intrinsic_fun("gcCollect", "rt_gc_collect", vec![], None),
        fun("main", vec![stmt(call("gcCollect", vec![]))]),
    ]);
    let core_provider = hir::IntrinsicProviderId::from_raw(3);
    let test_provider = hir::IntrinsicProviderId::from_raw(7);
    let unit = CompilationUnit {
        core: vec![ProviderSource {
            source: &core,
            provider: core_provider,
            name: "core.scoop",
            source_text: "",
        }],
        user: ProviderSource {
            source: &user,
            provider: test_provider,
            name: "user.scoop",
            source_text: "",
        },
    };
    let output = lower_compilation_unit(
        &unit,
        IntrinsicDeclarationPolicy::AllowListedForTesting {
            providers: std::collections::HashSet::from([test_provider]),
        },
    )
    .expect("an explicitly allowlisted test provider may define an intrinsic");
    let function = output
        .export
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == "gcCollect")
        .expect("the user declaration is present");
    assert!(matches!(
        function.kind,
        hir::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::GcCollect,
            provider,
        }) if provider == test_provider
    ));
    let concrete = output
        .local
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == "gcCollect")
        .expect("the concrete declaration is present");
    assert!(matches!(
        concrete.kind,
        hir::concrete::FunctionKind::Intrinsic(hir::IntrinsicFunction {
            kind: hir::IntrinsicFunctionKind::GcCollect,
            provider,
        }) if provider == test_provider
    ));
}

#[test]
fn a_typed_intrinsic_kind_has_one_defining_provider() {
    let core = core_file();
    let user = file(vec![
        intrinsic_generic_fun("anotherStart", "coroutine_start", vec!["T"], vec![], None),
        fun("main", vec![]),
    ]);
    let core_provider = hir::IntrinsicProviderId::from_raw(3);
    let test_provider = hir::IntrinsicProviderId::from_raw(7);
    let unit = CompilationUnit {
        core: vec![ProviderSource {
            source: &core,
            provider: core_provider,
            name: "core.scoop",
            source_text: "",
        }],
        user: ProviderSource {
            source: &user,
            provider: test_provider,
            name: "user.scoop",
            source_text: "",
        },
    };
    let errors = lower_compilation_unit(
        &unit,
        IntrinsicDeclarationPolicy::AllowListedForTesting {
            providers: std::collections::HashSet::from([test_provider]),
        },
    )
    .expect_err("one typed intrinsic kind cannot have two declarations");
    assert!(errors.iter().any(|error| {
        error.file == 1
            && error.message
                == "intrinsic `coroutine_start` is already defined by provider 3 as `startCoroutine`; provider 7 cannot define it again"
    }));
}

#[test]
fn unknown_annotation_is_an_error() {
    let mut core = core_file();
    core.declarations.push(Decl::Function(FunctionDecl {
        annotations: vec![ast::Annotation {
            name: ident("Unknown"),
            args: vec![],
            span: sp(),
        }],
        is_suspend: false,
        is_override: false,
        operator: None,
        infix: None,
        modifier: ast::MethodModifier::Final,
        receiver_ty: None,
        name: ident("pure_fn"),
        type_params: vec![],
        params: vec![],
        return_ty: None,
        where_clause: None,
        body: FunctionBody::Block(block(vec![])),
        span: sp(),
    }));
    let errors =
        lower(&[core, file(vec![fun("main", vec![])])]).expect_err("unknown annotation must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "unsupported annotation `@Unknown` in milestone M12"
    );
}
