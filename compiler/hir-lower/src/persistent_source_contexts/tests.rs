use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{CallableOwner, DeclarationName, SourceContextKey};

use crate::tests::{
    block, call, class_decl, core_file, core_source_identity, file, fun, fun_expr, ident,
    method_expr, sp, stmt, test_source_identity, ty_function, ty_named,
};

fn runtime_property(name: &str, expression: ast::Expr) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("SourceLocation"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(expression),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

fn source(unrelated_prefix: bool) -> ast::SourceFile {
    let callback = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(Vec::new()),
        body: block(vec![stmt(call("getCurrentSourceLocation", Vec::new()))]),
        span: sp(),
    };
    let mut recorder = class_decl(
        ast::ClassModifier::Final,
        "Recorder",
        vec![(false, "origin", ty_named("SourceLocation"))],
        None,
        Vec::new(),
        vec![method_expr(
            "locate",
            Vec::new(),
            Some(ty_named("SourceLocation")),
            call("getCurrentSourceLocation", Vec::new()),
        )],
    );
    let ast::Decl::Class(recorder_declaration) = &mut recorder else {
        unreachable!()
    };
    let ast::ClassConstructorDecl::Declared(constructor) = &mut recorder_declaration.constructor
    else {
        unreachable!()
    };
    constructor.parameters[0].syntax = ast::ParameterSyntax::Default {
        expression: call("getCurrentSourceLocation", Vec::new()),
        equals_span: sp(),
    };
    let mut declarations = vec![
        recorder,
        fun_expr(
            "probe",
            Vec::new(),
            Vec::new(),
            Some(ty_named("SourceLocation")),
            call("getCurrentSourceLocation", Vec::new()),
        ),
        fun_expr(
            "factory",
            Vec::new(),
            Vec::new(),
            Some(ty_function(false, Vec::new(), ty_named("SourceLocation"))),
            callback,
        ),
        runtime_property("bootLocation", call("getCurrentSourceLocation", Vec::new())),
        fun("main", Vec::new()),
    ];
    if unrelated_prefix {
        declarations.insert(0, fun("unrelated", Vec::new()));
    }
    file(declarations)
}

fn lower(unrelated_prefix: bool, display_locator: &str) -> hir::Output {
    let core = core_file();
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(
            test_source_identity("src/contexts.scoop"),
            source(unrelated_prefix),
        ),
        Vec::new(),
    ))
    .expect("source-context identity test source is valid");
    let input = crate::DefinedTestSources::try_new(
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
            display_locator,
            source_text: "",
        },
    )
    .expect("source-context identity test inputs are valid");
    crate::lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("source-context identity fixture lowers")
}

fn source_function_named(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(function, _)| {
            let identity = module.function_identities[function].source_identity()?;
            matches!(
                identity.declaration().name(),
                DeclarationName::Named(candidate) if candidate.as_str() == name
            )
            .then_some(function)
        })
        .unwrap_or_else(|| panic!("missing source function {name}"))
}

fn context_for_function(module: &hir::Module, function: hir::FunctionId) -> hir::SourceContextId {
    module
        .source_contexts
        .iter()
        .find_map(|(context, value)| {
            (value.subject() == &hir::SourceContextSubject::Function(function)).then_some(context)
        })
        .expect("function source context")
}

fn context_for_constructor(
    module: &hir::Module,
    constructor: hir::ClassConstructorId,
) -> hir::SourceContextId {
    module
        .source_contexts
        .iter()
        .find_map(|(context, value)| {
            (value.subject()
                == &hir::SourceContextSubject::Constructor(hir::SourceContextConstructor::Class(
                    constructor,
                )))
                .then_some(context)
        })
        .expect("constructor source context")
}

fn stable_context_ids(module: &hir::Module) -> [scoop_identity::PersistentSourceContextId; 6] {
    let source = test_source_identity("src/contexts.scoop");
    let file = module
        .source_contexts
        .iter()
        .find_map(|(context, value)| {
            (value.source() == &source && value.subject() == &hir::SourceContextSubject::File)
                .then_some(context)
        })
        .expect("file source context");
    let probe = source_function_named(module, "probe");
    let member = source_function_named(module, "locate");
    let factory = source_function_named(module, "factory");
    let lambda = module
        .lambdas
        .iter()
        .find(|(_, literal)| {
            literal.definition.source().unwrap().1 == hir::LexicalDefinitionRoot::Function(factory)
        })
        .expect("factory lambda")
        .1;
    let lambda_context = module
        .source_contexts
        .iter()
        .find_map(|(context, value)| {
            (value.subject()
                == &hir::SourceContextSubject::LexicalCallable {
                    root: lambda.definition.source().unwrap().1,
                    path: lambda.definition_path.clone(),
                    role: scoop_identity::LexicalCallableRole::LambdaBody,
                })
                .then_some(context)
        })
        .expect("lambda source context");
    let constructor = module
        .class_constructors
        .iter()
        .find(|(constructor, declaration)| {
            module.classes[declaration.owner].name == "Recorder"
                && module.constructor_identities[*constructor]
                    .source_record()
                    .is_some()
        })
        .expect("Recorder source constructor")
        .0;
    let boot_property = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "bootLocation")
        .expect("bootLocation property")
        .0;
    let initialization = module
        .initialization_units
        .iter()
        .find(|(_, unit)| {
            matches!(
                unit.kind,
                hir::InitializationUnitKind::EagerTopLevel { property, .. }
                    if property == boot_property
            )
        })
        .expect("bootLocation initialization unit")
        .1;
    let initialization_context = context_for_function(module, initialization.initializer);
    let constructor_context = context_for_constructor(module, constructor);
    let lambda_identity = module.function_identities[lambda.definition.source_function()]
        .generated_record()
        .expect("lambda generated identity");
    let constructor_identity = module.constructor_identities[constructor]
        .source_record()
        .expect("Recorder source constructor identity");
    let hir::HirFunctionIdentity::Initialization {
        unit: initialization_unit,
        ..
    } = &module.function_identities[initialization.initializer]
    else {
        panic!("initialization function identity")
    };

    assert_eq!(
        module.source_context_identities[file].key(),
        &SourceContextKey::File {
            source: source.clone(),
        }
    );
    assert_eq!(
        module.source_context_identities[lambda_context].key(),
        &SourceContextKey::Callable {
            source: source.clone(),
            owner: CallableOwner::Generated(lambda_identity.id()),
        }
    );
    assert_eq!(
        module.source_context_identities[constructor_context].key(),
        &SourceContextKey::Callable {
            source: source.clone(),
            owner: CallableOwner::Constructor(constructor_identity.id()),
        }
    );
    assert_eq!(
        module.source_context_identities[initialization_context].key(),
        &SourceContextKey::Initialization {
            source,
            unit: module.initialization_unit_identities[*initialization_unit].id(),
        }
    );

    [
        module.source_context_identities[file].id(),
        module.source_context_identities[context_for_function(module, probe)].id(),
        module.source_context_identities[context_for_function(module, member)].id(),
        module.source_context_identities[lambda_context].id(),
        module.source_context_identities[constructor_context].id(),
        module.source_context_identities[initialization_context].id(),
    ]
}

