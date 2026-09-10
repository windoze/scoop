use std::collections::{HashMap, HashSet};

use scoop_identity::{CallableArguments, CallableInstantiationOwner, CallableTemplateOrigin};

use super::*;

fn generic_class(
    name: &str,
    parameters: Vec<ast::TypeParamDecl>,
    constructor: Vec<(bool, &str, TypeRef)>,
) -> Decl {
    let mut declaration = class_decl(
        ast::ClassModifier::Final,
        name,
        constructor,
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!()
    };
    class.type_params = parameters;
    declaration
}

fn callback_registration(lambda_id: u32, mode: &str) -> ast::Expr {
    let native_signature = ty_function(
        false,
        vec![ty_named("Int"), ty_generic("Ptr", vec![ty_named("Unit")])],
        ty_named("Int"),
    );
    let callback = ast::Expr::Lambda {
        id: ast::LambdaId(lambda_id),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: Some(ty_named("Int")),
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

fn lambda_with_local(id: u32) -> ast::Expr {
    ast::Expr::Lambda {
        id: ast::LambdaId(id),
        is_suspend: false,
        parameters: Some(Vec::new()),
        body: block(vec![
            local_fun_sig(
                "plainLocal",
                Vec::new(),
                Vec::new(),
                Some(ty_named("T")),
                vec![ret(Some(var("value")))],
            ),
            stmt(call("plainLocal", Vec::new())),
        ]),
        span: sp(),
    }
}

#[test]
fn callable_applications_form_one_persistent_lexical_graph() {
    let output = lower_user_output(file(vec![
        fun_expr(
            "factory",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_function(false, Vec::new(), ty_named("T"))),
            lambda_with_local(0),
        ),
        fun(
            "main",
            vec![
                val("intFactory", call("factory", vec![int_lit(1)])),
                val("stringFactory", call("factory", vec![str_lit("value")])),
            ],
        ),
    ]))
    .expect("generic factories and their lexical callables must materialize");
    let module = &output.local;

    let factory_applications = module
        .functions
        .iter()
        .filter(|(_, function)| function.name == "factory")
        .map(|(_, function)| {
            assert!(matches!(
                function.materialization.template(),
                hir::concrete::CallableTemplateOwner::GenericFunction(_)
            ));
            assert!(matches!(
                function.emission,
                hir::concrete::FunctionEmission::Materialized { .. }
            ));
            let hir::concrete::CallableMaterializationContext::Application(application) =
                function.materialization.context()
            else {
                panic!("factory<T> has a persistent application")
            };
            let record = module
                .callable_applications
                .get(application)
                .expect("factory application record");
            assert!(matches!(
                record.key().origin(),
                CallableTemplateOrigin::GenericFunction(_)
            ));
            assert_eq!(
                record.key().instantiation_owner(),
                CallableInstantiationOwner::NoOwner
            );
            assert!(matches!(
                record.key().callable_arguments(),
                CallableArguments::Arguments(arguments) if arguments.as_slice().len() == 1
            ));
            application
        })
        .collect::<HashSet<_>>();
    assert_eq!(factory_applications.len(), 2);

    let lambda_contexts = module
        .functions
        .iter()
        .filter(|(_, function)| function.name.starts_with("$lambda"))
        .map(|(_, function)| {
            assert!(matches!(
                function.materialization.template(),
                hir::concrete::CallableTemplateOwner::Generated(_)
            ));
            let hir::concrete::CallableMaterializationContext::Application(application) =
                function.materialization.context()
            else {
                panic!("the generated lambda inherits the factory application")
            };
            application
        })
        .collect::<HashSet<_>>();
    assert_eq!(lambda_contexts, factory_applications);

    let application_positions = module
        .callable_applications
        .records()
        .iter()
        .enumerate()
        .map(|(position, record)| (record.id(), position))
        .collect::<HashMap<_, _>>();
    let local_owners = module
        .functions
        .iter()
        .filter(|(_, function)| function.name.ends_with(".plainLocal"))
        .map(|(_, function)| {
            assert!(matches!(
                function.materialization.template(),
                hir::concrete::CallableTemplateOwner::Function(_)
            ));
            let hir::concrete::CallableMaterializationContext::Application(application) =
                function.materialization.context()
            else {
                panic!("plainLocal has a persistent lexical application")
            };
            let record = module
                .callable_applications
                .get(application)
                .expect("plainLocal application record");
            assert!(matches!(
                record.key().origin(),
                CallableTemplateOrigin::Function(_)
            ));
            assert!(matches!(
                record.key().callable_arguments(),
                CallableArguments::NoCallableArguments
            ));
            let CallableInstantiationOwner::EnclosingCallableApplication(owner) =
                record.key().instantiation_owner()
            else {
                panic!("plainLocal is owned by the enclosing factory application")
            };
            assert!(application_positions[&owner] < application_positions[&application]);
            owner
        })
        .collect::<HashSet<_>>();
    assert_eq!(local_owners, factory_applications);
}

#[test]
fn generic_nominal_constructors_have_persistent_applications() {
    let output = lower_user_output(file(vec![
        generic_class(
            "Box",
            vec![type_param("T")],
            vec![(false, "value", ty_named("T"))],
        ),
        generic_struct_decl("Cell", vec!["T"], vec![("value", ty_named("T"))]),
        fun(
            "main",
            vec![
                val("intBox", call("Box", vec![int_lit(1)])),
                val("stringBox", call("Box", vec![str_lit("box")])),
                val("intCell", struct_init("Cell", vec![int_lit(2)])),
                val("stringCell", struct_init("Cell", vec![str_lit("cell")])),
            ],
        ),
    ]))
    .expect("generic class and struct constructors must materialize");
    let module = &output.local;

    let class_applications = module
        .class_constructors
        .iter()
        .filter(|(_, constructor)| module.classes[constructor.class].name.starts_with("Box$"))
        .map(|(_, constructor)| constructor.materialization)
        .collect::<Vec<_>>();
    let struct_applications = module
        .struct_constructors
        .iter()
        .filter(|(_, constructor)| {
            module.structs[constructor.structure]
                .name
                .starts_with("Cell$")
        })
        .map(|(_, constructor)| constructor.materialization)
        .collect::<Vec<_>>();

    for materializations in [&class_applications, &struct_applications] {
        assert_eq!(materializations.len(), 2);
        let applications = materializations
            .iter()
            .map(|materialization| {
                let hir::concrete::CallableTemplateOwner::Constructor(origin) =
                    materialization.template()
                else {
                    panic!("a source constructor keeps its constructor template")
                };
                let hir::concrete::CallableMaterializationContext::Application(application) =
                    materialization.context()
                else {
                    panic!("a generic nominal constructor has an application")
                };
                let record = module
                    .callable_applications
                    .get(application)
                    .expect("constructor application record");
                assert_eq!(
                    record.key().origin(),
                    CallableTemplateOrigin::Constructor(origin)
                );
                assert!(matches!(
                    record.key().instantiation_owner(),
                    CallableInstantiationOwner::ExactNominalOwner(_)
                ));
                assert!(matches!(
                    record.key().callable_arguments(),
                    CallableArguments::NoCallableArguments
                ));
                application
            })
            .collect::<HashSet<_>>();
        assert_eq!(applications.len(), 2);
    }

    let adapters = module
        .class_constructors
        .iter()
        .filter(|(_, constructor)| {
            matches!(
                constructor.materialization.template(),
                hir::concrete::CallableTemplateOwner::Generated(_)
            )
        })
        .map(|(_, constructor)| constructor.materialization)
        .collect::<Vec<_>>();
    assert_eq!(adapters.len(), 1);
    assert!(matches!(
        adapters[0].context(),
        hir::concrete::CallableMaterializationContext::NoSubstitution
    ));
}

#[test]
fn parameter_free_callback_has_one_persistent_application() {
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![unsafe_block(vec![val(
            "registered",
            callback_registration(0, "Reusable"),
        )])],
    )]))
    .expect("a parameter-free callback conversion must materialize");
    let module = &output.local;
    let (_, registration) = module
        .foreign_callback_registrations
        .iter()
        .next()
        .expect("one concrete callback registration");
    assert_eq!(module.foreign_callback_registrations.len(), 1);
    assert_eq!(module.callback_applications.len(), 1);

    let record = module
        .callback_applications
        .get(registration.application)
        .expect("the concrete registration references its canonical application record");
    assert_eq!(
        record.key().registration(),
        output.export.callback_registration_identities.records()[0].id()
    );
    assert!(matches!(
        record.key().context(),
        hir::concrete::CallableMaterializationContext::NoSubstitution
    ));
}

