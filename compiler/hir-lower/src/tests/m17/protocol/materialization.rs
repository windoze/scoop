use super::*;

#[test]
fn named_inputs_run_in_source_order_before_declaration_order_defaults() {
    let target = with_default(
        with_default(
            fun_sig(
                "target",
                vec![],
                vec![
                    ("x", ty_named("Int")),
                    ("y", ty_named("Int")),
                    ("z", ty_named("Int")),
                ],
                Some(ty_named("Int")),
                vec![ret(Some(var("x")))],
            ),
            0,
            call("defaultX", vec![]),
        ),
        2,
        call("defaultZ", vec![]),
    );
    let call = source_call(
        "target",
        vec![
            named_argument("z", call("explicitZ", vec![])),
            named_argument("y", call("explicitY", vec![])),
        ],
    );
    let module = lower_user(file(vec![
        fun_expr(
            "defaultX",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun_expr(
            "defaultZ",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun_expr(
            "explicitZ",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(3),
        ),
        fun_expr(
            "explicitY",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(4),
        ),
        target,
        fun("main", vec![val("result", call)]),
    ]))
    .expect("named/default call must lower");

    let dump = hir::dump(&module);
    let explicit_z = dump.find("Call explicitZ").expect("explicit z call");
    let explicit_y = dump.find("Call explicitY").expect("explicit y call");
    let default_x = dump.find("Call defaultX").expect("default x call");
    assert!(explicit_z < explicit_y && explicit_y < default_x, "{dump}");
    assert!(!dump.contains("Call defaultZ"), "{dump}");
}

#[test]
fn every_source_callable_and_constructor_uses_explicit_temporaries() {
    let output = lower_user_output(file(vec![
        fun_sig(
            "accept",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            vec![],
        ),
        struct_decl("Box", vec![("item", ty_named("Int"))]),
        class_decl(
            ast::ClassModifier::Final,
            "Holder",
            vec![(false, "content", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        enum_decl(
            "Choice",
            vec![],
            vec![variant_constructor(
                "Item",
                vec![("selected", ty_named("Int"), None)],
            )],
        ),
        fun(
            "main",
            vec![
                stmt(call("accept", vec![int_lit(1)])),
                val("boxed", call("Box", vec![int_lit(2)])),
                val("held", call("Holder", vec![int_lit(3)])),
                val(
                    "choice",
                    method_call(var("Choice"), "Item", vec![int_lit(4)]),
                ),
            ],
        ),
    ]))
    .expect("all source call shapes should share argument materialization");

    let body = function_body(&output.export, "main");
    let names = body
        .locals
        .iter()
        .map(|(_, local)| local.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names.iter().filter(|name| **name == "$argument.0").count(),
        4
    );
    for parameter in ["value", "item", "content", "selected"] {
        assert!(
            names.contains(&format!("$parameter.{parameter}").as_str()),
            "missing parameter temporary for {parameter}: {names:?}"
        );
    }
}

fn callback_registration(mode: &str) -> ast::Expr {
    callback_registration_with_value(mode, ty_named("Int"))
}

fn callback_registration_with_value(mode: &str, value_type: ast::TypeRef) -> ast::Expr {
    let native_signature = ty_function(
        false,
        vec![
            value_type.clone(),
            ty_generic("Ptr", vec![ty_named("Unit")]),
        ],
        value_type.clone(),
    );
    let callback = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: Some(value_type),
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    };
    typed_source_call(
        "foreignCallback",
        vec![native_signature],
        vec![
            named_argument("mode", field(var("ForeignCallbackMode"), mode)),
            named_argument("callback", callback),
            named_argument("contextIndex", int_lit(1)),
        ],
    )
}

fn callback_registration_source() -> ast::SourceFile {
    file(vec![fun(
        "main",
        vec![unsafe_block(vec![val(
            "registered",
            callback_registration("Reusable"),
        )])],
    )])
}

fn rebuild_callback_identities(
    module: &hir::Module,
) -> Result<hir::HirCallbackRegistrationIdentities, hir::HirCallbackRegistrationIdentityError> {
    hir::HirCallbackRegistrationIdentities::from_registrations(
        hir::HirCallbackRegistrationIdentityInputs {
            registrations: &module.foreign_callback_registrations,
            functions: &module.functions,
            lambdas: &module.lambdas,
            anonymous_functions: &module.anonymous_functions,
            local_functions: &module.local_functions,
            class_constructors: &module.class_constructors,
            struct_constructors: &module.struct_constructors,
            function_identities: &module.function_identities,
            property_accessor_identities: &module.property_accessor_identities,
            constructor_identities: &module.constructor_identities,
            enum_member_identities: &module.enum_member_identities,
            callback_modes: module.core_protocols.foreign_callbacks.modes,
            type_inputs: hir::HirTypeIdentityInputs {
                types: &module.types,
                function_types: &module.function_types,
                structs: &module.structs,
                struct_applications: &module.struct_applications,
                enums: &module.enums,
                enum_applications: &module.enum_applications,
                classes: &module.classes,
                class_applications: &module.class_applications,
                interfaces: &module.interfaces,
                interface_applications: &module.interface_applications,
                objects: &module.objects,
                core_types: hir::HirCoreTypeIdentityAuthority::Defined(
                    &module.core_protocols.fundamental_types,
                ),
                nominal_identities: &module.nominal_identities,
            },
            unit: module.unit,
        },
    )
}

#[test]
fn callback_intrinsic_reads_named_constants_through_materialized_temporaries() {
    let source = callback_registration_source();
    let mut shifted_source = source.clone();
    shifted_source
        .declarations
        .insert(0, fun("unrelated", vec![]));
    let module = lower_user(source)
        .expect("named callback arguments should survive explicit materialization");
    let shifted = lower_user(shifted_source)
        .expect("an unrelated declaration must not affect callback identity");

    let (registration_id, registration) = module
        .foreign_callback_registrations
        .iter()
        .next()
        .expect("one callback registration");
    assert_eq!(
        definition_path(&registration.definition_path),
        vec![(
            scoop_identity::StructuralDefinitionSiteRole::CallbackConversion,
            0,
        )]
    );
    assert_eq!(registration.context_index, 1);
    assert_eq!(
        registration.mode,
        module.core_protocols.foreign_callbacks.modes.reusable()
    );
    let identity = &module.callback_registration_identities[registration_id];
    let shifted_identity = shifted
        .callback_registration_identities
        .records()
        .first()
        .expect("the shifted module has one callback identity");
    assert_eq!(identity, shifted_identity);
    assert_eq!(module.callback_registration_identities.records().len(), 1);
    assert_eq!(identity.key().context_index().get(), 1);
    assert_eq!(
        identity.key().mode(),
        scoop_identity::CallbackMode::Reusable
    );
    assert_eq!(identity.key().source_signature().parameters().len(), 2);
    assert!(matches!(
        identity.key().source_signature().result(),
        scoop_identity::SourceCAbiReturn::Value(_)
    ));
    assert_eq!(identity.key().managed_signature().parameters().len(), 1);
    assert_eq!(
        identity.key().parent(),
        module.function_identities[module.entry()]
            .source_identity()
            .expect("main has a source identity")
            .lexical_parent()
    );
    let hir::FunctionKind::User(main) = &module.functions[module.entry()].kind else {
        panic!("main has a user body")
    };
    let dump = hir::dump(&module);
    assert!(dump.contains("Local $parameter.callback"), "{dump}");
    assert!(matches!(
        local_init(main, "registered").kind,
        hir::ExprKind::ForeignCallbackRegister { .. }
    ));
}

#[test]
fn callback_signature_adds_its_source_nominals_to_the_native_boundary_witness() {
    let mut payload = struct_decl("CallbackPayload", vec![("value", ty_named("Int"))]);
    let Decl::Struct(declaration) = &mut payload else {
        unreachable!("struct_decl creates a struct")
    };
    declaration.annotations.push(ast::Annotation {
        name: ident("CLayout"),
        args: Vec::new(),
        span: sp(),
    });
    let without_callback = lower_user_output(file(vec![payload.clone(), fun("main", vec![])]))
        .expect("the unused callback payload lowers");
    let with_callback = lower_user_output(file(vec![
        payload,
        fun(
            "main",
            vec![unsafe_block(vec![val(
                "registered",
                callback_registration_with_value("Reusable", ty_named("CallbackPayload")),
            )])],
        ),
    ]))
    .expect("the callback payload is C-safe and lowers");
    let module = with_callback.export.module();
    let payload = module
        .structs
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "CallbackPayload").then_some(id))
        .expect("CallbackPayload declaration exists");
    let owner = hir::NativeBoundaryNominalOwner::Concrete(
        module.nominal_identities[payload]
            .concrete_type_id()
            .expect("CallbackPayload is a concrete source nominal"),
    );

    assert!(
        without_callback
            .native_boundary_types
            .records()
            .iter()
            .all(|record| record.owner() != owner)
    );
    let record = with_callback
        .native_boundary_types
        .records()
        .iter()
        .find(|record| record.owner() == owner)
        .expect("callback signature contributes its payload witness");
    assert!(matches!(
        record.shape(),
        hir::NativeBoundaryNominalShape::Struct {
            c_layout: hir::NativeBoundaryCLayoutPolicy::CLayout {
                aligned: scoop_identity::CLayoutOverride::Natural,
                packed: scoop_identity::CLayoutOverride::Natural,
            },
            fields,
        } if fields.len() == 1
    ));
}

#[test]
fn callback_identity_relation_rejects_an_invalid_context_parameter() {
    let mut module = lower_user(callback_registration_source())
        .expect("the callback registration fixture lowers")
        .into_module();
    let (registration, _) = module
        .foreign_callback_registrations
        .iter()
        .next()
        .expect("one callback registration");
    module.foreign_callback_registrations[registration].context_index = u32::MAX;

    let error = rebuild_callback_identities(&module)
        .expect_err("the context parameter must belong to the native signature");
    assert!(error.to_string().contains("context index is out of bounds"));
}

#[test]
fn callback_identity_uses_the_nearest_local_callable_parent() {
    let local = local_fun_sig(
        "install",
        vec![],
        vec![],
        None,
        vec![unsafe_block(vec![val(
            "registered",
            callback_registration("OneShot"),
        )])],
    );
    let module = lower_user(file(vec![fun("main", vec![local])]))
        .expect("a callback conversion inside a local function lowers");
    let local_function = module
        .functions
        .iter()
        .find_map(|(function, declaration)| {
            declaration.name.ends_with(".install").then_some(function)
        })
        .expect("the local source function exists");
    let (registration, _) = module
        .foreign_callback_registrations
        .iter()
        .next()
        .expect("one callback registration");
    let identity = &module.callback_registration_identities[registration];

    assert_eq!(
        identity.key().parent(),
        module.function_identities[local_function]
            .source_identity()
            .expect("the local function has a source identity")
            .lexical_parent()
    );
    assert_eq!(identity.key().mode(), scoop_identity::CallbackMode::OneShot);
}

#[test]
fn local_default_uses_definition_binding_and_prior_parameter() {
    let mut local = local_fun_sig(
        "choose",
        vec![],
        vec![("first", ty_named("Int")), ("second", ty_named("Int"))],
        Some(ty_named("Int")),
        vec![ret(Some(var("second")))],
    );
    let ast::StatementKind::LocalFunction(function) = &mut local.kind else {
        unreachable!("local_fun_sig creates a local function")
    };
    function.params[0].syntax = ast::ParameterSyntax::Default {
        expression: var("base"),
        equals_span: sp(),
    };
    function.params[1].syntax = ast::ParameterSyntax::Default {
        expression: var("first"),
        equals_span: sp(),
    };
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("base", int_lit(11)),
            local,
            val("result", call("choose", vec![])),
        ],
    )]))
    .expect("local defaults may capture lexical vals and reference prior parameters");
    let dump = hir::dump(&module);
    assert!(dump.contains("Local base : Int"), "{dump}");
    assert!(dump.contains("Local $parameter.first : Int"), "{dump}");
    assert!(
        dump.contains("LocalFunctionCall local0 $local.0.choose"),
        "{dump}"
    );
}

