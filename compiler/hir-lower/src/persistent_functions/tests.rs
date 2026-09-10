use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{
    DeclarationName, DeclarationScope, DefinitionOwnerAtom, GeneratedCallableKey,
    LexicalCallableParent,
};

use crate::tests::{
    block, call, core_file, core_source_identity, file, fun, fun_expr, int_lit, local_fun_sig, ret,
    sp, stmt, test_source_identity, ty_function, ty_named, val_ty, var,
};

fn lambda(id: u32, statements: Vec<ast::Statement>) -> ast::Expr {
    ast::Expr::Lambda {
        id: ast::LambdaId(id),
        is_suspend: false,
        parameters: Some(Vec::new()),
        body: block(statements),
        span: sp(),
    }
}

fn lower(declarations: Vec<ast::Decl>) -> hir::Output {
    let core = core_file();
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(
            test_source_identity("src/functions.scoop"),
            file(declarations),
        ),
        Vec::new(),
    ))
    .expect("test source identity is valid");
    let input = crate::LegacyCombinedSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator: "/checkout/functions.scoop",
            source_text: "",
        },
    )
    .expect("test sources are valid");
    crate::lower_legacy_combined_sources(&input, crate::IntrinsicDeclarationPolicy::CoreOnly)
        .expect("persistent function identity fixture lowers")
}

fn source_function_named(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, _)| {
            let identity = module.function_identities[id].source_identity()?;
            matches!(
                identity.declaration().name(),
                DeclarationName::Named(declaration_name) if declaration_name.as_str() == name
            )
            .then_some(id)
        })
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

#[test]
fn local_source_identity_uses_own_binders_and_nearest_callable_owner() {
    let result_type = ty_function(false, Vec::new(), ty_named("T"));
    let closure = lambda(
        0,
        vec![
            local_fun_sig(
                "plainLocal",
                Vec::new(),
                Vec::new(),
                Some(ty_named("T")),
                vec![ret(Some(var("value")))],
            ),
            local_fun_sig(
                "genericLocal",
                vec!["U"],
                vec![("item", ty_named("U"))],
                Some(ty_named("U")),
                vec![ret(Some(var("item")))],
            ),
            stmt(var("value")),
        ],
    );
    let output = lower(vec![
        fun_expr(
            "identityFactory",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(result_type),
            closure,
        ),
        fun("main", Vec::new()),
    ]);
    let module = &output.export;
    let factory = source_function_named(module, "identityFactory");
    let plain = source_function_named(module, "plainLocal");
    let generic = source_function_named(module, "genericLocal");
    let (_, closure) = module.lambdas.iter().next().expect("lambda declaration");

    let hir::HirFunctionIdentity::Source(factory_identity) = &module.function_identities[factory]
    else {
        panic!("factory is a source function")
    };
    assert!(matches!(
        factory_identity,
        hir::HirSourceFunctionIdentity::Generic(_)
    ));

    let hir::HirFunctionIdentity::LexicalGenerated(closure_identity) =
        &module.function_identities[closure.function]
    else {
        panic!("lambda body has a generated identity")
    };
    assert!(matches!(
        closure_identity.key(),
        GeneratedCallableKey::Lexical { parent, .. }
            if *parent == factory_identity.lexical_parent()
    ));

    let hir::HirFunctionIdentity::Source(plain_identity) = &module.function_identities[plain]
    else {
        panic!("plain local is a source function")
    };
    assert!(matches!(
        plain_identity,
        hir::HirSourceFunctionIdentity::Plain(_)
    ));
    assert_eq!(
        plain_identity
            .declaration()
            .duplicate_signature()
            .type_parameter_count(),
        0
    );
    assert!(matches!(
        plain_identity.declaration().scope(),
        DeclarationScope::LexicalScoped { path, .. }
            if path == &module.local_functions
                .iter()
                .find(|(_, local)| local.function == plain)
                .expect("plain local declaration")
                .1
                .definition_path
    ));
    assert_eq!(
        plain_identity.declaration().owners().owners().last(),
        Some(&DefinitionOwnerAtom::GeneratedCallable(
            closure_identity.id()
        ))
    );

    let hir::HirFunctionIdentity::Source(generic_identity) = &module.function_identities[generic]
    else {
        panic!("generic local is a source function")
    };
    assert!(matches!(
        generic_identity,
        hir::HirSourceFunctionIdentity::Generic(_)
    ));
    assert_eq!(
        generic_identity
            .declaration()
            .duplicate_signature()
            .type_parameter_count(),
        1
    );
}

#[test]
fn nested_generated_callable_uses_the_nearest_template_parent() {
    let signature = ty_function(false, Vec::new(), ty_named("Int"));
    let inner = lambda(0, vec![stmt(int_lit(42))]);
    let outer = lambda(
        1,
        vec![
            val_ty("inner", Some(signature.clone()), inner),
            stmt(call("inner", Vec::new())),
        ],
    );
    let output = lower(vec![
        fun_expr(
            "nestedFactory",
            Vec::new(),
            Vec::new(),
            Some(signature),
            outer,
        ),
        fun("main", Vec::new()),
    ]);
    let module = &output.export;
    let factory = source_function_named(module, "nestedFactory");
    let factory = module.function_identities[factory]
        .source_identity()
        .expect("factory source identity");
    let mut declarations = module
        .lambdas
        .iter()
        .map(|(_, lambda)| lambda)
        .collect::<Vec<_>>();
    declarations.sort_by_key(|lambda| lambda.definition_path.segments().len());
    let [outer, inner] = declarations.as_slice() else {
        panic!("fixture has exactly two lambda declarations")
    };
    let outer = module.function_identities[outer.function]
        .generated_record()
        .expect("outer generated identity");
    let inner = module.function_identities[inner.function]
        .generated_record()
        .expect("inner generated identity");
    let expected_parent =
        LexicalCallableParent::from_generated_key(outer.key()).expect("outer is a lexical key");

    assert!(matches!(
        outer.key(),
        GeneratedCallableKey::Lexical { parent, .. }
            if *parent == factory.lexical_parent()
    ));
    assert!(matches!(
        inner.key(),
        GeneratedCallableKey::Lexical { parent, .. }
            if *parent == expected_parent && *parent != factory.lexical_parent()
    ));
}
