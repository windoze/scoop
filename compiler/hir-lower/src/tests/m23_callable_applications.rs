use std::collections::{HashMap, HashSet};

use scoop_identity::{CallableArguments, CallableInstantiationOwner, CallableTemplateOrigin};

use super::*;

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
