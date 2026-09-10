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
