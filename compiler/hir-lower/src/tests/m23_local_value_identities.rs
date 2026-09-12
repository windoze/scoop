use std::collections::HashSet;

use scoop_hir::concrete;
use scoop_identity::LocalValueSelector;

use super::*;

fn concrete_function<'module>(
    module: &'module concrete::Module,
    name: &str,
) -> (concrete::FunctionId, &'module concrete::Function) {
    module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .unwrap_or_else(|| panic!("missing concrete function `{name}`"))
}

fn concrete_body(function: &concrete::Function) -> &concrete::Body {
    let concrete::FunctionKind::User(body) = &function.kind else {
        panic!("expected a concrete user body")
    };
    body
}

fn concrete_local_by_name<'body>(
    body: &'body concrete::Body,
    name: &str,
) -> (concrete::LocalId, &'body concrete::Local) {
    body.locals
        .iter()
        .find(|(_, local)| local.name == name)
        .unwrap_or_else(|| panic!("missing concrete local `{name}`"))
}

fn local_definition<'module>(
    module: &'module concrete::Module,
    identity: &concrete::LocalValueIdentityRecord,
) -> &'module scoop_identity::DefinitionOrigin {
    let record = module
        .local_value_identities
        .definition_origins()
        .get(identity.id())
        .expect("the source-backed local value has one definition origin");
    assert!(matches!(
        record.subject(),
        scoop_identity::DefinitionOriginSubject::LocalValue(subject) if subject == identity.id()
    ));
    record.origin()
}

fn reference(receiver: Option<Expr>, name: &str) -> Expr {
    Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: receiver.map(Box::new),
        name: ident(name),
        span: sp(),
    }
}

fn lambda(tail: Expr) -> Expr {
    Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: None,
        body: block(vec![stmt(tail)]),
        span: sp(),
    }
}

fn anonymous(tail: Expr) -> Expr {
    Expr::AnonymousFunction {
        id: ast::AnonymousFunctionId(0),
        is_suspend: false,
        params: Vec::new(),
        return_ty: Some(ty_named("Int")),
        body: block(vec![ret(Some(tail))]),
        span: sp(),
    }
}

#[test]
fn generic_applications_give_the_same_template_local_distinct_persistent_values() {
    let mut copy = fun_sig(
        "copy",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        vec![val("saved", var("value")), ret(Some(var("saved")))],
    );
    let Decl::Function(copy_declaration) = &mut copy else {
        unreachable!("fun_sig creates a function")
    };
    copy_declaration.params[0].name.span = Span::new(10, 15);
    let ast::FunctionBody::Block(copy_body) = &mut copy_declaration.body else {
        unreachable!("the copy fixture has a block body")
    };
    let ast::StatementKind::ValDecl(saved) = &mut copy_body.statements[0].kind else {
        unreachable!("the first copy statement declares saved")
    };
    let ast::Pattern::Binding(saved_name) = &mut saved.target else {
        unreachable!("saved is a binding pattern")
    };
    saved_name.span = Span::new(20, 25);
    let output = lower_user_output(file(vec![
        copy,
        fun(
            "main",
            vec![
                stmt(call("copy", vec![int_lit(1)])),
                stmt(call("copy", vec![str_lit("value")])),
            ],
        ),
    ]))
    .expect("one generic body materializes in two exact contexts");
    let module = &output.local;
    let origins = module.local_value_identities.definition_origins().records();
    assert!(origins.windows(2).all(|pair| {
        pair[0]
            .subject()
            .compare_sort_key(pair[1].subject())
            .is_lt()
    }));
    assert!(origins.iter().all(|record| matches!(
        record.subject(),
        scoop_identity::DefinitionOriginSubject::LocalValue(_)
    )));
    let instances = module
        .functions
        .iter()
        .filter(|(_, function)| function.name == "copy")
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);

    let mut parameter_ids = HashSet::new();
    let mut saved_ids = HashSet::new();
    for (function_id, function) in instances {
        let body = concrete_body(function);
        let parameter = function.params[0].local;
        let parameter_identity = module
            .local_value_identities
            .function_local(function_id, parameter);
        assert_eq!(parameter_identity.key().owner(), function.materialization);
        assert!(matches!(
            parameter_identity.key().selector(),
            LocalValueSelector::Parameter {
                declaration_index: 0
            }
        ));
        assert_eq!(
            (
                local_definition(module, parameter_identity)
                    .span()
                    .start_byte(),
                local_definition(module, parameter_identity)
                    .span()
                    .end_byte(),
            ),
            (10, 15)
        );
        parameter_ids.insert(parameter_identity.id());

        let (saved, _) = concrete_local_by_name(body, "saved");
        let saved_identity = module
            .local_value_identities
            .function_local(function_id, saved);
        assert_eq!(saved_identity.key().owner(), function.materialization);
        assert!(matches!(
            saved_identity.key().selector(),
            LocalValueSelector::LocalDeclaration { .. }
        ));
        assert_eq!(
            (
                local_definition(module, saved_identity).span().start_byte(),
                local_definition(module, saved_identity).span().end_byte(),
            ),
            (20, 25)
        );
        saved_ids.insert(saved_identity.id());
    }
    assert_eq!(parameter_ids.len(), 2);
    assert_eq!(saved_ids.len(), 2);
}