#[test]
fn source_contexts_use_typed_persistent_owners_and_ignore_local_order_and_display_paths() {
    let first = lower(false, "/checkout/one/contexts.scoop");
    let module = &first.export;
    let source = test_source_identity("src/contexts.scoop");
    let probe = source_function_named(module, "probe");
    let probe_context = context_for_function(module, probe);
    let probe_identity = module.function_identities[probe]
        .source_identity()
        .expect("probe source identity");
    let expected_probe = match probe_identity {
        hir::HirSourceFunctionIdentity::Plain(record) => CallableOwner::Function(record.id()),
        hir::HirSourceFunctionIdentity::Generic(record) => {
            CallableOwner::GenericTemplate(record.id())
        }
    };
    assert_eq!(
        module.source_context_identities[probe_context].key(),
        &SourceContextKey::Callable {
            source: source.clone(),
            owner: expected_probe,
        }
    );

    let ids = stable_context_ids(module);
    let stable = lower(true, "/different/checkout/contexts.scoop");
    assert_eq!(ids, stable_context_ids(&stable.export));
}

fn identity_inputs(module: &hir::Module) -> hir::HirSourceContextIdentityInputs<'_> {
    hir::HirSourceContextIdentityInputs {
        source_files: &module.source_files,
        source_contexts: &module.source_contexts,
        structs: &module.structs,
        enums: &module.enums,
        classes: &module.classes,
        interfaces: &module.interfaces,
        objects: &module.objects,
        functions: &module.functions,
        struct_constructors: &module.struct_constructors,
        class_constructors: &module.class_constructors,
        properties: &module.properties,
        initialization_units: &module.initialization_units,
        singleton_values: &module.singleton_values,
        lambdas: &module.lambdas,
        anonymous_functions: &module.anonymous_functions,
        nominal_identities: &module.nominal_identities,
        function_identities: &module.function_identities,
        property_accessor_identities: &module.property_accessor_identities,
        constructor_identities: &module.constructor_identities,
        property_identities: &module.property_identities,
        initialization_unit_identities: &module.initialization_unit_identities,
    }
}

#[test]
fn source_context_relation_rejects_a_record_for_another_context() {
    let output = lower(false, "/checkout/contexts.scoop");
    let module = &output.export;
    let mut identities = module
        .source_context_identities
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let probe = source_function_named(module, "probe");
    let probe_context = context_for_function(module, probe);
    let file_context = module
        .source_contexts
        .iter()
        .find_map(|(context, value)| {
            (value.source() == &test_source_identity("src/contexts.scoop")
                && value.subject() == &hir::SourceContextSubject::File)
                .then_some(context)
        })
        .expect("file context");
    identities.swap(
        probe_context.into_raw().into_u32() as usize,
        file_context.into_raw().into_u32() as usize,
    );

    assert!(matches!(
        hir::HirSourceContextIdentities::checked(identity_inputs(module), identities),
        Err(hir::HirSourceContextIdentityError::IdentityMismatch { .. })
    ));
}

#[test]
fn source_context_relation_requires_one_file_context_per_source() {
    let output = lower(false, "/checkout/contexts.scoop");
    let module = &output.export;
    let identities = module
        .source_context_identities
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let mut source_files = module.source_files.clone();
    source_files.push(hir::SourceFileMetadata {
        provider: hir::IntrinsicProviderId::from_raw(1),
        identity: test_source_identity("src/unrepresented.scoop"),
        name: "ignored display path".to_string(),
        source: String::new(),
        canonical_record: None,
    });
    let mut inputs = identity_inputs(module);
    inputs.source_files = &source_files;

    assert!(matches!(
        hir::HirSourceContextIdentities::checked(inputs, identities),
        Err(hir::HirSourceContextIdentityError::MissingFileContext { .. })
    ));
}