#[test]
fn a_default_does_not_infer_an_unconstrained_type_parameter() {
    let declaration = with_default(
        fun_sig(
            "empty",
            vec!["T"],
            vec![("values", ty_generic("Array", vec![ty_named("T")]))],
            Some(ty_generic("Array", vec![ty_named("T")])),
            vec![ret(Some(var("values")))],
        ),
        0,
        array_lit(vec![]),
    );
    let errors = lower_user(file(vec![
        declaration,
        fun("main", vec![val("values", call("empty", vec![]))]),
    ]))
    .expect_err("the omitted default contributes no inference constraint");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot infer a unique type argument for `T`")
    }));
}

#[test]
fn a_generic_default_callable_body_keeps_the_callee_application() {
    let function_type = ty_function(false, vec![ty_named("T")], ty_named("T"));
    let factory = with_default(
        fun_sig(
            "factory",
            vec!["T"],
            vec![("callback", function_type.clone())],
            Some(function_type),
            vec![ret(Some(var("callback")))],
        ),
        0,
        identity_lambda(),
    );
    let output = lower_user_output(file(vec![
        factory,
        fun(
            "main",
            vec![val(
                "callback",
                typed_call("factory", vec![ty_named("Int")], vec![]),
            )],
        ),
    ]))
    .expect("a default lambda from a generic callee must concretize at Int");
    assert!(output.local.functions.iter().any(|(_, function)| {
        function.name.starts_with("$lambda")
            && matches!(
                output.local.types[function.return_ty].kind,
                hir::concrete::TypeKind::Integer(hir::IntegerKind::SIGNED_32)
            )
    }));
}