#[test]
fn local_function_capture_parameter_reuses_the_captured_value_identity() {
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![
            val("base", int_lit(2)),
            local_fun_sig(
                "add",
                Vec::new(),
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                vec![ret(Some(binary(
                    ast::BinOp::Add,
                    var("base"),
                    var("value"),
                )))],
            ),
            stmt(call("add", vec![int_lit(40)])),
        ],
    )]))
    .expect("a captured local function has one lifted capture parameter");
    let module = &output.local;
    let (main_id, main) = concrete_function(module, "main");
    let (base, _) = concrete_local_by_name(concrete_body(main), "base");
    let base_identity = module.local_value_identities.function_local(main_id, base);

    let (_, local_function) = module
        .local_functions
        .iter()
        .next()
        .expect("one concrete local function");
    let lifted = &module.functions[local_function.function];
    let capture_parameter = lifted.params[0].local;
    let capture_identity = module
        .local_value_identities
        .function_local(local_function.function, capture_parameter);

    assert_eq!(capture_identity.id(), base_identity.id());
    assert_eq!(capture_identity.key().owner(), main.materialization);
    assert!(matches!(
        capture_identity.key().selector(),
        LocalValueSelector::LocalDeclaration { .. }
    ));
    assert_eq!(
        lifted.materialization.context(),
        main.materialization.context(),
        "lexical capture aliases must remain in the enclosing materialization context"
    );
}

#[test]
fn nested_lambda_captures_resolve_to_the_original_persistent_value() {
    let nested_type = ty_function(
        false,
        Vec::new(),
        ty_function(false, Vec::new(), ty_named("Int")),
    );
    let output = lower_user_output(file(vec![
        fun_expr(
            "make",
            Vec::new(),
            vec![("base", ty_named("Int"))],
            Some(nested_type),
            lambda(lambda(var("base"))),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("nested captures retain one source value identity");
    let module = &output.local;
    let (make_id, make) = concrete_function(module, "make");
    let source = module
        .local_value_identities
        .function_local(make_id, make.params[0].local);
    assert_eq!(module.lambdas.len(), 2);

    for (lambda, declaration) in module.lambdas.iter() {
        assert_eq!(declaration.captures.len(), 1);
        let captured = module.local_value_identities.lambda_capture(lambda, 0);
        assert_eq!(captured.id(), source.id());
        assert_eq!(captured.key(), source.key());
    }
}

#[test]
fn anonymous_and_callable_reference_captures_share_the_source_identity() {
    let operation_type = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![
            val("base", int_lit(40)),
            val_ty(
                "anonymous",
                Some(ty_function(false, Vec::new(), ty_named("Int"))),
                anonymous(var("base")),
            ),
            local_fun_sig(
                "add",
                Vec::new(),
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                vec![ret(Some(binary(
                    ast::BinOp::Add,
                    var("base"),
                    var("value"),
                )))],
            ),
            val_ty("reference", Some(operation_type), reference(None, "add")),
        ],
    )]))
    .expect("anonymous and callable-reference captures materialize together");
    let module = &output.local;
    let (main_id, main) = concrete_function(module, "main");
    let (base, _) = concrete_local_by_name(concrete_body(main), "base");
    let source = module.local_value_identities.function_local(main_id, base);

    let (anonymous, declaration) = module.anonymous_functions.iter().next().unwrap();
    assert_eq!(declaration.captures.len(), 1);
    assert_eq!(
        module
            .local_value_identities
            .anonymous_function_capture(anonymous, 0)
            .id(),
        source.id()
    );

    let (reference, declaration) = module.callable_references.iter().next().unwrap();
    assert_eq!(declaration.captures.len(), 1);
    assert_eq!(
        module
            .local_value_identities
            .callable_reference_capture(reference, 0)
            .id(),
        source.id()
    );
}

