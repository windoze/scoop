use std::collections::HashSet;

use scoop_identity::{CallableMaterializationContext, CallableTemplateOwner, GeneratedCallableKey};

use super::*;

fn reference(receiver: Option<Expr>, name: &str) -> Expr {
    Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: receiver.map(Box::new),
        name: ident(name),
        span: sp(),
    }
}

#[test]
fn param_free_reference_has_one_exact_invoke_materialization() {
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
    .expect("a param-free callable reference materializes");
    let module = &output.local;
    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("one concrete callable reference");
    let record = reference.identity.callable_record();
    assert!(matches!(
        record.key(),
        GeneratedCallableKey::CallableReferenceInvoke { .. }
    ));
    assert_eq!(
        reference.identity.materialization().template(),
        CallableTemplateOwner::Generated(record.id())
    );
    assert_eq!(
        reference.identity.materialization().context(),
        CallableMaterializationContext::NoSubstitution
    );
    assert_eq!(
        reference.identity.definition_path(),
        match record.key() {
            GeneratedCallableKey::CallableReferenceInvoke { path, .. } => path,
            _ => unreachable!(),
        }
    );
}

#[test]
fn generic_parent_applications_share_the_reference_template_but_not_its_materialization() {
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
    .expect("one callable-reference site materializes in two generic contexts");
    let module = &output.local;
    let parent_materializations = module
        .functions
        .iter()
        .filter_map(|(_, function)| (function.name == "bind").then_some(function.materialization))
        .collect::<HashSet<_>>();
    assert_eq!(parent_materializations.len(), 2);

    let references = module.callable_references.iter().collect::<Vec<_>>();
    assert_eq!(references.len(), 2);
    let generated_templates = references
        .iter()
        .map(|(_, reference)| reference.identity.callable_record().id())
        .collect::<HashSet<_>>();
    assert_eq!(generated_templates.len(), 1);
    let materializations = references
        .iter()
        .map(|(_, reference)| *reference.identity.materialization())
        .collect::<HashSet<_>>();
    assert_eq!(materializations.len(), 2);

    for (_, reference) in references {
        let materialization = *reference.identity.materialization();
        assert!(
            parent_materializations
                .iter()
                .any(|parent| parent.context() == materialization.context())
        );
        let CallableMaterializationContext::Application(application) = materialization.context()
        else {
            panic!("a reference inside a generic instance has an application context")
        };
        assert!(
            module
                .callable_applications
                .generated_body(application, reference.identity.callable_record().id(),)
                .is_some()
        );
    }
}