#[test]
fn generic_callback_site_is_distinct_for_each_enclosing_application() {
    let output = lower_user_output(file(vec![
        fun_sig(
            "register",
            vec!["T"],
            vec![("value", ty_named("T"))],
            None,
            vec![unsafe_block(vec![val(
                "registered",
                callback_registration(0, "OneShot"),
            )])],
        ),
        fun(
            "main",
            vec![
                stmt(call("register", vec![int_lit(1)])),
                stmt(call("register", vec![str_lit("value")])),
            ],
        ),
    ]))
    .expect("a callback source site must materialize in both generic applications");
    let module = &output.local;

    let enclosing_applications = module
        .functions
        .iter()
        .filter(|(_, function)| function.name == "register")
        .map(|(_, function)| {
            let hir::concrete::CallableMaterializationContext::Application(application) =
                function.materialization.context()
            else {
                panic!("register<T> has a persistent callable application")
            };
            application
        })
        .collect::<HashSet<_>>();
    assert_eq!(enclosing_applications.len(), 2);

    let source_registration = output.export.callback_registration_identities.records()[0].id();
    let callback_applications = module
        .foreign_callback_registrations
        .iter()
        .map(|(_, registration)| {
            let record = module
                .callback_applications
                .get(registration.application)
                .expect("a concrete callback references its canonical application record");
            assert_eq!(record.key().registration(), source_registration);
            let hir::concrete::CallableMaterializationContext::Application(enclosing) =
                record.key().context()
            else {
                panic!("a callback in register<T> inherits its callable application")
            };
            assert!(enclosing_applications.contains(&enclosing));
            registration.application
        })
        .collect::<HashSet<_>>();
    assert_eq!(callback_applications.len(), 2);
    assert_eq!(module.callback_applications.len(), 2);
}