#[test]
fn constructor_receivers_and_parameters_have_typed_persistent_values() {
    let output = lower_user_output(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Box",
            vec![(false, "value", ty_named("Int"))],
            None,
            Vec::new(),
            Vec::new(),
        ),
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("box", call("Box", vec![int_lit(1)])),
                val("point", call("Point", vec![int_lit(2)])),
            ],
        ),
    ]))
    .expect("class and struct constructors materialize together");
    let module = &output.local;

    let class = module
        .classes
        .iter()
        .find_map(|(id, class)| (class.name == "Box").then_some(id))
        .expect("Box class");
    let (class_constructor, constructor) = module
        .class_constructors
        .iter()
        .find(|(_, constructor)| constructor.class == class)
        .expect("Box constructor");
    let receiver = module
        .local_value_identities
        .class_receiver(class_constructor);
    let parameter = module
        .local_value_identities
        .class_parameter(class_constructor, 0);
    local_definition(module, receiver);
    local_definition(module, parameter);
    assert_eq!(receiver.key().owner(), constructor.materialization);
    assert!(matches!(
        receiver.key().selector(),
        LocalValueSelector::This
    ));
    assert_eq!(parameter.key().owner(), constructor.materialization);
    assert!(matches!(
        parameter.key().selector(),
        LocalValueSelector::Parameter {
            declaration_index: 0
        }
    ));

    let structure = module
        .structs
        .iter()
        .find_map(|(id, structure)| (structure.name == "Point").then_some(id))
        .expect("Point struct");
    let (struct_constructor, constructor) = module
        .struct_constructors
        .iter()
        .find(|(_, constructor)| constructor.structure == structure)
        .expect("Point constructor");
    let parameter = module
        .local_value_identities
        .struct_parameter(struct_constructor, 0);
    local_definition(module, parameter);
    assert_eq!(parameter.key().owner(), constructor.materialization);
    assert!(matches!(
        parameter.key().selector(),
        LocalValueSelector::Parameter {
            declaration_index: 0
        }
    ));
    assert!(
        module
            .local_value_identities
            .struct_receiver(struct_constructor)
            .is_none(),
        "a primary struct constructor has no receiver value"
    );
}

#[test]
fn unbound_callable_reference_has_no_receiver_value() {
    let output = lower_user_output(file(vec![
        fun_sig(
            "identity",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("value")))],
        ),
        fun(
            "main",
            vec![val_ty(
                "operation",
                Some(ty_function(false, vec![ty_named("Int")], ty_named("Int"))),
                reference(None, "identity"),
            )],
        ),
    ]))
    .expect("an unbound callable reference materializes");
    let module = &output.local;
    let (reference_id, _) = module
        .callable_references
        .iter()
        .next()
        .expect("one concrete callable reference");

    assert!(
        module
            .local_value_identities
            .callable_reference_receiver(reference_id)
            .is_none()
    );
}

#[test]
fn bound_receiver_values_follow_the_reference_materialization_context() {
    let operation_ty = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let output = lower_user_output(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Mapper",
            vec![(false, "offset", ty_named("Int"))],
            None,
            Vec::new(),
            vec![method_expr(
                "map",
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                binary(ast::BinOp::Add, var("value"), var("offset")),
            )],
        ),
        fun_sig(
            "bind",
            vec!["T"],
            vec![("mapper", ty_named("Mapper")), ("value", ty_named("T"))],
            None,
            vec![val_ty(
                "operation",
                Some(operation_ty),
                reference(Some(var("mapper")), "map"),
            )],
        ),
        fun(
            "main",
            vec![
                val("mapper", call("Mapper", vec![int_lit(2)])),
                stmt(call("bind", vec![var("mapper"), int_lit(1)])),
                stmt(call("bind", vec![var("mapper"), str_lit("value")])),
            ],
        ),
    ]))
    .expect("a bound receiver materializes in two generic contexts");
    let module = &output.local;
    let references = module.callable_references.iter().collect::<Vec<_>>();
    assert_eq!(references.len(), 2);
    let mut receiver_identities = HashSet::new();

    for (reference_id, reference) in references {
        let receiver = module
            .local_value_identities
            .callable_reference_receiver(reference_id)
            .expect("a bound reference has one receiver value");
        assert_eq!(
            receiver.key().owner(),
            *reference.identity.materialization()
        );
        assert!(matches!(
            receiver.key().selector(),
            LocalValueSelector::BoundReceiver { path }
                if path == reference.identity.definition_path()
        ));
        local_definition(module, receiver);
        receiver_identities.insert(receiver.id());
    }
    assert_eq!(receiver_identities.len(), 2);
}

#[test]
fn synthetic_local_values_do_not_acquire_definition_origins() {
    let output = lower_user_output(file(vec![
        fun_expr(
            "truth",
            Vec::new(),
            vec![("value", ty_named("Boolean"))],
            Some(ty_named("Boolean")),
            var("value"),
        ),
        fun(
            "main",
            vec![val(
                "flag",
                binary(
                    ast::BinOp::And,
                    bool_lit(true),
                    call("truth", vec![bool_lit(false)]),
                ),
            )],
        ),
    ]))
    .expect("short-circuit lowering creates a synthetic local value");
    let module = &output.local;
    let (main_id, main) = concrete_function(module, "main");
    let (local, declaration) = concrete_body(main)
        .locals
        .iter()
        .find(|(_, local)| local.name.starts_with("$shortCircuit."))
        .expect("short-circuit lowering creates one hidden result");
    assert!(matches!(
        declaration.definition,
        hir::LocalValueDefinitionSite::Synthetic
    ));
    let identity = module.local_value_identities.function_local(main_id, local);
    assert!(
        module
            .local_value_identities
            .definition_origins()
            .get(identity.id())
            .is_none()
    );
}