#[test]
fn positional_vararg_assembles_fresh_array_but_named_vararg_keeps_identity() {
    let collect = with_vararg(
        fun_sig(
            "collect",
            vec![],
            vec![("values", ty_named("Int"))],
            Some(ty_generic("Array", vec![ty_named("Int")])),
            vec![ret(Some(var("values")))],
        ),
        0,
    );
    let mixed = source_call(
        "collect",
        vec![
            ast::CallArgument::positional(int_lit(1)),
            spread_argument(var("existing")),
        ],
    );
    let whole = source_call("collect", vec![named_argument("values", var("existing"))]);
    let module = lower_user(file(vec![
        collect,
        fun(
            "main",
            vec![
                val("existing", array_lit(vec![int_lit(2), int_lit(3)])),
                val("mixed", mixed),
                val("whole", whole),
                val("empty", call("collect", vec![])),
            ],
        ),
    ]))
    .expect("all vararg source forms must lower");
    let dump = hir::dump(&module);
    assert_eq!(
        dump.matches("ArrayAssembly : Array<Int>").count(),
        2,
        "{dump}"
    );
    assert!(
        dump.contains("CopyArray\n          Local $argument.1"),
        "{dump}"
    );
}

#[test]
fn base_constructor_delegation_uses_the_complete_source_protocol() {
    let base = with_constructor_syntax(
        with_constructor_syntax(
            with_constructor_syntax(
                class_decl(
                    ast::ClassModifier::Open,
                    "Base",
                    vec![
                        (false, "head", ty_named("Int")),
                        (false, "values", ty_named("Int")),
                        (false, "tail", ty_named("Int")),
                    ],
                    None,
                    vec![],
                    vec![],
                ),
                0,
                ast::ParameterSyntax::Default {
                    expression: int_lit(10),
                    equals_span: sp(),
                },
            ),
            1,
            ast::ParameterSyntax::Vararg {
                modifier_span: sp(),
                default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
            },
        ),
        2,
        ast::ParameterSyntax::Default {
            expression: int_lit(30),
            equals_span: sp(),
        },
    );
    let mut derived = class_decl(
        ast::ClassModifier::Final,
        "Derived",
        vec![(false, "more", ty_generic("Array", vec![ty_named("Int")]))],
        Some(("Base", vec![])),
        vec![],
        vec![],
    );
    let Decl::Class(class) = &mut derived else {
        unreachable!()
    };
    class.supertypes = vec![constructor_supertype(
        ty_named("Base"),
        vec![
            ast::CallArgument::positional(int_lit(1)),
            ast::CallArgument::positional(int_lit(2)),
            spread_argument(var("more")),
        ],
    )];
    let mut whole = class_decl(
        ast::ClassModifier::Final,
        "Whole",
        vec![
            (false, "more", ty_generic("Array", vec![ty_named("Int")])),
            (false, "last", ty_named("Int")),
        ],
        Some(("Base", vec![])),
        vec![],
        vec![],
    );
    let Decl::Class(class) = &mut whole else {
        unreachable!()
    };
    class.supertypes = vec![constructor_supertype(
        ty_named("Base"),
        vec![
            named_argument("values", var("more")),
            named_argument("tail", var("last")),
        ],
    )];

    let module = lower_user(file(vec![base, derived, whole, fun("main", vec![])]))
        .expect("base delegation must support defaults and positional vararg parts");
    let (_, derived) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Derived")
        .expect("derived class");
    let hir::ClassConstructorKind::Primary {
        base:
            hir::BaseInitialization::Super {
                arguments: delegation,
                ..
            },
        ..
    } = &module.class_constructors[derived.constructors[0]].kind
    else {
        panic!("derived primary constructor delegates to its base")
    };
    assert_eq!(delegation.args.len(), 3);
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::ArrayAssembly(_),
                ..
            },
            ..
        }
    )));
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(30)),
                ..
            },
            ..
        }
    )));

    let (_, whole) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Whole")
        .expect("whole-array derived class");
    let hir::ClassConstructorKind::Primary {
        base:
            hir::BaseInitialization::Super {
                arguments: delegation,
                ..
            },
        ..
    } = &module.class_constructors[whole.constructors[0]].kind
    else {
        panic!("whole primary constructor delegates to its base")
    };
    assert!(!delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::ArrayAssembly(_),
                ..
            },
            ..
        }
    )));
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(10)),
                ..
            },
            ..
        }
    )));
}
